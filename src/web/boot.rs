//! `/api/boot` — 페이지가 처음 한 번 읽는 바탕: 말·판·칸·글리프·거르개 낱말·정렬 필드.
//!
//! **`glyphs` 는 설정의 칸만 싣는다**(moai-jshy.yxh). backlog·미룸 칸과 모르는 칸의 글리프를 같은 맵에 두던 판은
//! `statuses = "backlog,todo,shelved,done"` 에서 키가 둘씩 서서 `JSON.parse` 가 뒤의 것을 남겼다. 설정의 칸이 아닌
//! 둘은 `columns` 에 이름과 함께, 모르는 칸은 `unknown_glyph` 로 선다.
//!
//! **요청마다 설정을 다시 읽는다**([`crate::store::Repo::find_from`]) — 서버를 띄운 뒤 `.moai/config.toml` 의
//! 칸을 고친 사람이 새로 고침 한 번으로 그것을 봐야 한다. 서버를 다시 띄워야 서는 값이면 그것은 캐시다.

use crate::i18n::{Lang, say};
use crate::query::{Dir, SortKey};
use crate::style;
use serde::Serialize;
use std::path::Path;

/// 키 차례를 지킨 맵 — 글리프는 설정의 칸 차례로 선다. `serde_json` 의 `Map` 은 키로 늘어놓는다.
struct Ordered<'a>(Vec<(&'a str, &'a str)>);

impl Serialize for Ordered<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut m = s.serialize_map(Some(self.0.len()))?;
        for (k, v) in &self.0 {
            m.serialize_entry(k, v)?;
        }
        m.end()
    }
}

/// 보드에만 서는 칸 하나 — 설정의 칸이 아니라서 이름도 화면의 말이다.
#[derive(Serialize)]
struct Column {
    name: &'static str,
    glyph: &'static str,
}

#[derive(Serialize)]
struct Columns {
    backlog: Column,
    shelved: Column,
}

#[derive(Serialize)]
struct Sort {
    name: &'static str,
    dir: &'static str,
}

#[derive(Serialize)]
struct Boot<'a> {
    lang: &'static str,
    version: &'static str,
    project: String,
    root: String,
    statuses: &'a [String],
    naming: &'static str,
    glyphs: Ordered<'a>,
    unknown_glyph: &'static str,
    columns: Columns,
    filter_keys: &'static [&'static str],
    sort_fields: Vec<Sort>,
}

/// `cwd`(자식이 도는 그 자리, [`super::run::Spawner::cwd`])에서 트래커를 찾아 바탕을 짓는다. 못 찾으면 CLI 가 내는
/// 그 거절이다.
pub fn boot(cwd: &Path, lang: Lang) -> crate::fail::R<String> {
    let repo = crate::store::Repo::find_from(cwd, || lang)?
        .ok_or_else(|| crate::fail::Fail::new(say(lang, "refuse.not_a_repo")))?;
    let statuses = &repo.config.statuses;
    let glyphs: Vec<(&str, &str)> = statuses.iter().map(|s| (s.as_str(), style::glyph(s))).collect();
    let boot = Boot {
        lang: lang.code(),
        version: env!("CARGO_PKG_VERSION"),
        project: repo.root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        root: repo.root.display().to_string(),
        statuses,
        naming: crate::cmd::config::naming(repo.config.naming),
        glyphs: Ordered(glyphs),
        unknown_glyph: style::glyph(""),
        columns: Columns {
            backlog: Column { name: say(lang, "tui.board.backlog"), glyph: style::BACKLOG_GLYPH },
            shelved: Column { name: say(lang, "tui.act.deferred"), glyph: style::SHELVED_GLYPH },
        },
        filter_keys: crate::query::KEYS,
        sort_fields: SortKey::ALL
            .into_iter()
            .map(|k| Sort {
                name: k.name(),
                dir: match k.default_dir() {
                    Dir::Asc => "asc",
                    Dir::Desc => "desc",
                },
            })
            .collect(),
    };
    serde_json::to_string(&boot).map_err(|e| crate::fail::Fail::new(e.to_string()))
}
