//! 색과 글리프.
//!
//! **철칙: 색이 혼자 뜻을 지지 않는다.** 모든 색에 글리프나 낱말이 붙는다.
//! `--no-color` 로도, 색맹인 눈으로도 같은 정보가 읽혀야 한다.
//!
//! 기본 16색만 쓴다. 256색·truecolor 를 쓰면 사용자의 터미널이 밝은 배경인지
//! 어두운 배경인지 모르는 채로 색을 고르게 된다. 기본색은 테마가 대신 고른다.

use anstyle::{AnsiColor, Color, Style};

const fn fg(c: AnsiColor) -> Style {
    Style::new().fg_color(Some(Color::Ansi(c)))
}

/// 다수파는 칠하지 않는다 — 다수파를 칠하면 신호가 죽는다.
pub const PLAIN: Style = Style::new();
pub const DIM: Style = Style::new().dimmed();

pub const TODO: Style = PLAIN;
pub const IN_PROGRESS: Style = fg(AnsiColor::BrightYellow).bold();
/// **가장 눈에 띄어야 한다** — 게이트를 없앤 대신 여기가 썩는다.
pub const REVIEW: Style = fg(AnsiColor::BrightMagenta);
pub const DONE: Style = fg(AnsiColor::Green).dimmed();
/// 진행 바의 찬 칸 — 16색 10번(사용자 결정, moai-a46g). `DONE` 과 가른다: 끝난 줄은 흐리게
/// 물러나야 하고, 바는 눈에 서야 한다.
pub const BAR: Style = fg(AnsiColor::BrightGreen);
pub const OTHER: Style = fg(AnsiColor::Cyan);

pub const EPIC: Style = fg(AnsiColor::BrightBlue).bold();
pub const TAG: Style = fg(AnsiColor::Cyan).dimmed();
/// 목록의 에픽 열. 곁다리라 에픽 제목 자체보다 약하게.
pub const EPIC_REF: Style = fg(AnsiColor::BrightBlue).dimmed();
pub const ID: Style = DIM;
/// 다른 워크트리에서 온 줄의 `⎇ <브랜치>`. **글리프가 뜻을 지고 색은 곁들인다** —
/// 색을 꺼도 `⎇` 가 남는다. 쓰이지 않은 색을 고른다: 노랑·자홍·초록은 칸이,
/// 파랑은 묶음이, 청록은 태그·코드가 이미 쓴다.
pub const BRANCH: Style = fg(AnsiColor::BrightCyan).bold();
/// 브랜치 머리표의 글리프.
pub const BRANCH_GLYPH: &str = "⎇";
/// 탐색기에서 **포커스 있는 칸의 테두리.** 초록은 사용자 기획이다(moai-b8wq).
/// 칸의 `done` 과 같은 계열이지만 **그쪽은 흐리게, 이쪽은 테두리에만** 쓰여 한
/// 자리에서 부딪치지 않는다. 뜻은 색이 아니라 테두리 모양이 진다 —
/// `draw::frame` 이 굵은 선을 함께 준다.
pub const FOCUS: Style = fg(AnsiColor::Green);
pub const ERROR: Style = fg(AnsiColor::Red);
pub const WARN: Style = fg(AnsiColor::BrightYellow);
pub const HEAD: Style = Style::new().bold();

/// 한 화면에 여러 프로젝트를 올릴 때 프로젝트마다 다는 색상 — [`project_colour`] 가 고른다.
///
/// **기본 16색의 보통 칸 셋만 쓴다.** 고른 까닭(moai-xs9x note 에 전문):
/// - bright 변종은 Solarized 가 회색에 매핑하고, 밝은 바탕(Tango)에서 초록·청록이
///   사라진다. 검정·흰색은 한쪽 바탕과 같은 색이다
/// - 빨강은 `ERROR`·`P1`, 노랑은 `IN_PROGRESS`·`WARN`, 자홍은 `REVIEW` 다. 한눈 보기
///   줄에는 우선순위 칸·집은 칸·review 줄이 id 곁에 실제로 서서, 그 색의 id 는 거짓
///   뜻으로 읽힌다
/// - 남는 초록(`DONE`)·파랑(`EPIC`)은 한눈 보기 줄에 그 뜻이 서지 않는다 — done 줄도
///   에픽 열도 없다. 청록의 `TAG` 도 태그 칸이 없어 안 선다
/// - **청록 하나는 알고 받아들인다.** `statuses` 에 칸을 더한 저장소(`blocked` 따위)는
///   그 칸의 줄이 집은 것으로 서고, 글리프 `○` 가 `OTHER`(청록)로 칠해져 청록 id 곁에
///   붙는다. 빼지 않는 까닭: 빨강·노랑·자홍이 지는 뜻은 급함·진행·review 라 잘못 읽으면
///   판단이 틀리지만, `OTHER` 는 "네 칸 밖" 이라는 뜻 없는 뜻이다. 그리고 청록을 빼면
///   팔레트가 둘로 줄어 **모든** 사용자의 두 프로젝트가 절반 확률로 겹친다 — 칸을 더한
///   저장소의 드문 줄 하나를 위해 치르기엔 비싸다. 칸은 글리프가 가른다
///
/// **색이 혼자 뜻을 지지 않는다.** 이 색을 다는 줄에는 늘 프로젝트 이름이 곁에 선다.
pub const PROJECT_HUES: [AnsiColor; 3] = [AnsiColor::Cyan, AnsiColor::Green, AnsiColor::Blue];

/// 프로젝트 → 색. **경로에 대한 순수 함수**라 부를 때마다, 표면마다(CLI·TUI) 같다.
///
/// - **등록 순서가 아니라 경로 해시다.** 순서로 고르면 앞의 것 하나를 빼는 순간 뒤의
///   것이 전부 색을 바꾼다. 해시는 다른 프로젝트를 더하고 빼도 제 색이 그대로다
/// - **이름이 아니라 경로다.** 이름은 겹치면 위 디렉터리를 붙이는 파생값이라 등록
///   목록을 따라 바뀐다
/// - 경로는 조각(`components`) 단위로 센다 — 끝 `/` 나 겹 `/` 같은 철자가 색을 가르지
///   않게. 링크는 안 푼다(순수 함수로 둔다). 등록이 이미 푼 경로로 적힌다
/// - FNV-1a([`crate::text::fnv1a64_from`])로 세고 MurmurHash3 의 끝섞기(`fmix64`)를
///   지난다. `DefaultHasher` 는 알고리즘이 바뀔 수 있다고 적혀 있고, 바뀌면
///   업그레이드 한 번에 모든 프로젝트의
///   색이 바뀐다. **끝섞기를 빼지 않는다** — 날 FNV 의 `% 3` 은 끝 글자 하나만 다른
///   경로(`/a`…`/f`)를 전부 한 색에 몰고, 형제 디렉터리 300개를 190:100:10 으로 쏠리게 냈다
/// - **겹치는 것은 받아들인다.** 색은 셋이라 프로젝트가 늘면 겹친다. 겹쳐도 이름이
///   곁에 서서 가른다 — 겹침을 피하려고 목록 안에서 밀어내면 남을 더할 때 제 색이 바뀐다.
///   겹침이 거슬리는 사람은 사용자 설정에 색을 정한다(`chosen`)
///
/// **`chosen` 이 있으면 그것이 해시를 이긴다**(moai-o04b). 사람이 설정에 적은 색이다 —
/// 이 모듈은 설정을 읽지 않고, 부르는 쪽이 등록 항목에서 들고 온다. CLI 한눈 보기·
/// `project ls`·TUI 가 모두 이 한 함수를 지나야 같은 프로젝트를 같은 색으로 낸다.
pub fn project_colour(path: &std::path::Path, chosen: Option<Hue>) -> Style {
    fg(PROJECT_HUES[chosen.unwrap_or_else(|| Hue::of_path(path)).0])
}

/// 팔레트([`PROJECT_HUES`]) 안의 색 하나. **첨자로는 못 만든다** — 이름([`Hue::named`])이나
/// 경로([`Hue::of_path`])로만 선다. 그래서 `Hue` 를 든 값은 늘 팔레트 안이고, 빨강·노랑·
/// 자홍이 설정을 거쳐 프로젝트 색으로 새어 들 길이 없다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hue(usize);

/// 사람이 설정과 명령줄에 적는 이름 — [`PROJECT_HUES`] 와 같은 차례다.
const PROJECT_HUE_NAMES: [&str; PROJECT_HUES.len()] = ["cyan", "green", "blue"];

impl Hue {
    /// 이름 → 색. 팔레트 밖이거나 대소문자가 섞였으면(`Green`) `None` 이다 — 받는 철자를
    /// 늘리면 되쓸 때 어느 철자로 쓸지를 또 정해야 한다.
    pub fn named(name: &str) -> Option<Hue> {
        PROJECT_HUE_NAMES.iter().position(|n| *n == name).map(Hue)
    }

    pub fn name(self) -> &'static str {
        PROJECT_HUE_NAMES[self.0]
    }

    /// 받는 이름 전부, 팔레트 차례로 — 거절문이 댄다.
    pub fn names() -> &'static [&'static str] {
        &PROJECT_HUE_NAMES
    }

    /// 정한 색이 없을 때 경로로 고른다. 까닭은 [`project_colour`] 에 있다.
    pub fn of_path(path: &std::path::Path) -> Hue {
        // 셈 자체는 [`crate::text::fnv1a64_from`] 이다(moai-2vrw) — 여기 한 벌 더 적으면 상수가
        // 두 자리에 서고, 한쪽만 고치는 날 모든 프로젝트의 색이 바뀐다.
        let mut h = crate::text::FNV64_OFFSET;
        for part in path.components() {
            // 조각 사이에 0 을 끼운다 — `a/bc` 와 `ab/c` 가 같은 바이트열로 섞이지 않게.
            h = crate::text::fnv1a64_from(h, part.as_os_str().as_encoded_bytes());
            h = crate::text::fnv1a64_from(h, &[0]);
        }
        h ^= h >> 33;
        h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
        h ^= h >> 33;
        h = h.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
        h ^= h >> 33;
        Hue((h % PROJECT_HUES.len() as u64) as usize)
    }
}

/// 본문 마크다운.
///
/// **앞서 이 자리에 틀린 말이 적혀 있었다** — "굵게는 속성이라 `NO_COLOR` 에서도
/// 살아남는다". `anstream` 은 색만이 아니라 **모든 SGR 을 걷어낸다**. 굵게도
/// 기울임도 함께 사라지고, 그것은 파이프로 넘긴 모든 출력에 해당한다.
///
/// 그래서 **뜻을 지는 것은 글자로 남긴다** — 코드는 백틱, 제목은 `#`, 인용은
/// 세로줄. 굵게·기울임만 색과 함께 사라지는데, 그 둘은 글맛이지 낱말의 정체가
/// 아니라 잃어도 문장이 여전히 같은 것을 말한다. 칸이나 우선순위처럼 판단이
/// 걸린 것에는 이 예외를 두지 않는다.
pub const STRONG: Style = Style::new().bold();
pub const EM: Style = Style::new().italic();
pub const CODE: Style = fg(AnsiColor::Cyan);

pub const P0: Style = fg(AnsiColor::BrightRed).bold();
pub const P1: Style = fg(AnsiColor::Red);

/// 칸 이름 → 글리프. 모르는 칸도 글리프를 갖는다.
pub fn glyph(status: &str) -> &'static str {
    match status {
        "todo" => "·",
        "in_progress" => "▸",
        "review" => "?",
        "done" => "✓",
        _ => "○",
    }
}

// **줄머리에 서던 표식 둘(idea `◇`·미룸 `‖`)을 걷었다**(moai-nb6w, 사용자 결정 2026-09-21).
//
// 축 셋(`kind`·`status`·`deferred_at`)을 글리프로 다 세우려던 것인데, 표식이 하나만 붙어도
// 줄머리가 한 칸 길어진다 — 없는 줄은 두 칸(` ·`), idea 는 세 칸(` ·◇`), 미룬 idea 는 네
// 칸이다. 그래서 한 목록 안에서 트리 선이 줄마다 다른 자리에 섰다.
//
//     moai-3y1p   ·◇ ├─ init 이 .gitattributes 의 …
//     moai-7cpd   ·◇‖ └─ Regression-of 표식을 세어 …
//
// 자리를 옮기는 것으로는 안 된다 — 미룸을 스피너 자리로 보내도 `◇` 가 남아 같은 어긋남이
// 그대로 선다. 남은 것은 칸 글리프 하나고, 그래서 **줄머리는 늘 두 칸이다**.
//
// **되돌리려면 줄머리 폭을 먼저 푼다.** 표식을 다시 세우는 일은 글자 하나를 더하는 일이
// 아니라, 폭이 줄마다 달라도 트리 선이 안 어긋나게 하는 일이다. 그 길을 안 풀고 상수만
// 되살리면 위의 화면이 그대로 돌아온다. 원래 요구(미룬 줄과 idea 줄을 목록에서 가른다)는
// 아직 서 있다 — moai-pkvw·moai-h06a 가 `todo` 로 그것을 들고 있다.
//
// **낱말은 그대로다.** "색이 혼자 뜻을 지지 않는다" 를 지키는 것은 목록 꼬리의
// `status.put_off` 와 상세·`--json` 의 낱말이고, 그것들은 표식과 따로 산다.

/// 시작한 칸이 도는 걸음. ora 기본 세트를 그대로 옮겼다 — 검증된 것을
/// 다시 재느니 그대로 가져온다.
pub const SPIN: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// 도는 줄이 `frame` 에 낼 글자. **돌지는 여기서 안 정한다** — 그것은 설정을 든
/// `tui::App::spins` 한 곳이다(moai-q59j). 여기가 칸 이름 `"in_progress"` 로 가르던
/// 때는 칸 이름을 바꾼 설정에서 아무것도 안 돌았다 — `style` 은 설정을 모른다.
///
/// **TUI 만 이 값을 넘긴다** —
/// `view`(CLI)는 한 번 찍고 끝이라 돌 프레임이 없고, 거기서 이걸 쓰면 같은
/// 이슈가 부를 때마다 다른 글자로 찍혀 파이프로 받는 쪽이 자를 잃는다.
///
/// 그래서 **두 표면의 글리프가 일부러 다르다** — CLI 는 `▸`·`?` 같은 멈춘 글리프,
/// 탐색기는 시작한 칸마다 도는 프레임(목록 줄은 그 뒤에 멈춘 글리프를 붙인다 — `draw::row_glyph`). 같은 뜻을 같은 모양으로 내자는 규칙(`draw::role_style`)의 예외로
/// 보이지만, 그 규칙이 막으려는 것은 **뜻이 갈라지는 것**이고 칸 이름은 두
/// 표면에 똑같이 적힌다. 움직임은 곁들이고, 뜻은 낱말이 진다. 이쪽을 맞추려고
/// CLI 글리프를 프레임 하나로 바꾸지 않는다 — 멈춘 스피너 한 칸은 아무 뜻이 없다.
pub fn spin_frame(frame: usize) -> &'static str {
    SPIN[frame % SPIN.len()]
}

pub fn status_style(status: &str) -> Style {
    match status {
        "todo" => TODO,
        "in_progress" => IN_PROGRESS,
        "review" => REVIEW,
        "done" => DONE,
        _ => OTHER,
    }
}

/// 기본값(p2)은 칠하지 않는다. 기본값을 칠하면 아무 뜻이 없다.
pub fn priority_style(p: u8) -> Style {
    match p {
        0 => P0,
        1 => P1,
        3 => DIM,
        _ => PLAIN,
    }
}

/// 칠한 글자. 색을 끄는 판단은 anstream 이 출력 시점에 한다.
pub fn paint(style: Style, text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    if style == Style::new() {
        return text.to_string(); // 안 칠할 것에 이스케이프를 붙이지 않는다
    }
    format!("{}{text}{}", style.render(), anstyle::Reset.render())
}

/// 색을 **글자에서** 걷어낸다.
///
/// `anstream` 은 출력 **스트림**에서 SGR 을 지운다 — 그래서 화면과 파이프가
/// 저절로 맞고, `paint` 가 무조건 칠해도 됐다. 그런데 훅은 이 글을 JSON
/// 문자열 **안에** 넣는다. 거기 들어간 이스케이프는 `\u001b` 로 인코딩되어
/// 더 이상 스트림의 SGR 이 아니고, 그래서 아무도 안 걷어낸다 — 받는 쪽 화면에
/// 그 글자가 그대로 뜬다. 걷어낼 마지막 기회가 여기다.
pub fn plain(s: &str) -> String {
    anstream::adapter::strip_str(s).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 칸마다 글리프가 다르다 — 색을 꺼도 구별돼야 하기 때문이다.
    #[test]
    fn every_column_has_its_own_glyph() {
        let g: Vec<&str> = ["todo", "in_progress", "review", "done"].iter().map(|s| glyph(s)).collect();
        let uniq: std::collections::BTreeSet<_> = g.iter().collect();
        assert_eq!(uniq.len(), g.len(), "{g:?}");
        assert_eq!(glyph("설정으로_더한_칸"), "○");
    }

    /// **칸 글리프는 한 칸씩이고 글자 하나씩이다**(moai-fgyg). 표가 어긋나는 자리가 여기다 —
    /// `moai-prp5` 에서 스피너 두 칸이 목록을 어긋낸 그 자리다.
    ///
    /// **코드포인트를 글자로 못박는다.** 폭은 글자가 정하므로, 모양이 비슷하다고 이모지 표현이
    /// 붙은 글자로 바꾸는 순간 터미널에 따라 두 칸으로 그려진다.
    ///
    /// 줄머리에 서던 표식 둘은 걷었다(moai-nb6w) — 까닭은 이 파일 위쪽에 있다. 되살릴 때 이 시험이
    /// 재던 것(글자 하나·한 칸)은 그때도 서야 하지만, 먼저 풀 것은 **줄머리 폭**이다.
    #[test]
    fn the_column_glyphs_are_one_column_each() {
        for st in ["todo", "in_progress", "review", "done", "?"] {
            let g = glyph(st);
            assert_eq!(g.chars().count(), 1, "{g:?} 가 글자 하나가 아니다");
            assert_eq!(crate::text::width(g), 1, "{g:?} 가 한 칸이 아니다");
        }
    }

    /// 프레임이 겹치지 않고 한 바퀴 돌면 처음으로 돌아온다. 어느 칸이 도는지는
    /// `tui::App::spins` 가 설정으로 정한다(`every_started_column_spins_whatever_it_is_named`).
    #[test]
    fn spin_frame_cycles_without_repeating() {
        let seen: std::collections::BTreeSet<_> = (0..SPIN.len()).map(spin_frame).collect();
        assert_eq!(seen.len(), SPIN.len(), "프레임이 겹친다");
        assert_eq!(spin_frame(SPIN.len()), spin_frame(0));
        assert!(
            !SPIN.iter().any(|f| ["todo", "in_progress", "review", "done"].iter().any(|s| glyph(s) == *f)),
            "도는 글자가 멈춘 글리프와 겹친다"
        );
    }

    #[test]
    fn paint_wraps_and_resets() {
        let s = paint(REVIEW, "x");
        assert!(s.starts_with('\u{1b}') && s.ends_with("\u{1b}[0m"), "{s:?}");
        assert!(s.contains('x'));
        assert_eq!(paint(REVIEW, ""), "");
    }

    /// 같은 경로는 늘 같은 색이고, 철자만 다른 같은 경로도 같은 색이다.
    #[test]
    fn project_colour_is_a_function_of_the_path_alone() {
        use std::path::Path;
        let hue = |s: &str| match project_colour(Path::new(s), None).get_fg_color() {
            Some(Color::Ansi(c)) => c,
            other => panic!("{s}: 16색이 아니다 — {other:?}"),
        };
        assert_eq!(hue("/home/me/work/api"), hue("/home/me/work/api/"));
        assert_eq!(hue("/home/me/work/api"), hue("/home/me//work/api"));
        // **값을 못 박는다.** 해시나 팔레트 차례를 바꾸면 사용자의 모든 프로젝트가 색을
        // 바꾼다 — 일부러 바꿀 때만 이 줄을 고친다.
        use AnsiColor as A;
        let got: Vec<_> = ["/a", "/b", "/c", "/d", "/e", "/f"].iter().map(|s| hue(s)).collect();
        assert_eq!(got, [A::Green, A::Cyan, A::Blue, A::Cyan, A::Blue, A::Blue], "배정이 바뀌었다");
    }

    /// 팔레트에는 한쪽 바탕에서 사라지는 색도, 한눈 보기 줄에서 다른 뜻을 지는 색도 없다.
    #[test]
    fn project_hues_avoid_vanishing_and_meaningful_colours() {
        use AnsiColor as A;
        let vanish = [A::Black, A::White, A::BrightBlack, A::BrightWhite];
        // bright 변종은 Solarized 에서 회색이 되고 밝은 바탕에서 사라진다.
        let bright = [A::BrightRed, A::BrightGreen, A::BrightYellow, A::BrightBlue, A::BrightMagenta, A::BrightCyan];
        // 한눈 보기 줄에 곁에 서는 뜻의 색상 — 보통·bright 둘 다.
        let meaning = [A::Red, A::Yellow, A::Magenta];
        // bright 는 같은 색상의 밝은 쪽이다 — `BrightYellow` 인 `IN_PROGRESS` 곁에 `Yellow` id
        // 는 같은 뜻으로 읽힌다. 그래서 색상 계열로 견준다.
        let family = |s: Style| match s.get_fg_color() {
            Some(Color::Ansi(c)) => match c {
                A::BrightRed => A::Red,
                A::BrightGreen => A::Green,
                A::BrightYellow => A::Yellow,
                A::BrightBlue => A::Blue,
                A::BrightMagenta => A::Magenta,
                A::BrightCyan => A::Cyan,
                c => c,
            },
            other => panic!("16색이 아니다 — {other:?}"),
        };
        for hue in PROJECT_HUES {
            assert!(!vanish.contains(&hue) && !bright.contains(&hue) && !meaning.contains(&hue), "{hue:?}");
            for used in [IN_PROGRESS, REVIEW, WARN, ERROR, P0, P1] {
                assert_ne!(hue, family(used), "{hue:?} 가 칸·경고·우선순위의 색과 같은 계열이다");
            }
        }
        // 알고 받아들인 겹침 하나 — 설정으로 더한 칸의 `OTHER`. 팔레트나 `OTHER` 를 바꾸면 여기서
        // 멈춰 `PROJECT_HUES` 문서의 근거를 다시 본다.
        let clash: Vec<_> = PROJECT_HUES.iter().filter(|h| **h == family(OTHER)).collect();
        assert_eq!(clash, [&A::Cyan], "OTHER 와의 겹침이 문서와 다르다");
        let uniq: std::collections::BTreeSet<_> = PROJECT_HUES.iter().map(|c| format!("{c:?}")).collect();
        assert_eq!(uniq.len(), PROJECT_HUES.len(), "팔레트에 같은 색이 두 번 있다");
        // 모든 칸이 실제로 쓰인다 — 나머지 연산이 한쪽으로 쏠리지 않는다.
        let seen: std::collections::BTreeSet<_> = (0..64)
            .map(|i| format!("{:?}", project_colour(std::path::Path::new(&format!("/p/{i}")), None).get_fg_color()))
            .collect();
        assert_eq!(seen.len(), PROJECT_HUES.len(), "{seen:?}");
    }

    /// **정한 색이 해시를 이긴다** — 경로가 어느 색을 고르든 정한 것이 선다. 이름은 팔레트의
    /// 이름뿐이고, 이름과 팔레트 칸은 한 차례로 짝지어 있다.
    #[test]
    fn a_chosen_hue_beats_the_path_hash_and_names_stay_inside_the_palette() {
        use std::path::Path;
        assert_eq!(Hue::names().len(), PROJECT_HUES.len());
        for (i, name) in Hue::names().iter().enumerate() {
            let hue = Hue::named(name).unwrap();
            assert_eq!(hue.name(), *name);
            // 이름이 뜻하는 색과 팔레트의 색이 같다 — `"green"` 이 파랑을 칠하면 안 된다.
            assert_eq!(format!("{:?}", PROJECT_HUES[i]).to_lowercase(), *name);
            for p in ["/a", "/b", "/c", "/d", "/e", "/f"] {
                assert_eq!(project_colour(Path::new(p), Some(hue)), fg(PROJECT_HUES[i]), "{p} 에서 {name} 가 졌다");
            }
        }
        for bad in ["red", "yellow", "magenta", "Green", " green", "", "auto", "brightcyan"] {
            assert_eq!(Hue::named(bad), None, "{bad:?} 를 받았다");
        }
        assert_eq!(project_colour(Path::new("/a"), None), fg(PROJECT_HUES[Hue::of_path(Path::new("/a")).0]));
    }

    /// 기본 우선순위는 안 칠한다.
    #[test]
    fn default_priority_is_unstyled() {
        assert_eq!(paint(priority_style(2), "p2"), "p2");
        assert_ne!(paint(priority_style(0), "p0"), "p0");
    }
}
