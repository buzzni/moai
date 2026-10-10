//! `moai init` 이 처음 심을 때 고르는 것들(moai-zynt.a7y). **순수하다** — 터미널도 파일도 모른다.
//!
//! 길은 하나다: 플래그와 선택 상자가 저마다 [`Choices`] 를 만들고, 같은 [`resolve`] 를 지나 [`Plan`] 이
//! 된다. `init::run` 은 `Plan` 만 받는다 — 그래서 화면으로 고른 것과 플래그로 준 것이 다른 결과를 낼
//! 길이 구조로 없다.
//!
//! **저장하는 것은 사람이 고른 값뿐이다.** 칸마다 `Option` 이고 `None` 은 "기본값을 따른다" 다.
//! 기본값·보이는가·잠겼는가는 고른 값에서 그때마다 다시 셈한다 — 파생값을 저장하지 않는다는 저장소의
//! 규약을 화면에도 그대로 건다.
//!
//! **칸 사이의 규칙은 한 방향으로만 흐른다**: 추적 → 안내 → 설치 → 선택 스킬 → 드라이버(moai-zynt.a7y 노트,
//! 2026-10-06 사용자 결정. 선택 스킬은 moai-3r7l.ocn 이 설치 바로 뒤에 넣었다 — 설치 칸만 읽는다). 앞 칸은 뒤 칸의 기본값·보임·잠김을 정할 수 있고 뒤 칸은 앞 칸을 못
//! 건드린다 — 그래서 [`resolve`] 는 그 차례로 한 번 훑으면 답이 난다. 거스르는 규칙이 필요해지면
//! 그때가 설계를 다시 볼 때다.
//!
//! 사람이 고른 값이 앞 칸 때문에 무효가 되면 **지우지 않는다**. 화면은 그 칸을 숨기거나 잠그고 실제
//! 값은 `resolve` 가 정한다 — 앞 칸을 되돌리면 고른 값이 그대로 돌아온다.

use crate::skill::OPTIONAL;
use clap::ValueEnum;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// 선택 스킬의 수([`crate::skill::OPTIONAL`]) — 칸 하나가 스킬 하나다.
const N: usize = OPTIONAL.len();

/// 트래커를 git 이 추적하는가(moai-zynt.own).
///
/// **`--tracking` 의 값은 clap 이 이 열거형에서 푼다**(moai-8gwh.86j) — 낱말 목록을 손으로 적고 `parse` 로 다시
/// 풀던 판은 두 목록이 갈리면 맞지 않는 값을 말없이 버렸다. 플래그의 낱말은 [`Tracking::word`] 와 같다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Tracking {
    /// 커밋해서 공유한다 — 지금까지의 `init` 이다.
    Commit,
    /// 이 클론에만 둔다. 무시 규칙을 `.git/info/exclude` 에 적는다 — 커밋되는 파일이 하나도 안 바뀐다.
    Exclude,
    /// 이 클론에만 둔다. 무시 규칙을 `.gitignore` 에 적는다 — 그 줄은 커밋되어 남에게도 보인다.
    Gitignore,
}

impl Tracking {
    pub const ALL: [Tracking; 3] = [Tracking::Commit, Tracking::Exclude, Tracking::Gitignore];

    /// 플래그와 `--json` 이 쓰는 낱말.
    pub fn word(self) -> &'static str {
        match self {
            Tracking::Commit => "commit",
            Tracking::Exclude => "exclude",
            Tracking::Gitignore => "gitignore",
        }
    }

    pub fn tracked(self) -> bool {
        self == Tracking::Commit
    }
}

/// 에이전트에게 moai 를 어떻게 알리는가(moai-cbfz). `--guide` 의 값도 [`Tracking`] 처럼 clap 이 푼다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Guide {
    /// AGENTS.md 에 안내 전문을 블록으로 쓴다 — 지금까지의 `init` 이다.
    Block,
    /// 안내 전문은 `.moai/guide.md` 에 쓰고 AGENTS.md 에는 그것을 가리키는 몇 줄만 둔다.
    File,
    /// AGENTS.md 를 안 건드리고 Claude Code 의 훅·스킬로 알린다(moai-6pld) — 첫 프롬프트에 보드와 함께
    /// "사용법은 moai 스킬에 있다" 를 싣는다. 훅·스킬이 깔려야 서는 값이라 설치 칸을 켠 채 잠근다.
    Hook,
    /// AGENTS.md 를 안 건드린다(`--no-agents`).
    None,
}

impl Guide {
    pub const ALL: [Guide; 4] = [Guide::Block, Guide::File, Guide::Hook, Guide::None];

    /// 플래그와 `--json` 이 쓰는 낱말.
    pub fn word(self) -> &'static str {
        match self {
            Guide::Block => "block",
            Guide::File => "file",
            Guide::Hook => "hook",
            Guide::None => "none",
        }
    }
}

/// 사람이 고른 값. `None` 은 고르지 않았다 — 기본값을 따른다.
///
/// **플래그도 이 꼴로 온다**(moai-8gwh.86j) — 따로 두던 `Flags` 는 칸이 같은데 이름만 달라(`register` 대
/// `project`) 다리(`from_flags`)가 하나 더 섰다. 칸이 늘면 세 자리를 고쳐야 했다. 끄는 플래그(`--no-*`)는
/// `Some(false)` 고, 안 준 플래그는 고르지 않은 것(`None`)이다.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Choices {
    /// id 접두어. `None` 이면 `run` 이 디렉터리 이름에서 짓는다 — 파일 시스템을 알아야 해서 여기서
    /// 셈하지 않는다.
    pub prefix: Option<String>,
    pub tracking: Option<Tracking>,
    /// `--guide`, 그리고 `--no-agents` 는 `--guide none` 이다.
    pub guide: Option<Guide>,
    /// 끝에 `moai skill install --scope local` 을 부르는가(moai-785q). `--skill`·`--no-skill`.
    pub skill: Option<bool>,
    /// `.git/config` 에 머지 드라이버를 심는가. `--driver`·`--no-driver` — 칸마다 짝 플래그가 선다. 켜는 쪽이
    /// 없던 판은 커밋하는 트래커에서 이 칸만 플래그로 못 잠가, 모든 칸에 플래그를 줘도 화면이 열렸다(리뷰
    /// moai-zynt.63u).
    pub driver: Option<bool>,
    /// 끝에 `moai project add` 로 사용자 설정의 프로젝트 목록에 올리는가(moai-785q). `--register`·`--no-register`.
    pub project: Option<bool>,
    /// 선택 스킬([`crate::skill::OPTIONAL`] 의 차례)마다 심는가(moai-3r7l.ocn). `--with <이름>` 이 `Some(true)` 다 —
    /// 끄는 플래그는 없다(빼는 것은 `moai skill install --without`). 칸은 설치 칸이 켜졌을 때만 선다.
    pub optional: [Option<bool>; N],
}

impl Choices {
    /// `self` 가 고른 칸은 `self` 의 것, 나머지는 `under` 의 것. 플래그(`self`)를 화면(`under`) 위에 얹는다.
    pub fn over(&self, under: &Choices) -> Choices {
        Choices {
            prefix: self.prefix.clone().or_else(|| under.prefix.clone()),
            tracking: self.tracking.or(under.tracking),
            guide: self.guide.or(under.guide),
            skill: self.skill.or(under.skill),
            driver: self.driver.or(under.driver),
            project: self.project.or(under.project),
            optional: std::array::from_fn(|i| self.optional[i].or(under.optional[i])),
        }
    }

    /// 묻지 않아도 되는가 — 화면에 서는 칸이 모두 골라졌다. **접두어도 센다**: 나중에 못 바꾸는 값이라
    /// 기본값(디렉터리 이름)을 그대로 쓰더라도 한 번 보이는 값어치가 있다. 숨는 칸은 안 센다 — 추적하지
    /// 않으면 드라이버는 묻지 않는다.
    pub fn complete(&self, d: &Defaults) -> bool {
        let plan = resolve(self, d);
        FIELDS.iter().filter(|f| shown(**f, &plan)).all(|f| match f {
            Field::Prefix => self.prefix.is_some(),
            Field::Tracking => self.tracking.is_some(),
            Field::Guide => self.guide.is_some(),
            // 훅으로 알리면 설치는 이미 정해졌다 — 물을 것이 없다.
            Field::Skill => self.skill.is_some() || plan.guide == Guide::Hook,
            Field::Driver => self.driver.is_some(),
            Field::Project => self.project.is_some(),
            // **선택 스킬 칸은 혼자 화면을 열지 않는다** — 다른 칸을 다 플래그로 준 사람은 묻지 말라는 것이고, 그때 심는
            // 것은 `--with` 와 이미 심긴 것뿐이다([`PLAIN`]). 화면이 서면 그 칸도 함께 묻는다.
            Field::Optional(_) => true,
        })
    }
}

/// 고르지 않은 칸이 따를 값.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Defaults {
    pub tracking: Tracking,
    /// 커밋할 때의 안내. 앞 칸(추적)이 이 칸의 기본을 고른다.
    pub guide_tracked: Guide,
    /// 이 클론에만 둘 때의 안내 — 커밋되는 AGENTS.md 에 이 클론에만 있는 파일을 가리키게 하지 않는다.
    pub guide_local: Guide,
    pub skill: bool,
    pub driver: bool,
    pub project: bool,
    /// 선택 스킬마다 미리 골라 둔 값 — **`None` 은 지금 심긴 그대로 둔다**(`skill install` 을 맨으로 부른다). 화면의 값은
    /// `init` 이 그때마다 채운다: 이미 심긴 것과 쓰는 사람이라는 표식이 선 것([`crate::skill::detected`])이 켜진다.
    /// 상수 둘은 채우지 않는다 — 사람이 안 보는 `init` 은 표식으로 심지 않는다(moai-3r7l, 사용자 결정).
    pub optional: [Option<bool>; N],
}

/// 선택 상자가 미리 골라 두는 값. 추적은 안 한다 — 이 클론에만 두고 커밋되는 파일을 하나도 안
/// 바꾸는 쪽이 기본이다(2026-10-06 사용자 결정, moai-zynt.own).
///
/// 안내는 커밋하면 별도 파일과 링크(moai-cbfz), 이 클론에만 두면 AGENTS.md 를 안 건드리고 훅으로 알린다
/// (사용자 결정, moai-6pld).
pub const SCREEN: Defaults = Defaults {
    tracking: Tracking::Exclude,
    guide_tracked: Guide::File,
    guide_local: Guide::Hook,
    skill: true,
    driver: true,
    project: true,
    optional: [Some(false); N],
};

/// 터미널이 아닌 곳(에이전트·스크립트)과 `--yes` 의 값. **지금까지의 `init` 과 바이트째 같아야 한다**
/// (2026-10-06 사용자 결정) — 에이전트가 부르던 결과를 이 묶음이 지킨다. 화면의 기본값이 달라져도
/// 이쪽은 안 따라간다.
///
/// 이 클론에만 두라고 플래그로만 준 것은 AGENTS.md 를 안 건드리는 데서 멈춘다 — 훅을 기본으로 걸면 스크립트가
/// 부른 `init` 이 `claude` 를 불러 플러그인을 깐다. 원하면 `--guide hook` 을 준다.
pub const PLAIN: Defaults = Defaults {
    tracking: Tracking::Commit,
    guide_tracked: Guide::Block,
    guide_local: Guide::None,
    skill: false,
    driver: true,
    project: false,
    optional: [None; N],
};

/// 빈칸 없는 계획. `init::run` 은 이것만 받고 판단하지 않는다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// `None` 이면 디렉터리 이름에서 짓는다(위 [`Choices::prefix`]).
    pub prefix: Option<String>,
    pub tracking: Tracking,
    pub guide: Guide,
    pub skill: bool,
    pub driver: bool,
    pub project: bool,
    /// `skill install --with` 에 건넬 선택 스킬 — 설치를 안 하면 비었다.
    pub with: Vec<&'static str>,
    /// `skill install --without` 에 건넬 선택 스킬 — **미리 켜 둔 것을 끈 것만이다.** 미리 안 켠 것은 심긴 것이 아니라
    /// 걷을 것이 없다. 미리 켜 둔 까닭이 표식뿐이면(심기지 않았다) 이것도 걷을 것이 없어 아무것도 안 바뀐다. 설치를 안 하면
    /// 비었다.
    pub without: Vec<&'static str>,
}

impl Plan {
    /// 고른 선택 스킬이 지금 심긴 것(`planted`)을 바꾸는가 — 심기지 않은 것을 켰거나, 켜 둔 것을 껐다. 이미 선 훅으로 읽은
    /// 안내에서 `init` 이 설치를 거둘 때 이것이 참이면 거두지 않는다(리뷰 moai-3r7l) — 그 칸은 설치 칸이 켜졌을 때 서서
    /// "함께 심는다" 를 보이는데, 설치를 거두면 고른 값이 말없이 버려진다.
    pub fn changes_optional(&self, planted: &[&str]) -> bool {
        !self.without.is_empty() || self.with.iter().any(|n| !planted.contains(n))
    }
}

/// 고른 값과 기본값으로 계획을 낸다. 머리글의 차례(추적 → 안내 → 설치 → 드라이버)로 적는다 — 앞 칸을
/// 먼저 정하고, 뒤 칸은 이미 정한 앞 칸만 읽는다.
pub fn resolve(c: &Choices, d: &Defaults) -> Plan {
    let tracking = c.tracking.unwrap_or(d.tracking);
    let guide = c.guide.unwrap_or(if tracking.tracked() { d.guide_tracked } else { d.guide_local });
    // 훅으로 알리면 훅이 깔려야 한다 — 설치를 켠다. 끄는 플래그와 부딪히면 [`conflict`] 가 잡는다.
    let skill = guide == Guide::Hook || c.skill.unwrap_or(d.skill);
    // 추적하지 않으면 머지 드라이버는 할 일이 없다 — git 이 그 파일을 병합할 일이 없다. 고른 값은 두고 끈다.
    let driver = tracking.tracked() && c.driver.unwrap_or(d.driver);
    let project = c.project.unwrap_or(d.project);
    // 선택 스킬은 설치 칸만 읽는다 — 설치를 안 하면 건넬 것이 없다.
    let (mut with, mut without) = (Vec::new(), Vec::new());
    for (i, o) in OPTIONAL.iter().enumerate().filter(|_| skill) {
        match (c.optional[i], d.optional[i]) {
            (Some(true), _) | (None, Some(true)) => with.push(o.name),
            (Some(false), Some(true)) => without.push(o.name),
            _ => {}
        }
    }
    Plan { prefix: c.prefix.clone(), tracking, guide, skill, driver, project, with, without }
}

/// 플래그로 준 것이 계획과 부딪히는가 — 뒤 칸에 준 플래그가 앞 칸이 정한 것을 뒤집으려 할 때다. 화면은 그 칸을
/// 잠가 못 고르게 하지만 플래그는 이미 와 있다. **조용히 한쪽을 이기게 하지 않는다**: 부르는 쪽이 거절한다.
pub fn conflict(fixed: &Choices, plan: &Plan) -> Option<Conflict> {
    if fixed.skill == Some(false) && plan.guide == Guide::Hook {
        Some(Conflict::HookWithoutSkill)
    } else if fixed.driver == Some(true) && !plan.tracking.tracked() {
        Some(Conflict::DriverWithoutTracking)
    } else {
        None
    }
}

/// [`conflict`] 의 갈래.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conflict {
    /// `--no-skill` 인데 안내가 훅이다.
    HookWithoutSkill,
    /// `--driver` 는 git 밖의 트래커에 심을 수 없다.
    DriverWithoutTracking,
}

/// 화면의 한 칸.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Prefix,
    Tracking,
    Guide,
    Skill,
    /// 선택 스킬 하나 — [`crate::skill::OPTIONAL`] 의 자리다(moai-3r7l.ocn).
    Optional(usize),
    Driver,
    Project,
}

/// 화면에 서는 차례. 규칙의 차례(머리글)와 맞춘다 — 앞 칸을 고르면 아래 칸이 바뀌는 쪽이 읽기 쉽다. 선택 스킬은 그것을
/// 세우는 설치 칸 바로 밑이다.
pub const FIELDS: [Field; 6 + N] = fields();

const fn fields() -> [Field; 6 + N] {
    let mut out = [Field::Prefix; 6 + N];
    (out[1], out[2], out[3]) = (Field::Tracking, Field::Guide, Field::Skill);
    let mut i = 0;
    while i < N {
        out[4 + i] = Field::Optional(i);
        i += 1;
    }
    (out[4 + N], out[5 + N]) = (Field::Driver, Field::Project);
    out
}

/// 이 계획에서 그 칸이 서는가. 숨는 칸은 앞 칸이 이미 답을 정한 칸이다 — 설치를 안 하면 고를 선택 스킬이 없다.
pub fn shown(f: Field, plan: &Plan) -> bool {
    match f {
        Field::Driver => plan.tracking.tracked(),
        Field::Optional(_) => plan.skill,
        Field::Prefix | Field::Tracking | Field::Guide | Field::Skill | Field::Project => true,
    }
}

/// 칸을 잠근 것.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lock {
    /// 플래그로 줬다.
    Flag,
    /// 안내가 훅이라 설치가 켜진 채 정해졌다.
    Hook,
}

/// 키 하나를 받은 뒤 부르는 쪽이 할 일.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    /// 화면에 머문다.
    Stay,
    /// 심는다. 접두어를 재는 것은 부르는 쪽이다 — 틀렸으면 [`Form::problem`] 을 세우고 머문다.
    Plant,
    /// 아무것도 안 쓰고 그만둔다.
    Stop,
}

/// 선택 상자의 상태. **조각이다**(`tui/picker.rs` 꼴) — 키를 받아 무엇을 할지만 돌려준다. 그리는 쪽과
/// 심는 쪽은 부르는 쪽(`init`)이다.
#[derive(Debug, Clone)]
pub struct Form {
    /// 플래그가 고른 것. 이 칸들은 잠긴다.
    pub fixed: Choices,
    /// 화면에서 고른 것.
    pub picked: Choices,
    pub defaults: Defaults,
    /// 디렉터리 이름에서 지은 접두어 — 접두어 칸이 처음 보이는 글이다.
    pub suggested: String,
    /// 접두어 칸에 지금 적힌 글.
    pub typed: String,
    /// 커서가 선 칸. 칸이 숨어도 칸으로 들고 있다 — 줄 번호로 들면 위 칸이 숨을 때 엉뚱한 칸으로 옮겨 간다.
    pub cursor: Field,
    /// 심으려다 멈춘 까닭(접두어·git 밖·부딪힘). **어느 칸이든 고치면 걷는다**(리뷰 moai-zynt.63u) — 접두어를
    /// 고칠 때만 걷던 판은 안내 칸을 바꿔 부딪힘을 풀어도 옛 까닭이 다음 Enter 까지 남았다.
    pub problem: Option<String>,
}

impl Form {
    pub fn new(fixed: Choices, defaults: Defaults, suggested: String) -> Form {
        let typed = fixed.prefix.clone().unwrap_or_else(|| suggested.clone());
        let mut form = Form {
            fixed,
            picked: Choices::default(),
            defaults,
            suggested,
            typed,
            cursor: Field::Prefix,
            problem: None,
        };
        // 고를 수 있는 첫 칸에 선다 — 플래그로 접두어를 줬으면 그 칸에 서 봐야 고칠 것이 없다.
        if let Some(f) = form.rows().into_iter().find(|f| !form.locked(*f)) {
            form.cursor = f;
        }
        form
    }

    /// 잠긴 칸인가, 무엇이 잠갔나.
    pub fn locked_by(&self, f: Field) -> Option<Lock> {
        let fixed = match f {
            Field::Prefix => self.fixed.prefix.is_some(),
            Field::Tracking => self.fixed.tracking.is_some(),
            Field::Guide => self.fixed.guide.is_some(),
            Field::Skill => self.fixed.skill.is_some(),
            Field::Driver => self.fixed.driver.is_some(),
            Field::Project => self.fixed.project.is_some(),
            Field::Optional(i) => self.fixed.optional[i].is_some(),
        };
        if fixed {
            return Some(Lock::Flag);
        }
        (f == Field::Skill && self.plan().guide == Guide::Hook).then_some(Lock::Hook)
    }

    pub fn locked(&self, f: Field) -> bool {
        self.locked_by(f).is_some()
    }

    /// 지금 서는 칸들, 위에서부터.
    pub fn rows(&self) -> Vec<Field> {
        let plan = self.plan();
        FIELDS.into_iter().filter(|f| shown(*f, &plan)).collect()
    }

    /// 지금 고른 것 — 플래그를 화면 위에 얹는다. 접두어는 적힌 글이 지은 값과 같으면 `None` 으로 둔다:
    /// `run` 이 그때만 "디렉터리 이름이 길어 줄였다" 를 알린다.
    pub fn choices(&self) -> Choices {
        let mut picked = self.picked.clone();
        if self.typed != self.suggested {
            picked.prefix = Some(self.typed.clone());
        }
        self.fixed.over(&picked)
    }

    pub fn plan(&self) -> Plan {
        resolve(&self.choices(), &self.defaults)
    }

    /// 고르는 칸의 선택지 수와 지금 선 자리. 접두어 칸은 고르는 칸이 아니다(`None`).
    pub fn option(&self, f: Field) -> Option<(usize, usize)> {
        let plan = self.plan();
        match f {
            Field::Prefix => None,
            Field::Tracking => Some((Tracking::ALL.len(), Tracking::ALL.iter().position(|t| *t == plan.tracking)?)),
            Field::Guide => Some((Guide::ALL.len(), Guide::ALL.iter().position(|g| *g == plan.guide)?)),
            // 켜는 값이 왼쪽(0), 끄는 값이 오른쪽(1)에 그려진다. **플래그로 잠긴 설치 칸은 플래그의 값을 그린다**(리뷰
            // moai-zynt.63u) — `--no-skill` 에 훅 안내를 고르면 계획은 설치를 켜고 Enter 가 부딪힘을 대는데, 그 칸이
            // "설치 · 플래그로 정함" 으로 서면 화면이 플래그와 거꾸로 말한다.
            Field::Skill => Some((2, usize::from(!self.fixed.skill.unwrap_or(plan.skill)))),
            Field::Driver => Some((2, usize::from(!plan.driver))),
            Field::Project => Some((2, usize::from(!plan.project))),
            // 켜졌는가는 계획이 아니라 고른 값에서 읽는다 — 계획은 미리 켜 둔 것을 끈 것만 `without` 에 든다.
            Field::Optional(i) => {
                let on = self.choices().optional[i].or(self.defaults.optional[i]).unwrap_or(false);
                Some((2, usize::from(!on)))
            }
        }
    }

    fn pick(&mut self, f: Field, at: usize) {
        match f {
            Field::Prefix => {}
            Field::Tracking => self.picked.tracking = Some(Tracking::ALL[at]),
            Field::Guide => self.picked.guide = Some(Guide::ALL[at]),
            Field::Skill => self.picked.skill = Some(at == 0),
            Field::Driver => self.picked.driver = Some(at == 0),
            Field::Project => self.picked.project = Some(at == 0),
            Field::Optional(i) => self.picked.optional[i] = Some(at == 0),
        }
    }

    pub fn key(&mut self, k: KeyEvent) -> Act {
        if k.modifiers.contains(KeyModifiers::CONTROL) {
            return match k.code {
                KeyCode::Char('c') | KeyCode::Char('d') => Act::Stop,
                _ => Act::Stay,
            };
        }
        match k.code {
            KeyCode::Esc => return Act::Stop,
            KeyCode::Enter => return Act::Plant,
            KeyCode::Up | KeyCode::BackTab => self.step(-1),
            KeyCode::Down | KeyCode::Tab => self.step(1),
            _ => self.edit(k.code),
        }
        Act::Stay
    }

    /// 잠긴 칸을 건너 옮긴다. 끝에서 멈춘다 — 감아 돌면 몇 칸 안 되는 화면에서 어디 섰는지 잃는다.
    fn step(&mut self, by: isize) {
        let rows = self.rows();
        let Some(mut at) = rows.iter().position(|f| *f == self.cursor).map(|i| i as isize) else {
            return;
        };
        loop {
            at += by;
            if at < 0 || at >= rows.len() as isize {
                return;
            }
            if !self.locked(rows[at as usize]) {
                self.cursor = rows[at as usize];
                return;
            }
        }
    }

    fn edit(&mut self, code: KeyCode) {
        let f = self.cursor;
        if self.locked(f) {
            return;
        }
        let Some((n, at)) = self.option(f) else {
            match code {
                KeyCode::Backspace => {
                    self.typed.pop();
                }
                // 받는 글자를 거르지 않는다 — 틀린 글자는 심을 때 까닭과 함께 잰다. 여기서 말없이
                // 삼키면 대문자를 친 사람은 왜 안 들어가는지 모른다.
                KeyCode::Char(c) if !c.is_control() => self.typed.push(c),
                _ => return,
            }
            self.problem = None;
            return;
        };
        // 화살표는 그 쪽으로 한 칸 가고 끝에서 멈춘다. 띄어쓰기는 감아 돈다.
        let to = match code {
            KeyCode::Left => at.saturating_sub(1),
            KeyCode::Right => (at + 1).min(n - 1),
            KeyCode::Char(' ') => (at + 1) % n,
            _ => return,
        };
        self.pick(f, to);
        self.problem = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyEventKind;

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new_with_kind(code, KeyModifiers::NONE, KeyEventKind::Press)
    }

    fn form(fixed: Choices) -> Form {
        Form::new(fixed, SCREEN, "moai".to_string())
    }

    /// 키로 칸에 가서 그 값을 고른다.
    fn choose(f: &mut Form, field: Field, at: usize) {
        while f.cursor != field {
            let was = f.cursor;
            f.key(press(KeyCode::Down));
            assert_ne!(f.cursor, was, "{field:?} 칸에 못 간다");
        }
        for _ in 0..4 {
            f.key(press(KeyCode::Left));
        }
        for _ in 0..at {
            f.key(press(KeyCode::Right));
        }
    }

    /// 플래그의 모든 조합이 화면에서 키로 고른 것과 같은 계획을 낸다 — 길이 하나라는 약속을 잰다.
    #[test]
    fn a_flag_and_the_same_pick_on_screen_make_one_plan() {
        for tracking in Tracking::ALL {
            for guide in Guide::ALL {
                for no_driver in [false, true] {
                    for prefix in [None, Some("abc")] {
                        let flags = Choices {
                            prefix: prefix.map(str::to_string),
                            tracking: Some(tracking),
                            guide: Some(guide),
                            driver: no_driver.then_some(false),
                            skill: Some(true),
                            project: Some(false),
                            optional: [None; N],
                        };
                        let by_flags = resolve(&flags, &SCREEN);
                        let mut f = form(Choices::default());
                        if let Some(p) = prefix {
                            f.typed.clear();
                            p.chars().for_each(|c| _ = f.key(press(KeyCode::Char(c))));
                        }
                        // 추적을 켜 드라이버 칸을 세우고 그것부터 고른다 — 추적을 끄면 그 칸이 숨는다.
                        choose(&mut f, Field::Tracking, 0);
                        choose(&mut f, Field::Driver, usize::from(no_driver));
                        f.cursor = Field::Prefix;
                        choose(&mut f, Field::Project, 1);
                        f.cursor = Field::Prefix;
                        choose(&mut f, Field::Guide, Guide::ALL.iter().position(|g| *g == guide).unwrap());
                        f.cursor = Field::Prefix;
                        choose(&mut f, Field::Tracking, Tracking::ALL.iter().position(|t| *t == tracking).unwrap());
                        assert_eq!(f.plan(), by_flags, "{flags:?}");
                    }
                }
            }
        }
    }

    /// **플래그가 받는 낱말은 `word` 가 내는 낱말과 같다**(moai-8gwh.86j) — 워크트리 거절문이 `word` 로 명령줄을
    /// 되살리고 `--json` 이 그것을 싣는다. clap 이 푸는 이름이 갈리면 되살린 줄을 따라 친 사람이 거절당한다.
    #[test]
    fn the_flag_words_are_the_words_the_tool_prints() {
        let names = |vs: Vec<clap::builder::PossibleValue>| vs.iter().map(|v| v.get_name().to_string()).collect();
        let tracking: Vec<String> = names(Tracking::ALL.iter().filter_map(|t| t.to_possible_value()).collect());
        assert_eq!(tracking, Tracking::ALL.map(|t| t.word().to_string()));
        assert_eq!(tracking, ["commit", "exclude", "gitignore"]);
        let guide: Vec<String> = names(Guide::ALL.iter().filter_map(|g| g.to_possible_value()).collect());
        assert_eq!(guide, Guide::ALL.map(|g| g.word().to_string()));
        assert_eq!(guide, ["block", "file", "hook", "none"]);
        assert_eq!(Tracking::value_variants(), Tracking::ALL);
        assert_eq!(Guide::value_variants(), Guide::ALL);
    }

    /// 터미널이 아닌 `init` 은 지금까지와 같다 — 커밋하고, 안내 블록도 드라이버도 심는다.
    #[test]
    fn plain_defaults_keep_what_init_always_did() {
        assert_eq!(
            resolve(&Choices::default(), &PLAIN),
            Plan {
                prefix: None,
                tracking: Tracking::Commit,
                guide: Guide::Block,
                skill: false,
                driver: true,
                project: false,
                with: Vec::new(),
                without: Vec::new(),
            }
        );
    }

    /// 화면의 기본은 추적하지 않는 것이다(사용자 결정) — 그러면 드라이버는 꺼지고 칸도 숨는다.
    #[test]
    fn the_screen_keeps_the_tracker_out_of_git_and_hides_the_driver() {
        let f = form(Choices::default());
        assert_eq!(f.plan().tracking, Tracking::Exclude);
        assert!(!f.plan().driver);
        assert!(!f.rows().contains(&Field::Driver));
    }

    /// 사람이 고른 값은 앞 칸이 무효로 만들어도 남는다 — 되돌리면 그대로 돌아온다.
    #[test]
    fn a_pick_hidden_by_an_earlier_row_comes_back_with_it() {
        let mut f = form(Choices::default());
        choose(&mut f, Field::Tracking, 0);
        choose(&mut f, Field::Driver, 1);
        assert!(!f.plan().driver);
        f.cursor = Field::Prefix;
        choose(&mut f, Field::Tracking, 1);
        assert!(!f.rows().contains(&Field::Driver));
        assert_eq!(f.picked.driver, Some(false), "hidden, not forgotten");
        // 켜 두고 숨긴 것도 숨은 동안은 꺼진다.
        f.picked.driver = Some(true);
        assert!(!f.plan().driver);
        f.cursor = Field::Prefix;
        choose(&mut f, Field::Tracking, 0);
        assert!(f.plan().driver);
    }

    /// 칸 사이 규칙은 모든 조합에서 선다 — 추적하지 않으면 드라이버는 없다.
    #[test]
    fn no_driver_without_tracking_in_any_combination() {
        for tracking in [None].into_iter().chain(Tracking::ALL.map(Some)) {
            for guide in [None].into_iter().chain(Guide::ALL.map(Some)) {
                for driver in [None, Some(true), Some(false)] {
                    for d in [SCREEN, PLAIN] {
                        for skill in [None, Some(true), Some(false)] {
                            let c = Choices { prefix: None, tracking, guide, skill, driver, ..Choices::default() };
                            let plan = resolve(&c, &d);
                            assert!(plan.tracking.tracked() || !plan.driver, "{c:?} {d:?}");
                            assert!(plan.guide != Guide::Hook || plan.skill, "hooks without the skill: {c:?} {d:?}");
                            // 설치를 끈 훅 안내와, git 밖인데 드라이버를 켠 플래그는 거절한다.
                            let clash = conflict(&c, &plan).is_some();
                            assert_eq!(
                                clash,
                                skill == Some(false) && plan.guide == Guide::Hook
                                    || driver == Some(true) && !plan.tracking.tracked(),
                                "{c:?} {d:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn a_flag_locks_its_row_and_the_cursor_skips_it() {
        let mut f = form(Choices { tracking: Some(Tracking::Commit), ..Choices::default() });
        assert!(f.locked(Field::Tracking));
        f.key(press(KeyCode::Down));
        assert_eq!(f.cursor, Field::Guide, "the locked row is stepped over");
        f.key(press(KeyCode::Up));
        assert_eq!(f.cursor, Field::Prefix);
        // 잠긴 칸은 키로 못 바꾼다.
        f.cursor = Field::Tracking;
        f.key(press(KeyCode::Right));
        assert_eq!(f.plan().tracking, Tracking::Commit);
    }

    #[test]
    fn a_prefix_left_as_suggested_stays_unchosen() {
        let mut f = form(Choices::default());
        assert_eq!(f.choices().prefix, None);
        f.key(press(KeyCode::Backspace));
        f.key(press(KeyCode::Char('i')));
        assert_eq!(f.choices().prefix, None, "typed back to the suggestion");
        f.key(press(KeyCode::Char('x')));
        assert_eq!(f.choices().prefix.as_deref(), Some("moaix"));
    }

    /// 접두어가 아닌 칸을 고쳐도 멈춘 까닭을 걷는다 — 그 까닭은 이제 그 칸의 것일 수 있다(부딪힘·git 밖).
    #[test]
    fn picking_another_row_clears_the_problem() {
        let mut f = form(Choices::default());
        f.problem = Some("hooks need the skill".into());
        choose(&mut f, Field::Guide, 3);
        assert_eq!(f.problem, None);
    }

    /// 칸마다 짝 플래그가 선다 — `--driver` 로 드라이버 칸까지 잠그면 커밋하는 트래커도 묻지 않는다.
    #[test]
    fn every_row_that_stands_can_be_pinned_by_a_flag() {
        let all = Choices {
            prefix: Some("abc".into()),
            tracking: Some(Tracking::Commit),
            guide: Some(Guide::Block),
            driver: Some(true),
            skill: Some(false),
            project: Some(false),
            optional: [None; N],
        };
        assert!(all.complete(&SCREEN));
        assert!(!Choices { driver: None, ..all }.complete(&SCREEN));
    }

    #[test]
    fn editing_the_prefix_clears_the_problem() {
        let mut f = form(Choices::default());
        f.problem = Some("bad".into());
        f.key(press(KeyCode::Backspace));
        assert_eq!(f.problem, None);
    }

    #[test]
    fn enter_plants_esc_and_ctrl_c_stop() {
        let mut f = form(Choices::default());
        assert_eq!(f.key(press(KeyCode::Enter)), Act::Plant);
        assert_eq!(f.key(press(KeyCode::Esc)), Act::Stop);
        assert_eq!(f.key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)), Act::Stop);
    }

    /// 안내의 기본은 앞 칸(추적)이 고른다 — 커밋하면 별도 파일, 이 클론에만 두면 AGENTS.md 를 안 건드린다.
    /// 사람이 고른 안내는 추적을 바꿔도 남는다.
    #[test]
    fn the_guide_default_follows_tracking_and_a_pick_stays() {
        let mut f = form(Choices::default());
        assert_eq!(f.plan().guide, Guide::Hook);
        choose(&mut f, Field::Tracking, 0);
        assert_eq!(f.plan().guide, Guide::File);
        choose(&mut f, Field::Guide, 0);
        f.cursor = Field::Prefix;
        choose(&mut f, Field::Tracking, 1);
        assert_eq!(f.plan().guide, Guide::Block);
    }

    /// 훅으로 알리면 설치 칸이 켜진 채 잠긴다 — 훅을 거두면 사람이 골랐던 값이 돌아온다.
    #[test]
    fn hooks_lock_the_skill_on_and_let_go_of_it() {
        let mut f = form(Choices::default());
        assert_eq!(f.plan().guide, Guide::Hook);
        assert_eq!(f.locked_by(Field::Skill), Some(Lock::Hook));
        assert!(f.plan().skill);
        choose(&mut f, Field::Guide, 3);
        assert_eq!(f.locked_by(Field::Skill), None);
        choose(&mut f, Field::Skill, 1);
        assert!(!f.plan().skill);
        f.cursor = Field::Guide;
        choose(&mut f, Field::Guide, 2);
        assert!(f.plan().skill, "hooks turn it back on");
        assert_eq!(f.picked.skill, Some(false), "the pick is kept");
    }

    #[test]
    fn space_cycles_and_arrows_stop_at_the_ends() {
        let mut f = form(Choices::default());
        f.cursor = Field::Tracking;
        f.key(press(KeyCode::Right));
        f.key(press(KeyCode::Right));
        f.key(press(KeyCode::Right));
        assert_eq!(f.plan().tracking, Tracking::Gitignore);
        f.key(press(KeyCode::Char(' ')));
        assert_eq!(f.plan().tracking, Tracking::Commit);
    }

    #[test]
    fn complete_counts_only_the_rows_that_stand() {
        let full = |tracking, driver| Choices {
            prefix: Some("a".into()),
            tracking: Some(tracking),
            guide: Some(Guide::Block),
            skill: Some(true),
            driver,
            project: Some(true),
            optional: [None; N],
        };
        assert!(full(Tracking::Exclude, None).complete(&SCREEN), "the driver row is hidden");
        assert!(!full(Tracking::Commit, None).complete(&SCREEN));
        assert!(full(Tracking::Commit, Some(false)).complete(&SCREEN));
        assert!(!Choices::default().complete(&SCREEN));
    }

    /// **선택 스킬 칸은 설치 칸이 켜졌을 때만 선다**(moai-3r7l.ocn) — 설치를 안 하면 고를 것이 없고, 건넬 것도 없다.
    /// 미리 켜 둔 칸(이미 심긴 것, 표식이 선 것)은 그대로 `--with` 로 가고, 그것을 끄면 `--without` 으로 간다. 미리 안 켠
    /// 칸을 그대로 두면 아무것도 안 건넨다 — 심긴 것이 아니라 걷을 것이 없다.
    #[test]
    fn the_optional_rows_stand_under_the_skill_row_and_pass_their_pick_on() {
        let mut f = form(Choices { guide: Some(Guide::None), ..Choices::default() });
        assert!(f.rows().contains(&Field::Optional(0)), "설치 칸이 켜졌는데 선택 스킬 칸이 없다");
        assert_eq!((f.plan().with, f.plan().without), (vec![], vec![]), "미리 안 켠 칸이 무엇을 건넸다");
        choose(&mut f, Field::Optional(0), 0);
        assert_eq!(f.plan().with, ["moai-tmux"]);
        f.cursor = Field::Prefix;
        choose(&mut f, Field::Skill, 1);
        assert!(!f.rows().contains(&Field::Optional(0)), "설치를 껐는데 선택 스킬 칸이 섰다");
        assert_eq!(f.plan().with, Vec::<&str>::new(), "설치를 안 하는데 건넸다");
        assert_eq!(f.picked.optional[0], Some(true), "hidden, not forgotten");

        // 미리 켜 둔 칸 — 끄면 `--without` 이다.
        let pre = Defaults { optional: [Some(true); N], ..SCREEN };
        let mut f = Form::new(Choices { guide: Some(Guide::None), ..Choices::default() }, pre, "moai".into());
        assert_eq!(f.plan().with, ["moai-tmux", "moai-cmux"], "미리 켜 둔 칸을 안 건넸다");
        choose(&mut f, Field::Optional(0), 1);
        assert_eq!((f.plan().with, f.plan().without), (vec!["moai-cmux"], vec!["moai-tmux"]));
    }

    /// **`--with` 는 그 칸을 잠그고, 사람이 안 보는 `init` 은 표식으로 심지 않는다**(moai-3r7l.ocn) — [`PLAIN`] 의 선택
    /// 스킬은 비어 맨 `skill install` 이 이미 심긴 것만 다시 심는다. 선택 스킬 칸은 혼자 화면을 열지 않는다.
    #[test]
    fn a_with_flag_locks_its_row_and_plain_plants_nothing_by_itself() {
        let mut with = Choices { skill: Some(true), ..Choices::default() };
        with.optional[0] = Some(true);
        let f = form(with.clone());
        assert_eq!(f.locked_by(Field::Optional(0)), Some(Lock::Flag));
        assert_eq!(resolve(&with, &PLAIN).with, ["moai-tmux"]);
        let plain = resolve(&Choices { skill: Some(true), ..Choices::default() }, &PLAIN);
        assert_eq!((plain.with, plain.without), (vec![], vec![]), "사람이 안 보는 init 이 선택 스킬을 골랐다");
        let all = Choices {
            prefix: Some("abc".into()),
            tracking: Some(Tracking::Exclude),
            guide: Some(Guide::None),
            skill: Some(true),
            project: Some(false),
            ..Choices::default()
        };
        assert!(all.complete(&SCREEN), "선택 스킬 칸 하나가 화면을 열었다");
    }

    /// **고른 선택 스킬이 심긴 것을 바꾸면 그것이 시킨 설치다**(리뷰 moai-3r7l) — 이미 선 훅으로 읽은 안내에서 `init` 이
    /// 설치를 거둘 때 이것으로 잰다. 심긴 것을 그대로 켜 둔 칸은 바꾼 것이 아니다.
    #[test]
    fn an_optional_pick_that_changes_what_is_planted_asks_for_the_install() {
        let mut f = form(Choices { guide: Some(Guide::None), ..Choices::default() });
        assert!(!f.plan().changes_optional(&[]), "고른 것이 없는데 바꿨다고 읽었다");
        choose(&mut f, Field::Optional(0), 0);
        assert!(f.plan().changes_optional(&[]), "심기지 않은 것을 켠 것을 안 읽었다");
        assert!(!f.plan().changes_optional(&["moai-tmux"]), "심긴 것을 그대로 켜 둔 것을 바꿨다고 읽었다");
        let pre = Defaults { optional: [Some(true); N], ..SCREEN };
        let mut f = Form::new(Choices { guide: Some(Guide::None), ..Choices::default() }, pre, "moai".into());
        choose(&mut f, Field::Optional(0), 1);
        assert!(f.plan().changes_optional(&["moai-tmux"]), "심긴 것을 끈 것을 안 읽었다");
    }
}
