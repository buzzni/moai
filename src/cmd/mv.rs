//! 상태를 옮긴다.
//!
//! **순서를 건너뛰어도, 되돌려도, 막지 않는다.** 이 도구에 승인은 없다 —
//! 되감긴 것과 오래 멈춘 것은 `moai status` 가 드러낸다. 여기서 막기
//! 시작하면 그게 게이트고, 이전 시도가 정확히 그것으로 죽었다.

use super::{Ctx, Fail, R};
use crate::cli::MvArgs;
use crate::model::{self, Issue, JournalEntry, Status};
use crate::store::Repo;
use crate::style::{self, paint};

#[derive(Default)]
struct Moved {
    /// 옮긴 이슈와 그것이 있던 칸.
    done: Vec<(Issue, Status)>,
    /// 이미 그 칸에 있던 것.
    already: Vec<String>,
    missing: Vec<String>,
    /// `--from` 을 걸었는데 그 사이 칸이 달라진 줄 — (그 줄, 지금 칸).
    stale: Vec<(String, String)>,
    /// 옮긴 것 중 계획에서 빠진 것 — (그 줄, 실제로 미룬 줄).
    shelved: Vec<(String, Vec<String>)>,
    /// 옮기려 한 묶음 → 멤버에서 읽은 칸. 적힌 칸은 어디서도 안 읽힌다.
    read: super::Read,
    /// 그 가운데 **끝난 멤버가 있는** 묶음 — 남은 멤버를 미뤄 접히는 것.
    finished: std::collections::BTreeSet<String>,
    /// `done` 으로 옮겨 **이로써 집을 수 있게 된 일**(moai-942k, `report::unblocked`).
    unblocked: Vec<Issue>,
    /// 그 쓰기로 **이제 닫을 수 있게 된 부모**(moai-j4xs, `report::Freed::closable`).
    closable: Vec<Issue>,
    /// 닫은 줄과 **같은 에픽에서 다음에 집을 것**(moai-j4xs, `report::Freed::next`).
    next: Vec<Issue>,
}

pub fn run(ctx: &Ctx, args: MvArgs) -> R<Vec<String>> {
    let repo = Repo::discover()?;
    if args.args.len() < 2 {
        return Err(Fail::coded(
            // **두 줄은 키 둘이다.** 말묶음의 값은 한 줄이라(`i18n` 의
            // `every_translation_keeps_the_places_english_marks`) 줄 나눔은 부르는 쪽이 짓는다.
            format!(
                "{}\n      {}",
                crate::i18n::fill(crate::i18n::say(ctx.lang(), "mv.no_status"), &[("ids", &args.args.join(" "))]),
                crate::i18n::fill(
                    crate::i18n::say(ctx.lang(), "mv.no_status_columns"),
                    &[("columns", &repo.config.statuses.join(", "))]
                ),
            ),
            super::code::BAD_STATUS,
        ));
    }
    let (ids, tail) = args.args.split_at(args.args.len() - 1);
    let to = Status::new(tail[0].clone());

    repo.config
        .require_known(to.as_str())
        .map_err(|e| Fail::coded(crate::view::no_such_column(ctx.lang(), &e), super::code::BAD_STATUS))?;
    let from = args.from.map(Status::new);

    // **누구인지는 락 밖에서 묻는다.** `model::actor` 는 `git` 을 두 번 띄운다 — 그것을
    // 락 안에 두면 같은 `.moai` 를 쓰는 옆 세션들이 그 subprocess 만큼 더 기다린다.
    // **말하는 차례는 그대로다**: 결과를 여기서 펴지 않고 아래 칸 검사 뒤에 편다.
    let who = model::actor(ctx.user.as_deref(), &repo.root);
    let moved: Moved = repo.with_write(|issues, cfg, _| {
        // **시각은 락을 쥔 뒤에 뜬다**(리뷰 moai-u5bk.3wq). 밖에서 뜨면 먼저 뜨고 늦게 락을 잡은
        // 쪽이 뒤에 써서, 칸 시각이 거꾸로 가고 안 덮이는 시작이 끝보다 늦게 선다 — 집기가 닫기를
        // 앞질러 `done_at − started_at` 이 음수가 된다. 락 안에서 뜨면 쓰는 차례가 곧 시각의 차례다.
        let at = model::now();
        // **칸부터 다 보고 누구인지는 그다음이다.** 신원 없는 기계에서 칸 오타가 "누가
        // 하는지 모른다" 로 덮이면, 부르는 쪽은 둘을 글로만 가를 수 있다. 칸 검사가
        // 줄을 봐야 하므로(`check_from`) 락 안에서 잰다. **`bad_status` 를 내는 검사는
        // 하나도 빠짐없이 `who?` 위에 선다** — 하나라도 아래로 내려가면 그 오타만
        // `no_actor` 로 덮여, 같은 자의 잘못이 명령마다 다른 `code` 로 나간다.
        // 말은 **거절할 때만** 푼다 — `ctx.lang()` 을 인자로 넘기면 락 안에서 사용자 설정을
        // 여는 일이 오타 없는 판마다 선다(리뷰). 위의 `require_known` 과 한 모양이다.
        super::check_from(from.as_ref().map(Status::as_str), issues, cfg)
            .map_err(|e| Fail::coded(crate::view::no_such_column(ctx.lang(), &e), super::code::BAD_STATUS))?;
        // **묶음은 화면이 보여 준 칸으로 잰다**(사람이 정했다, moai-o5ss.l07). 에픽·
        // 마일스톤의 칸은 멤버에서 읽히고 줄에 적힌 칸은 어디서도 안 읽히므로, 적힌 칸과
        // 견주면 보드가 `in_progress` 를 그리는 에픽에 `--from in_progress` 가 "이미
        // todo 다" 로 떨어진다. 묶음인지를 가르는 `if` 는 `report` 것이고 여기서는 그
        // 답을 나른다(`standing_of`).
        //
        // **돌기 전에 한 번 뜬다.** `--from` 이 재는 것은 부르는 쪽이 본 칸이지 이
        // 명령이 만든 칸이 아니다 — 돌면서 그때그때 보면 같은 id 를 두 번 적은 한
        // 명령이 제가 방금 쓴 값과 겨뤄, 옮겨 놓고도 진다.
        let asked_all: Vec<&str> = ids.iter().map(String::as_str).collect();
        // **묶음에는 `--from` 을 못 쓴다**(사람이 정했다, moai-8xwi.rzg). 묶음의 칸은
        // 멤버에서 읽고 쓰기는 줄에 적힌 칸에 한다 — 두 축이 갈려 있어, 재는 것이 맞아도
        // 쓰는 것은 아무도 안 지킨다. 겨루는 둘이 같은 에픽에 같은 `--from` 을 걸면 둘 다
        // 이겼다고 믿는다. 먹는 척하는 가드보다 없는 가드가 정직하다.
        if from.is_some()
            && let Some(g) = issues.iter().find(|i| asked_all.contains(&i.id.as_str()) && crate::report::is_group(i))
        {
            return Err(Fail::coded(
                format!(
                    "{}\n      {}",
                    crate::i18n::fill(crate::i18n::say(ctx.lang(), "mv.group_no_from"), &[("id", &g.id)]),
                    crate::i18n::say(ctx.lang(), "mv.group_no_from_how"),
                ),
                super::code::BAD_STATUS,
            ));
        }
        let by = who?;
        let mut m = Moved::default();
        let mut entries = Vec::new();
        // **닫을 때만 전의 모습을 뜬다.** 풀리는 것은 끝낼 때뿐이다 — 다른 칸으로의 이동은
        // 막음을 풀지 않고, 락을 쥔 채 목록을 한 벌 더 복사하는 값은 그때만 치른다.
        // 거절이 다 끝난 자리에서 뜬다 — 위에서 물러날 판에 한 벌 베끼지 않는다.
        let before = to.is_done().then(|| issues.clone());
        let seen: super::Read =
            if from.is_some() { super::standing_of(issues, cfg, &asked_all) } else { Default::default() };
        for id in ids {
            // #a-partial: 하나가 없다고 나머지를 안 옮기지 않는다.
            let Some(i) = issues.iter_mut().find(|i| &i.id == id) else {
                m.missing.push(id.clone());
                continue;
            };
            // **본 칸이 그대로일 때만 옮긴다.** 락 안에서 다시 읽은 줄로 재므로,
            // `ready` 와 이 자리 사이에 옆 에이전트가 집고 닫기까지 했어도 여기서
            // 갈린다. 이미 갈 칸에 있는 것보다 **먼저** 본다 — 남이 옮겨 둔 것을
            // "이미 그 칸" 으로 읽으면 진 쪽이 이겼다고 믿는다.
            if let Some(f) = &from {
                let stands = seen.get(&i.id).map(String::as_str).unwrap_or(i.status.as_str());
                if stands != f.as_str() {
                    m.stale.push((i.id.clone(), stands.to_string()));
                    continue;
                }
            }
            if i.status == to {
                // 옮길 것이 없어도 **적어 온 말은 버리지 않는다.** 되풀이해
                // 부르는 것(재시도·다른 에이전트가 먼저 옮긴 뒤)이 흔하고,
                // 그때 이유가 조용히 사라지면 저널을 믿을 수 없게 된다.
                //
                // **`--from` 에 진 줄은 이 자리에 못 온다** — 그쪽은 위에서 갈렸고
                // 말도 함께 버린다(사람이 정했다). 여기 오는 것은 *제가* 이미
                // 옮겨 둔 줄이고, 저기서 걸리는 것은 *남이* 옮긴 줄이다. 손대지
                // 않기로 한 줄의 이력에 메모만 남기면 그 줄에 무슨 일이 있었는지가
                // 거꾸로 읽힌다.
                if let Some(msg) = &args.msg {
                    entries.push(JournalEntry::note(&i.id, msg, &at, &by));
                }
                m.already.push(i.id.clone());
                continue;
            }
            // **`from`(`--from` 의 칸)과 이름이 갈려야 한다.** 둘 다 "떠나는 칸" 이라
            // 같은 이름을 쓰면 위의 검사와 이 줄이 한 값처럼 읽힌다 — 하나는 부르는
            // 쪽이 본 칸이고, 이것은 방금 락 안에서 읽은 칸이다.
            //
            // 칸과 그 시각들 — **시작·끝 시각**(moai-38mh)까지 — 은 `Issue::move_to` 가 한 번에
            // 옮긴다. 저널을 접어 세면 저널만 못 적힌 쓰기에서 조용히 틀리므로 이 쓰기에 싣고,
            // `idea promote` 도 같은 길이라 어느 동사로 닫든 같은 줄이 선다.
            let was = i.move_to(to.clone(), &at, cfg);
            entries.push(JournalEntry::status(&i.id, &was, &to, args.msg.clone(), &at, &by));
            // 저장 직전의 모습으로 맞춰 두고 뜬다 — 안 그러면 `--json` 이
            // 파일에 없는 값(기본 우선순위, 정렬 전 태그)을 말한다.
            i.normalize();
            m.done.push((i.clone(), was));
        }
        // **물려받은 미룸도 여기서 잰다.** 미룬 에픽의 멤버를 집으면 칸은
        // 옮겨져도 보드·`ready`·훅의 초점에서 빠진다 — 말하지 않으면 방금 집은
        // 일을 훅이 "집은 것 없음" 으로 막는 까닭이 아무 데도 없다.
        // 옮긴 것이 없으면 재지 않는다 — 락을 쥔 채 저장소 전체를 걷는 자리다.
        if !m.done.is_empty() {
            let roots = crate::report::deferred_sources(issues);
            m.shelved = m
                .done
                .iter()
                .filter_map(|(i, _)| {
                    roots.get(i.id.as_str()).map(|r| (i.id.clone(), r.iter().map(|s| s.to_string()).collect()))
                })
                .collect();
        }
        // **묶음을 옮기려 했으면 서 있는 칸을 잰다.** 막지 않는다 — 쓰기는 한다.
        // 다만 그 칸은 멤버에서 읽히므로(moai-j3b3), 말하지 않으면 옮긴 사람은
        // 에픽이 닫힌 줄 알고 화면은 계속 `in_progress` 를 그린다. 못 찾은 id 는
        // 저절로 빠진다 — `read_of` 가 있는 묶음만 고른다.
        // **진 줄은 여기 안 든다.** 안 옮긴 묶음에까지 "서 있는 칸은 …" 안내를 붙이면
        // 한 숨에 두 칸을 말한다 — stderr 는 "이미 todo 다", stdout 은 "서 있는 칸은
        // in_progress". 그리고 그 안내의 뒷말("계획에서 빼려면 `moai defer`")은
        // 일어나지도 않은 이동을 두고 다음 수를 댄다.
        // **덜어 세지 않고 실제로 손댄 줄에서 센다.** 뺄셈으로 적으면 "옮기지 않은"
        // 통이 하나 더 생기는 날 그것이 저절로 다시 끼어든다.
        let asked: Vec<&str> =
            m.done.iter().map(|(i, _)| i.id.as_str()).chain(m.already.iter().map(String::as_str)).collect();
        m.read = super::read_of(issues, cfg, &asked);
        // 접는 길이 갈리는 자리 — `report` 가 정하고 여기서는 그 답을 나른다.
        m.finished = issues
            .iter()
            .filter(|g| m.read.contains_key(&g.id) && crate::report::has_finished_member(issues, g))
            .map(|g| g.id.clone())
            .collect();
        // 이 쓰기가 연 것 셋. 옮긴 것이 없으면 연 것도 없다. 판단은 `report` 가 한다.
        if let Some(before) = before.filter(|_| !m.done.is_empty()) {
            let closed: Vec<&str> = m.done.iter().map(|(i, _)| i.id.as_str()).collect();
            let opened = crate::report::freed(&before, issues, cfg, &closed);
            m.unblocked = opened.unblocked.into_iter().cloned().collect();
            m.closable = opened.closable.into_iter().cloned().collect();
            m.next = opened.next.into_iter().cloned().collect();
        }
        Ok((entries, m))
    })?;

    for id in &moved.missing {
        super::note_partial();
        eprintln!("moai: {}", crate::i18n::fill(crate::i18n::say(ctx.lang(), "refuse.not_found"), &[("id", id)]));
    }
    // **진 집기도 못 찾은 줄과 같은 자리다.** 종료 코드로 갈려야 jq 없는 껍데기가
    // 이긴 쪽과 진 쪽을 가른다 — 여기서 실패로 끝내지는 않는다(나머지 id 는 옮겼다).
    // 못 찾은 줄과 **같은 표면 하나**에만 적는다 — 한때 stdout 에도 같은 말을 얹어,
    // 터미널에서 한 줄이 두 번 떴다.
    for (id, now) in &moved.stale {
        super::note_partial();
        eprintln!("moai: {}", crate::i18n::fill(crate::i18n::say(ctx.lang(), "mv.stale"), &[("id", id), ("now", now)]));
    }

    if ctx.json {
        // **옮긴 것만 내면 나머지를 말할 자리가 없다.** 이미 그 칸이던 것과
        // 못 찾은 것은 사람 출력에는 있는데 기계 출력에만 없으면, 받는 쪽이
        // 두 표면 중 하나를 못 믿게 된다.
        #[derive(serde::Serialize)]
        struct Stands<'a> {
            id: &'a str,
            derived_status: &'a str,
        }
        #[derive(serde::Serialize)]
        struct Out<'a> {
            moved: Vec<super::Row<'a>>,
            already: &'a [String],
            missing: &'a [String],
            /// `--from` 에 걸려 안 옮긴 줄. 사람 쪽의 stderr 한 줄과 같은 것이다.
            stale: Vec<super::Stale<'a>>,
            /// 옮겼어도 계획 밖인 것과 도로 집을 줄. 사람 출력의 안내와 같은 것이다.
            shelved: Vec<super::Shelved<'a>>,
            /// 옮기려 한 묶음 가운데 **서 있는 칸이 적은 칸과 다른 것.** 사람 출력의
            /// 한 줄과 같은 것이다 — 이미 그 칸이던 묶음은 `already` 에 id 뿐이라,
            /// 여기 없으면 되풀이해 부른 쪽만 그 칸을 모른다.
            stands: Vec<Stands<'a>>,
            /// 이로써 집을 수 있게 된 일. **늘 싣는다** — 없으면 `[]`. 다른 목록 키와 같은
            /// 모양이라 받는 쪽이 키가 있는지 가르지 않는다.
            unblocked: Vec<super::Row<'a>>,
            /// 이제 닫을 수 있게 된 부모. `unblocked` 와 같은 약속으로 **늘 싣는다**.
            closable: Vec<super::Row<'a>>,
            /// 같은 에픽에서 다음에 집을 것. `unblocked` 와 같은 약속으로 **늘 싣는다**.
            next: Vec<super::Row<'a>>,
        }
        return super::json_line(&Out {
            moved: moved.done.iter().map(|(i, _)| super::Row::from(i, &moved.read)).collect(),
            already: &moved.already,
            missing: &moved.missing,
            stale: super::stale(&moved.stale),
            shelved: super::shelved(&moved.shelved),
            stands: moved
                .read
                .iter()
                .filter(|(_, col)| col.as_str() != to.as_str())
                .map(|(id, col)| Stands { id, derived_status: col })
                .collect(),
            unblocked: moved.unblocked.iter().map(|i| super::Row::of(i, None)).collect(),
            closable: moved.closable.iter().map(|i| super::Row::of(i, None)).collect(),
            next: moved.next.iter().map(|i| super::Row::of(i, None)).collect(),
        });
    }

    let mut out: Vec<String> = moved
        .done
        .iter()
        // **`was` 다 — `from` 이 아니다.** 바깥에 `--from` 의 칸을 쥔 `from` 이 서 있어,
        // 같은 이름을 쓰면 여기서 그것을 가리고 둘이 한 값처럼 읽힌다. 하나는 부르는
        // 쪽이 본 칸이고, 이것은 락 안에서 실제로 떠나온 칸이다.
        .map(|(i, was)| {
            format!(
                "{}  {} → {}   {}",
                paint(style::ID, &i.id),
                paint(style::status_style(was.as_str()), was.as_str()),
                paint(style::status_style(to.as_str()), to.as_str()),
                paint(style::DIM, &i.title),
            )
        })
        .collect();
    for id in &moved.already {
        let said = crate::i18n::fill(crate::i18n::say(ctx.lang(), "mv.already"), &[("to", to.as_str())]);
        out.push(format!("{}  {}", paint(style::ID, id), paint(style::DIM, &said)));
    }
    // 묶음의 칸이 적은 칸과 다르면 한 줄. 같으면 말하지 않는다 — 멤버가 다 끝난
    // 에픽을 `done` 에 두는 것은 틀린 일이 아니다. 접는 길은 `view` 가 고른다.
    for (id, col) in moved.read.iter().filter(|(_, col)| col.as_str() != to.as_str()) {
        out.push(format!(
            "{}  {}",
            paint(style::ID, id),
            paint(
                style::DIM,
                &crate::view::group_moved(id, col, to.is_done(), moved.finished.contains(id), ctx.lang())
            )
        ));
    }
    // **미뤄 둔 줄을 옮겼으면 말한다.** 칸은 옮겨졌는데 그 줄은 보드에도
    // `ready` 에도 안 나오므로, 말하지 않으면 집어 든 일이 통째로 안 보인다 —
    // 막지는 않는다. 도로 집는 말은 `defer --undo` 하나다.
    // **닫은 줄에는 안 붙인다.** 끝난 일은 보드에도 `ready` 에도 원래 안
    // 나오므로 미뤘다는 것이 더는 그 줄이 안 보이는 까닭이 아니고, `--undo`
    // 는 끝난 일을 계획에 도로 넣으라는 엉뚱한 말이 된다 — `status` 의
    // `미뤄 둔 것` 줄도 같은 자로 닫힌 것을 뺀다.
    // 제가 미룬 줄이면 그 줄이, 물려받았으면 미룬 곳이 도로 집을 줄이다 — 말은
    // `view::shelved_by` 하나다. 한때 제 줄 쪽 안내만 id 없는 `moai defer --undo`
    // 를 대, 그대로 치면 clap 이 인자가 없다며 거절했다.
    for (id, root) in &moved.shelved {
        let said = crate::view::shelved_by(root, ctx.lang());
        out.push(format!("{}  {}", paint(style::ID, id), paint(style::DIM, &said)));
    }
    // **이 쓰기가 연 것은 한 줄씩.** 없으면 말하지 않는다 — 출력이 전과 같다.
    out.extend(crate::view::freed_lines(&moved.unblocked, &moved.closable, &moved.next, ctx.lang()));
    Ok(out)
}
