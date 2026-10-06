//! Explicit archive maintenance.

use super::{Ctx, R};
use crate::archive;
use crate::cli::ArchiveArgs;

#[derive(serde::Serialize)]
struct Report<'a> {
    dry_run: bool,
    rows: usize,
    ids: Vec<&'a str>,
}

pub fn run(ctx: &Ctx, args: ArchiveArgs) -> R<Vec<String>> {
    let repo = super::open_repo(ctx)?;
    let now = crate::model::now();
    let rows = if args.dry_run {
        let load = repo.read()?;
        archive::eligible(&load.issues, &repo.config, &now)
    } else {
        // Hold the repository write lock while selecting and moving rows so two
        // archive commands cannot select the same active snapshot concurrently.
        repo.with_write(
            || ctx.lang(),
            |issues, _, _| {
                let rows = archive::eligible(issues, &repo.config, &now);
                if rows.is_empty() {
                    return Ok((Vec::new(), rows));
                }
                archive::append(&repo.root, &rows, &repo.config)?;
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
