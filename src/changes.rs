//! Shared orchestration for the CLI, terminal view and web view.
use crate::{diff, git, model, report, store::Repo, vcs, worktree};
use report::changes::{Checkout, Offer, Offered, Scope};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use vcs::PATCH_LIMIT;
#[derive(Debug)]
pub enum RequestError {
    Scope(Vec<Scope>),
    Path,
    /// `--vcs` asked for a backend this root does not stand under (moai-0d0s.qg0) — what was asked and what is there.
    Vcs {
        asked: vcs::Backend,
        found: vcs::Detected,
    },
    /// `--scope branch` where the svn working copy has no fork point: the scopes there are, and the one line that says
    /// why `branch` is not among them.
    NoBranch {
        scopes: Vec<Scope>,
        notice: String,
    },
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
    /// The one line that `branch` is not served for this svn working copy — no fork point known (moai-0d0s.qg0). Said
    /// by the surfaces' notices, never in `--json`: the schema keeps git's keys.
    #[serde(skip)]
    pub notice: Option<String>,
    /// `[vcs] backend` holds a value nobody knows — the default answered. `moai diff` says it on stderr.
    #[serde(skip)]
    pub warning: Option<String>,
}
pub struct Prepared {
    id: String,
    root: PathBuf,
    checkouts: Vec<Checkout>,
    offered: Offered,
    commits: Vec<git::Commit>,
    error: Option<git::Told>,
    /// svn only: the id → revision table could not be read (the server out of reach, the table still filling). Unlike
    /// [`Prepared::error`] it does not stop the reads that need no server — `working`, and the scopes of a branch whose
    /// fork point is kept — so the answer is this only when nothing else can be read.
    unread: Option<git::Told>,
    lang: crate::i18n::Lang,
    /// The backend chosen for this read ([`vcs::choose`]); `None` when neither is found.
    backend: Option<vcs::Backend>,
    /// `--vcs` named a backend that is not found here: every read is refused.
    refused: Option<(vcs::Backend, vcs::Detected)>,
    /// `branch` is not served, known before any read (no trunk·branches convention).
    notice: Option<String>,
    warning: Option<String>,
    /// svn under `moai diff` (no budget): the whole command ends here — 30 seconds from the start, the person's
    /// "whole `moai diff` in 30 seconds" (moai-0d0s.4o8's table read keeps its own share of it).
    cap: Option<Instant>,
}
/// The read layer's error as one line for people, with its machine word — **the one place the surfaces' failure
/// lines are made**. `backend` picks the words: an svn read never says "git" (moai-0d0s.qg0).
pub(crate) fn told(root: &Path, lang: crate::i18n::Lang, backend: Option<vcs::Backend>, e: vcs::Error) -> git::Told {
    if backend == Some(vcs::Backend::Svn) {
        return svn_told(lang, e);
    }
    match e {
        vcs::Error::Git(e) => e.told(root, lang),
        vcs::Error::Svn(e) => svn_told(lang, vcs::Error::Svn(e)),
        vcs::Error::NoBase(why) => git::Told { kind: "no_base", said: no_base(lang, why) },
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
/// An svn read's failure in svn's words. What svn itself said is folded to one line with its absolute paths cut, as
/// git's is ([`git::cut_paths`]) — `E000002`'s message carries the working copy's path.
fn svn_told(lang: crate::i18n::Lang, e: vcs::Error) -> git::Told {
    use crate::i18n::{fill, say};
    use crate::svn::Error as S;
    // A `file://` URL is a local path with a scheme on it: without the scheme, the path is cut as git's are.
    let line = |s: &str| git::cut_paths(&crate::text::one_line(s).replace("file://", ""));
    let (kind, said) = match e {
        vcs::Error::Svn(e) => {
            let said = match &e {
                S::Spawn(io) if io.kind() == std::io::ErrorKind::NotFound => say(lang, "diff.svn.missing").to_owned(),
                S::Spawn(_) => fill(say(lang, "diff.svn.spawn"), &[("said", &line(&e.said()))]),
                S::Timeout => say(lang, "diff.svn.timeout").to_owned(),
                S::Offline(Some(_)) => fill(say(lang, "diff.svn.offline"), &[("said", &line(&e.said()))]),
                S::Offline(None) => say(lang, "diff.svn.offline_wait").to_owned(),
                S::Failed(_) => fill(say(lang, "diff.svn.failed"), &[("said", &line(&e.said()))]),
                S::Filling => say(lang, "diff.svn.filling").to_owned(),
            };
            (e.kind(), said)
        }
        vcs::Error::NoBase(why) => ("no_base", no_base(lang, why)),
        vcs::Error::Timeout => ("timeout", say(lang, "diff.svn.timeout").to_owned()),
        vcs::Error::Limit => ("failed", say(lang, "diff.svn.limit").to_owned()),
        vcs::Error::InvalidRevision => ("failed", say(lang, "diff.svn.invalid_revision").to_owned()),
        vcs::Error::InvalidPath => ("failed", say(lang, "diff.invalid_path").to_owned()),
        vcs::Error::Io(e) => ("stream", fill(say(lang, "diff.svn.stream"), &[("said", &line(&e.to_string()))])),
        vcs::Error::Git(e) => ("failed", fill(say(lang, "diff.svn.failed"), &[("said", &line(&e.said()))])),
    };
    git::Told { kind, said }
}
/// The person's one line that `branch` is not served — said in three places only: `moai diff`'s stderr, the
/// explorer's diff pane title, and the `--scope branch` refusal (the person's third-round answer).
fn no_base(lang: crate::i18n::Lang, why: vcs::NoBase) -> String {
    match why {
        vcs::NoBase::Convention => crate::i18n::say(lang, "diff.svn.no_base_convention").to_owned(),
        vcs::NoBase::NotCopied => crate::i18n::say(lang, "diff.svn.no_base_not_copied").to_owned(),
    }
}
/// Which backend a root reads with, and the one line about a `[vcs] backend` nobody knows. **The choice itself is
/// [`vcs::choose`]** — this only gathers what it weighs: what is on the disk, the window's or tab's pick, the config.
pub(crate) fn chosen(
    root: &Path,
    config: &crate::config::Config,
    flag: Option<vcs::Backend>,
    screen: Option<vcs::Backend>,
) -> (vcs::Detected, Option<vcs::Backend>, Option<String>) {
    let found = vcs::detected(root);
    let configured = vcs::configured(&config.vcs);
    let backend = vcs::choose(found, vcs::Asked { flag, screen, config: configured.backend });
    (found, backend, configured.unknown)
}
/// Read each checkout and commit table once. The placement decision itself is pure.
///
/// Workplaces are always measured: `workplaces` skips them inside a linked worktree unless
/// overlaying, which made `moai diff` in the epic's own worktree answer "no changes".
///
/// `vcs` is `moai diff --vcs`; the screens give `None` and read as `[vcs] backend` says. A Git read is the one it was
/// before svn; an svn working copy is read by [`svn_offers`].
pub fn offers(
    repo: &Repo,
    issues: &[model::Issue],
    id: &str,
    budget: Option<Duration>,
    lang: crate::i18n::Lang,
    vcs: Option<vcs::Backend>,
) -> Prepared {
    let started = Instant::now();
    let root = repo.here().to_path_buf();
    let (found, backend, unknown) = chosen(&root, &repo.config, vcs, None);
    let warning = unknown.map(|raw| crate::i18n::fill(crate::i18n::say(lang, "diff.vcs_unknown"), &[("raw", &raw)]));
    let mut prepared = Prepared {
        id: id.to_owned(),
        root,
        checkouts: Vec::new(),
        offered: Offered::default(),
        commits: Vec::new(),
        error: None,
        unread: None,
        lang,
        backend,
        refused: None,
        notice: None,
        warning,
        cap: None,
    };
    if let Some(asked) = vcs.filter(|b| !found.has(*b)) {
        prepared.refused = Some((asked, found));
        return prepared;
    }
    if backend == Some(vcs::Backend::Svn) {
        svn_offers(&mut prepared, repo, issues, budget, started);
        return prepared;
    }
    let root = prepared.root.clone();
    let workplaces = worktree::workplaces(repo.here(), true);
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
    prepared.checkouts = checkouts;
    prepared.offered = offered;
    prepared.commits = commits;
    prepared.error = error;
    prepared
}
/// An svn working copy as the scope rules see it (moai-0d0s.qg0): **one checkout**, key `.`, at the project root. Where
/// it stands is asked of [`crate::svn::layout_at`] alone — the trunk is a main checkout named `trunk`; `branches/<x>` is a
/// linked checkout named `<x>` with one workplace whose names are Git's ([`worktree::names`]), so the issue `<x>` (or
/// `worktree-<x>`) and its picked members get `working`·`branch` exactly as in a Git worktree; a working copy of no
/// convention is a main checkout with no name, and says once that `branch` is not served. The revisions come from
/// [`crate::svn::commits_within`]. Then [`report::changes::scopes`], untouched, decides.
fn svn_offers(p: &mut Prepared, repo: &Repo, issues: &[model::Issue], budget: Option<Duration>, started: Instant) {
    use crate::svn::Layout;
    let lang = p.lang;
    if budget.is_none() {
        p.cap = Some(started + crate::svn::SERVER_TIMEOUT);
    }
    let fail = |e: crate::svn::Error| Some(svn_told(lang, e.into()));
    let layout = match crate::svn::layout_at(&p.root, budget) {
        Ok(l) => l,
        Err(e) => {
            p.error = fail(e);
            return;
        }
    };
    let name = layout.branch_name().map(str::to_owned);
    let (branch, main) = match (&layout, &name) {
        (_, Some(x)) => (x.clone(), false),
        (Layout::Trunk, None) => ("trunk".to_owned(), true),
        _ => (String::new(), true),
    };
    if layout == Layout::Unknown {
        p.notice = Some(no_base(lang, vcs::NoBase::Convention));
    }
    let path = crate::path::canonical(&p.root).unwrap_or_else(|_| p.root.clone());
    let checkouts = vec![Checkout { key: ".".into(), path, branch, main, milestone: None }];
    let workplaces: Vec<report::Workplace> = name
        .iter()
        .map(|x| {
            let tree = worktree::Tree { path: PathBuf::new(), label: x.clone(), head: String::new() };
            report::Workplace {
                path: ".".into(),
                branch: x.clone(),
                names: worktree::names([&tree]),
                marked: Default::default(),
                broken: false,
            }
        })
        .collect();
    // A table that cannot be read leaves `commits` unknown, not every scope: the rest are offered as with no revision.
    let mut commits = match crate::svn::commits_within(&p.root, &p.id, budget) {
        Ok(c) => c,
        Err(e) => {
            p.unread = fail(e);
            Vec::new()
        }
    };
    commits.retain(|c| !c.tracker);
    commits.reverse();
    let hashes: Vec<_> = commits.iter().map(|c| c.hash.clone()).collect();
    let now = model::now();
    p.offered = report::changes::scopes(issues, &repo.config, &p.id, &workplaces, &checkouts, &hashes, &now);
    p.checkouts = checkouts;
    p.commits = commits;
}
impl Prepared {
    /// The backend this read goes with — `None` when neither git nor svn is found.
    pub fn backend(&self) -> Option<vcs::Backend> {
        self.backend
    }
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
            notice: self.notice.clone(),
            warning: self.warning.clone(),
        }
    }
    /// The backend that reads `root`: the chosen one when it is svn, else what the disk says as before svn — a Git
    /// checkout and its linked worktrees read as Git, a folder under neither reads as nothing.
    fn backend_at(&self, root: &Path) -> Option<vcs::Backend> {
        match self.backend {
            Some(vcs::Backend::Svn) => Some(vcs::Backend::Svn),
            _ => vcs::detect(root),
        }
    }
    /// A failed read as the surfaces say it. Under `moai diff`'s 30-second cap for svn, a server scope's read cut by the
    /// cap is the server's silence, so it is `offline`. `working` never asks the server: its read cut by the cap is a
    /// `timeout` — saying the server did not answer would name something never asked. A read that ran out of time while
    /// the revision table was not read ([`Prepared::unread`]) lost its time to the table, and says that.
    fn fail(&self, root: &Path, e: vcs::Error, budget: Option<Duration>, scope: Scope) -> git::Told {
        if let (Some(unread), vcs::Error::Timeout) = (&self.unread, &e) {
            return git::Told { kind: unread.kind, said: unread.said.clone() };
        }
        let e = match e {
            vcs::Error::Timeout if budget.is_none() && self.cap.is_some() && scope != Scope::Working => {
                vcs::Error::Svn(crate::svn::Error::Offline(None))
            }
            e => e,
        };
        told(root, self.lang, self.backend_at(root), e)
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
        self.backend_at(root)
            .ok_or_else(|| vcs::Error::Git(git::Error::Failed("not a git repository".into())))?
            .changes(root, &self.vcs_scope(offer.scope), budget)
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
        let started = std::time::Instant::now();
        let remaining = || match (budget, self.cap) {
            (Some(d), _) => Some(d.saturating_sub(started.elapsed())),
            (None, Some(cap)) => Some(cap.saturating_duration_since(Instant::now())),
            (None, None) => None,
        };
        if let Some((asked, found)) = self.refused {
            return Err(RequestError::Vcs { asked, found });
        }
        let mut answer = self.blank();
        if let Some(error) = &self.error {
            answer.vcs_error = Some(git::Told { kind: error.kind, said: error.said.clone() });
            return Ok(answer);
        }
        // svn's table not read: `commits` asked for by name, or nothing else offered, is that failure.
        let unread = || self.unread.as_ref().map(|t| git::Told { kind: t.kind, said: t.said.clone() });
        if self.unread.is_some() && (self.offered.order.is_empty() || requested == Some(Scope::Commits)) {
            answer.vcs_error = unread();
            return Ok(answer);
        }
        let candidates: Vec<_> = self.offered.order.iter().filter(|o| requested.is_none_or(|s| o.scope == s)).collect();
        if requested.is_some() && candidates.is_empty() {
            let scopes = self.offered.order.iter().map(|o| o.scope).collect();
            return Err(match (&self.notice, requested) {
                (Some(notice), Some(Scope::Branch)) => RequestError::NoBranch { scopes, notice: notice.clone() },
                _ => RequestError::Scope(scopes),
            });
        }
        let mut selected = None;
        for offer in candidates {
            match self.stat(offer, remaining()) {
                Ok(changes) => {
                    let empty = changes.total == 0;
                    if selected.is_none() || !empty {
                        selected = Some((offer, changes));
                    }
                    if requested.is_some() || !empty {
                        break;
                    }
                }
                // svn's `branch` with no fork point (a branch made without a copy): not served — said once, and the
                // next scope answers. Asked for by name, it is refused as an unoffered scope is.
                Err(vcs::Error::NoBase(why)) => {
                    let notice = no_base(self.lang, why);
                    answer.notice = Some(notice.clone());
                    if requested.is_some() {
                        let scopes = self.offered.order.iter().map(|o| o.scope).filter(|s| *s != offer.scope).collect();
                        return Err(RequestError::NoBranch { scopes, notice });
                    }
                }
                Err(e) => {
                    answer.vcs_error = Some(self.fail(self.checkout(offer), e, budget, offer.scope));
                    return Ok(answer);
                }
            }
        }
        // With the table not read, an empty answer is no "nothing changed" — the revisions may hold it.
        let Some((offer, mut changes)) =
            selected.filter(|(_, c)| requested.is_some() || self.unread.is_none() || c.total > 0)
        else {
            if self.unread.is_some() {
                answer.vcs_error = unread();
                return Ok(answer);
            }
            if path.is_some() {
                return Err(RequestError::Path);
            }
            return Ok(answer);
        };
        // A scope that needed no table answered: say once that the revisions were not looked up.
        if let Some(t) = &self.unread {
            answer.notice = Some(match answer.notice.take() {
                Some(n) => format!("{n} · {}", t.said),
                None => t.said.clone(),
            });
        }
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
        let Some(backend) = self.backend_at(root) else {
            let e = vcs::Error::Git(git::Error::Failed("not a git repository".into()));
            answer.vcs_error = Some(self.fail(root, e, budget, offer.scope));
            return Ok(answer);
        };
        // A renamed file is read by both names: Git narrows to the pathspec before it looks for renames, so the new name
        // alone would show the whole file as added. The old name comes from the list, never from the caller.
        let names: Option<Vec<String>> = path.map(|p| {
            std::iter::once(p.to_owned()).chain(changes.files.iter().filter_map(|c| c.old_path.clone())).collect()
        });
        let mut raw = String::new();
        let mut patch_cut = false;
        if offer.scope == Scope::Commits {
            // The old name rides along only to pair the rename. A commit that touched the old name alone — an edit
            // before the rename, or the step before it in a chain of renames, which reads as a whole new file — is not
            // this file's, and its patch would print under `--path` as if it were.
            let foreign = |text: &str| {
                path.is_some_and(|p| {
                    names.as_ref().is_some_and(|n| n.len() > 1) && !diff::parse(text).iter().any(|f| owns(f, p))
                })
            };
            for commit in &self.commits {
                let scope = vcs::Scope::Commits(vec![commit.hash.clone()]);
                let read = match &names {
                    Some(names) => backend.patch_of(root, &scope, names, remaining(), None),
                    None => backend.patch(root, &scope, None, remaining()),
                };
                match read {
                    Ok(patch) if !patch.text.is_empty() && !foreign(&patch.text) => {
                        // A subject is anyone's text; `moai show` sanitizes it before a terminal sees it too.
                        let subject = crate::text::sanitize(&commit.subject);
                        append(&mut raw, &format!("commit {} {subject}\n", commit.hash), &mut patch_cut);
                        append(&mut raw, &patch.text, &mut patch_cut);
                        patch_cut |= patch.truncated;
                        if patch_cut {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        answer.vcs_error = Some(self.fail(root, e, budget, Scope::Commits));
                        return Ok(answer);
                    }
                }
            }
        } else {
            let scope = self.vcs_scope(offer.scope);
            let read = match &names {
                Some(names) => backend.patch_of(root, &scope, names, remaining(), Some(&changes)),
                None => backend.patch_with_changes(root, &scope, None, remaining(), &changes),
            };
            match read {
                Ok(patch) => {
                    raw = patch.text;
                    patch_cut |= patch.truncated;
                }
                Err(e) => {
                    answer.vcs_error = Some(self.fail(root, e, budget, offer.scope));
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
                let hunks =
                    parsed.iter().filter(|f| owns(f, &change.path)).flat_map(|f| f.hunks.iter().cloned()).collect();
                File { change, hunks: Some(hunks) }
            })
            .collect();
        answer.patch = raw;
        Ok(answer)
    }
}
/// Whether a patch's file section is the list row `path`'s: by its new name, or by its old one when it was deleted.
fn owns(f: &diff::FileDiff, path: &str) -> bool {
    f.new_path.as_deref() == Some(path) || (f.new_path.is_none() && f.old_path.as_deref() == Some(path))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Lang;
    use crate::model::{Issue, Kind, Status};
    use crate::svn::tests::{Repo as SvnRepo, commit_files, svn_in, svn_ready};

    /// **An svn failure never says "git", and svn's own words lose their paths as git's do** (moai-0d0s.qg0) — every
    /// kind the surfaces can meet, in both bundles. `E000002`'s message carries the working copy's absolute path, and a
    /// `file://` repository's URL is a local path too.
    #[test]
    fn svn_failures_are_said_in_svn_words_with_their_paths_cut() {
        use crate::svn::Error as S;
        let io = |k| std::io::Error::new(k, "x");
        let cases = || {
            vec![
                (vcs::Error::Svn(S::Spawn(io(std::io::ErrorKind::NotFound))), "no_svn"),
                (vcs::Error::Svn(S::Spawn(io(std::io::ErrorKind::PermissionDenied))), "failed"),
                (vcs::Error::Timeout, "timeout"),
                (vcs::Error::Svn(S::Filling), "timeout"),
                (
                    vcs::Error::Svn(S::Offline(Some(
                        "svn: E170013: Unable to connect to a repository at URL 'file:///home/me/git/repo/trunk'\n\
                         svn: E180001: Unable to open repository 'file:///home/me/git/repo'"
                            .into(),
                    ))),
                    "offline",
                ),
                (vcs::Error::Svn(S::Offline(None)), "offline"),
                (
                    vcs::Error::Svn(S::Failed(
                        "svn: E000002: Can't open file '/home/me/git/wc/.svn/pristine/ab/x.svn-base': No such file"
                            .into(),
                    )),
                    "failed",
                ),
                (vcs::Error::NoBase(vcs::NoBase::Convention), "no_base"),
                (vcs::Error::NoBase(vcs::NoBase::NotCopied), "no_base"),
                (vcs::Error::Limit, "failed"),
                (vcs::Error::InvalidRevision, "failed"),
                (vcs::Error::InvalidPath, "failed"),
                (vcs::Error::Io(io(std::io::ErrorKind::BrokenPipe)), "stream"),
            ]
        };
        for lang in [Lang::En, Lang::Ko] {
            for (e, kind) in cases() {
                let shown = format!("{e:?}");
                let t = told(Path::new("/home/me/git/wc"), lang, Some(vcs::Backend::Svn), e);
                assert_eq!(t.kind, kind, "{shown}");
                assert!(!t.said.is_empty(), "{shown}: nothing said");
                assert!(!t.said.to_lowercase().contains("git"), "{shown}: {}", t.said);
                assert!(!t.said.contains("/home") && !t.said.contains('\n'), "{shown}: {}", t.said);
            }
        }
        // The two reasons `branch` is not served are told apart, and the server's silence from its refusal.
        let said = |e| told(Path::new("/"), Lang::En, Some(vcs::Backend::Svn), e).said;
        assert_ne!(said(vcs::Error::NoBase(vcs::NoBase::Convention)), said(vcs::Error::NoBase(vcs::NoBase::NotCopied)));
        assert_ne!(said(vcs::Error::Svn(S::Offline(None))), said(vcs::Error::Svn(S::Offline(Some("x".into())))));
        // A Git read keeps its words.
        let git = told(Path::new("/"), Lang::En, Some(vcs::Backend::Git), vcs::Error::Timeout);
        assert_eq!((git.kind, git.said.as_str()), ("timeout", crate::i18n::say(Lang::En, "diff.timeout")));
    }

    fn prepared(backend: Option<vcs::Backend>, cap: Option<Instant>) -> Prepared {
        Prepared {
            id: "argos-0001".into(),
            root: PathBuf::from("/"),
            checkouts: Vec::new(),
            offered: Offered::default(),
            commits: Vec::new(),
            error: None,
            unread: None,
            lang: Lang::En,
            backend,
            refused: None,
            notice: None,
            warning: None,
            cap,
        }
    }

    /// **Under `moai diff`'s 30-second cap a cut svn read of a server scope is the server's silence** — `offline`, not
    /// the caller's `timeout` (the person's "svn reads only, 30 seconds"). `working` asks no server, so its cut read
    /// stays `timeout`; a caller with its own budget (the screens) keeps `timeout`, and Git never has the cap. A read
    /// that ran out of time behind a table not read says the table's failure.
    #[test]
    fn a_read_cut_by_moai_diffs_cap_is_offline() {
        let svn = prepared(Some(vcs::Backend::Svn), Some(Instant::now()));
        let root = Path::new("/");
        for scope in [Scope::Commits, Scope::Branch] {
            assert_eq!(svn.fail(root, vcs::Error::Timeout, None, scope).kind, "offline", "{scope:?}");
        }
        assert_eq!(svn.fail(root, vcs::Error::Timeout, None, Scope::Working).kind, "timeout");
        assert_eq!(svn.fail(root, vcs::Error::Timeout, Some(Duration::from_secs(10)), Scope::Commits).kind, "timeout");
        let git = prepared(Some(vcs::Backend::Git), None);
        assert_eq!(git.fail(root, vcs::Error::Timeout, None, Scope::Commits).kind, "timeout");
        let mut filling = prepared(Some(vcs::Backend::Svn), None);
        let said = crate::i18n::say(Lang::En, "diff.svn.filling").to_owned();
        filling.unread = Some(git::Told { kind: "timeout", said: said.clone() });
        let t = filling.fail(root, vcs::Error::Timeout, Some(Duration::ZERO), Scope::Working);
        assert_eq!((t.kind, t.said), ("timeout", said));
    }

    fn issue(id: &str, kind: Kind, status: &str, epic: Option<&str>) -> Issue {
        let mut i = Issue::new(id.into(), id.into(), kind, Status::new(status), "2026-11-01T00:00:00Z");
        i.epic = epic.map(str::to_owned);
        i
    }

    fn tracker() -> crate::store::Repo {
        crate::store::Repo::at(PathBuf::new(), crate::config::Config::parse("prefix = \"argos\"\n").unwrap())
    }

    /// **An svn working copy is one checkout, placed by the trunk·branches convention alone** (moai-0d0s.qg0): on
    /// `branches/<epic>` the epic gets `working`·`branch`·`commits` as a Git worktree named after it does, and its
    /// picked member its own commits first; on the trunk every issue gets `commits` only; a working copy of no
    /// convention says once that `branch` is not served, and refuses `--scope branch` with that line.
    #[test]
    fn an_svn_working_copy_is_placed_by_its_convention() {
        if !svn_ready() {
            return;
        }
        let repo = SvnRepo::new("changes-svn-place");
        let (epic, member, other, plain) = ("argos-ep01", "argos-me01", "argos-ot01", "argos-pl01");
        let issues = vec![
            issue(epic, Kind::Epic, "in_progress", None),
            issue(member, Kind::Issue, "in_progress", Some(epic)),
            issue(other, Kind::Issue, "todo", None),
            issue(plain, Kind::Epic, "in_progress", None),
        ];
        let trunk = repo.checkout("trunk", "trunk");
        commit_files(&trunk, &[("a.txt", "a\n")], &format!("feat: {member}"));
        commit_files(&trunk, &[("b.txt", "b\n")], &format!("feat: {other}"));
        repo.branch(epic);
        let branch = repo.checkout(&format!("branches/{epic}"), "branch");
        std::fs::write(branch.join("a.txt"), "a\nlocal\n").unwrap();
        let cache = repo.scratch.join("cache");
        let ask = |root: &Path, id: &str| {
            let mut tracker = tracker();
            tracker.root = root.to_path_buf();
            crate::svn::cache::tests::with_dir(Some(&cache), || offers(&tracker, &issues, id, None, Lang::En, None))
        };
        let scopes = |p: &Prepared| p.offered.order.iter().map(|o| o.scope).collect::<Vec<_>>();

        let on_branch = ask(&branch, epic);
        assert_eq!(on_branch.error.as_ref().map(|e| e.said.clone()), None);
        // `repo.branch` made the branch with the subject `branch <epic>`: that revision names the epic too.
        assert_eq!(scopes(&on_branch), [Scope::Working, Scope::Branch, Scope::Commits]);
        assert_eq!(on_branch.checkouts[0].branch, epic);
        assert!(!on_branch.checkouts[0].main);
        let answer = on_branch.read(None, None, true, None).expect("read");
        assert_eq!((answer.scope, answer.vcs_error.is_none()), (Some(Scope::Working), true));
        assert!(answer.files.iter().any(|f| f.change.path == "a.txt"));
        // The picked member: its own commits first, the epic's checkout after.
        let mine = ask(&branch, member);
        assert_eq!(scopes(&mine), [Scope::Commits, Scope::Working, Scope::Branch]);
        assert_eq!(mine.commits.iter().map(|c| c.hash.as_str()).collect::<Vec<_>>(), ["r2"]);
        // The trunk: a main checkout named `trunk`, commits only — `working` asked by name is refused.
        let on_trunk = ask(&trunk, other);
        assert_eq!(scopes(&on_trunk), [Scope::Commits]);
        assert_eq!((on_trunk.checkouts[0].branch.as_str(), on_trunk.checkouts[0].main), ("trunk", true));
        assert!(matches!(on_trunk.read(Some(Scope::Working), None, true, None), Err(RequestError::Scope(_))));
        assert!(on_trunk.notice.is_none());
        // No convention (a tag): no name, the one line, and `--scope branch` refused with it.
        svn_in(
            &repo.scratch,
            &["copy", "-q", "-m", "tag", &format!("{}/trunk", repo.url), &format!("{}/tags/v1", repo.url)],
        );
        let tag = repo.checkout("tags/v1", "tag");
        let on_tag = ask(&tag, other);
        assert_eq!(on_tag.checkouts[0].branch, "");
        let notice = crate::i18n::say(Lang::En, "diff.svn.no_base_convention");
        assert_eq!(on_tag.read(None, None, true, None).expect("read").notice.as_deref(), Some(notice));
        match on_tag.read(Some(Scope::Branch), None, true, None) {
            Err(RequestError::NoBranch { scopes, notice: said }) => {
                assert_eq!((scopes, said.as_str()), (vec![Scope::Commits], notice));
            }
            other => panic!("not refused with the line: {:?}", other.err()),
        }
        // A branch made without a copy (`svn mkdir`): offered by its name, said not served once read — the next scope
        // answers — and refused by name with its own line.
        svn_in(&repo.scratch, &["mkdir", "-q", "-m", "plain", &format!("{}/branches/{plain}", repo.url)]);
        let wc = repo.checkout(&format!("branches/{plain}"), "plain");
        let on_plain = ask(&wc, plain);
        assert_eq!(scopes(&on_plain), [Scope::Working, Scope::Branch]);
        let not_copied = crate::i18n::say(Lang::En, "diff.svn.no_base_not_copied");
        let answer = on_plain.read(None, None, true, None).expect("read");
        assert_eq!((answer.scope, answer.notice.as_deref()), (Some(Scope::Working), Some(not_copied)));
        match on_plain.read(Some(Scope::Branch), None, true, None) {
            Err(RequestError::NoBranch { scopes, notice: said }) => {
                assert_eq!((scopes, said.as_str()), (vec![Scope::Working], not_copied));
            }
            other => panic!("not refused with the line: {:?}", other.err()),
        }
    }

    /// **A revision table that cannot be read leaves `commits` unknown, not every scope** (the epic-end review of
    /// moai-0d0s): on an epic's branch with the repository gone and no table kept, `working` still answers with the
    /// local edit and says once that the revisions were not looked up; `commits` asked for by name, and an issue offered
    /// nothing else, answer the table's failure.
    #[test]
    fn a_table_out_of_reach_leaves_the_local_scopes_answering() {
        if !svn_ready() {
            return;
        }
        let repo = SvnRepo::new("changes-svn-unread");
        let (epic, other) = ("argos-ep01", "argos-ot01");
        let issues = vec![issue(epic, Kind::Epic, "in_progress", None), issue(other, Kind::Issue, "todo", None)];
        let trunk = repo.checkout("trunk", "trunk");
        commit_files(&trunk, &[("a.txt", "a\n")], "base");
        repo.branch(epic);
        let branch = repo.checkout(&format!("branches/{epic}"), "branch");
        std::fs::write(branch.join("a.txt"), "a\nlocal\n").unwrap();
        std::fs::rename(repo.scratch.join("repo"), repo.scratch.join("gone")).unwrap();
        let cache = repo.scratch.join("cache");
        let ask = |id: &str| {
            let mut tracker = tracker();
            tracker.root = branch.clone();
            crate::svn::cache::tests::with_dir(Some(&cache), || offers(&tracker, &issues, id, None, Lang::En, None))
        };
        let on_branch = ask(epic);
        assert!(on_branch.error.is_none() && on_branch.unread.as_ref().is_some_and(|t| t.kind == "offline"));
        let answer = on_branch.read(None, None, true, None).expect("read");
        assert_eq!((answer.scope, answer.vcs_error.as_ref().map(|e| e.kind)), (Some(Scope::Working), None));
        assert!(answer.files.iter().any(|f| f.change.path == "a.txt"), "{:?}", answer.files.len());
        assert!(answer.notice.as_deref().is_some_and(|n| n.contains("svn")), "{:?}", answer.notice);
        let commits = on_branch.read(Some(Scope::Commits), None, true, None).expect("read");
        assert_eq!(commits.vcs_error.map(|e| e.kind), Some("offline"));
        let nothing_else = ask(other).read(None, None, true, None).expect("read");
        assert_eq!(nothing_else.vcs_error.map(|e| e.kind), Some("offline"));
    }
}
