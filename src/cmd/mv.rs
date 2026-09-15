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
    stale: Vec<(String, Status)>,
    /// 옮긴 것 중 계획에서 빠진 것 — (그 줄, 실제로 미룬 줄).
    shelved: Vec<(String, Vec<String>)>,
    /// 옮기려 한 묶음 → 멤버에서 읽은 칸. 적힌 칸은 어디서도 안 읽힌다.
    read: super::Read,
    /// 그 가운데 **끝난 멤버가 있는** 묶음 — 남은 멤버를 미뤄 접히는 것.
    finished: std::collections::BTreeSet<String>,
    /// `done` 으로 옮겨 **이로써 집을 수 있게 된 일**(moai-942k, `report::unblocked`).
    unblocked: Vec<Issue>,
}

pub fn run(ctx: &Ctx, args: MvArgs) -> R<Vec<String>> {
    let repo = Repo::discover()?;
    if args.args.len() < 2 {
        return Err(Fail::coded(
            format!(
                "옮길 칸을 안 적었다 — `moai mv {} <상태>`\n      있는 칸: {}",
                args.args.join(" "),
                repo.config.statuses.join(", ")
            ),
            super::code::BAD_STATUS,
        ));
    }
    let (ids, tail) = args.args.split_at(args.args.len() - 1);
    let to = Status::new(tail[0].clone());

    repo.config.require_known(to.as_str()).map_err(|e| Fail::coded(e, super::code::BAD_STATUS))?;
    // **`--from` 의 오타는 옮길 칸과 같은 자로 거절한다.** 모르는 칸을 그냥 "안 맞았다"
    // 로 읽으면 아무것도 안 옮기면서 0 아닌 코드만 내, 에이전트가 영영 아무 일도 못
    // 집는 까닭이 어디에도 안 남는다.
    let from = args.from.map(Status::new);
    if let Some(f) = &from {
        repo.config.require_known(f.as_str()).map_err(|e| Fail::coded(e, super::code::BAD_STATUS))?;
    }

    let at = model::now();
    let by = model::actor(ctx.user.as_deref())?;
    let moved: Moved = repo.with_write(|issues, cfg, _| {
        let mut m = Moved::default();
        let mut entries = Vec::new();
        // **닫을 때만 전의 모습을 뜬다.** 풀리는 것은 끝낼 때뿐이다 — 다른 칸으로의 이동은
        // 막음을 풀지 않고, 락을 쥔 채 목록을 한 벌 더 복사하는 값은 그때만 치른다.
        let before = to.is_done().then(|| issues.clone());
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
                if &i.status != f {
                    m.stale.push((i.id.clone(), i.status.clone()));
                    continue;
                }
            }
            if i.status == to {
                // 옮길 것이 없어도 **적어 온 말은 버리지 않는다.** 되풀이해
                // 부르는 것(재시도·다른 에이전트가 먼저 옮긴 뒤)이 흔하고,
                // 그때 이유가 조용히 사라지면 저널을 믿을 수 없게 된다.
                if let Some(msg) = &args.msg {
                    entries.push(JournalEntry::note(&i.id, msg, &at, &by));
                }
                m.already.push(i.id.clone());
                continue;
            }
            let from = i.status.clone();
            entries.push(JournalEntry::status(&i.id, &from, &to, args.msg.clone(), &at, &by));
            i.status = to.clone();
            i.status_since = at.clone();
            i.updated_at = at.clone();
            // 저장 직전의 모습으로 맞춰 두고 뜬다 — 안 그러면 `--json` 이
            // 파일에 없는 값(기본 우선순위, 정렬 전 태그)을 말한다.
            i.normalize();
            m.done.push((i.clone(), from));
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
        let asked: Vec<&str> = ids.iter().map(String::as_str).collect();
        m.read = super::read_of(issues, cfg, &asked);
        // 접는 길이 갈리는 자리 — `report` 가 정하고 여기서는 그 답을 나른다.
        m.finished = issues
            .iter()
            .filter(|g| m.read.contains_key(&g.id) && crate::report::has_finished_member(issues, g))
            .map(|g| g.id.clone())
            .collect();
        // 이로써 풀린 일. 옮긴 것이 없으면 풀린 것도 없다. 판단은 `report` 가 한다.
        if let Some(before) = before.filter(|_| !m.done.is_empty()) {
            m.unblocked = crate::report::unblocked(&before, issues, cfg).into_iter().cloned().collect();
        }
        Ok((entries, m))
    })?;

    for id in &moved.missing {
        super::note_partial();
        eprintln!("moai: {id} 를 못 찾았다");
    }
    // **진 집기도 못 찾은 줄과 같은 자리다.** 종료 코드로 갈려야 jq 없는 껍데기가
    // 이긴 쪽과 진 쪽을 가른다 — 여기서 실패로 끝내지는 않는다(나머지 id 는 옮겼다).
    for (id, now) in &moved.stale {
        super::note_partial();
        eprintln!("moai: {id} 는 이미 {now} 다 — 안 옮겼다");
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
        struct Stale<'a> {
            id: &'a str,
            /// 락 안에서 본 지금 칸. **함께 주지 않으면 진 쪽이 한 번 더 물어야 한다.**
            status: &'a str,
        }
        #[derive(serde::Serialize)]
        struct Out<'a> {
            moved: Vec<super::Row<'a>>,
            already: &'a [String],
            missing: &'a [String],
            /// `--from` 에 걸려 안 옮긴 줄. 사람 출력의 한 줄과 같은 것이다.
            stale: Vec<Stale<'a>>,
            /// 옮겼어도 계획 밖인 것과 도로 집을 줄. 사람 출력의 안내와 같은 것이다.
            shelved: Vec<super::Shelved<'a>>,
            /// 옮기려 한 묶음 가운데 **서 있는 칸이 적은 칸과 다른 것.** 사람 출력의
            /// 한 줄과 같은 것이다 — 이미 그 칸이던 묶음은 `already` 에 id 뿐이라,
            /// 여기 없으면 되풀이해 부른 쪽만 그 칸을 모른다.
            stands: Vec<Stands<'a>>,
            /// 이로써 집을 수 있게 된 일. **늘 싣는다** — 없으면 `[]`. 다른 목록 키와 같은
            /// 모양이라 받는 쪽이 키가 있는지 가르지 않는다.
            unblocked: Vec<super::Row<'a>>,
        }
        return super::json_line(&Out {
            moved: moved.done.iter().map(|(i, _)| super::Row::from(i, &moved.read)).collect(),
            already: &moved.already,
            missing: &moved.missing,
            stale: moved.stale.iter().map(|(id, now)| Stale { id, status: now.as_str() }).collect(),
            shelved: super::shelved(&moved.shelved),
            stands: moved
                .read
                .iter()
                .filter(|(_, col)| col.as_str() != to.as_str())
                .map(|(id, col)| Stands { id, derived_status: col })
                .collect(),
            unblocked: moved.unblocked.iter().map(|i| super::Row::of(i, None)).collect(),
        });
    }

    let mut out: Vec<String> = moved
        .done
        .iter()
        .map(|(i, from)| {
            format!(
                "{}  {} → {}   {}",
                paint(style::ID, &i.id),
                paint(style::status_style(from.as_str()), from.as_str()),
                paint(style::status_style(to.as_str()), to.as_str()),
                paint(style::DIM, &i.title),
            )
        })
        .collect();
    for id in &moved.already {
        out.push(format!(
            "{}  {}",
            paint(style::ID, id),
            paint(style::DIM, &format!("이미 {to} 다"))
        ));
    }
    for (id, now) in &moved.stale {
        out.push(format!(
            "{}  {}",
            paint(style::ID, id),
            paint(style::DIM, &format!("이미 {now} 다 — 안 옮겼다"))
        ));
    }
    // 묶음의 칸이 적은 칸과 다르면 한 줄. 같으면 말하지 않는다 — 멤버가 다 끝난
    // 에픽을 `done` 에 두는 것은 틀린 일이 아니다. 접는 길은 `view` 가 고른다.
    for (id, col) in moved.read.iter().filter(|(_, col)| col.as_str() != to.as_str()) {
        out.push(format!(
            "{}  {}",
            paint(style::ID, id),
            paint(
                style::DIM,
                &crate::view::group_moved(id, col, to.is_done(), moved.finished.contains(id))
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
        out.push(format!(
            "{}  {}",
            paint(style::ID, id),
            paint(style::DIM, &crate::view::shelved_by(root))
        ));
    }
    // **이로써 풀린 일은 한 줄.** 없으면 말하지 않는다 — 출력이 전과 같다.
    out.extend(crate::view::unblocked_line(&moved.unblocked));
    Ok(out)
}
