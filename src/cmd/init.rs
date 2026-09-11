//! `.moai/` 를 심는다. `store` 말고 파일을 만드는 유일한 곳이다.

use super::{Ctx, Fail, R};
use crate::config::DEFAULT_STATUSES;
use std::path::Path;

const GITATTRIBUTES: &str = "\
# moai — 이슈 트래커
# 스냅샷에는 merge=union 을 쓰지 않는다. 두 브랜치가 같은 이슈를 고치면
# union 이 같은 id 를 가진 줄 두 개를 조용히 남기고, 그건 데이터 손상이다.
# 진짜 충돌은 사람이 푼다 — 한 줄이 이슈 하나라 실제로 쉽다.
.moai/issues.jsonl   text eol=lf
# 저널은 추가 전용이고 순서가 무관하며 상태 계산에 읽히지 않는다. 여기선 맞다.
.moai/journal.jsonl  text eol=lf merge=union
";

const GITIGNORE: &str = "\
# moai
.moai/lock
.moai/*.tmp.*
";

/// 디렉터리 이름에서 접두어를 만든다. 소문자·숫자·`-` 만 남긴다.
fn prefix_from(dir: &Path) -> Option<String> {
    let name = dir.file_name()?.to_str()?.to_ascii_lowercase();
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').to_string();
    (!out.is_empty()).then_some(out)
}

/// 이미 있는 파일에는 **빠진 줄만** 덧붙인다. 남의 내용을 지우지 않는다.
fn ensure_lines(path: &Path, block: &str) -> Result<bool, String> {
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let missing: Vec<&str> = block
        .lines()
        .filter(|l| !l.trim().is_empty() && !existing.lines().any(|e| e.trim() == l.trim()))
        .collect();
    if missing.is_empty() {
        return Ok(false);
    }
    let mut out = existing;
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(&missing.join("\n"));
    out.push('\n');
    std::fs::write(path, out).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(true)
}

pub fn run(ctx: &Ctx, prefix: Option<&str>) -> R<Vec<String>> {
    let root = std::env::current_dir().map_err(|e| Fail::new(e.to_string()))?;
    let dir = root.join(".moai");
    if dir.exists() {
        return Err(Fail::coded(
            format!("{} 가 이미 있다", dir.display()),
            "already_exists",
        ));
    }

    let prefix = match prefix {
        Some(p) => p.to_string(),
        None => prefix_from(&root)
            .ok_or_else(|| Fail::new("디렉터리 이름에서 접두어를 만들 수 없다. `moai init <접두어>`"))?,
    };

    let config = format!(
        "# moai — {}\nprefix = \"{prefix}\"\nstatuses = \"{DEFAULT_STATUSES}\"\n",
        "이 저장소의 이슈 트래커 설정"
    );
    // 설정을 먼저 검사한다 — 접두어가 형식에 안 맞으면 파일을 만들기 전에 멈춘다.
    crate::config::Config::parse(&config).map_err(Fail::new)?;

    std::fs::create_dir_all(&dir).map_err(|e| Fail::new(format!("{}: {e}", dir.display())))?;
    for (name, body) in [
        ("config.toml", config.as_str()),
        ("issues.jsonl", ""),
        ("journal.jsonl", ""),
    ] {
        let p = dir.join(name);
        std::fs::write(&p, body).map_err(|e| Fail::new(format!("{}: {e}", p.display())))?;
    }

    let attrs = ensure_lines(&root.join(".gitattributes"), GITATTRIBUTES).map_err(Fail::new)?;
    let ignore = ensure_lines(&root.join(".gitignore"), GITIGNORE).map_err(Fail::new)?;

    if ctx.json {
        return super::json_line(&serde_json::json!({
            "root": root.display().to_string(),
            "prefix": prefix,
            "gitattributes": attrs,
            "gitignore": ignore,
        }));
    }

    let mut out = vec![
        format!(".moai/ 를 만들었다. 접두어는 `{prefix}` 다"),
        format!("  칸: {}", DEFAULT_STATUSES.replace(',', " → ")),
    ];
    if attrs {
        out.push("  .gitattributes 에 병합 규칙을 넣었다".into());
    }
    if ignore {
        out.push("  .gitignore 에 lock·tmp 를 넣었다".into());
    }
    out.push(String::new());
    out.push("다음:  moai add \"첫 이슈\"".into());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn makes_a_prefix_from_a_directory_name() {
        for (dir, want) in [
            ("/w/argos", Some("argos")),
            ("/w/moa-issue", Some("moa-issue")),
            ("/w/My_Project 2", Some("my-project-2")),
            ("/w/___", None),
        ] {
            assert_eq!(prefix_from(Path::new(dir)).as_deref(), want, "{dir}");
        }
    }
}
