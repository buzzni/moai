//! 간트 배치 — `SPC g g`(moai-i4hf). 목록을 **모두 펼친 채** 왼쪽에 고정하고, 오른쪽에 날짜 축과 막대를 둔다.
//!
//! **새 창이 아니라 목록의 배치다**([`super::view::Layout::Gantt`]). 줄은 목록과 같은 몸(`App::rows_in`)이 세우고,
//! 커서·거름망·보기·검색·상세가 한 벌이라 `SPC g l` ↔ `SPC g g` 를 오가도 자리를 잃지 않는다. 다른 것은 펼침
//! 하나다 — 간트는 접은 자리(`Site::gantt_shut`)를 따로 들어, 들어갈 때 모두 펼치고 목록의 접기는 그대로 둔다.
//!
//! **날짜를 판단하지 않는다.** 계획이 어디서 왔는지, 시작을 아는지, 늦었는지는 [`crate::report::gantt`] 가 정하고,
//! 여기는 그 값을 위젯의 줄([`moai_gantt::GanttRow`])로 옮기기만 한다 — 날짜를 지역 기준 초로, 칸을 색 칸으로.
//!
//! 조각이 아니다 — `App`·`Site` 를 고치고 저장소의 줄(`crate::model`)을 읽는다(`input::NOT_COMPONENTS`).

use super::keys::{self, Pan};
use super::{App, Mode, Path, Row, Seat, Site, Touched};
use crate::i18n::{fill, say};
use crate::model::{Edge, Kind, Unshifted, parse_date, parse_rfc3339};
use crate::report::gantt::{Actual, Origin};
use moai_gantt::{GanttRow, Link, RowKind, Tone};
use ratatui::text::Line;

/// 하루의 초 — 위젯의 축이 재는 단위다.
const DAY: i64 = 86_400;

/// 날짜 칸이 이보다 좁아지면 고정 칸을 줄이고, 그래도 모자라면 날짜 칸을 감춘다(moai-i4hf.25n). 한 주(Week 단계의
/// 일곱 칸) 남짓을 두 번 보일 폭이다 — 그보다 좁은 날짜 칸은 막대가 어디쯤인지 못 댄다.
const MIN_DATES: u16 = 20;

/// 고정 칸이 줄어드는 바닥 — 커서·칸 글리프·트리 선 뒤에 제목 머리가 남는 폭. 이보다 줄이면 열이 다 걷혀도 제목이
/// 안 서, 막대가 어느 줄의 것인지 못 읽는다 — 그때는 날짜 칸이 먼저 물러난다.
const MIN_LABEL: u16 = 20;

/// 본문이 이보다 적은 줄만 남는 낮은 창에서는 머리글을 두 줄로 줄인다 — 본문 한 줄이 머리의 오늘(`▼`) 줄보다 값지다.
const LOW_BODY: u16 = 4;

/// 쓰려는 계획 옮기기 하나(moai-i4hf.9xi). 누군지 물으러 갔다 오면 이것을 다시 쓴다.
///
/// **날이 아니라 옮기는 뜻을 든다**([`Plan`]) — 고칠 날은 쓰기 안에서 **그때의 줄로** 다시 잰다(`Issue::shifted`). 화면의
/// 줄로 미리 센 날을 들고 가면, 그새 옆 세션이 고친 날 위에 화면의 옛 날에서 센 값을 덮어 그 고침이 말없이 사라진다 —
/// 다시 읽기를 기다리는 틈과 누군지 묻는 틈이 그 자리다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PlanMove {
    pub id: String,
    pub plan: Plan,
}

/// 계획 옮기기의 뜻.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Plan {
    /// 제 계획을 날 수만큼 옮긴다 — 어느 날이 옮는지는 [`crate::model::Issue::shifted`] 가 정한다.
    Shift(Edge, i64),
    /// 날이 하나도 안 적힌 줄에 그 하루(1970-01-01 부터의 날 수)를 시작·기한으로 세운다. 쓰는 순간 날이 이미 적혀
    /// 있으면 안 쓴다 — 쓰면 그 날을 덮는다.
    Day(i64),
}

/// 옮길 수 없는 까닭의 글 — 키·끌기의 알림과 쓰기 안의 거절이 같은 말을 댄다.
fn unshifted(lang: crate::i18n::Lang, why: Unshifted, id: &str) -> String {
    let said = match why {
        Unshifted::Members => say(lang, "tui.gantt.members"),
        Unshifted::NoPlan | Unshifted::Still => say(lang, "tui.gantt.no_plan"),
    };
    fill(said, &[("id", id)])
}

/// 잡은 막대의 부분 → 옮기는 날.
/// 간트 머리의 달 이름과 그것을 엮는 차례를 화면 말로 채운다. 위젯의 기본은 영어 약어다.
pub(super) fn names_in(lang: crate::i18n::Lang) -> moai_gantt::Names {
    moai_gantt::Names {
        // 키는 `say` 에 글자째 적어야 번역표 시험이 부르는 키로 센다 — 반복문으로 만들지 않는다.
        months: [
            say(lang, "tui.gantt.month_1"),
            say(lang, "tui.gantt.month_2"),
            say(lang, "tui.gantt.month_3"),
            say(lang, "tui.gantt.month_4"),
            say(lang, "tui.gantt.month_5"),
            say(lang, "tui.gantt.month_6"),
            say(lang, "tui.gantt.month_7"),
            say(lang, "tui.gantt.month_8"),
            say(lang, "tui.gantt.month_9"),
            say(lang, "tui.gantt.month_10"),
            say(lang, "tui.gantt.month_11"),
            say(lang, "tui.gantt.month_12"),
        ]
        .map(String::from),
        month_year: say(lang, "tui.gantt.month_year").to_string(),
        month_day: say(lang, "tui.gantt.month_day").to_string(),
    }
}

fn edge_of(part: moai_gantt::Part) -> Edge {
    match part {
        moai_gantt::Part::Start => Edge::Start,
        moai_gantt::Part::Body => Edge::Body,
        moai_gantt::Part::End => Edge::End,
    }
}

/// 계획 날 둘(날 번호) → 막대의 `[시작, 끝)`(초). 시작만 적은 줄은 그 하루다 — 끝을 모르는 막대를 길게 그리면 없는
/// 기한을 지어낸다. 읽은 줄([`fill_row`])과 끄는 막대([`preview`])가 이 하나로 잰다.
fn plan_span(start: Option<i64>, end: Option<i64>) -> Option<(i64, i64)> {
    match (start, end) {
        (Some(s), Some(e)) => Some((s * DAY, (e + 1) * DAY)),
        (Some(s), None) => Some((s * DAY, (s + 1) * DAY)),
        _ => None,
    }
}

/// 끄는 막대를 **놓으면 쓸 날로** 그린다(moai-i4hf.exq) — 셈은 쓰기와 같은 [`crate::model::Issue::shifted`] 다. 위젯의 끌기
/// 셈(`drag_apply`)으로 그리던 판은 0 길이 막대(시작이 기한을 하루 넘는다)·시작만 든 줄·멤버에서 센 묶음에서 그린 날과 쓴
/// 날이 갈렸다. 쓰기의 검사가 거절할 모양(시작이 기한 뒤)이면 옮기지 않은 채 둔다.
fn preview(g: &mut GanttRow, i: &crate::model::Issue, (start, due): crate::model::DateChange) {
    let start = start.unwrap_or_else(|| i.starts_on.clone());
    let due = due.unwrap_or_else(|| i.due_on.clone());
    let (s, e) = (start.as_deref().and_then(parse_date), due.as_deref().and_then(parse_date));
    if s.zip(e).is_some_and(|(s, e)| s > e) {
        return;
    }
    g.planned = plan_span(s, e);
    g.due = e.map(|e| (e + 1) * DAY);
}

impl Site {
    /// 묶음 하나를 펴거나 접는다 — 목록이면 펼친 자리(`expanded`)에, 간트면 접은 자리(`gantt_shut`)에 적는다.
    /// 두 집합의 뜻이 거꾸로라 적는 쪽을 여기 하나에 모은다: 키마다 가르면 새 키 하나가 그것을 잊는다.
    pub(super) fn set_open(&mut self, gantt: bool, path: Path, open: bool) {
        match (gantt, open) {
            (false, true) => {
                self.expanded.insert(path);
            }
            (false, false) => {
                self.expanded.remove(&path);
            }
            (true, true) => {
                self.gantt_shut.remove(&path);
            }
            (true, false) => {
                self.gantt_shut.insert(path);
            }
        }
    }
}

/// 간트 위의 끌기(moai-i4hf.exq). 끄는 동안 [`App::chart_drag`] 가 든다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ChartDrag {
    /// 날짜 머리를 끌어 축을 민다 — 누른 칸과 그때의 축 원점. 손이 간 만큼 날짜가 따라온다.
    Axis { from: u16, origin: i64 },
    /// 고정 칸과 날짜 칸 사이의 선 — 누른 칸과 그때의 고정 칸 폭.
    Split { from: u16, width: u16 },
    /// 막대 — 그 줄의 id, 잡은 부분, 누른 날짜 칸, 지금 날짜 칸. **끄는 동안은 그리기만 하고**
    /// (`App::chart_of`), 놓을 때 한 번 쓴다([`App::drop_chart`]). 줄은 **id 로** 든다 — 끄는 동안 다시 읽어 줄의 차례가
    /// 바뀌어도 그리는 쪽과 쓰는 쪽이 같은 줄을 찾는다.
    Bar { id: String, part: moai_gantt::Part, from: i64, now: i64 },
}

/// 위젯에 넘길 한 판 — 줄([`App::rows`] 와 같은 차례, 같은 길이), 막음, 맞출 폭, 지금.
pub(super) struct Chart {
    pub rows: Vec<GanttRow>,
    pub links: Vec<Link>,
    /// 보이는 줄의 `[가장 이른, 가장 늦은]` 지역 기준 초 — 맞춤(`z`)이 쓴다. 날이 하나도 없으면 없다.
    pub range: Option<(i64, i64)>,
    pub now: i64,
}

impl App {
    /// 그 묶음이 지금 배치에서 펼쳐져 있는가 — 검색이 저절로 연 것은 따로 센다(`App::searched_open`).
    pub(super) fn opened(&self, site: &Site, path: &Path) -> bool {
        if self.gantt() { !site.gantt_shut.contains(path) } else { site.expanded.contains(path) }
    }

    /// 간트로 들어간다 — **모두 펼친다**. 지금 프로젝트와 한눈 보기에 이미 읽은 프로젝트 모두의 접은 자리를
    /// 걷는다. 목록의 펼침(`expanded`)은 안 건드린다 — 목록으로 돌아오면 접어 둔 그대로 선다.
    pub(super) fn open_gantt(&mut self) {
        self.site.gantt_shut.clear();
        if let Some(layer) = self.layer.as_mut() {
            for site in layer.places.iter_mut().filter_map(|p| p.site.as_mut()) {
                site.gantt_shut.clear();
            }
        }
    }

    /// 이 자리에 그릴 보기 상태(moai-i4hf.25n) — 사람이 고른 것(`App::chart`)을 창에 맞춘다. **그리기·키·마우스가 이
    /// 하나로 잰다** — 한쪽만 맞추면 그린 막대와 누른 막대가 어긋난다.
    ///
    /// - 날짜 칸이 [`MIN_DATES`] 보다 좁아지면 고정 칸을 [`MIN_LABEL`] 까지 줄인다 — 고정 칸의 열은 폭을 따라 담당 →
    ///   진행 → id 차례로 걷힌다(목록의 `Head::of`)
    /// - 그래도 모자라면 고정 칸이 창을 다 쓰고 날짜 칸은 0 폭이다([`App::dates_hidden`])
    /// - 낮은 창에서는 머리글이 두 줄이다([`LOW_BODY`])
    ///
    /// 사람이 고른 폭(`label_width`)은 안 바꾼다 — 창을 넓히면 그 폭으로 돌아온다.
    pub(super) fn chart_view(&self, area: ratatui::layout::Rect) -> moai_gantt::GanttState {
        let mut st = self.chart;
        let room = area.width.saturating_sub(1);
        let want = st.label_width.min(room);
        st.label_width = if room - want >= MIN_DATES {
            want
        } else if room >= MIN_LABEL + MIN_DATES {
            room - MIN_DATES
        } else {
            area.width
        };
        st.short_header = area.height < moai_gantt::HEADER_ROWS + LOW_BODY;
        st
    }

    /// 창이 좁아 날짜 칸을 감췄는가.
    pub(super) fn dates_hidden(&self, area: ratatui::layout::Rect) -> bool {
        moai_gantt::Layout::of(area, &self.chart_view(area)).dates_width() == 0
    }

    /// 간트의 지금 — 적재 때 잡은 시각(`Site::now`)을 보는 사람의 벽시계로 옮긴 지역 기준 초.
    fn chart_now(&self) -> i64 {
        parse_rfc3339(&self.site.now).map_or(0, |t| self.zone.local(t))
    }

    /// 날짜 축을 움직인다(moai-i4hf.27r). 폭은 **지난 프레임이 그린 자리**로 잰다(`Drawn::chart`) — 키는 그 화면을 본
    /// 사람의 손이다. 아직 안 그렸으면 맞춤만 깃발을 세우고 나머지는 아무 일도 안 한다: 잴 폭이 없다.
    ///
    /// 확대의 기준점은 **날짜 칸의 가운데**다 — 그 시각이 그 칸에 그대로 남는다.
    pub(super) fn pan(&mut self, p: Pan, rows: &[Row]) {
        let Some(area) = self.drawn.chart else {
            if p == Pan::Fit {
                self.chart.fitting = true;
            }
            return;
        };
        let w = moai_gantt::Layout::of(area, &self.chart_view(area)).dates_width();
        let mid = f64::from(w / 2);
        let anchor = self.chart.axis.secs_at_cell(mid);
        match p {
            Pan::ZoomIn => self.chart.zoom_in(anchor, mid),
            Pan::ZoomOut => self.chart.zoom_out(anchor, mid),
            Pan::Left => self.chart.scroll_cells(-1),
            Pan::Right => self.chart.scroll_cells(1),
            Pan::PageLeft => self.chart.scroll_pages(-1, w),
            Pan::PageRight => self.chart.scroll_pages(1, w),
            Pan::Today => {
                let now = self.chart_now();
                self.chart.to_today(now, w);
            }
            Pan::Fit => {
                let range = self.chart_of(rows, 0..0, &|_, _| Line::default()).range;
                self.chart.fit(range, w);
            }
        }
    }

    /// 커서 줄의 계획을 하루 옮긴다(moai-i4hf.9xi) — `<`·`>`·`{`·`}`. 어느 날이 옮는지는 `Issue::shifted` 가 정한다.
    pub(super) fn nudge(&mut self, n: keys::Nudge, rows: &[Row]) {
        let (edge, days) = match n {
            keys::Nudge::Earlier => (Edge::Body, -1),
            keys::Nudge::Later => (Edge::Body, 1),
            keys::Nudge::StartEarlier => (Edge::Start, -1),
            keys::Nudge::StartLater => (Edge::Start, 1),
        };
        if let Some(at) = self.here_at(rows, self.cursor) {
            self.shift_plan(at, edge, days);
        }
    }

    /// 목록의 n 번째 줄이 **지금 선 프로젝트의** 줄이면 그 첨자. 한눈 보기의 줄은 남의 프로젝트라 여기서 안 쓴다.
    pub(super) fn here_at(&self, rows: &[Row], n: usize) -> Option<usize> {
        match rows.get(n) {
            Some(Row::Item(Seat::Here, e, _)) => e.at().filter(|&at| at < self.site.issues.len()),
            _ => None,
        }
    }

    /// 그 줄의 계획을 옮겨 쓴다. 못 옮기면(날이 없다·멤버에서 센 막대다) 까닭을 알림으로 댄다 — 조용히 먹으면 키가
    /// 고장 난 것으로 보인다.
    pub(super) fn shift_plan(&mut self, at: usize, edge: Edge, days: i64) {
        let i = &self.site.issues[at];
        match i.shifted(edge, days) {
            Ok(_) => self.write_plan(PlanMove { id: i.id.clone(), plan: Plan::Shift(edge, days) }),
            Err(Unshifted::Still) => {}
            Err(why) => self.notice = Some(unshifted(self.site.lang, why, &i.id)),
        }
    }

    /// 간트 위의 손짓(moai-i4hf.exq). **받았으면 참이다** — 아니면 부르는 쪽(`mouse::pointer`)이 목록처럼 다룬다(세로 휠·
    /// 상세 칸의 누르기).
    ///
    /// - 고정 칸의 줄이나 막대를 누르면 커서가 그 줄로 간다 — 목록의 누르기와 같은 길(`App::move_to`)이다
    /// - 막대를 끌면 계획이 옮는다: 몸통은 둘 다, 왼쪽 끝은 시작만, 오른쪽 끝은 기한만. 날 단위로 붙고(`drag_days`), 끄는
    ///   동안은 그리기만 하고 놓을 때 한 번 쓴다. 쓰는 길은 키(`<`·`>`)와 같다([`App::shift_plan`])
    /// - 계획 날짜가 없는 줄의 빈 날을 누르면 그 날을 시작·기한으로 하루짜리 계획을 세운다
    /// - 날짜 머리를 끌면 축이 손을 따라 밀리고, 고정 칸과 날짜 칸 사이 선을 끌면 고정 칸 폭이 바뀐다
    /// - 가로 휠(`sideways`, 그리고 Shift+휠)은 축을 민다. 세로 휠은 목록과 같다 — 받지 않는다
    pub(super) fn chart_pointer(
        &mut self,
        kind: ratatui::crossterm::event::MouseEventKind,
        sideways: Option<isize>,
        at: ratatui::layout::Position,
    ) -> bool {
        use ratatui::crossterm::event::{MouseButton, MouseEventKind as K};
        if !self.gantt() {
            return false;
        }
        let Some(area) = self.drawn.chart else { return false };
        let laid = moai_gantt::Layout::of(area, &self.chart_view(area));
        if let (K::Drag(MouseButton::Left), Some(drag)) = (kind, self.chart_drag.clone()) {
            let dx = i64::from(at.x)
                - i64::from(match &drag {
                    ChartDrag::Axis { from, .. } | ChartDrag::Split { from, .. } => *from,
                    ChartDrag::Bar { .. } => at.x,
                });
            match drag {
                ChartDrag::Axis { origin, .. } => {
                    self.chart.fitting = false;
                    self.chart.axis.origin = origin - dx * self.chart.axis.zoom.secs_per_cell();
                }
                ChartDrag::Split { width, .. } => {
                    let w = (i64::from(width) + dx).clamp(1, i64::from(area.width.saturating_sub(2).max(1))) as u16;
                    self.chart.set_label_width(w);
                    self.gantt_label = Some(w);
                }
                ChartDrag::Bar { id, part, from, .. } => {
                    let now = i64::from(at.x) - i64::from(laid.dates_body.x);
                    self.chart_drag = Some(ChartDrag::Bar { id, part, from, now });
                }
            }
            return true;
        }
        if !area.contains(at) {
            return false;
        }
        if let Some(by) = sideways {
            self.chart.scroll_cells(by as i64);
            return true;
        }
        if !matches!(kind, K::Down(MouseButton::Left)) {
            return false;
        }
        self.focus = super::Pane::Explorer;
        let rows = self.rows();
        let chart = self.chart_of(&rows, 0..0, &|_, _| Line::default());
        let col = |x: u16| i64::from(x) - i64::from(laid.dates_body.x);
        match moai_gantt::hit(area, &self.chart_view(area), &chart.rows, chart.now, at.x, at.y) {
            moai_gantt::Hit::Splitter => {
                self.chart_drag = Some(ChartDrag::Split { from: at.x, width: laid.labels_body.width });
            }
            moai_gantt::Hit::Axis { .. } => {
                self.chart_drag = Some(ChartDrag::Axis { from: at.x, origin: self.chart.axis.origin });
            }
            moai_gantt::Hit::Label { row } => self.move_to(row),
            moai_gantt::Hit::Bar { row, part } => {
                self.move_to(row);
                if let Some(i) = self.here_at(&rows, row) {
                    let id = self.site.issues[i].id.clone();
                    let c = col(at.x);
                    self.chart_drag = Some(ChartDrag::Bar { id, part, from: c, now: c });
                }
            }
            moai_gantt::Hit::Empty { row, day } => {
                self.move_to(row);
                if let Some(i) = self.here_at(&rows, row) {
                    self.plan_day(i, day);
                }
            }
            moai_gantt::Hit::None => {}
        }
        true
    }

    /// 계획 날짜가 없는 줄의 빈 날을 눌렀다 — 그 날 하루를 계획으로 세운다(moai-i4hf.9xi). 날이 이미 있거나(막대 밖의 빈
    /// 칸을 누른 것이다) 멤버에서 센 묶음이면 커서만 옮긴 것이다.
    fn plan_day(&mut self, at: usize, day: i64) {
        let i = &self.site.issues[at];
        if i.has_days() || crate::report::is_group(i) {
            return;
        }
        self.write_plan(PlanMove { id: i.id.clone(), plan: Plan::Day(day) });
    }

    /// 간트의 끌기를 놓는다(moai-i4hf.exq) — 막대면 **그때 한 번** 트래커에 쓰고, 나눔 선이면 그때 한 번 고정 칸 폭을
    /// 사용자 설정에 적는다(목록·상세 사이 선과 같은 결, `App::drop_line`). 축 끌기는 화면의 것이라 적을 것이 없다.
    /// [`App::drop_line`] 이 부른다 — 뗌을 놓친 터미널에서도 다음 사건·키가 놓는다.
    pub(super) fn drop_chart(&mut self) {
        let (id, part, from, now) = match self.chart_drag.take() {
            Some(ChartDrag::Bar { id, part, from, now, .. }) => (id, part, from, now),
            Some(ChartDrag::Split { .. }) => return self.save_look(),
            Some(ChartDrag::Axis { .. }) | None => return,
        };
        let days = moai_gantt::drag_days(self.chart.axis.zoom, from, now);
        let Some(at) = self.site.index.find(&id) else { return };
        self.shift_plan(at, edge_of(part), days);
    }

    /// 계획 옮기기를 쓴다 — [`commit_plan`] 이 쓰기 길(`App::write` → `store::with_write`)을 탄다.
    pub(super) fn write_plan(&mut self, m: PlanMove) {
        self.plan_move = Some(m);
        commit_plan(self);
    }

    /// 목록의 줄을 위젯의 줄로 옮긴다. 고정 칸의 글(`label`)은 그리는 쪽이 짓는다 — 목록의 줄과 같은 자로 재야
    /// 해서다(`draw::row_line`). `window` 밖의 줄은 글을 안 짓는다: 위젯은 보이는 창만 그린다.
    ///
    /// **프로젝트마다 한 번 읽는다**(`report::gantt`) — 한눈 보기의 줄은 여러 프로젝트에서 온다. 지금은 프레임마다가
    /// 아니라 **적재 때 잡은 시각**(`Site::now`)이다 — 통계 창과 같은 시계라 오늘 선이 초마다 흔들리지 않는다.
    pub(super) fn chart_of(
        &self,
        rows: &[Row],
        window: std::ops::Range<usize>,
        label: &dyn Fn(usize, &Row) -> Line<'static>,
    ) -> Chart {
        let utc_now = self.site.now.clone();
        let now = self.chart_now();
        let mut seats: Vec<Seat> = Vec::new();
        for r in rows {
            if let Row::Item(seat, ..) = r
                && !seats.contains(seat)
            {
                seats.push(*seat);
            }
        }
        let sites: Vec<(Seat, &Site)> =
            seats.iter().filter_map(|&s| self.site_of_seat(s).map(|site| (s, site))).collect();
        let wheres: Vec<crate::query::Where> = sites.iter().map(|(_, s)| s.ground.here(&s.issues)).collect();
        let reads: Vec<crate::report::gantt::Gantt> = sites
            .iter()
            .zip(&wheres)
            .map(|((_, s), wh)| crate::report::gantt::gantt(&s.issues, &s.index, wh, &s.cfg, &utc_now, &self.zone))
            .collect();
        let read_of = |seat: Seat| sites.iter().position(|(s, _)| *s == seat);

        let mut out = Vec::with_capacity(rows.len());
        let mut kept = Vec::new();
        // 막음을 그릴 때 막는 줄의 자리를 id 로 찾는다 — 같은 프로젝트 안에서만 잇는다.
        let mut place: std::collections::HashMap<(usize, &str), usize> = std::collections::HashMap::new();
        for (n, r) in rows.iter().enumerate() {
            let mut g = GanttRow::default();
            if window.contains(&n) {
                g.label = label(n, r);
            }
            if let Row::Item(seat, e, _) = r
                && let Some(at) = e.at()
                && let Some(k) = read_of(*seat)
                && let Some(row) = reads[k].rows.get(at)
            {
                let site = sites[k].1;
                fill_row(&mut g, row, site.column(at), &self.zone);
                place.insert((k, row.id), n);
                kept.push(row);
            }
            out.push(g);
        }
        // **끄는 막대는 화면에만 옮긴다**(moai-i4hf.exq) — 쓰기는 놓을 때 한 번이다([`App::drop_chart`]). 날 수는 위젯의
        // 셈(`drag_days`)이고 옮긴 날은 쓰기의 셈(`Issue::shifted`, [`preview`])이라, 놓을 때 쓰는 날과 끄는 동안 그린
        // 날이 한 자다. 못 옮기는 줄(멤버에서 센 묶음)은 그대로 둔다 — 놓으면 까닭이 선다.
        if let Some(ChartDrag::Bar { id, part, from, now: to }) = &self.chart_drag
            && let Some(at) = self.site.index.find(id)
            && let Some(n) = rows.iter().position(|r| matches!(r, Row::Item(Seat::Here, e, _) if e.at() == Some(at)))
            && let Some(i) = self.site.issues.get(at)
            && let Ok(change) = i.shifted(edge_of(*part), moai_gantt::drag_days(self.chart.axis.zoom, *from, *to))
        {
            preview(&mut out[n], i, change);
        }
        let mut links = Vec::new();
        for (n, r) in rows.iter().enumerate() {
            if let Row::Item(seat, e, _) = r
                && let Some(at) = e.at()
                && let Some(k) = read_of(*seat)
                && let Some(row) = reads[k].rows.get(at)
            {
                for b in &row.blocked_by {
                    if let Some(&from) = place.get(&(k, b.id)) {
                        links.push(Link { from_row: from, to_row: n });
                    }
                }
            }
        }
        let range = crate::report::gantt::range_of(kept, &utc_now, &self.zone).and_then(|r| {
            let from = parse_date(&r.from)?;
            let to = parse_date(&r.to)?;
            Some((from * DAY, (to + 1) * DAY))
        });
        Chart { rows: out, links, range, now }
    }
}

/// 들고 있는 계획 옮기기를 쓴다(moai-i4hf.9xi). **[`super::Retry`] 로도 넘긴다** — 누군지 묻고 받으면 `App::answer` 가
/// 이것을 다시 부르고, 이것은 들고 있던 [`App::plan_move`] 를 다시 쓴다.
///
/// **고칠 날은 여기서, 락 안에서 다시 읽은 줄로 잰다**([`PlanMove`]) — 화면이 그새 낡았어도 옆 세션이 고친 날 위에서
/// 옮긴다. 그 줄이 그새 날을 잃었거나(옮기기) 얻었으면(하루 세우기) 아무것도 안 쓰고 까닭이 배너에 선다.
///
/// **고치는 차례는 `moai edit` 과 하나다** — `Issue::set_dates` 로 날을 적고 `Issue::settle` 이 정규화·검사·`updated_at` 을
/// 한다. 시작이 기한을 넘거나 위의 기한을 넘는 날(`store::write_locked`, moai-xf0p.cbu)은 거절되고 그 거절문이 배너에
/// 선다. **저널은 안 적는다** — 필드 고치기다(CLAUDE.md "저널은 상태 계산에 읽히지 않는다").
///
/// 되면 알림 끝에 **되돌리는 명령**을 붙인다(`moai edit <id> --start … --due …`) — 손이 미끄러진 끌기를 도구 안에서 되돌릴
/// 길이 보여야 한다. 위 기한을 당겨 아래 줄이 넘쳤으면 그 줄도 댄다 — `moai edit` 이 stderr 로 대는 그것이다.
pub(super) fn commit_plan(app: &mut App) {
    let Some(PlanMove { id, plan }) = app.plan_move.clone() else { return };
    let lang = app.site.lang;
    let at = crate::model::now();
    let done = say(lang, "tui.gantt.moved");
    let mut was: Option<(Option<String>, Option<String>)> = None;
    let mut overflow: Vec<String> = Vec::new();
    let wrote = app.write(commit_plan, |issues, cfg, _, _| {
        let Some(i) = issues.iter_mut().find(|i| i.id == id) else {
            return Err(crate::fail::Fail::not_found(&id, lang));
        };
        let before = i.clone();
        let (start, due) = match plan {
            Plan::Shift(edge, days) => {
                i.shifted(edge, days).map_err(|why| crate::fail::Fail::new(unshifted(lang, why, &id)))?
            }
            Plan::Day(_) if i.has_days() => {
                return Err(crate::fail::Fail::new(fill(say(lang, "tui.gantt.planned"), &[("id", &id)])));
            }
            Plan::Day(day) => {
                let d = Some(Some(crate::model::format_date(day)));
                (d.clone(), d)
            }
        };
        i.set_dates(start, due);
        i.settle(&before, cfg, &at).map_err(|why| {
            crate::fail::Fail::new(crate::view::invalid(lang, &crate::store::At::Id(id.clone()), &why))
        })?;
        let after = i.clone();
        overflow = crate::report::overflow_by(issues, &before, &after, cfg);
        was = Some((before.starts_on, before.due_on));
        Ok((Vec::new(), Touched { id: id.clone(), done }))
    });
    // 누군지 물으러 갔으면 들고 있는다 — 답이 오면 이것을 다시 쓴다. 됐든 거절됐든 끝난 쓰기는 비운다.
    if !matches!(app.mode, Mode::Ask(_)) {
        app.plan_move = None;
    }
    if wrote.is_none() {
        return;
    }
    let Some((start, due)) = was else { return };
    let none = || "none".to_string();
    let undo = format!("moai edit {id} --start {} --due {}", start.unwrap_or_else(none), due.unwrap_or_else(none));
    let mut told = app.notice.take().unwrap_or_default();
    told.push_str(&fill(say(lang, "tui.gantt.undo"), &[("cmd", &undo)]));
    if !overflow.is_empty() {
        told.push_str(&fill(say(lang, "tui.gantt.overflow"), &[("ids", &overflow.join(", "))]));
    }
    app.notice = Some(told);
}

/// 읽은 한 줄을 위젯의 줄에 옮긴다. 계획의 날은 달력 날짜라 옮기지 않고, 실제의 시각은 UTC 라 보는 사람의 벽시계로
/// 옮긴다(`report::gantt` 의 "시간대").
fn fill_row(g: &mut GanttRow, row: &crate::report::gantt::Row, column: &str, zone: &crate::tz::Zone) {
    g.kind = match row.kind {
        Kind::Milestone => RowKind::Milestone,
        _ if matches!(row.progress, crate::report::gantt::Standing::Rollup { .. }) => RowKind::Group,
        _ => RowKind::Item,
    };
    g.tone = tone(column);
    let day = |d: Option<&str>| d.and_then(parse_date);
    if let Some(p) = &row.planned {
        let (start, end) = (day(p.start), day(p.end));
        g.planned = plan_span(start, end);
        // 기한 표식은 **제 기한**만 — 멤버에서 센 끝은 그 줄의 기한이 아니다.
        if p.from == Origin::Own {
            g.due = end.map(|e| (e + 1) * DAY);
        }
    }
    let local = |t: &str| parse_rfc3339(t).map(|s| zone.local(s));
    g.actual = match &row.actual {
        Actual::Span { start, end } => local(start).map(|s| (s, end.and_then(local))),
        Actual::NotStarted | Actual::Unknown => None,
    };
    g.overdue = row.overdue.is_some();
    g.deferred = row.deferred;
}

/// 칸 → 실제 막대의 색 칸. 칸 이름으로 가르는 것은 글리프(`style::glyph`)와 같은 자다 — 모르는 칸은 진행 중으로 본다.
fn tone(column: &str) -> Tone {
    match column {
        "todo" => Tone::Todo,
        "review" => Tone::Review,
        crate::config::DONE => Tone::Done,
        _ => Tone::Progress,
    }
}

#[cfg(test)]
mod tests {
    use super::super::keys::Screen;
    use super::super::view::Layout;
    use super::super::{App, Row, Seat, draw};
    use crate::config::Config;
    use crate::model::{Issue, Kind, Status};
    use crate::nav::Path;

    fn cfg() -> Config {
        Config::parse("prefix = \"argos\"\n").unwrap()
    }

    fn make(id: &str, kind: Kind) -> Issue {
        Issue::new(id.into(), format!("{id} 제목"), kind, Status::new("todo"), "2026-09-01T00:00:00Z")
    }

    /// [`app`] 과 같은 줄을 **디스크의 트래커로** 세워 연다 — 쓰는 시험이 쓴다. 누군지는 늘 안다.
    fn on_disk(name: &str) -> (crate::scratch::Scratch, App) {
        let dir = crate::scratch::Scratch::new(&format!("tui-gantt-{name}"));
        std::fs::create_dir_all(dir.join(".moai/journal")).unwrap();
        std::fs::write(dir.join(".moai/config.toml"), "prefix = \"argos\"\n").unwrap();
        let rows: Vec<String> = app().site.issues.iter().map(|i| serde_json::to_string(i).unwrap() + "\n").collect();
        std::fs::write(dir.join(".moai/issues.jsonl"), rows.concat()).unwrap();
        let repo = crate::store::Repo::at(dir.to_path_buf(), cfg());
        let stamp = super::super::stamp_of(&repo);
        let load = repo.read().unwrap();
        let (index, ground) = super::super::measure(&load.issues, &repo.config);
        let mut a = App::open(repo, load, index, ground, Path::new(), stamp);
        a.identify = |_, _| Ok(crate::model::someone("raven"));
        a.site.now = "2026-10-22T03:00:00Z".into();
        (dir, a)
    }

    /// 디스크의 그 줄의 (시작, 기한).
    fn dates_on_disk(dir: &std::path::Path, id: &str) -> (Option<String>, Option<String>) {
        let text = std::fs::read_to_string(dir.join(".moai/issues.jsonl")).unwrap();
        let i: Issue = text.lines().map(|l| serde_json::from_str::<Issue>(l).unwrap()).find(|i| i.id == id).unwrap();
        (i.starts_on, i.due_on)
    }

    fn put_cursor(a: &mut App, id: &str) {
        a.cursor = ids(a).iter().position(|i| i == id).unwrap();
    }

    /// **`<`·`>` 는 막대를, `{`·`}` 는 시작만 하루 옮겨 쓴다**(moai-i4hf.9xi). 알림이 되돌리는 명령을 대고, 시작이 기한을
    /// 넘는 쓰기는 거절되어 디스크가 그대로다. 날 없는 줄과 멤버에서 센 묶음은 까닭을 대고 안 쓴다.
    #[test]
    fn nudging_writes_the_plan_and_names_the_undo() {
        let (dir, mut a) = on_disk("nudge");
        a.hit("SPC g g");
        put_cursor(&mut a, "argos-0003");
        a.hit(">");
        assert_eq!(
            dates_on_disk(&dir, "argos-0003"),
            (Some("2026-10-21".into()), Some("2026-10-25".into())),
            "> 가 막대를 하루 안 미뤘다 — {:?}",
            a.trouble
        );
        let told = a.notice.clone().unwrap_or_default();
        assert!(
            told.contains("moai edit argos-0003 --start 2026-10-20 --due 2026-10-24"),
            "되돌리는 명령이 없다: {told}"
        );
        assert_eq!(on(&a), "argos-0003", "쓰고 나서 커서가 그 줄을 놓쳤다");

        a.hit("{");
        assert_eq!(dates_on_disk(&dir, "argos-0003").0.as_deref(), Some("2026-10-20"), "{{ 가 시작만 안 당겼다");
        assert_eq!(dates_on_disk(&dir, "argos-0003").1.as_deref(), Some("2026-10-25"), "{{ 가 기한까지 옮겼다");

        // 시작이 기한을 넘으면 거절 — 디스크는 그대로고 거절문이 선다.
        for _ in 0..5 {
            a.hit("}");
        }
        assert_eq!(
            dates_on_disk(&dir, "argos-0003").0.as_deref(),
            Some("2026-10-25"),
            "시작이 기한에 닿을 때까지는 된다"
        );
        a.hit("}");
        assert_eq!(
            dates_on_disk(&dir, "argos-0003").0.as_deref(),
            Some("2026-10-25"),
            "시작이 기한을 넘는 쓰기가 들었다"
        );
        assert!(a.trouble.is_some(), "거절이 배너에 안 섰다");

        put_cursor(&mut a, "argos-0004");
        a.hit(">");
        assert_eq!(dates_on_disk(&dir, "argos-0004"), (None, None), "날 없는 줄에 날을 지어냈다");
        assert!(a.notice.as_deref().is_some_and(|n| n.contains("argos-0004")), "날 없는 줄이 까닭을 안 댔다");

        put_cursor(&mut a, "argos-0001");
        a.hit(">");
        assert_eq!(dates_on_disk(&dir, "argos-0001"), (None, None), "멤버에서 센 막대를 그 줄의 날로 적었다");
        assert!(a.notice.as_deref().is_some_and(|n| n.contains("argos-0001")), "묶음이 까닭을 안 댔다");
    }

    /// **간트 위의 마우스**(moai-i4hf.exq) — 고정 칸을 누르면 커서가 그 줄로, 막대를 끌면 놓을 때 계획이 옮아 쓰이고,
    /// 날 없는 줄의 빈 날을 누르면 그 하루가 계획이 된다. 가로 휠·Shift+휠·날짜 머리 끌기는 축을, 나눔 선 끌기는 고정
    /// 칸 폭을 바꾼다.
    #[test]
    fn the_mouse_picks_rows_drags_bars_and_slides_the_axis() {
        use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind as K};
        let (dir, mut a) = on_disk("mouse");
        a.hit("SPC g g");
        let _ = draw::tests::render(&mut a, 140, 20);
        let ev = |kind, column, row, modifiers| MouseEvent { kind, column, row, modifiers };
        let none = KeyModifiers::NONE;
        let area = a.drawn.chart.expect("간트가 제 자리를 안 남겼다");
        let laid = moai_gantt::Layout::of(area, &a.chart);
        let rows = a.rows();
        // 줄이 다 보이는 높이라 굴린 자리는 0 이고, 이 시험에서 줄의 차례는 안 바뀐다.
        let shown = ids(&a);
        let row_of = |id: &str| shown.iter().position(|i| i == id).unwrap();
        let top = a.chart.first_row;
        let y_of = |n: usize| laid.dates_body.y + (n - top) as u16;

        // 고정 칸의 줄을 누른다.
        let n9 = row_of("argos-0009");
        a.mouse(ev(K::Down(MouseButton::Left), laid.labels_body.x + 3, y_of(n9), none));
        a.mouse(ev(K::Up(MouseButton::Left), laid.labels_body.x + 3, y_of(n9), none));
        assert_eq!(on(&a), "argos-0009", "고정 칸을 눌러도 커서가 안 갔다");

        // 막대 몸통을 하루만큼 오른쪽으로 끌고 놓는다.
        let n3 = row_of("argos-0003");
        let chart = a.chart_of(&rows, 0..0, &|_, _| ratatui::text::Line::default());
        let (c0, c1) = moai_gantt::bar_cells(&a.chart, chart.rows[n3].planned.unwrap());
        let x = laid.dates_body.x + ((c0 + c1) / 2) as u16;
        let step = a.chart.axis.zoom.cells_per_day().round().max(1.0) as u16;
        let days = moai_gantt::drag_days(a.chart.axis.zoom, 0, i64::from(step));
        assert_eq!(days, 1, "시험의 전제 — 끈 칸 수가 하루다");
        a.mouse(ev(K::Down(MouseButton::Left), x, y_of(n3), none));
        assert_eq!(on(&a), "argos-0003", "막대를 눌러도 커서가 안 갔다");
        a.mouse(ev(K::Drag(MouseButton::Left), x + step, y_of(n3), none));
        assert_eq!(
            dates_on_disk(&dir, "argos-0003"),
            (Some("2026-10-20".into()), Some("2026-10-24".into())),
            "끄는 동안 썼다"
        );
        a.mouse(ev(K::Up(MouseButton::Left), x + step, y_of(n3), none));
        assert_eq!(
            dates_on_disk(&dir, "argos-0003"),
            (Some("2026-10-21".into()), Some("2026-10-25".into())),
            "놓았는데 막대가 안 옮았다 — {:?}",
            a.trouble
        );

        // 가로 휠과 Shift+휠은 축을 한 칸 민다. 세로 휠은 축을 안 건드린다.
        let _ = draw::tests::render(&mut a, 140, 20);
        let spc = a.chart.axis.zoom.secs_per_cell();
        let o = a.chart.axis.origin;
        a.mouse(ev(K::ScrollRight, x, y_of(n3), none));
        assert_eq!(a.chart.axis.origin, o + spc, "가로 휠이 축을 안 밀었다");
        a.mouse(ev(K::ScrollUp, x, y_of(n3), KeyModifiers::SHIFT));
        assert_eq!(a.chart.axis.origin, o, "Shift+휠이 축을 안 밀었다");
        a.mouse(ev(K::ScrollDown, x, y_of(n3), none));
        assert_eq!(a.chart.axis.origin, o, "세로 휠이 축을 밀었다");

        // 날짜 머리를 왼쪽으로 세 칸 끌면 축이 세 칸 미래로 간다.
        let hx = laid.dates_header.x + 10;
        a.mouse(ev(K::Down(MouseButton::Left), hx, area.y, none));
        a.mouse(ev(K::Drag(MouseButton::Left), hx - 3, area.y, none));
        a.mouse(ev(K::Up(MouseButton::Left), hx - 3, area.y, none));
        assert_eq!(a.chart.axis.origin, o + 3 * spc, "날짜 머리 끌기가 축을 손만큼 안 밀었다");

        // 나눔 선을 네 칸 끌면 고정 칸이 네 칸 넓어지고, 놓을 때 사용자 설정에 칸 수로 적힌다.
        let user = dir.join("user.toml");
        a.user_config = Some(user.clone());
        let w = laid.labels_body.width;
        a.mouse(ev(K::Down(MouseButton::Left), laid.splitter.x, area.y + 5, none));
        a.mouse(ev(K::Drag(MouseButton::Left), laid.splitter.x + 4, area.y + 5, none));
        assert!(!std::fs::read_to_string(&user).unwrap_or_default().contains("gantt_label"), "끄는 동안 적었다");
        a.mouse(ev(K::Up(MouseButton::Left), laid.splitter.x + 4, area.y + 5, none));
        assert_eq!(a.chart.label_width, w + 4, "나눔 선 끌기가 고정 칸 폭을 안 바꿨다");
        let text = std::fs::read_to_string(&user).unwrap_or_default();
        assert!(text.contains(&format!("gantt_label = {}", w + 4)), "놓은 폭이 설정에 안 적혔다: {text}");
        let (_d, mut b) = on_disk("mouse-reread");
        b.user_config = Some(user);
        b.load_look();
        assert_eq!(b.chart.label_width, w + 4, "다음 실행이 고정 칸 폭을 못 읽었다");

        // 날 없는 줄의 빈 날을 누르면 그 하루가 계획이 된다.
        let _ = draw::tests::render(&mut a, 140, 20);
        let laid = moai_gantt::Layout::of(area, &a.chart);
        let col = 5;
        let day = a.chart.axis.day_at_cell(f64::from(col));
        a.mouse(ev(K::Down(MouseButton::Left), laid.dates_body.x + col, laid.dates_body.y + (n9 - top) as u16, none));
        let d = crate::model::format_date(day);
        assert_eq!(dates_on_disk(&dir, "argos-0009"), (Some(d.clone()), Some(d)), "빈 날 누르기가 계획을 안 세웠다");
    }

    /// **폭을 훑어도 간트가 안 깨진다**(moai-i4hf.25n) — 20칸부터 200칸까지 줄의 제목은 늘 서고, 날짜 칸이 너무 좁아지면
    /// 고정 칸이 먼저 줄고 그래도 안 되면 날짜 칸을 감추며 그렇다고 댄다.
    #[test]
    fn every_width_keeps_the_rows_and_says_when_the_dates_go() {
        let mut a = app();
        a.hit("SPC g g SPC v d Esc");
        assert!(!a.detail_open, "시험의 전제 — 상세를 닫았다");
        for w in 20..=200 {
            let screen = draw::tests::render(&mut a, w, 20).join("\n");
            assert!(screen.matches("arg").count() >= 5, "{w}칸에서 줄이 사라졌다\n{screen}");
            let area = a.drawn.chart.expect("간트가 제 자리를 안 남겼다");
            let hidden = a.dates_hidden(area);
            assert_eq!(screen.contains("창이 좁아"), hidden, "{w}칸: 감춘 날짜 칸과 그 말이 안 맞는다\n{screen}");
            if !hidden {
                let laid = moai_gantt::Layout::of(area, &a.chart_view(area));
                assert!(laid.dates_width() >= super::MIN_DATES, "{w}칸: 날짜 칸이 바닥보다 좁게 섰다");
            }
        }
        let _ = draw::tests::render(&mut a, 200, 20);
        assert!(!a.dates_hidden(a.drawn.chart.unwrap()), "넓은 창에서 날짜 칸이 숨었다");
        assert_eq!(a.chart.label_width, moai_gantt::DEFAULT_LABEL_WIDTH, "창에 맞춘 폭이 사람의 폭을 덮었다");
    }

    /// 날 든 줄이 하나도 없으면 날짜 칸이 날을 넣는 길을 댄다. 고정 칸은 그대로 선다.
    #[test]
    fn a_tracker_without_dates_says_how_to_add_them() {
        let issues = vec![make("argos-0001", Kind::Epic), make("argos-0009", Kind::Issue)];
        let mut a = App::new(issues, cfg(), Path::new());
        a.hit("SPC g g");
        let screen = draw::tests::render(&mut a, 160, 20).join("\n");
        assert!(screen.contains("argos-0009"), "고정 칸이 안 섰다\n{screen}");
        assert!(screen.contains("아직 계획 날짜가 없다"), "날을 넣는 길을 안 댔다\n{screen}");
        let screen = draw::tests::render(&mut app(), 160, 20).join("\n");
        assert!(!screen.contains("아직 계획 날짜가 없다"), "날이 있는데 없다고 했다");
    }

    /// **머리의 달 이름은 화면 말이다**(moai-9p7z) — 한국어 화면에 "Oct 2026" 이 서지 않고, 영어 화면은 옛 그대로다.
    #[test]
    fn the_header_months_follow_the_screen_language() {
        let mut a = app();
        a.site.lang = crate::i18n::Lang::Ko;
        a.hit("SPC g g");
        let ko = draw::tests::render(&mut a, 160, 20).join("\n");
        assert!(ko.contains("10월"), "한국어 화면의 머리에 10월 이 없다\n{ko}");
        assert!(!ko.contains("Oct"), "한국어 화면에 영어 달 이름이 남았다\n{ko}");
        a.site.lang = crate::i18n::Lang::En;
        let en = draw::tests::render(&mut a, 160, 20).join("\n");
        assert!(en.contains("Oct"), "영어 화면의 머리에 Oct 가 없다\n{en}");
    }

    /// 낮은 창에서는 머리의 오늘(`▼`) 줄을 접고 그 줄을 본문에 준다. 높은 창에서는 선다.
    #[test]
    fn a_low_window_folds_the_today_row() {
        let mut a = app();
        a.hit("SPC g g SPC v d Esc");
        let tall = draw::tests::render(&mut a, 140, 24).join("\n");
        assert!(tall.contains('▼'), "높은 창에 오늘 표식이 없다\n{tall}");
        let low = draw::tests::render(&mut a, 140, 9).join("\n");
        assert!(!low.contains('▼'), "낮은 창이 오늘 줄을 안 접었다\n{low}");
        assert!(low.contains("argos-0001"), "낮은 창에 줄이 안 섰다\n{low}");
    }

    /// 디스크의 그 줄을 고친다 — 탐색기가 다시 읽기 전에 옆 세션이 쓴 것이다.
    fn edit_on_disk(dir: &std::path::Path, id: &str, f: impl Fn(&mut Issue)) {
        let path = dir.join(".moai/issues.jsonl");
        let text = std::fs::read_to_string(&path).unwrap();
        let rows: Vec<String> = text
            .lines()
            .map(|l| {
                let mut i: Issue = serde_json::from_str(l).unwrap();
                if i.id == id {
                    f(&mut i);
                }
                serde_json::to_string(&i).unwrap() + "\n"
            })
            .collect();
        std::fs::write(&path, rows.concat()).unwrap();
    }

    /// **옮기기는 쓰는 순간의 줄에서 센다**(리뷰) — 화면이 낡은 사이 옆 세션이 기한을 고쳤으면, `>` 는 그 기한에서 하루를
    /// 옮긴다. 화면의 옛 날에서 센 값을 쓰면 옆 세션의 고침이 말없이 사라진다. 그새 날이 선 줄의 하루 세우기는 안 쓴다.
    #[test]
    fn a_move_counts_from_the_row_as_it_stands_when_written() {
        let (dir, mut a) = on_disk("stale");
        a.hit("SPC g g");
        put_cursor(&mut a, "argos-0003");
        edit_on_disk(&dir, "argos-0003", |i| i.due_on = Some("2026-10-30".into()));
        a.hit(">");
        assert_eq!(
            dates_on_disk(&dir, "argos-0003"),
            (Some("2026-10-21".into()), Some("2026-10-31".into())),
            "옆 세션이 고친 기한을 화면의 옛 기한으로 덮었다 — {:?}",
            a.trouble
        );
        let told = a.notice.clone().unwrap_or_default();
        assert!(told.contains("--due 2026-10-30"), "되돌리는 명령이 쓰기 앞의 기한을 안 댔다: {told}");

        // 화면에는 날이 없는데 그새 날이 섰다 — 하루 세우기가 그 날을 덮지 않는다.
        let _ = draw::tests::render(&mut a, 140, 20);
        edit_on_disk(&dir, "argos-0009", |i| i.due_on = Some("2026-11-05".into()));
        let at = a.site.index.find("argos-0009").unwrap();
        a.plan_day(at, crate::model::parse_date("2026-10-22").unwrap());
        assert_eq!(dates_on_disk(&dir, "argos-0009"), (None, Some("2026-11-05".into())), "그새 선 기한을 덮었다");
        assert!(a.trouble.as_deref().is_some_and(|t| t.contains("argos-0009")), "안 쓴 까닭이 배너에 없다");
    }

    /// **끄는 동안 그린 막대가 놓으면 쓸 날이다**(리뷰) — 끝을 시작보다 앞으로 끌면 쓰기가 거절하니 막대는 그대로 서고,
    /// 시작 날까지만 끌면 하루짜리 막대가 서고 그 날이 쓰인다.
    #[test]
    fn the_dragged_bar_draws_the_days_the_drop_writes() {
        use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind as K};
        let (dir, mut a) = on_disk("preview");
        a.hit("SPC g g");
        let _ = draw::tests::render(&mut a, 140, 20);
        let ev = |kind, column, row| MouseEvent { kind, column, row, modifiers: KeyModifiers::NONE };
        let area = a.drawn.chart.expect("간트가 제 자리를 안 남겼다");
        let laid = moai_gantt::Layout::of(area, &a.chart_view(area));
        let n3 = ids(&a).iter().position(|i| i == "argos-0003").unwrap();
        let y = laid.dates_body.y + (n3 - a.chart.first_row) as u16;
        let bar = |a: &App| a.chart_of(&a.rows(), 0..0, &|_, _| ratatui::text::Line::default()).rows[n3].planned;
        let was = bar(&a).unwrap();
        let (_, c1) = moai_gantt::bar_cells(&a.chart, was);
        let step = a.chart.axis.zoom.cells_per_day().round().max(1.0) as u16;
        assert!(step >= 2, "시험의 전제 — 끝 칸을 따로 잡을 만큼 확대했다");
        let end = laid.dates_body.x + c1 as u16 - 1;
        a.mouse(ev(K::Down(MouseButton::Left), end, y));
        assert!(
            matches!(&a.chart_drag, Some(super::ChartDrag::Bar { part: moai_gantt::Part::End, .. })),
            "끝을 못 잡았다"
        );

        // 닷새 앞으로 — 기한이 시작보다 하루 앞선다. 쓰기가 거절하니 그리는 막대도 안 옮긴다.
        a.mouse(ev(K::Drag(MouseButton::Left), end - 5 * step, y));
        assert_eq!(bar(&a), Some(was), "거절될 끌기를 옮긴 막대로 그렸다");
        // 나흘 앞으로 — 시작 날 하루짜리다. 그린 날과 쓴 날이 같다.
        a.mouse(ev(K::Drag(MouseButton::Left), end - 4 * step, y));
        let start = crate::model::parse_date("2026-10-20").unwrap();
        assert_eq!(bar(&a), Some((start * super::DAY, (start + 1) * super::DAY)), "하루짜리 막대로 안 그렸다");
        a.mouse(ev(K::Up(MouseButton::Left), end - 4 * step, y));
        assert_eq!(dates_on_disk(&dir, "argos-0003"), (Some("2026-10-20".into()), Some("2026-10-20".into())));
    }

    /// **Shift+휠이 가로인 것은 간트 위에서만이다**(리뷰) — 목록에서는 이 에픽 전처럼 세로로 구른다.
    #[test]
    fn shift_and_the_wheel_still_roll_the_list() {
        use ratatui::crossterm::event::{KeyModifiers, MouseEvent, MouseEventKind as K};
        let issues = (1..=40).map(|k| make(&format!("argos-{k:04}"), Kind::Issue)).collect();
        let mut a = App::new(issues, cfg(), Path::new());
        let _ = draw::tests::render(&mut a, 100, 20);
        let (x, y) = (a.drawn.list.x + 3, a.drawn.list.y + 3);
        a.mouse(MouseEvent { kind: K::ScrollDown, column: x, row: y, modifiers: KeyModifiers::SHIFT });
        assert!(a.list.offset() > 0, "목록 위의 Shift+휠이 안 굴렀다");
    }

    /// 목록과 상세에서는 계획 옮기기가 조용하다 — 보이지 않는 막대를 옮기지 않는다.
    #[test]
    fn nudging_stands_only_on_the_gantt_list() {
        let (dir, mut a) = on_disk("nudge-off");
        put_cursor(&mut a, "argos-0009");
        let before = dates_on_disk(&dir, "argos-0009");
        a.hit("SPC g g");
        put_cursor(&mut a, "argos-0003");
        a.hit("Ctrl-w j >");
        assert_eq!(dates_on_disk(&dir, "argos-0003"), (Some("2026-10-20".into()), Some("2026-10-24".into())));
        a.hit("Ctrl-w k SPC g l");
        put_cursor(&mut a, "argos-0003");
        a.hit(">");
        assert_eq!(dates_on_disk(&dir, "argos-0003"), (Some("2026-10-20".into()), Some("2026-10-24".into())));
        assert_eq!(dates_on_disk(&dir, "argos-0009"), before);
    }

    /// 에픽 둘(하나는 멤버 둘), 에픽 밖 일 하나. 멤버 하나에 계획이 있고, 시계를 못박는다.
    fn app() -> App {
        let mut planned = make("argos-0003", Kind::Issue);
        planned.epic = Some("argos-0001".into());
        planned.starts_on = Some("2026-10-20".into());
        planned.due_on = Some("2026-10-24".into());
        let mut other = make("argos-0004", Kind::Issue);
        other.epic = Some("argos-0001".into());
        let issues = vec![
            make("argos-0001", Kind::Epic),
            make("argos-0002", Kind::Epic),
            planned,
            other,
            make("argos-0009", Kind::Issue),
        ];
        let mut a = App::new(issues, cfg(), Path::new());
        a.site.now = "2026-10-22T03:00:00Z".into();
        a
    }

    fn ids(a: &App) -> Vec<String> {
        a.rows()
            .iter()
            .filter_map(|r| if let Row::Item(Seat::Here, e, _) = r { e.at() } else { None })
            .map(|at| a.site.issues[at].id.clone())
            .collect()
    }

    fn on(a: &App) -> String {
        ids(a).get(a.cursor).cloned().unwrap_or_default()
    }

    /// **`SPC g g` 는 모두 펼친 채로 서고, 목록의 접기는 건드리지 않는다**(moai-i4hf.sas). 간트 안의 `h`·`l`·`Tab` 은
    /// 간트의 펼침만 바꾼다 — 목록으로 돌아오면 접어 둔 그대로다.
    #[test]
    fn spc_g_g_opens_everything_and_leaves_the_list_folds_alone() {
        let mut a = app();
        assert_eq!(ids(&a), ["argos-0001", "argos-0002", "argos-0009"], "시험의 전제 — 목록은 접힌 채다");
        a.hit("SPC g g");
        assert_eq!(a.layout, Layout::Gantt);
        assert_eq!(
            ids(&a),
            ["argos-0001", "argos-0003", "argos-0004", "argos-0002", "argos-0009"],
            "간트가 묶음을 다 펼치지 않았다"
        );
        // 간트 안의 접기 — 에픽 줄의 `h`.
        a.cursor = 0;
        a.hit("h");
        assert_eq!(ids(&a), ["argos-0001", "argos-0002", "argos-0009"], "간트의 `h` 가 안 접었다");
        a.hit("l");
        assert_eq!(ids(&a).len(), 5, "간트의 `l` 이 안 펼쳤다");
        a.hit("Tab");
        assert_eq!(ids(&a).len(), 3, "간트의 `Tab` 이 다 접지 않았다");
        a.hit("SPC g l");
        assert_eq!(ids(&a), ["argos-0001", "argos-0002", "argos-0009"], "간트의 펼침이 목록에 샜다");
        // 다시 들어가면 다시 모두 펼친다.
        a.hit("SPC g g");
        assert_eq!(ids(&a).len(), 5, "다시 들어간 간트가 접은 자리를 기억했다");
    }

    /// **오가도 커서가 그 줄에 남는다** — 목록이 접어 둔 폴더 안의 줄에서 목록으로 오면 그 폴더만 연다.
    #[test]
    fn the_cursor_stays_on_its_row_across_list_and_gantt() {
        let mut a = app();
        a.cursor = 2;
        a.hit("SPC g g");
        assert_eq!(on(&a), "argos-0009", "목록에서 고른 줄을 간트가 놓쳤다");
        a.cursor = ids(&a).iter().position(|i| i == "argos-0004").unwrap();
        a.hit("SPC g l");
        assert_eq!(a.layout, Layout::List);
        assert_eq!(on(&a), "argos-0004", "간트에서 고른 줄을 목록이 놓쳤다");
        assert_eq!(
            ids(&a),
            ["argos-0001", "argos-0003", "argos-0004", "argos-0002", "argos-0009"],
            "그 줄을 품은 폴더만 열려야 한다"
        );
    }

    /// **고정 칸은 목록의 줄이고, 날짜 칸에 머리와 막대가 선다.** 계획 든 줄에 막대가 서고, 날 없는 줄에는 안 선다.
    #[test]
    fn the_gantt_draws_the_fixed_column_and_bars() {
        let mut a = app();
        a.hit("SPC g g");
        let screen = draw::tests::render(&mut a, 140, 20).join("\n");
        assert!(screen.contains("argos-0003"), "고정 칸에 줄이 안 섰다\n{screen}");
        assert!(screen.contains("2026년 10월"), "날짜 머리가 안 섰다\n{screen}");
        // 간트 칸만 본다 — 오른쪽 상세의 진척 막대도 `░` 다.
        let pane = |id: &str| {
            let left = |l: &str| l.split("┃│").next().unwrap().to_string();
            screen.lines().map(left).find(|l| l.contains(id)).unwrap()
        };
        assert!(pane("argos-0003").contains('░'), "계획 막대가 안 섰다\n{screen}");
        assert!(pane("argos-0001").contains('▒'), "멤버에서 센 묶음 막대가 안 섰다\n{screen}");
        assert!(!pane("argos-0009").contains('░'), "날 없는 줄에 막대가 섰다\n{screen}");
    }

    /// **`keys.toml` 의 `[browse] expand = "L"` 은 목록에서 살고, 간트에서는 `L` 이 간트의 키다**(moai-l80o, 사람 결정
    /// 2026-10-25) — 간트의 키를 기본으로 묶은 업그레이드가 사람의 목록 키를 버리지 않는다.
    #[test]
    fn a_browse_key_the_gantt_also_uses_lives_in_the_list() {
        let (map, problems) = super::super::keymap::build("[browse]\nexpand = \"L\"\n");
        assert_eq!(problems, Vec::new(), "시험의 전제 — 그 줄이 버려졌다");
        let mut a = app();
        a.keys = map;
        a.cursor = 0;
        a.hit("L");
        assert_eq!(
            ids(&a),
            ["argos-0001", "argos-0003", "argos-0004", "argos-0002", "argos-0009"],
            "목록의 L 이 안 펼쳤다"
        );
        a.hit("SPC g g");
        let _ = draw::tests::render(&mut a, 140, 20);
        let origin = a.chart.axis.origin;
        a.hit("L");
        assert_eq!(a.chart.axis.origin - origin, a.chart.axis.zoom.secs_per_cell(), "간트의 L 이 축을 안 밀었다");
        assert_eq!(ids(&a).len(), 5, "간트의 L 이 목록의 펼침을 했다");
    }

    /// **날짜 축 키는 간트에서만 듣는다**(moai-i4hf.27r) — 확대·좌우·쪽·오늘·맞춤이 위젯의 보기를 옮기고, 움직이면
    /// 맞춤이 내려간다. 목록에서는 같은 글자가 아무 일도 안 한다.
    #[test]
    fn the_axis_keys_move_the_chart_only_in_the_gantt() {
        use moai_gantt::Zoom;
        let mut a = app();
        let before = a.chart;
        a.hit("H L - = t z");
        assert_eq!(a.chart, before, "목록에서 간트 키가 들었다");

        a.hit("SPC g g");
        let _ = draw::tests::render(&mut a, 140, 20);
        assert!(a.chart.fitting, "간트가 맞춤으로 안 섰다");
        let fitted = a.chart.axis;
        a.hit("L");
        assert!(!a.chart.fitting, "움직였는데 맞춤이 남았다");
        assert_eq!(a.chart.axis.origin - fitted.origin, fitted.zoom.secs_per_cell(), "L 이 한 칸 안 갔다");
        a.hit("H H");
        assert_eq!(a.chart.axis.origin - fitted.origin, -fitted.zoom.secs_per_cell(), "H 가 한 칸 안 돌아왔다");
        a.hit("]");
        assert!(a.chart.axis.origin > fitted.origin + fitted.zoom.secs_per_cell(), "] 가 한 쪽 안 넘겼다");
        a.hit("[");
        assert_eq!(a.chart.axis.origin - fitted.origin, -fitted.zoom.secs_per_cell(), "[ 가 그 쪽을 안 되돌렸다");

        let zoom = a.chart.axis.zoom;
        a.hit("=");
        assert_eq!(a.chart.axis.zoom, zoom.finer(), "= 가 확대하지 않았다");
        a.hit("+");
        assert_eq!(a.chart.axis.zoom, zoom.finer().finer(), "+ 가 = 와 다르다");
        a.hit("- - - - -");
        assert_eq!(a.chart.axis.zoom, Zoom::Month, "- 가 가장 거친 단계에서 안 멈췄다");

        a.hit("z");
        assert!(a.chart.fitting, "z 가 맞춤을 안 켰다");
        assert_eq!(a.chart.axis, fitted, "z 가 처음 맞춘 자리로 안 돌아왔다");

        a.hit("t");
        assert!(!a.chart.fitting);
        let screen = draw::tests::render(&mut a, 140, 20);
        let head = screen.iter().find(|l| l.contains('▼')).expect("오늘 표식이 안 섰다");
        let chars: Vec<char> = head.chars().collect();
        let col = |c: char| chars.iter().position(|&x| x == c).unwrap();
        let today = col('▼');
        let right = chars.iter().rposition(|&x| x == '┃').unwrap();
        let mid = (col('│') + right) / 2;
        assert!(today.abs_diff(mid) <= 6, "t 가 오늘을 가운데로 안 옮겼다\n{}", screen.join("\n"));
    }

    /// **간트의 상세는 처음에 아래에 선다**(moai-i4hf.sas) — 간트는 가로로 넓어야 한다. 간트 안의 `SPC o d` 는 간트의 자리만
    /// 돌리고, 목록으로 돌아오면 목록의 자리 그대로다. 칸 옮기기도 그 자리를 따른다.
    #[test]
    fn the_detail_stands_below_the_gantt_on_its_own_side() {
        use super::super::view::DetailAt;
        let mut a = app();
        let listed = a.detail_at;
        a.hit("SPC g g");
        let _ = draw::tests::render(&mut a, 140, 30);
        let (list, detail) = (a.drawn.list, a.drawn.detail.expect("상세가 안 섰다"));
        assert!(detail.y > list.y && detail.x == list.x, "간트의 상세가 아래에 안 섰다: {list:?} {detail:?}");
        a.hit("Ctrl-w j");
        assert_eq!(a.focus, super::super::Pane::Detail, "아래의 상세로 칸이 안 옮았다");
        a.hit("Ctrl-w k SPC o d Esc");
        assert_ne!(a.detail_side(), DetailAt::Bottom, "간트 안의 SPC o d 가 간트의 자리를 안 돌렸다");
        assert_eq!(a.detail_at, listed, "간트 안의 SPC o d 가 목록의 자리를 돌렸다");
        a.hit("SPC g l");
        assert_eq!(a.detail_side(), listed, "목록으로 오며 목록의 자리를 잃었다");
    }

    /// 배치 이름은 설정에 `gantt` 로 남고, 키 문맥은 간트 화면을 댄다.
    #[test]
    fn the_layout_goes_by_its_screen() {
        assert_eq!(Layout::named("gantt"), Some(Layout::Gantt));
        let mut a = app();
        a.hit("SPC g g");
        let rows = a.rows();
        assert_eq!(a.key_ctx(&rows).screen(), Screen::Gantt);
    }
}
