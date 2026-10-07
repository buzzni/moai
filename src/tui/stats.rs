//! 통계 창 — `SPC g s`(moai-1hka.bq9). 목록과 상세 자리를 **통째로** 덮고, Esc 로 닫으면 보던 자리 그대로다.
//!
//! **세는 자는 CLI 와 하나다**([`crate::report::stats`]). 창이 따로 세면 같은 저장소를 두고 `moai stats` 와 창이
//! 다른 수를 낸다 — 여기는 고를 줄과 거름망, 시간대를 대고 그 둘을 부르기만 한다.
//!
//! **여는 순간 센다** — 프레임마다 세지 않는다. 그리는 쪽(`draw::stats_window`)은 센 값을 펴기만 한다: 탐색기는
//! 프레임마다 이슈 전부를 훑던 자리로 여러 번 멈췄다(moai-ropk·moai-56jf). 흐름의 칸 너비(주·날)는 둘 다 연
//! 순간에 세어 들고 있다가 키 하나로 바꾼다 — 바꿀 때 다시 세면 그사이 다시 읽힌 줄로 수가 흔들린다.
//!
//! **노트는 탐색기가 이미 든 것이 있으면 그것을 쓴다**([`super::Ground`] 의 노트, `/` 의 노트 검색이 읽는 그 한 벌).
//! 안 들었으면 CLI 와 같은 걸음으로 저널에서 `model:` 줄만 읽고(`cmd::show::journal_of_rows`), **`Ground` 에 들이지
//! 않는다** — 들이면 레이어의 프로젝트에서는 다시 읽을 자가 없어 뒤에 적힌 `model:` 줄을 영영 못 세고, 두 벌(글과
//! 접은 글)이 쓸 데 없이 남는다(`App::leave_project` 가 걷는 그것이다).
//!
//! **무엇을 세나**
//!
//! - 프로젝트 안이면 그 프로젝트. 거름망(`SPC f`·`/`)이 걸려 있으면 그것으로 좁히고 제목이 그 글을 댄다. 아니면
//!   `moai stats` 를 아무 플래그 없이 친 것과 같다
//! - 한눈 보기(`0`)면 **커서가 선 줄의 프로젝트**다 — 머리줄이면 그 프로젝트, 이슈 줄이면 그 줄이 사는
//!   프로젝트(`SPC n`·`r` 과 같은 규칙). 한눈 보기에는 거름망이 안 선다
//! - **탐색기가 든 줄을 센다** — 옆 워크트리를 겹쳐 보고 있으면 거기서 온 줄도 든다. CLI 의 `moai stats` 는
//!   안 겹치므로, 그때는 제목이 그렇다고 말한다

use super::keys::{Browse, LEADER, STATS, Stat};
use super::layer::{Depth, Told};
use super::menu;
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

    /// 휠 한 칸(moai-irrj) — **키(`j`·`k`)가 옮기는 그 굴린 자리를 옮기고**([`App::stats_key`]), 기다리던 `g` 는 버린다:
    /// 휠을 사이에 둔 `g` 와 `g` 가 `gg` 로 이으면 사람이 친 적 없는 맨 위로 가기가 선다.
    pub(super) fn roll(&mut self, by: isize) {
        self.chord.clear();
        self.scroll.by(by);
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
/// **노트는 `ground` 가 이미 든 것만 빌린다** — 노트를 보는 검색이 걸려 있으면 거는 쪽(`App::apply`)과 다시 읽는
/// 걸음(`App::follow`)이 늘 새것으로 들고 있다. 안 들었으면 CLI 와 같은 걸음으로 고른 줄의 `model:` 줄만 읽고
/// (`cmd::show::journal_of_rows`·`work_in`), `ground` 에는 아무것도 안 들인다. 노트 글에서 `model:` 줄을 읽는 법은
/// 어느 쪽이든 같은 몸이다(`model::work_in_note`).
fn count(
    issues: &[Issue],
    cfg: &Config,
    ground: &Ground,
    repo: Option<&Repo>,
    origin: &crate::worktree::Origin,
    filter: &Filter,
    zone: &crate::tz::Zone,
    now: &str,
) -> (Stats, Stats) {
    let mut wh = ground.here(issues);
    // **흐름의 날은 보는 사람의 날이다** — 화면이 시각을 그리는 그 시간대고(`SPC o t`), 거름망의 `created=<날>` 도
    // 이것으로 읽는다(`App::apply`).
    wh.zone = Some(zone);
    let rows = stats::select(issues, filter, &wh, now);
    let work: BTreeMap<String, Vec<Work>> = match (ground.notes.is_some(), repo) {
        (false, Some(repo)) => crate::cmd::show::work_in(
            &crate::cmd::show::journal_of_rows(repo, origin, rows.iter().copied(), crate::model::may_hold_work),
            rows.iter().copied(),
        ),
        _ => rows
            .iter()
            .map(|i| {
                (i.id.clone(), ground.notes_of(&i.id).iter().flat_map(|t| crate::model::work_in_note(t)).collect())
            })
            .collect(),
    };
    let ask = |bucket: Bucket| Ask { kind: filter.kind, by: Vec::new(), bucket, last: bucket.default_last() };
    (
        stats::of(&rows, &wh, cfg, &work, &ask(Bucket::Week), now),
        stats::of(&rows, &wh, cfg, &work, &ask(Bucket::Day), now),
    )
}

impl App {
    /// `SPC g s` — 창을 연다. 셀 프로젝트를 못 정하거나 못 읽으면 알림 한 줄로 까닭을 대고 안 연다.
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
            let overlaid = !site.origin.roots().is_empty();
            let (week, day) = count(
                &site.issues,
                &site.cfg,
                &site.ground,
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
        // **펼쳐 든 줄이 있으면 그것을 센다** — 화면에 선 그 줄이다. **흐름의 끝은 지금이다** — 레이어의 프로젝트는
        // 표식(스냅샷·설정·워크트리)이 움직여야 다시 읽혀, 아무도 안 쓰는 프로젝트의 `site.now` 는 며칠 묵는다. 그
        // 값으로 세우면 날마다의 흐름이 그 읽은 날에서 끝난다. 거름망이 없는 자리라 고르는 데는 시각이 안 든다.
        if let Some(site) = self.site_mut(Seat::Place(at)) {
            let overlaid = !site.origin.roots().is_empty();
            let now = crate::model::now();
            let (week, day) =
                count(&site.issues, &site.cfg, &site.ground, site.repo.as_ref(), &site.origin, &filter, &zone, &now);
            self.mode = Mode::Stats(Box::new(Window::new(name, None, overlaid, week, day)));
            return;
        }
        // **접혀 아직 안 읽은 프로젝트는 그 자리에서 읽는다** — 누른 사람은 결과를 기다리고 있다(들어가는 길
        // `App::enter_project` 와 같다). 여는 데까지만 보고(`Depth::Lean`) 줄은 같은 읽기(`App::read`)로 읽는다.
        //
        // **열어 보기만 하고 레이어의 줄은 안 건드린다** — `open_place` 는 그 줄을 손으로 세운 것으로 적어
        // (`Layer::set_by_hand`) 그 줄에 돌던 읽기의 답을 버리고 다시 줄 세우며, 요약도 다시 읽힌다. 세기만 하는 키가
        // 그 값을 치를 까닭이 없다. 못 열 때만 그 길로 간다 — 그 줄을 고쳐 세우고 까닭을 알림으로 대는 자가 거기 하나다.
        let repo = match crate::projects::open_shallow(&path, lang) {
            Ok(repo) => repo,
            Err(_) => {
                let Some(repo) = self.open_place(at, Depth::Lean) else { return };
                repo
            }
        };
        let held = Told::held(self.layer.as_ref().and_then(|l| l.told_install(&path)));
        match (self.read)(&repo, self.worktree, lang, held) {
            Ok(fresh) => {
                // **물어서 센 설치 값은 그 줄에 적는다** — 들어가는 길(`App::enter_project`)과 같은 자다. 안 적으면 다음
                // 쓸기가 같은 디렉터리에 하위 프로세스 셋을 또 띄운다.
                if let (Some(asked), Some(layer)) = (fresh.asked_install, self.layer.as_mut()) {
                    layer.stamp_install(&path, asked);
                }
                let overlaid = !fresh.origin.roots().is_empty();
                let (week, day) = count(
                    &fresh.issues,
                    &repo.config,
                    &fresh.ground,
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
    pub(super) fn project_name(&self) -> String {
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
    ///
    /// **SPC 는 메뉴를 연다**(moai-z46r, 사용자 결정) — 서는 것은 화면 고르기(`SPC g`)뿐이다(`keys::Ctx::stats`). 메뉴는
    /// 탐색의 열([`App::chord`])이고 탐색과 같은 자([`menu::feed`])로 받는다: Esc·SPC 가 닫고 Bksp 가 한 층 올라가며,
    /// 이동키는 메뉴를 닫고 그 자리에서 **창을** 굴린다(moai-y8v2). 창의 열은 따로라, 메뉴를 열면 기다리던 `g` 를 버린다.
    ///
    /// **이 키가 알림을 걷는가**를 답한다 — 부르는 쪽([`App::key`])이 거짓이면 알림을 도로 세운다. 거짓은 아무 동작도
    /// 안 돈 키다: 메뉴만 만진 키(열기·닫기·한 층 오르내리기·모르는 키), 메뉴를 닫은 이동키(그 닫는 몫, moai-y8v2),
    /// 그리고 둘째 키를 기다리는 `g` — 탐색의 규칙과 같다(`a_prefix_waiting_for_its_next_key_keeps_the_notice`). 화면을
    /// 고른 키와 창의 키(굴리기·`b`·Esc), 모르는 키는 참이다.
    pub(super) fn stats_key(&mut self, k: KeyEvent) -> bool {
        if menu::open(&self.chord) || LEADER.matches(k) {
            if let Mode::Stats(w) = &mut self.mode {
                w.chord.clear();
            }
            let rows = self.rows();
            let ctx = self.key_ctx(&rows);
            let act = menu::feed(&mut self.chord, &ctx, k);
            // 메뉴를 닫은 `g` 는 탐색의 열에 남는다 — 창의 열로 옮겨 둘째 `g` 가 `gg` 로 잇게 한다.
            if !menu::open(&self.chord) {
                let held = self.chord.held().to_vec();
                self.chord.clear();
                if let Mode::Stats(w) = &mut self.mode {
                    for k in held {
                        w.chord.feed(STATS, k);
                    }
                }
            }
            return match act {
                Some(Browse::Go(to)) => {
                    self.go(to, &rows);
                    true
                }
                Some(Browse::Step(m)) => {
                    if let Mode::Stats(w) = &mut self.mode {
                        w.scroll.go(m);
                    }
                    false
                }
                _ => false,
            };
        }
        let Mode::Stats(w) = &mut self.mode else { return false };
        match w.chord.feed(STATS, k) {
            Some(Stat::Step(m)) => w.scroll.go(m),
            Some(Stat::Bucket) => w.switch(),
            Some(Stat::Close) => self.mode = Mode::Browse,
            // 뜻 없는 키는 열을 버리므로([`super::keys::Chord::feed`]) 남은 열이 곧 "기다린다" 다.
            None => return w.chord.held().is_empty(),
        }
        true
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
    use super::super::{App, Hung, Input, Mode, Pane, draw};
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

    /// 창 폭에서 안쪽 폭을 뺀 몫 — 테두리 두 칸과 좌우 여백(상세와 같은 [`draw::left_gutter`]). 시험이 폭을 손으로 셈하면
    /// 커서 글리프의 폭이 바뀐 날 재던 경계가 말없이 옮겨 간다.
    fn edge() -> u16 {
        2 + 2 * draw::left_gutter() as u16
    }

    fn window(a: &App) -> &Window {
        match &a.mode {
            Mode::Stats(w) => w,
            other => panic!("통계 창이 안 열렸다 — {other:?}"),
        }
    }

    /// **`SPC g s` 가 창을 열고, Esc 는 보던 화면을 그대로 돌려준다** — 커서·포커스·상세의 굴린 자리·걸린
    /// 거름망을 창이 안 건드린다. 창 안의 `b` 는 흐름을 주와 날 사이에서 바꾼다.
    #[test]
    fn spc_g_s_opens_the_window_and_esc_gives_the_screen_back_as_it_was() {
        let mut a = app();
        a.cursor = 1;
        a.focus = Pane::Detail;
        let before = (a.cursor, a.focus, a.hung.clone(), a.detail_open, a.detail);

        a.hit("SPC g s");
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

    /// **통계 창 위의 SPC 는 화면 고르기만 연다**(moai-z46r, 사용자 결정) — `SPC g l`·`b` 는 창을 닫고 그 배치로 서고,
    /// `SPC g s` 는 새로 센다(주/날이 처음으로 돌아간다). 다른 묶음은 안 서서 옛 `SPC p s` 는 메뉴에서 모르는 키다.
    /// 메뉴의 Esc 는 메뉴만 닫고 창은 남는다.
    #[test]
    fn spc_over_the_window_opens_only_the_screen_menu() {
        use super::super::{keys::Screen, menu, view::Layout};
        let mut a = app();
        a.hit("SPC g s SPC");
        assert!(menu::open(&a.chord), "통계 창 위에서 SPC 가 메뉴를 안 열었다");
        let ctx = a.key_ctx(&a.rows());
        assert_eq!(ctx.screen(), Screen::Stats);
        let root = menu::entries(a.chord.held(), &ctx, &[]);
        assert_eq!(
            root.iter().map(menu::Entry::text).collect::<Vec<_>>(),
            ["+화면 [통계]"],
            "화면 묶음 말고 다른 것이 섰다"
        );
        a.hit("p");
        assert_eq!(menu::title(a.chord.held()), "SPC", "통계 창 위에서 SPC p 가 열렸다");
        a.hit("Esc");
        assert!(!menu::open(&a.chord));
        window(&a);

        // 다시 고른 통계는 새로 센다 — 바꿔 둔 칸 너비가 처음(주)으로 돌아온다.
        a.hit("b");
        assert_eq!(window(&a).bucket, Bucket::Day);
        a.hit("SPC g s");
        assert_eq!(window(&a).bucket, Bucket::Week, "SPC g s 가 창을 새로 안 셌다");
        assert!(!menu::open(&a.chord), "SPC g s 가 메뉴를 열어 뒀다");

        a.hit("SPC g b");
        assert_eq!((&a.mode, a.layout), (&Mode::Browse, Layout::Board), "SPC g b 가 창을 닫고 보드로 안 갔다");
        a.hit("SPC g s SPC g l");
        assert_eq!((&a.mode, a.layout), (&Mode::Browse, Layout::List), "SPC g l 이 창을 닫고 목록으로 안 갔다");
        // 같은 배치를 고르면 창만 닫힌다 — 토글이 아니다.
        a.hit("SPC g s SPC g l");
        assert_eq!((&a.mode, a.layout), (&Mode::Browse, Layout::List), "목록에서 연 창의 SPC g l 이 배치를 뒤집었다");
    }

    /// **창의 키 바가 SPC 를 댄다**(moai-z46r) — 안 대면 창 위에서 메뉴가 열린다는 것을 알 길이 없다. 메뉴가 뜨면 바
    /// 자리에 접두어 줄이 서고 격자가 창을 밀어 올린다 — 목록 위의 메뉴와 같은 그림이다.
    #[test]
    fn the_window_bar_names_spc_and_the_menu_draws_over_the_window() {
        let mut a = app();
        a.hit("SPC g s");
        let lines = draw::tests::render(&mut a, 80, 24);
        let bar = lines.last().expect("빈 화면");
        assert!(bar.contains("SPC 메뉴") && bar.contains("Esc 닫기"), "{bar:?}");
        a.hit("SPC");
        let lines = draw::tests::render(&mut a, 80, 24);
        let screen = lines.join("\n");
        assert!(lines.last().is_some_and(|l| l.starts_with("SPC- 메뉴")), "접두어 줄이 안 섰다\n{screen}");
        assert!(screen.contains("g : +화면 [통계]"), "격자가 안 섰다\n{screen}");
        assert!(screen.contains(" 통계 · . · "), "메뉴가 창을 덮었다\n{screen}");
        a.hit("g");
        let screen = draw::tests::render(&mut a, 80, 24).join("\n");
        assert!(screen.contains("l : 목록") && screen.contains("s : 통계"), "SPC g 층이 안 섰다\n{screen}");
    }

    /// **메뉴를 닫는 이동키는 창을 굴린다**(moai-y8v2 를 통계 창에서) — `gg` 는 메뉴가 닫힌 뒤 창의 열로 이어진다.
    /// 메뉴를 여는 SPC 는 창이 기다리던 `g` 를 버리고, Bksp 는 한 층 오르다 뿌리에서 메뉴만 닫는다. 메뉴가 뜬 동안의
    /// 휠은 `mouse::tests::the_wheel_scrolls_the_stats_window_and_nothing_over_a_picker` 가 본다.
    #[test]
    fn a_move_key_closes_the_menu_over_the_window_and_scrolls_it() {
        use super::super::menu;
        let mut a = app();
        a.hit("SPC g s");
        let _ = draw::tests::render(&mut a, 40, 12);
        a.hit("SPC j");
        assert!(!menu::open(&a.chord), "j 가 메뉴를 안 닫았다");
        assert_eq!(window(&a).scroll.offset(), 1, "j 가 창을 안 굴렸다");
        a.hit("SPC Ctrl-d");
        assert!(window(&a).scroll.offset() > 1, "Ctrl-d 가 창을 안 굴렸다");
        a.hit("SPC s");
        assert!(menu::open(&a.chord), "시험의 전제 — 통계 창 위의 SPC s 는 모르는 키라 메뉴가 남는다");
        a.hit("Esc SPC g");
        assert_eq!(menu::title(a.chord.held()), "SPC g");
        a.hit("g");
        assert!(!menu::open(&a.chord), "SPC g 층의 g 가 메뉴를 안 닫았다");
        a.hit("g");
        assert_eq!(window(&a).scroll.offset(), 0, "메뉴에서 시작한 gg 가 창의 맨 위로 안 갔다");
        assert!(a.chord.held().is_empty(), "탐색의 열에 g 가 남았다");

        // **메뉴를 열면 창이 기다리던 `g` 를 버린다** — 메뉴를 사이에 둔 `g` 와 `g` 가 `gg` 로 이으면 사람이 친 적 없는
        // 맨 위로 가기가 선다(휠의 `Window::roll` 과 같은 까닭).
        a.hit("G");
        let bottom = window(&a).scroll.offset();
        assert!(bottom > 0, "시험의 전제 — 창이 맨 위가 아니다");
        a.hit("g SPC Esc g");
        assert_eq!(window(&a).scroll.offset(), bottom, "메뉴를 사이에 둔 g 와 g 가 gg 로 이었다");
        // 메뉴의 Bksp 는 한 층 오르고, 뿌리에서는 메뉴만 닫는다 — 창은 남는다.
        a.hit("SPC g Bksp");
        assert_eq!(menu::title(a.chord.held()), "SPC", "SPC g 의 Bksp 가 뿌리로 안 올랐다");
        a.hit("Bksp");
        assert!(!menu::open(&a.chord), "뿌리의 Bksp 가 메뉴를 안 닫았다");
        assert_eq!(window(&a).scroll.offset(), bottom, "메뉴의 Bksp 가 창을 건드렸다");
    }

    /// **바로 친 Ctrl·Alt 화살표와 쪽 키도 창을 굴린다**(moai-ug6x.3ip) — 메뉴를 거친 같은 키는 탐색의 표(`Key::any`)로
    /// 굴렸는데 창의 표만 `bare` 라 바로 친 것은 아무 일도 없었다. 글자는 정확히 견주는 그대로다(Ctrl-j 는 안 굴린다).
    #[test]
    fn ctrl_and_alt_arrows_scroll_the_window_pressed_directly() {
        use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers as M};
        let mut a = app();
        a.hit("SPC g s");
        let _ = draw::tests::render(&mut a, 40, 12);
        a.key(KeyEvent::new(KeyCode::Down, M::CONTROL));
        assert_eq!(window(&a).scroll.offset(), 1, "Ctrl-Down 이 창을 안 굴렸다");
        a.key(KeyEvent::new(KeyCode::PageDown, M::ALT));
        assert!(window(&a).scroll.offset() > 1, "Alt-PageDown 이 창을 안 굴렸다");
        let at = window(&a).scroll.offset();
        a.key(KeyEvent::new(KeyCode::Char('j'), M::CONTROL));
        assert_eq!(window(&a).scroll.offset(), at, "Ctrl-j 가 창을 굴렸다");
    }

    /// **창 위의 메뉴도 알림을 탐색과 같은 자로 다룬다**(moai-g56h·moai-y8v2) — 메뉴만 만진 키(열기·내려가기·Bksp·Esc)와
    /// 메뉴를 닫은 이동키, 둘째 키를 기다리는 창의 `g` 는 알림을 안 걷는다. 걷는 것은 창의 키와 화면을 고른 키다.
    #[test]
    fn the_menu_over_the_window_keeps_the_notice_until_a_key_acts() {
        let mut a = app();
        a.hit("SPC g s");
        let _ = draw::tests::render(&mut a, 40, 12);
        let said = "알림".to_string();
        a.notice = Some(said.clone());
        for keys in ["SPC", "g", "Bksp", "Esc", "SPC j", "g"] {
            a.hit(keys);
            assert_eq!(a.notice.as_deref(), Some(said.as_str()), "{keys:?} 가 알림을 걷었다");
        }
        a.hit("g");
        assert_eq!(window(&a).scroll.offset(), 0, "시험의 전제 — 창의 gg 가 맨 위로 갔다");
        assert_eq!(a.notice, None, "창의 키가 알림을 안 걷었다");

        a.notice = Some(said);
        a.hit("SPC g s");
        assert_eq!(a.notice, None, "화면을 고른 키가 알림을 안 걷었다");
    }

    /// **걸린 거름망으로 좁혀 세고 제목이 그 글을 댄다** — 닫아도 거름망은 그대로 걸려 있다.
    #[test]
    fn a_hung_filter_narrows_what_the_window_counts() {
        let mut a = app();
        a.apply(&Mode::Filter(Input::new("tag=bug"))).unwrap();
        a.hit("SPC g s");
        assert_eq!(window(&a).stats().rows, 1, "거름망을 안 따랐다");
        assert_eq!(window(&a).filter.as_deref(), Some("tag=bug"));
        a.hit("Esc");
        assert!(matches!(&a.hung, Some(Hung::Filter { text, .. }) if text == "tag=bug"), "닫으며 거름망이 풀렸다");

        // 검색(`/`)도 거르개다 — 뱃지 글 그대로 제목에 선다.
        a.apply(&Mode::Grep(Input::new("0003"), crate::query::GrepIn::All)).unwrap();
        a.hit("SPC g s");
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
        a.hit("SPC g s");
        assert_eq!(window(&a).stats().work.all, Tally { lines: 1, tokened: 1, tokens: Some(1000) });
    }

    /// **어떤 크기에서도 터지지 않는다** — 좁으면 차트 대신 글로 떨어지고, 낮으면 굴린다. 넓으면 막대마다 수와
    /// 이름(`+`·`✓`)이 서고, 0 인 막대에도 `0` 이 선다.
    #[test]
    fn the_window_draws_at_any_size_and_falls_back_to_text_when_narrow() {
        let mut a = app();
        a.hit("SPC g s");
        // `edge` 와 그 한 칸 위는 여백이 안쪽을 0칸·1칸만 남기는 폭이고, `cut` 의 앞뒤는 차트와 글이 갈리는 폭이다
        // ([`draw::STATS_CHART_W`]). 둘 다 [`edge`] 에서 셈한다 — 커서 글리프의 폭이 바뀌어도 같은 경계를 잰다.
        let cut = draw::STATS_CHART_W + edge();
        for (w, h) in [
            (1, 1),
            (4, 3),
            (edge(), 3),
            (edge() + 1, 3),
            (12, 6),
            (30, 8),
            (cut - 1, 20),
            (cut, 20),
            (60, 4),
            (80, 24),
            (140, 60),
        ] {
            let screen = draw::tests::render(&mut a, w, h).join("\n");
            assert!(matches!(a.mode, Mode::Stats(_)), "{w}x{h} 에서 창이 닫혔다");
            if w >= 80 && h >= 24 {
                assert!(screen.contains("통계"), "{w}x{h} 에 제목이 없다\n{screen}");
            }
            let _ = screen;
        }
        // 넓은 창 — 차트다. 셋을 다 9월 1일에 만들었고(08-31 주) 하나를 9월 3일에 닫았다. 칸마다 두 막대에
        // 수가 서고, 0 인 막대에도 `0` 이 **그 막대 밑에** 선다 — 칸 이름 줄과 같은 걸음으로.
        let wide = draw::tests::render(&mut a, 140, 60);
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
        // 좁은 창 — 같은 수를 글로 낸다. **창보다 긴 줄은 접는다** — 칸 줄의 `✓ done 1` 과 소요 줄의 까닭이 창 밖으로
        // 잘려 사라지던 자리다(옆으로 굴릴 길이 없다).
        let narrow = draw::tests::render(&mut a, 40, 40).join("\n");
        assert!(narrow.contains("센 줄  issue 3건") && narrow.contains("2026-08-31"), "글로 안 떨어졌다\n{narrow}");
        assert!(narrow.contains("✓ done 1"), "좁은 창이 칸 줄의 꼬리를 잘랐다\n{narrow}");
        assert!(narrow.contains("적혔다"), "좁은 창이 소요 줄의 까닭을 잘랐다\n{narrow}");
    }

    /// **수가 막대보다 먼저 자리를 얻는다** — 이름 열과 막대 몇 칸을 먼저 떼어 두던 판은 긴 모델 이름 곁의 토큰 합을
    /// 통째로 빼거나(이 폭에서는 `2줄 · 토큰` 까지만 섰다) `40,957` 처럼 그럴듯한 다른 수로 잘랐다. 이름은 `…` 로
    /// 잘려도 수는 온전히 선다.
    #[test]
    fn numbers_keep_their_digits_beside_a_long_model_name() {
        let mut a = app();
        let notes = [(
            "argos-0001".to_string(),
            vec![
                "model: anthropic/claude-sonnet-4-5-20250929 tokens=40957750 (high — x)\nmodel: anthropic/claude-sonnet-4-5-20250929 (high — y)"
                    .to_string(),
            ],
        )];
        a.site.ground.hand_notes(notes.into_iter().collect());
        a.hit("SPC g s");
        // 안쪽 폭 58 — 이 폭에서 이름이 `anthr…` 로 잘린다. 창 폭은 그 안쪽에 [`edge`] 를 더한 것이다.
        let screen = draw::tests::render(&mut a, 58 + edge(), 80).join("\n");
        // 막대 곁의 글 통째로 — 머리 줄(`AI 작업 …`)에도 같은 합이 서므로 막대 쪽에만 있는 머리(`2줄 · `)부터 잰다.
        assert!(screen.contains("2줄 · 토큰 40,957,750 — 1줄의 합 (안 적은 줄 1)"), "막대 곁의 수가 잘렸다\n{screen}");
        assert!(screen.contains("anthr…"), "넘친 모델 이름을 잘렸다고 안 댔다\n{screen}");
    }
}
