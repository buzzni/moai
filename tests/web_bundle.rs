//! `moai web` 이 내는 페이지의 번들(`src/web/dist/`)을 node 없이 재는 시험.
//!
//! 번들은 `web/` 에서 `scripts/gen-web.sh` 로 지어 커밋한다 — `cargo build` 가 node 없이 돌아야 해서다. 그러면
//! 소스와 번들이 갈라질 수 있으니 여기서 잡는다. CI 의 web 잡은 node 로 다시 지어 diff 로 한 번 더 잡는다.

use std::path::{Path, PathBuf};

fn at_root(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(name)
}

fn dist(name: &str) -> String {
    std::fs::read_to_string(at_root("src/web/dist").join(name)).unwrap_or_else(|e| panic!("src/web/dist/{name}: {e}"))
}

/// `web/` 밑의 파일들을 `/` 로 이은 상대 경로로 — `web/build.mjs` 의 `walk` 와 같다.
fn walk(web: &Path, dir: &str, out: &mut Vec<String>) {
    for entry in std::fs::read_dir(web.join(dir)).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().into_string().unwrap();
        let path = format!("{dir}{name}");
        if entry.file_type().unwrap().is_dir() {
            walk(web, &format!("{path}/"), out);
        } else {
            out.push(path);
        }
    }
}

/// `web/build.mjs` 의 `sources` 와 같은 접기 — 경로 바이트 차례로 (경로, 0x00, 내용, 0x00) 을 FNV-1a 64 에.
fn sources() -> String {
    let web = at_root("web");
    let mut paths = Vec::new();
    walk(&web, "panel/", &mut paths);
    paths.extend(["build.mjs", "package.json", "package-lock.json"].map(String::from));
    paths.sort();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut fold = |bytes: &[u8]| {
        for b in bytes {
            hash = (hash ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    for path in &paths {
        fold(path.as_bytes());
        fold(&[0]);
        fold(&std::fs::read(web.join(path)).unwrap());
        fold(&[0]);
    }
    format!("fnv1a64 {hash:016x}\n")
}

#[test]
fn the_web_bundle_is_built_from_these_sources() {
    assert_eq!(dist("SOURCES"), sources(), "src/web/dist/ 가 web/ 와 갈라졌다 — scripts/gen-web.sh 를 돌려 담는다");
}

#[test]
fn the_web_page_runs_no_inline_script() {
    let html = dist("index.html");
    let lower = html.to_ascii_lowercase();
    let mut rest = lower.as_str();
    let mut scripts = 0;
    while let Some(at) = rest.find("<script") {
        let tag_end = rest[at..].find('>').expect("<script 이 닫히지 않았다") + at;
        let tag = &rest[at..tag_end];
        assert!(tag.contains(" src="), "src 없는 <script> — 서버의 CSP(script-src 'self')가 막는다: {tag}");
        assert!(rest[tag_end + 1..].starts_with("</script>"), "<script src> 안에 글이 있다: {tag}");
        scripts += 1;
        rest = &rest[tag_end..];
    }
    assert!(scripts > 0, "index.html 이 app.js 를 안 부른다");
    // `on…=` 속성 — 인라인 처리기도 CSP 가 막는다.
    for (i, _) in lower.match_indices(" on") {
        let word: String = lower[i + 3..].chars().take_while(|c| c.is_ascii_alphabetic()).collect();
        assert!(!lower[i + 3 + word.len()..].starts_with('='), "index.html 에 인라인 처리기 on{word}= 가 있다");
    }
}

#[test]
fn the_web_bundle_writes_text_not_markup() {
    let js = dist("app.js");
    for word in ["innerHTML", "outerHTML", "insertAdjacentHTML", "document.write", "eval(", "new Function"] {
        assert!(!js.contains(word), "app.js 에 {word} 가 있다 — 글은 textContent 로만 넣는다");
    }
}

#[test]
fn the_web_bundle_fits_its_budget() {
    let bytes = dist("app.js").len() + dist("app.css").len();
    assert!(bytes <= 1024 * 1024, "app.js+app.css 가 {bytes} 바이트 — 상한 1MiB");
}
