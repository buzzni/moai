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

    let at = model::now();
    let by = model::actor(ctx.user.as_deref())?;
    let moved: Moved = repo.with_write(|issues, _, _| {
        let mut m = Moved::default();
        let mut entries = Vec::new();
        for id in ids {
            // #a-partial: 하나가 없다고 나머지를 안 옮기지 않는다.
            let Some(i) = issues.iter_mut().find(|i| &i.id == id) else {
                m.missing.push(id.clone());
                continue;
            };
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
        Ok((entries, m))
    })?;

    for id in &moved.missing {
        super::note_partial();
        eprintln!("moai: {id} 를 못 찾았다");
    }

    if ctx.json {
        // **옮긴 것만 내면 나머지를 말할 자리가 없다.** 이미 그 칸이던 것과
        // 못 찾은 것은 사람 출력에는 있는데 기계 출력에만 없으면, 받는 쪽이
        // 두 표면 중 하나를 못 믿게 된다.
        #[derive(serde::Serialize)]
        struct Out<'a> {
            moved: Vec<&'a Issue>,
            already: &'a [String],
            missing: &'a [String],
        }
        return super::json_line(&Out {
            moved: moved.done.iter().map(|(i, _)| i).collect(),
            already: &moved.already,
            missing: &moved.missing,
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
    // **미뤄 둔 줄을 옮겼으면 말한다.** 칸은 옮겨졌는데 그 줄은 보드에도
    // `ready` 에도 안 나오므로, 말하지 않으면 집어 든 일이 통째로 안 보인다 —
    // 막지는 않는다. 도로 집는 말은 `defer --undo` 하나다.
    for (i, _) in moved.done.iter().filter(|(i, _)| i.is_deferred()) {
        out.push(format!(
            "{}  {}",
            paint(style::ID, &i.id),
            paint(style::DIM, "미뤄 둔 것이라 보드와 ready 에서는 빠져 있다 — `moai defer --undo`"),
        ));
    }
    Ok(out)
}
