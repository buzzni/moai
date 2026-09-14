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

/// 사람을 어떻게 낼까. `레이븐 (raven@buzzni.com)` 은 22칸이라 좁은 화면에서
/// 줄을 다 먹는다. 무엇이 읽기 좋은지는 저장소마다 다르고, 그 판단은 도구가
/// 아니라 그 저장소가 한다.
///
/// **저장은 이것과 무관하다.** 파일에는 언제나 이름과 메일이 갈라져 들어간다 —
/// 표기를 바꿨다고 이미 쓴 줄이 달라지면 그건 설정이 아니라 마이그레이션이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Naming {
    /// `이름 (메일)`
    #[default]
    Full,
    /// `이름`
    Name,
    /// `메일`. 메일을 모르는 사람은 이름으로 낸다 — 빈칸을 내면 그 줄이
    /// 누구의 것인지 화면에서 사라진다.
    Email,
}

impl Naming {
    const ALL: [&'static str; 3] = ["full", "name", "email"];

    fn parse(raw: &str) -> Option<Naming> {
        match raw {
            "full" => Some(Naming::Full),
            "name" => Some(Naming::Name),
            "email" => Some(Naming::Email),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    /// id 접두어. 그 저장소의 프로젝트명이다.
    pub prefix: String,
    /// 칸반 컬럼. 적은 순서가 곧 보드의 순서다.
    pub statuses: Vec<String>,
    /// 화면이 사람을 내는 모양. 파일에 쓰는 모양이 아니다.
    pub naming: Naming,
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

        let naming = match scalar(src, "naming")? {
            None => Naming::default(),
            Some(raw) => Naming::parse(&raw).ok_or_else(|| {
                format!("`naming` 은 {} 중 하나다 — {raw:?}", Naming::ALL.join("·"))
            })?,
        };

        Ok(Config { prefix, statuses, naming })
    }

    /// 새 이슈가 놓이는 칸. 목록의 첫 칸이다.
    pub fn first_status(&self) -> &str {
        &self.statuses[0]
    }

    /// 시작했으나 안 끝난 칸인가 — 첫 칸도 `done` 도 아니다. **"시작했다" 의 뜻은 여기
    /// 하나다**(moai-p415). `wip`·`wip_overload`·훅의 집기 판단·묶음이 저마다 "첫 칸도 done
    /// 도 아니다" 를 적고 있었고, 묶음만 [`Config::started_status`] 로 **자리를** 골라 칸이 더
    /// 있는 설정에서 멤버가 `in_progress` 인 에픽이 `blocked` 로 읽혔다.
    ///
    /// 설정에 없는 칸도 시작한 것으로 센다 — 읽기는 관대하다. 칸을 알아야 하는 쪽(훅)은
    /// [`Config::knows`] 를 따로 묻는다.
    pub fn is_started(&self, status: &str) -> bool {
        status != self.first_status() && status != DONE
    }

    /// 시작한 것이 놓이는 칸 — 첫 칸도 끝난 칸도 아닌 첫 칸. 그런 칸이 없는
    /// 두 칸짜리 설정이면 첫 칸이다: 거기서는 "시작했지만 안 끝났다" 를 말할
    /// 낱말이 없고, `done` 으로 말하면 안 끝난 것을 끝났다고 한다.
    ///
    /// **"시작했는가" 를 묻는 데 쓰지 않는다** — 그것은 [`Config::is_started`] 다. 이 칸은
    /// 묶음이 반쯤 끝났는데 시작한 멤버가 없을 때 설 자리를 댈 때만 쓴다.
    pub fn started_status(&self) -> &str {
        self.statuses
            .iter()
            .find(|s| *s != self.first_status() && *s != DONE)
            .map_or(self.first_status(), String::as_str)
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

    /// 안 적으면 `full` 이다. 옛 저장소의 설정 파일에는 이 키가 없다.
    #[test]
    fn naming_defaults_to_full() {
        assert_eq!(Config::parse("prefix = \"argos\"\n").unwrap().naming, Naming::Full);
    }

    /// 오타를 조용히 통과시키면 왜 표기가 안 바뀌는지 아무도 못 찾는다.
    #[test]
    fn a_misspelled_naming_is_refused() {
        for raw in ["Full", "이름", "", "name,email"] {
            let src = format!("prefix = \"argos\"\nnaming = \"{raw}\"\n");
            let e = Config::parse(&src).unwrap_err();
            assert!(e.contains("full·name·email"), "{raw:?}: {e}");
        }
        assert_eq!(
            Config::parse("prefix = \"argos\"\nnaming = \"email\"\n").unwrap().naming,
            Naming::Email
        );
    }

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

    /// 시작한 칸은 첫 칸도 끝도 아닌 첫 칸이다. 그런 칸이 없으면 첫 칸 —
    /// 두 칸짜리 설정에서 `done` 으로 말하면 안 끝난 것을 끝났다고 한다.
    #[test]
    fn the_started_column_is_neither_first_nor_done() {
        let started = |cols: &str| {
            let c = Config::parse(&format!("prefix = \"a\"\nstatuses = \"{cols}\"\n")).unwrap();
            c.started_status().to_string()
        };
        assert_eq!(started("todo,in_progress,review,done"), "in_progress");
        assert_eq!(started("todo,done,doing"), "doing");
        assert_eq!(started("todo,done"), "todo");
    }

    #[test]
    fn extra_columns_are_allowed() {
        let c = Config::parse("prefix = \"a\"\nstatuses = \"todo, blocked, done\"\n").unwrap();
        assert_eq!(c.statuses, ["todo", "blocked", "done"]);
    }

    /// **시작했다는 판단은 자리가 아니라 뜻으로 한다**(moai-p415) — 첫 칸도 `done` 도 아니면
    /// 시작했다. 두 칸짜리 설정에는 시작한 칸이 없다.
    #[test]
    fn started_is_neither_first_nor_done() {
        let c = Config::parse("prefix = \"a\"\nstatuses = \"todo, blocked, in_progress, review, done\"\n").unwrap();
        let started: Vec<&str> = c.statuses.iter().map(String::as_str).filter(|s| c.is_started(s)).collect();
        assert_eq!(started, ["blocked", "in_progress", "review"]);
        let two = Config::parse("prefix = \"a\"\nstatuses = \"todo, done\"\n").unwrap();
        assert!(!two.is_started("todo") && !two.is_started("done"));
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
