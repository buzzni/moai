//! 마우스 — 누르기·휠·칸 끌기(moai-irrj).
//!
//! **키와 같은 길로 옮긴다.** 누른 칸으로 포커스가 가는 것은 `Tab` 과, 휠이 목록을 움직이는 것은 `j`·`k`
//! 와 같은 자리를 바꾼다 — 마우스만의 상태를 두면 키로 한 일과 마우스로 한 일이 따로 논다.
//!
//! **맞히는 바탕은 지난 프레임이 그린 자리다**([`Drawn`]). 루프는 사건 하나마다 한 번 그리므로 그 자리는
//! 한 걸음 넘게 낡지 않는다 — 키가 상세의 끝을 마지막으로 그린 줄 수로 재는 것(`Scroll::go`)과 같은 결이다.
//!
//! **듣는 자리는 둘뿐이다**(사용자 결정 2026-10-01). 목록·상세를 둘러보는 동안(한눈 보기 `0` 도 같은 칸이다)과
//! 통계 창 위의 휠이다. 폼·묻는 칸·고르는 창·지우기 확인·글을 받는 칸·SPC 메뉴가 떠 있으면 아무것도 안 한다 —
//! 그 창들은 키로 다루는 자리고, 뒤의 목록을 누른 것이 적던 글을 두고 커서를 옮기면 무엇에 대해 적던 것인지를 잃는다.

use super::{App, Mode, menu};
use ratatui::crossterm::event::MouseEvent;
use ratatui::layout::Rect;

/// 지난 프레임이 그린 자리. **쓰는 곳은 `draw::screen` 하나다** — 여기는 읽기만 한다.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Drawn {
    /// 목록과 상세가 나눠 쓰는 몸통. 통계 창·폼은 이 자리를 통째로 덮는다.
    pub body: Rect,
    /// 목록 칸 — 테두리까지.
    pub list: Rect,
    /// 목록의 줄이 서는 자리 — 테두리와 열 이름 줄을 뺀 안쪽. 이 자리의 n 번째 줄이 굴린 자리(`App::list`)에서
    /// n 만큼 내려간 줄이다.
    pub rows: Rect,
    /// 상세 칸 — 테두리까지. 숨겼거나 접혀 안 섰으면 없다(`draw::split_body`).
    pub detail: Option<Rect>,
}

impl App {
    /// 마우스 사건 하나. **루프가 받은 그대로 넘긴다**(`cmd::tui::take`).
    ///
    /// 놓은 뒤에 온 사건은 버린다 — 놓는 글이 터미널에 닿기 전에 이미 길에 있던 것이다.
    pub fn mouse(&mut self, m: MouseEvent) {
        if !self.mouse {
            return;
        }
        match &self.mode {
            // 메뉴가 열린 동안은 메뉴가 키를 기다린다 — 뒤의 칸을 누른 것으로 메뉴를 닫지 않는다.
            Mode::Browse if !menu::open(&self.chord) => {}
            _ => return,
        }
        let _ = m;
    }
}
