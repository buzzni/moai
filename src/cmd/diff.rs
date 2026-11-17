//! CLI wiring for the shared change reader.
use super::{Ctx, Fail, R};
use crate::cli::{DiffArgs, DiffScope};
use crate::report::changes::Scope;

pub fn run(ctx: &Ctx, args: DiffArgs) -> R<Vec<String>> {
    let repo = super::open_repo(ctx)?;
    let gathered = super::gather(ctx, &repo, args.worktree.worktree)?;
    let load = crate::archive::read_all(&repo.root, gathered.load)?;
    super::report_load_errors(ctx.lang(), &repo.issues_path(), &load.errors);
    if load.get(&args.id).is_none() {
        return Err(Fail::not_found(&args.id, ctx.lang()));
    }
    let prepared = crate::changes::offers(&repo, &load.issues, &args.id, None, ctx.lang());
    let requested = args.scope.map(|s| match s {
        DiffScope::Commits => Scope::Commits,
        DiffScope::Branch => Scope::Branch,
        DiffScope::Working => Scope::Working,
    });
    let answer = prepared.read(requested, args.path.as_deref(), args.stat, None).map_err(|e| {
        let message = match e {
            crate::changes::RequestError::Scope(available) => {
                let names = available.iter().map(crate::view::scope_name).collect::<Vec<_>>().join(", ");
                crate::i18n::fill(crate::i18n::say(ctx.lang(), "diff.bad_scope"), &[("scopes", &names)])
            }
            crate::changes::RequestError::Path => crate::i18n::say(ctx.lang(), "diff.bad_path").to_owned(),
        };
        Fail::coded(message, super::code::BAD_INPUT)
    })?;
    if ctx.json {
        return super::json_line(&answer);
    }
    super::tell(&crate::view::diff_notice(ctx.lang(), &answer));
    Ok(crate::view::diff_output(&answer, args.stat))
}
