//! 간트 배치 — `SPC g g`(moai-i4hf). 목록을 **모두 펼친 채** 왼쪽에 고정하고, 오른쪽에 날짜 축과 막대를 둔다.
//!
//! **새 창이 아니라 목록의 배치다**([`super::view::Layout::Gantt`]). 줄은 목록과 같은 몸(`App::rows_in`)이 세우고,
//! 커서·거름망·보기·검색·상세가 한 벌이라 `SPC g l` ↔ `SPC g g` 를 오가도 자리를 잃지 않는다. 다른 것은 펼침
//! 하나다 — 간트는 접은 자리(`Site::gantt_shut`)를 따로 들어, 들어갈 때 모두 펼치고 목록의 접기는 그대로 둔다.
//!
//! 조각이 아니다 — `App`·`Site` 를 고치고 저장소의 줄(`crate::model`)을 읽는다(`input::NOT_COMPONENTS`).

use super::{App, Path, Site};

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
}
