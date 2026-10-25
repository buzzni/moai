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

use super::{App, Path, Row, Seat, Site};
use crate::model::{Kind, parse_date, parse_rfc3339};
use crate::report::gantt::{Actual, Origin};
use moai_gantt::{GanttRow, Link, RowKind, Tone};
use ratatui::text::Line;

/// 하루의 초 — 위젯의 축이 재는 단위다.
const DAY: i64 = 86_400;

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
        let now = parse_rfc3339(&utc_now).map_or(0, |t| self.zone.local(t));
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
                fill(&mut g, row, site.column(at), &self.zone);
                place.insert((k, row.id), n);
                kept.push(row);
            }
            out.push(g);
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

/// 읽은 한 줄을 위젯의 줄에 옮긴다. 계획의 날은 달력 날짜라 옮기지 않고, 실제의 시각은 UTC 라 보는 사람의 벽시계로
/// 옮긴다(`report::gantt` 의 "시간대").
fn fill(g: &mut GanttRow, row: &crate::report::gantt::Row, column: &str, zone: &crate::tz::Zone) {
    g.kind = match row.kind {
        Kind::Milestone => RowKind::Milestone,
        _ if matches!(row.progress, crate::report::gantt::Standing::Rollup { .. }) => RowKind::Group,
        _ => RowKind::Item,
    };
    g.tone = tone(column);
    let day = |d: Option<&str>| d.and_then(parse_date);
    if let Some(p) = &row.planned {
        let (start, end) = (day(p.start), day(p.end));
        g.planned = match (start, end) {
            (Some(s), Some(e)) => Some((s * DAY, (e + 1) * DAY)),
            // 시작만 적은 줄은 그 하루를 계획으로 세운다 — 끝을 모르는 막대를 길게 그리면 없는 기한을 지어낸다.
            (Some(s), None) => Some((s * DAY, (s + 1) * DAY)),
            _ => None,
        };
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
        assert!(screen.contains("Oct"), "날짜 머리가 안 섰다\n{screen}");
        // 간트 칸만 본다 — 오른쪽 상세의 진척 막대도 `░` 다.
        let pane = |id: &str| {
            let left = |l: &str| l.split("┃│").next().unwrap().to_string();
            screen.lines().map(left).find(|l| l.contains(id)).unwrap()
        };
        assert!(pane("argos-0003").contains('░'), "계획 막대가 안 섰다\n{screen}");
        assert!(pane("argos-0001").contains('▒'), "멤버에서 센 묶음 막대가 안 섰다\n{screen}");
        assert!(!pane("argos-0009").contains('░'), "날 없는 줄에 막대가 섰다\n{screen}");
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
