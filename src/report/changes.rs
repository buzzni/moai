//! Pure selection of change scopes and the checkout that owns them.

use crate::config::Config;
use crate::model::{Issue, Kind};
use crate::report::{self, Place, Workplace};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Checkout {
    /// Path relative to the main checkout; "." names the main checkout.
    pub key: String,
    /// Absolute path used for I/O, never sent to clients as the key.
    pub path: PathBuf,
    pub branch: String,
    pub main: bool,
    pub milestone: Option<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Commits,
    Branch,
    Working,
}
#[derive(Debug, Clone, serde::Serialize)]
pub struct Offer {
    pub scope: Scope,
    pub checkout: Option<String>,
    pub branch: Option<String>,
    pub owner: Option<String>,
}
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Offered {
    pub order: Vec<Offer>,
}

/// Find the issue represented by a checkout using the same names and marks as
/// the placement report. Duplicate IDs resolve to the last row, as Load does.
pub fn issue_of<'a>(issues: &'a [Issue], checkout: &Checkout, workplaces: &[Workplace]) -> Option<&'a Issue> {
    if checkout.main {
        return None;
    }
    let lookup = |id: &str| issues.iter().rev().find(|i| i.id == id);
    if let Some(workplace) = workplaces.iter().find(|w| w.path.to_string_lossy() == checkout.key) {
        if let Some(issue) = workplace.names.iter().find_map(|id| lookup(id)) {
            return Some(issue);
        }
        if workplace.marked.len() == 1 {
            return workplace.marked.iter().next().and_then(|id| lookup(id));
        }
    }
    checkout.milestone.as_deref().and_then(lookup).filter(|i| i.kind == Kind::Milestone)
}
fn owner_of<'a>(issues: &'a [Issue], checkout: &Checkout, workplaces: &[Workplace]) -> Option<&'a Issue> {
    if checkout.main {
        return None;
    }
    let lookup = |id: &str| issues.iter().rev().find(|i| i.id == id);
    workplaces
        .iter()
        .find(|w| w.path.to_string_lossy() == checkout.key)
        .and_then(|w| w.names.iter().find_map(|id| lookup(id)))
        .or_else(|| checkout.milestone.as_deref().and_then(lookup).filter(|i| i.kind == Kind::Milestone))
}
/// `commits` contains only this row's non-tracker hashes, already collected by
/// the caller. This function does not read the filesystem or invoke Git.
pub fn scopes(
    issues: &[Issue],
    cfg: &Config,
    id: &str,
    workplaces: &[Workplace],
    checkouts: &[Checkout],
    commits: &[String],
    now: &str,
) -> Offered {
    let Some(issue) = issues.iter().rev().find(|i| i.id == id) else { return Offered::default() };
    let footing = report::Footing::of(issues, cfg);
    let places = report::places_in(&footing, workplaces, now);
    let at = match places.get(id) {
        Some(Place::At(trees)) => {
            trees.iter().find_map(|w| checkouts.iter().find(|c| w.path.to_string_lossy() == c.key))
        }
        _ => None,
    };
    let named = || {
        workplaces
            .iter()
            .filter(|w| w.names.contains(id))
            .find_map(|w| checkouts.iter().find(|c| w.path.to_string_lossy() == c.key))
    };
    let milestone = || {
        (issue.kind == Kind::Milestone).then(|| checkouts.iter().find(|c| c.milestone.as_deref() == Some(id))).flatten()
    };
    let selected = at.or_else(named).or_else(milestone);
    let owner = selected.and_then(|c| owner_of(issues, c, workplaces)).map(|i| i.id.clone());
    let owns = owner.as_deref() == Some(id);
    let mut order = Vec::new();
    let make = |scope, checkout: Option<&Checkout>| Offer {
        scope,
        checkout: checkout.map(|c| c.key.clone()),
        branch: checkout.map(|c| c.branch.clone()),
        owner: checkout.and_then(|c| owner_of(issues, c, workplaces)).map(|i| i.id.clone()),
    };
    let commit_offer =
        || (!commits.is_empty()).then(|| make(Scope::Commits, selected.or_else(|| checkouts.iter().find(|c| c.main))));
    let work_offer = || selected.map(|c| make(Scope::Working, Some(c)));
    let branch_offer = || selected.filter(|c| !c.main).map(|c| make(Scope::Branch, Some(c)));
    if owns {
        order.extend([work_offer(), branch_offer(), commit_offer()].into_iter().flatten());
    } else {
        order.extend([commit_offer(), work_offer(), branch_offer()].into_iter().flatten());
    }
    Offered { order }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Status;
    use std::collections::BTreeSet;
    fn issue(id: &str, kind: Kind, status: &str) -> Issue {
        Issue::new(id.into(), id.into(), kind, Status::new(status), "2026-01-01T00:00:00Z")
    }
    fn checkout(key: &str, branch: &str, main: bool, milestone: Option<&str>) -> Checkout {
        Checkout {
            key: key.into(),
            path: PathBuf::from(format!("/tmp/{key}")),
            branch: branch.into(),
            main,
            milestone: milestone.map(str::to_owned),
        }
    }
    fn workplace(key: &str, names: &[&str], marked: &[&str]) -> Workplace {
        Workplace {
            path: key.into(),
            branch: "worktree-moai-aaaa".into(),
            names: names.iter().map(|s| (*s).to_owned()).collect::<BTreeSet<_>>(),
            marked: marked.iter().map(|s| (*s).to_owned()).collect(),
            broken: false,
        }
    }
    #[test]
    fn owner_and_member_get_different_orders_from_the_same_placement() {
        let cfg = Config::parse("prefix = \"moai\"\n").unwrap();
        let epic = issue("moai-aaaa", Kind::Epic, "todo");
        let mut member = issue("moai-aaaa.bbb", Kind::Issue, "in_progress");
        member.epic = Some(epic.id.clone());
        let issues = vec![epic, member];
        let workplaces = vec![workplace(".worktrees/a", &["moai-aaaa"], &[])];
        let checkouts =
            vec![checkout(".", "main", true, None), checkout(".worktrees/a", "worktree-moai-aaaa", false, None)];
        let hashes = vec!["hash".to_owned()];
        let now = "2026-01-02T00:00:00Z";
        let owner = scopes(&issues, &cfg, "moai-aaaa", &workplaces, &checkouts, &hashes, now);
        assert_eq!(
            owner.order.iter().map(|o| o.scope).collect::<Vec<_>>(),
            [Scope::Working, Scope::Branch, Scope::Commits]
        );
        let member = scopes(&issues, &cfg, "moai-aaaa.bbb", &workplaces, &checkouts, &hashes, now);
        assert_eq!(
            member.order.iter().map(|o| o.scope).collect::<Vec<_>>(),
            [Scope::Commits, Scope::Working, Scope::Branch]
        );
        assert_eq!(member.order[1].owner.as_deref(), Some("moai-aaaa"));
        assert_eq!(member.order[1].checkout.as_deref(), Some(".worktrees/a"));
    }
    #[test]
    fn marked_nameless_and_milestone_are_resolved_without_inventing_an_owner() {
        let cfg = Config::parse("prefix = \"moai\"\n").unwrap();
        let i = issue("moai-bbbb", Kind::Issue, "in_progress");
        let m = issue("moai-cccc", Kind::Milestone, "todo");
        let issues = vec![i, m];
        let work = vec![workplace("w", &[], &["moai-bbbb"])];
        let checkouts = vec![
            checkout(".", "main", true, None),
            checkout("w", "agent", false, None),
            checkout("milestone", "milestone/moai-cccc", false, Some("moai-cccc")),
        ];
        assert!(issue_of(&issues, &checkouts[0], &work).is_none());
        assert_eq!(issue_of(&issues, &checkouts[1], &work).unwrap().id, "moai-bbbb");
        let o = scopes(&issues, &cfg, "moai-bbbb", &work, &checkouts, &["hash".into()], "2026-01-02T00:00:00Z");
        assert_eq!(o.order[0].scope, Scope::Commits);
        assert!(o.order[1].owner.is_none());
        let o = scopes(&issues, &cfg, "moai-cccc", &work, &checkouts, &[], "2026-01-02T00:00:00Z");
        assert_eq!(o.order.iter().map(|o| o.scope).collect::<Vec<_>>(), [Scope::Working, Scope::Branch]);
        assert_eq!(o.order[0].owner.as_deref(), Some("moai-cccc"));
    }
    #[test]
    fn closed_named_worktree_and_duplicate_ids_use_the_last_row() {
        let cfg = Config::parse("prefix = \"moai\"\n").unwrap();
        let old = issue("moai-dddd", Kind::Issue, "todo");
        let mut newer = issue("moai-dddd", Kind::Epic, "done");
        newer.title = "new".into();
        let issues = vec![old, newer];
        let work = vec![workplace("w", &["moai-dddd"], &[])];
        let checkouts = vec![checkout("w", "worktree-moai-dddd", false, None)];
        assert_eq!(issue_of(&issues, &checkouts[0], &work).unwrap().title, "new");
        assert_eq!(
            scopes(&issues, &cfg, "moai-dddd", &work, &checkouts, &[], "2026-01-02T00:00:00Z").order[0].scope,
            Scope::Working
        );
    }
}
