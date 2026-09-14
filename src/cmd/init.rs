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

// **워크트리 자리도 막는다**(moai-mxtb, 사용자와 정함). 감독 일꾼 절차가 `.claude/worktrees/` 에
// 워크트리를 뜨는데, 그 자리가 안 막히면 `git add -A` 에 남의 가지 전체가 딸려 온다. 넣는 것은
// 그 자리 하나다 — `.claude/` 통째는 저장소가 커밋하는 설정·스킬·플러그인을 가린다. 끄는 길은
// 두지 않는다: 워크트리를 안 쓰는 저장소에는 빈 자리를 막는 줄일 뿐이다.
const GITIGNORE: &str = "\
# moai
.moai/lock
.moai/*.tmp.*
/.claude/worktrees/
";

/// **새로 심는 접두어의 최대 길이**(moai-f7xs). id 는 `<접두어>-<4자>` 이고 사람과
/// 에이전트가 명령마다 친다 — 접두어가 길면 그만큼 매번 손이 늘고, 목록·트리·탐색기의 id
/// 열이 넓어져 제목 몫이 준다. 8자면 id 13자·자식 id 17자이고, `backend`·`frontend`
/// 같은 흔한 한 낱말이 그대로 들어간다.
///
/// **검사는 새로 심을 때만 한다.** `Config::parse` 에 두면 이미 긴 접두어로 심긴 저장소가
/// 통째로 안 열린다(읽기는 관대하게) — 접두어는 나중에 못 바꾸는 값이라 알려 봐야 고칠
/// 길도 없다.
pub const PREFIX_MAX: usize = 8;

/// 긴 접두어의 짧은 후보. **모양이 맞는 접두어만 받는다**(`config::check_prefix`) — 그래야
/// 내는 것도 모양이 맞는다. [`PREFIX_MAX`] 이하면 그대로, 넘으면 뜻을 더 남기는 것부터:
/// 1. 하이픈을 빼서 들어가면 그것 — `moa-issue` → `moaissue`. 머리글자(`mi`)는 알아보기
///    어렵고 접두어는 나중에 못 바꾼다(리뷰 moai-f7xs.z1x, 사용자와 정함)
/// 2. 낱말이 둘 이상이면 머리글자 — `my-company-backend` → `mcb`
/// 3. 낱말이 하나면 앞에서 [`PREFIX_MAX`] 자
fn shorten(prefix: &str) -> String {
    if prefix.chars().count() <= PREFIX_MAX {
        return prefix.to_string();
    }
    let words: Vec<&str> = prefix.split('-').filter(|w| !w.is_empty()).collect();
    let joined: String = words.concat();
    if words.len() >= 2 && joined.chars().count() <= PREFIX_MAX {
        joined
    } else if words.len() >= 2 {
        words.iter().filter_map(|w| w.chars().next()).take(PREFIX_MAX).collect()
    } else {
        prefix.chars().take(PREFIX_MAX).collect()
    }
}

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
        .filter(|l| !l.trim().is_empty() && !existing.lines().any(|e| covers(e, l)))
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

/// 이미 있는 줄 `have` 가 넣으려는 줄 `want` 를 **이미 막고 있는가**(moai-mxtb).
///
/// 글자가 같은 줄만 보면, `/.claude/worktrees` 를 이미 적은 저장소에 효과 없는
/// `/.claude/worktrees/` 가 하나 더 붙는다. 그래서 앞뒤 `/` 를 떼고 견주고, 윗 디렉터리를
/// 통째로 막은 줄(`.claude/`)도 그 밑의 줄을 막은 것으로 친다. **남의 줄은 안 바꾼다** —
/// 판정만 넓히고 쓰는 것은 빠진 줄을 덧붙이는 것뿐이다. 주석·빈 줄·`!` 되살림은 넓혀
/// 읽지 않는다.
fn covers(have: &str, want: &str) -> bool {
    let (have, want) = (have.trim(), want.trim());
    if have == want {
        return true;
    }
    if have.is_empty() || have.starts_with('#') || have.starts_with('!') || want.starts_with('#') {
        return false;
    }
    // 끝 `/` 는 "디렉터리만" 이라는 뜻이다. 같은 이름끼리 견줄 때 `have` 만 디렉터리 전용이면
    // (`.moai/lock/`) 파일 `.moai/lock` 을 막지 못하니 덮은 것으로 치지 않는다.
    let dir_only = have.ends_with('/') && !want.ends_with('/');
    let bare = |s: &str| s.trim_start_matches('/').trim_end_matches('/').to_string();
    let (have, want) = (bare(have), bare(want));
    !have.is_empty() && ((have == want && !dir_only) || want.starts_with(&format!("{have}/")))
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
    // 디렉터리 이름이 길어 줄였으면 그 원래 모양 — 무엇에서 줄였는지 말하려고 든다.
    let mut shortened: Option<String> = None;
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
        // **사람이 준 긴 접두어는 거절한다** — 쓰기는 엄하게. `init` 은 한 번 부르는 명령이라
        // 다시 부르는 비용이 작고, 거절문이 짧은 후보를 댄다. 이미 심긴 저장소의 긴 접두어는
        // 위 갈래가 그대로 받는다.
        // **모양을 먼저 본다** — 길이를 먼저 보면 `MyCompanyBackend` 에 그 자체로 틀린
        // `MyCompan` 을 후보로 대, 따라 친 쪽이 둘째 오류를 만났다(리뷰 moai-f7xs.z1x).
        // 후보는 명령줄이 아니라 접두어만 댄다 — `-C`·`--no-agents` 를 줬던 명령을 다시 짜서
        // 대면 붙여 넣은 자리에 엉뚱하게 심는다.
        (Some(p), false) => {
            crate::config::check_prefix(p).map_err(Fail::new)?;
            if p.chars().count() > PREFIX_MAX {
                return Err(Fail::coded(
                    format!(
                        "접두어는 {PREFIX_MAX}자까지다 — `{p}` 는 {}자다. id 를 칠 때마다 붙는다\n      \
                         짧은 후보: `{}`",
                        p.chars().count(),
                        shorten(p)
                    ),
                    super::code::BAD_INPUT,
                ));
            }
            p.to_string()
        }
        (None, true) => crate::config::Config::load(&root).map_err(Fail::new)?.prefix,
        // **디렉터리 이름에서 만든 것은 줄여서 쓴다** — 사람이 고른 이름이 아니라 거절할
        // 까닭이 없다. 줄였다는 것은 출력이 말한다.
        (None, false) => {
            let full = prefix_from(&root)
                .ok_or_else(|| Fail::new("디렉터리 이름에서 접두어를 만들 수 없다. `moai init <접두어>`"))?;
            let short = shorten(&full);
            if short != full {
                shortened = Some(full);
            }
            short
        }
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
        let mut v = serde_json::json!({
            "root": root.display().to_string(),
            "prefix": prefix,
            "created": !again,
            "gitattributes": attrs,
            "gitignore": ignore,
            "agents": agents,
        });
        // 줄였을 때만 싣는다 — 늘 `null` 을 두면 줄이지 않은 대부분의 줄이 헛 키를 든다.
        if let Some(full) = &shortened {
            v["shortened_from"] = serde_json::json!(full);
        }
        return super::json_line(&v);
    }

    let mut out = if again {
        vec![format!("이미 심겨 있다. 접두어는 `{prefix}` 다 — 딸린 파일만 다시 맞춘다")]
    } else {
        vec![
            format!(".moai/ 를 만들었다. 접두어는 `{prefix}` 다"),
            format!("  칸: {}", DEFAULT_STATUSES.replace(',', " → ")),
        ]
    };
    // 접두어는 나중에 못 바꾸므로 **지금** 말한다 — 이슈를 하나라도 만들면 되돌릴 길이 없다.
    if let Some(full) = &shortened {
        out.insert(
            1,
            format!(
                "  디렉터리 이름 `{full}` 이 {PREFIX_MAX}자를 넘어 줄였다 — 다른 것을 원하면 이슈를 만들기 전에 \
                 .moai/ 를 지우고 `moai init <접두어>`"
            ),
        );
    }
    if attrs {
        out.push("  .gitattributes 에 병합 규칙을 넣었다".into());
    }
    if ignore {
        out.push("  .gitignore 에 moai 가 쓰는 자리(lock·tmp·워크트리)를 넣었다".into());
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

    /// **커밋된 AGENTS.md 블록이 지금의 글과 같다.** `guide.rs` 만 고치고
    /// `moai init` 을 안 부르면 이 저장소의 에이전트가 옛 글을 배운다 —
    /// 탐색기가 idea 를 담게 된 뒤에도 "읽기 전용" 이라 적혀 있었다(`moai-ka9p`).
    /// 블록 밖의 산문은 사람의 것이라 보지 않는다.
    #[test]
    fn the_checked_in_agents_block_matches_the_guide() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("AGENTS.md");
        let existing = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!(
            with_block(&existing, &crate::guide::agents()) == existing,
            "AGENTS.md 블록이 guide.rs 의 글에서 낡았다 — `moai init` 을 다시 부른다"
        );
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

    /// **긴 접두어는 하이픈을 빼 들어가면 그것, 아니면 머리글자, 낱말이 하나면 앞 8자로
    /// 줄인다**(moai-f7xs). 8자 이하는 그대로.
    #[test]
    fn a_long_prefix_is_shortened_to_initials_or_cut() {
        for (full, want) in [
            ("argos", "argos"),
            ("backend8", "backend8"),
            ("moa-issue", "moaissue"),
            ("my-company-backend", "mcb"),
            ("my-2nd-project-x", "m2px"),
            ("2024-plan-final-draft", "2pfd"),
            ("supercalifragilistic", "supercal"),
            ("a-b-c-d-e-f-g-h-i-j", "abcdefgh"),
        ] {
            let got = shorten(full);
            assert_eq!(got, want, "{full}");
            assert!(got.chars().count() <= PREFIX_MAX, "{full} → {got}");
            // 줄인 것도 설정이 받는 접두어다.
            crate::config::Config::parse(&format!("prefix = \"{got}\"\n")).unwrap_or_else(|e| panic!("{full} → {got}: {e}"));
        }
    }
}
