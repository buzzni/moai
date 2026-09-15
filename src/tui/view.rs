//! 목록에 **무엇을 보일까**(moai-fmv5). 거름망이 아니라 보기다 — 사람이 적은 물음(`SPC f`·`/`)과
//! 따로 들고, 목록은 둘을 함께 통과한 줄만 세운다. Esc 는 거름망만 푼다: 늘 켜 두는 보기를 실수
//! 한 번에 잃으면 done 이 도로 쏟아진다.
//!
//! **조각이다.** `App` 도 설정도 모른다. 처음 무엇을 숨길지(done)는 든 쪽이 정하고, 줄마다의
//! 사실(묶음이면 멤버에서 읽은 칸, 물려받았든 미뤘는가)도 든 쪽이 재서 넘긴다 — 여기서 이슈를
//! 풀어 칸을 다시 읽으면 목록의 글리프와 숨김이 다른 칸을 본다.

/// 칸마다 보이는가, 미룬 것을 보이는가.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct View {
    /// 숨긴 칸의 이름. **보인 쪽이 아니라 숨긴 쪽을 든다** — 설정에 칸이 새로 생기면 저절로
    /// 보인다. 보인 쪽을 들면 새 칸의 줄이 이유 없이 사라진다.
    pub hidden: Vec<String>,
    pub hide_deferred: bool,
}

impl View {
    /// 칸 하나를 숨긴 채로.
    pub fn hiding(column: &str) -> View {
        View { hidden: vec![column.to_string()], hide_deferred: false }
    }

    pub fn hides(&self, column: &str) -> bool {
        self.hidden.iter().any(|h| h == column)
    }

    pub fn toggle(&mut self, column: &str) {
        match self.hidden.iter().position(|h| h == column) {
            Some(at) => {
                self.hidden.remove(at);
            }
            None => self.hidden.push(column.to_string()),
        }
    }

    /// 그 줄이 보이는가. `column` 은 묶음이면 멤버에서 읽은 칸이고, `known` 은 이 프로젝트의 칸이다.
    ///
    /// **숨김은 이 프로젝트에 있는 칸에만 건다**(moai-2kyl 단계 리뷰) — 뱃지([`View::badge`])와 같은 자다.
    /// 보기는 사람의 설정이라 다른 프로젝트의 칸 이름을 들고 다니는데, 그 이름이 여기서 줄을 숨기면 뱃지도
    /// 번호 토글도 없어 줄이 말없이 사라진다. 설정에서 칸 이름을 바꿔 옛 칸에 남은 줄도 그렇다.
    pub fn shows(&self, column: &str, deferred: bool, known: &[String]) -> bool {
        let hidden = self.hides(column) && known.iter().any(|k| k == column);
        !hidden && !(deferred && self.hide_deferred)
    }

    /// 모두 보인다(`SPC s a`). **이 프로젝트의 칸만 걷는다**(moai-2kyl 단계 리뷰) — 다른 프로젝트에만 있는
    /// 칸 이름은 여기서 아무것도 안 숨겼으니 들고 있는다. 통째로 비우면 그 프로젝트로 돌아갔을 때 숨겨 둔
    /// 칸이 쏟아진다.
    pub fn show_all(&mut self, known: &[String]) {
        self.hidden.retain(|h| !known.contains(h));
        self.hide_deferred = false;
    }

    /// 경로 줄에 댈 한 마디 — `done·미룸 숨김`. 숨긴 것이 없으면 없다.
    ///
    /// **이 프로젝트의 칸(`known`)만 댄다**(moai-2bzp). 보기는 사람의 설정이라 프로젝트를 옮겨도
    /// 이어지는데, 다른 프로젝트에만 있는 칸 이름까지 대면 여기서는 번호 토글이 없어 걷을 길이 없다.
    /// 그 이름은 버리지 않고 들고 있다 — 그 칸이 있는 프로젝트로 돌아가면 다시 숨는다. 여기서는 줄도
    /// 안 숨긴다([`View::shows`]).
    pub fn badge(&self, known: &[String]) -> Option<String> {
        let mut names: Vec<&str> = self.hidden.iter().filter(|h| known.contains(h)).map(String::as_str).collect();
        if self.hide_deferred {
            names.push("미룸");
        }
        (!names.is_empty()).then(|| format!("{} 숨김", names.join("·")))
    }
}

/// 목록 줄에 붙일 수 있는 열(moai-g7p8). 제목과 칸 글리프는 늘 선다 — 끄면 줄이 무엇인지 모른다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Id,
    Priority,
    Assignee,
    Created,
    Updated,
    /// 묶음의 `끝난/일` 셈.
    Tally,
    Tags,
    /// 목록 맨 위의 **열 이름 줄**(moai-3fnf). 값이 아니라 줄 하나지만 켜고 끄는 자리가 열과 같아
    /// 여기 든다 — `SPC c` 밑에 서고 설정에도 열과 같은 자리에 적힌다.
    Names,
    /// 제목 앞의 `⎇ <가지>` — 그 이슈를 이름에 단 옆 가지(moai-nxt4).
    Branch,
}

impl Field {
    pub fn word(self) -> &'static str {
        match self {
            Field::Id => "id",
            Field::Priority => "우선순위",
            Field::Assignee => "담당",
            Field::Created => "생성",
            Field::Updated => "수정",
            Field::Tally => "셈",
            Field::Tags => "태그",
            Field::Names => "열 이름",
            // `SPC t w`(`keys::Toggle::Worktree`)가 이미 "워크트리" 다 — 같은 낱말을 두 줄에
            // 세우면 메뉴에서 어느 쪽이 겹쳐 보기고 어느 쪽이 줄의 표시인지 못 가른다.
            Field::Branch => "옆 가지",
        }
    }

    /// 좁을 때 **걷는 차례** — 작을수록 먼저 걷힌다(사람의 결정: 날짜 → 담당 → 태그). id·우선순위·
    /// 셈은 원래 목록 줄에 있던 것이라 이 차례로 걷지 않는다 — 켜 두면 제목 몫을 줄여서라도 선다.
    pub fn drop_rank(self) -> Option<u8> {
        match self {
            Field::Created | Field::Updated => Some(0),
            Field::Assignee => Some(1),
            Field::Tags => Some(2),
            Field::Id | Field::Priority | Field::Tally | Field::Names | Field::Branch => None,
        }
    }

    fn bit(self) -> u16 {
        1 << self as u16
    }

    pub const ALL: [Field; 9] = [
        Field::Id,
        Field::Priority,
        Field::Assignee,
        Field::Created,
        Field::Updated,
        Field::Tally,
        Field::Tags,
        Field::Names,
        Field::Branch,
    ];

    /// 설정 파일에 적는 이름(moai-2bzp). 화면의 낱말([`Field::word`])과 따로 둔다 — 낱말을 다듬은 날
    /// 이미 적힌 설정이 안 읽히면 그건 다듬기가 아니라 마이그레이션이다.
    pub fn name(self) -> &'static str {
        match self {
            Field::Id => "id",
            Field::Priority => "priority",
            Field::Assignee => "assignee",
            Field::Created => "created",
            Field::Updated => "updated",
            Field::Tally => "tally",
            Field::Tags => "tags",
            Field::Names => "names",
            Field::Branch => "branch",
        }
    }

    /// `fields_known` 이 없던 때(moai-3fnf 앞)의 어휘 — 그때 이미 있던 열이다. 그 설정에서 안 적힌
    /// 이 열들은 **사람이 끈 것**이고, 여기 없는 열(열 이름 줄·⎇)은 그 바이너리가 몰랐던 것이라
    /// 기본값으로 선다.
    pub const BEFORE_KNOWN: [Field; 7] = [
        Field::Id,
        Field::Priority,
        Field::Assignee,
        Field::Created,
        Field::Updated,
        Field::Tally,
        Field::Tags,
    ];

    pub fn named(name: &str) -> Option<Field> {
        Field::ALL.into_iter().find(|f| f.name() == name)
    }
}

/// 켜 둔 열. 복사로 다닌다 — 키 표의 켜짐(`Ctx`)이 이것을 그대로 든다. **`u16` 이다**(moai-3fnf) —
/// 여덟 열에서 꽉 차는 `u8` 로 두면 아홉째 열을 더하는 날 `Field::Id` 와 비트가 겹친다(moai-7pd5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fields(u16);

/// **처음에는 원래 목록 줄에 열 이름을 얹은 것**이다 — id·우선순위·셈·열 이름. 열 이름이 기본 켬인 것은
/// 사용자 결정이고(moai-3fnf), 줄이 적은 창에서는 `SPC c h` 로 끈다.
impl Default for Fields {
    fn default() -> Fields {
        Fields(Field::Id.bit() | Field::Priority.bit() | Field::Tally.bit() | Field::Names.bit() | Field::Branch.bit())
    }
}

/// **열 하나가 비트 하나다.** 열이 [`Fields`] 의 폭을 넘기는 날 `1 << n` 이 감싸 첫 열과 비트가
/// 겹치고, release 에서는 그 감싸기가 조용하다 — 화면에서 id 를 끄면 새 열이 같이 꺼진다.
/// 그날 **컴파일이 멈추게** 못 박는다(moai-ggqf, moai-7pd5 가 적어 둔 것).
const _: () = assert!(
    widest_bit() < u16::BITS,
    "열이 Fields 의 비트 폭을 넘었다 — Fields 와 Field::bit 를 더 넓은 정수로 옮겨라"
);

/// [`Field::ALL`] 가운데 가장 큰 비트 자리. `Field::bit` 이 쓰는 그 자리다.
const fn widest_bit() -> u32 {
    let mut widest = 0;
    let mut at = 0;
    while at < Field::ALL.len() {
        let bit = Field::ALL[at] as u32;
        if bit > widest {
            widest = bit;
        }
        at += 1;
    }
    widest
}

impl Fields {
    /// 아무 열도 안 켠 것 — **비트 가드가 쓴다**(`every_column_owns_a_bit_and_stands_in_all`).
    /// 설정을 입히는 길은 처음값에서 시작하므로(moai-3fnf 리뷰, `App::apply_look`) 화면 코드에는
    /// 이 자리가 없다. 시험에만 서므로 `#[cfg(test)]` — 안 그러면 release 빌드마다 죽은 코드
    /// 경고가 한 줄 선다.
    #[cfg(test)]
    pub fn none() -> Fields {
        Fields(0)
    }

    pub fn shows(self, f: Field) -> bool {
        self.0 & f.bit() != 0
    }

    pub fn toggle(&mut self, f: Field) {
        self.0 ^= f.bit();
    }

    /// 켜거나 끈다 — 지금 어느 쪽이든 `on` 이 된다(moai-zrzo). 없던 때는 `shows` 로 물은 뒤 `toggle` 로
    /// 흉내 냈다.
    pub fn set(&mut self, f: Field, on: bool) {
        if on {
            self.0 |= f.bit();
        } else {
            self.0 &= !f.bit();
        }
    }

    /// 둘 다 켠 열.
    pub fn both(self, other: Fields) -> Fields {
        Fields(self.0 & other.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_start_as_the_old_row_and_toggle_one_at_a_time() {
        let mut f = Fields::default();
        assert!(f.shows(Field::Id) && f.shows(Field::Priority) && f.shows(Field::Tally));
        assert!(!f.shows(Field::Assignee) && !f.shows(Field::Created) && !f.shows(Field::Tags));
        f.toggle(Field::Assignee);
        assert!(f.shows(Field::Assignee) && f.shows(Field::Id), "하나를 켜며 다른 것을 건드렸다");
        f.toggle(Field::Assignee);
        assert_eq!(f, Fields::default());
        f.set(Field::Id, true);
        assert_eq!(f, Fields::default(), "켜진 것을 켜며 껐다");
        f.set(Field::Id, false);
        f.set(Field::Id, false);
        assert!(!f.shows(Field::Id) && f.shows(Field::Priority), "끈 것을 끄며 켰거나 옆 열을 건드렸다");
    }

    /// **열마다 제 비트를 쓴다**(moai-ggqf) — 둘이 같은 비트를 쓰면 하나를 끄며 다른 하나가 꺼진다.
    /// `Field::ALL` 에 빠진 열도 여기서 걸린다: 빠진 열은 설정에 저장되지도, 폭 가드에 세이지도 않는다.
    #[test]
    fn every_column_owns_a_bit_and_stands_in_all() {
        let mut seen = Fields::none();
        for f in Field::ALL {
            assert!(!seen.shows(f), "{} 가 앞 열과 같은 비트를 쓴다", f.name());
            seen.set(f, true);
        }
        for f in Field::ALL {
            assert!(seen.shows(f), "{} 를 켰는데 꺼졌다 — 비트가 겹친다", f.name());
        }
        // 갈래를 빠짐없이 적는 match — 새 열을 더하면 여기서 멈추고, ALL 에도 넣으라고 댄다.
        for f in Field::ALL {
            let in_all = match f {
                Field::Id
                | Field::Priority
                | Field::Assignee
                | Field::Created
                | Field::Updated
                | Field::Tally
                | Field::Tags
                | Field::Names
                | Field::Branch => true,
            };
            assert!(in_all);
        }
        assert_eq!(Field::ALL.len(), 9, "열을 더했으면 ALL 과 이 시험을 함께 고친다");
    }

    /// 이 프로젝트의 칸 — 시험마다 같은 설정이다.
    fn here() -> Vec<String> {
        ["todo", "blocked", "done"].map(String::from).to_vec()
    }

    #[test]
    fn a_hidden_column_comes_back_when_toggled_again() {
        let mut v = View::hiding("done");
        assert!(!v.shows("done", false, &here()));
        assert!(v.shows("todo", true, &here()), "미룬 것은 처음에 보인다");
        v.toggle("done");
        assert_eq!(v, View::default());
        v.toggle("done");
        assert_eq!(v, View::hiding("done"), "두 번 누르면 제자리다");
    }

    #[test]
    fn deferred_hides_on_its_own_axis() {
        let v = View { hide_deferred: true, ..View::default() };
        assert!(!v.shows("todo", true, &here()));
        assert!(v.shows("todo", false, &here()), "미룸을 숨겨도 안 미룬 칸은 그대로다");
    }

    #[test]
    fn a_column_the_config_adds_later_stays_visible() {
        assert!(View::hiding("done").shows("blocked", false, &here()));
    }

    /// **숨김은 이 프로젝트의 칸에만 건다**(moai-2kyl 단계 리뷰). 다른 프로젝트에서 숨긴 칸 이름은 여기서 줄을
    /// 숨기지도, 모두 보이기에 걷히지도 않는다 — 그 칸이 있는 프로젝트로 돌아가면 다시 숨는다.
    #[test]
    fn a_name_this_project_lacks_neither_hides_nor_is_cleared() {
        let lacks: Vec<String> = ["todo", "done"].map(String::from).to_vec();
        let mut v = View { hidden: vec!["blocked".into(), "done".into()], hide_deferred: true };
        assert!(v.shows("blocked", false, &lacks), "이 프로젝트에 없는 칸 이름이 줄을 숨겼다");
        assert!(!v.shows("done", false, &lacks));
        v.show_all(&lacks);
        assert_eq!(v, View { hidden: vec!["blocked".into()], hide_deferred: false });
    }

    #[test]
    fn the_badge_names_what_is_hidden() {
        let known: Vec<String> = ["todo", "review", "done"].map(String::from).to_vec();
        assert_eq!(View::default().badge(&known), None);
        assert_eq!(View::hiding("done").badge(&known).as_deref(), Some("done 숨김"));
        let v = View { hidden: vec!["review".into(), "done".into()], hide_deferred: true };
        assert_eq!(v.badge(&known).as_deref(), Some("review·done·미룸 숨김"));
        // 다른 프로젝트의 칸 이름은 들고만 있고 대지 않는다.
        let elsewhere = View { hidden: vec!["blocked".into(), "done".into()], hide_deferred: false };
        assert_eq!(elsewhere.badge(&known).as_deref(), Some("done 숨김"));
        assert_eq!(View::hiding("blocked").badge(&known), None);
    }

    #[test]
    fn field_names_round_trip() {
        for f in Field::ALL {
            assert_eq!(Field::named(f.name()), Some(f));
        }
        assert_eq!(Field::named("우선순위"), None, "화면 낱말을 설정 이름으로 받았다");
    }
}
