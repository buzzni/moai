//! Shared graphics protocols. No terminal escape queries; inherited TMUX is checked once against pane ttys.
use super::From;
use base64::{Engine, engine::general_purpose::STANDARD};
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Kitty,
    Iterm,
    Sixel,
    Off,
}
fn mode(s: &str) -> Option<Protocol> {
    match s {
        "kitty" => Some(Protocol::Kitty),
        "iterm" => Some(Protocol::Iterm),
        "sixel" => Some(Protocol::Sixel),
        "off" => Some(Protocol::Off),
        _ => None,
    }
}
pub fn detect(
    env: impl Fn(&str) -> Option<String>,
    setting: Option<&str>,
    in_tmux: impl FnOnce(&str) -> Option<bool>,
) -> Protocol {
    let explicit = env("MOAI_IMAGES");
    if let Some(chosen) = explicit.as_deref().or(setting) {
        if let Some(p) = mode(chosen) {
            return p;
        }
        if chosen != "auto" {
            return Protocol::Off;
        }
    }
    let term = env("TERM").unwrap_or_default();
    let program = env("TERM_PROGRAM").unwrap_or_default();
    if term.starts_with("screen")
        || term.starts_with("tmux")
        || env("TMUX").is_some_and(|value| in_tmux(&value) != Some(false))
    {
        Protocol::Off
    } else if env("KITTY_WINDOW_ID").is_some() || term == "xterm-kitty" || program == "ghostty" {
        Protocol::Kitty
    } else if ["iTerm.app", "WezTerm"].contains(&program.as_str()) || env("LC_TERMINAL").as_deref() == Some("iTerm2") {
        Protocol::Iterm
    } else if env("WT_SESSION").is_some() || term.starts_with("foot") || term == "mlterm" {
        Protocol::Sixel
    } else {
        Protocol::Off
    }
}
/// Compare stdout's device with the server's panes. Unknown means graphics stay off.
#[cfg(unix)]
pub fn in_tmux(value: &str) -> Option<bool> {
    use std::os::{fd::AsFd, unix::fs::MetadataExt};
    let stdout = std::io::stdout();
    if !stdout.is_terminal() {
        return None;
    }
    let socket = value.rsplitn(3, ',').nth(2)?;
    // Read through `MetadataExt::rdev` on both sides: `libc::stat::st_rdev` is `i32` on macOS and `u64` on Linux.
    let device = std::fs::File::from(stdout.as_fd().try_clone_to_owned().ok()?).metadata().ok()?.rdev();
    let mut command = std::process::Command::new("tmux");
    command.args(["-S", socket, "list-panes", "-a", "-F", "#{pane_tty}"]);
    let output = tmux_output(command, std::time::Duration::from_secs(2))?;
    let text = std::str::from_utf8(&output).ok()?;
    if text.trim().is_empty() {
        return None;
    }
    // A pane whose tty no longer stats (a dead pane kept by `remain-on-exit`) cannot be our open stdout, so it does
    // not decide anything. Only when no pane could be checked at all is the answer unknown.
    let mut checked = false;
    for tty in text.lines() {
        let Ok(meta) = std::fs::metadata(tty) else { continue };
        if meta.rdev() == device {
            return Some(true);
        }
        checked = true;
    }
    checked.then_some(false)
}
/// Bound the whole probe, including stdout collection. A nonblocking socket avoids a reader thread that could
/// outlive the timeout if an inherited stdout remains open in a descendant of the client.
#[cfg(unix)]
fn tmux_output(mut command: std::process::Command, timeout: std::time::Duration) -> Option<Vec<u8>> {
    use std::{
        io::Read,
        os::{fd::OwnedFd, unix::net::UnixStream},
        process::Stdio,
        time::{Duration, Instant},
    };
    let (mut reader, writer) = UnixStream::pair().ok()?;
    reader.set_nonblocking(true).ok()?;
    let stdout: OwnedFd = writer.into();
    let mut child = command.stdin(Stdio::null()).stderr(Stdio::null()).stdout(Stdio::from(stdout)).spawn().ok()?;
    let began = Instant::now();
    let result = (|| {
        let mut output = Vec::new();
        loop {
            let status = child.try_wait().ok()?;
            let mut chunk = [0; 4096];
            loop {
                match reader.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => {
                        // A pane list is small; excessive output is an unknown answer too.
                        if output.len() + n > 1024 * 1024 {
                            return None;
                        }
                        output.extend_from_slice(&chunk[..n]);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => return None,
                }
            }
            if let Some(status) = status {
                return status.success().then_some(output);
            }
            let left = timeout.checked_sub(began.elapsed())?;
            std::thread::sleep(left.min(Duration::from_millis(10)));
        }
    })();
    if result.is_none() {
        // Kill and reap the client on timeout/read failure; never leave a hung probe behind.
        let _ = child.kill();
        let _ = child.wait();
    }
    result
}
#[cfg(not(unix))]
pub fn in_tmux(_: &str) -> Option<bool> {
    None
}

/// Unknown terminal pixels use a cell twice as tall as it is wide.
const FALLBACK_CELL: (u32, u32) = (8, 16);
#[derive(Debug, Clone, Copy)]
pub struct Size {
    pub cols: u32,
    pub rows: u32,
    pub pixels: (u32, u32),
}
pub fn size(w: u32, h: u32, cols: u32, cell: (u32, u32)) -> Size {
    let (cw, ch) = (cell.0.max(1), cell.1.max(1));
    let mut scale = (cols.clamp(1, 76) * cw) as f64 / w.max(1) as f64;
    scale = scale.min((30 * ch) as f64 / h.max(1) as f64).min(1.0);
    let pixels = ((w as f64 * scale).round().max(1.0) as u32, (h as f64 * scale).round().max(1.0) as u32);
    Size { cols: pixels.0.div_ceil(cw), rows: pixels.1.div_ceil(ch), pixels }
}
pub fn kitty(png: &[u8], s: Size) -> Vec<u8> {
    kitty_chunks(png, format!("a=T,f=100,c={},r={},q=2", s.cols, s.rows), true)
}
pub fn kitty_virtual(png: &[u8], s: Size, id: u32) -> Vec<u8> {
    kitty_chunks(png, format!("a=T,U=1,f=100,i={id},c={},r={},q=2", s.cols, s.rows), false)
}
pub fn kitty_delete(id: u32) -> Vec<u8> {
    format!("\x1b_Ga=d,d=I,i={id},q=2;\x1b\\").into_bytes()
}
fn kitty_chunks(png: &[u8], first: String, newline: bool) -> Vec<u8> {
    let encoded = STANDARD.encode(png);
    let mut out = Vec::new();
    let chunks: Vec<_> = encoded.as_bytes().chunks(4096).collect();
    for (n, c) in chunks.iter().enumerate() {
        let more = usize::from(n + 1 < chunks.len());
        let header = if n == 0 { format!("\x1b_G{first},m={more};") } else { format!("\x1b_Gm={more};") };
        out.extend_from_slice(header.as_bytes());
        out.extend_from_slice(c);
        out.extend_from_slice(b"\x1b\\");
    }
    if newline {
        out.extend_from_slice(b"\n");
    }
    out
}
pub fn iterm(bytes: &[u8], s: Size) -> Vec<u8> {
    format!(
        "\x1b]1337;File=inline=1;size={};width={};height={};preserveAspectRatio=1:{}\x07\n",
        bytes.len(),
        s.cols,
        s.rows,
        STANDARD.encode(bytes)
    )
    .into_bytes()
}
pub fn sixel(img: &image::DynamicImage, s: Size) -> Vec<u8> {
    let img = img.resize_exact(s.pixels.0, s.pixels.1, image::imageops::FilterType::Triangle).to_rgba8();
    let (w, h) = img.dimensions();
    let mut out = format!("\x1bPq\"1;1;{w};{h}");
    for r in 0..6 {
        for g in 0..6 {
            for b in 0..6 {
                let n = r * 36 + g * 6 + b;
                out.push_str(&format!("#{n};2;{};{};{}", r * 20, g * 20, b * 20));
            }
        }
    }
    // Quantize each pixel once; a band then fills only the colour rows it actually uses instead of
    // rescanning the whole band for each of the 216 colours.
    let q = |c: u8| (c as usize * 5 + 127) / 255;
    let index: Vec<Option<usize>> =
        img.pixels().map(|p| (p[3] >= 128).then(|| q(p[0]) * 36 + q(p[1]) * 6 + q(p[2]))).collect();
    let mut rows = vec![vec![0u8; w as usize]; 216];
    for y in (0..h).step_by(6) {
        let mut used = [false; 216];
        for dy in 0..6 {
            if y + dy >= h {
                break;
            }
            for x in 0..w {
                if let Some(color) = index[((y + dy) * w + x) as usize] {
                    rows[color][x as usize] |= 1 << dy;
                    used[color] = true;
                }
            }
        }
        for color in (0..216).filter(|c| used[*c]) {
            let row = &mut rows[color];
            let Some(last) = row.iter().rposition(|b| *b != 0) else { continue };
            out.push_str(&format!("#{color}"));
            let mut x = 0;
            while x <= last {
                let value = row[x];
                let mut end = x + 1;
                while end <= last && row[end] == value {
                    end += 1;
                }
                if end - x >= 4 {
                    out.push_str(&format!("!{}{}", end - x, (value + 63) as char));
                } else {
                    for _ in x..end {
                        out.push((value + 63) as char);
                    }
                }
                x = end;
            }
            row.fill(0);
            out.push('$');
        }
        if y + 6 < h {
            out.push('-');
        }
    }
    out.push_str("\x1b\\");
    // The terminal moves the text cursor below the image itself (sixel scrolling); one newline ends
    // the line. A newline per image row left a blank gap as tall as the picture under it.
    out.push('\n');
    out.into_bytes()
}
pub fn file_url(path: &str) -> String {
    let path = path.replace('\\', "/");
    let mut out = if path.starts_with('/') { "file://".to_string() } else { "file:///".to_string() };
    for b in path.bytes() {
        if b.is_ascii_alphanumeric() || b"/-._~:".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}
fn linked(text: &str, path: &Path) -> String {
    format!("\x1b]8;;{}\x1b\\{text}\x1b]8;;\x1b\\", file_url(&path.to_string_lossy()))
}
#[derive(Clone)]
struct Draw {
    line: usize,
    path: PathBuf,
    dest: String,
    setting: Option<String>,
}
static DRAW: Mutex<Vec<Draw>> = Mutex::new(Vec::new());
/// Replace only the body's known image blocks; history and raw/JSON never call this.
pub fn prepare(
    lines: &mut [String],
    body: &str,
    from: From<'_>,
    base: &Path,
    allowed: &str,
    lang: crate::i18n::Lang,
    setting: Option<&str>,
) {
    let blocks = crate::markdown::parse(&crate::text::sanitize(body));
    let mut after = 0;
    let context = super::Context { base, allowed, from };
    for block in blocks {
        let crate::markdown::Block::Image { alt, dest, .. } = block else { continue };
        let found = context.file(&dest);
        let label = super::label(lang, found.is_none() && !super::remote(&dest), &alt);
        // The neutral view uses English labels; match the literal path and image marker,
        // not arbitrary occurrences of an alt in a heading or prose.
        if let Some(n) = lines.iter().enumerate().skip(after).find_map(|(n, l)| {
            let plain = crate::style::plain(l);
            (plain.contains("[image") && plain.ends_with(&format!(" {dest}"))).then_some(n)
        }) {
            let plain = crate::style::plain(&lines[n]);
            let start = plain.find("[image").unwrap();
            lines[n] = format!("{}{label} {dest}", &plain[..start]);
            after = n + 1;
            if let Some(path) = found {
                DRAW.lock().unwrap().push(Draw { line: n, path, dest, setting: setting.map(str::to_owned) });
            }
        }
    }
}
/// Interleave text (through anstream) and terminal bytes (directly). stdout is
/// tested here too: forcing a protocol never puts escape bytes in a pipe.
pub fn print(lines: &[String]) -> std::io::Result<bool> {
    let draws = std::mem::take(&mut *DRAW.lock().unwrap());
    if !std::io::stdout().is_terminal() || draws.is_empty() {
        return Ok(false);
    }
    let protocol = detect(|k| std::env::var(k).ok(), draws[0].setting.as_deref(), in_tmux);
    let mut start = 0;
    for d in draws {
        if d.line < start || d.line >= lines.len() {
            continue;
        }
        if d.line > start {
            writeln!(anstream::stdout(), "{}", lines[start..d.line].join("\n"))?;
        }
        anstream::stdout().flush()?;
        let raw = crate::style::plain(&lines[d.line]);
        let line = raw.replace(&d.dest, &linked(&d.dest, &d.path));
        let mut stdout = std::io::stdout().lock();
        writeln!(stdout, "{line}")?;
        if protocol != Protocol::Off
            && let Ok(bytes) = super::read_input(&d.path)
            && let Ok(img) = super::decode(&bytes)
        {
            let window = ratatui::crossterm::terminal::window_size().ok();
            let cols = window.as_ref().map_or(76, |w| w.columns as u32);
            let cell = window
                .as_ref()
                .and_then(|w| {
                    (w.columns > 0 && w.rows > 0 && w.width > 0 && w.height > 0)
                        .then(|| (w.width as u32 / w.columns as u32, w.height as u32 / w.rows as u32))
                })
                .unwrap_or(FALLBACK_CELL);
            let s = size(img.width(), img.height(), cols, cell);
            let payload = match protocol {
                Protocol::Kitty => super::png(&img).map(|b| kitty(&b, s)),
                Protocol::Iterm => Ok(iterm(&bytes, s)),
                Protocol::Sixel => Ok(sixel(&img, s)),
                Protocol::Off => Ok(vec![]),
            };
            if let Ok(payload) = payload {
                stdout.write_all(&payload)?;
            }
        }
        stdout.flush()?;
        start = d.line + 1;
    }
    if start < lines.len() {
        writeln!(anstream::stdout(), "{}", lines[start..].join("\n"))?;
    }
    Ok(true)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detect_table() {
        let cases: [(&[(&str, &str)], Protocol); 8] = [
            (&[("TERM", "xterm-kitty")], Protocol::Kitty),
            (&[("TERM_PROGRAM", "ghostty")], Protocol::Kitty),
            (&[("TERM_PROGRAM", "WezTerm")], Protocol::Iterm),
            (&[("WT_SESSION", "1")], Protocol::Sixel),
            (&[("TERM", "foot")], Protocol::Sixel),
            (&[("TMUX", "x"), ("KITTY_WINDOW_ID", "1")], Protocol::Off),
            (&[("MOAI_IMAGES", "kitty"), ("TMUX", "x")], Protocol::Kitty),
            (&[], Protocol::Off),
        ];
        for (env, want) in cases {
            assert_eq!(
                detect(|k| env.iter().find(|(key, _)| *key == k).map(|(_, v)| v.to_string()), None, |_| None),
                want
            );
        }
        assert_eq!(detect(|_| None, Some("off"), |_| panic!("explicit mode queried tmux")), Protocol::Off);
    }
    #[test]
    fn inherited_tmux_and_real_panes_are_distinct() {
        let vars =
            [("TMUX", "/tmp/server,12,3"), ("TERM", "xterm-kitty"), ("KITTY_WINDOW_ID", "5"), ("TERM_PROGRAM", "tmux")];
        let env = |k: &str| vars.iter().find(|(key, _)| *key == k).map(|(_, v)| v.to_string());
        assert_eq!(detect(env, None, |_| Some(false)), Protocol::Kitty);
        assert_eq!(detect(env, None, |_| Some(true)), Protocol::Off);
        assert_eq!(detect(env, None, |_| None), Protocol::Off);
        assert_eq!(detect(env, Some("kitty"), |_| panic!("override queried tmux")), Protocol::Kitty);
        assert_eq!(
            detect(
                |k| if k == "TERM" { Some("screen-256color".into()) } else { env(k) },
                None,
                |_| panic!("screen queried tmux")
            ),
            Protocol::Off
        );
        assert_eq!(
            detect(
                |k| if k == "MOAI_IMAGES" { Some("off".into()) } else { env(k) },
                None,
                |_| panic!("override queried tmux")
            ),
            Protocol::Off
        );
    }
    #[cfg(unix)]
    #[test]
    fn tmux_probe_times_out_without_a_real_server() {
        let mut command = std::process::Command::new("/bin/sh");
        // Output alone must not count as success before the client finishes.
        command.args(["-c", "printf '/dev/null\\n'; while :; do :; done"]);
        let began = std::time::Instant::now();
        let answer = tmux_output(command, std::time::Duration::from_millis(50));
        assert!(answer.is_none(), "a timed-out probe must leave graphics off");
        assert!(began.elapsed() < std::time::Duration::from_secs(2), "the probe did not respect its deadline");
        let mut command = std::process::Command::new("/bin/sh");
        command.args(["-c", "printf '/dev/null\\n'"]);
        assert_eq!(tmux_output(command, std::time::Duration::from_secs(2)), Some(b"/dev/null\n".to_vec()));
        let mut command = std::process::Command::new("/bin/sh");
        command.args(["-c", "printf '/dev/null\\n'; exit 1"]);
        assert!(tmux_output(command, std::time::Duration::from_secs(2)).is_none());
    }
    #[test]
    fn kitty_payload_is_chunked_at_4096_and_ends_with_m0() {
        let s = size(100, 50, 76, FALLBACK_CELL);
        let b = kitty(&vec![42; 12000], s);
        let b = String::from_utf8(b).unwrap();
        assert!(b.contains("m=1;"));
        assert!(b.contains("m=0;"));
        for chunk in b.split("\x1b_G").skip(1) {
            assert!(chunk.split_once(';').unwrap().1.split("\x1b\\").next().unwrap().len() <= 4096);
        }
    }
    #[test]
    fn iterm_payload_names_its_size() {
        assert!(String::from_utf8(iterm(b"abc", size(1, 1, 76, FALLBACK_CELL))).unwrap().contains("size=3;"));
    }
    #[test]
    fn sixel_output_is_a_dcs_with_at_most_256_colours() {
        let img = image::DynamicImage::new_rgb8(5, 5);
        let b = sixel(&img, size(5, 5, 76, FALLBACK_CELL));
        let s = String::from_utf8(b).unwrap();
        assert!(s.starts_with("\x1bPq"));
        assert_eq!(s.matches(";2;").count(), 216);
        // One newline ends the line; the terminal already put the cursor below the picture.
        assert!(s.ends_with("\x1b\\\n"), "{:?}", &s[s.len().saturating_sub(8)..]);
    }
    #[test]
    fn osc8_file_url_encodes_spaces_and_windows_drives() {
        assert_eq!(file_url("C:\\a b\\x.png"), "file:///C:/a%20b/x.png");
        assert_eq!(file_url("/a/한\x1b.png"), "file:///a/%ED%95%9C%1B.png");
    }
}
