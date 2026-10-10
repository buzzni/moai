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
use crate::path::real;
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
    /// 를 뗀 이름). 훅이 옆의 일을 가르는 자와 **같은 자**다(moai-nxt4 리뷰). [`gather`] 는 그 워크트리의
    /// 집은 표식([`marks_of`])도 더한다(moai-jn4d.ewm).
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
    /// (이름, 그 워크트리의 moai 뿌리, 쥐었다고 볼 id 후보 — [`Side::holds`]). 뿌리는 이력(저널)을 읽을 때 쓴다.
    trees: Vec<(String, PathBuf, BTreeSet<String>)>,
    /// 제 파일에는 없고 옆에서만 온 줄. 제 줄을 **덮은** 것과 가른다 —
    /// [`Origin::unreadable`] 이 그 차이로 거짓 중복을 거른다.
    added: BTreeSet<String>,
    /// 스냅샷은 못 겹쳤지만 **이름은 아는** 옆 워크트리 — (이름, 쥐었다고 볼 id 후보: 이름과 집은 표식)(moai-ncsf).
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

    /// **그 이슈를 쥔 옆 워크트리**(moai-nxt4) — 그 워크트리의 이름 후보([`names`])나 집은 표식
    /// ([`marks_of`])에 id 가 들면. 규약대로 일감마다 그 id 로 가지를 띄우고 워크트리를 그 이름으로
    /// 만드므로, 이것이 곧 "누가 무엇을 쥐고 있나" 다.
    ///
    /// **집은 표식도 본다**(moai-jn4d.ewm) — 이름이 id 가 아닌 워크트리(에이전트 격리)가 그 안에서 집은
    /// 줄은 이름으로는 안 잡힌다. 훅([`held_elsewhere`])과 자리 셈([`workplaces`])이 그 표식을 읽는데
    /// 이 자만 안 보면 `moai show` 의 `자리` 는 그 워크트리를 대는데 탐색기 목록에는 `⎇` 가 없다.
    ///
    /// **훅이 읽는 두 가지를 다 본다**(moai-nxt4 리뷰) — 이름 후보는 `hook::held` 가 옆의 일을 초점에서
    /// 뺄 때 쓰는 [`away`] 와 같고, 표식은 훅이 "옆이 쥐었을 수 있다" 로 읽는 [`held_elsewhere`] 의 것이다
    /// (훅은 그 표식으로 막지 않고 풀기만 한다 — `hook::unsure`). 이름 후보를 안 맞추면 화면은 ⎇ 를 다는데
    /// 훅은 "옆이 쥐었다" 를 모르는 줄이 생긴다. 후보는 **통째로 같아야** 한다 — 자식 id(`부모.자식`)의 가지가
    /// 부모 줄에 붙지 않는다.
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
/// **옆을 빌려 받는다**(moai-kos1) — 겹쳐 세우는 줄만 베끼므로 베끼는
/// 수는 보통 몇 줄이다 — 옆 줄은 거의 다 이쪽에도 같은 값으로 있어 그냥 지나간다.
pub fn overlay(mine: Vec<Issue>, others: &[Side]) -> (Vec<Issue>, Origin) {
    let (shown, origin, _) = overlay_keeping(mine, others, false);
    (shown, origin)
}

/// [`overlay`] 에 **겹치기 전의 제 줄**을 곁들인다(moai-ug6x.pi3) — 옆에서 한 줄이라도 들어왔을 때만
/// `Some` 이고, 아무것도 안 들어왔으면 겹친 줄이 곧 제 줄이라 `None` 이다([`Gathered::root`]).
///
/// **처음 바꾸는 순간에 베낀다.** 늘 베끼면 옆이 조용한 보통의 걸음(집기를 main 에 커밋하는 규약에서는
/// 옆 줄이 거의 안 선다)이 쓰지도 않을 한 벌을 걸음마다 치른다. 바꾼 뒤에는 제 줄을 되찾을 길이 없어
/// — 덮인 줄은 사라지고 정렬이 자리를 섞는다 — 바꾸기 **전에** 베낀다.
///
/// **`keep` 이 아니면 안 베낀다** — 겹친 줄만 쓰는 쪽([`overlay`], 훅이 부르는 [`fresh`])은 `None` 을 받는다.
/// 거기서 베끼면 옆에서 줄이 들어올 때마다 버릴 한 벌을 치른다.
fn overlay_keeping(mine: Vec<Issue>, others: &[Side], keep: bool) -> (Vec<Issue>, Origin, Option<Vec<Issue>>) {
    let mut shown = mine;
    let mut before: Option<Vec<Issue>> = None;
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
                    if keep {
                        before.get_or_insert_with(|| shown.clone());
                    }
                    origin.from.insert(i.id.clone(), tree);
                    shown[k] = i.clone();
                }
                Some(_) => {}
                None if base.get(&i.id).is_some_and(|then| i.updated_at <= *then) => {}
                None => {
                    if keep {
                        before.get_or_insert_with(|| shown.clone());
                    }
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
    (shown, origin, before)
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
    /// 그 워크트리의 스냅샷을 **안 읽기로 했다**([`crate::held::Unheld`], moai-itsu) — 가지·그 스냅샷의 자리·까닭.
    /// 까닭을 글로 받던 판은 한국어 머리 뒤에 moai 가 지은 영어(`not a regular file`)가 붙었고, 밖을 가리키는
    /// 링크는 까닭 없이 `<자리> -> <끝>` 만 댔다(리뷰 moai-itsu.n8z).
    Unheld { branch: String, at: PathBuf, why: crate::held::Unheld },
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
    /// **겹치기 전의 제 스냅샷** — 옆에서 줄이 하나라도 들어왔을 때만 서고, 아니면 `None` 이라 `load` 가 곧
    /// 그것이다(moai-ug6x.pi3) — 읽는 쪽은 `root.as_ref().unwrap_or(&load)` 로 읽는다.
    ///
    /// 아카이브의 충돌과 옮길 수는 루트의 스냅샷과 견준다(moai-bth3) — 옆의 낡은 사본이 그 둘을 부풀리지 않게.
    /// 그 스냅샷을 `moai status --worktree` 와 탐색기의 배너가 저마다 `repo.read()` 로 **다시 풀었는데**, 그것은
    /// 바로 여기서 방금 판 그 파일이다(리뷰 moai-3nrh.ige 7번). 같은 읽기를 한 번만 하니 겹친 줄과 견줄 줄이 한
    /// 때의 파일에서 온다 — 다시 읽던 판은 그 사이에 떨어진 쓰기를 한쪽만 봤다.
    pub root: Option<Load>,
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
}

/// 제 저장소를 읽고, `worktree` 면 다른 워크트리의 스냅샷을 겹친다.
///
/// **읽기는 관대하다.** git 이 없거나, 저장소가 아니거나, 남의 파일이 깨졌어도
/// 제 스냅샷은 그대로 낸다 — 그런 것은 `trouble` 로 말만 한다. 스냅샷 파일이
/// 없는 워크트리는 moai 를 들이기 전에 갈라진 브랜치라 **말하지도 않는다.**
pub fn gather(repo: &Repo, worktree: bool) -> crate::fail::R<Gathered> {
    gather_with(repo, worktree, true)
}

/// 옆을 겹치되 **집은 표식은 안 읽고 제 워크트리도 빼지 않는** [`gather`] — 겹친 줄의 id 와
/// `trouble`·`unfound` 만 쓰는 쪽(`moai read` 의 걷기가 지킬 id, `cmd::read::keep_for_prune`)이 부른다.
/// 표식은 [`Origin`] 의 `⎇` 이름에만 들고 겹치는 줄은 안 바꾸니, 그쪽이 옆 워크트리마다 표식 파일을 여는
/// 값을 치를 까닭이 없다. 그래서 여기서 낸 [`Origin::working`] 은 이름으로만 답한다 — 화면에 넘기지 않는다.
///
/// **제 워크트리를 빼지 않는다**(리뷰 moai-jn4d.adc) — 걷기가 지킬 id 는 어느 자리에서 연 화면이든 도장을
/// 찍을 수 있는 줄 전부다. 루트에서 연 탐색기는 이 워크트리를 옆으로 겹쳐 거기에만 있는 줄에 도장을 찍는데,
/// 여기서 그 사본을 빼면 이 워크트리에서 친 `moai read` 한 번이 그 도장을 조용히 걷는다.
pub fn gather_unmarked(repo: &Repo) -> crate::fail::R<Gathered> {
    gather_with(repo, true, false)
}

/// [`gather`] 와 [`gather_unmarked`] 의 몸 — `screen` 이 거짓이면 [`marks_of`] 를 안 부르고, 부른 자리의
/// 워크트리([`callers`])도 옆으로 그대로 겹친다.
fn gather_with(repo: &Repo, worktree: bool, screen: bool) -> crate::fail::R<Gathered> {
    let load = repo.read()?;
    if !worktree {
        return Ok(Gathered {
            load,
            root: None,
            origin: Origin::default(),
            trouble: Vec::new(),
            unfound: None,
            swept: false,
            watched: Vec::new(),
        });
    }
    let mut trouble = Vec::new();
    let mut unfound = None;
    let mut others = Vec::new();
    // 스냅샷을 못 겹친 옆 워크트리의 이름과 집은 표식 — 그래도 [`Origin::working`] 이 본다.
    let mut named = Vec::new();
    // HEAD 가 움직인 것도 다시 읽을 까닭이다 — **`others_of` 가 HEAD 를 읽기 전에** 잰다.
    let mut watched = heads(&repo.root);
    // **부른 자리가 딸린 워크트리면 그 워크트리는 옆이 아니다**(moai-jn4d.adc) — 까닭은 [`callers`].
    let callers = if screen { callers(repo) } else { Vec::new() };
    match others_of(&repo.root) {
        Err(why) => unfound = Some(why),
        Ok((me, trees)) => {
            let head = head_of(me.as_ref());
            let here: std::collections::HashSet<&str> = load.issues.iter().map(|i| i.id.as_str()).collect();
            let mut bases = Bases::new();
            for (tree, root) in trees {
                // 제 워크트리는 겹치지도, 이름을 들지도, 표식을 읽지도 않는다([`callers`]). **깨진 것은 그대로
                // 말한다**(리뷰) — 그 사본을 실제로 풀어 보는 자리는 여기뿐이고(자리 셈의 값싼 문
                // [`unreadable_snapshot`] 은 첫 바이트만 본다), `swept` 인 판의 `status` 는 그 말을 여기에
                // 맡겨 제 목록을 접는다. 말을 빼면 고칠 사람인 바로 그 세션에게만 아무 데서도 안 선다.
                let own = !callers.is_empty() && callers.contains(&real(&tree.path));
                // **마일스톤 워크트리는 겹치지 않는다**([`is_milestone`]). 트래커를 거기 쓰지 않는 것이
                // 규약이라 그 스냅샷은 갈라질 때(또는 develop 을 받을 때)의 사본뿐이다 — 겹쳐서 보탤 수 있는
                // 것은 낡은 줄밖에 없고, 그 줄이 루트보다 늦어 보이는 판(그 자리에서 `MOAI_HERE=1` 로 쓴
                // 것, 릴리스 머지가 남긴 줄)에는 남의 산 집기처럼 선다. 이름도 안 든다 — 아무 줄을 못 가리킨다.
                // 가지가 바뀌면 [`heads`] 가 지켜보는 HEAD 가 움직여 다시 읽는다.
                if is_milestone(&tree) {
                    continue;
                }
                let path = root.join(".moai").join("issues.jsonl");
                watched.push((path.clone(), crate::store::stamp(&path)));
                // **집은 표식도 그 워크트리가 쥔 것으로 센다**(moai-jn4d.ewm) — 자리 셈([`workplaces`])과
                // 훅([`held_elsewhere`])이 읽는 그 표식이다. 이름이 id 가 아닌 워크트리(에이전트 격리)가
                // 집은 줄은 이것으로만 `⎇` 를 단다. 표식 파일은 지켜보지 않는다 — 그것을 고치는 것은
                // 시작 칸을 드나드는 `moai mv` 뿐이고, 그 쓰기가 이미 지켜보는 루트의 트래커를 바꾼다.
                let marked = if screen && !own { marks_of(&tree) } else { BTreeSet::new() };
                match crate::store::read_snapshot(&root) {
                    unread @ (Err(_) | Ok(None)) => {
                        match unread {
                            Err(crate::store::Unsnapped::Held { at, why }) => {
                                trouble.push(Trouble::Unheld { branch: tree.label.clone(), at, why });
                            }
                            Err(crate::store::Unsnapped::Failed(e)) => {
                                trouble.push(Trouble::Unread { branch: tree.label.clone(), why: e.to_string() });
                            }
                            Ok(_) => {}
                        }
                        if own {
                            continue;
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
                            let mut holds = names([&tree]);
                            holds.extend(marked);
                            named.push((tree.label.clone(), holds));
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
                        if own {
                            continue;
                        }
                        let mut one = side(&repo.root, &here, head, tree, root, other.issues, &mut bases);
                        one.holds.extend(marked);
                        others.push(one);
                    }
                }
            }
        }
    }
    let Load { issues, errors } = load;
    let (issues, mut origin, before) = overlay_keeping(issues, &others, true);
    // 못 읽는 줄은 겹치기가 안 건드린다 — 루트의 것이 곧 겹친 것의 것이다.
    let root = before.map(|issues| Load { issues, errors: errors.clone() });
    origin.named = named;
    let swept = unfound.is_none();
    Ok(Gathered { load: Load { issues, errors }, root, origin, trouble, unfound, swept, watched })
}

/// 이 화면을 **부른 쪽의 딸린 워크트리** 꼭대기들(링크를 푼 자리) — [`gather`] 가 옆에서 빼는 자리다(moai-jn4d.adc).
///
/// moai-y7go 뒤로 딸린 워크트리에서 친 `moai` 는 루트의 트래커를 읽어 `repo.root` 가 주 체크아웃이 되고,
/// [`others_of`] 는 그 꼭대기로 가르니 제 워크트리가 옆 목록에 든다 — 그러면 갈라질 때의 낡은 사본이 겹치고,
/// 제 이름과 집은 표식이 [`Origin`] 에 들어 탐색기가 제 줄에 `⎇ <제 가지>` 를 단다.
///
/// **두 자리를 다 본다.** 트래커를 찾은 자리([`Repo::here`])와 명령을 친 자리([`crate::store::invoked_checkout`])다.
/// 앞만 보던 판은 규약이 권하는 `moai -C <루트> …` 를 워크트리에서 친 판에서 옮긴 것이 없어(`here` 가 루트다)
/// 제 워크트리를 다시 옆으로 셌다 — 집은 표식이 "어디서 쳤나" 를 친 자리로 가르는 것과 같은 까닭이다
/// ([`crate::store::Repo::note_held`]). 뒤만 보면 등록한 딸린 워크트리를 밖에서 연 한눈 보기와 탐색기 층이 그
/// 워크트리를 제 것으로 못 본다.
///
/// **딸린 워크트리만 든다** — 주 체크아웃의 꼭대기는 [`others_of`] 가 루트에서 부른 판에 이미 뺐고, `MOAI_HERE`
/// 로 딸린 워크트리의 트래커를 연 판에서는 주 체크아웃이 진짜 옆이다. git 을 띄우지 않고 `.git` 이 선 첫 조상
/// ([`own_git`])으로 찾는다. [`fresh`] 는 다른 설계라(거기서는 주 체크아웃이 옆이다) 이 자를 안 쓰고,
/// [`workplaces`] 는 제 워크트리도 자리로 대야 하니 거기도 안 쓴다. 읽음을 걷는 탐색기는 이것이 비지 않으면 안
/// 걷는다 — 화면이 이 사본의 줄을 안 들었다(`tui::App` 의 `fresh_enough`).
pub fn callers(repo: &Repo) -> Vec<PathBuf> {
    let moved = (repo.here() != repo.root.as_path()).then(|| repo.here().to_path_buf());
    let mut tops: Vec<PathBuf> = moved
        .into_iter()
        .chain(crate::store::invoked_checkout())
        .filter_map(|at| match own_git(&at) {
            Some((top, _, true)) => Some(real(top)),
            _ => None,
        })
        .collect();
    tops.dedup();
    tops
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
        // 마일스톤 워크트리는 [`gather`] 와 같은 까닭으로 겹치지 않는다([`is_milestone`]).
        for (tree, root) in trees.into_iter().filter(|(tree, _)| !is_milestone(tree)) {
            let Ok(Some(other)) = crate::store::read_snapshot(&root) else {
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
    let top = real(Path::new(top.trim_end_matches('\n')));
    let rel = real(root).strip_prefix(&top).map(Path::to_path_buf).unwrap_or_default();
    // `-z` 는 git 2.36 부터다. 그 전 git 에서 거절되면 줄로 가른 것을 NUL 로 바꿔
    // 같은 파서로 읽는다 — 줄바꿈 든 경로만 잃고, 겹쳐 보기 전체를 잃지는 않는다.
    let listed = git(root, &["worktree", "list", "--porcelain", "-z"])
        .or_else(|_| git(root, &["worktree", "list", "--porcelain"]).map(|s| s.replace('\n', "\0")))?;
    let (mine, others): (Vec<Tree>, Vec<Tree>) = parse(&listed).into_iter().partition(|t| real(&t.path) == top);
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
    /// 집은 표식([`held_here`])이 사는 자리다. **여기서는 경로만 든다** — [`on_disk`] 는 훅이 도구 호출마다
    /// 지나는 길이고([`away`]), 그 안의 파일을 읽는 것은 읽을 까닭이 있는 쪽([`held_elsewhere`]·[`workplaces`])
    /// 이다. moai-n2jh 가 이 길에서 git 두 번을 걷어낸 자리다.
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
/// **읽는 것은 집은 표식([`held_here`]) 하나다 — 옆의 스냅샷은 안 연다**(moai-h64l.59m, 사용자 결정).
/// moai-y7go 뒤로 트래커는 루트에만 쓰이므로, 딸린 워크트리의 `.moai` 는 그 가지가 갈라질 때의 낡은
/// 사본이다. 그 사본에 벌여 놓인 줄(과 거기서 루트보다 늦은 줄)을 "옆이 쥐었을 수 있다" 로 세던 판은,
/// 마일스톤 가지에서 뜬 에픽 워크트리처럼 몇 주 낡은 사본이 그때 열려 있던 줄을 영영 든 채라 — 그 뒤
/// 닫았다가 오늘 루트에서 다시 집은 줄까지 의심해 규칙 1 과 `Stop` 이 그 줄을 덜 붙들었다(moai-zo36).
///
/// 이름이 id 가 아닌 워크트리(에이전트 격리 `worktree-agent-<해시>`, 옛 id 로 뜬 워크트리)가 쥔 일은
/// 이름으로 못 가른다. 스냅샷을 읽던 까닭이 그것이었는데, 이제 그 자리는 표식이 맡는다 — `moai mv` 가
/// 시작 칸으로 옮길 때 **친 자리**의 표식에 적으므로([`note_held`]), 그 워크트리 안에서(또는 거기서
/// `-C <루트>` 로) 집은 일은 스냅샷 없이도 여기 든다. **남는 틈은 하나다** — 루트에서 집은 뒤에 그
/// 일을 이름 없는 워크트리로 가져간 것(루트에서 집고 `worktree-agent-<해시>` 를 띄우는 길)은 어느
/// 표식에도 없어 루트 세션의 초점으로 선다. 이름으로 뜨는 워크트리(`moai-<id>`)는 그 길이라도 [`away`]
/// 가 이름으로 가른다.
///
/// **main 워크트리는 쥔 곳으로 안 센다** — 모두의 집기가 모이는 자리라, 세면 모든 줄이 든다.
/// 마일스톤 워크트리도 같다([`is_milestone`]). **답은 짐작이다** — 받는 쪽은 이 줄로 막거나 붙들거나
/// 비추지 않기만 한다(`hook::unsure`). git 을 띄우지 않고 워크트리마다 작은 파일 하나만 읽는다.
pub fn held_elsewhere(root: &Path) -> BTreeSet<String> {
    let Some(disk) = on_disk(root) else { return BTreeSet::new() };
    let mut out = BTreeSet::new();
    for (tree, linked, me) in &disk.all {
        if *me || !*linked || is_milestone(tree) {
            continue;
        }
        // 자리 셈([`workplaces`])과 **같은 표식을 같은 자로** 읽는다(리뷰 moai-71ht 셋째 판).
        if let Some(dir) = disk.admin.get(&tree.path) {
            out.extend(held_here(dir));
        }
    }
    out
}

/// 그 워크트리의 트래커 뿌리 — 값싼 문([`unreadable_snapshot`])이 그 스냅샷을 찾는 자다. 뿌리가 워크트리
/// 꼭대기면 빈 조각을 안 붙인다 — `join("")` 은 끝에 가름선만 더해, 그 철자를 푸는 길이 디렉터리인지를 한 번
/// 더 묻는다.
fn root_in(disk: &Disk, tree: &Tree) -> PathBuf {
    match disk.rel.as_os_str().is_empty() {
        true => tree.path.clone(),
        false => tree.path.join(&disk.rel),
    }
}

/// **그 스냅샷을 못 읽는가** — 값싼 자다: 열어서 한 바이트를 읽어 본다.
///
/// 없는 것은 못 읽는 것이 아니다 — moai 를 들이기 전에 갈라진 가지다. 자리에 디렉터리가 섰으면 열리기는
/// 해도 보통 파일이 아니라 여기서 걸린다(moai-7p48 의 재현이 그 꼴이다).
///
/// **자리 판정과는 상관없다**(moai-jn4d.ewm) — 자리는 옆 스냅샷을 안 보고 가른다. 이것은 깨진 파일을
/// 고칠 사람에게 대는 말일 뿐이라([`crate::report::Workplace::broken`]) **풀어 보지는 않는다** —
/// 여는 것은 푸는 값의 수천분의 일이다(moai-7igy 의 측정). 그래서 열려서 첫 바이트도 읽히는데 그다음이
/// 깨진 파일은 여기서 안 걸린다 — 겹쳐 보는 길([`gather`])이 그 파일을 실제로 풀어 `⎇` 줄로 댄다.
///
/// **끊긴 것(`Interrupted`)은 못 읽은 것이 아니다** — `Read::read` 는 `EINTR` 를 스스로 다시
/// 걸지 않아(`read_exact` 와 다르다), 신호 하나가 멀쩡한 워크트리를 "깨졌다" 로 세울 수 있다.
/// 여기는 말만 하는 자리라 모를 때는 입을 다무는 쪽이 싸다.
///
/// **겹쳐 보는 길과 같은 자로 연다**([`crate::held::open_inside`], moai-itsu) — 그 체크아웃(`root`) 밖·`.git/`
/// 으로 가는 링크거나 보통 파일이 아니면 [`crate::store::read_snapshot`] 이 안 읽으므로 여기서도 못 읽는
/// 것이다. 잰 자리를 **막히지 않게** 열고 그 손잡이로 보통 파일인지를 본다 — 그 자리의 FIFO 를 그냥 열면 쓰는
/// 쪽이 올 때까지 `moai status` 가 멈췄고, 막히지 않게 연 FIFO 의 한 바이트 읽기는 실패가 아니라 0 을 낸다.
fn unreadable_snapshot(root: &Path) -> bool {
    use std::io::{ErrorKind, Read};
    let quiet = |k: ErrorKind| matches!(k, ErrorKind::NotFound | ErrorKind::Interrupted);
    let snapshot = root.join(".moai").join("issues.jsonl");
    match crate::held::open_inside(&snapshot, &crate::held::Home::of(root)) {
        Err(crate::held::Fell::Unheld(_)) => true,
        Err(crate::held::Fell::Io(e)) => !quiet(e.kind()),
        Ok(mut f) => f.read(&mut [0u8]).is_err_and(|e| !quiet(e.kind())),
    }
}

/// 살아 있는 **딸린** 워크트리마다 자리 하나(moai-ir8q) — 판정은 `report::places`·`report::stranded`.
///
/// main 워크트리는 안 든다 — 모두의 집기가 모이는 자리라 거기 선 줄은 "어디서 하는가" 에 답이
/// 안 된다. 파일만 읽는다. 저장소가 아니면 비어 있다. **경로는 main 워크트리의 꼭대기에서 잰
/// 상대 경로다**([`main_top`]) — `status`·`show` 가 이 값을 그대로 낸다.
///
/// **딸린 워크트리 안에서는 `worktree` 일 때만 잰다.** 그 안에서 겹쳐 보지 않은 줄은 갈라질 때의
/// main 이라, 그 뒤 main 에서 끝내거나 놓은 줄이 거기서는 아직 집혀 있다 — 그것으로 재면 끝난 일을
/// "자리 없다" 로 대고, 감독이 그 말대로 남에게 다시 준다. **이 판단은 여기 한 곳에만 둔다** — 부르는
/// 명령마다 두었더니 `status` 에만 걸리고 `show` 에는 안 걸려 감독 안내의 두 줄이 서로 다른 답을
/// 냈다(moai-6opu.p65).
///
/// **읽는 것은 이름과 집은 표식뿐이다 — 옆의 스냅샷은 안 연다**(moai-jn4d.ewm, 사용자 결정). 훅
/// ([`held_elsewhere`])이 moai-h64l.59m 에서 먼저 그렇게 됐고, 자리 셈만 옆 스냅샷(벌여 놓인 줄, 루트보다
/// 늦게 만진 줄, 그 워크트리가 뜬 때)을 파던 동안 `moai show` 의 `자리` 와 훅이 같은 줄을 두고 다른 답을
/// 냈다. moai-y7go 뒤로 트래커는 루트에만 쓰이니 그 스냅샷은 갈라질 때의 낡은 사본이다. **남는 틈은 하나다**
/// — 루트에서 집은 뒤에 이름 없는 워크트리로 가져간 일은 어느 표식에도 없어 `Lost` 로 선다(훅의 틈과 같다).
///
/// 깨진 스냅샷을 대는 값싼 문([`unreadable_snapshot`])만은 워크트리마다 연다 — 자리가 아니라 고칠 사람에게
/// 대는 말이다([`crate::report::Workplace::broken`]).
pub fn workplaces(root: &Path, worktree: bool) -> Vec<crate::report::Workplace> {
    if !worktree && is_linked(root) {
        return Vec::new();
    }
    let Some(disk) = on_disk(root) else { return Vec::new() };
    // **마일스톤 워크트리는 자리가 아니다**([`is_milestone`]) — 트래커를 거기 쓰지 않는 것이 규약이라
    // 그 자리의 표식은 비어 있고, 이름은 아무 집은 줄도 못 가리킨다.
    let linked: Vec<&Tree> =
        disk.all.iter().filter(|(tree, linked, _)| *linked && !is_milestone(tree)).map(|(tree, ..)| tree).collect();
    // 딸린 워크트리가 하나도 없으면 여기서 끝이다 — 아래의 꼭대기 재기([`main_top`], 디스크를 묻는
    // [`real`])를 치를 까닭이 없다. 워크트리 규약을 안 쓰는 저장소의 흔한 길이다.
    if linked.is_empty() {
        return Vec::new();
    }
    // **경로는 여기서 한 번 잰다**([`main_top`]) — 이미 읽은 목록으로 재므로 git 이 적어 둔
    // 파일을 다시 안 읽는다. 부르는 쪽마다 따로 재던 때는 자가 둘이라 빈 경로를 다루는 법이
    // 갈렸고, 둘 다 같은 목록을 한 벌 더 읽었다.
    let top = main_top(&disk);
    linked
        .into_iter()
        .map(|tree| crate::report::Workplace {
            path: from_top(&top, &tree.path),
            branch: tree.label.clone(),
            names: names([tree]),
            // 훅([`held_elsewhere`])과 **같은 표식을 같은 자로** 읽는다(리뷰 moai-71ht 셋째 판).
            marked: disk.admin.get(&tree.path).map(|dir| held_here(dir)).unwrap_or_default(),
            broken: unreadable_snapshot(&root_in(&disk, tree)),
        })
        .collect()
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
    Some((top, real(&top.join(text.trim_end().strip_prefix("gitdir:")?.trim())), true))
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
    let common = real(&common);
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
        .map(|rd| rd.filter_map(Result::ok).map(|e| real(&e.path())).collect())
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

/// git 목록([`others_of`])에서 온 옆 워크트리 하나의 집은 표식 — [`Origin::working`] 이 이름과 함께 본다
/// (moai-jn4d.ewm). git 이 적어 둔 목록([`Disk::admin`])이 없는 길이라 그 워크트리의 `.git` 파일
/// (`gitdir:` 한 줄)로 찾는다([`admin_dir`]). 그 파일이 없으면(main 워크트리는 `.git` 이 디렉터리다, 사라진
/// 워크트리는 아무것도 없다) 비어 있다 — 위로 훑어 남의 `.git` 에 닿지 않게 먼저 본다.
fn marks_of(tree: &Tree) -> BTreeSet<String> {
    if !tree.path.join(".git").is_file() {
        return BTreeSet::new();
    }
    admin_dir(&tree.path).map(|dir| held_here(&dir)).unwrap_or_default()
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
/// 워크트리)은 [`main_root`] 와 같은 자([`mirror`]·[`is_bare`])가 이미 `None` 으로 가른다 — 여기서
/// [`is_linked`] 로 한 번 더 물으면
/// 같은 조상 훑기를 두 번 하고, 두 자가 갈리면(중간에 선 `.git` 파일) 안 본 자리를 옮겨 준다.
///
/// **찾은 자리를 되돌려 주지 않는다**(리뷰 moai-71ht.jlh) — 그러면 부르는 쪽이 `root != found`
/// 를 견줘야 하는데, 경로 견주기는 풀린 꼴과 안 풀린 꼴이 갈려 조용히 틀리는 자다.
///
/// **트래커가 있다는 것은 설정이 있다는 것이다**(리뷰 moai-71ht 셋째 판) — `.moai` 가 디렉터리인
/// 것만 보던 판은 무시되는 `.moai/lock` 하나만 남은 루트(옛 커밋을 체크아웃하거나 bisect 하면
/// 남는다)를 트래커로 읽어, 그 저장소의 **모든** 워크트리가 "설정이 없다" 로 넘어졌다. 락은
/// `Lock::drop` 이 안 지우므로 트래커가 통째로 사라져도 그 파일만 남는다. 그 자는
/// [`crate::store::holds_tracker`] 하나고, `init` 의 거절도 같은 자로 묻는다(moai-r0x8.apz).
pub fn tracker_root(root: &Path) -> Option<PathBuf> {
    // 트래커가 있는가(`stat` 몇 번)를 먼저 본다 — 맨 저장소인가([`is_bare`])는 git 을 띄우므로, 옮길 것이
    // 없는 자리에서는 안 묻는다. 답은 [`main_root`] 와 같다.
    let (main, common) = mirror(root)?;
    (crate::store::holds_tracker(&main) && !is_bare(&common)).then_some(main)
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
    git_dirs(root).map(|(top, _)| real(top))
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
    disk.main().map_or_else(|| real(&disk.common), |t| real(&t.path))
}

/// 워크트리 경로를 [`main_top`] 에서 잰 것으로. **늘 상대 경로다**(moai-xpd7·moai-3aec, 2026-09-18
/// 사용자 결정) — 그 밑이 아니면(main 밖에 만든 워크트리) `../` 로 올라가서 잰다. 밖을 절대 경로로
/// 두던 판은 한 배열에 두 모양이 섞여 받는 쪽이 못 갈랐고, 기계의 홈 경로가 `--json` 으로 나갔다.
///
/// **꼭대기와 같은 자리면 `.` 이다** — 빈 경로로 두면 사람 화면의 `자리` 칸이 통째로 비고
/// `--json` 의 `path` 가 `""` 로 나간다. 겹치는 머리가 없으면(다른 드라이브) 그대로 둔다 — 잴 자가 없다.
fn from_top(top: &Path, path: &Path) -> PathBuf {
    use std::path::Component;
    let path = real(path);
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
    from_top(&real(here), path).display().to_string()
}

/// 자리 없는 줄 경고와, 못 읽은 워크트리들 — **표면 셋이 같은 자를 쓴다**(moai-p3bs).
///
/// `moai status`·`.moai` 밖 한눈 보기·탐색기의 프로젝트 층이 이것을 부른다. 한때 첫째만 자리를
/// 셌고, 그래서 **죽은 세션을 찾으러 돌아온 사람이 보는 화면**(층과 밖 한눈 보기)에만 그 말이
/// 없었다. 경로는 [`workplaces`] 가 이미 [`main_top`] 에서 잰 것이다 — `show` 와 같은 자다.
///
/// 자리는 워크트리 이름과 집은 표식으로만 잰다(moai-jn4d.ewm) — 옆 스냅샷을 안 판다. 깨진 스냅샷은
/// 어느 자리도 가리지 않고 둘째 값에 들어 말만 된다.
///
/// 둘째 값은 **스냅샷이 깨진 워크트리 전부**다(moai-giz3) — 자리를 재며 값싸게 열어 본 것이고
/// ([`workplaces`]), 사람 화면이 `⎇ <가지>: <경로>` 로 한 줄씩 대고 `옆 워크트리 문제 N건` 이 센다.
/// 기계에는 `status --json` 의 `broken_worktrees` 다(moai-zah3).
pub fn stranded_at(
    root: &Path,
    cfg: &crate::config::Config,
    issues: &[Issue],
    worktree: bool,
    now: &str,
) -> (Option<crate::report::Warning>, Vec<crate::report::Workplace>) {
    // 재료([`crate::report::Footing`], moai-rviv)는 게을러서, 볼 워크트리가 없으면 안 짓는다.
    let footing = crate::report::Footing::of(issues, cfg);
    let trees = workplaces(root, worktree);
    let warning = crate::report::stranded_in(&footing, &trees, now);
    (warning, trees.into_iter().filter(|t| t.broken).collect())
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
                .and_then(|g| real(&dir.join(g.trim_end())).parent().map(Path::to_path_buf))
                .filter(|p| p.exists())
            else {
                continue;
            };
            let Some(label) = label(&dir.join("HEAD")) else { continue };
            admin.insert(path.clone(), dir);
            all.push((Tree { path, label, head: String::new() }, true));
        }
    }
    let top = real(top);
    let rel = real(root).strip_prefix(&top).map(Path::to_path_buf).unwrap_or_default();
    let all = all
        .into_iter()
        .map(|(t, linked)| {
            let me = real(&t.path) == top;
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
/// `None`.
///
/// [`tracker_root`] 가 같은 자([`mirror`]·[`is_bare`])로 트래커를 루트로 옮긴다(moai-y7go) — 워크트리의 `.moai` 를 고치면
/// 병합에서 스냅샷이 충돌하기 때문이다.
///
/// **맨 저장소인가는 git 에게 묻는다**(moai-h64l.wst, 리뷰 moai-r0x8.qbh 4번의 남은 반). 자리는
/// [`mirror`] 가 git 이 적어 둔 파일로 재고, 그 공용 디렉터리가 맨 저장소면 주 체크아웃이 없다 —
/// [`is_bare`]. git 을 띄우는 것은 **딸린 워크트리 안에서, 공용 디렉터리의 이름이 `.git` 일 때뿐**이다.
/// 주 체크아웃(`.git` 이 디렉터리)과 git 밖은 지금처럼 파일 하나 안 읽고 답한다.
pub fn main_root(root: &Path) -> Option<PathBuf> {
    let (main, common) = mirror(root)?;
    (!is_bare(&common)).then_some(main)
}

/// [`main_root`] 의 **git 을 안 띄우는 반** — 비친 자리와 공용 디렉터리. 맨 저장소인가는 아직 안 물었다.
///
/// 공용 디렉터리의 **이름**은 여기서 자리를 재는 데만 쓴다 — git 도 주 워크트리의 경로를 그렇게 잰다
/// (`worktree list` 의 첫 줄은 공용 디렉터리에서 `/.git` 을 뗀 자리다). 그 이름이 `.git` 이 아니면 잴
/// 자리가 없다: `git init --separate-git-dir` 의 공용 디렉터리(`sep.git`)는 주 체크아웃을 되가리키는
/// 줄을 어디에도 안 두어, git 에게 물어도 `worktree list` 가 주 워크트리로 `sep.git` 자체를 댄다. 그래서
/// 그 꼴은 전처럼 `None` 이다 — 지어낸 자리로 옮기느니 안 옮긴다.
fn mirror(root: &Path) -> Option<(PathBuf, PathBuf)> {
    let (top, common) = git_dirs(root)?;
    if top.join(".git").is_dir() || common.file_name()? != ".git" {
        return None;
    }
    // 둘 다 풀고 견준다 — [`same_repo`]·[`on_disk`] 와 같은 자다. `main` 은 이미 푼 경로라, 푸지 않은
    // 쪽의 조각을 붙이면 없는 자리가 선다.
    let rel = real(root).strip_prefix(real(top)).ok()?.to_path_buf();
    let main = common.parent()?;
    // 빈 `rel` 을 붙이면 끝에 `/` 가 선다 — 내미는 줄이 제 자리를 두 꼴로 쓰게 된다.
    let main = if rel.as_os_str().is_empty() { main.to_path_buf() } else { main.join(rel) };
    Some((main, common))
}

/// 이 공용 git 디렉터리가 **맨 저장소인가** — 그러면 딸린 워크트리에 주 체크아웃이 없다.
///
/// **이름이 아니라 git 에게 묻는다**(moai-h64l.wst, 2026-10-08 사용자 결정). 공용 디렉터리의 이름이
/// `.git` 인가로 가르던 판은 `git clone --bare <url> bin/.git` 에 딸린 워크트리에서 없는 주 체크아웃
/// `bin` 을 댔다 — 거기 트래커가 있으면 모든 명령이 그리로 옮겨 갔고, `init` 의 거절은 없는 자리를
/// 댔다. 답은 `core.bare` 고, 그것을 읽는 자는 git 이다(`rev-parse --is-bare-repository` 를 공용
/// 디렉터리에서). `init` 의 거절문([`crate::cmd::init`])도 이 하나로 묻는다 — 자가 둘이면 찾기가 옮겨
/// 가는 자리와 거절이 대는 자리가 갈린다.
///
/// **값**: git 한 번(이 기계에서 약 3ms)이다. 딸린 워크트리 안에서만 들고([`main_root`]), 한 프로세스
/// 안에서는 공용 디렉터리마다 한 번만 묻는다 — 한 명령이 찾기·`init`·훅의 자리 셈에서 여러 번 묻고,
/// 탐색기는 걸음마다 다시 묻는다. `core.bare` 는 저장소를 다시 만들기 전에는 안 바뀌는 값이라 담아 둬도
/// 낡지 않는다. **못 얻은 답(아래의 `false`)도 담는다**(리뷰 moai-h64l) — 안 담던 판은 git 이 멈춘 기계에서
/// 한 훅 프로세스가 찾기·집기 기록마다 [`PROBE_BUDGET`] 을 다시 기다렸고, 탐색기는 걸음마다 그만큼 섰다. 담는
/// 값은 어차피 내는 그 `false` 라 답은 안 바뀐다.
///
/// **답을 못 얻으면 맨 저장소가 아니라고 둔다**(git 이 없거나, 실패하거나, [`PROBE_BUDGET`] 안에 안
/// 끝났다). 안 옮기는 쪽(`None`)이 얼핏 조심스러워 보이지만, 그러면 git 이 잠깐 늦은 한 번에 흔한
/// 저장소의 모든 워크트리가 갈라질 때 들고 온 **낡은 스냅샷**을 읽고 거기 써서 병합에서 겨룬다 —
/// moai-y7go 가 막으려던 바로 그 조용한 갈림이다. 틀리는 것은 `.git` 이라는 이름의 맨 저장소에 트래커까지
/// 선 드문 꼴에서 git 까지 못 물을 때뿐이고, 그때는 이 결정 전과 같다. `init` 의 거절도 이전부터 같은
/// 쪽으로 접었다.
///
/// [`PROBE_BUDGET`]: crate::cmd::merge_driver::PROBE_BUDGET
pub fn is_bare(common: &Path) -> bool {
    use std::sync::Mutex;
    static SAID: Mutex<BTreeMap<PathBuf, bool>> = Mutex::new(BTreeMap::new());
    let key = real(common);
    if let Some(said) = SAID.lock().ok().and_then(|m| m.get(&key).copied()) {
        return said;
    }
    let args = ["rev-parse", "--is-bare-repository"];
    let said = crate::git::run_reading_user_config(&key, &args, Some(crate::cmd::merge_driver::PROBE_BUDGET));
    let bare = matches!(said, Some(Ok(said)) if said.trim() == "true");
    if let Ok(mut m) = SAID.lock() {
        m.insert(key, bare);
    }
    bare
}

/// [`workplaces`] 의 답을 바꿀 수 있는 파일과 **지금 잰** 표식 — git 을 띄우지 않는다.
///
/// 프로젝트 층(`tui::layer`)이 줄마다 걸음마다 잰다(moai-al0x). 층은 자리 판정을 요약에 싣는데
/// `.moai` 두 파일만 재면 워크트리를 치우거나 띄워도 다음 자동 갱신 전까지 옛 수를 낸다(옛
/// `SPC r`(다시 읽기)은 moai-en4u 가 걷었다). 드는 것:
/// - 공용 디렉터리의 `worktrees` — `git worktree add`·`remove`·`prune` 이 그 목록을 바꾼다
/// - 딸린 워크트리마다 `HEAD` — 가지 이름이 곧 이름 후보다([`names`])
/// - 딸린 워크트리의 `.git` — 제거 명령 없이 `rm -rf` 로 치운 것은 이것만 사라진다([`gather`] 와 같은 까닭)
/// - 딸린 워크트리의 스냅샷 — 깨졌는지를 값싸게 열어 본다([`unreadable_snapshot`]). 자리 판정은 그것을
///   안 본다(moai-jn4d.ewm)
///
/// 집은 표식(`moai-held`)은 안 잰다 — 그것을 고치는 것은 시작 칸을 드나드는 `moai mv` 뿐이고, 그 쓰기가
/// 이미 재는 루트의 `.moai` 를 바꾼다. 재는 것은 `stat` 과 작은 파일 읽기뿐이라 워크트리 수에 비례해도 싸다.
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
        // 마일스톤 워크트리는 자리 셈이 안 든다([`is_milestone`]) — 릴리스 머지가 그 스냅샷을 바꿔도
        // 답은 그대로라 재지 않는다. 가지가 바뀌면 위의 HEAD 가 움직인다.
        if !is_milestone(tree) {
            let snapshot = tree.path.join(&disk.rel).join(".moai").join("issues.jsonl");
            out.push((snapshot.clone(), crate::store::stamp(&snapshot)));
        }
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
    Some((top, real(&own.join(up.trim_end()))))
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
    let rel = real(root).strip_prefix(real(top)).map(Path::to_path_buf).ok()?;
    Some((real(&common), rel))
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

/// 마일스톤 가지의 머리 — `milestone/<마일스톤 id>`(2026-10-08 사용자 결정, moai-nvju).
pub const MILESTONE_BRANCH: &str = "milestone/";

/// 이 워크트리가 **마일스톤 가지의 체크아웃**인가 — 일하는 자리가 아니라 모으는 자리다(moai-nvju.ztj).
///
/// 마일스톤이 도는 동안 그 가지가 `.worktrees/milestone-<id>` 에 서서 에픽 가지를 머지로 받고, 릴리스 때
/// develop 을 받은 뒤 루트가 그것을 머지한다. 트래커는 루트에만 쓰므로 그 `.moai` 는 갈라질 때(또는
/// develop 을 받을 때)의 **낡은 사본**이고, 거기 벌여 놓인 줄은 아무도 거기서 하지 않는다. main 을
/// 자리로 안 세는 것과 같은 까닭이다 — 모두의 일이 모이는 곳이라 세면 갈라질 때 집혀 있던 줄이 다
/// 거기 선다. 이 이름은 이름 후보([`names`])로도 아무 줄을 못 가리킨다(가지에는 `/` 가 들어 id 와 통째로
/// 같을 수 없고, 디렉터리 이름 `milestone-<id>` 는 id 꼴이 아니다). 자리 셈이 옆 사본을 안 읽게 된 뒤로
/// (moai-jn4d.ewm) 남은 몫은 그 워크트리를 겹쳐 보기와 자리 목록에서 빼는 것이다.
///
/// **읽는 자가 모두 이것 하나를 본다** — 자리 셈([`workplaces`]: `places`·`stranded`·`show` 의 자리),
/// 훅의 짐작([`held_elsewhere`]), 겹쳐 보기([`gather`]·[`fresh`]), 층의 표식([`place_marks`]). 한쪽만
/// 거르면 `status` 는 그 줄을 자리 없다 하는데 훅은 옆이 쥐었다고 푼다.
///
/// **가지로만 가른다 — 디렉터리 이름은 안 본다.** 접두어가 `milestone` 인 저장소의 에픽 워크트리가
/// `.worktrees/milestone-abcd` 로 뜨면 디렉터리 이름이 이 꼴과 같다. 가지(`worktree-milestone-abcd`)는
/// 안 겹친다. 대가: 마일스톤 워크트리에서 HEAD 를 떼어 내면 그동안은 이름 없는 워크트리로 읽힌다 —
/// 릴리스 머지 중의 짧은 걸음이고, 가지를 다시 받으면 돌아온다(HEAD 는 [`heads`] 가 지켜본다).
///
/// 이름 후보([`names`])에서는 안 뺀다 — 아무 줄도 못 가리키는 후보라 빼도 판정이 안 바뀐다.
pub fn is_milestone(tree: &Tree) -> bool {
    tree.label.strip_prefix(MILESTONE_BRANCH).is_some_and(|id| !id.is_empty())
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
    let Some(src) = snapshot_at(root, base.trim()) else { return then };
    for i in crate::store::parse_issues(&src).issues {
        let at = then.entry(i.id).or_insert_with(String::new);
        if i.updated_at > *at {
            *at = i.updated_at;
        }
    }
    then
}

/// 커밋 `at` 의 `.moai/issues.jsonl` 글 — **링크로 커밋됐으면 그 커밋 안에서 링크를 따라간다**(moai-iral).
///
/// `git show <커밋>:<링크>` 는 가리키는 파일이 아니라 링크 글(`../shared/issues.jsonl`)을 낸다. 그것을
/// 스냅샷으로 풀면 줄이 하나도 없어 지운 줄을 가를 바탕이 비고, main 에서 `moai rm` 한 줄이 옆
/// 워크트리의 것으로 되살아나 `ready --worktree` 에 말없이 섰다. 상대 링크는 그 링크가 든 자리에서
/// 잰다 — `<커밋>:./<경로>` 는 `-C` 로 준 디렉터리에서 풀리고 `..` 도 git 이 접는다. moai 뿌리가
/// 꼭대기가 아니어도 된다.
///
/// **그 커밋 안에서 못 푸는 링크는 `None` 이다** — 절대 경로이거나, 저장소 밖을 가리키거나, 고리다.
/// 그 파일의 옛 글은 git 에 없으니 바탕이 없는 것이고, [`base_of`] 가 말하지 않는 것과 같은 까닭이다.
/// 링크인 트래커는 `moai status` 가 따로 한 줄로 비춘다(moai-jo3h).
///
/// **흔한 판은 `show` 하나로 끝낸다**(리뷰). 모드를 묻는 `ls-tree` 를 늘 먼저 띄우던 판은 링크가 아닌
/// 트래커에도 옆 HEAD 마다 git 을 하나 더 띄웠다 — 이 길은 훅의 거절 길이고, 옆마다 띄우는 git 을
/// 줄인 것이 moai-h498 이다([`side`]). 줄이 선 스냅샷은 개행을 들고 링크 글은 안 든다. 빈 글도
/// 링크가 아니다 — git 은 빈 링크를 못 담는다. 그래서 모드는 개행 없는 한 줄 글에만 묻는다.
/// `ls-tree` 는 경로를 pathspec 으로 받아 `GIT_ICASE_PATHSPECS` 같은 환경에서 죽는데, 그 값도 이제
/// 링크인 트래커만 치른다.
///
/// **링크면 지금 줄이 사는 자리부터 묻는다**(리뷰, [`crate::cmd::init::tracker_file`]). 커밋 안의 링크를
/// 한 칸씩 따라가는 길은 가운데 디렉터리가 링크인 판(`.moai -> tracker`, `../data/…` 의 `data` 가
/// 링크)을 못 건너 바탕을 잃었는데, 병합 줄을 거는 쪽과 링크 알림은 같은 판을 이미 디스크에서 푼다.
/// 갈라진 뒤 배치가 바뀌어 그 자리에 글이 없으면 한 칸씩 따라가는 길로 물러선다.
fn snapshot_at(root: &Path, at: &str) -> Option<String> {
    // 줄이 선 스냅샷은 개행을 들고, 빈 글은 줄 없는 스냅샷이다 — 링크 글은 둘 다 아니다.
    let snapshot = |src: &str| src.is_empty() || src.contains('\n');
    let home = crate::path::real(root);
    let lives = crate::cmd::init::tracker_file(root)
        .and_then(|f| f.strip_prefix(&home).ok().map(Path::to_path_buf))
        .filter(|rel| rel.as_path() != Path::new(crate::cmd::merge_driver::SNAPSHOT));
    if let Some(rel) = lives
        && let Ok(src) = git(root, &["show", &format!("{at}:./{}", rel.to_str()?)])
        && snapshot(&src)
    {
        return Some(src);
    }
    let mut path = PathBuf::from("./.moai/issues.jsonl");
    // 고리는 같은 자리를 다시 밟는 데서 끊는다 — 마흔 번을 다 돌면 git 을 여든 번 넘게 띄운다(리뷰).
    // `..` 은 git 도 글자로 접으므로 같은 자로 접어 견준다.
    let mut seen = std::collections::HashSet::new();
    // 깊이의 끝은 [`crate::path::follow_links`] 와 같은 마흔 번이다.
    for _ in 0..=40 {
        if !seen.insert(crate::path::lexical(&path)) {
            return None;
        }
        let spec = format!("{at}:{}", path.to_str()?);
        let src = git(root, &["show", &spec]).ok()?;
        if snapshot(&src) {
            return Some(src);
        }
        let tree = git(root, &["ls-tree", at, "--", path.to_str()?]).ok()?;
        if !tree.starts_with("120000 ") {
            return Some(src);
        }
        let to = Path::new(&src);
        if to.is_absolute() {
            return None;
        }
        path = crate::path::dir_of(&path).join(to);
    }
    None
}

fn git(root: &Path, args: &[&str]) -> Result<String, Trouble> {
    use crate::git::Error;
    crate::git::run(root, args).map_err(|e| {
        // **git 이 댄 말을 뽑는 자는 하나다**([`crate::git::Error::said`], 리뷰) — 여기서 다시
        // 훑으면 한 enum 에 대한 같은 `match` 가 세 파일에 서고, 팔 하나를 고쳐도 컴파일러가
        // 나머지를 안 잡는다. 갈래([`Lost`])만 여기서 고른다.
        let lost = match e {
            Error::Spawn(_) => Lost::NoGit,
            Error::Failed(_) => Lost::Failed,
            Error::Stream(_) => Lost::Stream,
            Error::NotUtf8(_) => Lost::Encoding,
        };
        Trouble::Unfound { lost, why: e.said() }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::model::{Kind, Status};

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
        assert_eq!(top_of(dir.path()).as_deref(), Some(real(dir.path()).as_path()), "훑기가 울타리를 넘어갔다");
        assert!(away(dir.path()).names.is_empty(), "울타리 위의 저장소가 새어 나왔다 — {:?}", away(dir.path()));
        assert!(away(&dir.join("nowhere")).names.is_empty(), "없는 자리에서도 위의 저장소를 읽었다");
        assert!(!is_linked(dir.path()), "울타리를 딸린 워크트리로 읽었다");
        let git_top =
            crate::git::run(dir.path(), &["rev-parse", "--show-toplevel"]).map(|t| PathBuf::from(t.trim_end()));
        assert_eq!(git_top.ok(), Some(real(dir.path())), "git 이 울타리를 지나쳐 위의 저장소를 잡았다");
    }

    /// **딸린 워크트리의 트래커는 주 워크트리의 같은 자리로 옮겨 간다**(moai-y7go) — 하위 디렉터리의
    /// 트래커도 그 자리째. 주 워크트리와 git 밖은 옮길 곳이 없다. **옮길 곳에 설정이 있어야 옮긴다** —
    /// 무시되는 `.moai/lock` 하나만 남은 루트는 트래커가 아니다(리뷰 moai-71ht 셋째 판).
    #[test]
    fn a_linked_tracker_points_at_the_main_one() {
        let scratch = crate::scratch::Scratch::fenced("main-root");
        let base = real(scratch.path());
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

    /// **맨 저장소인가는 이름이 아니라 git 에게 묻는다**(moai-h64l.wst, 리뷰 moai-r0x8.qbh 4번의 남은 반).
    /// 공용 디렉터리의 이름이 `.git` 인가로 가르던 판은 `git clone --bare <url> bin/.git` 에 딸린 워크트리에서
    /// 없는 주 체크아웃 `bin` 을 댔다 — 거기 트래커가 있으면 모든 명령이 그리로 옮겨 갔다.
    /// `--separate-git-dir` 의 주 체크아웃은 git 도 모른다(`worktree list` 가 `sep.git` 을 주 워크트리로 댄다) —
    /// 지어낸 자리로 옮기지 않고 전처럼 `None` 이다.
    #[test]
    fn a_bare_dot_git_has_no_main_checkout_to_move_to() {
        let scratch = crate::scratch::Scratch::fenced("main-root-bare");
        let base = real(scratch.path());
        let run = |dir: &Path, args: &[&str]| {
            let out = crate::git::isolated(dir).args(args).output().unwrap();
            assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        };
        let src = base.join("src");
        std::fs::create_dir_all(&src).unwrap();
        run(&src, &["init", "-q"]);
        run(&src, &["commit", "-q", "--allow-empty", "-m", "a"]);

        // 이름이 `.git` 인 맨 저장소 — 그 부모 `bin` 은 체크아웃이 아니다. 트래커가 거기 서 있어도 안 옮긴다.
        let bin = base.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        run(&base, &["clone", "-q", "--bare", "src", "bin/.git"]);
        run(&bin.join(".git"), &["worktree", "add", "-q", "../../wt-bin"]);
        let wt = base.join("wt-bin");
        std::fs::create_dir_all(bin.join(".moai")).unwrap();
        std::fs::write(bin.join(".moai/config.toml"), "prefix = \"t\"\n").unwrap();
        assert_eq!(main_root(&wt), None, "맨 저장소의 부모를 주 체크아웃으로 댔다");
        assert_eq!(tracker_root(&wt), None, "없는 주 체크아웃의 트래커로 옮겨 갔다");

        // 주 체크아웃이 공용 디렉터리를 딴 데 둔 저장소 — git 도 주 체크아웃을 모른다.
        let sep = base.join("sep.git");
        run(&base, &["init", "-q", "--separate-git-dir", sep.to_str().unwrap(), "main"]);
        let main = base.join("main");
        run(&main, &["commit", "-q", "--allow-empty", "-m", "a"]);
        run(&main, &["worktree", "add", "-q", "../wt-sep"]);
        assert_eq!(main_root(&base.join("wt-sep")), None, "git 도 모르는 주 체크아웃을 지어냈다");

        // 흔한 꼴은 그대로다 — 이름이 `.git` 이고 맨 저장소가 아니다.
        run(&src, &["worktree", "add", "-q", "../wt-src"]);
        assert_eq!(main_root(&base.join("wt-src")), Some(src.clone()));
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
        // **어느 워크트리인지를 자료로 든다**(moai-dpbi) — 글은 `view::trouble_line` 이 편다. 디렉터리는 보통
        // 파일이 아니라 안 읽기로 한 갈래로 선다(moai-itsu).
        let said = |b: &str| {
            got.trouble
                .iter()
                .any(|t| matches!(t, Trouble::Unread { branch, .. } | Trouble::Unheld { branch, .. } if branch == b))
        };
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

    /// **자리 셈은 옆의 낡은 사본을 안 세고 이름과 집은 표식만 읽는다**(moai-jn4d.ewm, 사용자 결정) — 훅
    /// ([`held_elsewhere`], moai-h64l.59m)과 같은 자라 `moai show` 의 `자리` 와 훅이 한 답을 낸다.
    ///
    /// - 루트에서 두 주 전에 집고 이름 없는 워크트리의 표식에만 적힌 줄(`t-0001`)은 그 워크트리에 선다
    /// - 이름 없는 워크트리의 사본에는 벌여 놓였지만 표식에는 없는 줄(`t-0002`)은 거기 안 선다 — 갈라질 때
    ///   물려받은 것이다. 사본을 읽던 판은 이것을 그 워크트리의 자리로 대, 훅은 "아무도 안 쥐었다" 고 하는데
    ///   `show` 는 그 워크트리로 이어받는 세션을 보냈다
    #[test]
    fn places_read_the_marks_not_a_siblings_stale_copy() {
        let scratch = crate::scratch::Scratch::fenced("places-marks-only");
        let base = scratch.path().to_path_buf();
        let main = base.join("main");
        std::fs::create_dir_all(&main).unwrap();
        let run = |dir: &Path, args: &[&str]| crate::git::tests::run_git(dir, None, args);
        run(&main, &["init", "-q"]);
        run(&main, &["commit", "-q", "--allow-empty", "-m", "a"]);
        // 이름이 아무 id 도 안 가리키는 워크트리(에이전트 격리).
        run(&main, &["worktree", "add", "-q", "../agent-x", "-b", "worktree-agent-x"]);
        let row = |id: &str, status: &str, at: &str| {
            format!(
                "{{\"id\":\"{id}\",\"kind\":\"issue\",\"title\":\"일\",\"status\":\"{status}\",\
                 \"created_at\":\"2026-09-11T00:00:00Z\",\"updated_at\":\"{at}\",\"status_since\":\"{at}\"}}\n"
            )
        };
        let old = "2026-09-11T00:00:00Z";
        let root = [row("t-0001", "in_progress", old), row("t-0002", "in_progress", old)].concat();
        // 사본: 갈라질 때 t-0002 가 벌여 놓여 있었고, 거기서 루트보다 늦게 고친 것으로도 보인다.
        let copy = [row("t-0001", "todo", old), row("t-0002", "in_progress", "2026-09-12T00:00:00Z")].concat();
        for (dir, rows) in [(main.clone(), &root), (base.join("agent-x"), &copy)] {
            std::fs::create_dir_all(dir.join(".moai")).unwrap();
            std::fs::write(dir.join(".moai/config.toml"), "prefix = \"t\"\n").unwrap();
            std::fs::write(dir.join(".moai/issues.jsonl"), rows).unwrap();
        }
        std::fs::write(main.join(".git/worktrees/agent-x").join(HELD), "t-0001\n").unwrap();
        let cfg = crate::config::Config::parse("prefix = \"t\"\n").unwrap();
        let mine = crate::store::read_snapshot(&main).unwrap().unwrap().issues;

        let trees = workplaces(&main, false);
        let at = crate::report::places(&mine, &cfg, &trees, "2026-09-25T00:00:00Z");
        let branches = |id: &str| at[id].at().iter().map(|t| t.branch.clone()).collect::<Vec<_>>();
        assert_eq!(branches("t-0001"), ["worktree-agent-x"], "표식에만 적힌 줄을 그 워크트리에 안 세웠다");
        assert!(
            matches!(at["t-0002"], crate::report::Place::Lost),
            "옆의 낡은 사본에 벌여 놓인 줄을 그 워크트리의 자리로 셌다 — {:?}",
            at["t-0002"].word()
        );
        // 훅도 같은 답이다 — 둘이 갈리던 것이 이 일의 까닭이다.
        assert_eq!(held_elsewhere(&main), BTreeSet::from(["t-0001".to_string()]));
    }

    /// **목록의 `⎇` 도 집은 표식을 본다**(moai-jn4d.ewm) — 이름이 id 가 아닌 워크트리가 그 안에서 집은 줄은
    /// 이름으로는 안 잡혀, 자리 셈과 훅은 그 워크트리를 대는데 탐색기 목록의 줄만 그 이름을 안 달았다.
    /// 겹친 워크트리(사본이 있다)와 이름만 든 워크트리(사본이 깨졌다) 둘 다다. main 의 `.git` 은 디렉터리라
    /// 표식이 없다.
    #[test]
    fn origin_names_a_worktree_by_its_marks() {
        let scratch = crate::scratch::Scratch::fenced("origin-marks");
        let base = scratch.path().to_path_buf();
        let main = base.join("main");
        std::fs::create_dir_all(&main).unwrap();
        let run = |dir: &Path, args: &[&str]| crate::git::tests::run_git(dir, None, args);
        run(&main, &["init", "-q"]);
        run(&main, &["commit", "-q", "--allow-empty", "-m", "a"]);
        run(&main, &["worktree", "add", "-q", "../agent-x", "-b", "worktree-agent-x"]);
        run(&main, &["worktree", "add", "-q", "../agent-y", "-b", "worktree-agent-y"]);
        let row = "{\"id\":\"t-0001\",\"title\":\"일\",\"status\":\"in_progress\",\"created_at\":\"2026-09-11T00:00:00Z\",\"updated_at\":\"2026-09-11T00:00:00Z\",\"status_since\":\"2026-09-11T00:00:00Z\"}\n";
        for dir in [main.clone(), base.join("agent-x"), base.join("agent-y")] {
            std::fs::create_dir_all(dir.join(".moai")).unwrap();
            std::fs::write(dir.join(".moai/config.toml"), "prefix = \"t\"\n").unwrap();
        }
        std::fs::write(main.join(".moai/issues.jsonl"), row).unwrap();
        std::fs::write(base.join("agent-x/.moai/issues.jsonl"), row).unwrap();
        // 사본이 못 읽히는 워크트리 — 이름만 드는 길로 간다.
        std::fs::create_dir_all(base.join("agent-y/.moai/issues.jsonl")).unwrap();
        std::fs::write(main.join(".git/worktrees/agent-x").join(HELD), "t-0001\n").unwrap();
        std::fs::write(main.join(".git/worktrees/agent-y").join(HELD), "t-0002\n").unwrap();

        let crate::store::Opened::Repo(repo) = Repo::open(&main, || crate::i18n::Lang::Ko).unwrap() else {
            panic!("저장소가 안 열렸다")
        };
        let got = gather(&repo, true).unwrap();
        assert_eq!(got.origin.working("t-0001"), Some("worktree-agent-x"), "겹친 워크트리의 표식을 안 봤다");
        assert_eq!(got.origin.working("t-0002"), Some("worktree-agent-y"), "이름만 든 워크트리의 표식을 안 봤다");
        assert_eq!(got.origin.working("t-0003"), None, "적지도 않은 줄을 옆이 쥐었다고 했다");
    }

    /// **딸린 워크트리에서 부르면 그 워크트리는 옆이 아니다**(moai-jn4d.adc). moai-y7go 뒤로 거기서 연
    /// 저장소는 루트의 트래커를 읽어 `repo.root` 가 주 체크아웃이고, 옆을 그 꼭대기로만 가르던 판은 제
    /// 워크트리를 옆으로 셌다 — 갈라질 때의 낡은 사본이 루트보다 늦어 보이면 제 줄 위에 겹치고, 제 이름과
    /// 집은 표식이 [`Origin`] 에 들어 탐색기가 제 줄에 `⎇ worktree-t-1` 을 달았다. 진짜 옆(`t-2`)은 그대로
    /// 겹치고 이름을 단다. 사본을 못 읽는 갈래(이름만 드는 길)도 같다 — 다만 **깨진 것은 말한다**(리뷰).
    /// 읽음을 걷는 쪽([`gather_unmarked`])은 제 사본도 센다 — 루트의 탐색기가 거기에만 있는 줄에 도장을 찍는다.
    #[test]
    fn the_callers_own_worktree_is_not_a_sibling() {
        let scratch = crate::scratch::Scratch::fenced("gather-own-tree");
        let base = scratch.path().to_path_buf();
        let main = base.join("main");
        std::fs::create_dir_all(&main).unwrap();
        let run = |dir: &Path, args: &[&str]| crate::git::tests::run_git(dir, None, args);
        run(&main, &["init", "-q"]);
        run(&main, &["commit", "-q", "--allow-empty", "-m", "a"]);
        run(&main, &["worktree", "add", "-q", "../t-1", "-b", "worktree-t-1"]);
        run(&main, &["worktree", "add", "-q", "../t-2", "-b", "worktree-t-2"]);
        let row = |id: &str, status: &str, at: &str| {
            format!(
                "{{\"id\":\"{id}\",\"title\":\"일\",\"status\":\"{status}\",\"created_at\":\"2026-09-11T00:00:00Z\",\
                 \"updated_at\":\"{at}\",\"status_since\":\"{at}\"}}\n"
            )
        };
        let (old, late) = ("2026-09-11T00:00:00Z", "2026-09-12T00:00:00Z");
        let root = [row("t-0001", "todo", old), row("t-0003", "todo", old)].concat();
        // 제 사본은 t-0001 을, 옆 사본은 t-0003 을 루트보다 늦게 고친 것으로 보인다. t-0009 는 제 사본에만 있다.
        let mine =
            [row("t-0001", "in_progress", late), row("t-0003", "todo", old), row("t-0009", "todo", old)].concat();
        let theirs = [row("t-0001", "todo", old), row("t-0003", "in_progress", late)].concat();
        for (dir, rows) in [(main.clone(), &root), (base.join("t-1"), &mine), (base.join("t-2"), &theirs)] {
            std::fs::create_dir_all(dir.join(".moai")).unwrap();
            std::fs::write(dir.join(".moai/config.toml"), "prefix = \"t\"\n").unwrap();
            std::fs::write(dir.join(".moai/issues.jsonl"), rows).unwrap();
        }
        std::fs::write(main.join(".git/worktrees/t-1").join(HELD), "t-0005\n").unwrap();
        std::fs::write(main.join(".git/worktrees/t-2").join(HELD), "t-0004\n").unwrap();

        let open = || {
            let crate::store::Opened::Repo(repo) = Repo::open(&base.join("t-1"), || crate::i18n::Lang::Ko).unwrap()
            else {
                panic!("저장소가 안 열렸다")
            };
            assert_eq!(real(&repo.root), real(&main), "시험의 전제 — 트래커가 루트로 옮겨 갔다");
            assert_eq!(real(repo.here()), real(&base.join("t-1")), "시험의 전제 — 부른 자리는 t-1 이다");
            repo
        };
        let got = gather(&open(), true).unwrap();
        let status = |g: &Gathered, id: &str| {
            g.load.issues.iter().find(|i| i.id == id).map(|i| i.status.as_str().to_string()).unwrap()
        };
        assert_eq!(got.origin.branch("t-0001"), None, "제 워크트리의 낡은 사본을 겹쳤다");
        assert_eq!(status(&got, "t-0001"), "todo", "제 사본의 줄이 루트의 줄을 덮었다");
        assert!(!got.origin.labels().contains(&"worktree-t-1"), "{:?}", got.origin.labels());
        assert!(!got.origin.named_only().contains(&"worktree-t-1"), "{:?}", got.origin.named_only());
        for id in ["t-0001", "t-0005"] {
            assert_eq!(got.origin.working(id), None, "{id}: 제 이름이나 표식으로 `⎇` 를 단다");
        }
        assert!(got.load.issues.iter().all(|i| i.id != "t-0009"), "제 사본에만 있는 줄을 겹쳤다");
        // 진짜 옆은 그대로다.
        assert_eq!(got.origin.branch("t-0003"), Some("worktree-t-2"), "옆의 줄을 안 겹쳤다");
        assert_eq!(got.origin.working("t-0004"), Some("worktree-t-2"), "옆의 표식을 안 봤다");
        // 걷기가 지킬 id 는 제 사본의 것까지다 — 빼면 이 워크트리의 `moai read` 가 루트에서 찍은 도장을 걷는다.
        let kept = gather_unmarked(&open()).unwrap();
        assert!(kept.load.issues.iter().any(|i| i.id == "t-0009"), "걷기가 제 사본에만 있는 줄을 안 셌다");

        // 사본을 못 읽는 갈래 — 이름만 드는 길로도 제 이름이 안 든다.
        std::fs::remove_file(base.join("t-1/.moai/issues.jsonl")).unwrap();
        std::fs::create_dir_all(base.join("t-1/.moai/issues.jsonl")).unwrap();
        let got = gather(&open(), true).unwrap();
        assert!(!got.origin.named_only().contains(&"worktree-t-1"), "{:?}", got.origin.named_only());
        assert_eq!(got.origin.working("t-0005"), None, "이름만 드는 길로 제 표식을 들었다");
        // **깨진 것은 말한다** — `swept` 인 `status` 는 이 말에 맡겨 제 목록을 접으므로, 빼면 아무 데도 안 선다.
        let told = got.trouble.iter().filter(|t| matches!(t, Trouble::Unread { branch, .. } | Trouble::Unheld { branch, .. } if branch == "worktree-t-1"));
        assert_eq!(told.count(), 1, "제 사본이 깨진 것을 안 말했다 — {:?}", got.trouble);
        assert!(got.swept, "시험의 전제 — 옆을 빠짐없이 열었다");
    }

    /// 마일스톤 워크트리는 **가지로만** 가른다 — 접두어가 `milestone` 인 저장소의 에픽 워크트리는 디렉터리
    /// 이름이 그 꼴과 같다.
    #[test]
    fn a_milestone_tree_is_told_by_its_branch_alone() {
        let t = |path: &str, label: &str| Tree { path: PathBuf::from(path), label: label.into(), head: String::new() };
        assert!(is_milestone(&t("/r/.worktrees/milestone-moai-zzok", "milestone/moai-zzok")));
        assert!(is_milestone(&t("/r/anywhere", "milestone/moai-zzok")), "디렉터리 이름에 기댔다");
        assert!(
            !is_milestone(&t("/r/.worktrees/milestone-abcd", "worktree-milestone-abcd")),
            "에픽 워크트리를 마일스톤으로 읽었다"
        );
        assert!(!is_milestone(&t("/r/.worktrees/moai-nvju", "worktree-moai-nvju")));
        assert!(!is_milestone(&t("/r/x", "milestone/")), "id 없는 가지를 마일스톤으로 읽었다");
        assert!(!is_milestone(&t("/r/x", "milestones/x")));
    }

    /// **마일스톤 가지의 워크트리는 일하는 자리가 아니다**(moai-nvju.ztj, 2026-10-08 사용자 결정). 그
    /// 워크트리(`milestone/<id>`)는 마일스톤이 도는 내내 서서 에픽 가지를 머지로 받을 뿐이고 트래커는 루트에만
    /// 쓴다 — 그 스냅샷은 갈라질 때(또는 develop 을 받을 때)의 낡은 사본이다. 이름이 아무 줄도 안 가리키니
    /// "이름 없는 워크트리" 로 읽혀, 사본에 벌여 놓인 줄이 거기서 도는 것으로 서던 판은 자리를 잃은 줄을
    /// `stranded` 에서 감췄고(`places`), 훅은 그 줄을 "옆이 쥐었을 수 있다" 로 풀었고(`held_elsewhere`),
    /// `--worktree` 는 그 사본의 늦은 줄을 남의 산 집기처럼 겹쳤다(`gather`·`fresh`).
    #[test]
    fn a_milestone_worktree_is_not_a_place_where_work_stands() {
        let scratch = crate::scratch::Scratch::fenced("milestone-tree");
        let base = scratch.path().to_path_buf();
        let main = base.join("main");
        std::fs::create_dir_all(&main).unwrap();
        let run = |dir: &Path, args: &[&str]| crate::git::tests::run_git(dir, None, args);
        run(&main, &["init", "-q"]);
        run(&main, &["commit", "-q", "--allow-empty", "-m", "a"]);
        run(&main, &["worktree", "add", "-q", "../milestone-t-zzzz", "-b", "milestone/t-zzzz"]);
        // 볼 워크트리가 하나도 없으면 `places` 가 아무 답도 안 낸다 — 스냅샷 없는 에픽 워크트리 하나를 세운다.
        run(&main, &["worktree", "add", "-q", "../t-eeee", "-b", "worktree-t-eeee"]);
        let row = |id: &str, kind: &str, status: &str, at: &str| {
            format!(
                "{{\"id\":\"{id}\",\"kind\":\"{kind}\",\"title\":\"일\",\"status\":\"{status}\",\
                 \"created_at\":\"2026-09-11T00:00:00Z\",\"updated_at\":\"{at}\",\"status_since\":\"{at}\"}}\n"
            )
        };
        let old = "2026-09-11T00:00:00Z";
        // 루트: t-0001 은 갈라진 뒤 루트에서 옮겼고(칸이 늦게 섰다), t-0002 는 아직 첫 칸이다.
        let root = [
            row("t-0001", "issue", "in_progress", "2099-01-01T00:00:00Z"),
            row("t-0002", "issue", "todo", old),
            row("t-zzzz", "milestone", "todo", old),
        ]
        .concat();
        // 마일스톤 워크트리의 사본: 갈라질 때 t-0001 이 벌여 놓여 있었고, t-0002 는 루트보다 늦은 줄이다.
        let copy = [
            row("t-0001", "issue", "in_progress", old),
            row("t-0002", "issue", "in_progress", "2026-09-12T00:00:00Z"),
            row("t-zzzz", "milestone", "todo", old),
        ]
        .concat();
        for (dir, rows) in [(main.clone(), &root), (base.join("milestone-t-zzzz"), &copy)] {
            std::fs::create_dir_all(dir.join(".moai")).unwrap();
            std::fs::write(dir.join(".moai/config.toml"), "prefix = \"t\"\n").unwrap();
            std::fs::write(dir.join(".moai/issues.jsonl"), rows).unwrap();
        }
        let cfg = crate::config::Config::parse("prefix = \"t\"\n").unwrap();
        let mine = crate::store::read_snapshot(&main).unwrap().unwrap().issues;

        // 세기·그리기 — 자리 목록에 안 서고, 그 사본으로 집은 줄의 자리를 대지 않는다.
        let trees = workplaces(&main, false);
        assert!(
            trees.iter().all(|t| t.branch != "milestone/t-zzzz"),
            "마일스톤 워크트리를 일하는 자리로 셌다 — {:?}",
            trees.iter().map(|t| &t.branch).collect::<Vec<_>>()
        );
        let at = crate::report::places(&mine, &cfg, &trees, "2099-01-02T00:00:00Z");
        assert!(
            matches!(at.get("t-0001"), Some(crate::report::Place::Lost)),
            "마일스톤 워크트리의 낡은 사본이 자리를 잃은 줄을 감췄다 — {:?}",
            at.get("t-0001").map(|p| p.word())
        );

        // 훅의 짐작 — 사본에 벌여 놓인 줄과 사본에서 늦은 줄을 "옆이 쥐었을 수 있다" 로 안 센다.
        let elsewhere = held_elsewhere(&main);
        assert!(elsewhere.is_empty(), "마일스톤 워크트리의 사본을 옆의 집기로 셌다 — {elsewhere:?}");

        // 겹쳐 보기 — 사본의 늦은 줄을 남의 산 줄처럼 겹치지 않는다(`--worktree` 와 훅의 `fresh` 둘 다).
        let crate::store::Opened::Repo(repo) = Repo::open(&main, || crate::i18n::Lang::Ko).unwrap() else {
            panic!("저장소가 안 열렸다")
        };
        let got = gather(&repo, true).unwrap();
        assert!(
            !got.origin.labels().contains(&"milestone/t-zzzz"),
            "마일스톤 워크트리를 겹쳤다 — {:?}",
            got.origin.labels()
        );
        let status = |rows: &[Issue]| rows.iter().find(|i| i.id == "t-0002").map(|i| i.status.as_str().to_string());
        assert_eq!(status(&got.load.issues).as_deref(), Some("todo"), "사본의 줄이 겹쳐 섰다");
        let (fresh_rows, _) = fresh(&repo, mine.clone()).expect("git 목록을 못 읽었다");
        assert_eq!(status(&fresh_rows).as_deref(), Some("todo"), "훅이 사본의 줄을 겹쳤다");

        // 층의 표식 — 자리 셈이 안 드는 사본은 안 재되, 가지가 바뀌면 다시 읽도록 HEAD 는 잰다.
        let marks = place_marks(&main);
        let copy_path = Path::new("milestone-t-zzzz").join(".moai").join("issues.jsonl");
        assert!(marks.iter().all(|(p, _)| !p.ends_with(&copy_path)), "자리 셈이 안 드는 사본을 잰다 — {marks:#?}");
        let head = Path::new("worktrees").join("milestone-t-zzzz").join("HEAD");
        assert!(marks.iter().any(|(p, _)| p.ends_with(&head)), "마일스톤 워크트리의 HEAD 를 안 잰다 — {marks:#?}");
    }

    /// **훅의 짐작은 옆의 낡은 사본을 안 세고 집은 표식만 읽는다**(moai-h64l.59m, 사용자 결정). 트래커는
    /// 루트에만 쓰이므로(moai-y7go) 딸린 워크트리의 `.moai` 는 갈라질 때의 사본이다 — 거기 벌여 놓인 채
    /// 남은 줄(그 뒤 닫았다가 오늘 루트에서 다시 집은 일)을 "옆이 쥐었을 수 있다" 로 세면 규칙 1 과
    /// `Stop` 이 그 줄을 덜 붙든다(moai-zo36). 사본에서 루트보다 늦은 줄도 안 센다. 표식에 적힌 줄은 든다.
    #[test]
    fn held_elsewhere_reads_the_marks_not_a_siblings_stale_copy() {
        let scratch = crate::scratch::Scratch::fenced("held-marks-only");
        let base = scratch.path().to_path_buf();
        let main = base.join("main");
        std::fs::create_dir_all(&main).unwrap();
        let run = |dir: &Path, args: &[&str]| crate::git::tests::run_git(dir, None, args);
        run(&main, &["init", "-q"]);
        run(&main, &["commit", "-q", "--allow-empty", "-m", "a"]);
        // 이름이 아무 id 도 안 가리키는 워크트리 — 스냅샷을 읽던 판은 이것의 사본을 셌다.
        run(&main, &["worktree", "add", "-q", "../agent-x", "-b", "worktree-agent-x"]);
        let row = |id: &str, status: &str, at: &str| {
            format!(
                "{{\"id\":\"{id}\",\"kind\":\"issue\",\"title\":\"일\",\"status\":\"{status}\",\
                 \"created_at\":\"2026-09-11T00:00:00Z\",\"updated_at\":\"{at}\",\"status_since\":\"{at}\"}}\n"
            )
        };
        let old = "2026-09-11T00:00:00Z";
        // 루트: t-0001 은 오늘 다시 집었고, t-0002 는 첫 칸, t-0003 은 옆 워크트리에서 집은 일이다.
        let root = [
            row("t-0001", "in_progress", "2099-01-01T00:00:00Z"),
            row("t-0002", "todo", old),
            row("t-0003", "in_progress", old),
        ]
        .concat();
        // 사본: 갈라질 때 t-0001 이 벌여 놓여 있었고, t-0002 는 사본에서 루트보다 늦게 옮겼다.
        let copy = [
            row("t-0001", "in_progress", old),
            row("t-0002", "in_progress", "2026-09-12T00:00:00Z"),
            row("t-0003", "todo", old),
        ]
        .concat();
        for (dir, rows) in [(main.clone(), &root), (base.join("agent-x"), &copy)] {
            std::fs::create_dir_all(dir.join(".moai")).unwrap();
            std::fs::write(dir.join(".moai/config.toml"), "prefix = \"t\"\n").unwrap();
            std::fs::write(dir.join(".moai/issues.jsonl"), rows).unwrap();
        }
        std::fs::write(main.join(".git/worktrees/agent-x").join(HELD), "t-0003\n").unwrap();

        let elsewhere = held_elsewhere(&main);
        assert_eq!(
            elsewhere,
            BTreeSet::from(["t-0003".to_string()]),
            "옆의 낡은 사본을 옆의 집기로 셌거나 표식을 놓쳤다"
        );
    }

    /// **옆 워크트리의 스냅샷도 그 체크아웃 안에서만 읽는다**(moai-itsu). 밖을 가리키는 링크는 겹치지 않고
    /// 고른 말로 까닭까지 말하며, FIFO 는 열다 멈추지 않는다 — 고침이 없으면 이 시험은 `t-2` 의 FIFO 앞에서
    /// 영영 멈춘다(쓰는 쪽이 없다). 겹쳐 보는 길([`gather`])과 자리 셈의 값싼 문([`unreadable_snapshot`])을
    /// 다 지난다 — 자리 셈은 집은 줄과 상관없이 워크트리마다 그 문을 연다(moai-jn4d.ewm).
    #[cfg(unix)]
    #[test]
    fn a_siblings_snapshot_is_read_only_inside_its_checkout() {
        let scratch = crate::scratch::Scratch::fenced("held-sibling");
        let base = scratch.path().to_path_buf();
        let main = base.join("main");
        std::fs::create_dir_all(&main).unwrap();
        let run = |dir: &Path, args: &[&str]| crate::git::tests::run_git(dir, None, args);
        run(&main, &["init", "-q"]);
        run(&main, &["commit", "-q", "--allow-empty", "-m", "a"]);
        run(&main, &["worktree", "add", "-q", "../t-1", "-b", "worktree-t-1"]);
        run(&main, &["worktree", "add", "-q", "../t-2", "-b", "worktree-t-2"]);
        let row = |id: &str| {
            format!(
                "{{\"id\":\"{id}\",\"title\":\"집힌 일\",\"status\":\"in_progress\",\"created_at\":\"2026-09-11T00:00:00Z\",\
                 \"updated_at\":\"2026-09-11T00:00:00Z\",\"status_since\":\"2026-09-11T00:00:00Z\"}}\n"
            )
        };
        for dir in [main.clone(), base.join("t-1"), base.join("t-2")] {
            std::fs::create_dir_all(dir.join(".moai")).unwrap();
            std::fs::write(dir.join(".moai/config.toml"), "prefix = \"t\"\n").unwrap();
        }
        std::fs::write(main.join(".moai/issues.jsonl"), row("t-0001")).unwrap();
        // 밖에 선 멀쩡한 스냅샷 — 고침이 없으면 그 줄이 겹쳐 선다.
        std::fs::write(base.join("away.jsonl"), row("t-0009")).unwrap();
        std::os::unix::fs::symlink(base.join("away.jsonl"), base.join("t-1/.moai/issues.jsonl")).unwrap();
        let fifo = std::ffi::CString::new(base.join("t-2/.moai/issues.jsonl").as_os_str().as_encoded_bytes()).unwrap();
        // SAFETY: 널로 끝나는 경로와 권한 비트만 넘긴다.
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0, "FIFO 를 못 지었다");

        let crate::store::Opened::Repo(repo) = Repo::open(&main, || crate::i18n::Lang::Ko).unwrap() else {
            panic!("저장소가 안 열렸다")
        };
        // FIFO 앞에서 멈추면 기다리지 않고 진다([`crate::held::tests::within`]) — 붉은 시험의 이름이 CI 의 시간
        // 끝보다 먼저 서게 한다.
        use crate::held::tests::within;
        let got = within("gather", move || gather(&repo, true).unwrap());
        assert!(got.load.issues.iter().all(|i| i.id != "t-0009"), "체크아웃 밖의 스냅샷을 겹쳤다");
        // **까닭은 자료로 실려 고른 말로 펴진다**(리뷰 moai-itsu.n8z) — 글로 지어 싣던 판은 한국어 머리 뒤에
        // moai 가 지은 영어(`not a regular file`)가 붙었고, 밖 링크는 까닭 없이 `<자리> -> <끝>` 만 댔다.
        let lang = crate::i18n::Lang::Ko;
        for (b, outside) in [("worktree-t-1", true), ("worktree-t-2", false)] {
            let t = got.trouble.iter().find(|t| matches!(t, Trouble::Unheld { branch, .. } if branch == b));
            let Some(t @ Trouble::Unheld { why, .. }) = t else {
                panic!("{b}: 안 읽은 스냅샷을 자료로 말하지 않는다 — {:?}", got.trouble)
            };
            assert_eq!(
                matches!(why, crate::held::Unheld::Outside { .. }),
                outside,
                "{b}: 까닭의 갈래가 틀렸다 — {why:?}"
            );
            let line = crate::view::trouble_line(lang, t);
            assert!(line.contains(&crate::held::said(lang, why)), "{b}: 고른 말의 까닭이 안 선다 — {line}");
            assert!(!line.contains("not a regular file"), "{b}: moai 가 지은 영어가 붙었다 — {line}");
        }
        for t in within("값싼 문", move || workplaces(&main, false)) {
            assert!(t.broken, "{}: 값싼 문이 안 읽는 스냅샷을 깨졌다고 안 했다", t.branch);
        }
    }

    /// **겹치기 전의 제 스냅샷을 실어 보낸다**(moai-ug6x.pi3) — 옆에서 줄이 들어왔을 때만이다. 아카이브의 충돌과 옮길
    /// 수를 루트와 견주는 자리(`moai status --worktree`·탐색기의 배너·한눈 보기)가 이것을 받아, 방금 판 파일을 걸음마다
    /// `repo.read()` 로 다시 풀지 않는다. 옆이 조용하면 겹친 줄이 곧 그것이라 베끼지 않는다 — 늘 베끼면 보통의
    /// 걸음이 안 쓸 한 벌을 치른다.
    #[test]
    fn the_gathered_load_keeps_the_root_snapshot_from_before_the_overlay() {
        let scratch = crate::scratch::Scratch::fenced("gathered-root");
        let base = scratch.path().to_path_buf();
        let main = base.join("main");
        std::fs::create_dir_all(&main).unwrap();
        let run = |dir: &Path, args: &[&str]| crate::git::tests::run_git(dir, None, args);
        run(&main, &["init", "-q"]);
        run(&main, &["commit", "-q", "--allow-empty", "-m", "a"]);
        run(&main, &["worktree", "add", "-q", "../t-1", "-b", "worktree-t-1"]);
        let row = |status: &str, at: &str| {
            format!(
                "{{\"id\":\"t-0001\",\"title\":\"일\",\"status\":\"{status}\",\"created_at\":\"2026-09-11T00:00:00Z\",\
                 \"updated_at\":\"{at}\",\"status_since\":\"{at}\"}}\n"
            )
        };
        for dir in [main.clone(), base.join("t-1")] {
            std::fs::create_dir_all(dir.join(".moai")).unwrap();
            std::fs::write(dir.join(".moai/config.toml"), "prefix = \"t\"\n").unwrap();
            std::fs::write(dir.join(".moai/issues.jsonl"), row("todo", "2026-09-11T00:00:00Z")).unwrap();
        }
        let open = || {
            let crate::store::Opened::Repo(repo) = Repo::open(&main, || crate::i18n::Lang::Ko).unwrap() else {
                panic!("저장소가 안 열렸다")
            };
            repo
        };
        // 옆이 조용하다 — 같은 줄뿐이라 겹친 것이 곧 루트다.
        let quiet = gather(&open(), true).unwrap();
        assert!(quiet.origin.branches().is_empty(), "시험의 전제 — 옆에서 온 줄이 없다");
        assert!(quiet.root.is_none(), "옆에서 온 줄이 없는데 루트를 한 벌 베꼈다");

        // 옆에서 늦게 집었다 — 겹친 줄은 옆의 것, 루트는 제 파일의 것이다.
        std::fs::write(base.join("t-1/.moai/issues.jsonl"), row("in_progress", "2026-09-12T00:00:00Z")).unwrap();
        let got = gather(&open(), true).unwrap();
        assert_eq!(got.load.issues[0].status.as_str(), "in_progress", "시험의 전제 — 옆의 줄이 겹쳐 섰다");
        let root = got.root.as_ref().expect("옆에서 줄이 들어왔는데 겹치기 전의 스냅샷을 안 실었다");
        assert_eq!(root.issues, open().read().unwrap().issues, "실은 것이 루트의 스냅샷이 아니다");
        assert_eq!(root.issues[0].status.as_str(), "todo");

        // 겹쳐 보지 않으면 실을 것이 없다.
        assert!(gather(&open(), false).unwrap().root.is_none());
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
