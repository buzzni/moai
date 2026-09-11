//! `blocked_by` 를 고친다.
//!
//! **막히는 쪽에 쓴다.** `moai link A --blocks B` 는 A 가 B 를 막는다는
//! 뜻이고, 그것을 `B.blocked_by` 에 A 를 더하는 것으로 적는다. 그래야 A 를
//! 닫을 때 A 줄만 쓰면 된다.

use super::{Ctx, Fail, R};
use crate::cli::LinkArgs;
use crate::model::{self, Issue};
use crate::report::creates_cycle;
use crate::store::Repo;
use crate::style::{self, paint};

pub fn run(ctx: &Ctx, args: LinkArgs) -> R<Vec<String>> {
    if args.blocks.is_empty() && args.unblocks.is_empty() {
        return Err(Fail::new("`--blocks` 나 `--unblocks` 를 적는다"));
    }
    // 같은 id 가 둘 다에 있으면 어느 쪽이 이기는지 조용히 정하지 않는다.
    if let Some(dup) = args.blocks.iter().find(|b| args.unblocks.contains(b)) {
        return Err(Fail::new(format!("{dup} 를 막으면서 동시에 막음을 없앨 수는 없다")));
    }
    let repo = Repo::discover()?;
    let at = model::now();

    // `(대상, 막을지 없앨지)`. 어느 쪽에서 왔는지를 갖고 있어야, 다시
    // `args.blocks.contains` 로 되짚어 묻다가 판단을 두 번 다르게 하는
    // 일이 없다.
    let edits: Vec<(String, bool)> = args
        .blocks
        .iter()
        .map(|t| (t.clone(), true))
        .chain(args.unblocks.iter().map(|t| (t.clone(), false)))
        .collect();

    let touched: Vec<(Issue, bool)> = repo.with_write(|issues, cfg| {
        // 막는 쪽의 존재는 **더할 때만** 따진다. `--unblocks` 만이면 그것이
        // 이미 지워졌을 수 있고, 그때도 남은 참조는 풀려야 한다 — 아니면
        // `status` 가 드러낸 끊긴 참조를 손으로 파일을 고쳐야만 없앨 수 있다.
        if !args.blocks.is_empty() && !issues.iter().any(|i| i.id == args.id) {
            return Err(Fail::not_found(&args.id));
        }
        for (target, wants_block) in &edits {
            if !issues.iter().any(|i| &i.id == target) {
                return Err(Fail::not_found(target));
            }
            if *wants_block && creates_cycle(issues, &args.id, target) {
                return Err(Fail::coded(
                    format!(
                        "{} 가 {target} 를 막으면 고리가 된다 — {target} 는 이미 (곧바로든 건너서든) {} 를 막고 있다",
                        args.id, args.id
                    ),
                    super::code::BAD_INPUT,
                ));
            }
        }

        let mut out = Vec::new();
        for (target, wants_block) in edits {
            let t = issues.iter_mut().find(|i| i.id == target).expect("위에서 존재를 확인했다");
            let has = t.blocked_by.contains(&args.id);
            if wants_block == has {
                continue; // 이미 원하는 모양이다
            }
            if wants_block {
                t.blocked_by.push(args.id.clone());
            } else {
                t.blocked_by.retain(|b| b != &args.id);
            }
            t.updated_at = at.clone();
            t.normalize();
            t.validate(cfg)?;
            out.push((t.clone(), wants_block));
        }
        Ok((vec![], out))
    })?;

    if ctx.json {
        let issues_only: Vec<&Issue> = touched.iter().map(|(i, _)| i).collect();
        return super::json_line(&issues_only);
    }
    if touched.is_empty() {
        return Ok(vec![format!(
            "{}  {}",
            paint(style::ID, &args.id),
            paint(style::DIM, "바뀐 것이 없다")
        )]);
    }
    Ok(touched
        .iter()
        .map(|(t, wants_block)| {
            format!(
                "{}  {}  {}",
                paint(style::ID, &args.id),
                paint(style::DIM, if *wants_block { "→ 막는다 →" } else { "→ 막음을 없앤다 →" }),
                paint(style::ID, &t.id),
            )
        })
        .collect())
}
