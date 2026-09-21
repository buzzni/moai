//! `blocked_by` 를 고친다.
//!
//! **막히는 쪽에 쓴다.** `moai link A --blocks B` 는 A 가 B 를 막는다는
//! 뜻이고, 그것을 `B.blocked_by` 에 A 를 더하는 것으로 적는다. 그래야 A 를
//! 닫을 때 A 줄만 쓰면 된다.

use super::{Ctx, Fail, R};
use crate::cli::LinkArgs;
use crate::model::{self, Issue};
use crate::report::creates_cycle;
use crate::style::{self, paint};

pub fn run(ctx: &Ctx, args: LinkArgs) -> R<Vec<String>> {
    if args.blocks.is_empty() && args.unblocks.is_empty() {
        return Err(Fail::new(crate::i18n::say(ctx.lang(), "refuse.link_needs_side")));
    }
    // 같은 id 가 둘 다에 있으면 어느 쪽이 이기는지 조용히 정하지 않는다.
    if let Some(dup) = args.blocks.iter().find(|b| args.unblocks.contains(b)) {
        return Err(Fail::new(crate::i18n::fill(
            crate::i18n::say(ctx.lang(), "refuse.link_both_ways"),
            &[("id", dup)],
        )));
    }
    let repo = super::open_repo(ctx.lang())?;
    let at = model::now();

    // `(대상, 막을지 없앨지)`. 어느 쪽에서 왔는지를 갖고 있어야, 다시
    // `args.blocks.contains` 로 되짚어 묻다가 판단을 두 번 다르게 하는
    // 일이 없다.
    let edits: Vec<(String, bool)> =
        args.blocks.iter().map(|t| (t.clone(), true)).chain(args.unblocks.iter().map(|t| (t.clone(), false))).collect();

    let (touched, read): (Vec<(Issue, bool)>, super::Read) = repo.with_write(|issues, cfg, _| {
        // 막는 쪽의 존재는 **더할 때만** 따진다. `--unblocks` 만이면 그것이
        // 이미 지워졌을 수 있고, 그때도 남은 참조는 풀려야 한다 — 아니면
        // `status` 가 드러낸 끊긴 참조를 손으로 파일을 고쳐야만 없앨 수 있다.
        if !args.blocks.is_empty() {
            let Some(blocker) = issues.iter().find(|i| i.id == args.id) else {
                return Err(Fail::not_found(&args.id, ctx.lang()));
            };
            // **담아 둔 생각은 막지 않는다.** idea 는 보통 `done` 에 닿지
            // 않으므로, 막게 두면 막힌 이슈가 영영 안 풀리면서 `status` 는
            // 그것을 "계획이 멈춘 자리" 로 센다 — 도구가 스스로 만든 막다른
            // 길이고, 막는 쪽이 목록에 안 나오니 풀 방법도 안 보인다.
            if crate::report::is_idea(blocker) {
                return Err(Fail::coded(
                    format!(
                        "{} 는 담아 둔 생각이라 막을 수 없다 — 생각은 닫히지 않으니 영영 안 풀린다.\n      \
                         먼저 `moai idea promote {} --from -` 으로 펼친다",
                        args.id, args.id
                    ),
                    super::code::BAD_TARGET,
                ));
            }
        }
        for (target, wants_block) in &edits {
            let Some(t) = issues.iter().find(|i| &i.id == target) else {
                return Err(Fail::not_found(target, ctx.lang()));
            };
            // 막히는 쪽도 마찬가지다. 생각은 집는 것이 아니라서 막힐 것도 없다.
            if *wants_block && crate::report::is_idea(t) {
                return Err(Fail::coded(
                    format!("{target} 는 담아 둔 생각이라 막힐 것이 없다 — 생각은 집는 것이 아니다"),
                    super::code::BAD_TARGET,
                ));
            }
            if *wants_block && creates_cycle(issues, &args.id, target) {
                return Err(Fail::coded(
                    format!(
                        "{} 가 {target} 를 막으면 고리가 된다 — {target} 는 이미 (곧바로든 건너서든, 묶음의 멤버로든) {} 를 막고 있다",
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
            let kept = t.status.clone();
            t.updated_at = at.clone();
            t.normalize();
            // **안 바꾼 칸은 다시 안 묻는다 — `store::with_write`·`edit` 과 한 자다**
            // (moai-hym7). 막음은 `blocked_by` 에 쓰지 칸에 쓰지 않으므로, 여기서 칸
            // 이름을 다시 물으면 `config` 에서 칸 이름을 고친 뒤 옛 이름에 선 줄은
            // 막지도 풀지도 못한다 — 끊긴 참조를 도구 안에서 못 없애는 자리다.
            t.validate_keeping(cfg, t.status == kept)?;
            out.push((t.clone(), wants_block));
        }
        // 막히는 쪽이 묶음일 수 있다 — 적힌 칸을 그대로 내면 받는 쪽이 안 읽히는
        // 칸을 읽는다(`cmd::Row`).
        let ids: Vec<&str> = out.iter().map(|(i, _)| i.id.as_str()).collect();
        let read = super::read_of(issues, cfg, &ids);
        Ok((vec![], (out, read)))
    })?;

    if ctx.json {
        let rows: Vec<super::Row> = touched.iter().map(|(i, _)| super::Row::from(i, &read)).collect();
        return super::json_line(&rows);
    }
    if touched.is_empty() {
        // `edit` 과 **한 키다**(`edit.nothing_changed`, moai-95g1) — 같은 문장을 두 벌로 두면
        // 한 도구가 같은 처지를 두 말로 말한다(리뷰). 이 줄과 아래 화살표 줄은 서로 배타라
        // 한 판에 같이 서지 않는다 — 나머지 `link` 의 글은 그 표면의 차례에 옮긴다.
        let said = crate::i18n::say(ctx.lang(), "edit.nothing_changed");
        return Ok(vec![format!("{}  {}", paint(style::ID, &args.id), paint(style::DIM, said))]);
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
