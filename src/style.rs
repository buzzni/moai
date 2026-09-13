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
/// - 남는 초록(`DONE`)·청록(`TAG`)·파랑(`EPIC`)은 한눈 보기 줄에 그 뜻이 서지 않는다 —
///   done 줄도 태그 칸도 에픽 열도 없다
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
/// - FNV-1a 로 세고 MurmurHash3 의 끝섞기(`fmix64`)를 지난다. `DefaultHasher` 는
///   알고리즘이 바뀔 수 있다고 적혀 있고, 바뀌면 업그레이드 한 번에 모든 프로젝트의
///   색이 바뀐다. **끝섞기를 빼지 않는다** — 날 FNV 의 `% 3` 은 끝 글자 하나만 다른
///   경로(`/a`…`/f`)를 전부 한 색에 몰고, 형제 디렉터리 300개를 190:100:10 으로 쏠리게 냈다
/// - **겹치는 것은 받아들인다.** 색은 셋이라 프로젝트가 늘면 겹친다. 겹쳐도 이름이
///   곁에 서서 가른다 — 겹침을 피하려고 목록 안에서 밀어내면 남을 더할 때 제 색이 바뀐다
pub fn project_colour(path: &std::path::Path) -> Style {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut h = OFFSET;
    for part in path.components() {
        // 조각 사이에 0 을 끼운다 — `a/bc` 와 `ab/c` 가 같은 바이트열로 섞이지 않게.
        for b in part.as_os_str().as_encoded_bytes().iter().chain([&0u8]) {
            h ^= u64::from(*b);
            h = h.wrapping_mul(PRIME);
        }
    }
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
    h ^= h >> 33;
    h = h.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    h ^= h >> 33;
    fg(PROJECT_HUES[(h % PROJECT_HUES.len() as u64) as usize])
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

/// `in_progress` 가 도는 걸음. ora 기본 세트를 그대로 옮겼다 — 검증된 것을
/// 다시 재느니 그대로 가져온다.
pub const SPIN: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// 도는 칸인가. **한 군데서만 정한다** — 그리는 쪽과 깨어날 걸음을 재는 쪽이
/// 따로 판단하면, 도는 글리프를 그려 놓고 아무도 안 깨워 첫 프레임에 멈춘다.
pub fn spins(status: &str) -> bool {
    status == "in_progress"
}

/// [`glyph`] 와 같되 도는 칸만 `frame` 으로 돈다. **TUI 만 이 값을 넘긴다** —
/// `view`(CLI)는 한 번 찍고 끝이라 돌 프레임이 없고, 거기서 이걸 쓰면 같은
/// 이슈가 부를 때마다 다른 글자로 찍혀 파이프로 받는 쪽이 자를 잃는다.
///
/// 그래서 **두 표면의 글리프가 일부러 다르다** — CLI 는 `▸`, 탐색기는 도는
/// 프레임. 같은 뜻을 같은 모양으로 내자는 규칙(`draw::role_style`)의 예외로
/// 보이지만, 그 규칙이 막으려는 것은 **뜻이 갈라지는 것**이고 칸 이름은 두
/// 표면에 똑같이 적힌다. 움직임은 곁들이고, 뜻은 낱말이 진다. 이쪽을 맞추려고
/// CLI 글리프를 프레임 하나로 바꾸지 않는다 — 멈춘 스피너 한 칸은 아무 뜻이 없다.
pub fn spin_glyph(status: &str, frame: usize) -> &'static str {
    if spins(status) { SPIN[frame % SPIN.len()] } else { glyph(status) }
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

    /// 프레임이 겹치지 않고 한 바퀴 돌면 처음으로 돌아온다. 다른 칸은 안 돈다.
    #[test]
    fn spin_glyph_cycles_in_progress_and_leaves_others_still() {
        let seen: std::collections::BTreeSet<_> =
            (0..SPIN.len()).map(|f| spin_glyph("in_progress", f)).collect();
        assert_eq!(seen.len(), SPIN.len(), "프레임이 겹친다");
        assert_eq!(spin_glyph("in_progress", SPIN.len()), spin_glyph("in_progress", 0));
        for st in ["todo", "review", "done", "설정으로_더한_칸"] {
            assert_eq!(spin_glyph(st, 3), glyph(st), "{st} 는 안 돌아야 한다");
        }
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
        let hue = |s: &str| match project_colour(Path::new(s)).get_fg_color() {
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
        let family = |s: Style| match s.get_fg_color() {
            Some(Color::Ansi(c)) => Some(c),
            _ => None,
        };
        for hue in PROJECT_HUES {
            assert!(!vanish.contains(&hue) && !bright.contains(&hue) && !meaning.contains(&hue), "{hue:?}");
            for used in [IN_PROGRESS, REVIEW, WARN, ERROR, P0, P1] {
                let c = family(used).unwrap();
                assert_ne!(Some(hue), Some(c), "{hue:?} 가 칸·경고·우선순위의 색과 같다");
            }
        }
        let uniq: std::collections::BTreeSet<_> = PROJECT_HUES.iter().map(|c| format!("{c:?}")).collect();
        assert_eq!(uniq.len(), PROJECT_HUES.len(), "팔레트에 같은 색이 두 번 있다");
        // 모든 칸이 실제로 쓰인다 — 나머지 연산이 한쪽으로 쏠리지 않는다.
        let seen: std::collections::BTreeSet<_> = (0..64)
            .map(|i| format!("{:?}", project_colour(std::path::Path::new(&format!("/p/{i}"))).get_fg_color()))
            .collect();
        assert_eq!(seen.len(), PROJECT_HUES.len(), "{seen:?}");
    }

    /// 기본 우선순위는 안 칠한다.
    #[test]
    fn default_priority_is_unstyled() {
        assert_eq!(paint(priority_style(2), "p2"), "p2");
        assert_ne!(paint(priority_style(0), "p0"), "p0");
    }
}
