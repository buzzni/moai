//! Shared orchestration for the CLI, terminal view and web view.
use crate::{diff, git, model, report, store::Repo, vcs, worktree};
use report::changes::{Checkout, Offer, Offered, Scope};
use std::path::{Path, PathBuf};
use std::time::Duration;

const PATCH_LIMIT: usize = 2 * 1024 * 1024;
#[derive(Debug)]
pub enum RequestError {
    Scope(Vec<Scope>),
    Path,
}
#[derive(serde::Serialize)]
pub struct File {
    #[serde(flatten)]
    pub change: vcs::Change,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hunks: Option<Vec<diff::Hunk>>,
}
#[derive(serde::Serialize)]
pub struct Answer {
    pub id: String,
    pub scope: Option<Scope>,
    pub offers: Vec<Offer>,
    pub files: Vec<File>,
    pub total: usize,
    pub truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vcs_error: Option<git::Told>,
    #[serde(skip)]
    pub patch: String,
}
pub struct Prepared {
    id: String,
    root: PathBuf,
    checkouts: Vec<Checkout>,
    offered: Offered,
    commits: Vec<git::Commit>,
    error: Option<git::Told>,
    lang: crate::i18n::Lang,
}
fn told(root: &Path, lang: crate::i18n::Lang, e: vcs::Error) -> git::Told {
    match e {
        vcs::Error::Git(e) => e.told(root, lang),
        vcs::Error::Io(e) => git::Error::Stream(e).told(root, lang),
        vcs::Error::Timeout => git::Told { kind: "timeout", said: crate::i18n::say(lang, "diff.timeout").to_owned() },
        vcs::Error::Limit => git::Told { kind: "failed", said: crate::i18n::say(lang, "diff.limit").to_owned() },
        vcs::Error::InvalidRevision => {
            git::Told { kind: "failed", said: crate::i18n::say(lang, "diff.invalid_revision").to_owned() }
        }
        vcs::Error::InvalidPath => {
            git::Told { kind: "failed", said: crate::i18n::say(lang, "diff.invalid_path").to_owned() }
        }
    }
}
/// Read each checkout and commit table once. The placement decision itself is pure.
pub fn offers(
    repo: &Repo,
    issues: &[model::Issue],
    id: &str,
    overlay: bool,
    budget: Option<Duration>,
    lang: crate::i18n::Lang,
) -> Prepared {
    let root = repo.here().to_path_buf();
    let workplaces = worktree::workplaces(repo.here(), overlay);
    let checkouts = worktree::checkouts(repo.here());
    let now = model::now();
    let tentative = report::changes::scopes(issues, &repo.config, id, &workplaces, &checkouts, &[], &now);
    let source = tentative
        .order
        .iter()
        .find_map(|o| o.checkout.as_ref())
        .and_then(|key| checkouts.iter().find(|c| &c.key == key))
        .map_or(root.as_path(), |c| c.path.as_path());
    let (mut commits, error) = match git::commits_within(source, id, budget) {
        Some(Ok(commits)) => (commits, None),
        Some(Err(e)) => (Vec::new(), Some(e.told(source, lang))),
        None => {
            (Vec::new(), Some(git::Told { kind: "timeout", said: crate::i18n::say(lang, "diff.timeout").to_owned() }))
        }
    };
    commits.retain(|c| !c.tracker);
    commits.reverse();
    let hashes: Vec<_> = commits.iter().map(|c| c.hash.clone()).collect();
    let offered = report::changes::scopes(issues, &repo.config, id, &workplaces, &checkouts, &hashes, &now);
    Prepared { id: id.to_owned(), root, checkouts, offered, commits, error, lang }
}
impl Prepared {
    fn blank(&self) -> Answer {
        Answer {
            id: self.id.clone(),
            scope: None,
            offers: self.offered.order.clone(),
            files: Vec::new(),
            total: 0,
            truncated: false,
            vcs_error: None,
            patch: String::new(),
        }
    }
    fn checkout(&self, offer: &Offer) -> &Path {
        offer
            .checkout
            .as_ref()
            .and_then(|key| self.checkouts.iter().find(|c| &c.key == key))
            .map_or(self.root.as_path(), |c| c.path.as_path())
    }
    fn vcs_scope(&self, scope: Scope) -> vcs::Scope {
        match scope {
            Scope::Commits => vcs::Scope::Commits(self.commits.iter().map(|c| c.hash.clone()).collect()),
            Scope::Branch => vcs::Scope::Branch,
            Scope::Working => vcs::Scope::Working,
        }
    }
    fn stat(&self, offer: &Offer, budget: Option<Duration>) -> Result<vcs::Changes, vcs::Error> {
        let root = self.checkout(offer);
        vcs::detect(root).ok_or_else(|| vcs::Error::Git(git::Error::Failed("not a git repository".into())))?.changes(
            root,
            &self.vcs_scope(offer.scope),
            budget,
        )
    }
    /// Explicit scope is validated before any path reaches Git. Without one,
    /// the first non-empty offer wins, falling back to the first empty offer.
    pub fn read(
        &self,
        requested: Option<Scope>,
        path: Option<&str>,
        stat_only: bool,
        budget: Option<Duration>,
    ) -> Result<Answer, RequestError> {
        let mut answer = self.blank();
        if let Some(error) = &self.error {
            answer.vcs_error = Some(git::Told { kind: error.kind, said: error.said.clone() });
            return Ok(answer);
        }
        let candidates: Vec<_> = self.offered.order.iter().filter(|o| requested.is_none_or(|s| o.scope == s)).collect();
        if requested.is_some() && candidates.is_empty() {
            return Err(RequestError::Scope(self.offered.order.iter().map(|o| o.scope).collect()));
        }
        let mut selected = None;
        for offer in candidates {
            match self.stat(offer, budget) {
                Ok(changes) => {
                    let empty = changes.total == 0;
                    if selected.is_none() || !empty {
                        selected = Some((offer, changes));
                    }
                    if requested.is_some() || !empty {
                        break;
                    }
                }
                Err(e) => {
                    answer.vcs_error = Some(told(self.checkout(offer), self.lang, e));
                    return Ok(answer);
                }
            }
        }
        let Some((offer, mut changes)) = selected else {
            if path.is_some() {
                return Err(RequestError::Path);
            }
            return Ok(answer);
        };
        answer.scope = Some(offer.scope);
        answer.total = changes.total;
        answer.truncated = changes.truncated;
        if let Some(path) = path {
            if !changes.files.iter().any(|c| c.path == path) {
                return Err(RequestError::Path);
            }
            changes.files.retain(|c| c.path == path);
        }
        if stat_only {
            answer.files = changes.files.into_iter().map(|change| File { change, hunks: None }).collect();
            return Ok(answer);
        }
        let root = self.checkout(offer);
        let Some(backend) = vcs::detect(root) else {
            answer.vcs_error =
                Some(told(root, self.lang, vcs::Error::Git(git::Error::Failed("not a git repository".into()))));
            return Ok(answer);
        };
        let mut raw = String::new();
        let mut patch_cut = false;
        if offer.scope == Scope::Commits {
            for commit in &self.commits {
                let scope = vcs::Scope::Commits(vec![commit.hash.clone()]);
                match backend.patch(root, &scope, path, budget) {
                    Ok(patch) if !patch.text.is_empty() => {
                        append(&mut raw, &format!("commit {} {}\n", commit.hash, commit.subject), &mut patch_cut);
                        append(&mut raw, &patch.text, &mut patch_cut);
                        patch_cut |= patch.truncated;
                        if patch_cut {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        answer.vcs_error = Some(told(root, self.lang, e));
                        return Ok(answer);
                    }
                }
            }
        } else {
            match backend.patch(root, &self.vcs_scope(offer.scope), path, budget) {
                Ok(patch) => {
                    raw = patch.text;
                    patch_cut |= patch.truncated;
                }
                Err(e) => {
                    answer.vcs_error = Some(told(root, self.lang, e));
                    return Ok(answer);
                }
            }
        }
        answer.truncated |= patch_cut;
        if patch_cut && !raw.ends_with('\n') {
            if let Some(i) = raw.rfind('\n') {
                raw.truncate(i + 1);
            } else {
                raw.clear();
            }
        }
        let parsed = diff::parse(&raw);
        answer.files = changes
            .files
            .into_iter()
            .map(|change| {
                let hunks = parsed
                    .iter()
                    .filter(|f| {
                        f.new_path.as_deref() == Some(&change.path)
                            || (f.new_path.is_none() && f.old_path.as_deref() == Some(&change.path))
                    })
                    .flat_map(|f| f.hunks.iter().cloned())
                    .collect();
                File { change, hunks: Some(hunks) }
            })
            .collect();
        answer.patch = raw;
        Ok(answer)
    }
}
fn append(out: &mut String, text: &str, truncated: &mut bool) {
    let available = PATCH_LIMIT.saturating_sub(out.len());
    if text.len() <= available {
        out.push_str(text);
        return;
    }
    let mut end = available;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    out.push_str(&text[..end]);
    *truncated = true;
}
