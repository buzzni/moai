//! 차례 편집 창 — `SPC s`(moai-r170.x22).
//!
//! **조각이다.** 차례([`Sorting`])와 키([`Sorter`])만 안다. 고친 차례를 화면에 입히고 설정에 적는 것은 든 쪽
//! (`App::edit_sort`)이 한다 — 시간대 창(`zones`)이 고른 이름만 내고 여는 일은 `App` 이 하는 것과 같은 꼴이다.
//!
//! **그 층에서 쓸 수 있는 필드가 다 선다** — 차례에 든 것이 먼저 그 차례대로, 안 든 것이 뒤에 [`Order::ALL`] 의 차례로. 안 든
//! 필드를 숨기면 넣을 길이 따로 있어야 하고, 한 목록에서 넣고 빼면 그 길이 `SPC` 하나다.

use super::keys::{Order, Ordered, Sorter, Sorting};

/// 창의 상태 — 고치는 중의 차례와 커서.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Editor {
    /// 고치는 중의 차례. **늘 한 필드 이상이다** — 마지막 필드는 못 뺀다([`Editor::act`]). 빈 차례는 차례가 아니고
    /// (`query::parse_order` 가 거절한다), Enter 가 그것을 받으면 무엇으로 설지 정할 길이 없다.
    fields: Vec<Ordered>,
    /// 커서 — [`Editor::rows`] 의 자리.
    pub cursor: usize,
    allow_progress: bool,
}

/// 창의 한 줄 — 필드와, 차례에 들었으면 그 자리(0부터)와 방향.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Row {
    pub by: Order,
    pub at: Option<(usize, Ordered)>,
}

/// 키 하나의 끝.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Done {
    /// 창에 남는다.
    Stay,
    /// 지금 선 자리에 입힌다.
    Apply(Sorting),
    /// 입히고 기본 차례로도 적는다.
    Default(Sorting),
    /// 아무것도 안 바꾸고 닫는다.
    Cancel,
}

impl Editor {
    /// 지금 차례로 연다. 커서는 첫 필드에 선다.
    pub fn open(s: Sorting) -> Editor {
        Editor { fields: s.fields().to_vec(), cursor: 0, allow_progress: true }
    }

    /// 그릴 줄 — 차례에 든 필드가 그 차례대로, 안 든 필드가 뒤에.
    pub fn rows(&self) -> Vec<Row> {
        let used = self.fields.iter().enumerate().map(|(n, f)| Row { by: f.by, at: Some((n, *f)) });
        let unused = Order::ALL.into_iter().filter(|o| (self.allow_progress || *o != Order::Progress) && !self.fields.iter().any(|f| f.by == *o));
        used.chain(unused.map(|by| Row { by, at: None })).collect()
    }

    /// 고치는 중의 차례.
    pub fn sorting(&self) -> Sorting {
        Sorting::of(&self.fields).expect("편집 창의 차례는 늘 한 필드 이상이고 한 필드는 한 번만 선다")
    }

    /// 커서가 선 필드.
    fn here(&self) -> Order {
        self.rows()[self.cursor].by
    }

    /// 커서를 그 필드의 줄에 세운다 — 넣고 빼고 옮긴 뒤 커서가 **필드를 따라간다**. 자리를 따라가면 `SPC` 로 뺀
    /// 필드 대신 옆 필드에 서서, 한 번 더 누르면 엉뚱한 필드가 빠진다.
    fn follow(&mut self, by: Order) {
        self.cursor = self.rows().iter().position(|r| r.by == by).unwrap_or(0);
    }

    /// 키 하나를 한다.
    pub fn act(&mut self, a: Sorter) -> Done {
        let by = self.here();
        let at = self.fields.iter().position(|f| f.by == by);
        match a {
            Sorter::Down => self.cursor = (self.cursor + 1).min(self.rows().len() - 1),
            Sorter::Up => self.cursor = self.cursor.saturating_sub(1),
            // 차례 안에서만 옮긴다 — 안 든 필드는 차례가 없고, 끝 필드를 더 뒤로 보내면 안 든 필드 사이로 섞인다.
            Sorter::Lower => {
                if let Some(n) = at.filter(|n| n + 1 < self.fields.len()) {
                    self.fields.swap(n, n + 1);
                    self.follow(by);
                }
            }
            Sorter::Raise => {
                if let Some(n) = at.filter(|n| *n > 0) {
                    self.fields.swap(n, n - 1);
                    self.follow(by);
                }
            }
            Sorter::Toggle => {
                match at {
                    Some(n) if self.fields.len() > 1 => {
                        self.fields.remove(n);
                    }
                    Some(_) => {}
                    None => self.fields.push(Ordered::of(by)),
                }
                self.follow(by);
            }
            Sorter::Flip => {
                if let Some(n) = at {
                    self.fields[n] = self.fields[n].flipped();
                }
            }
            Sorter::NextLevel | Sorter::NextGroup | Sorter::Preset(_) => {}
            Sorter::Apply => return Done::Apply(self.sorting()),
            Sorter::Default => return Done::Default(self.sorting()),
            Sorter::Close => return Done::Cancel,
        }
        Done::Stay
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order(e: &Editor) -> Vec<(Order, bool)> {
        e.sorting().fields().iter().map(|f| (f.by, f.down)).collect()
    }

    /// 여섯 필드가 늘 다 서고, 든 것이 먼저 그 차례대로다.
    #[test]
    fn every_field_stands_and_the_used_ones_come_first() {
        let s = Sorting::by(Order::Updated).append(Order::Priority);
        let e = Editor::open(s);
        let rows = e.rows();
        assert_eq!(rows.len(), Order::ALL.len());
        assert_eq!(rows[0].by, Order::Updated);
        assert_eq!(rows[1].by, Order::Priority);
        assert_eq!(rows[1].at.map(|(n, _)| n), Some(1));
        assert!(rows[2..].iter().all(|r| r.at.is_none()));
        assert_eq!(rows[2].by, Order::Created, "안 든 필드가 Order::ALL 의 차례가 아니다");
    }

    /// 넣고, 옮기고, 방향을 뒤집고, 빼고 — 커서는 필드를 따라간다. 마지막 필드는 못 뺀다.
    #[test]
    fn toggle_reorder_and_flip_follow_the_field() {
        let mut e = Editor::open(Sorting::by(Order::Priority));
        // 칸(column)에 내려가 넣는다 — 끝에 제 방향으로 서고 커서가 따라간다.
        while e.here() != Order::Column {
            e.act(Sorter::Down);
        }
        assert_eq!(e.act(Sorter::Toggle), Done::Stay);
        assert_eq!(order(&e), [(Order::Priority, false), (Order::Column, false)]);
        assert_eq!(e.here(), Order::Column);
        // 앞으로 올린다 — 커서가 따라 맨 위에 선다. 맨 위에서 더 올려도 그대로다.
        e.act(Sorter::Raise);
        assert_eq!(order(&e), [(Order::Column, false), (Order::Priority, false)]);
        assert_eq!(e.cursor, 0);
        e.act(Sorter::Raise);
        assert_eq!(order(&e), [(Order::Column, false), (Order::Priority, false)]);
        // 방향을 뒤집는다 — 안 든 필드의 뒤집기는 아무 일도 없다.
        e.act(Sorter::Flip);
        assert_eq!(order(&e), [(Order::Column, true), (Order::Priority, false)]);
        e.act(Sorter::Down);
        e.act(Sorter::Down);
        assert_eq!(e.rows()[e.cursor].at, None);
        e.act(Sorter::Flip);
        assert_eq!(order(&e), [(Order::Column, true), (Order::Priority, false)]);
        // 끝 필드를 더 내리면 그대로다 — 안 든 필드 사이로 안 섞인다.
        e.act(Sorter::Up);
        e.act(Sorter::Lower);
        assert_eq!(order(&e), [(Order::Column, true), (Order::Priority, false)]);
        // 빼면 커서가 그 필드를 따라 안 든 자리로 간다 — 한 번 더 누르면 같은 필드가 도로 든다.
        e.act(Sorter::Toggle);
        assert_eq!(order(&e), [(Order::Column, true)]);
        assert_eq!(e.here(), Order::Priority);
        // 마지막 필드는 못 뺀다.
        e.cursor = 0;
        e.act(Sorter::Toggle);
        assert_eq!(order(&e), [(Order::Column, true)], "마지막 필드가 빠졌다");
    }

    /// Enter·`D`·Esc 는 창을 끝낸다 — 앞의 둘은 고친 차례를 낸다.
    #[test]
    fn apply_default_and_cancel_end_the_window() {
        let mut e = Editor::open(Sorting::by(Order::Priority));
        e.act(Sorter::Flip);
        let flipped = Sorting::by(Order::Priority).press(Order::Priority);
        assert_eq!(e.act(Sorter::Apply), Done::Apply(flipped));
        assert_eq!(e.act(Sorter::Default), Done::Default(flipped));
        assert_eq!(e.act(Sorter::Close), Done::Cancel);
    }
}

/// Screen editing state, translated to and from the shared query engine by App.
/// Keep engine conversion at this boundary rather than duplicating query comparisons in the UI.
#[allow(dead_code)]
pub(super) mod screen {
    use super::Sorting;
    #[cfg(test)]
    use super::{Order, Ordered};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Level {
        Milestone,
        Epic,
        Item,
    }

    pub use crate::nav::Group;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ScreenOrder {
        pub milestone: Sorting,
        pub epic: Sorting,
        pub item: Sorting,
        pub group: Group,
    }

    impl ScreenOrder {
        pub fn get(&self, level: Level) -> Sorting {
            match level {
                Level::Milestone => self.milestone,
                Level::Epic => self.epic,
                Level::Item => self.item,
            }
        }

        pub fn set(&mut self, level: Level, value: Sorting) {
            match level {
                Level::Milestone => self.milestone = value,
                Level::Epic => self.epic = value,
                Level::Item => self.item = value,
            }
        }

        pub fn active(&self, level: Level) -> bool {
            match self.group {
                Group::Structure => true,
                Group::Milestone => level != Level::Epic,
                Group::Epic => level != Level::Milestone,
                _ => level == Level::Item,
            }
        }
    }

    /// A separate editor per level keeps field order and cursor when switching tabs.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ScreenEditor {
        pub group: Group,
        pub level: Level,
        original: ScreenOrder,
        milestone: super::Editor,
        epic: super::Editor,
        item: super::Editor,
    }

    impl ScreenEditor {
        pub fn open(state: ScreenOrder) -> Self {
            let level = if state.active(Level::Milestone) {
                Level::Milestone
            } else if state.active(Level::Epic) {
                Level::Epic
            } else {
                Level::Item
            };
            let mut item = super::Editor::open(state.item);
            item.allow_progress = false;
            Self {
                group: state.group,
                level,
                original: state,
                milestone: super::Editor::open(state.milestone),
                epic: super::Editor::open(state.epic),
                item,
            }
        }

        pub fn original(&self) -> ScreenOrder {
            self.original
        }
        pub fn rows(&mut self) -> Vec<super::Row> {
            self.editor().rows()
        }
        pub fn cursor(&mut self) -> usize {
            self.editor().cursor
        }
        pub fn act(&mut self, action: super::Sorter) -> super::Done {
            use super::Sorter;
            match action {
                Sorter::NextLevel => self.next_level(),
                Sorter::NextGroup => {
                    let groups = Group::ALL;
                    let at = groups.iter().position(|g| *g == self.group).unwrap_or(0);
                    self.choose_group(groups[(at + 1) % groups.len()]);
                }
                Sorter::Preset(_) => {}
                _ => return self.editor().act(action),
            }
            super::Done::Stay
        }

        pub fn state(&self) -> ScreenOrder {
            ScreenOrder {
                milestone: self.milestone.sorting(),
                epic: self.epic.sorting(),
                item: self.item.sorting(),
                group: self.group,
            }
        }

        pub fn editor(&mut self) -> &mut super::Editor {
            match self.level {
                Level::Milestone => &mut self.milestone,
                Level::Epic => &mut self.epic,
                Level::Item => &mut self.item,
            }
        }

        pub fn choose_group(&mut self, group: Group) {
            self.group = group;
            if !self.state().active(self.level) {
                self.level = if self.state().active(Level::Milestone) {
                    Level::Milestone
                } else if self.state().active(Level::Epic) {
                    Level::Epic
                } else {
                    Level::Item
                };
            }
        }

        pub fn next_level(&mut self) {
            let levels = [Level::Milestone, Level::Epic, Level::Item];
            let start = levels.iter().position(|level| *level == self.level).unwrap_or(0);
            self.level = (1..=levels.len())
                .map(|n| levels[(start + n) % levels.len()])
                .find(|level| self.state().active(*level))
                .expect("item is always active");
        }

        pub fn choose_state(&mut self, state: ScreenOrder) {
            let original = self.original;
            *self = Self::open(state);
            self.original = original;
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::query::Preset;
        fn preset(p: Preset) -> ScreenOrder {
            super::super::super::App::screen_preset(p)
        }

        #[test]
        fn presets_replace_all_levels_and_group() {
            let release = preset(Preset::Release);
            assert_eq!(
                release.milestone.fields(),
                &[Ordered { by: Order::Due, down: false }, Ordered { by: Order::Created, down: true }]
            );
            assert_eq!(
                release.epic.fields(),
                &[Ordered { by: Order::Priority, down: false }, Ordered { by: Order::Created, down: false }]
            );
            assert_eq!(preset(Preset::Recent).item, Sorting::by(Order::Updated));
            assert_eq!(preset(Preset::People).group, Group::Assignee);
        }

        #[test]
        fn editing_one_level_leaves_the_others_and_group_intact() {
            let mut state = preset(Preset::Release);
            let before = state;
            state.set(Level::Epic, Sorting::by(Order::Title));
            assert_eq!(state.get(Level::Epic), Sorting::by(Order::Title));
            assert_eq!((state.milestone, state.item, state.group), (before.milestone, before.item, before.group));
        }

        #[test]
        fn editor_preserves_each_tab_and_skips_inactive_tabs() {
            let mut editor = ScreenEditor::open(preset(Preset::Release));
            editor.editor().act(super::super::Sorter::Flip);
            let changed = editor.state().milestone;
            editor.next_level();
            assert_eq!(editor.level, Level::Epic);
            editor.next_level();
            assert_eq!(editor.level, Level::Item);
            editor.next_level();
            assert_eq!(editor.state().milestone, changed);
            editor.choose_group(Group::Assignee);
            assert_eq!(editor.level, Level::Item);
            editor.next_level();
            assert_eq!(editor.level, Level::Item);
            editor.choose_state(preset(Preset::Release));
            assert_eq!(editor.state(), preset(Preset::Release));
        }

        #[test]
        fn active_tabs_follow_grouping() {
            let mut state = preset(Preset::Release);
            assert!([Level::Milestone, Level::Epic, Level::Item].into_iter().all(|l| state.active(l)));
            state.group = Group::Milestone;
            assert!(state.active(Level::Milestone) && !state.active(Level::Epic) && state.active(Level::Item));
            state.group = Group::Epic;
            assert!(!state.active(Level::Milestone) && state.active(Level::Epic) && state.active(Level::Item));
            state.group = Group::Assignee;
            assert!(!state.active(Level::Milestone) && !state.active(Level::Epic) && state.active(Level::Item));
        }
    }
}
