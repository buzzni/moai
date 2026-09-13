//! 사용자 단위 설정 — 등록한 프로젝트 목록.
//!
//! `.moai/config.toml` 은 **그 저장소의** 것이고 이것은 **그 사람의** 것이다.
//! 둘을 한 모듈에 두지 않는다: 저장소 설정은 엄하게 읽고(오타가 id 접두어를
//! 틀어 놓는다) 손으로만 쓰지만, 이 파일은 관대하게 읽고 도구가 고쳐 쓴다.
//! 읽는 태도와 쓰는 길이 반대라 한 파서에 섞으면 둘 중 하나가 무너진다.
//!
//! **TOML 라이브러리(`toml_edit`)를 쓴다.** `config.rs` 가 안 쓰는 까닭은
//! "중첩도 배열도 없어서" 였는데, 여기는 `[[project]]` 배열이고 무엇보다
//! **고쳐 쓸 때 모르는 키·주석·줄 모양을 보존해야 한다.** 줄 단위로 흉내 내면
//! 여러 줄 문자열이나 여러 줄 배열 안의 `[[project]]` 를 표 머리로 읽는 날이
//! 오고, 그날 남의 키를 지운다. 무게는 `winnow`·`indexmap` 정도고
//! `hashbrown` 은 이미 있던 판이다.
//!
//! 모양은 이렇다. 프로젝트마다 표 하나라 나중에 이름·색 같은 필드를 옛 줄을
//! 깨지 않고 곁에 더할 수 있다 — `projects = ["/a", "/b"]` 로 두면 필드 하나
//! 더하는 순간 모양을 바꿔야 하고, 그건 설정이 아니라 마이그레이션이다.
//!
//! ```toml
//! [[project]]
//! path = "/home/raven/work/argos"
//! ```

use crate::fail::{Fail, R, code};
use crate::store::{Lock, write_atomic};
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};
use toml_edit::{ArrayOfTables, DocumentMut, Item, Table};

/// 프로젝트 목록이 사는 키.
const PROJECT: &str = "project";
const PATH: &str = "path";

/// 설정 파일의 자리. `MOAI_CONFIG` → `$XDG_CONFIG_HOME/moai/config.toml` →
/// `$HOME/.config/moai/config.toml`. 셋 다 없으면 `None`.
///
/// **환경을 직접 읽지 않고 조회 함수를 받는다.** 시험이 `set_var` 로 프로세스
/// 환경을 바꾸면 병렬로 도는 옆 시험이 그것을 본다.
///
/// - 빈 값은 없는 것이다 (환경변수의 관례, `MOAI_ACTOR` 와 같다)
/// - `XDG_CONFIG_HOME` 이 상대경로면 무시한다 — XDG 명세가 그렇게 정했고,
///   따르지 않으면 부른 자리마다 다른 파일을 읽는다
/// - `MOAI_CONFIG` 는 준 그대로다. 시험·격리가 가리키는 파일이라 고르는 쪽의 몫이다
pub fn path_from(env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let set = |k: &str| env(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    if let Some(p) = set("MOAI_CONFIG") {
        return Some(p);
    }
    if let Some(x) = set("XDG_CONFIG_HOME").filter(|p| p.is_absolute()) {
        return Some(x.join("moai/config.toml"));
    }
    set("HOME").map(|h| h.join(".config/moai/config.toml"))
}

/// 이 프로세스의 환경으로 [`path_from`].
pub fn path() -> Option<PathBuf> {
    path_from(|k| std::env::var_os(k))
}

/// 등록한 프로젝트 하나. **지금은 경로뿐이다** — 이름·색은 파생값이거나
/// 나중 필드다. 파일에 있는 모르는 필드는 여기 안 올라오지만 되쓸 때 보존된다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    /// 절대경로. 등록할 때 [`resolve_dir`] 로 정규화한 것이지만, 손으로 적은
    /// 줄이면 심볼릭 링크를 지난 모양일 수 있다 — 견줄 때는 받는 쪽이 푼다.
    pub path: PathBuf,
}

/// 읽은 결과. **실패하지 않는다** — 파일이 깨졌으면 목록은 비고 `problems`
/// 에 까닭이 선다. 사용자 설정 하나 때문에 제 저장소를 보던 명령이 넘어지면
/// 도구가 고장 난 것으로 보인다.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Registry {
    /// 읽은 파일. 자리를 모르면 `None`.
    pub path: Option<PathBuf>,
    /// 읽을 수 있는 항목. 파일에 적힌 차례 그대로, 같은 경로는 한 번만.
    pub projects: Vec<Project>,
    /// 사람이 읽을 한 줄씩. 비어 있으면 아무 일 없다.
    pub problems: Vec<String>,
}

/// 설정을 관대하게 읽는다. 파일이 없으면 빈 목록이고 문제도 아니다 — 아직
/// 아무것도 등록하지 않은 사람의 정상이다.
///
/// **락을 잡지 않는다.** 쓰기가 `rename` 으로 갈아끼우므로 찢어진 파일을 못 본다
/// (`store::Repo::read` 와 같은 까닭).
pub fn read(path: Option<&Path>) -> Registry {
    let Some(path) = path else {
        return Registry {
            path: None,
            projects: Vec::new(),
            problems: vec!["사용자 설정의 자리를 모른다 — MOAI_CONFIG·XDG_CONFIG_HOME·HOME 이 다 없다".into()],
        };
    };
    let mut reg = Registry { path: Some(path.to_path_buf()), ..Registry::default() };
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return reg,
        Err(e) => {
            reg.problems.push(format!("{}: {e}", path.display()));
            return reg;
        }
    };
    match Doc::parse(&src) {
        Ok(doc) => {
            let (projects, problems) = doc.projects();
            reg.projects = projects;
            reg.problems = problems.into_iter().map(|p| format!("{}: {p}", path.display())).collect();
        }
        Err(e) => reg.problems.push(format!("{}: {e}", path.display())),
    }
    reg
}

/// 설정을 고치는 **유일한 길**. 락 → 락 안에서 읽기 → 고치기 → 바뀌었으면
/// temp+rename. `store::Repo::with_write` 와 같은 모양이고 같은 까닭이다:
///
/// **락이 있어야 한다.** `moai project add` 두 개가 동시에 돌면 둘 다 옛 목록을
/// 읽고 제 것 하나를 더해 쓰고, 나중에 `rename` 한 쪽이 앞의 등록을 조용히
/// 지운다. 조용한 손실은 이 도구가 못 견디는 유일한 실패다.
///
/// **깨진 파일에는 쓰지 않는다.** 읽기는 관대하지만 여기서는 보존할 수 없는
/// 것을 덮어쓰게 된다 — 사람이 고칠 때까지 멈추는 편이 싸다. 그러나 **읽을 수
/// 없는 항목 하나**(상대경로 등)는 쓰기를 막지 않고 글자 그대로 들고 간다.
/// 엄함은 지금 쓰는 줄에 대한 것이다.
///
/// 디렉터리가 없으면 만든다. 락 파일은 설정 곁의 `<이름>.lock` 이다.
pub fn update<T>(path: &Path, f: impl FnOnce(&mut Doc) -> R<T>) -> R<T> {
    let err = |p: &Path, e: std::io::Error| Fail::new(format!("{}: {e}", p.display()));
    let dir = path
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir).map_err(|e| err(dir, e))?;
    let mut lock_name = path.file_name().map(OsString::from).unwrap_or_else(|| "config".into());
    lock_name.push(".lock");
    let _lock = Lock::acquire(&dir.join(lock_name))?;

    // 락을 잡은 **뒤에** 읽는다. 밖에서 읽으면 두 프로세스가 같은 옛 목록을 고친다.
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(err(path, e)),
    };
    let mut doc = Doc::parse(&src).map_err(|e| {
        Fail::coded(format!("{}: {e} — 고치기 전까지 쓰지 않는다", path.display()), code::BROKEN)
    })?;
    let out = f(&mut doc)?;
    // **바꾼 것이 없으면 건드리지 않는다.** 렌더 결과를 원문과 견주지 않고 깃발을
    // 보는 까닭은, 라이브러리가 어느 날 공백 하나를 달리 내더라도 헛 쓰기가 안
    // 생기게 하려는 것이다.
    if doc.dirty {
        write_atomic(path, doc.render().as_bytes())?;
    }
    Ok(out)
}

/// 읽어 들인 설정 문서. 모르는 키·표·주석은 문서가 들고 있다가 그대로 낸다.
pub struct Doc {
    doc: DocumentMut,
    bom: bool,
    dirty: bool,
}

impl Doc {
    /// 파싱한다. **`project` 가 표 배열이 아니면 거절한다** — 그 키가 무엇인지
    /// 모르는 채로 항목을 더하면 남의 값을 덮는다.
    pub fn parse(src: &str) -> Result<Doc, String> {
        let (bom, body) = match src.strip_prefix('\u{feff}') {
            Some(rest) => (true, rest),
            None => (false, src),
        };
        let doc: DocumentMut = body.parse().map_err(|e: toml_edit::TomlError| {
            // 라이브러리의 오류는 여러 줄 그림이다. 한 줄로 접어야 목록 속 한 줄로 선다.
            e.to_string().split_whitespace().collect::<Vec<_>>().join(" ")
        })?;
        match doc.get(PROJECT) {
            None => {}
            Some(item) if item.is_array_of_tables() => {}
            Some(item) => {
                return Err(format!(
                    "`{PROJECT}` 는 `[[{PROJECT}]]` 표 배열이어야 한다 — 지금은 {}",
                    item.type_name()
                ));
            }
        }
        Ok(Doc { doc, bom, dirty: false })
    }

    pub fn render(&self) -> String {
        let body = self.doc.to_string();
        if self.bom { format!("\u{feff}{body}") } else { body }
    }

    fn tables(&self) -> Option<&ArrayOfTables> {
        self.doc.get(PROJECT).and_then(Item::as_array_of_tables)
    }

    /// 읽을 수 있는 항목과, 못 읽는 항목의 까닭. 같은 경로가 두 번 적혔으면
    /// 앞의 것 하나만 낸다 — 손으로 적은 겹침으로 같은 프로젝트가 두 번 보이면
    /// 어느 쪽이 진짜인지 묻게 된다.
    pub fn projects(&self) -> (Vec<Project>, Vec<String>) {
        let mut out: Vec<Project> = Vec::new();
        let mut problems = Vec::new();
        for (i, t) in self.tables().into_iter().flat_map(ArrayOfTables::iter).enumerate() {
            match entry_path(t) {
                Ok(path) if out.iter().any(|p| p.path == path) => {}
                Ok(path) => out.push(Project { path }),
                Err(e) => problems.push(format!("{}번째 [[{PROJECT}]]: {e}", i + 1)),
            }
        }
        (out, problems)
    }

    /// 등록한다. 이미 있으면 아무것도 안 하고 `false` — **멱등이다.**
    ///
    /// 받는 것은 이미 정규화한 경로다([`resolve_dir`]). 여기서 파일 시스템을
    /// 만지지 않는 까닭은 이 연산을 시험에서 디렉터리 없이 재려는 것이다.
    /// 대신 적는 줄은 엄하게 잰다: 절대경로여야 하고, UTF-8 이어야 한다(TOML
    /// 문자열은 UTF-8 뿐이라 아니면 적을 수 없다). `.moai` 가 있는지는 안 본다 —
    /// 나중에 `moai init` 하면 보이는 것이 요구다.
    pub fn add(&mut self, dir: &Path) -> R<bool> {
        let text = writable(dir)?;
        if self.projects().0.iter().any(|p| p.path == dir) {
            return Ok(false);
        }
        let mut t = Table::new();
        t.insert(PATH, toml_edit::value(text));
        match self.doc.get_mut(PROJECT).and_then(Item::as_array_of_tables_mut) {
            Some(aot) => aot.push(t),
            None => {
                let mut aot = ArrayOfTables::new();
                aot.push(t);
                self.doc.insert(PROJECT, Item::ArrayOfTables(aot));
            }
        }
        self.dirty = true;
        Ok(true)
    }

    /// 경로가 `any_of` 중 하나와 같은 항목을 **모두** 뺀다. 뺀 수를 낸다.
    ///
    /// 한 경로를 여러 철자로 받는 까닭은 [`spellings`] 에 있다. 못 읽는 항목은
    /// 건드리지 않는다 — 무엇을 가리키는지 모르는 줄을 지우면 되돌릴 수 없다.
    /// 목록에서만 뺀다. 그 디렉터리의 `.moai` 는 이 모듈이 모른다.
    pub fn remove(&mut self, any_of: &[PathBuf]) -> usize {
        let Some(aot) = self.doc.get_mut(PROJECT).and_then(Item::as_array_of_tables_mut) else {
            return 0;
        };
        let before = aot.len();
        aot.retain(|t| !entry_path(t).is_ok_and(|p| any_of.contains(&p)));
        let removed = before - aot.len();
        if removed > 0 {
            self.dirty = true;
        }
        removed
    }
}

/// 항목 표 하나에서 경로를 읽는다. 상대경로는 거절한다 — 부른 자리마다 다른
/// 디렉터리를 가리키게 된다.
fn entry_path(t: &Table) -> Result<PathBuf, String> {
    let raw = match t.get(PATH) {
        None => return Err(format!("`{PATH}` 가 없다")),
        Some(item) => item
            .as_str()
            .ok_or_else(|| format!("`{PATH}` 는 문자열이어야 한다 — 지금은 {}", item.type_name()))?,
    };
    let p = PathBuf::from(raw);
    if raw.is_empty() || !p.is_absolute() {
        return Err(format!("`{PATH}` 는 절대경로여야 한다 — {raw:?}"));
    }
    Ok(p)
}

fn writable(dir: &Path) -> R<&str> {
    let text = dir
        .to_str()
        .ok_or_else(|| Fail::coded(format!("UTF-8 이 아닌 경로는 적을 수 없다 — {}", dir.display()), code::BAD_INPUT))?;
    if !dir.is_absolute() {
        return Err(Fail::coded(format!("절대경로여야 한다 — {text:?}"), code::BAD_INPUT));
    }
    Ok(text)
}

/// 등록할 디렉터리를 정규화한다. 상대경로는 `cwd` 에 붙이고 **심볼릭 링크를 푼다.**
///
/// 푸는 까닭: `Repo::discover` 가 출발하는 `current_dir()` 은 커널이 준 물리
/// 경로다. 링크를 지난 철자로 적어 두면 "지금 서 있는 저장소가 등록한 것인가"
/// 를 견줄 때 어긋나고, 같은 디렉터리가 두 철자로 두 번 등록된다. 대가는 링크를
/// 다른 곳으로 옮기면 옛 물리 경로가 남는 것인데, 그건 "사라진 디렉터리" 한
/// 줄로 드러나고 다시 등록하면 된다 — 조용히 틀린 쪽을 보는 것보다 싸다.
///
/// 디렉터리가 있어야 한다(`canonicalize` 가 그것을 요구한다). `.moai` 는 없어도 된다.
pub fn resolve_dir(input: &Path, cwd: &Path) -> R<PathBuf> {
    let joined = cwd.join(input);
    let real = std::fs::canonicalize(&joined).map_err(|e| {
        let code = if e.kind() == std::io::ErrorKind::NotFound { code::NOT_FOUND } else { code::ERROR };
        Fail::coded(format!("{}: {e}", joined.display()), code)
    })?;
    if !real.is_dir() {
        return Err(Fail::coded(format!("디렉터리가 아니다 — {}", real.display()), code::BAD_INPUT));
    }
    Ok(real)
}

/// 뺄 때 견줄 철자들 — 글자로 정리한 절대경로와, 있으면 링크를 푼 경로.
///
/// 뺄 때는 디렉터리가 이미 없을 수 있어 [`resolve_dir`] 만으로는 못 찾는다.
/// 반대로 손으로 적은 링크 철자는 푼 경로로는 안 맞는다. 둘 다 댄다.
pub fn spellings(input: &Path, cwd: &Path) -> Vec<PathBuf> {
    let lexical = lexical(&cwd.join(input));
    let mut out = vec![lexical.clone()];
    if let Ok(real) = std::fs::canonicalize(&lexical)
        && real != lexical
    {
        out.push(real);
    }
    out
}

/// `.` 을 버리고 `..` 은 앞 조각을 뗀다. 파일 시스템을 안 본다.
fn lexical(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                // 뿌리 위로는 못 올라간다 (`/..` 은 `/`).
                if !matches!(out.components().next_back(), None | Some(Component::RootDir | Component::Prefix(_))) {
                    out.pop();
                }
            }
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("moai-user-config-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let m: HashMap<String, OsString> = pairs.iter().map(|(k, v)| (k.to_string(), OsString::from(v))).collect();
        move |k| m.get(k).cloned()
    }

    #[test]
    fn the_path_prefers_moai_config_then_xdg_then_home() {
        let all = [("MOAI_CONFIG", "/t/c.toml"), ("XDG_CONFIG_HOME", "/x"), ("HOME", "/h")];
        assert_eq!(path_from(env(&all)), Some("/t/c.toml".into()));
        assert_eq!(path_from(env(&all[1..])), Some("/x/moai/config.toml".into()));
        assert_eq!(path_from(env(&all[2..])), Some("/h/.config/moai/config.toml".into()));
        assert_eq!(path_from(env(&[])), None);
    }

    /// 빈 값은 없는 것이고, 상대 `XDG_CONFIG_HOME` 은 XDG 명세대로 무시한다.
    #[test]
    fn empty_and_relative_values_fall_through() {
        let e = env(&[("MOAI_CONFIG", ""), ("XDG_CONFIG_HOME", "rel/x"), ("HOME", "/h")]);
        assert_eq!(path_from(e), Some("/h/.config/moai/config.toml".into()));
        let e = env(&[("XDG_CONFIG_HOME", ""), ("HOME", "/h")]);
        assert_eq!(path_from(e), Some("/h/.config/moai/config.toml".into()));
    }

    #[test]
    fn a_missing_file_is_an_empty_list_not_a_problem() {
        let d = scratch("missing");
        let reg = read(Some(&d.join("없음/config.toml")));
        assert!(reg.projects.is_empty() && reg.problems.is_empty(), "{reg:?}");
        assert!(!read(None).problems.is_empty(), "자리를 모르면 그렇다고 말해야 한다");
    }

    /// 깨진 파일은 읽기에서 알리고 계속, 쓰기에서 멈춘다. 파일은 한 글자도 안 바뀐다.
    #[test]
    fn a_broken_file_is_reported_on_read_and_refused_on_write() {
        let d = scratch("broken");
        let path = d.join("config.toml");
        for src in ["[[project]\npath = \"/a\"\n", "project = \"/a\"\n", "project = [{ path = \"/a\" }]\n"] {
            std::fs::write(&path, src).unwrap();
            let reg = read(Some(&path));
            assert!(reg.projects.is_empty(), "{src:?} → {reg:?}");
            assert_eq!(reg.problems.len(), 1, "{src:?} → {reg:?}");
            assert!(!reg.problems[0].contains('\n'), "문제는 한 줄이어야 한다 — {:?}", reg.problems[0]);

            let e = update(&path, |doc| doc.add(Path::new("/b"))).unwrap_err();
            assert_eq!(e.code, code::BROKEN, "{src:?} → {e}");
            assert_eq!(std::fs::read_to_string(&path).unwrap(), src, "깨진 파일을 덮어썼다");
        }
    }

    /// 못 읽는 항목 하나는 그 줄만 말하고, 나머지는 보이고, 쓰기를 막지 않는다.
    #[test]
    fn a_bad_entry_is_reported_carried_and_does_not_block() {
        let d = scratch("bad-entry");
        let path = d.join("config.toml");
        let src = "[[project]]\npath = \"상대/경로\"\n\n[[project]]\npath = \"/good\"\n\n[[project]]\nname = \"경로 없음\"\n\n[[project]]\npath = 3\n";
        std::fs::write(&path, src).unwrap();
        let reg = read(Some(&path));
        assert_eq!(reg.projects, [Project { path: "/good".into() }]);
        assert_eq!(reg.problems.len(), 3, "{reg:?}");
        assert!(reg.problems[0].contains("1번째") && reg.problems[0].contains("절대경로"), "{reg:?}");

        update(&path, |doc| doc.add(Path::new("/new"))).expect("못 읽는 항목이 쓰기를 막았다");
        let after = std::fs::read_to_string(&path).unwrap();
        assert!(after.starts_with(src), "못 읽는 항목을 잃거나 옮겼다\n{after}");
        assert_eq!(read(Some(&path)).projects.len(), 2);
    }

    /// 모르는 키·표·주석이 되쓰기를 지나 살아남는다. 새 바이너리가 쓴 키를
    /// 옛 바이너리가 한 번 만지는 것만으로 지우면 안 된다.
    #[test]
    fn unknown_keys_tables_and_comments_survive_a_rewrite() {
        let d = scratch("unknown");
        let path = d.join("config.toml");
        let src = "\u{feff}# 내 설정\ntheme = \"dark\"  # 뒤 주석\n\n[[project]]\npath = \"/a\"\ncolor = \"cyan\"\n\n[ui]\nlayers = [\n  \"[[project]]\",\n]\n";
        std::fs::write(&path, src).unwrap();

        // 고치지 않고 파싱→렌더는 바이트 그대로다.
        assert_eq!(Doc::parse(src).unwrap().render(), src);

        update(&path, |doc| doc.add(Path::new("/b"))).unwrap();
        let after = std::fs::read_to_string(&path).unwrap();
        for kept in ["\u{feff}# 내 설정\n", "theme = \"dark\"  # 뒤 주석", "color = \"cyan\"", "\"[[project]]\",", "[ui]"] {
            assert!(after.contains(kept), "{kept:?} 를 잃었다\n{after}");
        }
        let reg = read(Some(&path));
        assert_eq!(reg.projects.iter().map(|p| p.path.to_str().unwrap()).collect::<Vec<_>>(), ["/a", "/b"]);
        assert!(reg.problems.is_empty(), "{reg:?}");

        // 더했다 빼면 처음 바이트로 돌아온다.
        update(&path, |doc| Ok(doc.remove(&["/b".into()]))).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), src);
    }

    /// 바꾼 것이 없으면 파일을 건드리지 않는다 — 헛 쓰기도 헛 diff 도 없다.
    #[test]
    fn a_no_op_update_leaves_the_file_alone() {
        let d = scratch("noop");
        let path = d.join("config.toml");
        let src = "[[project]]\npath   =   \"/a\"   # 손으로 띄운 칸\n";
        std::fs::write(&path, src).unwrap();
        let before = std::fs::metadata(&path).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));

        assert!(!update(&path, |doc| doc.add(Path::new("/a"))).unwrap());
        assert_eq!(update(&path, |doc| Ok(doc.remove(&["/없음".into()]))).unwrap(), 0);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), src);
        assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), before);

        // 없는 파일에 아무것도 안 하면 파일을 만들지도 않는다.
        let fresh = d.join("sub/dir/config.toml");
        update(&fresh, |_| Ok(())).unwrap();
        assert!(!fresh.exists());
    }

    #[test]
    fn add_is_idempotent_and_remove_takes_every_spelling() {
        let d = scratch("add-rm");
        let path = d.join("nested/moai/config.toml");
        assert!(update(&path, |doc| doc.add(Path::new("/a"))).unwrap(), "디렉터리를 만들고 써야 한다");
        assert!(!update(&path, |doc| doc.add(Path::new("/a/"))).unwrap(), "같은 경로를 두 번 넣었다");
        assert!(update(&path, |doc| doc.add(Path::new("/b"))).unwrap());
        assert_eq!(read(Some(&path)).projects.len(), 2);

        // 손으로 겹쳐 적은 줄도 하나로 보이고, 뺄 때는 전부 빠진다.
        let src = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, format!("{src}\n[[project]]\npath = \"/a\"\n")).unwrap();
        assert_eq!(read(Some(&path)).projects.len(), 2);
        assert_eq!(update(&path, |doc| Ok(doc.remove(&["/x".into(), "/a".into()]))).unwrap(), 2);
        assert_eq!(read(Some(&path)).projects, [Project { path: "/b".into() }]);
    }

    /// 적는 줄은 엄하다 — 상대경로는 부른 자리마다 다른 곳을 가리킨다.
    #[test]
    fn add_refuses_a_relative_path() {
        let mut doc = Doc::parse("").unwrap();
        assert_eq!(doc.add(Path::new("rel")).unwrap_err().code, code::BAD_INPUT);
        assert!(!doc.dirty);
    }

    #[test]
    fn resolve_dir_makes_absolute_and_follows_symlinks() {
        let d = scratch("resolve").canonicalize().unwrap();
        std::fs::create_dir_all(d.join("real/apps/a")).unwrap();
        assert_eq!(resolve_dir(Path::new("real/apps/./a"), &d).unwrap(), d.join("real/apps/a"));
        assert_eq!(resolve_dir(&d.join("real/apps/a/.."), Path::new("/")).unwrap(), d.join("real/apps"));
        assert_eq!(resolve_dir(Path::new("없음"), &d).unwrap_err().code, code::NOT_FOUND);
        std::fs::write(d.join("file"), "").unwrap();
        assert_eq!(resolve_dir(Path::new("file"), &d).unwrap_err().code, code::BAD_INPUT);

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(d.join("real"), d.join("link")).unwrap();
            assert_eq!(resolve_dir(Path::new("link/apps"), &d).unwrap(), d.join("real/apps"));
            // 뺄 때는 글자 철자와 푼 철자를 둘 다 댄다.
            assert_eq!(spellings(Path::new("link/./apps"), &d), [d.join("link/apps"), d.join("real/apps")]);
        }
        // 사라진 디렉터리도 글자로는 찾는다.
        assert_eq!(spellings(Path::new("gone/../gone2"), &d), [d.join("gone2")]);
        assert_eq!(lexical(Path::new("/../a")), PathBuf::from("/a"));
    }

    /// **동시 등록이 서로를 지우지 않는다.** 락이 없으면 둘 다 옛 목록을 읽고
    /// 나중에 `rename` 한 쪽이 앞의 것을 조용히 지운다. `flock` 은 열린 파일마다라
    /// 한 프로세스의 스레드끼리도 서로 막는다.
    #[test]
    fn concurrent_updates_all_survive() {
        let d = scratch("concurrent");
        let path = d.join("config.toml");
        let (threads, each) = (8, 5);
        let handles: Vec<_> = (0..threads)
            .map(|t| {
                let path = path.clone();
                std::thread::spawn(move || {
                    for i in 0..each {
                        let dir = PathBuf::from(format!("/p/{t}/{i}"));
                        update(&path, |doc| doc.add(&dir)).unwrap();
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        let reg = read(Some(&path));
        assert!(reg.problems.is_empty(), "{reg:?}");
        assert_eq!(reg.projects.len(), threads * each, "{}개를 동시에 넣었는데 {}개만 남았다", threads * each, reg.projects.len());
    }
}
