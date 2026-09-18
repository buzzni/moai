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
    /// 이 워크트리가 쥐었다고 볼 id 후보 — [`names`] 가 낸 것(디렉터리 이름·가지 이름·`worktree-`
    /// 를 뗀 이름). 훅이 옆의 일을 가르는 자와 **같은 자**다(moai-nxt4 리뷰).
    pub holds: BTreeSet<String>,
}

impl Side {
    /// 이름만으로 세운다 — 후보는 가지 이름에서 낸다([`names`] 의 절반, **같은 함수로**). 디렉터리
    /// 이름까지 아는 곳은 [`gather`] 다.
    pub fn new(label: impl Into<String>, root: impl Into<PathBuf>, issues: Vec<Issue>) -> Side {
        let label = label.into();
        let mut holds = BTreeSet::new();
        from_label(&label, &mut holds);
        Side { label, root: root.into(), issues, base: BTreeMap::new(), holds }
    }
}

/// 줄 id → 그 줄을 보여 준 워크트리. **제 워크트리에서 온 줄은 없다** — 없다는
/// 것이 곧 "지금 브랜치의 줄" 이라는 뜻이라, 받는 쪽이 한 규칙으로 읽는다.
#[derive(Debug, Default)]
pub struct Origin {
    from: BTreeMap<String, usize>,
    /// (이름, 그 워크트리의 moai 뿌리, 쥐었다고 볼 id 후보). 뿌리는 이력(저널)을 읽을 때 쓴다.
    trees: Vec<(String, PathBuf, BTreeSet<String>)>,
    /// 제 파일에는 없고 옆에서만 온 줄. 제 줄을 **덮은** 것과 가른다 —
    /// [`Origin::unreadable`] 이 그 차이로 거짓 중복을 거른다.
    added: BTreeSet<String>,
}

impl Origin {
    /// 그 줄을 보여 준 브랜치. 지금 브랜치의 줄이면 `None`.
    pub fn branch(&self, id: &str) -> Option<&str> {
        self.from.get(id).map(|&k| self.trees[k].0.as_str())
    }

    /// **그 이슈를 쥔 옆 워크트리**(moai-nxt4) — 그 워크트리의 이름 후보([`names`])에 id 가 들면.
    /// 규약대로 일감마다 그 id 로 가지를 띄우고 워크트리를 그 이름으로 만드므로, 이것이 곧
    /// "누가 무엇을 쥐고 있나" 다.
    ///
    /// **훅과 같은 자다**(moai-nxt4 리뷰) — `hook::held` 가 옆의 일을 초점에서 뺄 때 쓰는 [`away`]
    /// 와 같은 후보를 본다. 자가 둘이면 화면은 ⎇ 를 다는데 훅은 "옆이 쥐었다" 를 모르는 줄이 생긴다.
    /// 후보는 **통째로 같아야** 한다 — 자식 id(`부모.자식`)의 가지가 부모 줄에 붙지 않는다.
    ///
    /// [`Origin::branch`] 와 가르는 것: 그쪽은 **줄이 어디서 왔나**(스냅샷의 출처)이고, 집기를
    /// main 에 커밋하는 지금 규약에서는 양쪽 줄이 같아 거의 안 선다 — 목록의 ⎇ 가 사라진 까닭이다.
    pub fn working(&self, id: &str) -> Option<&str> {
        self.trees.iter().find(|(.., holds)| holds.contains(id)).map(|(label, ..)| label.as_str())
    }

    /// 줄을 보태 온 옆 워크트리의 뿌리들 — [`Origin::root`] 가 댈 수 있는 자리 전부. 탐색기가
    /// 이 뿌리마다 커밋 표를 짓는다(moai-a4i0). 줄을 하나도 안 보탠 곳은 뺀다 — 찾을 id 가 없다.
    pub fn roots(&self) -> Vec<&Path> {
        let used: BTreeSet<usize> = self.from.values().copied().collect();
        used.into_iter().map(|k| self.trees[k].1.as_path()).collect()
    }

    /// 겹쳐 본 워크트리의 이름들 — 줄을 하나도 안 보탠 곳까지. 겹쳐 봤는데 옆이
    /// 조용한 것과 아예 안 겹쳐 본 것을 화면이 가를 수 있어야 한다.
    pub fn labels(&self) -> Vec<&str> {
        self.trees.iter().map(|(l, ..)| l.as_str()).collect()
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
    for (tree, Side { label, root, issues, base, holds }) in others.into_iter().enumerate() {
        origin.trees.push((label, root, holds));
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
    /// 프로젝트를 열 때마다 시키지 않은 배너를 세우게 된다(moai-zcuh). 탐색기는 사람이
    /// `SPC t w` 로 켰을 때만 알림으로 댄다(`tui::App::unfound`, moai-d5vn).
    pub unfound: Option<String>,
    /// 읽으러 간 옆 스냅샷마다 **읽기 전에** 잰 표식. 탐색기가 바뀐 것을 알아채는 데
    /// 쓴다. 파일이 없던 곳도 든다 — 거기 스냅샷이 생기는 것도 바뀐 것이다.
    ///
    /// **재는 것이 읽는 것보다 먼저다** — 읽고 나서 재면 그 사이에 떨어진 쓰기가
    /// "이미 본 것" 으로 적혀 영영 안 보인다(`cmd::tui::run` 과 같은 까닭).
    ///
    /// 워크트리들의 HEAD 가 움직인 것을 알리는 git 파일도 든다([`heads`]) — 갈라진 자리가
    /// 바뀌면 지운 줄 숨김의 답이 바뀐다(moai-pqrq).
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
    // HEAD 가 움직인 것도 다시 읽을 까닭이다 — **`others_of` 가 HEAD 를 읽기 전에** 잰다.
    let mut watched = heads(&repo.root);
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
                        others.push(side(&repo.root, &here, mine.as_deref(), tree, root, other.issues));
                    }
                }
            }
        }
    }
    let Load { issues, errors } = load;
    let (issues, origin) = overlay(issues, others);
    Ok(Gathered { load: Load { issues, errors }, origin, trouble, unfound, watched })
}

/// 옆 워크트리 하나의 줄을 겹칠 모양으로 — 옆에만 있는 줄이 있으면 갈라진 자리([`Side::base`])를 댄다.
///
/// 갈라진 자리는 옆에만 있는 줄을 가를 때만 쓴다. 다 여기에도 있으면 git 을 두 번 더 부르지
/// 않는다 — 탐색기는 다시 읽을 때마다 여기를 지난다.
fn side(
    repo_root: &Path,
    here: &std::collections::HashSet<&str>,
    mine: Option<&str>,
    tree: Tree,
    root: PathBuf,
    issues: Vec<Issue>,
) -> Side {
    let lonely = issues.iter().any(|i| !here.contains(i.id.as_str()));
    let base = match mine {
        Some(m) if lonely => base_of(repo_root, m, &tree.head),
        _ => BTreeMap::new(),
    };
    // 이름 후보는 훅과 같은 자로 낸다 — 디렉터리 이름까지 여기서 안다.
    let holds = names([&tree]);
    Side { base, holds, ..Side::new(tree.label, root, issues) }
}

/// 제 줄을 **옆 워크트리의 스냅샷과 겹친 것**과, 옆 워크트리의 이름이 가리키는 id 후보([`away`]
/// 와 같은 자) — 훅이 막기 전에 한 번 더 비춰 보는 자리다(moai-w2iy).
///
/// 트래커는 main 에서 만지는 것이 규약이라(CLAUDE.md "워크트리"), 워크트리의 스냅샷(HEAD)은
/// main 에서 방금 세우고 집은 줄을 모른다. 그 낡은 스냅샷만 보고 막으면 시킨 대로 한 일이
/// 막힌다. 겹치는 규칙은 `--worktree` 와 같다([`overlay`]) — 훅만의 셈을 따로 두지 않는다.
///
/// **git 목록은 한 번만 읽는다** — 겹칠 줄과 이름 후보가 한 목록에서 나온다. 못 찾으면 `None`
/// 이고, 남의 못 읽는 줄은 말없이 빼고 겹친다 — 훅은 무엇이 어긋나도 조용해야 한다.
pub fn fresh(repo: &Repo, mine: Vec<Issue>) -> Option<(Vec<Issue>, BTreeSet<String>)> {
    let (head, trees) = others_of(&repo.root).ok()?;
    let away = names(trees.iter().map(|(t, _)| t));
    let mut others = Vec::new();
    {
        let here: std::collections::HashSet<&str> = mine.iter().map(|i| i.id.as_str()).collect();
        for (tree, root) in trees {
            let Ok(Some(other)) = crate::store::read_snapshot(&root.join(".moai").join("issues.jsonl")) else {
                continue;
            };
            others.push(side(&repo.root, &here, head.as_deref(), tree, root, other.issues));
        }
    }
    Some((overlay(mine, others).0, away))
}

/// 워크트리들의 HEAD 가 움직인 것을 알아챌 git 파일과 **지금 잰** 표식 — 제 워크트리와
/// 옆 워크트리 모두.
///
/// 갈라진 자리(`merge-base`)는 두 HEAD 에서 나오므로, 어느 쪽이든 커밋·merge·checkout 하면
/// 지운 줄 숨김([`Side::base`])의 답이 바뀐다. 스냅샷 파일만 지켜보면 탐색기는 다른 까닭으로
/// 다시 읽을 때까지 낡은 답을 든다(moai-pqrq).
///
/// **git 은 한 번만 부른다**(공용 git 디렉터리를 찾는 데) — 탐색기는 다시 읽을 때마다 여기를
/// 지난다. 나머지는 파일을 직접 본다: 공용 디렉터리의 `HEAD`(주 워크트리), `worktrees/*/HEAD`
/// (딸린 워크트리), 그 HEAD 가 가리키는 가지 파일, `packed-refs`. 커밋·merge 는 가지 파일을,
/// checkout·떼어 낸 HEAD 의 커밋은 HEAD 파일을 갈아끼우고, `pack-refs` 는 가지 파일을 지우고
/// `packed-refs` 를 쓴다.
///
/// **HEAD 파일을 잰 뒤에 그 안을 읽어** 가지를 찾는다 — 그 사이에 HEAD 가 바뀌면 HEAD 표식이
/// 이미 달라 다음 걸음이 다시 읽는다. 못 찾으면(git 밖, 가지를 파일로 두지 않는 저장소) 비어
/// 있고 말하지 않는다 — 전처럼 스냅샷만 지켜본다.
///
/// 겹쳐 보지 않는 탐색기도 부른다 — 커밋 칸의 표(moai-a4i0)는 HEAD 가 움직여야 낡는다.
pub fn heads(root: &Path) -> Vec<(PathBuf, crate::store::Stamp)> {
    // `--path-format=absolute` 는 git 2.31 부터고, 그 전 rev-parse 는 모르는 플래그를 **출력에
    // 그대로 되뱉고 성공한다** — 경로가 두 줄이 되어 표식이 영영 안 바뀐다. 상대 경로는 `-C`
    // 로 준 디렉터리에서 푼 것이라 `root` 에 붙인다(절대 경로면 `join` 이 그대로 둔다).
    let Ok(common) = git(root, &["rev-parse", "--git-common-dir"]) else {
        return Vec::new();
    };
    let common = root.join(common.trim_end_matches('\n'));
    // 워크트리가 생기거나 없어지면 이 디렉터리의 수정 시각이 바뀐다 — git 을 안 띄우고도
    // 새 워크트리를 다시 읽을 까닭으로 센다.
    let worktrees = common.join("worktrees");
    let mut files = vec![common.join("HEAD")];
    // 목록을 읽기 **전에** 잰다 — 읽고 나서 재면 그 사이에 생긴 워크트리를 놓친다.
    let mut watched = vec![(worktrees.clone(), crate::store::stamp(&worktrees))];
    if let Ok(linked) = std::fs::read_dir(&worktrees) {
        let mut linked: Vec<PathBuf> = linked.filter_map(Result::ok).map(|e| e.path().join("HEAD")).collect();
        linked.sort();
        files.extend(linked);
    }
    for head in files {
        watched.push((head.clone(), crate::store::stamp(&head)));
        let Ok(text) = std::fs::read_to_string(&head) else { continue };
        if let Some(branch) = text.trim_end().strip_prefix("ref: ") {
            let file = common.join(branch);
            watched.push((file.clone(), crate::store::stamp(&file)));
        }
    }
    let packed = common.join("packed-refs");
    watched.push((packed.clone(), crate::store::stamp(&packed)));
    watched
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

/// 옆 워크트리들의 **이름이 가리키는 id 후보** — 훅이 초점에서 뺄 것(`hook::held`).
///
/// **못 찾으면 비어 있다.** git 이 없거나 저장소가 아니면 옆도 없는 것이고, 그러면
/// 전처럼 스냅샷의 집은 줄이 다 제 초점이다. 훅은 무엇이 어긋나도 조용해야 한다.
pub fn away(root: &Path) -> BTreeSet<String> {
    on_disk(root).map(|d| names(d.others())).unwrap_or_default()
}

/// git 이 적어 둔 파일에서 읽은 워크트리 목록([`on_disk`]).
struct Disk {
    /// 워크트리 꼭대기에서 moai 뿌리까지 — 같은 저장소의 워크트리는 같은 나무 모양이다([`others_of`]).
    rel: PathBuf,
    /// (워크트리, 딸린 워크트리인가, 제 워크트리인가).
    all: Vec<(Tree, bool, bool)>,
    /// 딸린 워크트리의 꼭대기 → git 이 그 워크트리를 적어 둔 디렉터리(`worktrees/<이름>`).
    ///
    /// **뜬 때는 여기서 재지 않는다** — [`on_disk`] 는 훅이 도구 호출마다 지나는 길이고
    /// ([`away`]), 뜬 때를 읽는 것은 [`workplaces`] 뿐이다. 여기서 `stat` 을 걸면 훅이 쓰지도
    /// 않을 값을 워크트리 수만큼 읽는다 — moai-n2jh 가 이 길에서 git 두 번을 걷어낸 자리다.
    admin: BTreeMap<PathBuf, PathBuf>,
}

impl Disk {
    fn others(&self) -> impl Iterator<Item = &Tree> {
        self.all.iter().filter(|(_, _, me)| !me).map(|(t, ..)| t)
    }
}

/// 옆 **딸린** 워크트리가 쥐었을 수 있는 줄 id 와, 제 워크트리의 이름 후보 (moai-ntl6, 사용자 결정 B).
///
/// 이름이 id 가 아닌 워크트리(에이전트 격리 `worktree-agent-<해시>`, 옛 id 로 뜬 워크트리)가
/// 쥔 일은 이름으로 못 가른다. 대신 **갈라진 자리**로 짐작한다 — 규약상 집기는 main 에서 커밋한
/// 뒤 워크트리가 뜨므로, 그 워크트리의 스냅샷 파일에 벌여 놓인 줄은 갈라질 때 이미 집혀 있던
/// 일이다. 그 워크트리에서 main 보다 늦게 옮긴 줄(`planned`·`updated_at`)도 든다. 갈라진 **뒤에**
/// main 에서 집은 일은 그 파일에 없어 들지 않는다.
///
/// **main 워크트리는 쥔 곳으로 안 센다** — 모두의 집기가 모이는 자리라, 세면 모든 줄이 든다.
/// **답은 짐작이다** — 받는 쪽은 이 줄로 막거나 붙들지 않기만 한다(`hook::unsure`). git 을 띄우지
/// 않고 파일만 읽지만 옆 스냅샷을 다 풀어 싸지 않다 — 거절 길과 `Stop` 에서만 부른다.
pub fn held_elsewhere(root: &Path, mine: &[Issue], cfg: &crate::config::Config) -> (BTreeSet<String>, BTreeSet<String>) {
    let Some(disk) = on_disk(root) else { return Default::default() };
    let own = names(disk.all.iter().filter(|(_, _, me)| *me).map(|(t, ..)| t));
    let mut out = BTreeSet::new();
    for (tree, linked, me) in &disk.all {
        if *me || !*linked {
            continue;
        }
        let (open, later) = holds(&disk, tree, mine, cfg).unwrap_or_default();
        out.extend(open);
        out.extend(later);
    }
    (out, own)
}

/// 딸린 워크트리 하나의 스냅샷이 쥔 줄 — [`held_elsewhere`] 의 한 워크트리 몫. (벌여 놓인 줄,
/// 여기보다 늦게 만진 줄). 앞의 것에는 갈라질 때 물려받은 줄도 들고, 뒤의 것은 그 워크트리가
/// 실제로 만진 흔적이다 — [`workplaces`] 가 둘을 따로 싣는다.
///
/// **열려다 못 열면 `None` 이다**(moai-lt7h) — 빈 답과 가른다. 권한이 없거나 파일 자리에 엉뚱한
/// 것이 선 워크트리를 "아무도 거기서 일 안 한다" 로 읽으면, 거기서 도는 일이 통째로 자리를 잃는다.
///
/// **읽히는데 안 풀리는 줄은 여기 안 든다**(리뷰 moai-ya06). `store::read_snapshot` 은 열리는
/// 파일을 늘 `Ok` 로 내고 못 푸는 줄은 `Load::errors` 에 담으므로, 머지 충돌 표시가 박힌
/// `issues.jsonl` 도 여기서는 멀쩡히 읽힌다 — 충돌 난 줄에 있던 일만 조용히 빠져 `Lost` 로 선다.
/// 그 사실을 아는 것은 같은 파일을 읽는 [`gather`] 뿐이다(`읽을 수 없는 줄 N개는 빼고 겹쳤다`).
/// 여기서 `errors` 를 모름으로 세지 않는 것은 **낡은 줄 하나가 저장소의 `stranded` 를 통째로
/// 재우기 때문**이다(아래 문단과 같은 까닭) — 대신 그 갈래를 좁히는 것이 남은 일이다.
///
/// **스냅샷이 아예 없는 것은 모르는 것이 아니다.** `moai init` 전에 갈라졌거나 이 도구와 아무
/// 상관 없는 가지를 띄운 워크트리다 — 거기에 적힐 수 있는 줄이 없으니 "아무것도 안 쥐었다" 가
/// 사실이고, 이것을 모름으로 세면 그런 워크트리 하나가 저장소 전체의 `stranded` 를 영영 재운다
/// ([`crate::report::places`] 의 `blind`). 스냅샷 없는 워크트리를 두고 말하지 않는 것은 이미 선
/// 규약이다(`worktree_writes_nothing_and_skips_trees_without_a_snapshot`).
fn holds(disk: &Disk, tree: &Tree, mine: &[Issue], cfg: &crate::config::Config) -> Option<(BTreeSet<String>, BTreeSet<String>)> {
    let path = tree.path.join(&disk.rel).join(".moai").join("issues.jsonl");
    let side = match crate::store::read_snapshot(&path) {
        Ok(Some(side)) => side,
        Ok(None) => return Some(Default::default()),
        Err(_) => return None,
    };
    let by_id: BTreeMap<&str, &Issue> = mine.iter().map(|i| (i.id.as_str(), i)).collect();
    let open = crate::report::wip(&side.issues, cfg).into_iter().map(|i| i.id.clone()).collect();
    let later = side
        .issues
        .iter()
        .filter(|i| {
            // 여기에 없는 줄은 그 워크트리에서 세운 것이다 — 그것도 만진 흔적이다.
            by_id
                .get(i.id.as_str())
                .is_none_or(|m| (i.planned(), i.updated_at.as_str()) > (m.planned(), m.updated_at.as_str()))
        })
        .map(|i| i.id.clone())
        .collect();
    Some((open, later))
}

/// 살아 있는 **딸린** 워크트리마다 자리 하나(moai-ir8q) — 판정은 `report::places`·`report::stranded`.
///
/// main 워크트리는 안 든다 — 모두의 집기가 모이는 자리라 거기 선 줄은 "어디서 하는가" 에 답이
/// 안 된다. 파일만 읽는다. 저장소가 아니면 비어 있다.
///
/// **딸린 워크트리 안에서는 `worktree` 일 때만 잰다.** 그 스냅샷은 갈라질 때의 main 이라, 그 뒤
/// main 에서 끝내거나 놓은 줄이 거기서는 아직 집혀 있다 — 그것으로 재면 끝난 일을 "자리 없다" 로
/// 대고, 감독이 그 말대로 남에게 다시 준다. 집기가 적히는 곳은 main 이고 `--worktree` 가 그것을
/// 겹친다. **이 판단은 여기 한 곳에만 둔다** — 부르는 명령마다 두었더니 `status` 에만 걸리고
/// `show` 에는 안 걸려 감독 안내의 두 줄이 서로 다른 답을 냈다(moai-6opu.p65).
///
/// **"늦게 만진 줄" 은 main 의 스냅샷 파일에 대어 잰다** — 부르는 쪽의 파일도, 부르는 쪽이 든
/// 줄도 아니다. 집기가 적히는 곳이 main 하나라 모두가 견줄 바닥도 거기 하나고, 딸린 워크트리에서
/// 제 파일에 대면 그 파일은 갈라질 때의 main 이라 **그 뒤 main 이 옮긴 줄이 전부 "옆이 만졌다"**
/// 로 서서 한 줄이 워크트리 여럿에 동시에 선다. main 에서 부르면 같은 파일이라 답이 안 바뀐다.
/// 못 읽으면 옆의 줄이 다 만진 흔적이 된다 — 자리를 넉넉히 대는 쪽으로 틀린다.
///
/// **옆 스냅샷은 셀 일이 있을 때만 판다**(moai-7igy, 사용자 결정). 집은 줄이 하나도 없으면 아예
/// 안 열고, 있으면 **이름으로 먼저 가른 뒤** 그래도 자리를 못 찾은 줄이 남을 때만 연다. `moai
/// status` 는 세션마다 도는데 이 저장소에서 워크트리 일곱이면 스냅샷 여덟 벌을 다시 파
/// 40→95ms(따뜻)·138→458ms(참)이었다. 이름이 답을 내는 흔한 경우에는 한 벌도 안 판다.
///
/// **치르는 값이 둘 있다**(리뷰 moai-ya06) — 적어 두고 고르는 것이지 공짜가 아니다.
/// 1. 문은 **저장소 하나로** 여닫힌다. 자리를 못 찾은 줄이 하나라도 있으면 그 한 줄 때문에 옆
///    스냅샷을 전부 판다. 규약이 집기를 커밋한 뒤 워크트리를 띄우므로 갓 집은 줄은 잠깐
///    `Fresh` 고, 자리를 잃은 줄은 고칠 때까지 `Lost` 다 — 빠른 길이 꺼져 있는 것이 드물지 않다.
/// 2. 이름이 답을 낸 줄은 **스냅샷으로만 보이는 둘째 자리를 못 본다.** 이름이 가리키는
///    워크트리가 하나 있으면 거기서 멈추므로, 같은 줄을 실제로 만지고 있는 이름 없는 워크트리
///    (에이전트 격리)가 `moai show <id>` 의 `자리` 줄에서 빠진다. 그 줄을 읽고 들어가는 것이
///    이어받는 세션이라 값이 0 은 아니다. 훅은 이 길을 지나지 않는다(`hook` 은 `workplaces` 도
///    `places` 도 안 부른다) — 무는 것은 사람과 감독이 부르는 `status`·`show` 다.
///
/// **팔 까닭은 부르는 쪽의 줄로 잰다**(`asked`) — `touched` 의 기준인 `mine`(main 의 스냅샷,
/// moai-40ht.hom) 으로 재면 안 된다. `--worktree` 로 겹쳐 본 쪽은 옆 워크트리에서 만들고 집은
/// 줄까지 `places`·`stranded` 에 거는데, main 의 스냅샷에는 그 줄이 없어 "이름으로 다 잡혔다" 로
/// 읽힌다 — 그러면 `holds` 가 빈 채로 나가 그 줄이 통째로 `Lost` 로 서고, 살아 있는 세션의 일이
/// `stranded` 경고와 `show` 의 `자리 없다` 로 뒤집힌다.
pub fn workplaces(
    root: &Path,
    cfg: &crate::config::Config,
    worktree: bool,
    asked: &[Issue],
) -> Vec<crate::report::Workplace> {
    if !worktree && is_linked(root) {
        return Vec::new();
    }
    let Some(disk) = on_disk(root) else { return Vec::new() };
    let bare = |tree: &Tree| crate::report::Workplace {
        path: tree.path.clone(),
        branch: tree.label.clone(),
        names: names([tree]),
        holds: BTreeSet::new(),
        touched: BTreeSet::new(),
        born: None,
        unknown: false,
    };
    // **거를 자를 한 번만 적는다** — 자리와 그 워크트리를 아래에서 `zip` 으로 맞추므로, 거르는
    // 줄이 둘이면 한쪽만 고쳐졌을 때 자리가 남의 워크트리의 스냅샷을 받아 든다.
    let linked: Vec<&Tree> = disk.all.iter().filter(|(_, linked, _)| *linked).map(|(tree, ..)| tree).collect();
    // 딸린 워크트리가 하나도 없으면 여기서 끝이다 — 아래의 문도, main 의 스냅샷도 볼 까닭이 없다
    // (`report::stranded` 도 빈 목록에는 조용하다). 워크트리 규약을 안 쓰는 저장소의 흔한 길이다.
    if linked.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<crate::report::Workplace> = linked.iter().map(|tree| bare(tree)).collect();
    // 이름만으로 자리가 다 잡히면(또는 집은 줄이 없으면) 스냅샷을 한 벌도 안 판다.
    //
    // **묻는 것은 하나다 — 이름이 집은 줄을 다 가리키는가.** `holds`·`touched` 가 아직 비었으니
    // `report::places` 의 판정도 여기서는 그 하나로 접히는데, 그렇다고 판정을 통째로 돌려 한
    // 낱말만 건지면 (1) 시계를 한 번 더 읽고 (2) 소속 지도와 굴림까지 지어 버리고 (3) 판정에
    // 변형이 하나 늘 때마다 **디스크를 언제 만지는가**가 조용히 따라 바뀐다. 그래서 그 하나를
    // 재는 자(`report::claimed` — `hook::held` 와 `places` 가 이미 같이 쓴다)를 바로 쓴다.
    let all_names = names(linked.iter().copied());
    let named = crate::report::claimed(asked, &all_names);
    if crate::report::wip(asked, cfg).iter().all(|i| named(i)) {
        return out;
    }
    // 여기서부터가 파는 길이다 — **뜬 때도 여기서 읽는다.** `born` 은 이름 없는 워크트리의
    // `holds` 를 가릴 때만 보는 값이라(`report::places`), 안 파는 길에서는 워크트리마다
    // `logs/HEAD` 를 한 번씩 읽고 버리는 헛일이었다(`Disk::admin` 이 같은 까닭으로 `on_disk`
    // 에서 이것을 뺐다).
    let base = disk
        .all
        .iter()
        .find(|(_, linked, _)| !*linked)
        .map(|(t, ..)| t.path.join(&disk.rel))
        .unwrap_or_else(|| root.to_path_buf());
    let own = crate::store::read_snapshot(&base.join(".moai").join("issues.jsonl"));
    let mine: &[Issue] = match &own {
        Ok(Some(load)) => &load.issues,
        _ => &[],
    };
    for (place, tree) in out.iter_mut().zip(&linked) {
        place.born = disk.admin.get(&tree.path).and_then(|dir| born_of(dir));
        // **제 워크트리도 남과 같은 자로 잰다.** 한때 비워 두었더니, 이름이 id 가 아닌
        // 워크트리(에이전트 격리)가 제가 하고 있는 일을 제 화면에서 "자리 없다" 로 댔다.
        match holds(&disk, tree, mine, cfg) {
            Some((holds, touched)) => {
                place.holds = holds;
                place.touched = touched;
            }
            None => place.unknown = true,
        }
    }
    out
}

/// 그 워크트리가 뜬 때(RFC3339) — **git 이 파일 안에 적어 둔 시각**이다. `worktrees/<이름>/logs/HEAD`
/// 첫 줄이 `worktree add` 가 HEAD 를 처음 세운 기록이고, 거기 사람 뒤에 epoch 초가 있다.
///
/// **파일의 고친 때로 재지 않는다**(리뷰 moai-40ht.hom, 사용자 결정) — 시각을 안 지키는 복사
/// (`cp -r`·`rsync` 의 `--times` 없는 판·Docker `COPY`·백업 복원)가 그것을 통째로 새로 해,
/// `.moai` 는 하나도 안 바뀌었는데 저장소의 집은 줄이 전부 "자리 없다" 로 뒤집힌다.
///
/// 못 읽으면(`core.logAllRefUpdates=false`) 없고, 그러면 부르는 쪽이 `holds` 를 다 믿는다
/// ([`crate::report::places`]) — 산 일을 남에게 다시 주는 쪽이 더 비싸다.
fn born_of(dir: &Path) -> Option<String> {
    let log = std::fs::read_to_string(dir.join("logs").join("HEAD")).ok()?;
    // `<옛> <새> <이름> <메일> <epoch> <시간대>\t<무엇>` — 메일은 `>` 로 닫힌다.
    let head = log.lines().next()?;
    let at = head.split_once('>')?.1;
    let secs: i64 = at.split_whitespace().next()?.parse().ok()?;
    Some(crate::model::format_rfc3339(secs))
}

/// 이 트래커가 든 **워크트리의 꼭대기** — [`workplaces`] 의 경로를 여기서 잰다. git 을 띄우지
/// 않는다. 저장소가 아니면 없다.
///
/// 뿌리(`.moai` 가 든 디렉터리)로 재면 안 된다 — 모노레포처럼 `.moai` 가 아래에 있으면 워크트리
/// 경로가 그 밑에 없어 하나도 안 잘리고, 기계의 절대 경로가 `--json` 으로 그대로 나간다.
pub fn top_of(root: &Path) -> Option<PathBuf> {
    git_dirs(root).map(|(top, _)| canonical(top))
}

/// 제 워크트리가 아닌 워크트리들을 **git 을 띄우지 않고** 읽는다 — 이름 후보([`away`])만 쓴다.
///
/// 훅은 도구 호출마다 이름 후보를 읽는다. `git rev-parse` 와 `git worktree list` 두 번이 호출당
/// 값의 약 40%(9ms/22ms)였다(moai-n2jh). 이름에는 경로와 가지만 들면 되고, 그 둘은 git 이 적어
/// 두는 파일에 그대로 있다 — `.git`(주 워크트리면 디렉터리, 딸린 워크트리면 `gitdir:` 한 줄),
/// 공용 디렉터리의 `commondir`·`HEAD`, `worktrees/<이름>/gitdir`·`HEAD`. [`heads`] 도 같은 파일을 본다.
///
/// **목록이 틀려도 싸다** — 후보일 뿐이라 id 와 정확히 같은 이름만 뺀다. 경로가 사라진 워크트리
/// (`prunable`)는 뺀다. 맨몸 저장소는 공용 디렉터리 이름이 `.git` 이 아니라 주 워크트리가 없다.
/// 겹쳐 보기([`gather`]·[`fresh`])는 HEAD 커밋이 필요해 여전히 git 으로 읽는다.
fn on_disk(root: &Path) -> Option<Disk> {
    let (top, common) = git_dirs(root)?;
    let label = |head: &Path| {
        let text = std::fs::read_to_string(head).ok()?;
        let text = text.trim_end();
        Some(match text.strip_prefix("ref: ") {
            Some(r) => r.strip_prefix("refs/heads/").unwrap_or(r).to_string(),
            None => text.chars().take(7).collect(),
        })
    };
    let mut all = Vec::new();
    let mut admin = BTreeMap::new();
    if common.file_name().is_some_and(|n| n == ".git")
        && let (Some(path), Some(label)) = (common.parent(), label(&common.join("HEAD")))
    {
        all.push((Tree { path: path.to_path_buf(), label, head: String::new() }, false));
    }
    if let Ok(linked) = std::fs::read_dir(common.join("worktrees")) {
        // **차례를 고정한다** — `read_dir` 의 차례는 파일 시스템이 정하는 것이라, 그대로 두면
        // `moai show --json` 의 `workplaces` 와 사람 화면의 `자리` 줄이 부를 때마다 뒤바뀐다.
        let mut dirs: Vec<PathBuf> = linked.filter_map(|e| e.ok()).map(|e| e.path()).collect();
        dirs.sort();
        for dir in dirs {
            // `worktree.useRelativePaths` 면 이 경로는 이 디렉터리에서 푼 상대 경로다 — 프로세스의
            // 자리로 풀면 멀쩡한 워크트리가 "사라졌다" 로 빠진다(`join` 은 절대 경로면 그대로 둔다).
            let Some(path) = std::fs::read_to_string(dir.join("gitdir"))
                .ok()
                .and_then(|g| canonical(&dir.join(g.trim_end())).parent().map(Path::to_path_buf))
                .filter(|p| p.exists())
            else {
                continue;
            };
            let Some(label) = label(&dir.join("HEAD")) else { continue };
            admin.insert(path.clone(), dir);
            all.push((Tree { path, label, head: String::new() }, true));
        }
    }
    let top = canonical(top);
    let rel = canonical(root).strip_prefix(&top).map(Path::to_path_buf).unwrap_or_default();
    let all = all
        .into_iter()
        .map(|(t, linked)| {
            let me = canonical(&t.path) == top;
            (t, linked, me)
        })
        .collect();
    Some(Disk { rel, all, admin })
}

/// 이 자리가 **딸린 워크트리 안인가** — 가장 가까운 `.git` 이 디렉터리가 아니라 `gitdir:` 파일이다.
/// git 을 띄우지 않는다. git 밖이면 아니다.
///
/// 훅이 `-C`·`cd` 로 가리킨 트래커를 누구의 눈으로 볼지 가른다(moai-23ky). 딸린 워크트리는 그
/// 이름이 곧 거기서 하는 일이라 그 워크트리의 눈으로 보고, 모두의 집기가 모이는 main 은 세션의
/// 눈으로 본다.
pub fn is_linked(root: &Path) -> bool {
    root.ancestors().map(|d| d.join(".git")).find(|g| g.exists()).is_some_and(|g| g.is_file())
}

/// 워크트리 꼭대기와 공용 git 디렉터리 — git 이 적어 둔 파일로만 읽는다([`on_disk`]).
///
/// 주 워크트리면 공용 디렉터리는 `.git` 그대로(풀지 않는다 — 끝 이름으로 주 워크트리를 알아본다),
/// 딸린 워크트리면 `gitdir:` 가 가리킨 곳의 `commondir` 를 푼 것이다.
fn git_dirs(root: &Path) -> Option<(&Path, PathBuf)> {
    let top = root.ancestors().find(|d| d.join(".git").exists())?;
    let dotgit = top.join(".git");
    let common = if dotgit.is_dir() {
        dotgit
    } else {
        let text = std::fs::read_to_string(&dotgit).ok()?;
        let gitdir = top.join(text.trim_end().strip_prefix("gitdir:")?.trim());
        let up = std::fs::read_to_string(gitdir.join("commondir")).ok()?;
        // `commondir` 는 대개 `../..` 다 — 풀지 않으면 끝 이름이 `..` 라 주 워크트리를 못 알아본다.
        canonical(&gitdir.join(up.trim_end()))
    };
    Some((top, common))
}

/// 두 뿌리가 **같은 git 저장소의 워크트리에서 같은 자리의 트래커인가** — 공용 git 디렉터리가
/// 같고, 워크트리 꼭대기에서 moai 뿌리까지가 같다. 못 찾으면 아니다.
///
/// 훅이 `-C`·`cd` 로 가리킨 트래커가 세션의 옆 워크트리인지 가른다(moai-23ky). 옆 워크트리면
/// 세션의 자리로 본다 — 그쪽 눈으로 보면 이 세션이 쥔 일이 "옆의 것" 이라 초점에서 빠져,
/// `moai -C <main> add` 한 번으로 규칙 1 을 넘는다. 다른 트래커를 가리킬 때만 부른다.
///
/// **꼭대기에서의 자리도 견준다.** 공용 디렉터리만 보던 판은 한 저장소에 트래커를 둘 둔
/// 모노레포(`a/.moai`·`b/.moai`)에서 `moai -C ../b add` 를 `a` 의 초점으로 막고 `b` 의 초점은
/// 안 봤다. git 을 띄우지 않는다 — 두 번의 `rev-parse` 가 이 길의 값 대부분이었다.
pub fn same_repo(a: &Path, b: &Path) -> bool {
    let place = |root: &Path| {
        let (top, common) = git_dirs(root)?;
        let rel = canonical(root).strip_prefix(canonical(top)).map(Path::to_path_buf).ok()?;
        Some((canonical(&common), rel))
    };
    matches!((place(a), place(b)), (Some(x), Some(y)) if x == y)
}

/// 워크트리 이름에서 id 후보를 읽는다 — 경로의 끝 이름, 가지 이름, `worktree-` 를 뗀 가지 이름.
///
/// 규약(CLAUDE.md "워크트리")이 `.claude/worktrees/<id>` 에 `worktree-<id>` 가지로 뜬다.
/// **집은 곳을 스냅샷에 적지 않는다.** 집기는 main 에서 커밋하니 적히는 곳은 늘 main 이고,
/// 워크트리를 치운 뒤에도 그 줄은 남는다 — 지금 살아 있는 워크트리에서 읽는 파생값이다.
/// 후보일 뿐이라 id 인지는 받는 쪽이 줄과 견준다.
pub fn names<'a>(trees: impl IntoIterator<Item = &'a Tree>) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for t in trees {
        if let Some(name) = t.path.file_name() {
            out.insert(name.to_string_lossy().into_owned());
        }
        from_label(&t.label, &mut out);
    }
    out
}

/// 가지 이름 하나가 가리키는 id 후보 — 이름 그대로와 `worktree-` 를 뗀 것. **[`names`] 와
/// [`Side::new`] 가 같이 쓴다**(moai-6bc0 단계 리뷰): 자가 둘이면 화면이 ⎇ 를 다는 규칙과 훅이
/// 옆의 일을 가르는 규칙이 언젠가 조용히 갈라진다.
fn from_label(label: &str, out: &mut BTreeSet<String>) {
    out.insert(label.strip_prefix("worktree-").unwrap_or(label).to_string());
    out.insert(label.to_string());
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
    use crate::git::Error;
    crate::git::run(root, args).map_err(|e| match e {
        Error::Spawn(e) => format!("git 을 부르지 못해 워크트리를 못 찾았다 — {e}"),
        Error::Failed(err) => format!("워크트리를 못 찾았다 — {err}"),
        Error::Stream(e) => format!("워크트리 목록을 읽다 끊겼다 — {e}"),
        Error::NotUtf8(e) => format!("워크트리 목록을 못 읽었다 — {e}"),
    })
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

    /// 규약대로 뜬 워크트리는 디렉터리와 가지 둘 다로 id 를 댄다. 규약 밖의 이름도
    /// 후보로는 선다 — id 인지는 훅이 줄과 견준다.
    #[test]
    fn a_worktree_names_its_work_by_directory_and_branch() {
        let src = "worktree /repo/.claude/worktrees/moai-ab12\0HEAD 1111111aaaa\0branch refs/heads/worktree-moai-ab12\0\0\
                   worktree /elsewhere/x\0HEAD 2222222bbbb\0branch refs/heads/moai-cd34\0\0";
        let got = names(&parse(src));
        for want in ["moai-ab12", "moai-cd34", "x"] {
            assert!(got.contains(want), "{want} 가 없다 — {got:?}");
        }
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

    /// **그 이슈를 쥔 옆 워크트리를 찾는다**(moai-nxt4) — 줄이 어디서 왔는지와 따로다. 집기를 main 에
    /// 커밋하면 양쪽 줄이 같아 출처는 안 서는데, 옆에서 그 일을 쥐고 있다는 것은 여전히 보여야 한다.
    /// **훅과 같은 자**([`names`])로 가른다 — 후보가 통째로 같아야 쥔 것이다(moai-nxt4 리뷰).
    #[test]
    fn a_sibling_worktree_holding_the_issue_is_found() {
        let (_, origin) = overlay(vec![issue("moai-3fnf", "in_progress", "2026-09-15T00:00:00Z")], vec![
            tree("worktree-moai-3fnf", vec![]),
            tree("feat/moai-9xyz-따로", vec![]),
        ]);
        assert_eq!(origin.working("moai-3fnf"), Some("worktree-moai-3fnf"), "`worktree-` 를 뗀 이름이 안 걸렸다");
        assert_eq!(origin.branch("moai-3fnf"), None, "제 줄인데 출처가 붙었다");
        // 이름 **통째로** 같아야 한다 — 훅(`away`·`hook::held`)과 같은 자다.
        assert_eq!(origin.working("moai-9xyz"), None, "id 가 이름의 한 토막일 뿐인데 쥐었다고 했다");
        assert_eq!(origin.working("moai-3fn"), None, "id 의 앞토막이 남의 가지에 걸렸다");
        // 자식의 워크트리는 부모 줄에 안 붙는다 — 부모가 제가 집힌 줄 안다.
        let (_, child) = overlay(vec![], vec![tree("worktree-moai-3fnf.abc", vec![])]);
        assert_eq!(child.working("moai-3fnf"), None);
        assert_eq!(child.working("moai-3fnf.abc"), Some("worktree-moai-3fnf.abc"));
        // 가지 이름 그대로도 후보다 — `-b` 없이 띄운 워크트리.
        let (_, plain) = overlay(vec![], vec![tree("moai-3fnf", vec![])]);
        assert_eq!(plain.working("moai-3fnf"), Some("moai-3fnf"));
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

    /// **임시 자리가 어느 체크아웃 안이어도 그 저장소의 워크트리가 새지 않는다**(moai-46xz).
    ///
    /// 파일로 읽는 반쪽은 `Path::ancestors()` 로 `.git` 을 찾아 올라간다. 컨테이너나 CI 에서
    /// `TMPDIR` 이 체크아웃 밑이면 시험의 임시 자리 위에 그 체크아웃이 서고, 만들지도 않은
    /// 워크트리 이름이 답에 섞인다 — 환경 변수를 걷어서는 못 막는 길이다. 그래서 임시 자리에
    /// 빈 `.git` 울타리를 세운다([`Scratch::fenced`]).
    ///
    /// 여기서는 그 상황을 **이 저장소 안에 자리를 잡아** 그대로 흉내 낸다. 이 체크아웃은
    /// 진짜 저장소라, 울타리가 없으면 그 꼭대기가 그대로 잡힌다.
    ///
    /// **먼저 울타리 없는 자리로 견준다.** `away` 만 보면 워크트리가 하나도 안 딸린 새
    /// 클론에서는 울타리를 걷어내도 양쪽이 똑같이 비어, 이 시험이 아무것도 안 본 채
    /// 초록으로 끝난다. 훑기가 어디서 멈추는가(`top_of`)가 울타리가 지는 값이다.
    #[test]
    fn a_temp_place_inside_a_checkout_does_not_leak_that_repos_worktrees() {
        let inside = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/tmp");
        std::fs::create_dir_all(&inside).unwrap();

        // 울타리 없는 자리는 위의 체크아웃을 꼭대기로 읽는다 — 이 시험이 막는 그 길이다.
        let bare = crate::scratch::Scratch::in_place(&inside, "fence-none");
        let above = top_of(bare.path());
        assert!(above.is_some_and(|t| t != *bare.path()), "임시 자리가 체크아웃 밖이다 — 이 시험이 흉내 낼 것이 없다");

        let dir = crate::scratch::Scratch::fenced_in(&inside, "fence");
        assert_eq!(top_of(dir.path()).as_deref(), Some(canonical(dir.path()).as_path()), "훑기가 울타리를 넘어갔다");
        assert!(away(dir.path()).is_empty(), "울타리 위의 저장소가 새어 나왔다 — {:?}", away(dir.path()));
        assert!(away(&dir.join("nowhere")).is_empty(), "없는 자리에서도 위의 저장소를 읽었다");
        assert!(!is_linked(dir.path()), "울타리를 딸린 워크트리로 읽었다");
    }

    /// 어느 워크트리에서든 커밋·`pack-refs`·떼어 낸 checkout 이 지켜보는 표식을 바꾼다 —
    /// 탐색기가 갈라진 자리가 바뀐 것을 알아챈다(moai-pqrq).
    #[test]
    fn a_moved_head_in_any_worktree_changes_a_watched_stamp() {
        let scratch = crate::scratch::Scratch::fenced("heads");
        let base = scratch.path().to_path_buf();
        let (main, feat) = (base.join("main"), base.join("feat"));
        std::fs::create_dir_all(&main).unwrap();
        let run = |dir: &Path, args: &[&str]| {
            let out = crate::git::isolated(dir).args(args).output().unwrap();
            assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        };
        run(&main, &["init", "-q"]);
        run(&main, &["commit", "-q", "--allow-empty", "-m", "a"]);
        run(&main, &["worktree", "add", "-q", "../feat", "-b", "feat/x"]);

        let seen = heads(&main);
        let has = |tail: &str| seen.iter().any(|(p, _)| p.ends_with(tail));
        for tail in [".git/HEAD", "worktrees/feat/HEAD", "refs/heads/main", "refs/heads/feat/x", "packed-refs"] {
            assert!(has(tail), "{tail} 를 안 지켜본다 — {seen:#?}");
        }
        let changed = |was: &[(PathBuf, crate::store::Stamp)]| was.iter().any(|(p, s)| crate::store::stamp(p) != *s);
        assert!(!changed(&seen), "아무것도 안 했는데 표식이 바뀌었다");

        run(&feat, &["commit", "-q", "--allow-empty", "-m", "b"]);
        assert!(changed(&seen), "옆 워크트리의 커밋을 못 알아챈다");
        let seen = heads(&main);
        run(&main, &["pack-refs", "--all"]);
        assert!(changed(&seen), "pack-refs 를 못 알아챈다");
        let seen = heads(&main);
        run(&feat, &["checkout", "-q", "--detach"]);
        assert!(changed(&seen), "떼어 낸 checkout 을 못 알아챈다");
        let seen = heads(&main);
        run(&main, &["worktree", "add", "-q", "../more", "-b", "more"]);
        assert!(changed(&seen), "새로 생긴 워크트리를 못 알아챈다");

        // **파일로 읽은 이름 후보가 git 이 낸 목록과 같다**(moai-n2jh) — 주 워크트리에서도, 딸린
        // 워크트리에서도, 떼어 낸 HEAD 여도. 경로가 사라진 워크트리는 둘 다 뺀다.
        let gone = base.join("gone");
        run(&main, &["worktree", "add", "-q", "../gone", "-b", "gone"]);
        std::fs::remove_dir_all(&gone).unwrap();
        let by_git = |at: &Path| others_of(at).map(|(_, t)| names(t.iter().map(|(t, _)| t))).unwrap();
        for at in [&main, &feat, &base.join("more")] {
            assert_eq!(away(at), by_git(at), "{} 에서 파일로 읽은 목록이 git 과 다르다", at.display());
        }
        assert!(!away(&main).contains("gone"), "사라진 워크트리를 이름으로 댄다");
        assert!(away(&base.join("nowhere")).is_empty());

        // **같은 저장소의 같은 자리 트래커만 같다**(moai-23ky) — 옆 워크트리의 main 은 같고, 한
        // 저장소에 트래커를 둘 둔 모노레포의 `a`·`b` 는 다르다.
        for d in [main.join("a"), main.join("b"), feat.join("a")] {
            std::fs::create_dir_all(d).unwrap();
        }
        assert!(same_repo(&main, &feat), "옆 워크트리를 다른 저장소로 본다");
        assert!(same_repo(&main.join("a"), &feat.join("a")));
        assert!(!same_repo(&main.join("a"), &main.join("b")), "모노레포의 다른 트래커를 같은 자리로 본다");
        assert!(!same_repo(&main.join("a"), &feat), "꼭대기와 하위 트래커를 같은 자리로 본다");
        assert!(!same_repo(&main, &base.join("nowhere")));

        // 하위 디렉터리에서 부르면 `--git-common-dir` 이 상대 경로(`../.git`)로 온다.
        let sub = main.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        let seen = heads(&sub);
        assert!(seen.iter().any(|(p, s)| p.ends_with("refs/heads/more") && s.is_some()), "하위에서 가지 파일을 못 찾는다 — {seen:#?}");
        let _ = std::fs::remove_dir_all(&base);
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
