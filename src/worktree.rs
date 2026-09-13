//! 다른 git 워크트리의 스냅샷을 **보여줄 때만** 겹친다.
//!
//! 에이전트 여럿이 워크트리를 하나씩 잡고 일하면 제 워크트리의 스냅샷은 그들이
//! 집고 옮긴 것을 모른다. `--worktree` 는 그것을 한 화면에 올린다.
//!
//! **어느 파일에도 쓰지 않는다.** 겹친 결과는 명령 하나가 끝나면 사라진다. 파일에
//! 합쳐 쓰는 순간 옛 moai 의 `merge=union` 이 돌아온다 — id 가 겹치고, 겹친 id 를
//! 가르려 relabel 하고, 그 별칭을 모든 조회가 풀어야 했다. 쓰기는 여전히
//! `store::with_write` 가 **제 워크트리 파일에만** 한다.
//!
//! **출처도 저장하지 않는다.** 줄이 어느 브랜치에서 왔는지는 겹칠 때만 아는
//! 파생값이라 [`Origin`] 으로 곁에 들고 다니고, `report`·`query` 는 겹친
//! `&[Issue]` 만 받는다 — 그쪽은 이 기능이 있는 줄 모른다.

use crate::model::Issue;
use crate::store::{Load, Repo};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// 제 워크트리가 아닌 워크트리 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tree {
    /// 워크트리의 꼭대기.
    pub path: PathBuf,
    /// 화면에 댈 이름 — 브랜치, 떼어 낸 HEAD 면 커밋 앞 일곱 자.
    pub label: String,
}

/// 줄 id → 그 줄을 보여 준 워크트리. **제 워크트리에서 온 줄은 없다** — 없다는
/// 것이 곧 "지금 브랜치의 줄" 이라는 뜻이라, 받는 쪽이 한 규칙으로 읽는다.
#[derive(Debug, Default)]
pub struct Origin {
    from: BTreeMap<String, usize>,
    /// (이름, 그 워크트리의 moai 뿌리). 뿌리는 이력(저널)을 읽을 때 쓴다.
    trees: Vec<(String, PathBuf)>,
    /// 제 파일에는 없고 옆에서만 온 줄. 제 줄을 **덮은** 것과 가른다 —
    /// [`Origin::unreadable`] 이 그 차이로 거짓 중복을 거른다.
    added: BTreeSet<String>,
}

impl Origin {
    /// 그 줄을 보여 준 브랜치. 지금 브랜치의 줄이면 `None`.
    pub fn branch(&self, id: &str) -> Option<&str> {
        self.from.get(id).map(|&k| self.trees[k].0.as_str())
    }

    /// 겹쳐 본 워크트리의 이름들 — 줄을 하나도 안 보탠 곳까지. 겹쳐 봤는데 옆이
    /// 조용한 것과 아예 안 겹쳐 본 것을 화면이 가를 수 있어야 한다.
    pub fn labels(&self) -> Vec<&str> {
        self.trees.iter().map(|(l, _)| l.as_str()).collect()
    }

    /// 다른 브랜치에서 온 줄 전부 — id → 브랜치. `--json` 이 이 모양으로 낸다.
    pub fn branches(&self) -> BTreeMap<&str, &str> {
        self.from.iter().map(|(id, &k)| (id.as_str(), self.trees[k].0.as_str())).collect()
    }

    /// 그 줄을 보여 준 워크트리의 moai 뿌리 — **이력은 줄이 온 곳에서 읽는다.**
    /// 스냅샷은 저쪽 줄을 내면서 이력은 이쪽 저널에서 읽으면, 저쪽에서 옮긴
    /// 칸이 이력에 없어 상세가 제 머리글과 모순된다.
    pub fn root(&self, id: &str) -> Option<&Path> {
        self.from.get(id).map(|&k| self.trees[k].1.as_path())
    }

    /// 제 파일의 못 읽는 줄이 쓰는 id 를 `report::Unreadable` 에 넘길 모양으로 고른다.
    ///
    /// **옆에서만 온 줄과 겹치는 것은 중복이 아니다.** 제 파일에 X 가 못 읽는 줄로만
    /// 있고 옆 워크트리에 X 가 멀쩡하면, 그대로 넘기는 순간 `--worktree` 를 붙였을
    /// 때만 `duplicate_id` 가 서는데 그 두 줄은 한 파일에 있지 않다. 그런 id 는 첫
    /// 번만 떼고, 둘째부터는 남긴다 — 제 파일 안에서 못 읽는 줄끼리 겹친 것은 여전히
    /// 한 번 드러나야 한다.
    pub fn unreadable<'a>(&self, ids: impl Iterator<Item = Option<&'a str>>) -> Vec<Option<&'a str>> {
        let mut first = BTreeSet::new();
        ids.map(|id| id.filter(|id| !self.added.contains(*id) || !first.insert(*id))).collect()
    }
}

/// `git worktree list --porcelain -z` 를 읽는다.
///
/// **`-z` 를 쓴다** — 경로에 줄바꿈이 들 수 있고, 줄로 가르면 그런 워크트리가
/// 둘로 쪼개져 없는 경로를 읽으러 간다. 맨몸(`bare`) 저장소와 경로가 사라진
/// 것(`prunable`)은 읽을 스냅샷이 없어 뺀다.
pub fn parse(porcelain: &str) -> Vec<Tree> {
    let mut out = Vec::new();
    // 레코드는 빈 필드(NUL 두 개)로 끝난다.
    for record in porcelain.split("\0\0") {
        let mut path = None;
        let mut head = None;
        let mut branch = None;
        let mut skip = false;
        for field in record.split('\0') {
            let (key, value) = field.split_once(' ').unwrap_or((field, ""));
            match key {
                "worktree" => path = Some(PathBuf::from(value)),
                "HEAD" => head = Some(value),
                "branch" => branch = Some(value.strip_prefix("refs/heads/").unwrap_or(value)),
                "bare" | "prunable" => skip = true,
                _ => {}
            }
        }
        let Some(path) = path.filter(|_| !skip) else { continue };
        let label = match (branch, head) {
            (Some(b), _) => b.to_string(),
            (None, Some(h)) => h.chars().take(7).collect(),
            (None, None) => continue,
        };
        out.push(Tree { path, label });
    }
    out
}

/// 겹칠 줄들을 **제 워크트리가 먼저**인 차례로 받아 하나로 보인다.
///
/// 같은 id 는 `updated_at` 이 가장 늦은 줄이 선다. **같으면 앞선 쪽** — 제
/// 워크트리가 맨 앞이라 동률이면 지금 브랜치의 줄이고, 남끼리는 git 이 댄
/// 차례다. 결정적이어야 부를 때마다 같은 줄이 선다. 시각은 RFC3339 UTC
/// 고정폭이라 문자열로 견준다(`model::now`).
///
/// **제 줄은 한 줄도 접지 않는다.** 제 파일에 같은 id 가 둘이면 둘 다 남긴다 —
/// 여기서 접으면 `moai status` 의 `duplicate_id` 가 `--worktree` 를 붙인
/// 순간에만 사라져, 깨진 파일이 멀쩡해 보인다.
pub fn overlay(mine: Vec<Issue>, others: Vec<(String, PathBuf, Vec<Issue>)>) -> (Vec<Issue>, Origin) {
    let mut shown = mine;
    let mut origin = Origin::default();
    // id → `shown` 의 자리. 제 줄은 첫 자리만 적는다 — 둘째는 이미 깨진 줄이라
    // 남의 줄로 덮을 자리가 아니다.
    let mut at: BTreeMap<String, usize> = BTreeMap::new();
    for (k, i) in shown.iter().enumerate() {
        at.entry(i.id.clone()).or_insert(k);
    }
    for (tree, (label, root, issues)) in others.into_iter().enumerate() {
        origin.trees.push((label, root));
        for i in issues {
            match at.get(&i.id) {
                Some(&k) if i.updated_at > shown[k].updated_at => {
                    origin.from.insert(i.id.clone(), tree);
                    shown[k] = i;
                }
                Some(_) => {}
                None => {
                    at.insert(i.id.clone(), shown.len());
                    origin.added.insert(i.id.clone());
                    origin.from.insert(i.id.clone(), tree);
                    shown.push(i);
                }
            }
        }
    }
    // `store::read` 와 같은 차례로 돌려준다. **안정 정렬이다** — 제 파일의 겹친
    // id 두 줄이 읽은 차례를 지킨다.
    shown.sort_by(|a, b| a.id.cmp(&b.id));
    (shown, origin)
}

/// 겹쳐 읽은 결과.
pub struct Gathered {
    /// 제 워크트리의 `Load` 에 남의 줄을 겹친 것. **`errors` 는 제 파일의 것뿐이다**
    /// — 남의 못 읽는 줄로 `moai status` 가 비영 종료하면, 내 파일은 멀쩡한데
    /// 옆 워크트리 때문에 도구가 실패로 읽힌다.
    pub load: Load,
    pub origin: Origin,
    /// 남의 워크트리에서 만난 문제. **막지 않는다** — 부르는 쪽이 한 줄씩 알린다.
    pub trouble: Vec<String>,
}

/// 제 저장소를 읽고, `worktree` 면 다른 워크트리의 스냅샷을 겹친다.
///
/// **읽기는 관대하다.** git 이 없거나, 저장소가 아니거나, 남의 파일이 깨졌어도
/// 제 스냅샷은 그대로 낸다 — 그런 것은 `trouble` 로 말만 한다. 스냅샷 파일이
/// 없는 워크트리는 moai 를 들이기 전에 갈라진 브랜치라 **말하지도 않는다.**
pub fn gather(repo: &Repo, worktree: bool) -> crate::fail::R<Gathered> {
    let load = repo.read()?;
    if !worktree {
        return Ok(Gathered { load, origin: Origin::default(), trouble: Vec::new() });
    }
    let mut trouble = Vec::new();
    let mut others = Vec::new();
    match others_of(&repo.root) {
        Err(why) => trouble.push(why),
        Ok(trees) => {
            for (tree, root) in trees {
                let path = root.join(".moai").join("issues.jsonl");
                match crate::store::read_snapshot(&path) {
                    Err(e) => trouble.push(format!("⎇ {}: {e}", tree.label)),
                    Ok(None) => {}
                    Ok(Some(other)) => {
                        if !other.errors.is_empty() {
                            trouble.push(format!(
                                "⎇ {}: {} — 읽을 수 없는 줄 {}개는 빼고 겹쳤다",
                                tree.label,
                                path.display(),
                                other.errors.len()
                            ));
                        }
                        others.push((tree.label, root, other.issues));
                    }
                }
            }
        }
    }
    let Load { issues, errors } = load;
    let (issues, origin) = overlay(issues, others);
    Ok(Gathered { load: Load { issues, errors }, origin, trouble })
}

/// 다른 워크트리마다 (워크트리, 그 안의 moai 뿌리).
///
/// moai 뿌리가 워크트리 꼭대기가 아닐 수 있다(`.moai/` 를 하위 디렉터리에 둔
/// 저장소). **제 뿌리가 꼭대기에서 떨어진 만큼 남의 꼭대기에서도 떨어뜨린다** —
/// 같은 저장소의 워크트리는 같은 나무 모양이다.
fn others_of(root: &Path) -> Result<Vec<(Tree, PathBuf)>, String> {
    let git = |args: &[&str]| -> Result<String, String> {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .map_err(|e| format!("git 을 부르지 못해 워크트리를 못 찾았다 — {e}"))?;
        if !out.status.success() {
            return Err(format!(
                "워크트리를 못 찾았다 — {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        String::from_utf8(out.stdout).map_err(|e| format!("워크트리 목록을 못 읽었다 — {e}"))
    };
    let top = git(&["rev-parse", "--show-toplevel"])?;
    let top = canonical(Path::new(top.trim_end_matches('\n')));
    let rel = canonical(root).strip_prefix(&top).map(Path::to_path_buf).unwrap_or_default();
    // `-z` 는 git 2.36 부터다. 그 전 git 에서 거절되면 줄로 가른 것을 NUL 로 바꿔
    // 같은 파서로 읽는다 — 줄바꿈 든 경로만 잃고, 겹쳐 보기 전체를 잃지는 않는다.
    let listed = git(&["worktree", "list", "--porcelain", "-z"])
        .or_else(|_| git(&["worktree", "list", "--porcelain"]).map(|s| s.replace('\n', "\0")))?;
    Ok(parse(&listed)
        .into_iter()
        .filter(|t| canonical(&t.path) != top)
        .map(|t| {
            let root = t.path.join(&rel);
            (t, root)
        })
        .collect())
}

/// 견줄 수 있는 경로. 못 풀면(사라진 경로) 받은 그대로 — 그런 경로는 어차피
/// 제 워크트리와 같을 수 없다.
fn canonical(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Kind, Status};

    fn issue(id: &str, status: &str, updated: &str) -> Issue {
        let mut i = Issue::new(id.into(), format!("제목 {id}"), Kind::Issue, Status::new(status), "2026-09-10T00:00:00Z");
        i.updated_at = updated.into();
        i
    }

    fn tree(label: &str, issues: Vec<Issue>) -> (String, PathBuf, Vec<Issue>) {
        (label.into(), PathBuf::from(format!("/wt/{label}")), issues)
    }

    #[test]
    fn porcelain_names_branches_and_detached_heads_and_skips_bare_and_prunable() {
        let src = "worktree /repo\0HEAD 1111111aaaa\0branch refs/heads/main\0\0\
                   worktree /repo/.wt/x\0HEAD 2222222bbbb\0branch refs/heads/feat/x\0\0\
                   worktree /tmp/detached\0HEAD 3333333cccc\0detached\0\0\
                   worktree /gone\0HEAD 4444444dddd\0branch refs/heads/old\0prunable gitdir file points to non-existent location\0\0\
                   worktree /bare.git\0bare\0\0";
        let got: Vec<(String, String)> =
            parse(src).into_iter().map(|t| (t.path.display().to_string(), t.label)).collect();
        assert_eq!(
            got,
            [
                ("/repo".to_string(), "main".to_string()),
                ("/repo/.wt/x".into(), "feat/x".into()),
                ("/tmp/detached".into(), "3333333".into()),
            ]
        );
    }

    /// 경로에 줄바꿈이 들어도 한 워크트리다.
    #[test]
    fn a_newline_in_a_path_does_not_split_the_worktree() {
        let got = parse("worktree /a\nb\0HEAD 1234567890\0branch refs/heads/nl\0\0");
        assert_eq!(got, [Tree { path: PathBuf::from("/a\nb"), label: "nl".into() }]);
    }

    #[test]
    fn the_latest_line_wins_and_names_where_it_came_from() {
        let mine = vec![issue("m-0001", "todo", "2026-09-12T00:00:00Z"), issue("m-0002", "todo", "2026-09-12T00:00:00Z")];
        let (shown, origin) = overlay(
            mine,
            vec![tree("feat/x", vec![
                issue("m-0001", "in_progress", "2026-09-13T00:00:00Z"),
                issue("m-0002", "done", "2026-09-11T00:00:00Z"),
            ])],
        );
        assert_eq!(shown[0].status.as_str(), "in_progress", "늦은 남의 줄이 서야 한다");
        assert_eq!(origin.branch("m-0001"), Some("feat/x"));
        assert_eq!(origin.root("m-0001"), Some(Path::new("/wt/feat/x")));
        assert_eq!(shown[1].status.as_str(), "todo", "이른 남의 줄이 제 줄을 덮었다");
        assert_eq!(origin.branch("m-0002"), None, "제 줄인데 출처가 붙었다");
    }

    /// 동률이면 제 줄, 남끼리는 앞선 워크트리.
    #[test]
    fn a_tie_keeps_the_current_branch_then_the_earlier_worktree() {
        let at = "2026-09-13T00:00:00Z";
        let (shown, origin) = overlay(
            vec![issue("m-0001", "todo", at)],
            vec![
                tree("a", vec![issue("m-0001", "done", at), issue("m-0002", "review", at)]),
                tree("b", vec![issue("m-0001", "review", at), issue("m-0002", "done", at)]),
            ],
        );
        assert_eq!(shown[0].status.as_str(), "todo");
        assert_eq!(origin.branch("m-0001"), None);
        assert_eq!(shown[1].status.as_str(), "review");
        assert_eq!(origin.branch("m-0002"), Some("a"));
    }

    /// 남에게만 있는 줄도 서고, 차례는 id 다.
    #[test]
    fn a_line_only_elsewhere_is_shown_in_id_order() {
        let (shown, origin) = overlay(
            vec![issue("m-0003", "todo", "2026-09-12T00:00:00Z")],
            vec![tree("feat/x", vec![issue("m-0001", "todo", "2026-09-01T00:00:00Z")])],
        );
        let ids: Vec<&str> = shown.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["m-0001", "m-0003"]);
        assert_eq!(origin.branch("m-0001"), Some("feat/x"));
    }

    /// 제 파일의 겹친 id 는 접지 않는다 — `duplicate_id` 가 사라지면 안 된다.
    #[test]
    fn duplicates_in_my_own_file_survive_the_overlay() {
        let at = "2026-09-12T00:00:00Z";
        let (shown, _) = overlay(
            vec![issue("m-0001", "todo", at), issue("m-0001", "review", at)],
            vec![tree("feat/x", vec![])],
        );
        assert_eq!(shown.len(), 2);
        assert_eq!(shown[1].status.as_str(), "review", "읽은 차례가 뒤집혔다");
    }

    /// 제 파일의 못 읽는 줄이 옆에서만 온 id 를 쓰면 중복으로 넘기지 않는다. 제 줄을
    /// 덮은 id 나, 못 읽는 줄끼리 겹친 것은 그대로 넘긴다.
    #[test]
    fn an_unreadable_line_is_not_a_duplicate_of_a_line_only_elsewhere() {
        let at = "2026-09-12T00:00:00Z";
        let later = "2026-09-13T00:00:00Z";
        let (_, origin) = overlay(
            vec![issue("m-0002", "todo", at)],
            vec![tree("feat/x", vec![issue("m-0001", "todo", at), issue("m-0002", "review", later)])],
        );
        assert_eq!(origin.unreadable([Some("m-0001")].into_iter()), [None]);
        assert_eq!(origin.unreadable([Some("m-0002")].into_iter()), [Some("m-0002")]);
        assert_eq!(origin.unreadable([Some("m-0001"), Some("m-0001")].into_iter()), [None, Some("m-0001")]);
        assert_eq!(origin.unreadable([None, Some("m-0009")].into_iter()), [None, Some("m-0009")]);
    }
}
