//! 다른 git 워크트리의 스냅샷을 **보여줄 때만** 겹친다.
//!
//! 에이전트 여럿이 워크트리를 하나씩 잡고 일하면 제 워크트리의 스냅샷은 그들이
//! 집고 옮긴 것을 모른다. `--worktree` 는 그것을 한 화면에 올린다.
//!
//! **어느 파일에도 쓰지 않는다.** 겹친 결과는 명령 하나가 끝나면 사라진다. 파일에
//! 합쳐 쓰는 순간 옛 moai 의 `merge=union` 이 돌아온다 — id 가 겹치고, 겹친 id 를
//! 가르려 relabel 하고, 그 별칭을 모든 조회가 풀어야 했다. 쓰기는 여전히
//! `store::with_write` 가 **파일 하나에만** 한다 — 2026-09-19 부터 그 하나는
//! 루트의 트래커다(moai-y7go, [`tracker_root`]). 겹친 것을 되쓰지 않는다는 말은
//! 그대로고, 바뀐 것은 어느 체크아웃의 `.moai` 냐다.
//!
//! **딱 하나 쓰는 것이 집은 표식이다**([`note_held`], 2026-09-19 사용자 결정) — git 이
//! 워크트리 몫으로 들고 있는 디렉터리의 `moai-held` 다. 겹친 결과가 아니라 "이 체크아웃이
//! 집었다" 는 기록이고, 커밋도 병합도 안 타며 워크트리를 치우면 같이 사라진다.
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
    /// 스냅샷은 못 겹쳤지만 **이름은 아는** 옆 워크트리 — (이름, 쥐었다고 볼 id 후보)(moai-ncsf).
    /// 스냅샷이 없거나(moai 를 들이기 전에 갈라졌다) 못 읽혀도 훅의 [`away`] 는 그 이름을 센다.
    /// 여기서 빠지면 훅은 "옆이 쥐었다" 로 아는 줄에 화면만 `⎇` 를 안 단다. [`Origin::labels`]·
    /// [`Origin::roots`] 에는 안 든다 — 겹쳐 본 곳도, 줄을 보탠 곳도 아니다.
    named: Vec<(String, BTreeSet<String>)>,
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
    ///
    /// **스냅샷을 못 읽은 옆 워크트리도 본다**(moai-ncsf) — 훅의 [`away`] 는 디스크의 옆 워크트리
    /// 전부에서 이름을 내므로, 겹친 곳만 보면 자가 다시 둘이 된다.
    pub fn working(&self, id: &str) -> Option<&str> {
        self.trees
            .iter()
            .map(|(label, _, holds)| (label, holds))
            .chain(self.named.iter().map(|(label, holds)| (label, holds)))
            .find(|(_, holds)| holds.contains(id))
            .map(|(label, _)| label.as_str())
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

    /// 스냅샷을 **못 겹친** 옆 워크트리의 이름들 — [`Origin::working`] 은 이것도 본다(moai-ncsf).
    /// [`Origin::labels`] 가 비었는데 이것이 있으면 옆 워크트리는 있되 겹칠 스냅샷이 없는 것이다 —
    /// 둘을 "옆 워크트리 없음" 한 말로 대면 같은 화면의 줄이 그 워크트리의 이름으로 `⎇` 를 단다.
    pub fn named_only(&self) -> Vec<&str> {
        self.named.iter().map(|(l, _)| l.as_str()).collect()
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
/// **옆을 빌려 받는다**(moai-kos1) — 부르는 쪽([`gather`])이 그 스냅샷을 그대로 들고 있어야
/// 자리 셈([`workplaces_in`])이 같은 파일을 다시 열지 않는다. 겹쳐 세우는 줄만 베끼므로 베끼는
/// 수는 보통 몇 줄이다 — 옆 줄은 거의 다 이쪽에도 같은 값으로 있어 그냥 지나간다.
pub fn overlay(mine: Vec<Issue>, others: &[Side]) -> (Vec<Issue>, Origin) {
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
    for (tree, Side { label, root, issues, base, holds }) in others.iter().enumerate() {
        origin.trees.push((label.clone(), root.clone(), holds.clone()));
        for i in issues {
            match at.get(&i.id) {
                Some(&k)
                    if (i.planned(), i.updated_at.as_str()) > (shown[k].planned(), shown[k].updated_at.as_str()) =>
                {
                    origin.from.insert(i.id.clone(), tree);
                    shown[k] = i.clone();
                }
                Some(_) => {}
                None if base.get(&i.id).is_some_and(|then| i.updated_at <= *then) => {}
                None => {
                    at.insert(i.id.clone(), shown.len());
                    origin.added.insert(i.id.clone());
                    origin.from.insert(i.id.clone(), tree);
                    shown.push(i.clone());
                }
            }
        }
    }
    // `store::read` 와 같은 차례로 돌려준다. **안정 정렬이다** — 제 파일의 겹친
    // id 두 줄이 읽은 차례를 지킨다.
    shown.sort_by(|a, b| a.id.cmp(&b.id));
    (shown, origin)
}

/// 옆 워크트리를 겹치다 만난 것 — **말이 아니라 자료다**(moai-dpbi). 글자는 [`crate::view`] 가
/// 쥔다(`view::trouble`).
///
/// 여기서 문장을 지으면 그 줄이 `moai status` 의 stderr 로 나가는데, 그 곁의 화면은 말묶음에서
/// 오고 이 줄만 한국어로 남아 **섞인 화면**이 된다. 그렇다고 [`gather`] 가 말을 받으면 그것을
/// 부르는 자리가 모두 말을 들고 와야 하는데, 그중에는 이 줄을 아예 안 쓰는 길(`cmd::read`)과
/// 말을 모르는 길(탐색기의 다시 읽기는 제 스레드에서 돈다)이 있다. 자료로 내면 쓰는 쪽만 말을 든다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trouble {
    /// 그 워크트리의 스냅샷을 못 읽었다 — 가지와 까닭.
    Unread { branch: String, why: String },
    /// 못 푸는 줄을 빼고 겹쳤다 — 가지·그 파일·뺀 줄 수.
    Skipped { branch: String, path: PathBuf, lines: usize },
    /// 옆 워크트리를 **찾지 못했다**([`Gathered::unfound`]) — git 이 없거나 저장소가 아니다.
    Unfound { lost: Lost, why: String },
}

/// 옆 워크트리를 못 찾은 갈래 — git 의 네 실패([`crate::git::Error`]). **낱말은 여기 없다.**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lost {
    NoGit,
    Failed,
    Stream,
    Encoding,
}

/// 겹쳐 읽은 결과.
pub struct Gathered {
    /// 제 워크트리의 `Load` 에 남의 줄을 겹친 것. **`errors` 는 제 파일의 것뿐이다**
    /// — 남의 못 읽는 줄로 `moai status` 가 비영 종료하면, 내 파일은 멀쩡한데
    /// 옆 워크트리 때문에 도구가 실패로 읽힌다.
    pub load: Load,
    pub origin: Origin,
    /// 남의 워크트리에서 만난 문제. **막지 않는다** — 부르는 쪽이 한 줄씩 알린다([`Trouble`]).
    pub trouble: Vec<Trouble>,
    /// 옆 워크트리를 **찾지 못한** 까닭(git 이 없거나 저장소가 아니다). `trouble` 과 가른다 —
    /// `--worktree` 를 시킨 CLI 는 말하지만, 겹쳐 보기를 기본으로 켜는 탐색기는 git 밖의
    /// 프로젝트를 열 때마다 시키지 않은 배너를 세우게 된다(moai-zcuh). 탐색기는 사람이
    /// `SPC v w` 로 켰을 때만 알림으로 댄다(`tui::App::unfound`, moai-d5vn).
    pub unfound: Option<Trouble>,
    /// **옆 워크트리를 빠짐없이 열어 봤다** — 겹쳐 보라고 시켰고 목록도 찾았다. 그러면 못 읽은 옆
    /// 스냅샷은 `trouble` 에 `⎇ <가지>: …` 로 이미 섰으니, 자리를 재다 못 읽은 워크트리
    /// (`stranded_at`)를 받는 쪽은 그것을 다시 말하지 않는다.
    ///
    /// **가지 이름으로 견주지 않는다**(moai-rgz9) — 떼어 낸 HEAD 의 이름은 커밋 앞 일곱 자라, 같은
    /// 커밋에 선 워크트리 둘이 글자까지 같아 하나를 말한 것이 둘을 다 말한 것으로 읽힌다. 한때
    /// 안쪽 `status` 는 이 자로, 밖 한눈 보기는 가지 이름 앞머리로 걸러 같은 상태에 답이 갈렸다.
    pub swept: bool,
    /// 읽으러 간 옆 스냅샷마다 **읽기 전에** 잰 표식. 탐색기가 바뀐 것을 알아채는 데
    /// 쓴다. 파일이 없던 곳도 든다 — 거기 스냅샷이 생기는 것도 바뀐 것이다.
    ///
    /// **재는 것이 읽는 것보다 먼저다** — 읽고 나서 재면 그 사이에 떨어진 쓰기가
    /// "이미 본 것" 으로 적혀 영영 안 보인다(`cmd::tui::run` 과 같은 까닭).
    ///
    /// 워크트리들의 HEAD 가 움직인 것을 알리는 git 파일도 든다([`heads`]) — 갈라진 자리가
    /// 바뀌면 지운 줄 숨김의 답이 바뀐다(moai-pqrq).
    pub watched: Vec<(PathBuf, crate::store::Stamp)>,
    /// **읽어 낸 옆 스냅샷 그대로**(moai-kos1) — 자리 셈([`workplaces_in`])이 같은 파일을 다시
    /// 열어 파지 않게 실어 보낸다. 바로 앞에서 겹치며 이미 판 것이라, 탐색기는 걸음마다 옆
    /// 스냅샷을 두 벌씩 풀고 있었다(이 저장소에서 ~130ms).
    ///
    /// **못 읽은 옆은 여기 없다** — 그쪽은 `trouble` 이 대고, 자리 셈이 제 손으로 열어 보고
    /// `unknown`·`broken` 을 세운다. 겹쳐 보지 않았으면 비어 있다.
    pub sides: Vec<Side>,
    /// **제 스냅샷을 겹치기 전에 잰 바닥**(moai-mafv) — 자리 셈([`workplaces_in`])이 같은 파일을
    /// 다시 열어 풀지 않게 실어 보낸다. 옆 스냅샷을 걷은 자리(moai-kos1)에 제 것이 남아 있었다:
    /// 그 파일은 `repo.read()` 가 방금 판 것이고 저장소에서 가장 큰 스냅샷이라, `status --worktree`
    /// 와 탐색기의 다시 읽기가 걸음마다 그것을 두 벌씩 풀었다.
    ///
    /// **옆 스냅샷과 달리 겹쳐 보지 않아도 선다** — main 체크아웃의 `moai status` 도 자리를 재려고
    /// 그 파일을 열었다. 재는 값은 줄마다 짧은 글 둘을 베끼는 것뿐이라(`Floor`), 자리를 안 묻고
    /// 끝나는 길(`ready`)이 치르는 값도 그 읽기의 백분의 일 아래다.
    pub mine: Floor,
}

/// 옆 워크트리의 줄이 **여기보다 늦게 만져졌는가** 를 견줄 바닥([`holds_of`] 의 `later`) — 제
/// 스냅샷에서 그 물음에 드는 만큼만(id → `planned`·`updated_at`) 잰 것이다.
///
/// **줄을 통째로 안 들고 다닌다.** 겹치는 길([`overlay`])이 그 `Vec` 을 먹으므로 남기려면 한 벌을
/// 베껴야 하는데, 재는 데 드는 것은 줄마다 짧은 글 둘뿐이다. 겹친 **뒤의** 줄로는 잴 수 없다 —
/// 옆에서 온 줄이 바닥에 서면 그 워크트리가 제 줄과 저를 견주게 되어 만진 흔적이 통째로 사라진다.
///
/// 재는 자는 하나다([`holds_of`]) — 건네받은 길과 제 손으로 파는 길이 같은 것을 지난다.
pub struct Floor {
    /// 어느 파일에서 잰 바닥인가 — 자리 셈([`workplaces_in`])은 제 `base`(main 의 스냅샷)와 **같을
    /// 때만** 건네받은 것을 쓴다. `MOAI_HERE=1` 은 부르는 쪽의 트래커를 그 워크트리의 `.moai` 로
    /// 가르는데, 그것을 main 의 바닥으로 삼으면 갈라질 때의 main 으로 지금 main 을 재어 옆이 만진
    /// 흔적이 뒤집힌다(갈라진 뒤 main 이 옮긴 줄이 전부 "옆이 만졌다" 로 선다).
    at: PathBuf,
    /// id → (`planned`, `updated_at`). 같은 id 가 둘이면 **뒷줄이 선다** — `Load::get` 과 같은 자다.
    rows: BTreeMap<String, (String, String)>,
}

impl Floor {
    /// 그 파일에서 읽은 줄로 — **자리를 함께 든다.**
    pub fn of(at: PathBuf, mine: &[Issue]) -> Floor {
        let rows = mine.iter().map(|i| (i.id.clone(), (i.planned().to_string(), i.updated_at.clone()))).collect();
        Floor { at, rows }
    }

    /// 자리 없이 — 그 파일과 견줄 일이 없는 부르는 쪽(훅의 [`held_elsewhere`])이 쓴다. 빈 자리는
    /// 어떤 스냅샷 자리와도 안 같아([`Dug::floor`]), [`Dug`] 에 실려도 자리 셈이 안 집어 든다.
    pub fn loose(mine: &[Issue]) -> Floor {
        Floor::of(PathBuf::new(), mine)
    }

    /// 옆의 이 줄이 **여기보다 늦게 만져졌는가.** 여기에 없는 줄은 그 워크트리에서 세운 것이라
    /// 든다 — 그것도 만진 흔적이다.
    fn later(&self, i: &Issue) -> bool {
        self.rows
            .get(i.id.as_str())
            .is_none_or(|(p, u)| (i.planned(), i.updated_at.as_str()) > (p.as_str(), u.as_str()))
    }
}

/// 부르는 쪽이 **이미 판 것** — 자리 셈([`workplaces_in`])이 같은 파일을 다시 열어 풀지 않게
/// 실어 보낸다. 옆 워크트리의 스냅샷(moai-kos1)과 제 스냅샷(moai-mafv) 둘 다 여기 실린다.
#[derive(Default)]
pub struct Dug<'a> {
    /// 옆 워크트리의 moai 뿌리(정규화) → 그 파일에서 읽은 줄([`dug`]).
    sides: BTreeMap<PathBuf, &'a [Issue]>,
    /// 부르는 쪽이 읽은 제 스냅샷([`Gathered::mine`]) — **자리가 같을 때만** 쓴다([`Floor::at`]).
    mine: Option<&'a Floor>,
}

impl<'a> Dug<'a> {
    /// 아무것도 안 건네받은 것 — 제 손으로 파는 길(훅·시험·[`stranded_at`])이 쓴다.
    pub fn new() -> Dug<'a> {
        Dug::default()
    }

    /// 그 워크트리의 스냅샷을 이미 팠으면 그 줄.
    ///
    /// **뿌리를 정규화해 맞춘다** — 겹치기는 git 에게 물어 목록을 얻고([`others_of`]) 자리 셈은 git 이
    /// 적어 둔 파일만 읽어([`on_disk`]), 같은 워크트리가 글자만 다른 경로로 올 수 있다. 못 맞추면 그
    /// 워크트리만 예전처럼 다시 판다 — 최악이 지금과 같다. **건네받은 것이 없으면 뿌리를 정규화하지도
    /// 않는다** — `canonical` 은 디스크를 묻는 자라, 안 겹쳐 보는 흔한 길이 워크트리마다 그 값을
    /// 헛되이 치르면 안 된다.
    fn side(&self, root: &Path) -> Option<&'a [Issue]> {
        match self.sides.is_empty() {
            true => None,
            false => self.sides.get(&canonical(root)).copied(),
        }
    }

    /// 자리 셈이 견줄 바닥을 부르는 쪽이 이미 쟀으면 그것 — **그 파일이 그 파일일 때만.**
    fn floor(&self, snapshot: &Path) -> Option<&'a Floor> {
        let mine = self.mine?;
        (!mine.at.as_os_str().is_empty() && canonical(&mine.at) == canonical(snapshot)).then_some(mine)
    }
}

/// 판 것([`Gathered::sides`]·[`Gathered::mine`])을 자리 셈([`workplaces_in`])이 찾을 모양으로.
pub fn dug<'a>(sides: &'a [Side], mine: &'a Floor) -> Dug<'a> {
    Dug { sides: sides.iter().map(|s| (canonical(&s.root), s.issues.as_slice())).collect(), mine: Some(mine) }
}

/// 제 저장소를 읽고, `worktree` 면 다른 워크트리의 스냅샷을 겹친다.
///
/// **읽기는 관대하다.** git 이 없거나, 저장소가 아니거나, 남의 파일이 깨졌어도
/// 제 스냅샷은 그대로 낸다 — 그런 것은 `trouble` 로 말만 한다. 스냅샷 파일이
/// 없는 워크트리는 moai 를 들이기 전에 갈라진 브랜치라 **말하지도 않는다.**
pub fn gather(repo: &Repo, worktree: bool) -> crate::fail::R<Gathered> {
    let load = repo.read()?;
    // **겹치기 전에 잰다**(moai-mafv) — 자리 셈이 견줄 바닥은 옆이 안 섞인 제 줄이다([`Floor`]).
    // 겹친 뒤에 재면 옆에서 온 줄이 바닥에 서서 그 워크트리가 만진 흔적이 통째로 사라진다.
    let mine = Floor::of(repo.issues_path(), &load.issues);
    if !worktree {
        return Ok(Gathered {
            load,
            origin: Origin::default(),
            trouble: Vec::new(),
            unfound: None,
            swept: false,
            watched: Vec::new(),
            sides: Vec::new(),
            mine,
        });
    }
    let mut trouble = Vec::new();
    let mut unfound = None;
    let mut others = Vec::new();
    // 스냅샷을 못 겹친 옆 워크트리의 이름 — 그래도 이름은 [`Origin::working`] 이 본다.
    let mut named = Vec::new();
    // HEAD 가 움직인 것도 다시 읽을 까닭이다 — **`others_of` 가 HEAD 를 읽기 전에** 잰다.
    let mut watched = heads(&repo.root);
    match others_of(&repo.root) {
        Err(why) => unfound = Some(why),
        Ok((me, trees)) => {
            // **이름을 `mine` 으로 두지 않는다**(리뷰) — 이 함수의 `mine` 은 위에서 잰 [`Floor`] 고,
            // 그것을 그대로 싣는 곳이 아래 `Gathered { .., mine }` 이다. 같은 이름이 둘이면 이 팔
            // 안팎으로 줄을 옮기는 날 어느 쪽이 실리는지가 눈으로 안 갈린다.
            let head = head_of(me.as_ref());
            let here: std::collections::HashSet<&str> = load.issues.iter().map(|i| i.id.as_str()).collect();
            let mut bases = Bases::new();
            for (tree, root) in trees {
                let path = root.join(".moai").join("issues.jsonl");
                watched.push((path.clone(), crate::store::stamp(&path)));
                match crate::store::read_snapshot(&path) {
                    unread @ (Err(_) | Ok(None)) => {
                        if let Err(e) = unread {
                            trouble.push(Trouble::Unread { branch: tree.label.clone(), why: e.to_string() });
                        }
                        // **디렉터리가 사라진 워크트리는 이름도 안 든다** — 훅의 `away` 가 읽는
                        // `on_disk` 가 그렇게 거른다. git 은 잠근 워크트리를 경로가 사라져도 목록에
                        // 남겨, 여기서 들면 훅은 제 초점으로 세는 줄에 화면만 `⎇` 를 단다.
                        //
                        // 그래서 **그 디렉터리가 사라지는 것도 다시 읽을 까닭이다**(moai-uyu9). 스냅샷
                        // 경로는 없음 → 없음이라 안 바뀌고, 워크트리 제거 명령 없이 `rm -rf` 로 치우면
                        // git 파일도 안 바뀐다 — 딸린 워크트리의 `.git`(`gitdir:` 한 줄)을 **재고 나서**
                        // 있는지 본다. 주 워크트리의 `.git` 은 디렉터리라 커밋마다 바뀌니 안 넣는다.
                        let dot_git = tree.path.join(".git");
                        if !dot_git.is_dir() {
                            watched.push((dot_git.clone(), crate::store::stamp(&dot_git)));
                        }
                        if tree.path.exists() {
                            named.push((tree.label.clone(), names([&tree])));
                        }
                    }
                    Ok(Some(other)) => {
                        if !other.errors.is_empty() {
                            trouble.push(Trouble::Skipped {
                                branch: tree.label.clone(),
                                path: path.clone(),
                                lines: other.errors.len(),
                            });
                        }
                        others.push(side(&repo.root, &here, head, tree, root, other.issues, &mut bases));
                    }
                }
            }
        }
    }
    let Load { issues, errors } = load;
    let (issues, mut origin) = overlay(issues, &others);
    origin.named = named;
    let swept = unfound.is_none();
    Ok(Gathered { load: Load { issues, errors }, origin, trouble, unfound, swept, watched, sides: others, mine })
}

/// 옆 워크트리 하나의 줄을 겹칠 모양으로 — 옆에만 있는 줄이 있으면 갈라진 자리([`Side::base`])를 댄다.
///
/// 갈라진 자리는 옆에만 있는 줄을 가를 때만 쓴다. 다 여기에도 있으면 git 을 두 번 더 부르지
/// 않는다 — 탐색기는 다시 읽을 때마다 여기를 지난다.
///
/// **HEAD 가 같은 옆끼리는 갈라진 자리를 나눠 쓴다**(moai-h498). 제 HEAD 는 하나라 옆 HEAD 가
/// 같으면 `merge-base`·`show` 의 답도 같다 — 갓 뜬 워크트리들은 흔히 같은 커밋에 서 있어(잴 때
/// 여덟 가운데 셋과 둘), 옆마다 git 을 두 번씩 부르던 판은 훅의 거절 길 하나에 174ms 를 썼다.
fn side(
    repo_root: &Path,
    here: &std::collections::HashSet<&str>,
    mine: Option<&str>,
    tree: Tree,
    root: PathBuf,
    issues: Vec<Issue>,
    bases: &mut Bases,
) -> Side {
    let lonely = issues.iter().any(|i| !here.contains(i.id.as_str()));
    let base = match mine {
        Some(m) if lonely => {
            bases.entry(tree.head.clone()).or_insert_with(|| base_of(repo_root, m, &tree.head)).clone()
        }
        _ => BTreeMap::new(),
    };
    // 이름 후보는 훅과 같은 자로 낸다 — 디렉터리 이름까지 여기서 안다.
    let holds = names([&tree]);
    Side { base, holds, ..Side::new(tree.label, root, issues) }
}

/// 제 줄을 **옆 워크트리의 스냅샷과 겹친 것**과, 그 목록이 낸 이름 후보 — 옆 이름과 **제 이름**
/// ([`away`] 와 같은 자) — 훅이 막기 전에 한 번 더 비춰 보는 자리다(moai-w2iy). `Stop` 도 집은 것이
/// 남을 때 에픽이 닫히는지를 이것으로 잰다(moai-8ema).
///
/// **제 이름도 여기서 낸다**(리뷰 moai-3k2d.1df). 옆 이름은 제 이름과 거리를 겨루는데(moai-m62u), 훅은 제
/// 스냅샷에 집은 줄이 없으면 워크트리 목록을 안 읽어 제 이름이 빈다 — 겹쳐 보는 까닭이 바로 그 스냅샷이
/// main 의 집기를 모를 때라, 빈 이름으로 재면 제 이름 워크트리의 멤버를 옆 에픽 워크트리에 넘겨 막았다.
///
/// 트래커는 main 에서 만지는 것이 규약이라(CLAUDE.md "워크트리"), 워크트리의 스냅샷(HEAD)은
/// main 에서 방금 세우고 집은 줄을 모른다. 그 낡은 스냅샷만 보고 막으면 시킨 대로 한 일이
/// 막힌다. 겹치는 규칙은 `--worktree` 와 같다([`overlay`]) — 훅만의 셈을 따로 두지 않는다.
///
/// **git 목록은 한 번만 읽는다** — 겹칠 줄과 이름 후보가 한 목록에서 나온다. 못 찾으면 `None`
/// 이고, 남의 못 읽는 줄은 말없이 빼고 겹친다 — 훅은 무엇이 어긋나도 조용해야 한다.
pub fn fresh(repo: &Repo, mine: Vec<Issue>) -> Option<(Vec<Issue>, crate::hook::Away)> {
    // **이름은 세션이 선 체크아웃에서 읽는다**(moai-y7go) — 트래커는 루트로 옮겨 가지만
    // (`store::Repo::find_from`) 제 이름은 그 워크트리의 것이다. 트래커의 자리로 읽던 판은 워크트리
    // 세션의 제 일이 겹쳐 본 판정에서 "옆의 것" 이 되어, 규칙 1 이 막아야 할 생성을 풀어 줬다.
    let (me, trees) = others_of(repo.here()).ok()?;
    let away =
        crate::hook::Away { names: names(trees.iter().map(|(t, _)| t)), own: names(me.as_ref()), ..Default::default() };
    let head = head_of(me.as_ref());
    let mut others = Vec::new();
    {
        let here: std::collections::HashSet<&str> = mine.iter().map(|i| i.id.as_str()).collect();
        let mut bases = Bases::new();
        for (tree, root) in trees {
            let Ok(Some(other)) = crate::store::read_snapshot(&root.join(".moai").join("issues.jsonl")) else {
                continue;
            };
            others.push(side(&repo.root, &here, head, tree, root, other.issues, &mut bases));
        }
    }
    Some((overlay(mine, &others).0, away))
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

/// 제 워크트리와, 다른 워크트리마다 (워크트리, 그 안의 moai 뿌리).
///
/// moai 뿌리가 워크트리 꼭대기가 아닐 수 있다(`.moai/` 를 하위 디렉터리에 둔
/// 저장소). **제 뿌리가 꼭대기에서 떨어진 만큼 남의 꼭대기에서도 떨어뜨린다** —
/// 같은 저장소의 워크트리는 같은 나무 모양이다.
fn others_of(root: &Path) -> Result<(Option<Tree>, Vec<(Tree, PathBuf)>), Trouble> {
    let top = git(root, &["rev-parse", "--show-toplevel"])?;
    let top = canonical(Path::new(top.trim_end_matches('\n')));
    let rel = canonical(root).strip_prefix(&top).map(Path::to_path_buf).unwrap_or_default();
    // `-z` 는 git 2.36 부터다. 그 전 git 에서 거절되면 줄로 가른 것을 NUL 로 바꿔
    // 같은 파서로 읽는다 — 줄바꿈 든 경로만 잃고, 겹쳐 보기 전체를 잃지는 않는다.
    let listed = git(root, &["worktree", "list", "--porcelain", "-z"])
        .or_else(|_| git(root, &["worktree", "list", "--porcelain"]).map(|s| s.replace('\n', "\0")))?;
    let (mine, others): (Vec<Tree>, Vec<Tree>) = parse(&listed).into_iter().partition(|t| canonical(&t.path) == top);
    Ok((
        mine.into_iter().next(),
        others
            .into_iter()
            .map(|t| {
                let root = t.path.join(&rel);
                (t, root)
            })
            .collect(),
    ))
}

/// 제 워크트리의 HEAD 커밋 — 갈라진 자리를 찾는 데 쓴다([`side`]). 아직 커밋이 없으면 없다.
fn head_of(me: Option<&Tree>) -> Option<&str> {
    me.map(|t| t.head.as_str()).filter(|h| !h.is_empty())
}

/// 옆 워크트리들의 **이름이 가리키는 id 후보**와 제 워크트리의 이름 후보 — 훅이 초점에서 뺄 것을
/// 잰다(`hook::Away`). 옆 이름은 제 이름보다 가까이 가리킬 때만 이긴다(moai-m62u).
///
/// **못 찾으면 비어 있다.** git 이 없거나 저장소가 아니면 옆도 없는 것이고, 그러면
/// 전처럼 스냅샷의 집은 줄이 다 제 초점이다. 훅은 무엇이 어긋나도 조용해야 한다.
pub fn away(root: &Path) -> crate::hook::Away {
    on_disk(root)
        .map(|d| crate::hook::Away { names: names(d.others()), own: names(d.mine()), ..Default::default() })
        .unwrap_or_default()
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
    /// 공용 git 디렉터리([`git_dirs`]) — main 이 없는 맨몸 저장소에서 자리 경로를 잴 자다([`main_top`]).
    common: PathBuf,
}

impl Disk {
    fn others(&self) -> impl Iterator<Item = &Tree> {
        self.all.iter().filter(|(_, _, me)| !me).map(|(t, ..)| t)
    }

    /// 제 워크트리 — 그 이름 후보가 훅의 제 이름([`crate::hook::Away::own`])이다.
    fn mine(&self) -> impl Iterator<Item = &Tree> {
        self.all.iter().filter(|(_, _, me)| *me).map(|(t, ..)| t)
    }

    /// main 워크트리. 맨몸 저장소면 없다.
    fn main(&self) -> Option<&Tree> {
        self.all.iter().find(|(_, linked, _)| !*linked).map(|(t, ..)| t)
    }
}

/// 옆 **딸린** 워크트리가 쥐었을 수 있는 줄 id (moai-ntl6, 사용자 결정 B). 제 이름은 훅이 이미 든
/// [`away`] 의 것을 쓴다(`hook::unsure`) — 여기서 한 벌 더 재던 판은 한 판정의 두 쪽이 제 이름을 따로 읽었다.
///
/// 이름이 id 가 아닌 워크트리(에이전트 격리 `worktree-agent-<해시>`, 옛 id 로 뜬 워크트리)가
/// 쥔 일은 이름으로 못 가른다. 대신 **갈라진 자리**로 짐작한다 — 규약상 집기는 main 에서 커밋한
/// 뒤 워크트리가 뜨므로, 그 워크트리의 스냅샷 파일에 벌여 놓인 줄은 갈라질 때 이미 집혀 있던
/// 일이다. 그 워크트리에서 main 보다 늦게 옮긴 줄(`planned`·`updated_at`)도 든다. 갈라진 **뒤에**
/// main 에서 집은 일은 그 파일에 없어 들지 않는다.
///
/// **main 워크트리는 쥔 곳으로 안 센다** — 모두의 집기가 모이는 자리라, 세면 모든 줄이 든다.
/// **답은 짐작이다** — 받는 쪽은 이 줄로 막거나 붙들거나 비추지 않기만 한다(`hook::unsure`). git 을
/// 띄우지 않고 파일만 읽지만 옆 스냅샷을 다 풀어 싸지 않다 — 거절 길, 비추는 길(`idea add` 의 물음),
/// `Stop` 에서만 부른다.
pub fn held_elsewhere(root: &Path, mine: &[Issue], cfg: &crate::config::Config) -> BTreeSet<String> {
    let Some(disk) = on_disk(root) else { return BTreeSet::new() };
    // 견줄 바닥은 **워크트리 수와 상관없이 한 벌이다** — 워크트리마다 지으면 제 줄을 그만큼 다시
    // 훑는다. 훅은 제 손으로 읽은 줄을 그대로 주므로 자리는 안 든다([`Floor::loose`]).
    // **첫 옆을 만날 때 짓는다**(리뷰) — 딸린 워크트리가 하나도 없는 흔한 체크아웃에서는 아래 고리가
    // 한 번도 안 도는데, 미리 지으면 그 판이 줄마다 짧은 글 셋을 헛되이 베낀다. 훅은 툴 부름마다
    // 도는 길이라(`hook::unsure`) 그 헛일이 제일 자주 걸린다.
    let floor = std::cell::OnceCell::new();
    let mut out = BTreeSet::new();
    for (tree, linked, me) in &disk.all {
        if *me || !*linked {
            continue;
        }
        // 훅은 겹쳐 본 결과를 들고 오지 않는다 — 판 것이 없으니 제 손으로 연다.
        let (open, later) =
            holds(&disk, tree, floor.get_or_init(|| Floor::loose(mine)), cfg, &Dug::new()).unwrap_or_default();
        out.extend(open);
        out.extend(later);
        // **그 워크트리가 적어 둔 집기도 든다**(moai-y7go, 리뷰 moai-71ht 셋째 판) — 쓰기가 루트로
        // 옮겨 가 옆 스냅샷이 안 움직이므로, 훅이 스냅샷만 보면 옆이 방금 집은 줄을 제 것으로 읽는다.
        // 그러면 `Stop` 이 남의 산 일을 닫으라고 붙들고 규칙 1 이 그 단위로 생성을 좁힌다. 자리 셈
        // ([`workplaces`])과 **같은 표식을 같은 자로** 읽어야 두 표면이 한 답을 낸다.
        if let Some(dir) = disk.admin.get(&tree.path) {
            out.extend(held_here(dir));
        }
    }
    out
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
fn holds(
    disk: &Disk,
    tree: &Tree,
    mine: &Floor,
    cfg: &crate::config::Config,
    dug: &Dug<'_>,
) -> Option<(BTreeSet<String>, BTreeSet<String>)> {
    // **이미 판 것이 있으면 다시 안 판다**(moai-kos1) — 겹쳐 보는 길([`gather`])은 바로 앞에서
    // 같은 파일을 열어 풀었다. 재는 자는 그대로 아래 하나다: 건네받은 것도 안 건네받은 것도
    // [`holds_of`] 를 지난다.
    if let Some(side) = dug.side(&tree.path.join(&disk.rel)) {
        return Some(holds_of(side, mine, cfg));
    }
    let path = snapshot_in(disk, tree);
    let side = match crate::store::read_snapshot(&path) {
        Ok(Some(side)) => side,
        Ok(None) => return Some(Default::default()),
        Err(_) => return None,
    };
    Some(holds_of(&side.issues, mine, cfg))
}

/// [`holds`] 의 셈 — **판 줄을 받아 잰다.** 파는 것과 재는 것을 가른 까닭은 겹쳐 보는 길이 같은
/// 파일을 이미 풀어 두기 때문이다(moai-kos1). 판정은 여기 하나다.
fn holds_of(side: &[Issue], mine: &Floor, cfg: &crate::config::Config) -> (BTreeSet<String>, BTreeSet<String>) {
    // **벌여 놓인 줄은 자리 셈의 자로 잰다**([`crate::report::started`]) — 가려진 쌍둥이 줄도 든다
    // (moai-es40, 사용자 결정). `report::wip` 은 그 줄을 빼므로, 그것으로 재면 머지가 남긴 id 충돌
    // 하나로 이 워크트리에서 도는 줄이 `places` 에서 자리를 잃는다. 훅의 짐작(`hook::unsure`)은
    // 제 초점(`wip`)과 겹치는 id 만 쓰므로 더 든 id 로 답이 안 바뀐다.
    let open = crate::report::started(side, cfg).into_iter().map(|i| i.id.clone()).collect();
    let later = side.iter().filter(|i| mine.later(i)).map(|i| i.id.clone()).collect();
    (open, later)
}

/// 그 워크트리의 스냅샷 파일 자리 — [`holds`] 와 값싼 문([`unreadable_snapshot`])이 같은 자를 쓴다.
fn snapshot_in(disk: &Disk, tree: &Tree) -> PathBuf {
    tree.path.join(&disk.rel).join(".moai").join("issues.jsonl")
}

/// **그 스냅샷을 못 읽는가** — 값싼 자다: 열어서 한 바이트를 읽어 본다.
///
/// 없는 것은 못 읽는 것이 아니다([`holds`] 의 `Ok(None)` 과 같은 답) — moai 를 들이기 전에
/// 갈라진 가지다. 자리에 디렉터리가 섰으면 열리기는 해도 안 읽혀 여기서 걸린다(moai-7p48 의
/// 재현이 그 꼴이다).
///
/// **읽어 보지는 않는다** — 스냅샷을 푸는 값이 이 문을 둔 까닭이고(moai-7igy 의 측정), 여는
/// 것은 그 값의 수천분의 일이다. 같은 자리에서 이미 워크트리마다 작은 파일 하나를 읽고 있다
/// ([`held_here`]). 그래서 이 문과 파는 길의 답이 갈리는 자리가 하나 남는다 — 열려서 첫 바이트도
/// 읽히는데 그다음이 깨진 파일. 그런 판은 파는 길이 제 답으로 덮는다.
///
/// **끊긴 것(`Interrupted`)은 못 읽은 것이 아니다** — `Read::read` 는 `EINTR` 를 스스로 다시
/// 걸지 않아(`read_exact` 와 다르다), 신호 하나가 멀쩡한 워크트리를 "깨졌다" 로 세울 수 있다.
/// 여기는 말만 하는 자리라 모를 때는 입을 다무는 쪽이 싸다 — 판정이 걸리는 자리면 파는 길이
/// 제 답으로 덮는다.
fn unreadable_snapshot(path: &Path) -> bool {
    use std::io::{ErrorKind, Read};
    let quiet = |k: ErrorKind| matches!(k, ErrorKind::NotFound | ErrorKind::Interrupted);
    match std::fs::File::open(path) {
        Err(e) => !quiet(e.kind()),
        Ok(mut f) => f.read(&mut [0u8]).is_err_and(|e| !quiet(e.kind())),
    }
}

/// 살아 있는 **딸린** 워크트리마다 자리 하나(moai-ir8q) — 판정은 `report::places`·`report::stranded`.
///
/// main 워크트리는 안 든다 — 모두의 집기가 모이는 자리라 거기 선 줄은 "어디서 하는가" 에 답이
/// 안 된다. 파일만 읽는다. 저장소가 아니면 비어 있다. **경로는 main 워크트리의 꼭대기에서 잰
/// 상대 경로다**([`main_top`]) — `status`·`show` 가 이 값을 그대로 낸다.
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
/// 2. 이름이 답을 낸 줄은 **스냅샷과 집은 표식([`note_held`])으로만 보이는 둘째 자리를 못 본다.**
///    이름이 가리키는 워크트리가 하나 있으면 거기서 멈추므로, 같은 줄을 실제로 만지고 있는 이름 없는 워크트리
///    (에이전트 격리)가 `moai show <id>` 의 `자리` 줄에서 빠진다. 그 줄을 읽고 들어가는 것이
///    이어받는 세션이라 값이 0 은 아니다. 훅은 이 길을 지나지 않는다(`hook` 은 `workplaces` 도
///    `places` 도 안 부른다) — 무는 것은 사람과 감독이 부르는 `status`·`show` 다.
///
/// **팔 까닭은 부르는 쪽의 줄로 잰다**(`asked`) — `touched` 의 기준인 `mine`(main 의 스냅샷,
/// moai-40ht.hom) 으로 재면 안 된다. `--worktree` 로 겹쳐 본 쪽은 옆 워크트리에서 만들고 집은
/// 줄까지 `places`·`stranded` 에 거는데, main 의 스냅샷에는 그 줄이 없어 "이름으로 다 잡혔다" 로
/// 읽힌다 — 그러면 `holds` 가 빈 채로 나가 그 줄이 통째로 `Lost` 로 서고, 살아 있는 세션의 일이
/// `stranded` 경고와 `show` 의 `자리 없다` 로 뒤집힌다.
// 바이너리는 재료를 든 [`workplaces_in`] 을 부른다(moai-rviv) — 이 꼴은 시험의 짧은 길이다.
#[cfg(test)]
pub fn workplaces(
    root: &Path,
    cfg: &crate::config::Config,
    worktree: bool,
    asked: &[Issue],
) -> Vec<crate::report::Workplace> {
    workplaces_in(root, worktree, &crate::report::Footing::of(asked, cfg), &Dug::new())
}

/// [`workplaces`] 와 같은 것. **자리 판정의 재료를 받는다**([`crate::report::Footing`]) — 문을 여는
/// 자리와 그 뒤의 판정([`crate::report::places`]·[`crate::report::stranded`])이 같은 재료를 나눠
/// 쓴다. 저마다 지으면 `moai status` 한 번에 집은 줄과 소속 지도를 네댓 벌 짓는다(moai-rviv).
///
/// 재료는 **게으르다** — 아래 세 갈래(딸린 워크트리에서 안 겹쳐 볼 때, git 밖, 딸린 워크트리가
/// 하나도 없을 때)는 그것을 하나도 안 보고 돌아서므로, 그 길의 값은 예전 그대로다.
pub fn workplaces_in(
    root: &Path,
    worktree: bool,
    footing: &crate::report::Footing<'_, '_>,
    dug: &Dug<'_>,
) -> Vec<crate::report::Workplace> {
    if !worktree && is_linked(root) {
        return Vec::new();
    }
    let Some(disk) = on_disk(root) else { return Vec::new() };
    // **거를 자를 한 번만 적는다** — 자리와 그 워크트리를 아래에서 `zip` 으로 맞추므로, 거르는
    // 줄이 둘이면 한쪽만 고쳐졌을 때 자리가 남의 워크트리의 스냅샷을 받아 든다.
    let linked: Vec<&Tree> = disk.all.iter().filter(|(_, linked, _)| *linked).map(|(tree, ..)| tree).collect();
    // 딸린 워크트리가 하나도 없으면 여기서 끝이다 — 아래의 문도, main 의 스냅샷도 볼 까닭이 없다
    // (`report::stranded` 도 빈 목록에는 조용하다). 워크트리 규약을 안 쓰는 저장소의 흔한 길이다.
    if linked.is_empty() {
        return Vec::new();
    }
    // **경로는 여기서 한 번 잰다**([`main_top`]) — 이미 읽은 목록으로 재므로 git 이 적어 둔
    // 파일을 다시 안 읽는다. 부르는 쪽마다 따로 재던 때는 자가 둘이라 빈 경로를 다루는 법이
    // 갈렸고, 둘 다 같은 목록을 한 벌 더 읽었다.
    let top = main_top(&disk);
    let bare = |tree: &Tree| crate::report::Workplace {
        path: from_top(&top, &tree.path),
        branch: tree.label.clone(),
        names: names([tree]),
        holds: BTreeSet::new(),
        touched: BTreeSet::new(),
        marked: BTreeSet::new(),
        born: None,
        unknown: false,
        broken: false,
    };
    let mut out: Vec<crate::report::Workplace> = linked.iter().map(|tree| bare(tree)).collect();
    // **집은 표식은 빠른 길에서도 읽는다**([`note_held`], 리뷰 moai-71ht 셋째 판의 훑기) — 옆 스냅샷과
    // 달리 워크트리마다 작은 파일 하나라 값이 거의 없고, 파는 길에서만 읽으면 같은 상태에 두 답이
    // 난다(이름이 다 답하면 안 읽혀 그 자리가 사라진다). `report::places` 가 이것만은 이름 좁히기에
    // 안 버린다.
    for (place, tree) in out.iter_mut().zip(&linked) {
        if let Some(dir) = disk.admin.get(&tree.path) {
            place.marked = held_here(dir);
        }
    }
    // 이름만으로 자리가 다 잡히면(또는 집은 줄이 없으면) 스냅샷을 한 벌도 안 판다.
    //
    // **묻는 것은 하나다 — 이름이 집은 줄을 다 가리키는가.** `holds`·`touched` 가 아직 비었으니
    // `report::places` 의 판정도 여기서는 그 하나로 접히는데, 그렇다고 판정을 통째로 돌려 한
    // 낱말만 건지면 (1) 시계를 한 번 더 읽고 (2) 소속 지도와 굴림까지 지어 버리고 (3) 판정에
    // 변형이 하나 늘 때마다 **디스크를 언제 만지는가**가 조용히 따라 바뀐다. 그래서 그 하나를
    // 재는 자(`report::claimed` — 훅의 초점과 `places` 가 거리를 겨루는 `report::nearness` 의 "가리키는가"
    // 쪽이다)를 바로 쓴다.
    // 집은 줄도 `places` 가 되짚는 그 집합([`crate::report::started`])이다 — `wip` 은 가려진 쌍둥이
    // 줄을 빼므로(moai-es40), 그것으로 재면 그 줄 하나 때문에 열어야 할 문이 닫혀 스냅샷으로만 찾을
    // 수 있는 그 줄이 `stranded` 로 선다. 이름으로는 거의 못 찾는다 — 쌍둥이(뒷줄)가 그 id 의 에픽을
    // `groups` 에서 지워, 규약대로 에픽 이름으로 뜬 워크트리도 그 줄을 못 가리킨다.
    let all_names = names(linked.iter().copied());
    if footing.picked().values().all(|i| footing.claims(&all_names, i)) {
        // **깨진 스냅샷은 안 파는 길에서도 선다**(moai-giz3, idea moai-7p48) — 이 문이 닫히면
        // 이 워크트리는 `unknown` 도 `holds` 도 없이 지나가, 깨진 파일이 어느 화면에도 안 섰다.
        // 그 사실은 자리 판정과 상관없이 고칠 사람이 있어야 고쳐진다([`crate::report::Workplace::broken`]).
        //
        // **묻는 곳은 여기 하나다** — 파는 길에서는 아래가 실제로 읽은 답으로 `broken` 을 통째로
        // 덮으므로, 거기서 또 열어 보면 워크트리마다 버릴 `open` 하나씩이다.
        for (place, tree) in out.iter_mut().zip(&linked) {
            place.broken = unreadable_snapshot(&snapshot_in(&disk, tree));
        }
        return out;
    }
    // 여기서부터가 파는 길이다 — **뜬 때도 여기서 읽는다.** `born` 은 이름 없는 워크트리의
    // `holds` 를 가릴 때만 보는 값이라(`report::places`), 안 파는 길에서는 워크트리마다
    // `logs/HEAD` 를 한 번씩 읽고 버리는 헛일이었다(`Disk::admin` 이 같은 까닭으로 `on_disk`
    // 에서 이것을 뺐다).
    let base = disk.main().map(|t| t.path.join(&disk.rel)).unwrap_or_else(|| root.to_path_buf());
    let snapshot = base.join(".moai").join("issues.jsonl");
    // **부르는 쪽이 이미 판 파일이면 다시 안 판다**(moai-mafv) — 옆 스냅샷을 걷은 자리(moai-kos1)에
    // 제 것이 남아 있었다. 이것은 `repo.read()` 가 방금 판 그 파일이고 저장소에서 가장 큰 스냅샷이다.
    //
    // **그 파일이 그 파일일 때만이다**([`Dug::floor`]) — `MOAI_HERE=1` 은 부르는 쪽의 트래커를 그
    // 워크트리의 `.moai` 로 가르는데, 여기 `base` 는 언제나 main 의 것이다. 안 보고 쓰면 갈라질 때의
    // main 으로 지금 main 을 재어 자리가 뒤집힌다. 못 맞추면 예전처럼 제 손으로 판다.
    let own;
    let mine = match dug.floor(&snapshot) {
        Some(floor) => floor,
        None => {
            // 못 읽으면 바닥이 빈다 — 옆의 줄이 다 만진 흔적이 되어 자리를 넉넉히 대는 쪽으로 틀린다.
            let read = crate::store::read_snapshot(&snapshot).ok().flatten().unwrap_or_default();
            own = Floor::of(snapshot, &read.issues);
            &own
        }
    };
    for (place, tree) in out.iter_mut().zip(&linked) {
        place.born = disk.admin.get(&tree.path).and_then(|dir| born_of(dir));
        // **제 워크트리도 남과 같은 자로 잰다.** 한때 비워 두었더니, 이름이 id 가 아닌
        // 워크트리(에이전트 격리)가 제가 하고 있는 일을 제 화면에서 "자리 없다" 로 댔다.
        // **판 결과가 임자다** — 위의 값싼 문은 열어 보는 데까지고, 여기는 실제로 읽은 답이다.
        match holds(&disk, tree, mine, footing.cfg(), dug) {
            Some((holds, touched)) => {
                place.holds = holds;
                place.touched = touched;
                place.broken = false;
            }
            None => {
                place.unknown = true;
                place.broken = true;
            }
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

/// 이 체크아웃의 꼭대기와 **제 git 디렉터리**, 그리고 딸린 워크트리인가 — 주 워크트리면 `.git`
/// 그대로고, 딸린 워크트리면 `gitdir:` 가 가리킨 `<공용>/worktrees/<이름>` 이다. git 밖이면 `None`.
///
/// **`gitdir:` 을 푸는 자는 하나다** — [`git_dirs`] 도 집은 표식([`note_held`])도 여기를 지난다.
/// 따로 적던 판은 같은 한 줄을 두 벌로 풀었고, `place_marks` 의 문이 이미 이름 붙여 둔 갈림이다
/// ("여기서 따로 훑으면 `gitdir` 를 푸는 법이 두 벌이 되어…").
fn own_git(root: &Path) -> Option<(&Path, PathBuf, bool)> {
    let top = root.ancestors().find(|d| d.join(".git").exists())?;
    let dotgit = top.join(".git");
    if dotgit.is_dir() {
        return Some((top, dotgit, false));
    }
    let text = std::fs::read_to_string(&dotgit).ok()?;
    Some((top, canonical(&top.join(text.trim_end().strip_prefix("gitdir:")?.trim())), true))
}

/// 이 체크아웃이 딸린 워크트리면 git 이 그 몫으로 들고 있는 디렉터리(`<공용>/worktrees/<이름>`) —
/// 주 워크트리와 git 밖은 `None` 이다.
///
/// 집은 표식([`note_held`])이 사는 자리다. **거기는 커밋도 병합도 안 탄다** — 워크트리를 치우면
/// 표식도 같이 사라지고, 스냅샷을 안 건드리니 병합에서 겨룰 것이 없다.
fn admin_dir(root: &Path) -> Option<PathBuf> {
    match own_git(root)? {
        (_, dir, true) => Some(dir),
        (_, _, false) => None,
    }
}

/// 집은 표식을 고친다(moai-y7go, 2026-09-19 사용자 결정) — **친 자리**(`at`)가 벌여 놓은 칸으로
/// 옮긴 id(`claimed`)를 그 자리의 표식에 더하고, 놓거나 지운 id(`released`)는 **이 저장소의 모든**
/// 표식에서 뺀다. 못 적으면 조용히 넘어간다.
///
/// **왜 있는가.** 쓰기가 루트로 옮겨 가면서([`tracker_root`]) 워크트리의 스냅샷이 더는 안 움직인다.
/// 이름이 id 인 워크트리는 [`names`] 가 가리켜 그대로지만, 이름이 id 가 아닌 워크트리(에이전트 격리
/// `worktree-agent-…`)는 "여기서 집었다" 를 말할 길을 통째로 잃어 산 일이 [`crate::report::places`]
/// 에서 자리를 잃고 `stranded` 경고로 섰다. 스냅샷에 적지 않는 까닭은 늘 같다 — 그것은 병합에서
/// 겨룬다. git 이 그 워크트리 몫으로 들고 있는 디렉터리는 안 겨룬다.
///
/// **놓기는 어디서 쳤든 모든 표식에서 뺀다**(리뷰 moai-71ht 셋째 판). 제 자리에서 놓은 것만 빼던
/// 판은 규약대로 루트에서 닫은 줄(`moai mv <id> done` on develop)을 옛 워크트리의 표식에 남겼고,
/// 그 줄을 다시 집는 순간 아무도 일하지 않는 그 워크트리가 자리로 서서 `stranded` 가 영영 안 섰다.
/// 같은 까닭으로 **옆이 집어 간 줄은 옛 자리에서 뺀다** — 넘겨받은 일이 두 곳에 서면 안 된다.
///
/// **주 체크아웃의 트래커일 때만 만진다** — `MOAI_HERE` 로 딸린 워크트리의 트래커에 일부러 쓴 것은
/// 갈라 놓은 스냅샷이라, 그 줄로 남의 표식을 고칠 근거가 못 된다.
pub fn note_held(tracker: &Path, at: Option<&Path>, claimed: &[String], released: &[String]) {
    if claimed.is_empty() && released.is_empty() {
        return;
    }
    let Some((_, common, false)) = own_git(tracker) else { return };
    let common = canonical(&common);
    // 딸린 워크트리가 하나도 없으면 표식도 없다 — 워크트리를 안 쓰는 저장소의 흔한 길에서 락 파일
    // 하나도 안 만든다.
    if !common.join("worktrees").is_dir() {
        return;
    }
    // **표식은 제 락으로 지킨다**(리뷰 moai-71ht 셋째 판) — 트래커의 락으로는 모자라다. 모노레포는
    // 한 워크트리에 트래커를 여럿 두고(`a/.moai`·`b/.moai`) 그 락이 따로인데 표식 파일은 하나라,
    // 둘이 같이 고치면 한쪽의 집기가 조용히 사라졌다(잰 판: 1,228번에 159번). 락은 갈아끼우지 않는
    // 파일에 건다 — 표식 자체에 걸면 `write_atomic` 의 rename 뒤로 둘이 다른 inode 를 쥔다.
    // 못 잡으면 그냥 적는다: 표식은 없는 것보다 낡은 것이 낫고, 훅은 무엇이 어긋나도 조용해야 한다.
    // 못 잡은 까닭의 글은 **버린다** — 여기서는 답을 안 쓰므로 말이 설 자리가 없다(moai-iq7j).
    let _lock = crate::store::Lock::acquire(&common.join("moai-held.lock"), || crate::i18n::Lang::En);
    // 친 자리의 표식 — **같은 저장소의 딸린 워크트리일 때만.** `<공용>/worktrees/<이름>` 의 두 단계
    // 위가 이 저장소의 공용 디렉터리인지로 잰다(남의 저장소에서 친 것과 주 체크아웃은 여기서 빠진다).
    let own = at.and_then(admin_dir).filter(|dir| dir.parent().and_then(Path::parent) == Some(common.as_path()));
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(common.join("worktrees"))
        .map(|rd| rd.filter_map(Result::ok).map(|e| canonical(&e.path())).collect())
        .unwrap_or_default();
    if let Some(own) = &own
        && !dirs.contains(own)
    {
        dirs.push(own.clone());
    }
    for dir in dirs {
        let was = held_here(&dir);
        let mut ids = was.clone();
        for id in released {
            ids.remove(id);
        }
        match own.as_ref() == Some(&dir) {
            true => ids.extend(claimed.iter().cloned()),
            // 옆이 집어 간 줄은 옛 자리에서 뺀다 — 친 자리를 모를 때는 아무 데도 안 건드린다.
            false if own.is_some() => ids.retain(|id| !claimed.contains(id)),
            false => {}
        }
        if ids != was {
            // **제자리에서 갈아끼운다**([`crate::store::write_atomic`]) — 읽는 쪽(`workplaces`)은 락을
            // 안 잡으므로, 잘라 놓고 쓰는 사이에 읽으면 산 일이 잠깐 자리를 잃는다.
            let text: String = ids.iter().map(|id| format!("{id}\n")).collect();
            let _ = crate::store::write_atomic(&dir.join(HELD), text.as_bytes());
        }
    }
}

/// 그 체크아웃이 적어 둔 집은 줄들([`note_held`]). 없으면 비어 있다.
fn held_here(admin: &Path) -> BTreeSet<String> {
    std::fs::read_to_string(admin.join(HELD))
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

/// 집은 표식 파일의 이름 — 적는 자와 읽는 자가 같은 글자를 쓴다.
const HELD: &str = "moai-held";

/// 이 트래커를 **옮겨 쓸 자리** — 딸린 워크트리면 주 체크아웃의 같은 자리고, 그대로 둘 것이면
/// `None` 이다(moai-y7go).
///
/// 워크트리의 `.moai` 를 고치면 병합에서 스냅샷이 충돌한다. [`crate::store::Repo::find_from`] 이
/// 이것으로 옮겨 가, 워크트리 안에서 친 `moai` 도 루트의 트래커를 읽고 쓴다.
///
/// **옮길 곳에 트래커가 있어야 옮긴다** — 이 가지에서 처음 `init` 한 워크트리는 루트에 `.moai` 가
/// 없다. 딸린 워크트리가 아닌 것과 주 체크아웃이 없는 것(git 밖·서브모듈·맨 저장소에 딸린
/// 워크트리)은 [`main_root`] 가 이미 `None` 으로 가른다 — 여기서 [`is_linked`] 로 한 번 더 물으면
/// 같은 조상 훑기를 두 번 하고, 두 자가 갈리면(중간에 선 `.git` 파일) 안 본 자리를 옮겨 준다.
///
/// **찾은 자리를 되돌려 주지 않는다**(리뷰 moai-71ht.jlh) — 그러면 부르는 쪽이 `root != found`
/// 를 견줘야 하는데, 경로 견주기는 풀린 꼴과 안 풀린 꼴이 갈려 조용히 틀리는 자다.
///
/// **트래커가 있다는 것은 설정이 있다는 것이다**(리뷰 moai-71ht 셋째 판) — `.moai` 가 디렉터리인
/// 것만 보던 판은 무시되는 `.moai/lock` 하나만 남은 루트(옛 커밋을 체크아웃하거나 bisect 하면
/// 남는다)를 트래커로 읽어, 그 저장소의 **모든** 워크트리가 "설정이 없다" 로 넘어졌다. 락은
/// `Lock::drop` 이 안 지우므로 트래커가 통째로 사라져도 그 파일만 남는다.
pub fn tracker_root(root: &Path) -> Option<PathBuf> {
    let main = main_root(root)?;
    main.join(".moai").join("config.toml").is_file().then_some(main)
}

/// 이 트래커가 든 **제** 워크트리의 꼭대기. git 을 띄우지 않는다. 저장소가 아니면 없다.
///
/// 뿌리(`.moai` 가 든 디렉터리)로 재면 안 된다 — 모노레포처럼 `.moai` 가 아래에 있으면 워크트리
/// 경로가 그 밑에 없어 하나도 안 잘리고, 기계의 절대 경로가 `--json` 으로 그대로 나간다.
///
/// **자리 경로를 이것으로 재지 않는다** — 부르는 쪽이 딸린 워크트리면 옆 워크트리가 하나도 안
/// 잘린다. 자리 경로는 [`main_top`] 이 잰다(moai-fygk). main 이 없을 때의 물러설 곳이던 자리도
/// 저장소 디렉터리가 맡아(moai-3aec), 이제는 시험이 [`git_dirs`] 의 훑기를 재는 데만 쓴다.
#[cfg(test)]
fn top_of(root: &Path) -> Option<PathBuf> {
    git_dirs(root).map(|(top, _)| canonical(top))
}

/// **main 워크트리의 꼭대기** — 자리 경로를 여기서 잰다(moai-fygk).
///
/// [`top_of`] 로 재면 부르는 쪽이 딸린 워크트리일 때 **옆 워크트리의 경로가 하나도 안 잘린다** —
/// 규약상 세션은 대개 워크트리 안에서 도므로(CLAUDE.md "워크트리") 그게 흔한 경우다. 딸린
/// 워크트리는 모두 main 아래 `.claude/worktrees/` 에 서므로, main 에서 재면 어느 자리에서 부르든
/// 같은 상대 경로가 나온다 — 그래야 그 글자를 그대로 `EnterWorktree` 에 옮길 수 있다.
///
/// **main 을 못 찾으면(맨몸 저장소) 그 저장소 디렉터리에서 잰다**(moai-3aec, 2026-09-18 사용자 결정).
/// 제 꼭대기로 돌아가던 판은 같은 워크트리가 부르는 자리마다 다른 글자로 나왔다 — main 이 하던
/// "어디서 불러도 같은 자" 의 역을 저장소 그 자체가 맡는다. [`workplaces`] 가 이미 읽은 목록으로
/// 잰다 — git 이 적어 둔 파일을 다시 안 읽는다. 그래서 **늘 자가 있다** — 목록을 읽었으면 공용
/// 디렉터리도 이미 찾은 것이다.
fn main_top(disk: &Disk) -> PathBuf {
    disk.main().map_or_else(|| canonical(&disk.common), |t| canonical(&t.path))
}

/// 워크트리 경로를 [`main_top`] 에서 잰 것으로. **늘 상대 경로다**(moai-xpd7·moai-3aec, 2026-09-18
/// 사용자 결정) — 그 밑이 아니면(main 밖에 만든 워크트리) `../` 로 올라가서 잰다. 밖을 절대 경로로
/// 두던 판은 한 배열에 두 모양이 섞여 받는 쪽이 못 갈랐고, 기계의 홈 경로가 `--json` 으로 나갔다.
///
/// **꼭대기와 같은 자리면 `.` 이다** — 빈 경로로 두면 사람 화면의 `자리` 칸이 통째로 비고
/// `--json` 의 `path` 가 `""` 로 나간다. 겹치는 머리가 없으면(다른 드라이브) 그대로 둔다 — 잴 자가 없다.
fn from_top(top: &Path, path: &Path) -> PathBuf {
    use std::path::Component;
    let path = canonical(path);
    let (up, down): (Vec<Component>, Vec<Component>) = (top.components().collect(), path.components().collect());
    let same = up.iter().zip(&down).take_while(|(a, b)| a == b).count();
    if same == 0 {
        return path;
    }
    let rel: PathBuf =
        std::iter::repeat_n(Component::ParentDir, up.len() - same).chain(down[same..].iter().copied()).collect();
    match rel.as_os_str().is_empty() {
        true => PathBuf::from("."),
        false => rel,
    }
}

/// 이 자리에서 잰 **사람이 읽을 파일 이름** — [`from_top`] 과 같은 자이되 글자로 낸다. 보드의 머리가
/// "이 판을 어느 파일에서 읽었나" 를 댈 때 쓴다(`cmd::status`): 딸린 워크트리 안에서는 그 자리의
/// `.moai` 가 아니라 루트의 트래커라, `.moai/issues.jsonl` 로 못박아 두면 보드가 안 읽은 파일을 댄다.
pub fn told_from(here: &Path, path: &Path) -> String {
    from_top(&canonical(here), path).display().to_string()
}

/// 자리를 재다 못 읽은 워크트리들 — **두 사실을 두 채널로 가른다**(사용자 결정 2026-09-18, 리뷰
/// moai-rgz9.7vt).
///
/// `all` 은 "이 스냅샷이 깨졌다" 고, `blinding` 은 "그래서 자리를 다 못 셌다" 다. 이름이 집은 줄을
/// 가리키는 워크트리는 못 읽어도 판정을 안 가리므로 `blinding` 에 안 드는데, 그렇다고 깨진 파일을
/// 아무 데서도 안 말하면 그 워크트리를 고칠 사람이 그것을 영영 모른다 — 고치는 것과 못 센 것은
/// 다른 말이라 세는 자리를 가른다.
///
/// **`all` 은 판 것에 매이지 않는다**(moai-giz3, idea moai-7p48 이 재현). [`workplaces_in`] 은
/// 이름만으로 자리가 다 잡히면 옆 스냅샷을 한 벌도 안 푸는데(moai-7igy, 잰 뒤 사람이 정한 문),
/// 한때는 그 길에서 깨진 파일이 어느 화면에도 안 서서 같은 저장소를 `moai status` 는 조용히
/// 지나고 `moai status --worktree` 는 그 워크트리를 댔다. 이제 안 파는 길도 파일을 **열어 보고**
/// (`unreadable_snapshot`) `Workplace::broken` 을 세운다 — 여는 값은 푸는 값의 수천분의 일이고,
/// 깨진 파일은 고칠 사람이 있어야 고쳐진다. `blinding` 은 여전히 **판 것 가운데** 다 —
/// "자리를 다 못 셌다" 는 실제로 풀어 봐야 나오는 말이다.
pub struct Unread {
    /// 스냅샷을 못 읽은 워크트리 전부 — 사람 화면이 `⎇ <가지>: <경로>` 로 한 줄씩 대고
    /// `옆 워크트리 문제 N건` 이 센다. 기계에는 `status --json` 의 `broken_worktrees` 다(moai-zah3).
    pub all: Vec<crate::report::Workplace>,
    /// 그중 **자리 판정을 가린** 것([`crate::report::blinding`]) — `status --json` 의
    /// `unreadable_worktrees` 와 탐색기 층의 셈이 이것이다. `stranded` 의 침묵이 "없다" 인지
    /// "못 셌다" 인지를 가르는 자라, 안 가린 것까지 들면 다 세고도 "못 셌다" 가 된다.
    pub blinding: Vec<crate::report::Workplace>,
}

/// 자리 없는 줄 경고와, 못 읽은 워크트리들 — **표면 셋이 같은 자를 쓴다**(moai-p3bs).
///
/// `moai status`·`.moai` 밖 한눈 보기·탐색기의 프로젝트 층이 이것을 부른다. 한때 첫째만 자리를
/// 셌고, 그래서 **죽은 세션을 찾으러 돌아온 사람이 보는 화면**(층과 밖 한눈 보기)에만 그 말이
/// 없었다. 경로는 [`workplaces`] 가 이미 [`main_top`] 에서 잰 것이다 — `show` 와 같은 자다.
pub fn stranded_at(
    root: &Path,
    cfg: &crate::config::Config,
    issues: &[Issue],
    worktree: bool,
    now: &str,
) -> (Option<crate::report::Warning>, Unread) {
    stranded_at_in(root, cfg, issues, worktree, now, &Dug::new())
}

/// [`stranded_at`] 과 같은 것. **겹치며 이미 판 옆 스냅샷을 받는다**([`Dug`], moai-kos1) — 탐색기와
/// `moai status --worktree` 는 바로 앞에서 그 파일들을 열어 풀었다.
pub fn stranded_at_in(
    root: &Path,
    cfg: &crate::config::Config,
    issues: &[Issue],
    worktree: bool,
    now: &str,
    dug: &Dug<'_>,
) -> (Option<crate::report::Warning>, Unread) {
    // **재료는 한 벌이다**([`crate::report::Footing`], moai-rviv) — 문(`workplaces_in`)과 그 뒤의
    // 판정 둘이 같은 줄을 세고 같은 소속 지도를 읽는다. 저마다 지으면 `moai status` 한 번에 집은
    // 줄을 네 번 고르고 `groups` 를 예닐곱 번 짓는다. 게을러서, 볼 워크트리가 없으면 안 짓는다.
    let footing = crate::report::Footing::of(issues, cfg);
    let trees = workplaces_in(root, worktree, &footing, dug);
    let warning = crate::report::stranded_in(&footing, &trees, now);
    let blinding: Vec<crate::report::Workplace> =
        crate::report::blinding_in(&footing, &trees).into_iter().cloned().collect();
    // **깨진 것 전부다** — 판정을 가렸는지(`unknown`)가 아니라 그 파일이 깨졌는가다(moai-giz3).
    // `unknown` 도 함께 본다: 판 길은 둘을 같이 세우지만(`workplaces_in`), 그 짝을 손으로 맞추는
    // 자리가 하나라도 어긋나면 `blinding` 이 `all` 의 부분집합이라는 이 구조체의 약속이 깨져
    // `unreadable_worktrees` 만 서고 `broken_worktrees` 는 비는 화면이 난다.
    let all = trees.into_iter().filter(|t| t.broken || t.unknown).collect();
    (warning, Unread { all, blinding })
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
    Some(Disk { rel, all, admin, common })
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

/// 딸린 워크트리의 트래커에 대응하는 **주 워크트리의 트래커 자리** — 주 워크트리이거나 git 밖이면
/// `None`. git 을 띄우지 않는다.
///
/// [`tracker_root`] 가 이것으로 트래커를 루트로 옮긴다(moai-y7go) — 워크트리의 `.moai` 를 고치면
/// 병합에서 스냅샷이 충돌하기 때문이다. `crate::cmd::skill` 의 한국어 플러그인 셈도 같은 저장소의
/// 워크트리끼리를 한 자리로 읽는 데 이것을 쓴다.
/// 공용 디렉터리가 `.git` 이 아니면(맨 저장소에 딸린 워크트리) 주 체크아웃이 없다 — `None`.
pub fn main_root(root: &Path) -> Option<PathBuf> {
    let (top, common) = git_dirs(root)?;
    if top.join(".git").is_dir() || common.file_name()? != ".git" {
        return None;
    }
    // 둘 다 풀고 견준다 — [`same_repo`]·[`on_disk`] 와 같은 자다. `main` 은 이미 푼 경로라, 푸지 않은
    // 쪽의 조각을 붙이면 없는 자리가 선다.
    let rel = canonical(root).strip_prefix(canonical(top)).ok()?.to_path_buf();
    let main = common.parent()?;
    // 빈 `rel` 을 붙이면 끝에 `/` 가 선다 — 내미는 줄이 제 자리를 두 꼴로 쓰게 된다.
    Some(if rel.as_os_str().is_empty() { main.to_path_buf() } else { main.join(rel) })
}

/// [`workplaces`] 의 답을 바꿀 수 있는 파일과 **지금 잰** 표식 — git 을 띄우지 않는다.
///
/// 프로젝트 층(`tui::layer`)이 줄마다 걸음마다 잰다(moai-al0x). 층은 자리 판정을 요약에 싣는데
/// `.moai` 두 파일만 재면 워크트리를 치우거나 띄워도 다음 자동 갱신 전까지 옛 수를 낸다(옛
/// `SPC r`(다시 읽기)은 moai-en4u 가 걷었다). 드는 것:
/// - 공용 디렉터리의 `worktrees` — `git worktree add`·`remove`·`prune` 이 그 목록을 바꾼다
/// - 딸린 워크트리마다 `HEAD` — 가지 이름이 곧 이름 후보다([`names`])
/// - 딸린 워크트리의 `.git` — 제거 명령 없이 `rm -rf` 로 치운 것은 이것만 사라진다([`gather`] 와 같은 까닭)
/// - 딸린 워크트리의 스냅샷 — 이름으로 안 잡히는 집은 줄이 있으면 판다
///
/// 스냅샷은 **안 팔 때도** 잰다 — 파는지는 줄이 정하고(`report::claimed`), 줄이 바뀌면 `.moai` 표식이
/// 이미 다시 읽게 한다. 재는 것은 `stat` 과 작은 파일 읽기뿐이라 워크트리 수에 비례해도 싸다.
/// 딸린 워크트리가 뿌리면 비어 있다 — 겹쳐 보지 않는 [`workplaces`] 는 그 자리를 안 잰다.
pub fn place_marks(root: &Path) -> Vec<(PathBuf, crate::store::Stamp)> {
    if is_linked(root) {
        return Vec::new();
    }
    let Some((_, common)) = git_dirs(root) else { return Vec::new() };
    let worktrees = common.join("worktrees");
    // 목록을 읽기 **전에** 잰다 — 읽고 나서 재면 그 사이에 생긴 워크트리를 놓친다([`heads`] 와 같다).
    let mut out = vec![(worktrees.clone(), crate::store::stamp(&worktrees))];
    // **목록은 [`on_disk`] 하나로 읽는다** — 자리 판정이 보는 그 목록이다(리뷰 moai-3lul.kt0).
    // 여기서 따로 훑으면 `gitdir` 를 푸는 법·거르는 법이 두 벌이 되어, 한쪽만 고쳐질 때 층이
    // "바뀐 것 없다" 로 서고 판정만 달라진다(그 풀이는 moai-23ky 에서 한 번 고쳐진 자리다).
    // 경로가 사라진 워크트리는 `on_disk` 가 거르므로, 치우면 이 목록이 짧아져 그 자체가 바뀜이다.
    let Some(disk) = on_disk(root) else { return out };
    for (tree, linked, _) in &disk.all {
        if !linked {
            continue;
        }
        // 가지 이름이 곧 이름 후보다([`names`]) — HEAD 가 움직이면 자리 판정의 답이 바뀐다.
        if let Some(head) = disk.admin.get(&tree.path).map(|dir| dir.join("HEAD")) {
            out.push((head.clone(), crate::store::stamp(&head)));
        }
        let snapshot = tree.path.join(&disk.rel).join(".moai").join("issues.jsonl");
        out.push((snapshot.clone(), crate::store::stamp(&snapshot)));
        // **딸린 워크트리의 `.git`(`gitdir:` 한 줄)도 든다**([`gather`] 와 같은 까닭). 제거 명령
        // 없이 디렉터리째 치우면 git 이 적어 둔 `worktrees/<이름>` 은 그대로라 위의 둘이 안
        // 움직이고, 목록이 짧아진 것은 **목록째 견주는 쪽**(`layer::Marks`)만 본다 — 경로마다
        // 표식을 견주는 쪽(`App::follow`)은 이 파일이 사라지는 것으로 안다.
        let dot_git = tree.path.join(".git");
        out.push((dot_git.clone(), crate::store::stamp(&dot_git)));
    }
    out
}

/// 워크트리 꼭대기와 공용 git 디렉터리 — git 이 적어 둔 파일로만 읽는다([`on_disk`]).
///
/// 주 워크트리면 공용 디렉터리는 `.git` 그대로(풀지 않는다 — 끝 이름으로 주 워크트리를 알아본다),
/// 딸린 워크트리면 `gitdir:` 가 가리킨 곳의 `commondir` 를 푼 것이다.
fn git_dirs(root: &Path) -> Option<(&Path, PathBuf)> {
    let (top, own, linked) = own_git(root)?;
    if !linked {
        return Some((top, own));
    }
    let up = std::fs::read_to_string(own.join("commondir")).ok()?;
    // `commondir` 는 대개 `../..` 다 — 풀지 않으면 끝 이름이 `..` 라 주 워크트리를 못 알아본다.
    Some((top, canonical(&own.join(up.trim_end()))))
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
    matches!((tracker_place(a), tracker_place(b)), (Some(x), Some(y)) if x == y)
}

/// 이 트래커가 선 **저장소 안의 자리** — (공용 git 디렉터리, 워크트리 꼭대기에서 moai 뿌리까지). 같은
/// 저장소의 어느 워크트리에서 재도 같은 자리의 트래커면 같은 값이다([`same_repo`]). git 밖이면 없다.
/// git 을 띄우지 않는다.
///
/// 훅이 세션의 집기를 적는 자리도 이것으로 잡는다(`cmd::hook`) — main 워크트리의 뿌리로 잡던 판은 main
/// 이 없는 맨몸 저장소에서 워크트리마다 딴 자리에 적어 서로의 집기를 못 봤다(리뷰 moai-3k2d.1df).
/// `--separate-git-dir` 로 뜬 main 은 공용 디렉터리를 못 찾아 여전히 없다([`git_dirs`]) — 그 자리는
/// 제 뿌리로 적고, 기록이 갈리면 전처럼 판정한다.
pub fn tracker_place(root: &Path) -> Option<(PathBuf, PathBuf)> {
    let (top, common) = git_dirs(root)?;
    let rel = canonical(root).strip_prefix(canonical(top)).map(Path::to_path_buf).ok()?;
    Some((canonical(&common), rel))
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

/// 옆 HEAD → 그 HEAD 와 갈라진 자리의 스냅샷([`base_of`]). 한 번 겹치는 동안만 든다 — 그 사이에
/// 제 HEAD 는 하나다.
type Bases = std::collections::HashMap<String, BTreeMap<String, String>>;

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

fn git(root: &Path, args: &[&str]) -> Result<String, Trouble> {
    use crate::git::Error;
    crate::git::run(root, args).map_err(|e| {
        let (lost, why) = match e {
            Error::Spawn(e) => (Lost::NoGit, e.to_string()),
            Error::Failed(err) => (Lost::Failed, err),
            Error::Stream(e) => (Lost::Stream, e.to_string()),
            Error::NotUtf8(e) => (Lost::Encoding, e.to_string()),
        };
        Trouble::Unfound { lost, why }
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

    /// **건네받은 바닥은 그 파일 하나에만 선다**(moai-mafv). `MOAI_HERE=1` 은 부르는 쪽의 트래커를
    /// 그 워크트리의 `.moai` 로 가르는데, 자리 셈의 `base` 는 언제나 main 의 것이다 — 자리를 안 보고
    /// 쓰면 갈라질 때의 main 으로 지금 main 을 재어 옆이 만진 흔적이 통째로 뒤집힌다.
    #[test]
    fn a_handed_floor_stands_for_the_one_file_it_was_measured_from() {
        let main = PathBuf::from("/nowhere/repo/.moai/issues.jsonl");
        let here = Path::new("/nowhere/repo/.claude/worktrees/w/.moai/issues.jsonl");
        let floor = Floor::of(main.clone(), &[]);
        assert!(dug(&[], &floor).floor(&main).is_some(), "같은 파일인데 판 것을 안 썼다");
        assert!(dug(&[], &floor).floor(here).is_none(), "MOAI_HERE 로 갈린 파일을 main 의 바닥으로 썼다");
        // 자리 없는 바닥(훅)은 어떤 파일에도 안 선다 — 실수로 실려도 자리 셈이 안 집어 든다.
        assert!(dug(&[], &Floor::loose(&[])).floor(&main).is_none(), "자리 없는 바닥이 남의 파일에 섰다");
        assert!(Dug::new().floor(&main).is_none(), "아무것도 안 건네받았는데 바닥이 섰다");
    }

    /// **"옆이 여기보다 늦게 만졌는가" 를 재는 자는 하나다**([`Floor::later`]) — 여기에 없는 줄,
    /// 늦게 고친 줄, 늦게 미룬 줄이 든다. 미룸은 칸을 안 옮기므로 `updated_at` 만 보면 옆에서
    /// 미룬 것이 여기서 먼저 집은 것에 가려진다(moai-l11z).
    #[test]
    fn the_floor_names_only_lines_touched_later_there() {
        let at = "2026-09-12T00:00:00Z";
        let later = "2026-09-13T00:00:00Z";
        let floor = Floor::of(PathBuf::new(), &[issue("m-0001", "todo", at), issue("m-0002", "todo", at)]);
        assert!(!floor.later(&issue("m-0001", "todo", at)), "그대로인 줄이 만진 흔적이 됐다");
        assert!(floor.later(&issue("m-0001", "in_progress", later)), "옆에서 늦게 만진 줄을 안 세웠다");
        assert!(floor.later(&issue("m-0003", "todo", at)), "옆에서 세운 줄을 안 세웠다");
        let mut put_off = issue("m-0002", "todo", at);
        put_off.planned_at = Some(later.into());
        assert!(floor.later(&put_off), "옆에서 늦게 미룬 줄을 안 세웠다 — 미룸은 칸을 안 옮긴다");
    }

    /// **파는 길은 건네받은 바닥으로 잰다 — 그 파일을 다시 안 연다**(moai-mafv). 옆 스냅샷을 걷은
    /// 자리(moai-kos1)에 제 것이 남아, `status`·탐색기가 걸음마다 저장소에서 가장 큰 파일을 두 벌씩
    /// 풀었다. **재는 자가 한 자리에 서 있는가**를 두 쪽에서 본다 — 건네받은 자리가 자리 셈의
    /// `base` 와 같으면 그것으로 재고(디스크의 값과 답이 갈리게 두어 확인한다), 갈리면 예전처럼
    /// 제 손으로 판다.
    #[test]
    fn the_dug_path_measures_against_the_handed_floor_and_digs_again_when_it_is_elsewhere() {
        let s = crate::scratch::Scratch::new("floor");
        let main = s.path().join("main");
        let side = s.path().join("wt");
        let admin = main.join(".git").join("worktrees").join("w1");
        std::fs::create_dir_all(&admin).unwrap();
        std::fs::create_dir_all(main.join(".moai")).unwrap();
        std::fs::create_dir_all(side.join(".moai")).unwrap();
        std::fs::write(main.join(".git").join("HEAD"), "ref: refs/heads/develop\n").unwrap();
        std::fs::write(admin.join("HEAD"), "ref: refs/heads/worktree-w1\n").unwrap();
        std::fs::write(admin.join("gitdir"), format!("{}\n", side.join(".git").display())).unwrap();
        std::fs::write(side.join(".git"), format!("gitdir: {}\n", admin.display())).unwrap();
        let snapshot = |root: &Path, issues: &[Issue]| {
            let text: String = issues.iter().map(|i| serde_json::to_string(i).unwrap() + "\n").collect();
            let path = root.join(".moai").join("issues.jsonl");
            std::fs::write(&path, text).unwrap();
            path
        };
        let early = "2026-09-12T00:00:00Z";
        let touched = "2026-09-13T00:00:00Z";
        let late = "2026-09-14T00:00:00Z";
        // 옆은 `m-0001` 을 `touched` 에 만졌고, 디스크의 main 은 그보다 **늦다** — 그대로 읽으면
        // 옆이 만진 것이 아니다. 집은 줄은 이름으로 안 잡혀(`w1`·`wt`) 파는 길이 열린다.
        snapshot(&side, &[issue("m-0001", "todo", touched)]);
        let at = snapshot(&main, &[issue("m-0001", "todo", late)]);
        let asked = vec![issue("m-0001", "in_progress", late)];
        let cfg = crate::config::Config::parse("prefix = \"m\"\n").unwrap();
        let dig = |floor: &Floor| {
            let trees = workplaces_in(&main, false, &crate::report::Footing::of(&asked, &cfg), &dug(&[], floor));
            assert_eq!(trees.len(), 1, "딸린 워크트리 하나를 못 찾았다");
            trees[0].touched.clone()
        };
        // 건네받은 바닥은 그 줄을 `early` 로 든다 — 디스크를 다시 팠으면 답이 갈린다.
        let handed = Floor::of(at, &[issue("m-0001", "todo", early)]);
        assert_eq!(dig(&handed), ["m-0001".to_string()].into(), "건네받은 바닥을 두고 그 파일을 다시 팠다");
        // **철자가 달라도 같은 파일이면 쓴다**(`Dug::floor` 의 `canonical`) — 부르는 쪽이 든 자리는
        // `Repo` 가 찾아 오른 경로고 자리 셈의 것은 git 이 적어 둔 목록에서 지은 경로라, 같은 파일이
        // 글자만 다르게 올 수 있다. 정규화를 걷으면 이 줄이 먼저 붉어진다.
        let spelt = main.join(".moai").join("..").join(".moai").join("issues.jsonl");
        let handed = Floor::of(spelt, &[issue("m-0001", "todo", early)]);
        assert_eq!(dig(&handed), ["m-0001".to_string()].into(), "같은 파일을 글자가 다르다고 다시 팠다");
        // 자리가 갈리면(`MOAI_HERE=1`) 안 쓴다 — 디스크의 `late` 로 재어 만진 흔적이 없다.
        let elsewhere = Floor::of(side.join(".moai").join("issues.jsonl"), &[issue("m-0001", "todo", early)]);
        assert_eq!(dig(&elsewhere), BTreeSet::new(), "남의 파일에서 잰 바닥으로 main 을 쟀다");
    }

    /// **자리 경로는 늘 상대 경로다**(moai-xpd7) — 밑이면 잘라서, 밖이면 `../` 로 올라가서, 같은
    /// 자리면 `.`.
    #[test]
    fn from_top_is_always_relative() {
        let top = Path::new("/nowhere/work/main");
        assert_eq!(
            from_top(top, Path::new("/nowhere/work/main/.claude/worktrees/a")),
            PathBuf::from(".claude/worktrees/a")
        );
        assert_eq!(from_top(top, Path::new("/nowhere/work/feat")), PathBuf::from("../feat"));
        assert_eq!(from_top(top, Path::new("/nowhere/other/x")), PathBuf::from("../../other/x"));
        assert_eq!(from_top(top, top), PathBuf::from("."));
    }

    fn issue(id: &str, status: &str, updated: &str) -> Issue {
        let mut i =
            Issue::new(id.into(), format!("제목 {id}"), Kind::Issue, Status::new(status), "2026-09-10T00:00:00Z");
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
        let mut side = tree(
            "feat/x",
            vec![issue("m-0001", "todo", at), issue("m-0002", "in_progress", later), issue("m-0003", "todo", at)],
        );
        side.base = [("m-0001".to_string(), at.to_string()), ("m-0002".to_string(), at.to_string())].into();
        let (shown, origin) = overlay(vec![], &[side]);
        let ids: Vec<&str> = shown.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["m-0002", "m-0003"], "지운 줄이 되살았거나 옆의 작업이 사라졌다");
        assert_eq!(origin.branch("m-0001"), None);
        assert_eq!(origin.unreadable([Some("m-0001")].into_iter()), [Some("m-0001")]);

        // 둘째 워크트리가 같은 줄을 그 뒤에 만졌으면 그 줄은 선다.
        let mut quiet = tree("a", vec![issue("m-0001", "todo", at)]);
        quiet.base = [("m-0001".to_string(), at.to_string())].into();
        let mut busy = tree("b", vec![issue("m-0001", "review", later)]);
        busy.base = quiet.base.clone();
        let (shown, origin) = overlay(vec![], &[quiet, busy]);
        assert_eq!(shown.len(), 1);
        assert_eq!(origin.branch("m-0001"), Some("b"));
    }

    /// **그 이슈를 쥔 옆 워크트리를 찾는다**(moai-nxt4) — 줄이 어디서 왔는지와 따로다. 집기를 main 에
    /// 커밋하면 양쪽 줄이 같아 출처는 안 서는데, 옆에서 그 일을 쥐고 있다는 것은 여전히 보여야 한다.
    /// **훅과 같은 자**([`names`])로 가른다 — 후보가 통째로 같아야 쥔 것이다(moai-nxt4 리뷰).
    #[test]
    fn a_sibling_worktree_holding_the_issue_is_found() {
        let (_, origin) = overlay(
            vec![issue("moai-3fnf", "in_progress", "2026-09-15T00:00:00Z")],
            &[tree("worktree-moai-3fnf", vec![]), tree("feat/moai-9xyz-따로", vec![])],
        );
        assert_eq!(origin.working("moai-3fnf"), Some("worktree-moai-3fnf"), "`worktree-` 를 뗀 이름이 안 걸렸다");
        assert_eq!(origin.branch("moai-3fnf"), None, "제 줄인데 출처가 붙었다");
        // 이름 **통째로** 같아야 한다 — 훅(`away`·`hook::held`)과 같은 자다.
        assert_eq!(origin.working("moai-9xyz"), None, "id 가 이름의 한 토막일 뿐인데 쥐었다고 했다");
        assert_eq!(origin.working("moai-3fn"), None, "id 의 앞토막이 남의 가지에 걸렸다");
        // 자식의 워크트리는 부모 줄에 안 붙는다 — 부모가 제가 집힌 줄 안다.
        let (_, child) = overlay(vec![], &[tree("worktree-moai-3fnf.abc", vec![])]);
        assert_eq!(child.working("moai-3fnf"), None);
        assert_eq!(child.working("moai-3fnf.abc"), Some("worktree-moai-3fnf.abc"));
        // 가지 이름 그대로도 후보다 — `-b` 없이 띄운 워크트리.
        let (_, plain) = overlay(vec![], &[tree("moai-3fnf", vec![])]);
        assert_eq!(plain.working("moai-3fnf"), Some("moai-3fnf"));
    }

    #[test]
    fn the_latest_line_wins_and_names_where_it_came_from() {
        let mine =
            vec![issue("m-0001", "todo", "2026-09-12T00:00:00Z"), issue("m-0002", "todo", "2026-09-12T00:00:00Z")];
        let (shown, origin) = overlay(
            mine,
            &[tree(
                "feat/x",
                vec![
                    issue("m-0001", "in_progress", "2026-09-13T00:00:00Z"),
                    issue("m-0002", "done", "2026-09-11T00:00:00Z"),
                ],
            )],
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
        let (shown, origin) = overlay(vec![edited], &[tree("feat/x", vec![picked])]);
        assert_eq!(shown[0].status.as_str(), "in_progress", "늦은 필드 편집이 옆에서 집은 것을 풀었다");
        assert_eq!(origin.branch("m-0001"), Some("feat/x"));

        // 거꾸로 — 여기서 늦게 옮겼으면 옆의 늦은 필드 편집이 덮지 못한다.
        let mut moved = issue("m-0002", "done", "2026-09-12T00:00:00Z");
        moved.status_since = "2026-09-12T00:00:00Z".into();
        let theirs = issue("m-0002", "todo", "2026-09-13T00:00:00Z");
        let (shown, origin) = overlay(vec![moved], &[tree("feat/x", vec![theirs])]);
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
        let (shown, origin) = overlay(vec![picked], &[tree("feat/x", vec![shelved.clone()])]);
        assert!(shown[0].is_deferred(), "옆에서 늦게 미룬 것이 여기서 먼저 집은 줄에 가려졌다");
        assert_eq!(origin.branch("m-0001"), Some("feat/x"));

        // 도로 집으면 `deferred_at` 은 사라져도 `planned_at` 이 늦어 이긴다. 여기 줄은 칸을
        // 도로 집은 줄보다 늦게 옮겼다 — 칸 시각만 보면 여기 미룬 줄이 선다.
        let mut here = shelved.clone();
        here.status_since = t2.into();
        let mut back = issue("m-0001", "todo", t3);
        back.planned_at = Some(t3.into());
        let (shown, origin) = overlay(vec![here], &[tree("feat/x", vec![back])]);
        assert!(!shown[0].is_deferred(), "옆에서 도로 집은 것이 여기서 미룬 줄에 가려졌다");
        assert_eq!(origin.branch("m-0001"), Some("feat/x"));

        // 미룬 뒤에 칸을 옮긴 줄은 `planned_at` 이 낡았어도 이긴다 — 옛 바이너리는 칸만 옮긴다.
        let mut moved = shelved.clone();
        moved.status = Status::new("review");
        moved.status_since = t4.into();
        let mut later_shelf = issue("m-0001", "todo", t3);
        later_shelf.deferred_at = Some(t3.into());
        later_shelf.planned_at = Some(t3.into());
        let (shown, origin) = overlay(vec![later_shelf], &[tree("feat/x", vec![moved])]);
        assert_eq!(shown[0].status.as_str(), "review", "늦게 옮긴 칸이 그 전의 미룸에 졌다");
        assert_eq!(origin.branch("m-0001"), Some("feat/x"));
    }

    /// **임시 자리가 어느 체크아웃 안이어도 그 저장소의 워크트리가 새지 않는다**(moai-46xz).
    ///
    /// 파일로 읽는 반쪽은 `Path::ancestors()` 로 `.git` 을 찾아 올라간다. 컨테이너나 CI 에서
    /// `TMPDIR` 이 체크아웃 밑이면 시험의 임시 자리 위에 그 체크아웃이 서고, 만들지도 않은
    /// 워크트리 이름이 답에 섞인다 — 환경 변수를 걷어서는 못 막는 길이다. 그래서 임시 자리에
    /// 아무것도 없는 저장소를 울타리로 세운다([`Scratch::fenced`]).
    ///
    /// **git 으로 읽는 길도 울타리에서 선다**(`heads`·`gather`). 빈 `.git` 디렉터리만 두면 git 은 그것을
    /// 지나쳐 위의 체크아웃을 잡는다 — 파일로 읽는 반쪽만 보면 그 틈이 안 드러난다.
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
        assert!(away(dir.path()).names.is_empty(), "울타리 위의 저장소가 새어 나왔다 — {:?}", away(dir.path()));
        assert!(away(&dir.join("nowhere")).names.is_empty(), "없는 자리에서도 위의 저장소를 읽었다");
        assert!(!is_linked(dir.path()), "울타리를 딸린 워크트리로 읽었다");
        let git_top =
            crate::git::run(dir.path(), &["rev-parse", "--show-toplevel"]).map(|t| PathBuf::from(t.trim_end()));
        assert_eq!(git_top.ok(), Some(canonical(dir.path())), "git 이 울타리를 지나쳐 위의 저장소를 잡았다");
    }

    /// **딸린 워크트리의 트래커는 주 워크트리의 같은 자리로 옮겨 간다**(moai-y7go) — 하위 디렉터리의
    /// 트래커도 그 자리째. 주 워크트리와 git 밖은 옮길 곳이 없다. **옮길 곳에 설정이 있어야 옮긴다** —
    /// 무시되는 `.moai/lock` 하나만 남은 루트는 트래커가 아니다(리뷰 moai-71ht 셋째 판).
    #[test]
    fn a_linked_tracker_points_at_the_main_one() {
        let scratch = crate::scratch::Scratch::fenced("main-root");
        let base = canonical(scratch.path());
        let main = base.join("main");
        std::fs::create_dir_all(main.join("sub")).unwrap();
        let run = |dir: &Path, args: &[&str]| {
            let out = crate::git::isolated(dir).args(args).output().unwrap();
            assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        };
        run(&main, &["init", "-q"]);
        run(&main, &["commit", "-q", "--allow-empty", "-m", "a"]);
        run(&main, &["worktree", "add", "-q", "../feat", "-b", "feat/x"]);
        let feat = base.join("feat");
        std::fs::create_dir_all(feat.join("sub")).unwrap();

        assert_eq!(main_root(&feat), Some(main.clone()));
        assert_eq!(main_root(&feat.join("sub")), Some(main.join("sub")), "하위 트래커의 자리를 잃었다");
        assert_eq!(main_root(&main), None, "주 워크트리를 딸린 것으로 읽었다");
        assert_eq!(main_root(&main.join("sub")), None);
        assert_eq!(main_root(&base), None, "git 밖에서 지어냈다");

        // **락만 남은 `.moai` 는 트래커가 아니다** — 옛 커밋을 체크아웃하면 추적하는 파일은 사라지고
        // 무시되는 `lock` 만 남는데, 그것을 트래커로 읽으면 이 저장소의 모든 워크트리가 "설정이 없다"
        // 로 넘어진다(리뷰 moai-71ht 셋째 판).
        std::fs::create_dir_all(main.join(".moai")).unwrap();
        std::fs::write(main.join(".moai/lock"), "").unwrap();
        assert_eq!(tracker_root(&feat), None, "락만 남은 자리로 옮겼다");
        std::fs::write(main.join(".moai/config.toml"), "prefix = \"t\"\n").unwrap();
        assert_eq!(tracker_root(&feat), Some(main.clone()), "설정이 있는데 안 옮겼다");
        assert_eq!(tracker_root(&main), None, "주 워크트리를 옮겼다");
    }

    /// **자리 판정을 바꾸는 것은 층의 표식도 바꾼다**(moai-al0x) — 워크트리를 띄우거나, 가지를
    /// 옮기거나, 옆 스냅샷을 쓰거나, 제거 명령 없이 디렉터리째 치우는 것. 아무것도 안 하면 그대로다.
    #[test]
    fn place_marks_move_when_a_worktree_comes_goes_or_writes() {
        let scratch = crate::scratch::Scratch::fenced("place-marks");
        let base = scratch.path().to_path_buf();
        let main = base.join("main");
        std::fs::create_dir_all(&main).unwrap();
        let run = |dir: &Path, args: &[&str]| crate::git::tests::run_git(dir, None, args);
        run(&main, &["init", "-q"]);
        run(&main, &["commit", "-q", "--allow-empty", "-m", "a"]);
        let changed = |was: &[(PathBuf, crate::store::Stamp)]| place_marks(&main) != was;

        let seen = place_marks(&main);
        assert!(!changed(&seen), "아무것도 안 했는데 표식이 바뀌었다");
        run(&main, &["worktree", "add", "-q", "../t-1", "-b", "worktree-t-1"]);
        assert!(changed(&seen), "새로 띄운 워크트리를 못 알아챈다");

        let seen = place_marks(&main);
        std::fs::create_dir_all(base.join("t-1/.moai")).unwrap();
        std::fs::write(base.join("t-1/.moai/issues.jsonl"), "").unwrap();
        assert!(changed(&seen), "옆 워크트리의 스냅샷이 생긴 것을 못 알아챈다");

        let seen = place_marks(&main);
        run(&base.join("t-1"), &["checkout", "-q", "-b", "other"]);
        assert!(changed(&seen), "옆 워크트리가 가지를 옮긴 것을 못 알아챈다 — 이름 후보가 바뀐다");

        let seen = place_marks(&main);
        std::fs::remove_dir_all(base.join("t-1")).unwrap();
        assert!(changed(&seen), "디렉터리째 치운 워크트리를 못 알아챈다");

        // 딸린 워크트리를 뿌리로 두면 재지 않는다 — 겹쳐 보지 않는 자리 판정이 그 자리를 안 본다.
        run(&main, &["worktree", "add", "-q", "../t-2", "-b", "worktree-t-2"]);
        assert!(place_marks(&base.join("t-2")).is_empty());
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
            assert_eq!(away(at).names, by_git(at), "{} 에서 파일로 읽은 목록이 git 과 다르다", at.display());
        }
        assert!(!away(&main).names.contains("gone"), "사라진 워크트리를 이름으로 댄다");
        assert!(away(&base.join("nowhere")).names.is_empty());

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
        assert!(
            seen.iter().any(|(p, s)| p.ends_with("refs/heads/more") && s.is_some()),
            "하위에서 가지 파일을 못 찾는다 — {seen:#?}"
        );
    }

    /// **스냅샷을 못 겹친 옆 워크트리도 이름으로는 쥔다**(moai-ncsf) — 훅의 [`away`] 는 디스크의
    /// 옆 워크트리 전부에서 이름을 내므로, 겹친 곳만 보면 훅은 "옆이 쥐었다" 로 아는 줄에 화면만
    /// `⎇` 를 안 단다. moai 를 들이기 전에 갈라진 워크트리(스냅샷이 없다)와 스냅샷이 깨진 워크트리
    /// 둘 다다. 겹쳐 본 곳(`labels`)에는 안 든다. **디렉터리가 사라진 잠근 워크트리는 안 든다** —
    /// git 은 목록에 남기지만 훅의 자(`on_disk`)는 거기서 거른다.
    #[test]
    fn a_sibling_without_a_readable_snapshot_still_names_what_it_holds() {
        let scratch = crate::scratch::Scratch::fenced("unread-names");
        let base = scratch.path().to_path_buf();
        let main = base.join("main");
        std::fs::create_dir_all(&main).unwrap();
        let run = |dir: &Path, args: &[&str]| crate::git::tests::run_git(dir, None, args);
        run(&main, &["init", "-q"]);
        run(&main, &["commit", "-q", "--allow-empty", "-m", "a"]);
        // moai 를 들이기 전에 갈라진 워크트리 — 스냅샷이 없다.
        run(&main, &["worktree", "add", "-q", "../t-1", "-b", "worktree-t-1"]);
        std::fs::create_dir_all(main.join(".moai")).unwrap();
        std::fs::write(main.join(".moai/issues.jsonl"), "").unwrap();
        std::fs::write(main.join(".moai/config.toml"), "prefix = \"t\"\n").unwrap();
        // 스냅샷이 못 읽히는 워크트리 — 파일 자리에 디렉터리가 섰다.
        run(&main, &["worktree", "add", "-q", "../t-2", "-b", "worktree-t-2"]);
        std::fs::create_dir_all(base.join("t-2/.moai/issues.jsonl")).unwrap();
        // 잠그고 디렉터리를 치운 워크트리 — git 은 잠근 것을 `prunable` 로 안 적어 목록에 남긴다.
        run(&main, &["worktree", "add", "-q", "../t-3", "-b", "worktree-t-3"]);
        run(&main, &["worktree", "lock", "../t-3"]);
        std::fs::remove_dir_all(base.join("t-3")).unwrap();

        let crate::store::Opened::Repo(repo) = Repo::open(&main, || crate::i18n::Lang::Ko).unwrap() else {
            panic!("저장소가 안 열렸다")
        };
        let got = gather(&repo, true).unwrap();
        assert_eq!(got.origin.working("t-1"), Some("worktree-t-1"), "스냅샷 없는 워크트리의 이름을 못 봤다");
        assert_eq!(got.origin.working("t-2"), Some("worktree-t-2"), "스냅샷이 깨진 워크트리의 이름을 못 봤다");
        assert!(got.origin.labels().is_empty(), "겹치지 않은 곳을 겹쳐 봤다고 댄다 — {:?}", got.origin.labels());
        // **어느 워크트리인지를 자료로 든다**(moai-dpbi) — 글은 `view::trouble_line` 이 편다.
        let said = |b: &str| got.trouble.iter().any(|t| matches!(t, Trouble::Unread { branch, .. } if branch == b));
        assert!(said("worktree-t-2"), "깨진 스냅샷을 말하지 않는다 — {:?}", got.trouble);
        for id in ["t-1", "t-2"] {
            assert!(away(&main).names.contains(id), "훅의 자가 {id} 를 안 센다 — 이 시험이 견줄 것이 없다");
        }
        assert!(!away(&main).names.contains("t-3"), "훅의 자가 사라진 워크트리를 센다 — 이 시험이 견줄 것이 없다");
        assert_eq!(got.origin.working("t-3"), None, "디렉터리가 사라진 워크트리의 이름을 훅과 달리 들었다");
        assert!(!got.origin.named_only().contains(&"worktree-t-3"), "{:?}", got.origin.named_only());

        // **이름만 든 워크트리를 `rm -rf` 로 치우면 표식이 움직인다**(moai-uyu9) — 안 움직이면
        // 탐색기는 다른 까닭으로 다시 읽을 때까지 사라진 곳의 `⎇` 를 든다.
        let moved = |seen: &[(PathBuf, crate::store::Stamp)]| seen.iter().any(|(p, s)| crate::store::stamp(p) != *s);
        assert!(!moved(&got.watched), "아무것도 안 했는데 표식이 움직였다");
        std::fs::remove_dir_all(base.join("t-1")).unwrap();
        assert!(moved(&got.watched), "스냅샷 없는 워크트리를 치운 것을 못 알아챈다");
        let got = gather(&repo, true).unwrap();
        assert_eq!(got.origin.working("t-1"), None, "치운 워크트리의 이름을 아직 든다");

        // 옆에서 본 주 워크트리의 `.git` 은 디렉터리다 — 커밋마다 바뀌니 지켜보지 않는다. 주
        // 워크트리의 스냅샷을 못 읽게 해 이름만 드는 길로 보낸다.
        std::fs::remove_dir_all(base.join("t-2/.moai")).unwrap();
        std::fs::create_dir_all(base.join("t-2/.moai")).unwrap();
        std::fs::write(base.join("t-2/.moai/issues.jsonl"), "").unwrap();
        std::fs::write(base.join("t-2/.moai/config.toml"), "prefix = \"t\"\n").unwrap();
        std::fs::remove_file(main.join(".moai/issues.jsonl")).unwrap();
        std::fs::create_dir_all(main.join(".moai/issues.jsonl")).unwrap();
        // **옆 워크트리에 뿌리내린 채로 연다** — `Repo::open` 은 이제 루트로 옮겨 가므로(moai-y7go)
        // 그것으로 열면 이 시험이 보려던 "옆에서 주 워크트리를 못 읽는 길" 이 아니라 주 워크트리
        // 그 자체가 열린다.
        let side = Repo::at(base.join("t-2"), crate::config::Config::load(&base.join("t-2")).unwrap());
        let got = gather(&side, true).unwrap();
        assert!(
            got.origin.named_only().iter().any(|l| !l.starts_with("worktree-")),
            "주 워크트리가 이름만 드는 길로 안 갔다 — {:?}",
            got.origin.named_only()
        );
        let dirs: Vec<_> = got.watched.iter().filter(|(p, _)| p.ends_with(".git") && p.is_dir()).collect();
        assert!(dirs.is_empty(), "주 워크트리의 .git 디렉터리를 지켜본다 — {dirs:#?}");
    }

    /// **옆 스냅샷을 팔지와 거기 벌여 놓인 줄은 자리 셈의 자로 잰다**(moai-es40, 에픽 끝 리뷰
    /// moai-r8gw.b4s) — 종류가 다른 쌍둥이에게 가려진 줄도 든다(`report::started`, 사용자 결정).
    /// `report::wip` 으로 재면 머지가 남긴 id 충돌 하나로 문이 닫히고 `holds` 에서도 빠져, 스냅샷으로만
    /// 찾을 수 있는 그 줄이 `stranded` 와 `show` 의 `자리 없다` 로 선다.
    #[test]
    fn an_eclipsed_picked_row_opens_and_fills_the_side_snapshot() {
        let scratch = crate::scratch::Scratch::fenced("eclipsed-holds");
        let base = scratch.path().to_path_buf();
        let main = base.join("main");
        std::fs::create_dir_all(&main).unwrap();
        let run = |dir: &Path, args: &[&str]| crate::git::tests::run_git(dir, None, args);
        run(&main, &["init", "-q"]);
        run(&main, &["commit", "-q", "--allow-empty", "-m", "a"]);
        // 이름이 id 가 아닌 워크트리(에이전트 격리) — 자리는 스냅샷으로만 갈린다.
        run(&main, &["worktree", "add", "-q", "../agent-x", "-b", "worktree-agent-x"]);
        // 두 스냅샷 모두 쌍둥이를 든다 — 옆이 develop 을 받았다.
        let rows = concat!(
            "{\"id\":\"t-0001\",\"title\":\"집힌 일\",\"status\":\"in_progress\",\"created_at\":\"2026-09-11T00:00:00Z\",\"updated_at\":\"2026-09-11T00:00:00Z\",\"status_since\":\"2026-09-11T00:00:00Z\"}\n",
            "{\"id\":\"t-0001\",\"kind\":\"epic\",\"title\":\"쌍둥이 에픽\",\"status\":\"todo\",\"created_at\":\"2026-09-11T00:00:00Z\",\"updated_at\":\"2026-09-11T00:00:00Z\",\"status_since\":\"2026-09-11T00:00:00Z\"}\n",
        );
        for dir in [main.clone(), base.join("agent-x")] {
            std::fs::create_dir_all(dir.join(".moai")).unwrap();
            std::fs::write(dir.join(".moai/issues.jsonl"), rows).unwrap();
            std::fs::write(dir.join(".moai/config.toml"), "prefix = \"t\"\n").unwrap();
        }
        let cfg = crate::config::Config::parse("prefix = \"t\"\n").unwrap();
        let mine = crate::store::read_snapshot(&main.join(".moai/issues.jsonl")).unwrap().unwrap().issues;
        assert!(crate::report::wip(&mine, &cfg).is_empty(), "가려진 줄이 집은 일로 섰다 — 이 시험이 견줄 것이 없다");

        let trees = workplaces(&main, &cfg, false, &mine);
        let side = trees.iter().find(|t| t.branch == "worktree-agent-x").expect("옆 워크트리가 없다");
        assert!(side.holds.contains("t-0001"), "가려진 집힌 줄을 옆 스냅샷에서 안 셌다 — {:?}", side.holds);
    }

    /// 동률이면 제 줄, 남끼리는 앞선 워크트리.
    #[test]
    fn a_tie_keeps_the_current_branch_then_the_earlier_worktree() {
        let at = "2026-09-13T00:00:00Z";
        let (shown, origin) = overlay(
            vec![issue("m-0001", "todo", at)],
            &[
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
            &[tree("feat/x", vec![issue("m-0001", "todo", "2026-09-01T00:00:00Z")])],
        );
        let ids: Vec<&str> = shown.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["m-0001", "m-0003"]);
        assert_eq!(origin.branch("m-0001"), Some("feat/x"));
    }

    /// 제 파일의 겹친 id 는 접지 않는다 — `duplicate_id` 가 사라지면 안 된다.
    #[test]
    fn duplicates_in_my_own_file_survive_the_overlay() {
        let at = "2026-09-12T00:00:00Z";
        let (shown, _) =
            overlay(vec![issue("m-0001", "todo", at), issue("m-0001", "review", at)], &[tree("feat/x", vec![])]);
        assert_eq!(shown.len(), 2);
        assert_eq!(shown[1].status.as_str(), "review", "읽은 차례가 뒤집혔다");

        // 옆의 더 새 줄은 **뒷줄을** 덮는다 — 상세·트리가 여는 줄이 그것이다.
        let later = "2026-09-13T00:00:00Z";
        let (shown, _) = overlay(
            vec![issue("m-0001", "todo", at), issue("m-0001", "review", at)],
            &[tree("feat/x", vec![issue("m-0001", "done", later)])],
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
            &[tree("feat/x", vec![issue("m-0001", "todo", at), issue("m-0002", "review", later)])],
        );
        assert_eq!(origin.unreadable([Some("m-0001")].into_iter()), [None]);
        assert_eq!(origin.unreadable([Some("m-0002")].into_iter()), [Some("m-0002")]);
        assert_eq!(origin.unreadable([Some("m-0001"), Some("m-0001")].into_iter()), [None, Some("m-0001")]);
        assert_eq!(origin.unreadable([None, Some("m-0009")].into_iter()), [None, Some("m-0009")]);
    }
}
