//! 통계 창 — `SPC p s`(moai-1hka.bq9). 목록과 상세 자리를 **통째로** 덮고, Esc 로 닫으면 보던 자리 그대로다.
//!
//! **세는 자는 CLI 와 하나다**([`crate::report::stats`]). 창이 따로 세면 같은 저장소를 두고 `moai stats` 와 창이
//! 다른 수를 낸다 — 여기는 고를 줄과 거름망, 시간대를 대고 그 둘을 부르기만 한다.
//!
//! **여는 순간 센다** — 프레임마다 세지 않는다. 그리는 쪽(`draw::stats_window`)은 센 값을 펴기만 한다: 탐색기는
//! 프레임마다 이슈 전부를 훑던 자리로 여러 번 멈췄다(moai-ropk·moai-56jf). 흐름의 칸 너비(주·날)는 둘 다 연
//! 순간에 세어 들고 있다가 키 하나로 바꾼다 — 바꿀 때 다시 세면 그사이 다시 읽힌 줄로 수가 흔들린다.
//!
//! **노트는 탐색기가 든 것을 쓴다**([`super::Ground`] 의 노트, `/` 의 노트 검색이 읽는 그 한 벌) — 아직 안
//! 읽었으면 여는 이 한 번에 읽어 그 자리에 든다. 저널을 따로 한 번 더 풀지 않는다.
//!
//! **무엇을 세나**
//!
//! - 프로젝트 안이면 그 프로젝트. 거름망(`SPC f`·`/`)이 걸려 있으면 그것으로 좁히고 제목이 그 글을 댄다. 아니면
//!   `moai stats` 를 아무 플래그 없이 친 것과 같다
//! - 한눈 보기(`0`)면 **커서가 선 줄의 프로젝트**다 — 머리줄이면 그 프로젝트, 이슈 줄이면 그 줄이 사는
//!   프로젝트(`SPC n`·`r` 과 같은 규칙). 한눈 보기에는 거름망이 안 선다
//! - **탐색기가 든 줄을 센다** — 옆 워크트리를 겹쳐 보고 있으면 거기서 온 줄도 든다. CLI 의 `moai stats` 는
//!   안 겹치므로, 그때는 제목이 그렇다고 말한다

use super::keys::{STATS, Stat};
use super::layer::{Depth, Told};
use super::scroll::Scroll;
use super::{App, Ground, Mode, Row, Seat};
use crate::config::Config;
use crate::i18n::{fill, say};
use crate::model::{Issue, Work};
use crate::query::Filter;
use crate::report::stats::{self, Ask, Bucket, Stats};
use crate::store::Repo;
use ratatui::crossterm::event::KeyEvent;
use std::collections::BTreeMap;

/// 창의 상태 — 센 것 두 벌(주·날)과 지금 보는 쪽, 굴린 자리.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    /// 무엇을 셌나 — 프로젝트 이름.
    pub project: String,
    /// 걸린 거름망을 화면에 적은 글(`draw::badge`) — 없으면 프로젝트의 기본이다.
    pub filter: Option<String>,
    /// 옆 워크트리에서 온 줄이 섞였는가. CLI 는 안 겹치므로 제목이 말한다.
    pub overlaid: bool,
    week: Stats,
    day: Stats,
    /// 지금 보는 흐름의 칸 너비.
    pub bucket: Bucket,
    /// 굴린 자리 — 그림이 높이와 길이를 재어 넣는다([`Scroll::fit`]).
    pub scroll: Scroll,
    /// 접두어(`gg` 의 첫 `g`)를 기다리는 열.
    pub chord: super::keys::Chord,
}

impl Window {
    /// 지금 보는 쪽의 셈.
    pub fn stats(&self) -> &Stats {
        match self.bucket {
            Bucket::Week => &self.week,
            Bucket::Day => &self.day,
        }
    }

    /// 칸 너비를 바꾼다. **굴린 자리는 그대로 둔다** — 흐름 밑의 소요·AI 작업을 보다가 바꾼 사람이 맨 위로
    /// 튕기면 보던 것을 다시 찾아야 한다. 흐름의 높이는 칸 너비와 상관없이 같다.
    pub fn switch(&mut self) {
        self.bucket = match self.bucket {
            Bucket::Week => Bucket::Day,
            Bucket::Day => Bucket::Week,
        };
    }
}

/// 한 프로젝트의 줄을 센다 — 주마다 한 벌, 날마다 한 벌.
///
/// **노트는 `ground` 가 든 것이다** — 안 읽었으면 여기서 한 번 읽어 그 자리에 든다([`Ground::read_notes`]). 읽은
/// 노트 글에서 `model:` 줄을 읽는 법은 CLI 와 같은 몸이다(`model::work_in_note`).
fn count(
    issues: &[Issue],
    cfg: &Config,
    ground: &mut Ground,
    repo: Option<&Repo>,
    origin: &crate::worktree::Origin,
    filter: &Filter,
    zone: &crate::tz::Zone,
    now: &str,
) -> (Stats, Stats) {
    if let Some(repo) = repo {
        ground.read_notes(repo, origin, issues);
    }
    let mut wh = ground.here(issues);
    // **흐름의 날은 보는 사람의 날이다** — 화면이 시각을 그리는 그 시간대고(`SPC o t`), 거름망의 `created=<날>` 도
    // 이것으로 읽는다(`App::apply`).
    wh.zone = Some(zone);
    let rows = stats::select(issues, filter, &wh, now);
    let work: BTreeMap<String, Vec<Work>> = rows
        .iter()
        .map(|i| (i.id.clone(), ground.notes_of(&i.id).iter().flat_map(|t| crate::model::work_in_note(t)).collect()))
        .collect();
    let ask = |bucket: Bucket| Ask { kind: filter.kind, by: Vec::new(), bucket, last: bucket.default_last() };
    (
        stats::of(&rows, &wh, cfg, &work, &ask(Bucket::Week), now),
        stats::of(&rows, &wh, cfg, &work, &ask(Bucket::Day), now),
    )
}

impl App {
    /// `SPC p s` — 창을 연다. 셀 프로젝트를 못 정하거나 못 읽으면 알림 한 줄로 까닭을 대고 안 연다.
    pub(super) fn open_stats(&mut self) {
        let lang = self.site.lang;
        let zone = self.zone.clone();
        if !self.on_layer() {
            // **걸린 거름망으로 좁힌다** — 거는 자와 같은 자로 다시 짓는다(`App::build_filter`). 걸린 것은 이미 한 번
            // 지난 글이라 실패할 까닭이 없지만, 실패하면 좁히지 않은 채로 세지 않고 까닭을 댄다: 걸린 줄 아는 사람이
            // 전체 수를 좁힌 수로 읽는다.
            let filter = match self.hung.as_ref() {
                None => Filter::default(),
                Some(h) => match self.build_filter(&h.mode()) {
                    Ok(f) => f,
                    Err(why) => {
                        self.notice = Some(why);
                        return;
                    }
                },
            };
            let shown = self.hung.as_ref().map(|h| super::draw::badge(h, lang));
            let project = self.project_name();
            let site = &mut self.site;
            let overlaid = !site.origin.labels().is_empty();
            let (week, day) = count(
                &site.issues,
                &site.cfg,
                &mut site.ground,
                site.repo.as_ref(),
                &site.origin,
                &filter,
                &zone,
                &site.now,
            );
            self.mode = Mode::Stats(Box::new(Window::new(project, shown, overlaid, week, day)));
            return;
        }
        // **한눈 보기에는 "지금 선 프로젝트" 가 없다 — 커서가 댄다**(`App::open_form` 과 같은 규칙).
        let at = match self.current() {
            Some(Row::Project(at)) | Some(Row::Item(Seat::Place(at), ..)) => at,
            _ => {
                self.notice = Some(say(lang, "tui.stats.no_target").to_string());
                return;
            }
        };
        let Some(place) = self.layer.as_ref().and_then(|l| l.places.get(at)) else { return };
        let (name, path) = (place.name.clone(), place.path.clone());
        let filter = Filter::default();
        // **펼쳐 든 줄이 있으면 그것을 센다** — 화면에 선 그 줄이다.
        if let Some(site) = self.site_mut(Seat::Place(at)) {
            let overlaid = !site.origin.labels().is_empty();
            let (week, day) = count(
                &site.issues,
                &site.cfg,
                &mut site.ground,
                site.repo.as_ref(),
                &site.origin,
                &filter,
                &zone,
                &site.now,
            );
            self.mode = Mode::Stats(Box::new(Window::new(name, None, overlaid, week, day)));
            return;
        }
        // **접혀 아직 안 읽은 프로젝트는 그 자리에서 읽는다** — 누른 사람은 결과를 기다리고 있다(들어가는 길
        // `App::enter_project` 와 같다). 여는 데까지만 보고(`Depth::Lean`) 줄은 같은 읽기(`App::read`)로 읽는다 —
        // 못 열면 `open_place` 가 그 줄을 고쳐 세우고 까닭을 알림으로 댄다.
        let Some(repo) = self.open_place(at, Depth::Lean) else { return };
        let held = Told::held(self.layer.as_ref().and_then(|l| l.told_install(&path)));
        match (self.read)(&repo, self.worktree, lang, held) {
            Ok(mut fresh) => {
                let overlaid = !fresh.origin.labels().is_empty();
                let (week, day) = count(
                    &fresh.issues,
                    &repo.config,
                    &mut fresh.ground,
                    Some(&repo),
                    &fresh.origin,
                    &filter,
                    &zone,
                    &fresh.now,
                );
                self.mode = Mode::Stats(Box::new(Window::new(name, None, overlaid, week, day)));
            }
            Err(e) => {
                self.notice = Some(fill(say(lang, "tui.stats.unread"), &[("name", &name), ("why", &e.message)]));
            }
        }
    }

    /// 지금 선 프로젝트의 이름 — 층이 있으면 그 줄의 이름, 없으면 저장소 뿌리의 디렉터리 이름이다.
    fn project_name(&self) -> String {
        if let Some(p) = self.project() {
            return p.name.clone();
        }
        let root = self.site.repo.as_ref().map(|r| r.root.clone());
        root.as_deref()
            .and_then(std::path::Path::file_name)
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| ".".to_string())
    }

    /// 통계 창의 키 하나. **닫으면 탐색으로 돌아갈 뿐이다** — 커서·걸린 거름망·상세는 창이 안 건드려 그대로다.
    pub(super) fn stats_key(&mut self, k: KeyEvent) {
        let Mode::Stats(w) = &mut self.mode else { return };
        match w.chord.feed(STATS, k) {
            Some(Stat::Step(m)) => w.scroll.go(m),
            Some(Stat::Bucket) => w.switch(),
            Some(Stat::Close) => self.mode = Mode::Browse,
            None => {}
        }
    }
}

impl Window {
    fn new(project: String, filter: Option<String>, overlaid: bool, week: Stats, day: Stats) -> Window {
        Window {
            project,
            filter,
            overlaid,
            week,
            day,
            bucket: Bucket::Week,
            scroll: Scroll::default(),
            chord: super::keys::Chord::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{App, Hung, Input, Mode, Pane};
    use super::Window;
    use crate::config::Config;
    use crate::model::{Issue, Kind, Status};
    use crate::nav::Path;
    use crate::report::stats::{Bucket, Tally};

    fn cfg() -> Config {
        Config::parse("prefix = \"argos\"\n").unwrap()
    }

    fn row(id: &str, status: &str, tags: &[&str]) -> Issue {
        let mut i =
            Issue::new(id.into(), format!("{id} 제목"), Kind::Issue, Status::new(status), "2026-09-01T00:00:00Z");
        i.tags = tags.iter().map(|t| t.to_string()).collect();
        i
    }

    /// 일 셋(하나는 끝났고 하나는 bug)과 에픽 하나. **시계를 못박는다** — 흐름의 칸은 `Site::now` 로 서므로 안
    /// 박으면 시험이 오늘 날짜를 따라 달라진다.
    fn app() -> App {
        let mut epic = row("argos-0009", "todo", &[]);
        epic.kind = Kind::Epic;
        let mut done = row("argos-0002", "done", &[]);
        done.status_since = "2026-09-03T00:00:00Z".into();
        done.done_at = Some("2026-09-03T00:00:00Z".into());
        let issues = vec![row("argos-0001", "todo", &["bug"]), done, row("argos-0003", "in_progress", &[]), epic];
        let mut a = App::new(issues, cfg(), Path::new());
        a.site.now = "2026-09-10T00:00:00Z".into();
        a
    }

    fn window(a: &App) -> &Window {
        match &a.mode {
            Mode::Stats(w) => w,
            other => panic!("통계 창이 안 열렸다 — {other:?}"),
        }
    }

    /// **`SPC p s` 가 창을 열고, Esc 는 보던 화면을 그대로 돌려준다** — 커서·포커스·상세의 굴린 자리·걸린
    /// 거름망을 창이 안 건드린다. 창 안의 `b` 는 흐름을 주와 날 사이에서 바꾼다.
    #[test]
    fn spc_p_s_opens_the_window_and_esc_gives_the_screen_back_as_it_was() {
        let mut a = app();
        a.cursor = 1;
        a.focus = Pane::Detail;
        let before = (a.cursor, a.focus, a.hung.clone(), a.detail_open, a.detail);

        a.hit("SPC p s");
        let w = window(&a);
        assert_eq!((w.stats().rows, w.filter.clone(), w.bucket), (3, None, Bucket::Week), "일 셋을 주마다 센다");
        assert_eq!(w.project, ".", "저장소 없이 세운 App 은 이름이 없다");

        a.hit("b");
        assert_eq!(window(&a).bucket, Bucket::Day);
        assert_eq!(window(&a).stats().flow.bucket, Bucket::Day, "보이는 셈이 칸 너비를 안 따랐다");
        a.hit("b");
        assert_eq!(window(&a).bucket, Bucket::Week);
        // `q` 는 창을 안 닫는다 — 탐색에서 아무것도 안 하는 글자다(moai-en4u).
        a.hit("q");
        window(&a);

        a.hit("Esc");
        assert_eq!(a.mode, Mode::Browse);
        assert_eq!((a.cursor, a.focus, a.hung.clone(), a.detail_open, a.detail), before, "창이 보던 자리를 바꿨다");
    }

    /// **걸린 거름망으로 좁혀 세고 제목이 그 글을 댄다** — 닫아도 거름망은 그대로 걸려 있다.
    #[test]
    fn a_hung_filter_narrows_what_the_window_counts() {
        let mut a = app();
        a.apply(&Mode::Filter(Input::new("tag=bug"))).unwrap();
        a.hit("SPC p s");
        assert_eq!(window(&a).stats().rows, 1, "거름망을 안 따랐다");
        assert_eq!(window(&a).filter.as_deref(), Some("tag=bug"));
        a.hit("Esc");
        assert!(matches!(&a.hung, Some(Hung::Filter { text, .. }) if text == "tag=bug"), "닫으며 거름망이 풀렸다");

        // 검색(`/`)도 거르개다 — 뱃지 글 그대로 제목에 선다.
        a.apply(&Mode::Grep(Input::new("0003"), crate::query::GrepIn::All)).unwrap();
        a.hit("SPC p s");
        assert_eq!(window(&a).stats().rows, 1);
        assert_eq!(window(&a).filter.as_deref(), Some("/0003"));
    }

    /// **노트는 탐색기가 든 것을 쓴다** — `model:` 줄의 토큰이 CLI 와 같은 꼴로 센다. 울타리 안의 줄은 예라 안 센다.
    #[test]
    fn the_window_reads_work_from_the_notes_the_explorer_holds() {
        let mut a = app();
        let notes = [(
            "argos-0001".to_string(),
            vec![
                "model: anthropic/opus-5 tokens=1000 (high — x)\n```\nmodel: anthropic/opus-5 tokens=9 (low — 예)\n```"
                    .to_string(),
            ],
        )];
        a.site.ground.hand_notes(notes.into_iter().collect());
        a.hit("SPC p s");
        assert_eq!(window(&a).stats().work.all, Tally { lines: 1, tokened: 1, tokens: Some(1000) });
    }

    /// **어떤 크기에서도 터지지 않는다** — 좁으면 차트 대신 글로 떨어지고, 낮으면 굴린다. 넓으면 막대마다 수와
    /// 이름(`+`·`✓`)이 서고, 0 인 막대에도 `0` 이 선다.
    #[test]
    fn the_window_draws_at_any_size_and_falls_back_to_text_when_narrow() {
        let mut a = app();
        a.hit("SPC p s");
        for (w, h) in [(1, 1), (4, 3), (12, 6), (30, 8), (49, 20), (50, 20), (60, 4), (80, 24), (140, 60)] {
            let screen = super::super::draw::tests::render(&mut a, w, h).join("\n");
            assert!(matches!(a.mode, Mode::Stats(_)), "{w}x{h} 에서 창이 닫혔다");
            if w >= 80 && h >= 24 {
                assert!(screen.contains("통계"), "{w}x{h} 에 제목이 없다\n{screen}");
            }
            let _ = screen;
        }
        // 넓은 창 — 차트다. 셋을 다 9월 1일에 만들었고(08-31 주) 하나를 9월 3일에 닫았다. 칸마다 두 막대에
        // 수가 서고, 0 인 막대에도 `0` 이 **그 막대 밑에** 선다 — 칸 이름 줄과 같은 걸음으로.
        let wide = super::super::draw::tests::render(&mut a, 140, 60);
        let all = wide.join("\n");
        assert!(all.contains("+ 생성") && all.contains("✓ 완료"), "범례가 없다\n{all}");
        assert!(all.contains("· todo"), "칸 막대에 이름이 없다\n{all}");
        let days = wide.iter().position(|l| l.contains("08-31")).expect("칸 이름 줄이 없다");
        let values: Vec<String> = wide[days - 2]
            .split(|c: char| c.is_whitespace() || c == '┃' || "█▁▂▃▄▅▆▇".contains(c))
            .filter(|w| !w.is_empty())
            .map(str::to_string)
            .collect();
        let mut want = vec!["0"; 12];
        want.extend(["3", "1", "0", "0"]);
        assert_eq!(values, want, "막대 밑의 수가 칸과 안 맞는다\n{all}");
        // 수는 제 칸 밑에 선다 — 칸 이름(날짜)의 첫 글자와 그 칸의 첫 막대의 수가 같은 걸음으로 늘어선다.
        let col = |line: &str, needle: &str| line.find(needle).map(|b| line[..b].chars().count());
        let label_at = col(&wide[days], "08-31").unwrap();
        let three_at = col(&wide[days - 2], "3").unwrap();
        assert!(three_at.abs_diff(label_at) <= 1, "08-31 칸의 수가 제 칸 밑에 안 섰다\n{all}");
        // 좁은 창 — 같은 수를 글로 낸다.
        let narrow = super::super::draw::tests::render(&mut a, 40, 40).join("\n");
        assert!(narrow.contains("센 줄  issue 3건") && narrow.contains("2026-08-31"), "글로 안 떨어졌다\n{narrow}");
    }
}
