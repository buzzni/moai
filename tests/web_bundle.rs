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
    paths.extend(["build.mjs", "i18n-bundles.mjs", "package.json", "package-lock.json"].map(String::from));
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
    // 페이지의 말은 `i18n/*.json` 에서 온다 — 그 파일의 `page.` 키만 app.js 에 들어가므로 그것만 접는다(CLI 자신의 말이 바뀌어도
    // 번들은 안 낡는다). 언어 파일마다 이름 차례로 (파일 이름, 0x00, [키, 0x00, 값, 0x00]…) — `web/build.mjs` 의 `sources` 와 같다.
    let mut codes: Vec<String> = std::fs::read_dir(at_root("i18n"))
        .unwrap()
        .filter_map(|e| e.unwrap().file_name().into_string().ok())
        .filter_map(|name| name.strip_suffix(".json").map(String::from))
        .collect();
    codes.sort();
    for code in &codes {
        fold(format!("i18n/{code}.json").as_bytes());
        fold(&[0]);
        let all: std::collections::BTreeMap<String, String> =
            serde_json::from_str(&std::fs::read_to_string(at_root("i18n").join(format!("{code}.json"))).unwrap()).unwrap();
        for (key, value) in all.iter().filter(|(k, _)| k.starts_with("page.")) {
            fold(key.as_bytes());
            fold(&[0]);
            fold(value.as_bytes());
            fold(&[0]);
        }
    }
    format!("fnv1a64 {hash:016x}\n")
}

#[test]
fn the_web_bundle_is_built_from_these_sources() {
    assert_eq!(dist("SOURCES"), sources(), "src/web/dist/ 가 web/ 와 갈라졌다 — scripts/gen-web.sh 를 돌려 담는다");
}

/// **페이지 시험이 읽는 자료는 이 바이너리의 답이다**(moai-jshy.hfr) — `scripts/gen-web-fixtures.sh` 로 스크래치에 다시
/// 지어 커밋된 것과 견준다. 손으로 적거나 옛 판이 낸 자료로 초록인 페이지 시험은 moai 가 내는 값을 안 잰다.
#[cfg(unix)]
#[test]
fn the_web_fixtures_match_the_binary() {
    let made = Path::new(env!("CARGO_TARGET_TMPDIR")).join("web-fixtures");
    let _ = std::fs::remove_dir_all(&made);
    // 스크립트가 제 환경(사람·시계·말·git)을 세운다. `BASH_ENV` 만은 여기서 걷는다 — 비대화형 bash 가 스크립트의 첫 줄보다
    // 먼저 그 파일을 돌려, 거기서 내보낸 `MOAI` 가 여기서 준 바이너리를 덮는다(tests/cli.rs `isolated` 와 같은 까닭).
    let out = std::process::Command::new("bash")
        .arg(at_root("scripts/gen-web-fixtures.sh"))
        .arg(&made)
        .env("MOAI", env!("CARGO_BIN_EXE_moai"))
        .env_remove("BASH_ENV")
        .output()
        .expect("bash 를 실행하지 못했다 — 이 시험에는 bash 가 있어야 한다");
    assert!(out.status.success(), "자료를 못 지었다\n{}", String::from_utf8_lossy(&out.stderr));
    let kept = at_root("web/tests/fixtures/moai");
    let mut names: Vec<String> =
        std::fs::read_dir(&made).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
    names.sort();
    assert!(!names.is_empty(), "생성기가 아무것도 안 썼다");
    for name in &names {
        let fresh = std::fs::read_to_string(made.join(name)).unwrap();
        let old = std::fs::read_to_string(kept.join(name)).unwrap_or_default();
        assert!(
            fresh == old,
            "web/tests/fixtures/moai/{name} 가 바이너리의 답과 갈라졌다 — \
             `cargo build && MOAI=target/debug/moai scripts/gen-web-fixtures.sh` 를 돌리고 담는다"
        );
    }
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

/// 서버의 CSP 는 `style-src 'self'` 다(moai-m4ez.h2o). 인라인 스타일을 만드는 길 — 마크업의 `style` 속성,
/// `setAttribute("style", …)`, `style.cssText`(속성을 통째로 갈아 쓴다) — 이 번들에 다시 서면 그 자리가 조용히 안 그려진다.
/// CSSOM 의 `el.style.x = …`·`style.setProperty` 는 CSP 가 막지 않으니 걸지 않는다.
#[test]
fn the_web_bundle_sets_no_inline_style_attribute() {
    let js = dist("app.js");
    for word in ["cssText", "setAttribute(\"style\"", "setAttribute('style'", "style=\"", "style='"] {
        assert!(!js.contains(word), "app.js 에 {word} 가 있다 — CSP 가 style 속성을 막는다. 클래스나 CSSOM 으로 넣는다");
    }
    // `el(doc, tag, { …, style: "…" })` — 속성 모음의 `style:` 키. 꼴은 `style:"`·`style:` + 백틱·`style:` + 변수.
    for (i, _) in js.match_indices("style:") {
        let before = js[..i].chars().last().unwrap_or(' ');
        assert!(
            before.is_ascii_alphanumeric() || before == '_' || before == '.',
            "app.js 에 `style:` 키가 있다(…{}) — el() 의 style 속성은 CSP 가 막는다",
            &js[i.saturating_sub(40)..(i + 40).min(js.len())]
        );
    }
    let html = dist("index.html").to_ascii_lowercase();
    assert!(!html.contains(" style=") && !html.contains("<style"), "index.html 에 인라인 스타일이 있다");
    let css = dist("app.css");
    assert!(!css.contains("@import"), "app.css 가 다른 시트를 부른다 — style-src 'self' 밖일 수 있다");
}

#[test]
fn the_web_bundle_fits_its_budget() {
    let bytes = dist("app.js").len() + dist("app.css").len();
    assert!(bytes <= 1024 * 1024, "app.js+app.css 가 {bytes} 바이트 — 상한 1MiB");
}
