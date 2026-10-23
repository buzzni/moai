//! 탐색기의 키 표를 보인다(moai-so7w) — 사람의 키 파일(`keys.toml`)이 바꾼 것과 버린 줄. 읽기만 한다: 트래커를 안
//! 찾고 파일을 안 쓴다. 빈 틀(`--template`)도 표준 출력으로만 낸다 — 받아 두는 것은 사람이다.

use super::{Ctx, R};
use crate::cli::KeysArgs;
use crate::tui::keymap;

pub fn run(ctx: &Ctx, args: KeysArgs) -> R<Vec<String>> {
    if args.template {
        let template = keymap::template();
        if ctx.json {
            #[derive(serde::Serialize)]
            struct Out {
                template: String,
            }
            return super::json_line(&Out { template });
        }
        return Ok(template.lines().map(str::to_string).collect());
    }
    let lang = ctx.lang();
    let (file, keys, problems) = keymap::load_from(|k| std::env::var_os(k));
    // 서 있는가만 본다 — 디렉터리처럼 못 읽는 것은 "없다" 가 아니라 stderr 의 못 읽은 까닭이 댄다(리뷰 13번).
    let found = file.as_deref().is_some_and(std::path::Path::exists);
    // **버린 줄은 stderr 로, 종료 0 으로** — `moai models` 와 같다. 성한 줄은 산다.
    for p in &problems {
        super::tell(&p.said(file.as_deref(), lang));
    }
    let listed = keymap::listing(&keys);
    if ctx.json {
        #[derive(serde::Serialize)]
        struct Problem<'a> {
            at: &'a str,
            line: Option<usize>,
            kind: &'static str,
            said: String,
        }
        #[derive(serde::Serialize)]
        struct Row<'a> {
            #[serde(flatten)]
            listed: &'a keymap::Listed,
            changed: bool,
        }
        #[derive(serde::Serialize)]
        struct Out<'a> {
            path: Option<&'a std::path::Path>,
            found: bool,
            problems: Vec<Problem<'a>>,
            keys: Vec<Row<'a>>,
        }
        return super::json_line(&Out {
            path: file.as_deref(),
            found,
            problems: problems
                .iter()
                .map(|p| Problem { at: &p.at, line: p.line, kind: p.why.kind(), said: p.said(file.as_deref(), lang) })
                .collect(),
            keys: listed.iter().map(|l| Row { listed: l, changed: l.changed() }).collect(),
        });
    }
    Ok(crate::view::keys(file.as_deref(), found, &listed, lang))
}
