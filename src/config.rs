//! `.moai/config.toml` 의 아주 작은 부분집합 파서와 설정 값.
//!
//! 중첩도 배열도 쓰지 않으므로 TOML 라이브러리를 넣지 않는다.
//! **엄격하게 읽는다** — 설정 오타가 조용히 통과하면 id 접두어가 틀어지거나
//! 상태 목록이 비고, 둘 다 되돌리기 어렵다.

use std::path::Path;

/// 상태 목록을 안 적었을 때의 칸반 컬럼.
pub const DEFAULT_STATUSES: &str = "todo,in_progress,review,done";

/// 종결 상태. **설정으로 바꿀 수 없다.**
///
/// 코드가 상태에 묻는 질문은 사실상 이것 하나뿐이라(진행률·열린 이슈 수·방치
/// 검사) beads 식 상태 카테고리 표를 두지 않는다. 어휘가 둘이 되면 둘을 계속
/// 맞춰야 한다. 둘째 종결 상태가 실제로 필요해지면 그때 목록 하나를 더한다.
pub const DONE: &str = "done";

/// 따옴표 **밖의** `#` 부터가 주석이다. 값 안의 `#` 을 자르면
/// `title = "규칙 #1"` 이 조용히 깨진다.
///
/// 따옴표가 짝이 안 맞으면 어디까지가 값인지 알 수 없다. 세어서 넘기지 말고 거절한다.
fn strip_comment(line: &str, n: usize) -> Result<&str, String> {
    let mut quoted = false;
    let mut cut = line.len();
    for (i, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '#' if !quoted => {
                cut = i;
                break;
            }
            _ => {}
        }
    }
    if quoted {
        return Err(format!("{n}줄: 따옴표가 짝이 맞지 않는다"));
    }
    Ok(&line[..cut])
}

/// 값은 반드시 큰따옴표로 감싼다. `trim_matches` 로 벗기면
/// `prefix = "argos" 오타` 가 `argos" 오타` 로 조용히 통과한다.
fn value(raw: &str) -> Option<&str> {
    let inner = raw.trim().strip_prefix('"')?.strip_suffix('"')?;
    (!inner.contains('"')).then_some(inner)
}

/// 최상위 `키 = "값"` 하나를 읽는다.
pub fn scalar(src: &str, key: &str) -> Result<Option<String>, String> {
    let src = src.strip_prefix('\u{feff}').unwrap_or(src); // BOM
    for (i, line) in src.lines().enumerate() {
        let n = i + 1;
        let l = strip_comment(line, n)?.trim();
        if l.is_empty() || l.starts_with('[') {
            continue;
        }
        let (k, v) = l
            .split_once('=')
            .ok_or_else(|| format!("{n}줄: `키 = \"값\"` 형식이 아니다"))?;
        if k.trim() != key {
            continue;
        }
        let v = value(v)
            .ok_or_else(|| format!("{n}줄: `{key}` 의 값은 큰따옴표로 감싸야 한다 — {v:?}"))?;
        return Ok(Some(v.to_string()));
    }
    Ok(None)
}

#[derive(Debug, Clone)]
pub struct Config {
    /// id 접두어. 그 저장소의 프로젝트명이다.
    pub prefix: String,
    /// 칸반 컬럼. 적은 순서가 곧 보드의 순서다.
    pub statuses: Vec<String>,
}

impl Config {
    pub fn load(root: &Path) -> Result<Config, String> {
        let path = root.join(".moai/config.toml");
        let src = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Config::parse(&src).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn parse(src: &str) -> Result<Config, String> {
        let prefix = scalar(src, "prefix")?
            .filter(|p| !p.is_empty())
            .ok_or("`prefix` 가 없다")?;
        if !prefix
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        {
            return Err(format!("`prefix` 는 소문자·숫자·`-` 만 쓴다 — {prefix:?}"));
        }
        if prefix.starts_with('-') || prefix.ends_with('-') {
            return Err(format!("`prefix` 는 `-` 로 시작하거나 끝날 수 없다 — {prefix:?}"));
        }

        let raw = scalar(src, "statuses")?.unwrap_or_else(|| DEFAULT_STATUSES.into());
        let statuses: Vec<String> = raw
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if statuses.is_empty() {
            return Err("`statuses` 가 비었다".into());
        }
        if !statuses.iter().any(|s| s == DONE) {
            return Err(format!("`statuses` 에 `{DONE}` 이 있어야 한다 — {raw:?}"));
        }
        if let Some(dup) = statuses
            .iter()
            .enumerate()
            .find(|(i, s)| statuses[..*i].contains(s))
        {
            return Err(format!("`statuses` 에 `{}` 가 두 번 있다", dup.1));
        }

        Ok(Config { prefix, statuses })
    }

    /// 새 이슈가 놓이는 칸. 목록의 첫 칸이다.
    pub fn first_status(&self) -> &str {
        &self.statuses[0]
    }

    pub fn knows(&self, status: &str) -> bool {
        self.statuses.iter().any(|s| s == status)
    }

    /// 모르는 칸을 거부한다. **문장이 여기 하나다** — `add`·`mv`·`show` 와
    /// `Issue::validate` 가 저마다 같은 말을 짓고 있으면 반드시 갈라진다.
    pub fn require_known(&self, status: &str) -> Result<(), String> {
        if self.knows(status) {
            return Ok(());
        }
        Err(format!("`{status}` 라는 칸이 없다. 있는 칸: {}", self.statuses.join(", ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_quoted_scalar() {
        assert_eq!(scalar("prefix = \"argos\"\n", "prefix").unwrap().as_deref(), Some("argos"));
        assert_eq!(scalar("\u{feff}prefix = \"argos\"\n", "prefix").unwrap().as_deref(), Some("argos"));
        assert_eq!(
            scalar("# 주석\nprefix = \"ai-argos\"  # 뒤 주석\n", "prefix").unwrap().as_deref(),
            Some("ai-argos")
        );
        assert_eq!(scalar("other = \"x\"\n", "prefix").unwrap(), None);
    }

    /// 값 안의 `#` 은 주석이 아니다.
    #[test]
    fn hash_inside_quotes_survives() {
        assert_eq!(scalar("t = \"규칙 #1\"\n", "t").unwrap().as_deref(), Some("규칙 #1"));
    }

    #[test]
    fn refuses_sloppy_values() {
        for (src, want) in [
            ("prefix = argos\n", "큰따옴표"),
            ("prefix = \"argos\" 오타\n", "큰따옴표"),
            ("prefix = \"argos\n", "짝이 맞지"),
            ("prefix\n", "형식이 아니다"),
        ] {
            let e = scalar(src, "prefix").unwrap_err();
            assert!(e.contains(want), "{src:?} → {e:?}");
        }
    }

    #[test]
    fn defaults_the_kanban_columns() {
        let c = Config::parse("prefix = \"argos\"\n").unwrap();
        assert_eq!(c.statuses, ["todo", "in_progress", "review", "done"]);
        assert_eq!(c.first_status(), "todo");
        assert!(c.knows("done") && !c.knows("blocked"));
    }

    #[test]
    fn extra_columns_are_allowed() {
        let c = Config::parse("prefix = \"a\"\nstatuses = \"todo, blocked, done\"\n").unwrap();
        assert_eq!(c.statuses, ["todo", "blocked", "done"]);
    }

    #[test]
    fn refuses_broken_config() {
        for (src, want) in [
            ("statuses = \"todo,done\"\n", "`prefix` 가 없다"),
            ("prefix = \"Argos\"\n", "소문자"),
            ("prefix = \"-a\"\n", "`-` 로 시작"),
            ("prefix = \"a\"\nstatuses = \"todo,review\"\n", "`done` 이 있어야"),
            ("prefix = \"a\"\nstatuses = \" , \"\n", "비었다"),
            ("prefix = \"a\"\nstatuses = \"todo,todo,done\"\n", "두 번"),
        ] {
            let e = Config::parse(src).unwrap_err();
            assert!(e.contains(want), "{src:?} → {e:?}");
        }
    }
}
