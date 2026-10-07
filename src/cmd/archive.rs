//! Explicit archive maintenance.

use super::{Ctx, R};
use crate::archive;
use crate::cli::ArchiveArgs;
use crate::fail::Fail;
use crate::i18n::{fill, say};
use crate::store::Repo;

#[derive(serde::Serialize)]
struct Report<'a> {
    dry_run: bool,
    rows: usize,
    ids: Vec<&'a str>,
}

/// `--drop <id>` 의 `--json` — 걷은 아카이브 줄의 수. 0 이면 사본이 없었다.
#[derive(serde::Serialize)]
struct Dropped<'a> {
    id: &'a str,
    removed: usize,
}

pub fn run(ctx: &Ctx, args: ArchiveArgs) -> R<Vec<String>> {
    let repo = super::open_repo(ctx)?;
    if let Some(id) = args.drop {
        return drop_copies(ctx, &repo, &id);
    }
    let now = crate::model::now();
    let rows = if args.dry_run {
        // **미리 보기는 옮기기와 같은 자로 고른다**(moai-bth3 리뷰) — 아카이브 사본과 갈린 묶음은 옮기기가 산 채로
        // 두므로, 그것까지 세던 판은 옮기지 않을 줄을 약속했고 보드의 `archive_pending` 은 영영 안 줄었다.
        let load = repo.read()?;
        archive::movable(&load.issues, &archive::read(&repo.root)?, &repo.config, &now)
    } else {
        // Hold the repository write lock while selecting and moving rows so two
        // archive commands cannot select the same active snapshot concurrently.
        repo.with_write(
            || ctx.lang(),
            |issues, _, _| {
                let rows = archive::movable(issues, &archive::read(&repo.root)?, &repo.config, &now);
                if rows.is_empty() {
                    return Ok((Vec::new(), rows));
                }
                let rows = archive::append(&repo.root, &rows, &repo.config)?;
                let ids: std::collections::BTreeSet<String> = rows.iter().map(|i| i.id.clone()).collect();
                let mut moved = Vec::new();
                issues.retain(|i| {
                    if ids.contains(&i.id) {
                        moved.push(i.clone());
                        false
                    } else {
                        true
                    }
                });
                Ok((Vec::new(), moved))
            },
        )?
    };
    let ids: Vec<&str> = rows.iter().map(|i| i.id.as_str()).collect();
    if ctx.json {
        return super::json_line(&Report { dry_run: args.dry_run, rows: ids.len(), ids });
    }
    Ok(vec![if args.dry_run {
        format!("{} rows eligible for archive", rows.len())
    } else {
        format!("archived {} rows", rows.len())
    }])
}

/// 산 줄을 두고 그 줄의 낡은 아카이브 사본을 걷는다(moai-bth3, 사람이 정했다: 산 줄이 없는 id 는 안 걷는다).
///
/// **걷는 것은 산 스냅샷이 검사를 지난 뒤다**(`with_write_after`, 리뷰) — 닫힘 안에서 걷던 판은 산 파일의 다른 중복에
/// 걸려 실패로 끝나면서도 사본은 이미 지웠다. **같은 이슈만 사본이다**([`archive::drop_copy`]) — id 만 같은 다른 이슈는
/// 걷지 않고 댄다. 걷으면 그 이슈가 아무 데도 안 남는다.
fn drop_copies(ctx: &Ctx, repo: &Repo, id: &str) -> R<Vec<String>> {
    let lang = ctx.lang();
    let dropped = std::cell::RefCell::new(archive::Dropped::default());
    repo.with_write_after(
        || lang,
        |issues, _, _, reserved| match issues.iter().find(|i| i.id == id) {
            Some(live) => Ok((Vec::new(), live.clone())),
            // 산 줄은 없는데 그 id 가 아카이브(나 못 읽는 산 줄)에 서 있다 — 걷으면 그 줄이 아무 데도 안 남는다.
            None if reserved.contains(id) => {
                Err(Fail::coded(fill(say(lang, "refuse.drop_no_live"), &[("id", id)]), super::code::BAD_TARGET))
            }
            None => Err(Fail::not_found(id, lang)),
        },
        |live| {
            *dropped.borrow_mut() = archive::drop_copy(&repo.root, live)?;
            Ok(())
        },
    )?;
    let dropped = dropped.into_inner();
    if !dropped.others.is_empty() {
        return Err(Fail::coded(
            fill(say(lang, "refuse.drop_other_issue"), &[("id", id), ("created", &dropped.others.join(", "))]),
            super::code::BAD_TARGET,
        ));
    }
    for said in &dropped.unread {
        super::tell_after(fill(say(lang, "archive.drop_unread"), &[("said", &crate::text::one_line(said))]));
    }
    if ctx.json {
        return super::json_line(&Dropped { id, removed: dropped.removed });
    }
    Ok(vec![match dropped.removed {
        0 => fill(say(lang, "archive.drop_none"), &[("id", id)]),
        n => fill(say(lang, "archive.dropped"), &[("id", id), ("n", &n.to_string())]),
    }])
}
