//! Content-addressed pictures. Storage belongs to Shelf; references stay in markdown.
pub mod term;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader};
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Png,
    Jpeg,
    Gif,
    Webp,
}
impl Kind {
    pub fn sniff(b: &[u8]) -> Option<Self> {
        if b.starts_with(b"\x89PNG\r\n\x1a\n") {
            Some(Self::Png)
        } else if b.starts_with(b"\xff\xd8\xff") {
            Some(Self::Jpeg)
        } else if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") {
            Some(Self::Gif)
        } else if b.len() >= 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP" {
            Some(Self::Webp)
        } else {
            None
        }
    }
    pub fn ext(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Gif => "gif",
            Self::Webp => "webp",
        }
    }
    pub fn mime(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Gif => "image/gif",
            Self::Webp => "image/webp",
        }
    }
}
#[derive(Debug, Clone)]
pub struct Settings {
    pub compress: bool,
    pub max_edge: u32,
    pub quality: u8,
    pub warn_bytes: u64,
}
impl Default for Settings {
    fn default() -> Self {
        Self { compress: true, max_edge: 1920, quality: 85, warn_bytes: 2048 * 1024 }
    }
}
impl Settings {
    pub fn from_config(cfg: &crate::config::Config) -> Result<Self, String> {
        let mut s = Self::default();
        for e in &cfg.images {
            let invalid = || format!("invalid {} at config line {}: {}", e.key, e.line, e.raw);
            match e.key.as_str() {
                "image_compress" => {
                    s.compress = match e.word() {
                        Some("on") => true,
                        Some("off") => false,
                        _ => return Err(invalid()),
                    }
                }
                "image_max_edge" => {
                    s.max_edge = e.raw.parse::<u32>().ok().filter(|n| (1..=16384).contains(n)).ok_or_else(invalid)?
                }
                "image_jpeg_quality" => {
                    s.quality = e.raw.parse::<u8>().ok().filter(|n| (1..=100).contains(n)).ok_or_else(invalid)?
                }
                "image_warn_kb" => {
                    s.warn_bytes = e.raw.parse::<u64>().ok().and_then(|n| n.checked_mul(1024)).ok_or_else(invalid)?
                }
                _ => {}
            }
        }
        Ok(s)
    }
}
pub struct Shrunk {
    pub bytes: Vec<u8>,
    pub kind: Kind,
    pub width: u32,
    pub height: u32,
    pub from: (u32, u32),
    pub original_len: usize,
    pub changed: bool,
}
/// Strict dimension limits apply even when original bytes will be kept. Allocation
/// limits alone are best effort in image; the dimension limits are strict.
pub fn decode(bytes: &[u8]) -> Result<DynamicImage, String> {
    let mut decoder = reader(bytes)?.into_decoder().map_err(|e| e.to_string())?;
    let orientation = decoder.orientation().map_err(|e| e.to_string())?;
    let mut img = DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
    img.apply_orientation(orientation);
    Ok(img)
}
fn reader(bytes: &[u8]) -> Result<ImageReader<Cursor<&[u8]>>, String> {
    let mut r = ImageReader::new(Cursor::new(bytes)).with_guessed_format().map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    limits.max_alloc = Some(512 * 1024 * 1024);
    r.limits(limits);
    Ok(r)
}
pub fn png(img: &DynamicImage) -> Result<Vec<u8>, String> {
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}
fn jpeg(img: &DynamicImage, quality: u8) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality)
        .encode_image(&img.to_rgb8())
        .map_err(|e| e.to_string())?;
    Ok(out)
}
pub fn shrink(bytes: &[u8], s: &Settings) -> Result<Shrunk, String> {
    let kind = Kind::sniff(bytes).ok_or("not a PNG, JPEG, GIF or WebP picture")?;
    let img = if matches!(kind, Kind::Png | Kind::Jpeg) { Some(decode(bytes)?) } else { None };
    let from = img.as_ref().map(|i| (i.width(), i.height())).unwrap_or_else(|| {
        if kind == Kind::Gif && bytes.len() >= 10 {
            (u16::from_le_bytes([bytes[6], bytes[7]]) as u32, u16::from_le_bytes([bytes[8], bytes[9]]) as u32)
        } else {
            (0, 0)
        }
    });
    let mut out = Shrunk {
        bytes: bytes.to_vec(),
        kind,
        width: from.0,
        height: from.1,
        from,
        original_len: bytes.len(),
        changed: false,
    };
    if !s.compress {
        return Ok(out);
    }
    let Some(mut img) = img else {
        return Ok(out);
    };
    let resized = img.width().max(img.height()) > s.max_edge;
    if resized {
        img = img.resize(s.max_edge, s.max_edge, image::imageops::FilterType::Lanczos3);
    }
    let (encoded, kind) = if kind == Kind::Jpeg {
        (jpeg(&img, s.quality)?, Kind::Jpeg)
    } else {
        let p = png(&img)?;
        // Only a colour type with alpha can hold a transparent pixel; skip the RGBA copy otherwise.
        if img.color().has_alpha() && img.to_rgba8().pixels().any(|p| p[3] < 255) {
            (p, Kind::Png)
        } else {
            let j = jpeg(&img, s.quality)?;
            if j.len() < p.len() { (j, Kind::Jpeg) } else { (p, Kind::Png) }
        }
    };
    if resized || encoded.len() < bytes.len() {
        out.bytes = encoded;
        out.kind = kind;
        out.width = img.width();
        out.height = img.height();
        out.changed = out.bytes != bytes;
    }
    Ok(out)
}
pub fn name_of(bytes: &[u8], kind: Kind) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, bytes);
    let hex: String = digest.as_ref()[..8].iter().map(|b| format!("{b:02x}")).collect();
    format!("{hex}.{}", kind.ext())
}
/// Add a storage branch here for S3 or another external store. Content hashes are
/// independent of the storage location; there is deliberately no speculative trait.
pub struct Shelf {
    base: PathBuf,
    dir: PathBuf,
    prefix: String,
}
pub struct Put {
    pub name: String,
    pub path: PathBuf,
    pub rel: String,
    pub fresh: bool,
}
impl Shelf {
    pub fn tracker(repo: &crate::store::Repo) -> Self {
        Self { base: repo.root.clone(), dir: repo.dir().join("images"), prefix: ".moai/images".into() }
    }
    pub fn wiki(checkout: &Path, wiki_dir: &str) -> Result<Self, String> {
        let dir = match crate::wiki::dir_of(checkout, wiki_dir) {
            Ok(dir) => dir,
            Err(crate::wiki::DirTrouble::NoDir) => {
                let rel = Path::new(wiki_dir);
                if rel.components().any(|c| {
                    matches!(
                        c,
                        std::path::Component::ParentDir
                            | std::path::Component::RootDir
                            | std::path::Component::Prefix(_)
                    )
                }) || crate::held::into_git(rel)
                {
                    return Err("invalid wiki directory".into());
                }
                let at = checkout.join(rel);
                crate::held::place(&at, &crate::held::Home::of(checkout)).map_err(|e| crate::held::spelled(&at, &e))?
            }
            Err(e) => return Err(crate::view::wiki::dir_trouble(crate::i18n::Lang::En, wiki_dir, &e)),
        };
        Ok(Self { base: checkout.into(), dir: dir.join("images"), prefix: "images".into() })
    }
    pub fn locate(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }
    pub fn rel(&self, name: &str) -> String {
        format!("{}/{name}", self.prefix)
    }
    pub fn put(&self, img: &Shrunk) -> Result<Put, String> {
        // Measure the existing ancestors before creating anything: `create_dir_all` follows a
        // committed `.moai -> <outside>` or `docs -> <outside>` link and would build the shelf
        // outside the checkout before the containment check below could refuse it (`archive` does the same).
        let home = crate::held::Home::of(&self.base);
        let real = crate::held::place_dir(&self.dir, &home).map_err(|e| crate::held::spelled(&self.dir, &e))?;
        std::fs::create_dir_all(&real).map_err(|e| e.to_string())?;
        let base = crate::path::canonical(&self.base).map_err(|e| e.to_string())?;
        let dir = crate::path::canonical(&real).map_err(|e| e.to_string())?;
        if !dir.starts_with(&base) {
            return Err("image shelf escapes the checkout".into());
        }
        // Serialize compare + rename, including on Windows where rename cannot replace a file.
        let lock_at = dir.join(".lock");
        let lock = crate::held::lock(&lock_at, &home).map_err(|e| match e {
            crate::held::Fell::Unheld(why) => crate::held::spelled(&lock_at, &why),
            crate::held::Fell::Io(e) => e.to_string(),
        })?;
        fs2::FileExt::lock_exclusive(&lock).map_err(|e| e.to_string())?;
        let name = name_of(&img.bytes, img.kind);
        let path = self.locate(&name);
        let fresh = match std::fs::symlink_metadata(&path) {
            Ok(meta) => {
                if !meta.is_file()
                    || meta.file_type().is_symlink()
                    || std::fs::read(&path).map_err(|e| e.to_string())? != img.bytes
                {
                    return Err(format!("image name collision: {name}"));
                }
                false
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let temp = dir.join(format!(".{}.tmp", std::process::id()));
                let result = (|| {
                    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&temp)?;
                    file.write_all(&img.bytes)?;
                    file.sync_all()?;
                    std::fs::rename(&temp, &path)
                })();
                if result.is_err() {
                    let _ = std::fs::remove_file(&temp);
                }
                result.map_err(|e| e.to_string())?;
                true
            }
            Err(e) => return Err(e.to_string()),
        };
        Ok(Put { rel: self.rel(&name), name, path, fresh })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn image(w: u32, h: u32, alpha: u8) -> DynamicImage {
        DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(w, h, image::Rgba([30, 80, 150, alpha])))
    }
    #[test]
    fn a_wide_jpeg_is_shrunk_to_the_long_edge() {
        let s = shrink(&jpeg(&image(3000, 1000, 255), 95).unwrap(), &Settings::default()).unwrap();
        assert_eq!((s.width, s.height), (1920, 640));
        assert_eq!(s.from, (3000, 1000));
    }
    #[test]
    fn a_small_image_is_never_enlarged() {
        let s = shrink(&png(&image(12, 8, 128)).unwrap(), &Settings::default()).unwrap();
        assert_eq!((s.width, s.height), (12, 8));
        assert_eq!(s.kind, Kind::Png);
    }
    #[test]
    fn reencoding_that_does_not_help_keeps_the_original_bytes() {
        let b = jpeg(&image(12, 8, 255), 1).unwrap();
        let s = shrink(&b, &Settings::default()).unwrap();
        assert_eq!(s.bytes, b);
        assert!(!s.changed);
    }
    #[test]
    fn an_opaque_png_keeps_the_smaller_of_jpeg_and_png() {
        let i = image(200, 100, 255);
        let p = png(&i).unwrap();
        let j = jpeg(&i, 85).unwrap();
        let s = shrink(&p, &Settings::default()).unwrap();
        assert_eq!(s.bytes.len(), p.len().min(j.len()));
    }
    #[test]
    fn compress_off_keeps_the_bytes() {
        let b = png(&image(8, 8, 255)).unwrap();
        let s = shrink(&b, &Settings { compress: false, ..Settings::default() }).unwrap();
        assert_eq!(s.bytes, b);
    }
    #[test]
    fn the_name_is_the_content_hash() {
        assert_eq!(name_of(b"abc", Kind::Png), "ba7816bf8f01cfea.png");
    }
    #[test]
    fn a_non_image_is_refused() {
        assert!(shrink(b"text", &Settings::default()).is_err());
    }
    #[test]
    fn a_decompression_bomb_is_refused() {
        let b = png(&image(16385, 1, 255)).unwrap();
        assert!(shrink(&b, &Settings::default()).is_err());
    }
    #[test]
    fn exif_orientation_is_applied_before_reencoding() {
        let b = jpeg(&image(80, 40, 255), 95).unwrap();
        // APP1, Exif header, little-endian TIFF, one SHORT orientation tag with value 6.
        let exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
        let mut oriented = b[..2].to_vec();
        oriented.extend_from_slice(b"\xff\xe1");
        oriented.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
        oriented.extend_from_slice(exif);
        oriented.extend_from_slice(&b[2..]);
        let s = shrink(&oriented, &Settings { max_edge: 20, ..Settings::default() }).unwrap();
        assert_eq!((s.width, s.height), (10, 20));
        assert!(!s.bytes.windows(6).any(|b| b == b"Exif\0\0"));
    }
    #[test]
    fn image_keys_are_read() {
        let cfg=crate::config::Config::parse("prefix = \"moai\"\nimage_compress = \"off\"\nimage_max_edge = 40\nimage_jpeg_quality = 90\nimage_warn_kb = 1\n").unwrap();
        let s = Settings::from_config(&cfg).unwrap();
        assert!(!s.compress);
        assert_eq!((s.max_edge, s.quality, s.warn_bytes), (40, 90, 1024));
        let cfg = crate::config::Config::parse("prefix = \"moai\"\nimage_compress = \"typo\"\n").unwrap();
        assert!(Settings::from_config(&cfg).is_err());
        // A key under a table is not the flat setting.
        let cfg = crate::config::Config::parse("prefix = \"moai\"\n[other]\nimage_compress = \"off\"\n").unwrap();
        assert!(Settings::from_config(&cfg).unwrap().compress);
    }
    #[cfg(unix)]
    #[test]
    fn a_shelf_behind_an_outside_link_builds_nothing_outside() {
        let tmp = crate::scratch::Scratch::new("image-shelf-link");
        let outside = crate::scratch::Scratch::new("image-shelf-outside");
        std::os::unix::fs::symlink(outside.path(), tmp.path().join(".moai")).unwrap();
        let shelf = Shelf {
            base: tmp.path().into(),
            dir: tmp.path().join(".moai").join("images"),
            prefix: ".moai/images".into(),
        };
        let s = shrink(&png(&image(4, 4, 128)).unwrap(), &Settings::default()).unwrap();
        assert!(shelf.put(&s).is_err());
        assert!(!outside.path().join("images").exists());
    }
    #[test]
    fn putting_the_same_bytes_twice_writes_once_and_refuses_collisions() {
        let tmp = crate::scratch::Scratch::new("image-shelf");
        let shelf = Shelf { base: tmp.path().into(), dir: tmp.path().join("images"), prefix: "images".into() };
        let s = shrink(&png(&image(10, 10, 128)).unwrap(), &Settings::default()).unwrap();
        let p = shelf.put(&s).unwrap();
        assert!(p.fresh);
        assert!(!shelf.put(&s).unwrap().fresh);
        std::fs::write(&p.path, b"other").unwrap();
        assert!(shelf.put(&s).is_err());
    }
}

pub fn alt(raw: &str) -> String {
    crate::text::one_line(raw).chars().filter(|c| !matches!(c, '[' | ']' | '\\')).take(60).collect()
}
pub fn wiki_prefix(slug: &str) -> Result<String, String> {
    if slug.contains(['\\', ':', '%']) || slug.starts_with('/') || slug.split('/').any(|s| matches!(s, "." | "..")) {
        return Err("invalid wiki slug".into());
    }
    Ok("../".repeat(slug.split('/').count().saturating_sub(1)))
}
pub fn read_input(path: &Path) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("picture must be a regular file".into());
    }
    let mut bytes = Vec::new();
    file.take(64 * 1024 * 1024 + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("picture exceeds 64 MiB input limit".into());
    }
    Ok(bytes)
}
/// Called only with rows re-read under the tracker lock. Field edits add no journal kind.
/// Hands back the row as it stood before and after, so the caller can keep the writer's read mark (`cmd::edit`).
pub fn append(
    issues: &mut [crate::model::Issue],
    cfg: &crate::config::Config,
    id: &str,
    text: &str,
    lang: crate::i18n::Lang,
) -> crate::cmd::R<(crate::model::Issue, crate::model::Issue)> {
    let i = issues.iter_mut().find(|i| i.id == id).ok_or_else(|| crate::cmd::Fail::not_found(id, lang))?;
    let before = i.clone();
    let old = i.body.as_deref().unwrap_or("").trim_end();
    i.body = Some(if old.is_empty() { text.into() } else { format!("{old}\n\n{text}") });
    i.settle(&before, cfg, &crate::model::now())
        .map_err(|why| crate::cmd::Fail::new(crate::view::invalid(lang, &crate::store::At::Id(id.into()), &why)))?;
    Ok((before, i.clone()))
}

#[derive(Clone, Copy)]
pub enum From<'a> {
    Issue,
    Wiki { page_rel: &'a str, wiki_dir: &'a str },
}
/// Pure reference policy shared by all surfaces. No decoding, probing or file I/O.
pub fn resolve(dest: &str, from: From<'_>) -> Result<String, &'static str> {
    if dest.is_empty()
        || dest.starts_with('/')
        || dest.contains(['\\', ':', '%', '\0'])
        || dest.chars().any(char::is_control)
    {
        return Err("absolute, encoded or external image path");
    }
    let dest = dest.strip_prefix("./").unwrap_or(dest);
    match from {
        From::Issue => {
            let name = dest.strip_prefix(".moai/images/").ok_or("not a tracker image")?;
            let (hash, ext) = name.rsplit_once('.').ok_or("missing extension")?;
            if hash.len() != 16
                || !hash.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                || !["png", "jpg", "gif", "webp"].contains(&ext)
            {
                return Err("invalid tracker image name");
            }
            Ok(dest.into())
        }
        From::Wiki { page_rel, wiki_dir } => {
            let normalize = |raw: &str| -> Result<Vec<String>, &'static str> {
                let mut parts = Vec::new();
                for p in raw.split('/') {
                    match p {
                        "" | "." => {}
                        ".." => {
                            parts.pop().ok_or("path escapes checkout")?;
                        }
                        _ => parts.push(p.to_string()),
                    }
                }
                Ok(parts)
            };
            if wiki_dir.contains(['\\', ':', '%']) || wiki_dir.starts_with('/') {
                return Err("invalid wiki directory");
            }
            let root = normalize(wiki_dir)?;
            let page = normalize(page_rel)?;
            if !page.starts_with(&root) {
                return Err("page outside wiki");
            }
            let parent = page[..page.len().saturating_sub(1)].join("/");
            let path = normalize(&format!("{parent}/{dest}"))?;
            if !path.starts_with(&root) || path.len() <= root.len() || path.iter().any(|p| p == ".git" || p == ".moai")
            {
                return Err("image outside wiki");
            }
            let out = path.join("/");
            if !out.rsplit_once('.').is_some_and(|(_, ext)| {
                ["png", "jpg", "jpeg", "gif", "webp"].contains(&ext.to_ascii_lowercase().as_str())
            }) {
                return Err("not an image extension");
            }
            Ok(out)
        }
    }
}
/// Canonical containment check at the I/O boundary, also excluding symlinks out of
/// the approved shelf. Callers never serve/open a reference that resolve refused.
/// Resolved through `path::canonical`, so a Windows path carries no `\\?\` prefix into an OSC 8 file URL.
pub fn local_file(base: &Path, allowed: &str, rel: &str) -> Option<PathBuf> {
    let base = crate::path::canonical(base).ok()?;
    let boundary = crate::path::canonical(base.join(allowed)).ok()?;
    if !boundary.starts_with(&base) {
        return None;
    }
    let path = crate::path::canonical(base.join(rel)).ok()?;
    (path.starts_with(boundary) && path.is_file()).then_some(path)
}
/// The placeholder every surface draws for a picture: `[image]` or `[image: alt]`, and
/// `[image missing: alt]` when a local reference does not resolve. One spelling, so the shell
/// (`term::prepare`) and the explorer (`Context::labels`) cannot drift apart.
pub fn label(lang: crate::i18n::Lang, missing: bool, alt: &str) -> String {
    let word = crate::i18n::say(lang, if missing { "image.missing" } else { "image.placeholder" });
    if alt.is_empty() { format!("[{word}]") } else { format!("[{word}: {alt}]") }
}
/// A reference that is not ours to open (remote) is never called missing.
pub fn remote(dest: &str) -> bool {
    dest.starts_with("https://") || dest.starts_with("http://")
}
/// Neutral markdown knows neither language nor files. Surfaces supply both.
pub fn labels(blocks: &mut [crate::markdown::Block], lang: crate::i18n::Lang) {
    for b in blocks {
        if let crate::markdown::Block::Image { alt, .. } = b {
            *alt = label(lang, false, alt);
        }
    }
}

#[cfg(test)]
mod refs_tests {
    use super::*;
    #[test]
    fn only_tracker_images_resolve_from_an_issue() {
        assert_eq!(
            resolve("./.moai/images/0123456789abcdef.jpg", From::Issue).unwrap(),
            ".moai/images/0123456789abcdef.jpg"
        );
        assert!(resolve("docs/a.png", From::Issue).is_err());
    }
    #[test]
    fn wiki_paths_resolve_from_the_page() {
        assert_eq!(
            resolve("../images/a.png", From::Wiki { page_rel: "docs/lessons/x.md", wiki_dir: "docs" }).unwrap(),
            "docs/images/a.png"
        );
    }
    #[test]
    fn escaping_paths_are_refused() {
        for bad in [
            "../../x.png",
            "/x.png",
            "C:/x.png",
            "C:\\x.png",
            "a\\x.png",
            "%2e%2e/x.png",
            "file:x.png",
            "https://x/a.png",
            "data:x.png",
            "../.git/a.png",
            "../../.moai/images/a.png",
        ] {
            assert!(resolve(bad, From::Wiki { page_rel: "docs/a.md", wiki_dir: "docs" }).is_err(), "{bad}");
        }
        assert!(resolve("a.txt", From::Wiki { page_rel: "docs/a.md", wiki_dir: "docs" }).is_err());
    }
}

/// The server's first binary-file route is an I/O exception to the child-JSON
/// transport, not a second issue-policy engine. resolve remains its only path policy.
pub fn served(repo: &crate::store::Repo, dest: &str) -> Option<(Kind, Vec<u8>)> {
    use std::io::Read;
    let cfg = &repo.config;
    let (base, allowed, rel) = if let Ok(rel) = resolve(dest, From::Issue) {
        (repo.root.as_path(), ".moai/images", rel)
    } else {
        crate::wiki::dir_of(repo.here(), &cfg.wiki_dir).ok()?;
        // The page asks with the normalized path (`docs/images/x.png`, view/image.ts), so compare against the
        // normalized `wiki_dir` too — `./docs` and `docs/` name the same wiki that `wiki::load` reads.
        let wiki = cfg.wiki_dir.split('/').filter(|p| !p.is_empty() && *p != ".").collect::<Vec<_>>().join("/");
        let relative = if wiki.is_empty() { dest } else { dest.strip_prefix(&format!("{wiki}/"))? };
        let page = format!("{wiki}/__image__.md");
        let rel = resolve(relative, From::Wiki { page_rel: &page, wiki_dir: &wiki }).ok()?;
        (repo.here(), cfg.wiki_dir.as_str(), rel)
    };
    let path = local_file(base, allowed, &rel)?;
    let file = crate::held::open_inside(&path, &crate::held::Home::of(base)).ok()?;
    let len = file.metadata().ok()?.len();
    const MAX: u64 = 20 * 1024 * 1024;
    if len > MAX {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(MAX + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() as u64 > MAX {
        return None;
    }
    Some((Kind::sniff(&bytes)?, bytes))
}

/// A caller-selected filesystem context. Markdown/layout themselves never do I/O.
pub struct Context<'a> {
    pub base: &'a Path,
    pub allowed: &'a str,
    pub from: From<'a>,
}
impl<'a> Context<'a> {
    pub fn issue(repo: &'a crate::store::Repo) -> Self {
        Self { base: &repo.root, allowed: ".moai/images", from: From::Issue }
    }
    pub fn file(&self, dest: &str) -> Option<PathBuf> {
        let rel = resolve(dest, self.from).ok()?;
        local_file(self.base, self.allowed, &rel)
    }
    pub fn labels(&self, blocks: &mut [crate::markdown::Block], lang: crate::i18n::Lang) {
        for b in blocks {
            if let crate::markdown::Block::Image { alt, dest, .. } = b {
                let missing = !remote(dest) && self.file(dest).is_none();
                *alt = label(lang, missing, alt);
            }
        }
    }
    pub fn links(&self, body: &str) -> Vec<(String, PathBuf)> {
        crate::markdown::parse(&crate::text::sanitize(body))
            .into_iter()
            .filter_map(|b| match b {
                crate::markdown::Block::Image { dest, .. } => self.file(&dest).map(|path| (dest, path)),
                _ => None,
            })
            .collect()
    }
}
