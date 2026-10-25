//! 색과 글리프. 크레이트는 색을 정하지 않는다 — 부르는 쪽이 [`Palette`] 로 넘기고, 기본값은 시안의
//! 16색 흉내일 뿐이다. 색만으로 뜻을 지지 않는다: 아래 모든 색 곁에 [`Glyphs`] 의 글리프나 낱말이 선다.

use ratatui::style::{Color, Modifier, Style};

use crate::axis::HeaderStyle;

/// 줄의 칸(상태). 실제 막대(`█`)의 색을 고른다 — moai 의 `style` 색을 부르는 쪽이 이 넷에 대 준다.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Tone {
    /// 아직 시작 전.
    #[default]
    Todo,
    /// 진행 중.
    Progress,
    /// 리뷰 중.
    Review,
    /// 끝.
    Done,
}

/// 모든 색. 기본값은 시안(`gantt-tui-mockup.html`)의 16색이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// 계획 막대(`░`·`▒`) — 흐린 색.
    pub planned: Style,
    /// 미룬 줄의 계획(`┄`) — 계획보다 더 흐리다.
    pub deferred: Style,
    pub todo: Style,
    pub progress: Style,
    pub review: Style,
    pub done: Style,
    /// 기한 넘김(`▓…!`).
    pub overdue: Style,
    /// 기한 표식(`◆`).
    pub due: Style,
    /// 오늘 선(`┊`)과 머리글의 `▼`.
    pub today: Style,
    /// 머리글 아래 줄에서 오늘이 든 날의 라벨.
    pub today_label: Style,
    /// 화면 밖으로 잘림(`‹`·`›`).
    pub clip: Style,
    /// 주말 바탕(Day·Hour). 바탕만 바꾸는 꾸밈이라 글리프가 없어도 날은 머리글 라벨이 말한다.
    pub weekend: Style,
    /// 의존 화살표(`─┐┤│└▶`)와 표식(`↑`·`↓`)의 자주.
    pub link: Style,
    /// 분할선.
    pub splitter: Style,
    /// 머리글 글자.
    pub header: Style,
    /// 커서 줄의 고정 칸 바탕. 시안은 날짜 칸을 칠하지 않는다.
    pub focus: Style,
}

impl Default for Palette {
    fn default() -> Self {
        let fg = |c| Style::new().fg(c);
        Palette {
            planned: fg(Color::DarkGray),
            deferred: Style::new().fg(Color::DarkGray).add_modifier(Modifier::DIM),
            todo: fg(Color::Gray),
            progress: fg(Color::Yellow),
            review: fg(Color::Magenta),
            done: fg(Color::Green),
            overdue: fg(Color::Red),
            due: Style::new().fg(Color::Blue).add_modifier(Modifier::BOLD),
            today: fg(Color::Red),
            today_label: Style::new().fg(Color::Red).add_modifier(Modifier::BOLD),
            clip: fg(Color::Cyan),
            weekend: Style::new().bg(Color::Indexed(235)),
            link: fg(Color::Magenta),
            splitter: fg(Color::DarkGray),
            header: Style::new(),
            focus: Style::new().bg(Color::Indexed(237)),
        }
    }
}

impl Palette {
    /// 칸에 맞는 실제 막대 색.
    pub fn tone(&self, t: Tone) -> Style {
        match t {
            Tone::Todo => self.todo,
            Tone::Progress => self.progress,
            Tone::Review => self.review,
            Tone::Done => self.done,
        }
    }

    pub(crate) fn header_style(&self) -> HeaderStyle {
        HeaderStyle { label: self.header, weekend: self.weekend, today: self.today, today_label: self.today_label }
    }
}

/// 모든 글리프. [`Glyphs::ascii`] 는 글리프가 모자란 터미널용이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Glyphs {
    /// 이슈의 계획.
    pub planned: char,
    /// 묶음(에픽·마일스톤)의 계획.
    pub planned_group: char,
    /// 미룬 줄의 계획.
    pub deferred: char,
    /// 실제.
    pub actual: char,
    /// 칸의 8분의 1…7 을 왼쪽부터 채우는 글리프. `None` 이면 끝 조각을 안 그리고 반 칸 넘게
    /// 걸친 칸만 채운다.
    pub eighths: Option<[char; 7]>,
    /// 기한 넘김 칸.
    pub overdue: char,
    /// 기한 넘김의 끝(지금).
    pub overdue_end: char,
    /// 기한 표식.
    pub due: char,
    pub clip_left: char,
    pub clip_right: char,
    pub today: char,
    pub splitter: char,
    /// 의존 화살표: 가로·세로·꺾임 넷(`┐┘┌└`)·가지 둘(`┤├`)·화살촉, 그리고 막힌 길의 표식 둘.
    pub link: LinkGlyphs,
}

/// 의존 화살표의 글리프. 팔 모양(왼·오른·위·아래)이 글리프를 고른다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkGlyphs {
    pub h: char,
    pub v: char,
    /// 왼쪽에서 와 아래로 꺾음 `┐`.
    pub down_left: char,
    /// 왼쪽에서 와 위로 꺾음 `┘`.
    pub up_left: char,
    /// 오른쪽으로 나가 아래로 `┌`.
    pub down_right: char,
    /// 오른쪽으로 나가 위로 `└`.
    pub up_right: char,
    /// 세로줄에 왼쪽 가지 `┤`.
    pub tee_left: char,
    /// 세로줄에 오른쪽 가지 `├`.
    pub tee_right: char,
    pub arrow: char,
    /// 길이 막힌 줄의 표식과 화면 밖 개수의 머리. 커서 줄이 아래면 `up`.
    pub up: char,
    pub down: char,
}

impl Default for Glyphs {
    fn default() -> Self {
        Glyphs {
            planned: '░',
            planned_group: '▒',
            deferred: '┄',
            actual: '█',
            eighths: Some(['▏', '▎', '▍', '▌', '▋', '▊', '▉']),
            overdue: '▓',
            overdue_end: '!',
            due: '◆',
            clip_left: '‹',
            clip_right: '›',
            today: '┊',
            splitter: '│',
            link: LinkGlyphs {
                h: '─',
                v: '│',
                down_left: '┐',
                up_left: '┘',
                down_right: '┌',
                up_right: '└',
                tee_left: '┤',
                tee_right: '├',
                arrow: '▶',
                up: '↑',
                down: '↓',
            },
        }
    }
}

impl Glyphs {
    /// ASCII 만. 오늘 선과 묶음 계획이 겹치지 않도록 글자를 따로 쓴다. 미룬 계획도 연결선의 `-` 와 갈라
    /// `~` 다 — 같은 글자면 미룬 막는 줄의 막대와 그 뒤의 연결선이 색으로만 갈린다.
    pub fn ascii() -> Self {
        Glyphs {
            planned: '.',
            planned_group: '=',
            deferred: '~',
            actual: '#',
            eighths: None,
            overdue: '%',
            overdue_end: '!',
            due: '*',
            clip_left: '<',
            clip_right: '>',
            today: ':',
            splitter: '|',
            link: LinkGlyphs {
                h: '-',
                v: '|',
                down_left: '+',
                up_left: '+',
                down_right: '+',
                up_right: '+',
                tee_left: '+',
                tee_right: '+',
                arrow: '>',
                up: '^',
                down: 'v',
            },
        }
    }
}
