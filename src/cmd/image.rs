//! Transport for picture storage. Snapshot mutation still uses the single store write path.
use super::{Ctx, Fail, R};
use crate::cli::ImageAddArgs;
use crate::image::{Settings, Shelf};

pub fn add(ctx: &Ctx, args: ImageAddArgs) -> R<Vec<String>> {
    let repo = super::open_repo(ctx)?;
    let mut settings = Settings::from_config(&repo.config).map_err(Fail::new)?;
    if args.original {
        settings.compress = false;
    }
    let lang = ctx.lang();
    // Only --to requires identity. Validate it before storing any files.
    if let Some(id) = &args.to {
        crate::model::actor(ctx.user.as_deref(), &repo.root).map_err(|e| Fail::no_actor(&e, lang))?;
        // A mistyped id is the common failure: refuse it before a picture lands on the shelf. The write below
        // still finds the row under the lock — this read only spares the orphan file.
        if !repo.read()?.issues.iter().any(|i| i.id == *id) {
            return Err(Fail::not_found(id, lang));
        }
    }
    let shelf = match &args.wiki {
        Some(_) => Shelf::wiki(repo.here(), &repo.config.wiki_dir).map_err(Fail::new)?,
        None => Shelf::tracker(&repo),
    };
    let page_prefix =
        args.wiki.as_deref().map(crate::image::wiki_prefix).transpose().map_err(Fail::new)?.unwrap_or_default();
    // Do all input I/O and decoding before the first mutation, including multi-file calls.
    let pictures: Vec<_> = args
        .files
        .iter()
        .map(|file| {
            let bytes = crate::image::read_input(file).map_err(Fail::new)?;
            let img = crate::image::shrink(&bytes, &settings)
                .map_err(|e| Fail::new(format!("{}: {e}", crate::text::one_line(&file.to_string_lossy()))))?;
            let alt = crate::image::alt(
                args.alt.as_deref().unwrap_or_else(|| file.file_stem().and_then(|s| s.to_str()).unwrap_or("image")),
            );
            Ok((img, alt))
        })
        .collect::<R<_>>()?;
    let mut output = Vec::new();
    let mut markdown = Vec::new();
    for (img, alt) in pictures {
        let put = shelf.put(&img).map_err(Fail::new)?;
        let line = format!("![{alt}]({page_prefix}{})", put.rel);
        let warnings = if img.bytes.len() as u64 > settings.warn_bytes {
            vec![crate::i18n::fill(
                crate::i18n::say(lang, "image.large"),
                &[
                    ("file", &put.name),
                    ("bytes", &img.bytes.len().to_string()),
                    ("limit", &settings.warn_bytes.to_string()),
                ],
            )]
        } else {
            Vec::new()
        };
        for warning in &warnings {
            super::tell(&format!("moai: {warning}"));
        }
        output.push(serde_json::json!({"name":put.name,"path":put.path,"markdown":line,"bytes":img.bytes.len(),"original_bytes":img.original_len,"width":img.width,"height":img.height,"from":img.from,"compressed":img.changed,"reused":!put.fresh,"warnings":warnings}));
        markdown.push(line);
    }
    if let Some(id) = &args.to {
        let text = markdown.join("\n");
        // **The writer's own append is not news to them** — the same as `moai edit` (moai-uwi9.0xu): read the marks
        // outside the lock, judge them against the row read under it, and move the mark after the lock is released.
        let read_config = crate::user_config::path();
        let seen = read_config.as_ref().and_then(|config| {
            let marks = crate::read_marks::read(config, &repo.root, &ctx.registry().read);
            marks.trouble.is_none().then_some(marks.seen)
        });
        let (after, was_read) = repo.with_write(
            || lang,
            |issues, cfg, _| {
                let (before, after) = crate::image::append(issues, cfg, id, &text, lang)?;
                let was_read = seen.as_ref().is_some_and(|m| !crate::query::changed_since_seen(&before, m));
                Ok((vec![], (after, was_read)))
            },
        )?;
        if was_read && let Some(config) = read_config {
            let said: Vec<String> = match crate::read_marks::mark_written(&config, &repo.root, lang, &after) {
                Ok(wrote) => wrote.problems.iter().map(|p| crate::view::sheet_trouble(lang, p)).collect(),
                Err(e) => vec![crate::i18n::fill(
                    crate::i18n::say(lang, "edit.read_unmarked"),
                    &[("id", &after.id), ("why", &crate::text::one_line(&e.message))],
                )],
            };
            for why in said {
                super::tell(&format!("moai: {}", crate::text::one_line(&why)));
            }
        }
        for o in &mut output {
            o["to"] = serde_json::json!(id);
        }
    }
    if ctx.json { super::json_line(&output) } else { Ok(markdown) }
}
