//! Bounded virtual kitty placements. Only the final frame's visible cells trigger uploads.
use crate::image::term::{self, Protocol, Size};
use ratatui::{
    buffer::{Buffer, CellDiffOption},
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use std::{
    collections::VecDeque,
    io::Write,
    path::{Path, PathBuf},
};

pub const PLACEHOLDER: char = '\u{10eeee}';
const LIMIT: usize = 8;

#[derive(Default)]
pub struct Images {
    pub active: bool,
    pub setting: Option<String>,
    protocol: Option<Protocol>,
    geometry: (u16, u16, u16, u16),
    entries: VecDeque<Entry>,
    deleted: Vec<u32>,
    next: u32,
}
struct Entry {
    path: PathBuf,
    stamp: crate::store::Stamp,
    cols: usize,
    size: Size,
    id: u32,
    payload: Vec<u8>,
    sent: bool,
}
impl Images {
    #[cfg(test)]
    pub(super) fn testing() -> Self {
        Self {
            active: true,
            setting: None,
            protocol: Some(Protocol::Kitty),
            geometry: (80, 40, 640, 640),
            entries: VecDeque::new(),
            deleted: Vec::new(),
            next: 0,
        }
    }
    /// Called before layout; pixel changes matter even if the terminal has the same cell count.
    pub fn geometry(&mut self, width: u16, height: u16) -> bool {
        let window = if self.active { ratatui::crossterm::terminal::window_size().ok() } else { None };
        let pixels = window.map_or((0, 0), |w| (w.width, w.height));
        let geometry = (width, height, pixels.0, pixels.1);
        let changed = geometry != self.geometry;
        self.geometry = geometry;
        let stale = self.entries.iter().any(|e| crate::store::stamp(&e.path) != e.stamp);
        if changed || stale {
            self.clear_entries();
        }
        changed || stale
    }
    fn clear_entries(&mut self) {
        self.deleted.extend(self.entries.drain(..).filter(|e| e.sent).map(|e| e.id));
    }
    fn enabled(&mut self) -> bool {
        self.active
            && *self
                .protocol
                .get_or_insert_with(|| term::detect(|k| std::env::var(k).ok(), self.setting.as_deref(), term::in_tmux))
                == Protocol::Kitty
    }
    pub fn valid(&self, lines: &[Line<'_>]) -> bool {
        if !self.active {
            return true;
        }
        lines
            .iter()
            .flat_map(|l| &l.spans)
            .all(|s| !s.content.contains(PLACEHOLDER) || self.entries.iter().any(|e| s.style.fg == Some(color(e.id))))
    }
    pub fn lines(&mut self, path: &Path, cols: usize) -> Vec<Line<'static>> {
        if cols == 0 || !self.enabled() {
            return Vec::new();
        }
        let stamp = crate::store::stamp(path);
        let found = self.entries.iter().position(|e| e.path == path && e.stamp == stamp && e.cols == cols);
        let index = if let Some(index) = found {
            let entry = self.entries.remove(index).unwrap();
            self.entries.push_back(entry);
            self.entries.len() - 1
        } else {
            let Some((img, size)) = self.prepare(path, cols) else {
                return Vec::new();
            };
            let Ok(png) = crate::image::png(&img) else {
                return Vec::new();
            };
            if self.entries.len() == LIMIT
                && let Some(old) = self.entries.pop_front()
                && old.sent
            {
                self.deleted.push(old.id);
            }
            // A process-specific 24-bit namespace; zero is never a valid id.
            if self.next == 0 {
                self.next = ((std::process::id().wrapping_mul(2654435761)) & 0xff_ffff).max(1);
            }
            let id = self.next;
            self.next = (self.next % 0xff_ffff) + 1;
            self.entries.push_back(Entry {
                path: path.into(),
                stamp,
                cols,
                size,
                id,
                payload: term::kitty_virtual(&png, size, id),
                sent: false,
            });
            self.entries.len() - 1
        };
        let e = &self.entries[index];
        (0..e.size.rows as usize)
            .map(|row| {
                Line::from(Span::styled(
                    (0..e.size.cols as usize).map(|col| symbol(row, col)).collect::<String>(),
                    Style::new().fg(color(e.id)),
                ))
            })
            .collect()
    }
    fn prepare(&self, path: &Path, cols: usize) -> Option<(image::DynamicImage, Size)> {
        let bytes = crate::image::read_input(path).ok()?;
        let img = crate::image::decode(&bytes).ok()?;
        let (w, h, pw, ph) = self.geometry;
        let cell = if w > 0 && h > 0 && pw > 0 && ph > 0 {
            (u32::from(pw / w).max(1), u32::from(ph / h).max(1))
        } else {
            (8, 16)
        };
        let mut size = term::size(img.width(), img.height(), cols.min(76) as u32, cell);
        let rows = u32::from((h / 2).clamp(1, 20));
        if size.rows > rows {
            let scale = (rows * cell.1) as f64 / size.pixels.1 as f64;
            size.pixels = ((size.pixels.0 as f64 * scale).round().max(1.) as u32, rows * cell.1);
            size.cols = size.pixels.0.div_ceil(cell.0);
            size.rows = rows;
        }
        Some((img.resize_exact(size.pixels.0, size.pixels.1, image::imageops::FilterType::Triangle), size))
    }
    /// Runs after popups and search styles, before the backend emits the frame's diff.
    pub fn frame(&mut self, buffer: &mut Buffer, out: &mut impl Write) -> std::io::Result<()> {
        for id in self.deleted.drain(..) {
            out.write_all(&term::kitty_delete(id))?;
        }
        for e in &mut self.entries {
            let mut visible = false;
            for cell in &mut buffer.content {
                if cell.symbol().contains(PLACEHOLDER) && cell.fg == color(e.id) {
                    visible = true;
                    cell.set_diff_option(CellDiffOption::ForcedWidth(std::num::NonZeroU16::new(1).unwrap()));
                    cell.modifier.remove(Modifier::DIM | Modifier::REVERSED | Modifier::HIDDEN);
                }
            }
            if visible && !e.sent {
                out.write_all(&e.payload)?;
                e.sent = true;
            }
        }
        out.flush()
    }
    pub fn cleanup(&mut self, out: &mut impl Write) -> std::io::Result<()> {
        for id in self.deleted.drain(..) {
            out.write_all(&term::kitty_delete(id))?;
        }
        for e in &mut self.entries {
            if e.sent {
                out.write_all(&term::kitty_delete(e.id))?;
                e.sent = false;
            }
        }
        out.flush()
    }
}
impl Drop for Images {
    fn drop(&mut self) {
        if self.active {
            let _ = self.cleanup(&mut std::io::stdout().lock());
        }
    }
}
fn color(id: u32) -> Color {
    Color::Rgb((id >> 16) as u8, (id >> 8) as u8, id as u8)
}
fn symbol(row: usize, col: usize) -> String {
    format!("{PLACEHOLDER}{}{}", DIACRITICS[row], DIACRITICS[col])
}

// Kitty's gen/rowcolumn-diacritics.txt, Unicode 6.0. Explicit row AND column support isolated diffs.
const DIACRITICS: &[char] = &[
    '\u{0305}',
    '\u{030d}',
    '\u{030e}',
    '\u{0310}',
    '\u{0312}',
    '\u{033d}',
    '\u{033e}',
    '\u{033f}',
    '\u{0346}',
    '\u{034a}',
    '\u{034b}',
    '\u{034c}',
    '\u{0350}',
    '\u{0351}',
    '\u{0352}',
    '\u{0357}',
    '\u{035b}',
    '\u{0363}',
    '\u{0364}',
    '\u{0365}',
    '\u{0366}',
    '\u{0367}',
    '\u{0368}',
    '\u{0369}',
    '\u{036a}',
    '\u{036b}',
    '\u{036c}',
    '\u{036d}',
    '\u{036e}',
    '\u{036f}',
    '\u{0483}',
    '\u{0484}',
    '\u{0485}',
    '\u{0486}',
    '\u{0487}',
    '\u{0592}',
    '\u{0593}',
    '\u{0594}',
    '\u{0595}',
    '\u{0597}',
    '\u{0598}',
    '\u{0599}',
    '\u{059c}',
    '\u{059d}',
    '\u{059e}',
    '\u{059f}',
    '\u{05a0}',
    '\u{05a1}',
    '\u{05a8}',
    '\u{05a9}',
    '\u{05ab}',
    '\u{05ac}',
    '\u{05af}',
    '\u{05c4}',
    '\u{0610}',
    '\u{0611}',
    '\u{0612}',
    '\u{0613}',
    '\u{0614}',
    '\u{0615}',
    '\u{0616}',
    '\u{0617}',
    '\u{0657}',
    '\u{0658}',
    '\u{0659}',
    '\u{065a}',
    '\u{065b}',
    '\u{065d}',
    '\u{065e}',
    '\u{06d6}',
    '\u{06d7}',
    '\u{06d8}',
    '\u{06d9}',
    '\u{06da}',
    '\u{06db}',
    '\u{06dc}',
    '\u{06df}',
    '\u{06e0}',
    '\u{06e1}',
    '\u{06e2}',
    '\u{06e4}',
    '\u{06e7}',
    '\u{06e8}',
    '\u{06eb}',
    '\u{06ec}',
    '\u{0730}',
    '\u{0732}',
    '\u{0733}',
    '\u{0735}',
    '\u{0736}',
    '\u{073a}',
    '\u{073d}',
    '\u{073f}',
    '\u{0740}',
    '\u{0741}',
    '\u{0743}',
    '\u{0745}',
    '\u{0747}',
    '\u{0749}',
    '\u{074a}',
    '\u{07eb}',
    '\u{07ec}',
    '\u{07ed}',
    '\u{07ee}',
    '\u{07ef}',
    '\u{07f0}',
    '\u{07f1}',
    '\u{07f3}',
    '\u{0816}',
    '\u{0817}',
    '\u{0818}',
    '\u{0819}',
    '\u{081b}',
    '\u{081c}',
    '\u{081d}',
    '\u{081e}',
    '\u{081f}',
    '\u{0820}',
    '\u{0821}',
    '\u{0822}',
    '\u{0823}',
    '\u{0825}',
    '\u{0826}',
    '\u{0827}',
    '\u{0829}',
    '\u{082a}',
    '\u{082b}',
    '\u{082c}',
    '\u{082d}',
    '\u{0951}',
    '\u{0953}',
    '\u{0954}',
    '\u{0f82}',
    '\u{0f83}',
    '\u{0f86}',
    '\u{0f87}',
    '\u{135d}',
    '\u{135e}',
    '\u{135f}',
    '\u{17dd}',
    '\u{193a}',
    '\u{1a17}',
    '\u{1a75}',
    '\u{1a76}',
    '\u{1a77}',
    '\u{1a78}',
    '\u{1a79}',
    '\u{1a7a}',
    '\u{1a7b}',
    '\u{1a7c}',
    '\u{1b6b}',
    '\u{1b6d}',
    '\u{1b6e}',
    '\u{1b6f}',
    '\u{1b70}',
    '\u{1b71}',
    '\u{1b72}',
    '\u{1b73}',
    '\u{1cd0}',
    '\u{1cd1}',
    '\u{1cd2}',
    '\u{1cda}',
    '\u{1cdb}',
    '\u{1ce0}',
    '\u{1dc0}',
    '\u{1dc1}',
    '\u{1dc3}',
    '\u{1dc4}',
    '\u{1dc5}',
    '\u{1dc6}',
    '\u{1dc7}',
    '\u{1dc8}',
    '\u{1dc9}',
    '\u{1dcb}',
    '\u{1dcc}',
    '\u{1dd1}',
    '\u{1dd2}',
    '\u{1dd3}',
    '\u{1dd4}',
    '\u{1dd5}',
    '\u{1dd6}',
    '\u{1dd7}',
    '\u{1dd8}',
    '\u{1dd9}',
    '\u{1dda}',
    '\u{1ddb}',
    '\u{1ddc}',
    '\u{1ddd}',
    '\u{1dde}',
    '\u{1ddf}',
    '\u{1de0}',
    '\u{1de1}',
    '\u{1de2}',
    '\u{1de3}',
    '\u{1de4}',
    '\u{1de5}',
    '\u{1de6}',
    '\u{1dfe}',
    '\u{20d0}',
    '\u{20d1}',
    '\u{20d4}',
    '\u{20d5}',
    '\u{20d6}',
    '\u{20d7}',
    '\u{20db}',
    '\u{20dc}',
    '\u{20e1}',
    '\u{20e7}',
    '\u{20e9}',
    '\u{20f0}',
    '\u{2cef}',
    '\u{2cf0}',
    '\u{2cf1}',
    '\u{2de0}',
    '\u{2de1}',
    '\u{2de2}',
    '\u{2de3}',
    '\u{2de4}',
    '\u{2de5}',
    '\u{2de6}',
    '\u{2de7}',
    '\u{2de8}',
    '\u{2de9}',
    '\u{2dea}',
    '\u{2deb}',
    '\u{2dec}',
    '\u{2ded}',
    '\u{2dee}',
    '\u{2def}',
    '\u{2df0}',
    '\u{2df1}',
    '\u{2df2}',
    '\u{2df3}',
    '\u{2df4}',
    '\u{2df5}',
    '\u{2df6}',
    '\u{2df7}',
    '\u{2df8}',
    '\u{2df9}',
    '\u{2dfa}',
    '\u{2dfb}',
    '\u{2dfc}',
    '\u{2dfd}',
    '\u{2dfe}',
    '\u{2dff}',
    '\u{a66f}',
    '\u{a67c}',
    '\u{a67d}',
    '\u{a6f0}',
    '\u{a6f1}',
    '\u{a8e0}',
    '\u{a8e1}',
    '\u{a8e2}',
    '\u{a8e3}',
    '\u{a8e4}',
    '\u{a8e5}',
    '\u{a8e6}',
    '\u{a8e7}',
    '\u{a8e8}',
    '\u{a8e9}',
    '\u{a8ea}',
    '\u{a8eb}',
    '\u{a8ec}',
    '\u{a8ed}',
    '\u{a8ee}',
    '\u{a8ef}',
    '\u{a8f0}',
    '\u{a8f1}',
    '\u{aab0}',
    '\u{aab2}',
    '\u{aab3}',
    '\u{aab7}',
    '\u{aab8}',
    '\u{aabe}',
    '\u{aabf}',
    '\u{aac1}',
    '\u{fe20}',
    '\u{fe21}',
    '\u{fe22}',
    '\u{fe23}',
    '\u{fe24}',
    '\u{fe25}',
    '\u{fe26}',
    '\u{10a0f}',
    '\u{10a38}',
    '\u{1d185}',
    '\u{1d186}',
    '\u{1d187}',
    '\u{1d188}',
    '\u{1d189}',
    '\u{1d1aa}',
    '\u{1d1ab}',
    '\u{1d1ac}',
    '\u{1d1ad}',
    '\u{1d242}',
    '\u{1d243}',
    '\u{1d244}',
];
#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend, widgets::Paragraph};
    fn images() -> Images {
        Images::testing()
    }
    fn picture(root: &Path, name: &str) -> PathBuf {
        let p = root.join(name);
        image::DynamicImage::new_rgb8(160, 160).save(&p).unwrap();
        p
    }
    #[test]
    fn placeholder_cells_upload_once_scroll_cover_cleanup_and_resume() {
        let root = crate::scratch::Scratch::new("virtual-kitty");
        let path = picture(&root, "image.png");
        let mut images = images();
        let lines = images.lines(&path, 20);
        let entry = &images.entries[0];
        let id = entry.id;
        assert_eq!(lines[0].spans[0].style.fg, Some(color(id)));
        assert!(lines[0].spans[0].content.starts_with(&symbol(0, 0)));
        assert!(lines[1].spans[0].content.starts_with(&symbol(1, 0)));
        let mut terminal = Terminal::new(TestBackend::new(20, 10)).unwrap();
        let mut out = Vec::new();
        terminal
            .draw(|f| {
                f.render_widget(Paragraph::new(lines.clone()), f.area());
                images.frame(f.buffer_mut(), &mut out).unwrap();
            })
            .unwrap();
        let first = String::from_utf8(out.clone()).unwrap();
        assert!(first.contains(&format!("a=T,U=1,f=100,i={id},c=20,r=10,q=2")));
        assert!(!first.ends_with('\n'));
        out.clear();
        terminal
            .draw(|f| {
                f.render_widget(Paragraph::new(lines.clone()).scroll((2, 0)), f.area());
                images.frame(f.buffer_mut(), &mut out).unwrap();
            })
            .unwrap();
        assert!(out.is_empty(), "scroll retransmitted image");
        terminal
            .draw(|f| {
                f.render_widget(Paragraph::new("help popup"), f.area());
                images.frame(f.buffer_mut(), &mut out).unwrap();
            })
            .unwrap();
        assert!(out.is_empty());
        assert!(!terminal.backend().buffer().content.iter().any(|c| c.symbol().contains(PLACEHOLDER)));
        images.cleanup(&mut out).unwrap();
        assert_eq!(out, term::kitty_delete(id));
        out.clear();
        terminal
            .draw(|f| {
                f.render_widget(Paragraph::new(lines), f.area());
                images.frame(f.buffer_mut(), &mut out).unwrap();
            })
            .unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), first);
        images.active = false;
    }
    #[test]
    fn cache_is_bounded_and_replacement_invalidates_old_lines() {
        let root = crate::scratch::Scratch::new("virtual-limit");
        let mut images = images();
        images.geometry(80, 40);
        let p = picture(&root, "0.png");
        let old = images.lines(&p, 20);
        let id = images.entries[0].id;
        images.entries[0].sent = true;
        for n in 1..9 {
            images.lines(&picture(&root, &format!("{n}.png")), 20);
        }
        assert_eq!(images.entries.len(), LIMIT);
        assert!(!images.valid(&old));
        assert_eq!(images.deleted, vec![id]);
        let lines = images.lines(&p, 20);
        assert!(images.valid(&lines));
        std::fs::write(&p, b"corrupt now").unwrap();
        assert!(images.geometry(80, 40));
        assert!(!images.valid(&lines));
        assert!(images.lines(&p, 20).is_empty());
        images.active = false;
    }
    #[test]
    fn resize_rebuilds_placements_at_the_new_height() {
        let root = crate::scratch::Scratch::new("virtual-resize");
        let p = picture(&root, "image.png");
        let mut images = images();
        let old = images.lines(&p, 20);
        assert!(images.geometry(80, 6));
        let new = images.lines(&p, 20);
        assert!(!images.valid(&old));
        assert_eq!(new.len(), 3);
        images.active = false;
    }
    #[test]
    fn disabled_and_unsupported_images_keep_text_layout() {
        let root = crate::scratch::Scratch::new("virtual-off");
        let p = picture(&root, "one.png");
        assert!(Images::default().lines(&p, 20).is_empty());
        let mut images = images();
        images.protocol = Some(Protocol::Off);
        assert!(images.lines(&p, 20).is_empty());
        images.protocol = Some(Protocol::Kitty);
        let bad = root.join("bad.gif");
        std::fs::write(&bad, b"GIF89a").unwrap();
        assert!(images.lines(&bad, 20).is_empty());
        images.active = false;
    }
}
