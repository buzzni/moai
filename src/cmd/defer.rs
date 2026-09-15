//! 지금 안 할 일을 계획에서 잠시 뺀다.
//!
//! **칸을 옮기지 않는다.** 미루는 것은 `todo → deferred` 같은 이동이 아니다 —
//! 그 일이 어느 칸에 있었는지는 도로 집을 때 그대로 필요하다. 종류도 안
//! 바꾼다: 같은 줄이 그대로 돌아와야 한다.
//!
//! 여기도 승인은 없다. 무엇이든 미룰 수 있고 언제든 도로 집는다. 쌓인 것은
//! `moai status` 가 드러낸다.

use super::{Ctx, Fail, R};
use crate::cli::DeferArgs;
use crate::model::{self, Issue, JournalEntry};
use crate::store::Repo;
use crate::style::{self, paint};

#[derive(Default)]
struct Moved {
    /// 실제로 바뀐 것.
    done: Vec<Issue>,
    /// 이미 그 모양이던 것.
    already: Vec<String>,
    missing: Vec<String>,
    /// 도로 집으라 했는데 **아직 계획 밖인 것** — (그 줄, 실제로 미룬 줄).
    shelved: Vec<(String, Vec<String>)>,
    /// `--from` 을 걸었는데 그 사이 칸이 달라진 줄 — (그 줄, 지금 칸).
    stale: Vec<(String, crate::model::Status)>,
}

pub fn run(ctx: &Ctx, args: DeferArgs) -> R<Vec<String>> {
    let repo = Repo::discover()?;
    let at = model::now();
    let by = model::actor(ctx.user.as_deref())?;
    let back = args.undo;
    // **빈 까닭은 안 적는다.** `moai note` 가 같은 자리에서 거절하는데 여기만
    // 받으면, 이력에 내용 없는 `note:` 줄이 부를 때마다 하나씩 쌓인다.
    let msg = args.msg.as_deref().map(str::trim).filter(|m| !m.is_empty());
    if args.msg.is_some() && msg.is_none() {
        return Err(Fail::new("까닭이 비었다"));
    }
    // **모르는 칸은 거절한다** — `mv --from` 과 같은 자다. 오타를 "안 맞았다" 로
    // 읽으면 아무것도 안 하면서 0 아닌 코드만 내, 부르는 쪽이 까닭을 못 읽는다.
    let from = args.from.map(crate::model::Status::new);
    if let Some(f) = &from {
        repo.config.require_known(f.as_str()).map_err(|e| Fail::coded(e, super::code::BAD_STATUS))?;
    }

    let (moved, read): (Moved, super::Read) = repo.with_write(|issues, cfg, _| {
        let mut m = Moved::default();
        let mut entries = Vec::new();
        for id in &args.ids {
            // #a-partial: 하나가 없다고 나머지를 안 미루지 않는다.
            let Some(i) = issues.iter_mut().find(|i| &i.id == id) else {
                m.missing.push(id.clone());
                continue;
            };
            // **본 칸이 그대로일 때만 손댄다.** 락 안에서 다시 읽은 줄로 잰다 —
            // 옆에서 집어 일하기 시작한 줄을 뒤늦은 미루기가 계획 밖으로 빼면,
            // 일하던 쪽은 제 일이 보드에서 사라진 까닭을 어디서도 못 읽는다.
            // 이미 그 모양인지보다 **먼저** 본다(`mv` 와 같은 차례다).
            if from.as_ref().is_some_and(|f| &i.status != f) {
                m.stale.push((i.id.clone(), i.status.clone()));
                continue;
            }
            if i.is_deferred() == !back {
                // **시각을 밀지 않는다.** 밀면 "언제부터 미뤄 뒀나" 가 마지막
                // 으로 명령을 친 때가 되고, `status` 의 나이가 거짓말한다.
                // 적어 온 말은 그래도 버리지 않는다 — 되풀이해 부르는 것이
                // 흔하고, 그때 이유가 조용히 사라지면 저널을 못 믿게 된다.
                if let Some(msg) = msg {
                    entries.push(JournalEntry::note(&i.id, msg, &at, &by));
                }
                m.already.push(i.id.clone());
                continue;
            }
            i.deferred_at = (!back).then(|| at.clone());
            // 도로 집으면 `deferred_at` 이 지워져 시각이 안 남는다 — 겹쳐 볼 때 견줄 시각은 여기
            // 둔다(`Issue::planned_at`).
            i.planned_at = Some(at.clone());
            i.updated_at = at.clone();
            // **필드 변경은 저널에 안 적는다** (CLAUDE.md). 저널을 접어야
            // 답이 나오는 물음이 생기면 그 답은 스냅샷의 필드가 되어야 하고,
            // 여기서는 이미 `deferred_at` 이 그 필드다.
            if let Some(msg) = msg {
                entries.push(JournalEntry::note(&i.id, msg, &at, &by));
            }
            i.normalize();
            m.done.push(i.clone());
        }
        // **물려받은 미룸은 제 줄을 풀어도 안 풀린다.** 미룬 에픽의 멤버에 `--undo`
        // 를 치면 제 `deferred_at` 이 없어 "이미 계획에 있다" 로 끝났는데, 실제로는
        // 그 에픽 때문에 여전히 빠져 있었다(moai-kluk). 제 줄을 풀었어도 에픽이 아직
        // 미뤄져 있으면 같다. **쓰기를 마친 모습에서** 재야 에픽과 멤버를 한 번에 푼
        // 경우를 헛되이 안 센다.
        if back && !(m.done.is_empty() && m.already.is_empty()) {
            let roots = crate::report::deferred_sources(issues);
            m.shelved = m
                .done
                .iter()
                .map(|i| i.id.as_str())
                .chain(m.already.iter().map(String::as_str))
                .filter_map(|id| roots.get(id).map(|r| (id.to_string(), r.iter().map(|s| s.to_string()).collect())))
                .collect();
            m.already.retain(|id| !roots.contains_key(id.as_str()));
        }
        // 묶음도 미룬다 — 그 줄을 내면서 적힌 칸을 그대로 내면 받는 쪽이
        // 안 읽히는 칸을 읽는다(`cmd::Row`).
        let ids: Vec<&str> = m.done.iter().map(|i| i.id.as_str()).collect();
        let read = super::read_of(issues, cfg, &ids);
        Ok((entries, (m, read)))
    })?;

    for id in &moved.missing {
        super::note_partial();
        eprintln!("moai: {id} 를 못 찾았다");
    }
    // 진 줄은 못 찾은 줄과 같은 표면 하나(stderr)와 같은 종료 코드로 선다 —
    // `mv --from` 과 한 자다. 나머지 id 는 그대로 처리한다.
    for (id, now) in &moved.stale {
        super::note_partial();
        eprintln!("moai: {id} 는 이미 {now} 다 — 그대로 뒀다");
    }

    if ctx.json {
        // **바뀐 것만 내면 나머지를 말할 자리가 없다.** 사람 출력에는 있는데
        // 기계 출력에만 없으면 받는 쪽이 두 표면 중 하나를 못 믿게 된다.
        #[derive(serde::Serialize)]
        struct Stale<'a> {
            id: &'a str,
            /// 락 안에서 본 지금 칸. 함께 주지 않으면 진 쪽이 한 번 더 물어야 한다.
            status: &'a str,
        }
        #[derive(serde::Serialize)]
        struct Out<'a> {
            deferred: bool,
            changed: Vec<super::Row<'a>>,
            already: &'a [String],
            missing: &'a [String],
            /// `--from` 에 걸려 그대로 둔 줄. 사람 쪽의 stderr 한 줄과 같은 것이다.
            stale: Vec<Stale<'a>>,
            /// 도로 집으라 했는데 아직 계획 밖인 것과, 실제로 도로 집어야 할 줄.
            shelved: Vec<super::Shelved<'a>>,
        }
        return super::json_line(&Out {
            deferred: !back,
            changed: moved.done.iter().map(|i| super::Row::from(i, &read)).collect(),
            already: &moved.already,
            missing: &moved.missing,
            stale: moved.stale.iter().map(|(id, now)| Stale { id, status: now.as_str() }).collect(),
            shelved: super::shelved(&moved.shelved),
        });
    }

    let word = if back { "도로 집음" } else { "미룸" };
    let mut out: Vec<String> = moved
        .done
        .iter()
        .map(|i| {
            format!(
                "{}  {}   {}",
                paint(style::ID, &i.id),
                paint(if back { style::PLAIN } else { style::DIM }, word),
                paint(style::DIM, &i.title),
            )
        })
        .collect();
    for id in &moved.already {
        out.push(format!(
            "{}  {}",
            paint(style::ID, id),
            paint(style::DIM, if back { "이미 계획에 있다" } else { "이미 미뤄 뒀다" })
        ));
    }
    // **아직 계획 밖이면 도로 집을 줄을 댄다.** 제 줄을 풀었든 원래 안 미뤘든,
    // 미룬 에픽·부모 밑이면 그 줄을 도로 집어야 풀린다.
    for (id, root) in &moved.shelved {
        out.push(format!(
            "{}  {}",
            paint(style::ID, id),
            paint(style::DIM, &crate::view::shelved_by(root))
        ));
    }
    Ok(out)
}
