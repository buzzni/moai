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
    /// 그 워크트리의 HEAD 커밋. 갈라진 자리(`merge-base`)를 찾는 데 쓴다.
    pub head: String,
}

/// 겹칠 옆 워크트리 하나의 줄들.
#[derive(Debug, Default)]
pub struct Side {
    pub label: String,
    /// 그 워크트리의 moai 뿌리 — 이력을 읽을 곳.
    pub root: PathBuf,
    pub issues: Vec<Issue>,
    /// 제 워크트리와 **갈라진 자리의 스냅샷**에 있던 id → 그때의 `updated_at`.
    ///
    /// 옆에만 있는 줄이 "여기서 지웠다" 인지 "여기서 아직 안 받았다" 인지는 지금
    /// 스냅샷 둘로는 못 가른다. 갈라진 자리에 있었으면 이쪽 역사에도 있었던 줄이니
    /// 여기서 지운 것이다. 저널이 아니라 **커밋된 스냅샷**을 읽는다 — 저널은 상태
    /// 계산에 읽히지 않는다(moai-0a0u). 못 찾으면 비어 있고, 그러면 전처럼 다 선다.
    pub base: BTreeMap<String, String>,
}

impl Side {
    pub fn new(label: impl Into<String>, root: impl Into<PathBuf>, issues: Vec<Issue>) -> Side {
        Side { label: label.into(), root: root.into(), issues, base: BTreeMap::new() }
    }
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
        out.push(Tree { path, label, head: head.unwrap_or_default().to_string() });
    }
    out
}

/// 겹칠 줄들을 **제 워크트리가 먼저**인 차례로 받아 하나로 보인다.
///
/// 같은 id 는 **계획에서의 자리를 늦게 바꾼 줄**([`Issue::planned`] — 칸을 옮기거나
/// 미루거나 도로 집은 때)이 통째로 선다. 같으면
/// `updated_at` 이 늦은 줄, 그것도 같으면 앞선 쪽 — 제 워크트리가 맨 앞이라
/// 동률이면 지금 브랜치의 줄이고, 남끼리는 git 이 댄 차례다. 결정적이어야 부를
/// 때마다 같은 줄이 선다. 시각은 RFC3339 UTC 고정폭이라 문자열로 견준다(`model::now`).
///
/// **칸이 먼저인 까닭** — 제목·우선순위·태그를 고친 것도 `updated_at` 을 올린다.
/// 그것으로만 견주면 옆에서 집은 뒤 여기서 우선순위 하나만 고쳐도 옆 줄이 가려져
/// `ready --worktree` 가 옆에서 잡은 일을 다시 집으라고 낸다(moai-2f5g). 칸만 보면
/// 옆에서 늦게 미룬 것이 여기서 먼저 집은 칸에 가려진다 — 미루기는 칸을 안 옮긴다
/// (moai-l11z). 필드마다
/// 따로 고르지는 않는다 — 한 줄의 출처가 둘이면 `⎇` 와 이력을 읽을 뿌리가 갈린다.
///
/// **제 줄은 한 줄도 접지 않는다.** 제 파일에 같은 id 가 둘이면 둘 다 남긴다 —
/// 여기서 접으면 `moai status` 의 `duplicate_id` 가 `--worktree` 를 붙인
/// 순간에만 사라져, 깨진 파일이 멀쩡해 보인다.
///
/// **여기서 지운 줄은 되살리지 않는다.** 옆에만 있는 줄이 갈라진 자리
/// ([`Side::base`])에도 있었고 그 뒤로 옆에서 안 만졌으면 세우지 않는다. 옆에서
/// 그 뒤에 집거나 고쳤으면 세운다 — 지운 것과 옆의 작업이 부딪힌 것을 감추면
/// 옆에서 하던 일이 화면에서 사라진다. **메모는 만진 것으로 안 센다** — `note` 는
/// 스냅샷을 안 바꾸고, 그것을 세려고 옆 저널을 읽으면 "저널은 상태 계산에 읽히지
/// 않는다" 가 무너진다(moai-dyeu, 사용자와 정함). 옆에서 지운 줄(여기에만 있다)은 그대로
/// 둔다 — 그 삭제는 브랜치가 합쳐질 때 반영된다.
pub fn overlay(mine: Vec<Issue>, others: Vec<Side>) -> (Vec<Issue>, Origin) {
    let mut shown = mine;
    let mut origin = Origin::default();
    // id → `shown` 의 자리. 제 줄이 둘이면 **뒷자리를** 적는다 — `store::Load::get`·
    // 트리·탐색기가 모두 뒷줄을 연다. 앞자리를 덮으면 `show <id> --worktree` 가 덮지
    // 않은 낡은 뒷줄을 열면서 이력은 옆 워크트리에서 읽어, 머리와 이력이 서로 다른
    // 줄을 말한다.
    let mut at: BTreeMap<String, usize> = BTreeMap::new();
    for (k, i) in shown.iter().enumerate() {
        at.insert(i.id.clone(), k);
    }
    for (tree, Side { label, root, issues, base }) in others.into_iter().enumerate() {
        origin.trees.push((label, root));
        for i in issues {
            match at.get(&i.id) {
                Some(&k) if (i.planned(), i.updated_at.as_str()) > (shown[k].planned(), shown[k].updated_at.as_str()) => {
                    origin.from.insert(i.id.clone(), tree);
                    shown[k] = i;
                }
                Some(_) => {}
                None if base.get(&i.id).is_some_and(|then| i.updated_at <= *then) => {}
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
    /// 옆 워크트리를 **찾지 못한** 까닭(git 이 없거나 저장소가 아니다). `trouble` 과 가른다 —
    /// `--worktree` 를 시킨 CLI 는 말하지만, 겹쳐 보기를 기본으로 켜는 탐색기는 git 밖의
    /// 프로젝트를 열 때마다 시키지 않은 배너를 세우게 된다(moai-zcuh). 탐색기는 경로 줄의
    /// "옆 워크트리 없음" 으로 말한다.
    pub unfound: Option<String>,
    /// 읽으러 간 옆 스냅샷마다 **읽기 전에** 잰 표식. 탐색기가 바뀐 것을 알아채는 데
    /// 쓴다. 파일이 없던 곳도 든다 — 거기 스냅샷이 생기는 것도 바뀐 것이다.
    ///
    /// **재는 것이 읽는 것보다 먼저다** — 읽고 나서 재면 그 사이에 떨어진 쓰기가
    /// "이미 본 것" 으로 적혀 영영 안 보인다(`cmd::tui::run` 과 같은 까닭).
    pub watched: Vec<(PathBuf, crate::store::Stamp)>,
}

/// 제 저장소를 읽고, `worktree` 면 다른 워크트리의 스냅샷을 겹친다.
///
/// **읽기는 관대하다.** git 이 없거나, 저장소가 아니거나, 남의 파일이 깨졌어도
/// 제 스냅샷은 그대로 낸다 — 그런 것은 `trouble` 로 말만 한다. 스냅샷 파일이
/// 없는 워크트리는 moai 를 들이기 전에 갈라진 브랜치라 **말하지도 않는다.**
pub fn gather(repo: &Repo, worktree: bool) -> crate::fail::R<Gathered> {
    let load = repo.read()?;
    if !worktree {
        return Ok(Gathered {
            load,
            origin: Origin::default(),
            trouble: Vec::new(),
            unfound: None,
            watched: Vec::new(),
        });
    }
    let mut trouble = Vec::new();
    let mut unfound = None;
    let mut others = Vec::new();
    let mut watched = Vec::new();
    match others_of(&repo.root) {
        Err(why) => unfound = Some(why),
        Ok((mine, trees)) => {
            let here: std::collections::HashSet<&str> = load.issues.iter().map(|i| i.id.as_str()).collect();
            for (tree, root) in trees {
                let path = root.join(".moai").join("issues.jsonl");
                watched.push((path.clone(), crate::store::stamp(&path)));
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
                        // 갈라진 자리는 옆에만 있는 줄을 가를 때만 쓴다. 다 여기에도 있으면
                        // git 을 두 번 더 부르지 않는다 — 탐색기는 다시 읽을 때마다 여기를 지난다.
                        let lonely = other.issues.iter().any(|i| !here.contains(i.id.as_str()));
                        let base = match mine.as_deref() {
                            Some(m) if lonely => base_of(&repo.root, m, &tree.head),
                            _ => BTreeMap::new(),
                        };
                        others.push(Side { base, ..Side::new(tree.label, root, other.issues) });
                    }
                }
            }
        }
    }
    let Load { issues, errors } = load;
    let (issues, origin) = overlay(issues, others);
    Ok(Gathered { load: Load { issues, errors }, origin, trouble, unfound, watched })
}

/// 제 워크트리의 HEAD 와, 다른 워크트리마다 (워크트리, 그 안의 moai 뿌리).
///
/// moai 뿌리가 워크트리 꼭대기가 아닐 수 있다(`.moai/` 를 하위 디렉터리에 둔
/// 저장소). **제 뿌리가 꼭대기에서 떨어진 만큼 남의 꼭대기에서도 떨어뜨린다** —
/// 같은 저장소의 워크트리는 같은 나무 모양이다.
fn others_of(root: &Path) -> Result<(Option<String>, Vec<(Tree, PathBuf)>), String> {
    let top = git(root, &["rev-parse", "--show-toplevel"])?;
    let top = canonical(Path::new(top.trim_end_matches('\n')));
    let rel = canonical(root).strip_prefix(&top).map(Path::to_path_buf).unwrap_or_default();
    // `-z` 는 git 2.36 부터다. 그 전 git 에서 거절되면 줄로 가른 것을 NUL 로 바꿔
    // 같은 파서로 읽는다 — 줄바꿈 든 경로만 잃고, 겹쳐 보기 전체를 잃지는 않는다.
    let listed = git(root, &["worktree", "list", "--porcelain", "-z"])
        .or_else(|_| git(root, &["worktree", "list", "--porcelain"]).map(|s| s.replace('\n', "\0")))?;
    let (mine, others): (Vec<Tree>, Vec<Tree>) = parse(&listed).into_iter().partition(|t| canonical(&t.path) == top);
    let mine = mine.into_iter().next().map(|t| t.head).filter(|h| !h.is_empty());
    Ok((
        mine,
        others
            .into_iter()
            .map(|t| {
                let root = t.path.join(&rel);
                (t, root)
            })
            .collect(),
    ))
}

/// 제 HEAD 와 옆 HEAD 가 갈라진 자리의 스냅샷 — id → 그때의 `updated_at` ([`Side::base`]).
///
/// **못 찾으면 비어 있고, 말하지 않는다.** 이어지지 않는 역사, 아직 커밋이 없는 브랜치,
/// 그 커밋에 스냅샷이 없는 것(moai 를 들이기 전에 갈라졌다)은 모두 "지운 줄을 가를
/// 바탕이 없다" 는 뜻이라 전처럼 다 세운다. 옆 파일이 멀쩡한데 말이 서면 안 된다.
fn base_of(root: &Path, mine: &str, theirs: &str) -> BTreeMap<String, String> {
    let mut then = BTreeMap::new();
    let Ok(base) = git(root, &["merge-base", mine, theirs]) else { return then };
    // `<커밋>:./<경로>` 는 `-C` 로 준 디렉터리에서 푼다 — moai 뿌리가 꼭대기가 아니어도 된다.
    let Ok(src) = git(root, &["show", &format!("{}:./.moai/issues.jsonl", base.trim())]) else { return then };
    for i in crate::store::parse_issues(&src).issues {
        let at = then.entry(i.id).or_insert_with(String::new);
        if i.updated_at > *at {
            *at = i.updated_at;
        }
    }
    then
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|e| format!("git 을 부르지 못해 워크트리를 못 찾았다 — {e}"))?;
    if !out.status.success() {
        return Err(format!("워크트리를 못 찾았다 — {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    String::from_utf8(out.stdout).map_err(|e| format!("워크트리 목록을 못 읽었다 — {e}"))
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

    fn tree(label: &str, issues: Vec<Issue>) -> Side {
        Side::new(label, format!("/wt/{label}"), issues)
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
        assert_eq!(got, [Tree { path: PathBuf::from("/a\nb"), label: "nl".into(), head: "1234567890".into() }]);
    }

    /// 옆에만 있는 줄이 갈라진 자리에 있었으면 여기서 지운 것이다 — 옆에서 그 뒤로 안
    /// 만졌으면 안 서고, 만졌으면 선다. 갈라진 자리에 없던 줄은 전처럼 선다.
    #[test]
    fn a_line_removed_here_since_the_fork_is_not_revived_unless_touched_there() {
        let at = "2026-09-12T00:00:00Z";
        let later = "2026-09-13T00:00:00Z";
        let mut side = tree("feat/x", vec![
            issue("m-0001", "todo", at),
            issue("m-0002", "in_progress", later),
            issue("m-0003", "todo", at),
        ]);
        side.base = [("m-0001".to_string(), at.to_string()), ("m-0002".to_string(), at.to_string())].into();
        let (shown, origin) = overlay(vec![], vec![side]);
        let ids: Vec<&str> = shown.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["m-0002", "m-0003"], "지운 줄이 되살았거나 옆의 작업이 사라졌다");
        assert_eq!(origin.branch("m-0001"), None);
        assert_eq!(origin.unreadable([Some("m-0001")].into_iter()), [Some("m-0001")]);

        // 둘째 워크트리가 같은 줄을 그 뒤에 만졌으면 그 줄은 선다.
        let mut quiet = tree("a", vec![issue("m-0001", "todo", at)]);
        quiet.base = [("m-0001".to_string(), at.to_string())].into();
        let mut busy = tree("b", vec![issue("m-0001", "review", later)]);
        busy.base = quiet.base.clone();
        let (shown, origin) = overlay(vec![], vec![quiet, busy]);
        assert_eq!(shown.len(), 1);
        assert_eq!(origin.branch("m-0001"), Some("b"));
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

    /// 칸을 늦게 옮긴 줄이 선다 — 그 뒤에 필드만 고친 줄은 칸을 덮지 못한다(moai-2f5g).
    #[test]
    fn a_later_move_beats_a_later_field_edit() {
        let mut picked = issue("m-0001", "in_progress", "2026-09-12T00:00:00Z");
        picked.status_since = "2026-09-12T00:00:00Z".into();
        let edited = issue("m-0001", "todo", "2026-09-13T00:00:00Z");
        let (shown, origin) = overlay(vec![edited], vec![tree("feat/x", vec![picked])]);
        assert_eq!(shown[0].status.as_str(), "in_progress", "늦은 필드 편집이 옆에서 집은 것을 풀었다");
        assert_eq!(origin.branch("m-0001"), Some("feat/x"));

        // 거꾸로 — 여기서 늦게 옮겼으면 옆의 늦은 필드 편집이 덮지 못한다.
        let mut moved = issue("m-0002", "done", "2026-09-12T00:00:00Z");
        moved.status_since = "2026-09-12T00:00:00Z".into();
        let theirs = issue("m-0002", "todo", "2026-09-13T00:00:00Z");
        let (shown, origin) = overlay(vec![moved], vec![tree("feat/x", vec![theirs])]);
        assert_eq!(shown[0].status.as_str(), "done");
        assert_eq!(origin.branch("m-0002"), None);
    }

    /// 옆에서 늦게 미룬 것·도로 집은 것이 여기서 먼저 옮긴 칸에 안 가려진다(moai-l11z). 미루기는
    /// `status_since` 를 안 올리고, 도로 집기는 `deferred_at` 을 지운다 — 남는 시각은 `planned_at` 이다.
    #[test]
    fn a_later_defer_or_undo_beats_an_earlier_move() {
        let (t1, t2, t3, t4) =
            ("2026-09-12T00:00:00Z", "2026-09-13T00:00:00Z", "2026-09-14T00:00:00Z", "2026-09-15T00:00:00Z");
        let mut picked = issue("m-0001", "in_progress", t1);
        picked.status_since = t1.into();
        let mut shelved = issue("m-0001", "todo", t2);
        shelved.deferred_at = Some(t2.into());
        shelved.planned_at = Some(t2.into());
        let (shown, origin) = overlay(vec![picked], vec![tree("feat/x", vec![shelved.clone()])]);
        assert!(shown[0].is_deferred(), "옆에서 늦게 미룬 것이 여기서 먼저 집은 줄에 가려졌다");
        assert_eq!(origin.branch("m-0001"), Some("feat/x"));

        // 도로 집으면 `deferred_at` 은 사라져도 `planned_at` 이 늦어 이긴다. 여기 줄은 칸을
        // 도로 집은 줄보다 늦게 옮겼다 — 칸 시각만 보면 여기 미룬 줄이 선다.
        let mut here = shelved.clone();
        here.status_since = t2.into();
        let mut back = issue("m-0001", "todo", t3);
        back.planned_at = Some(t3.into());
        let (shown, origin) = overlay(vec![here], vec![tree("feat/x", vec![back])]);
        assert!(!shown[0].is_deferred(), "옆에서 도로 집은 것이 여기서 미룬 줄에 가려졌다");
        assert_eq!(origin.branch("m-0001"), Some("feat/x"));

        // 미룬 뒤에 칸을 옮긴 줄은 `planned_at` 이 낡았어도 이긴다 — 옛 바이너리는 칸만 옮긴다.
        let mut moved = shelved.clone();
        moved.status = Status::new("review");
        moved.status_since = t4.into();
        let mut later_shelf = issue("m-0001", "todo", t3);
        later_shelf.deferred_at = Some(t3.into());
        later_shelf.planned_at = Some(t3.into());
        let (shown, origin) = overlay(vec![later_shelf], vec![tree("feat/x", vec![moved])]);
        assert_eq!(shown[0].status.as_str(), "review", "늦게 옮긴 칸이 그 전의 미룸에 졌다");
        assert_eq!(origin.branch("m-0001"), Some("feat/x"));
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

        // 옆의 더 새 줄은 **뒷줄을** 덮는다 — 상세·트리가 여는 줄이 그것이다.
        let later = "2026-09-13T00:00:00Z";
        let (shown, _) = overlay(
            vec![issue("m-0001", "todo", at), issue("m-0001", "review", at)],
            vec![tree("feat/x", vec![issue("m-0001", "done", later)])],
        );
        let cols: Vec<&str> = shown.iter().map(|i| i.status.as_str()).collect();
        assert_eq!(cols, ["todo", "done"], "옆 줄이 앞줄을 덮었다");
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
