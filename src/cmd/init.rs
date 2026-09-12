//! `.moai/` 를 심는다. `store` 말고 파일을 만드는 유일한 곳이다.

use super::{Ctx, Fail, R};
use crate::config::DEFAULT_STATUSES;
use std::path::Path;

const BEGIN: &str = "<!-- moai:begin -->";
const END: &str = "<!-- moai:end -->";

/// 마커 사이만 갈아 끼운다. 사람이 쓴 산문은 **한 글자도 건드리지 않는다.**
///
/// 남의 파일에 제 것을 쓰는 도구는 이 약속을 지켜야만 신뢰를 얻는다.
fn with_block(existing: &str, block: &str) -> String {
    let body = format!("{BEGIN}
{block}{END}
");
    match (existing.find(BEGIN), existing.find(END)) {
        (Some(a), Some(b)) if b > a => {
            let tail = &existing[b + END.len()..];
            format!("{}{body}{}", &existing[..a], tail.strip_prefix('\n').unwrap_or(tail))
        }
        _ if existing.trim().is_empty() => body,
        _ => {
            let mut out = existing.to_string();
            if !out.ends_with('\n') {
                out.push('\n');
            }
            out.push('\n');
            out.push_str(&body);
            out
        }
    }
}

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

pub fn run(ctx: &Ctx, prefix: Option<&str>, no_agents: bool) -> R<Vec<String>> {
    let root = std::env::current_dir().map_err(|e| Fail::new(e.to_string()))?;
    let dir = root.join(".moai");

    // 이미 심긴 곳에서 다시 부르면 **딸린 파일만 다시 맞춘다.**
    //
    // 도구가 자라면 `AGENTS.md` 블록은 반드시 낡는다. 그걸 다시 쓸 길이
    // 없으면 새 세션의 에이전트가 없는 명령을 쓰고 있는 명령을 모른다.
    // 명령을 하나 더 만드는 대신 `init` 이 그 일을 맡는다 — 이슈와 저널은
    // 손대지 않으므로 다시 불러도 잃을 것이 없다.
    let again = dir.exists();
    let prefix = match (prefix, again) {
        // 접두어는 나중에 못 바꾼다. 이미 발급된 id 가 전부 그것을 달고 있고,
        // 바꾸면 그 줄들이 제 접두어를 잃는다.
        (Some(p), true) => {
            let cur = crate::config::Config::load(&root).map_err(Fail::new)?.prefix;
            if p != cur {
                return Err(Fail::coded(
                    format!(
                        "접두어는 `{cur}` 로 이미 정해졌다. 나중에 못 바꾼다 —\n      \
                         이미 발급된 id 가 전부 그것을 달고 있다"
                    ),
                    super::code::ALREADY_EXISTS,
                ));
            }
            cur
        }
        (Some(p), false) => p.to_string(),
        (None, true) => crate::config::Config::load(&root).map_err(Fail::new)?.prefix,
        (None, false) => prefix_from(&root)
            .ok_or_else(|| Fail::new("디렉터리 이름에서 접두어를 만들 수 없다. `moai init <접두어>`"))?,
    };

    let config = format!(
        "# moai — {}\nprefix = \"{prefix}\"\nstatuses = \"{DEFAULT_STATUSES}\"\n\
         # 화면이 사람을 내는 모양: full(`이름 (메일)`) · name · email\nnaming = \"full\"\n",
        "이 저장소의 이슈 트래커 설정"
    );
    // 설정을 먼저 검사한다 — 접두어가 형식에 안 맞으면 파일을 만들기 전에 멈춘다.
    crate::config::Config::parse(&config).map_err(Fail::new)?;

    if !again {
        std::fs::create_dir_all(&dir)
            .map_err(|e| Fail::new(format!("{}: {e}", dir.display())))?;
        for (name, body) in [
            ("config.toml", config.as_str()),
            ("issues.jsonl", ""),
            ("journal.jsonl", ""),
        ] {
            let p = dir.join(name);
            std::fs::write(&p, body).map_err(|e| Fail::new(format!("{}: {e}", p.display())))?;
        }
    }

    let attrs = ensure_lines(&root.join(".gitattributes"), GITATTRIBUTES).map_err(Fail::new)?;
    let ignore = ensure_lines(&root.join(".gitignore"), GITIGNORE).map_err(Fail::new)?;

    // `AGENTS.md` **하나만** 쓴다. `CLAUDE.md` 에도 같은 것을 쓰면 곧 갈라지고,
    // 갈라진 두 벌 중 어느 것이 참인지 아무도 모른다.
    let agents_path = root.join("AGENTS.md");
    let agents = if no_agents {
        false
    } else {
        let existing = std::fs::read_to_string(&agents_path).unwrap_or_default();
        let next = with_block(&existing, &crate::guide::agents());
        if next != existing {
            std::fs::write(&agents_path, next)
                .map_err(|e| Fail::new(format!("{}: {e}", agents_path.display())))?;
        }
        true
    };
    let claude_needs_pointer = agents
        && root.join("CLAUDE.md").exists()
        && !std::fs::read_to_string(root.join("CLAUDE.md"))
            .unwrap_or_default()
            .contains("AGENTS.md");

    if ctx.json {
        return super::json_line(&serde_json::json!({
            "root": root.display().to_string(),
            "prefix": prefix,
            "created": !again,
            "gitattributes": attrs,
            "gitignore": ignore,
            "agents": agents,
        }));
    }

    let mut out = if again {
        vec![format!("이미 심겨 있다. 접두어는 `{prefix}` 다 — 딸린 파일만 다시 맞춘다")]
    } else {
        vec![
            format!(".moai/ 를 만들었다. 접두어는 `{prefix}` 다"),
            format!("  칸: {}", DEFAULT_STATUSES.replace(',', " → ")),
        ]
    };
    if attrs {
        out.push("  .gitattributes 에 병합 규칙을 넣었다".into());
    }
    if ignore {
        out.push("  .gitignore 에 lock·tmp 를 넣었다".into());
    }
    if agents {
        out.push("  AGENTS.md 블록을 맞췄다".into());
    }
    if again && out.len() == 1 {
        out.push("  이미 다 맞아 있다".into());
    }
    if claude_needs_pointer {
        out.push(String::new());
        out.push("CLAUDE.md 가 있다. 그 안에 `@AGENTS.md` 한 줄을 넣으면 같이 읽힌다".into());
    }
    if !again {
        out.push(String::new());
        out.push("다음:  moai add \"첫 이슈\"".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 두 번 넣어도 블록은 하나고, 사람이 쓴 산문은 바이트 단위로 그대로다.
    #[test]
    fn the_block_is_replaced_never_repeated() {
        let mine = "# 우리 규약\n\n손으로 쓴 것.\n";
        let once = with_block(mine, "옛 내용\n");
        assert!(once.starts_with(mine), "{once:?}");
        assert_eq!(once.matches(BEGIN).count(), 1);

        let twice = with_block(&once, "새 내용\n");
        assert_eq!(twice.matches(BEGIN).count(), 1, "{twice:?}");
        assert!(twice.contains("새 내용") && !twice.contains("옛 내용"), "{twice:?}");
        assert!(twice.starts_with(mine), "산문을 건드렸다 — {twice:?}");

        // 한 번 더 넣어도 더는 안 바뀐다
        assert_eq!(with_block(&twice, "새 내용\n"), twice);
    }

    #[test]
    fn an_empty_file_gets_just_the_block() {
        let got = with_block("", "내용\n");
        assert_eq!(got, format!("{BEGIN}\n내용\n{END}\n"));
    }

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
