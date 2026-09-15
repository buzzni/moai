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
//! color = "green"      # 없으면 경로로 고른다 (moai-o04b)
//! ```

use crate::fail::{Fail, R, code};
use crate::store::Lock;
use crate::style::Hue;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};
use toml_edit::{ArrayOfTables, DocumentMut, Item, Table};

/// 프로젝트 목록이 사는 키.
const PROJECT: &str = "project";
const PATH: &str = "path";
/// 프로젝트 색. **사람이 치는 철자는 `color` 하나다** — 전역 `--color`·`NO_COLOR` 와 같다.
const COLOR: &str = "color";
/// 틀리기 쉬운 철자. 받지 않고 알린다 — 조용히 버리면 적었는데 안 먹고, 둘 다 받으면
/// 둘이 다를 때 어느 쪽이냐는 둘째 규칙이 생긴다.
const COLOUR: &str = "colour";
/// 색을 정하지 않은 것 — 경로로 고른다. 명령줄에서는 키를 지우는 낱말이고, 설정에
/// 적혀 있어도 같은 뜻으로 읽는다.
pub const AUTO: &str = "auto";

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

/// 등록한 프로젝트 하나. 이름은 파생값이라 여기 없다. 파일에 있는 모르는 필드는
/// 여기 안 올라오지만 되쓸 때 보존된다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    /// 절대경로. 등록할 때 [`resolve_dir`] 로 정규화한 것이지만, 손으로 적은
    /// 줄이면 심볼릭 링크를 지난 모양일 수 있다 — 견줄 때는 받는 쪽이 푼다.
    pub path: PathBuf,
    /// 사람이 정한 색. `None` 이면 경로로 고른다(`style::project_colour`). **파생값이
    /// 아니라 사람이 적은 값이라** 들고 다닌다 — 틀린 값은 읽을 때 `None` 으로 접고 알린다.
    pub hue: Option<Hue>,
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
    /// 적어 둔 화면 언어(moai-slfv). 없으면 `None` 이고 영어로 떨어진다.
    pub lang: Option<String>,
    /// 적어 둔 탐색기 보기와 그것을 읽다 만난 까닭(moai-2bzp). **같은 파싱에서 함께 읽는다**(moai-u8cs) —
    /// 탐색기를 띄우면 층과 보기가 저마다 파일을 읽고 파싱해 한 번 띄울 때 설정을 두세 번 읽었다.
    /// 까닭을 `problems` 와 따로 드는 것은 대는 자리가 달라서다 — 층의 문제는 층이, 보기의 문제는 알림이 댄다.
    pub look: Look,
    pub look_problems: Vec<String>,
}

/// 설정을 관대하게 읽는다. 파일이 없으면 빈 목록이고 문제도 아니다 — 아직
/// 아무것도 등록하지 않은 사람의 정상이다.
///
/// **락을 잡지 않는다.** 쓰기가 `rename` 으로 갈아끼우므로 찢어진 파일을 못 본다
/// (`store::Repo::read` 와 같은 까닭).
pub fn read(path: Option<&Path>) -> Registry {
    let Some(path) = path else {
        return Registry {
            problems: vec!["사용자 설정의 자리를 모른다 — MOAI_CONFIG·XDG_CONFIG_HOME·HOME 이 다 없다".into()],
            ..Registry::default()
        };
    };
    let mut reg = Registry { path: Some(path.to_path_buf()), ..Registry::default() };
    let at = |e: String| format!("{}: {e}", path.display());
    let parsed = match std::fs::read_to_string(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return reg,
        Err(e) => Err(e.to_string()),
        Ok(src) => Doc::parse(&src),
    };
    match parsed {
        Ok(doc) => {
            let (projects, problems) = doc.projects();
            reg.projects = projects;
            reg.problems = problems.into_iter().map(at).collect();
            let (lang, lang_problems) = doc.lang();
            reg.lang = lang;
            reg.problems.extend(lang_problems.into_iter().map(at));
            let (look, problems) = doc.look();
            reg.look = look;
            reg.look_problems = problems.into_iter().map(at).collect();
        }
        // 못 읽었거나 깨진 까닭은 **층과 보기가 둘 다 댄다** — 따로 읽던 때와 같다(moai-z0q6 이 따로 본다).
        Err(e) => {
            reg.problems = vec![at(e)];
            reg.look_problems = reg.problems.clone();
        }
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
    let _lock = Lock::acquire(&lock_beside(path))?;

    // 설정 파일이 심볼릭 링크면(dotfiles 저장소가 흔히 그렇게 건다) **링크가 가리키는
    // 파일을** 고친다. 링크 자리에 `rename` 하면 링크가 보통 파일로 갈아끼워져
    // dotfiles 쪽은 옛 내용에 멈추고, 사람은 그것을 모른다. 푸는 것은 락 **안에서**
    // 한다: 밖에서 풀면 그 사이에 파일이 링크로 갈아끼워질 수 있다.
    let resolved = std::fs::canonicalize(path).ok();
    let real = resolved.as_deref().unwrap_or(path);

    // **푼 자리에도 락을 잡는다.** 준 철자 곁의 락만으로는 같은 파일을 두 철자로 부른
    // 둘이 서로 다른 락 파일을 잡아 아무도 막지 않는다 — `~/.config/moai/config.toml`
    // 이 `~/dotfiles/…` 로 걸린 사람이 한쪽 철자로 등록하는 동안 다른 철자로 등록하면
    // 나중에 `rename` 한 쪽이 앞의 등록을 지운다(재 보면 스무 개 중 열 개가 사라진다).
    // 그것이 이 모듈이 못 견딘다고 적어 둔 조용한 손실이다. 대가는 dotfiles 저장소에
    // 락 파일 하나가 어른거리는 것인데, 잃는 것보다 싸다.
    //
    // **차례가 있어 엉키지 않는다**: 푼 경로는 `canonicalize` 의 고정점이라 모든
    // 프로세스가 같은 자리를 둘째로 잡고, 준 철자가 곧 푼 경로인 쪽은 하나만 잡는다.
    let _real_lock = (real != path).then(|| Lock::acquire(&lock_beside(real))).transpose()?;
    let path = real;

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
        // **사람이 정한 권한을 지킨다.** 설정은 사람의 파일이다 — `chmod 600` 해 둔 것이
        // 등록 한 번에 0644 로 풀리면 안 되고, 권한까지 추적하는 dotfiles 저장소에 헛 변경이
        // 뜬다. 파일이 없던 처음 쓰기만 umask 를 따른다 — `store::write_atomic` 이 한다.
        crate::store::write_atomic(path, doc.render().as_bytes())?;
    }
    Ok(out)
}

/// 그 설정 파일의 락 자리 — 곁의 `<이름>.lock`.
fn lock_beside(path: &Path) -> PathBuf {
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let mut name = path.file_name().map(OsString::from).unwrap_or_else(|| "config".into());
    name.push(".lock");
    dir.join(name)
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

    /// 고친 것이 있나 — [`update`] 가 쓸지 가르는 깃발 그대로다.
    pub fn changed(&self) -> bool {
        self.dirty
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
    ///
    /// **색이 틀린 항목은 빼지 않는다.** 경로로 고른 색으로 서고 까닭이 한 줄 선다 —
    /// 빼면 색 오타 하나로 한눈 보기에서 저장소가 통째로 사라진다.
    pub fn projects(&self) -> (Vec<Project>, Vec<String>) {
        let mut out: Vec<Project> = Vec::new();
        let mut problems = Vec::new();
        for (i, t) in self.tables().into_iter().flat_map(ArrayOfTables::iter).enumerate() {
            let at = |e: String| format!("{}번째 [[{PROJECT}]]: {e}", i + 1);
            match entry_path(t) {
                Ok(path) if out.iter().any(|p| p.path == path) => {}
                Ok(path) => {
                    let hue = entry_hue(t).unwrap_or_else(|e| {
                        problems.push(at(format!("{e} — 지금은 경로로 고른 색을 쓴다")));
                        None
                    });
                    // 둘 다 적혔으면 `color` 를 읽되 `colour` 도 알린다 — 조용히 버리면 뒤에 고쳐
                    // 적은 `colour` 가 안 먹는 까닭을 아무도 말하지 않는다.
                    if t.contains_key(COLOR) && t.contains_key(COLOUR) {
                        problems.push(at(format!("`{COLOUR}` 는 모르는 키다 — `{COLOR}` 만 읽는다")));
                    }
                    out.push(Project { path, hue });
                }
                Err(e) => problems.push(at(e)),
            }
        }
        (out, problems)
    }

    /// 경로가 `any_of` 중 하나인 항목의 색을 바꾼다. `None` 이면 키를 **지운다** — 경로로
    /// 고르는 것으로 돌아가고, 더했다 지우면 처음 바이트로 돌아온다. 같은 값이면 파일을 안
    /// 건드린다. 맞은 항목 수를 낸다(0 이면 등록돼 있지 않다).
    ///
    /// 손으로 겹쳐 적은 줄은 **모두** 바꾼다 — 읽기는 앞의 것만 보지만, 앞의 것을 지웠을 때
    /// 뒤의 것이 옛 색으로 되살아나면 안 된다([`Doc::remove`] 가 모두 빼는 것과 같다).
    /// 틀린 철자 `colour` 는 건드리지 않는다 — 무엇을 뜻했는지 모르는 남의 키다.
    pub fn set_hue(&mut self, any_of: &[PathBuf], hue: Option<Hue>) -> usize {
        let Some(aot) = self.doc.get_mut(PROJECT).and_then(Item::as_array_of_tables_mut) else {
            return 0;
        };
        let mut hit = 0;
        for t in aot.iter_mut().filter(|t| entry_path(t).is_ok_and(|p| any_of.contains(&p))) {
            hit += 1;
            match hue {
                Some(h) if t.get(COLOR).and_then(Item::as_str) == Some(h.name()) => {}
                Some(h) => {
                    // 옛 값 뒤의 주석(`color = "blue"  # 회사 것`)은 들고 간다 — 값만 바꾼 것이다.
                    let mut v = toml_edit::Value::from(h.name());
                    if let Some(old) = t.get(COLOR).and_then(Item::as_value) {
                        *v.decor_mut() = old.decor().clone();
                    }
                    t.insert(COLOR, Item::Value(v));
                    self.dirty = true;
                }
                None => {
                    if t.remove(COLOR).is_some() {
                        self.dirty = true;
                    }
                }
            }
        }
        hit
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

    /// 적어 둔 화면 언어(moai-slfv). **사람의 설정이지 프로젝트의 것이 아니다** — 같은 사람이
    /// 프로젝트를 옮겨 다녀도 읽는 말은 그대로고, 한 프로젝트를 여럿이 볼 때 서로 다른 말로
    /// 읽는다. 없거나 낱말이 아니면 `None` 이고 [`crate::i18n::pick`] 이 영어로 떨어진다 —
    /// 여기서 까닭을 쌓지 않는 것은, 글자 하나 때문에 도구가 안 도는 것처럼 보이면 안 돼서다.
    /// **틀린 값은 알린다**(리뷰 moai-slfv.vrw). `lang = "kr"` 오타나 `lang = 3` 은 조용히
    /// 영어가 되는데, 그러면 고친 설정이 왜 안 듣는지 알 길이 없다 — `look()` 이 틀린 보기 키를
    /// 대는 것과 같은 자다. 이 까닭은 **막지 않는다**: `problems` 는 알림이지 게이트가 아니다.
    pub fn lang(&self) -> (Option<String>, Vec<String>) {
        let mut problems = Vec::new();
        let Some(item) = self.doc.get(I18N) else { return (None, problems) };
        let Some(t) = item.as_table_like() else {
            problems.push(format!("`{I18N}` 은 `[{I18N}]` 표여야 한다 — 지금은 {}", item.type_name()));
            return (None, problems);
        };
        let Some(item) = t.get(LANG) else { return (None, problems) };
        let Some(raw) = item.as_str() else {
            problems.push(format!("`{I18N}.{LANG}` 은 낱말이어야 한다 — 지금은 {}", item.type_name()));
            return (None, problems);
        };
        if crate::i18n::Lang::parse(raw).is_none() {
            let known: Vec<&str> = crate::i18n::Lang::ALL.into_iter().map(crate::i18n::Lang::code).collect();
            problems.push(format!("`{I18N}.{LANG}` 은 {} 중 하나다 — {raw:?}", known.join("·")));
            return (None, problems);
        }
        (Some(raw.to_string()), problems)
    }

    /// 적어 둔 탐색기 보기와, 못 읽은 키의 까닭(moai-2bzp). **관대하게 읽는다** — 틀린 키 하나가
    /// 나머지 보기를 버리게 두지 않는다. `[tui]` 가 없으면 빈 `Look` 이다.
    pub fn look(&self) -> (Look, Vec<String>) {
        let mut problems = Vec::new();
        let Some(item) = self.doc.get(TUI) else {
            return (Look::default(), problems);
        };
        let Some(t) = item.as_table_like() else {
            problems.push(format!("`{TUI}` 는 `[{TUI}]` 표여야 한다 — 지금은 {}", item.type_name()));
            return (Look::default(), problems);
        };
        let word = |i: &Item| i.as_str().map(String::from);
        let look = Look {
            hidden: look_words(t, HIDDEN, &mut problems),
            hide_deferred: look_one(t, HIDE_DEFERRED, "true·false 여야", Item::as_bool, &mut problems),
            sort: look_one(t, SORT, "낱말이어야", word, &mut problems),
            sort_reversed: look_one(t, SORT_REVERSED, "true·false 여야", Item::as_bool, &mut problems),
            fields: look_words(t, FIELDS, &mut problems),
            fields_known: look_words(t, FIELDS_KNOWN, &mut problems),
            detail: look_one(t, DETAIL, "true·false 여야", Item::as_bool, &mut problems),
        };
        (look, problems)
    }

    /// 이 세션이 보기를 `base` 에서 `new` 로 바꾼 **만큼만** 지금 파일 위에 옮긴다(moai-2kyl 단계 리뷰).
    ///
    /// 화면이 든 보기를 통째로 적으면 그사이 옆 탐색기·손·새 바이너리가 적은 것을 토글 한 번이 되돌린다 —
    /// 락 안에서 다시 읽는 까닭이 사라지는 조용한 손실이다. 그래서
    /// - **이 세션이 안 바꾼 키는 건드리지 않는다.** 모르는 낱말·모르는 모양(`sort = { … }`)·틀린 값도 그대로다
    /// - **낱말 배열(`hidden`·`fields`)은 뺀 낱말만 빼고 더한 낱말만 끝에 더한다** — 남이 더한 낱말은 남는다
    /// - **차례와 방향(`sort`·`sort_reversed`)은 한 벌이다** — 하나를 고르면 둘을 함께 적는다
    /// - **값만 바꾼다** — 키 위의 주석·값 뒤의 주석·여러 줄로 벌인 배열은 그대로다(`put_value`)
    ///
    /// `None` 으로 바꾼 키는 지운다. 파일이 이미 그렇게 적혀 있으면(옆에서 같게 적었으면) 아무것도 안
    /// 한다 — 헛 쓰기가 없다. **`tui` 가 표가 아니면 적지 않는다** — 무엇인지 모르는 값을 덮으면 되돌릴
    /// 수 없다.
    pub fn merge_look(&mut self, base: &Look, new: &Look) -> R<()> {
        if base == new {
            return Ok(());
        }
        match self.doc.get(TUI) {
            None => {
                self.doc.insert(TUI, Item::Table(Table::new()));
            }
            // 읽기(`look`)가 받는 모양은 쓰기도 받는다 — `tui = { … }` 인라인 표도 표다.
            Some(item) if item.is_table_like() => {}
            Some(item) => {
                return Err(Fail::new(format!(
                    "`{TUI}` 가 `[{TUI}]` 표가 아니라({}) 보기를 적지 않는다 — 손으로 고친다",
                    item.type_name()
                )));
            }
        }
        let t = self.doc.get_mut(TUI).and_then(Item::as_table_like_mut).expect("방금 표로 섰다");
        let mut changed = merge_words(t, HIDDEN, base.hidden.as_deref(), new.hidden.as_deref());
        if base.hide_deferred != new.hide_deferred {
            changed |= put_value(t, HIDE_DEFERRED, new.hide_deferred.map(toml_edit::Value::from));
        }
        if (&base.sort, base.sort_reversed) != (&new.sort, new.sort_reversed) {
            changed |= put_value(t, SORT, new.sort.as_deref().map(toml_edit::Value::from));
            changed |= put_value(t, SORT_REVERSED, new.sort_reversed.map(toml_edit::Value::from));
        }
        changed |= merge_words(t, FIELDS, base.fields.as_deref(), new.fields.as_deref());
        // **`fields_known` 은 빼지 않고 더하기만 한다**(moai-6bc0 단계 리뷰) — `base` 를 비워 두는 까닭이다.
        // 이 키는 사람이 고른 것이 아니라 *적는 쪽이 아는 열 전부*라, 여기 있는데 이 바이너리가 모르는
        // 이름은 **새 바이너리가 적어 둔 것**이다. 그것을 빼면 그쪽의 다음 실행이 제가 적어 둔 열을
        // "몰랐던 열" 로 읽어 사람이 끈 것을 도로 켠다 — 낱말 배열을 합치는 까닭(`남이 더한 낱말은
        // 남는다`)이 여기서는 더 세게 걸린다.
        changed |= merge_words(t, FIELDS_KNOWN, None, new.fields_known.as_deref());
        if base.detail != new.detail {
            changed |= put_value(t, DETAIL, new.detail.map(toml_edit::Value::from));
        }
        self.dirty |= changed;
        Ok(())
    }
}

/// 화면 언어가 사는 표(moai-slfv).
const I18N: &str = "i18n";
const LANG: &str = "lang";

/// 탐색기 보기가 사는 표(moai-2bzp).
const TUI: &str = "tui";
const HIDDEN: &str = "hidden";
const HIDE_DEFERRED: &str = "hide_deferred";
const SORT: &str = "sort";
const SORT_REVERSED: &str = "sort_reversed";
const FIELDS: &str = "fields";
const DETAIL: &str = "detail";
const FIELDS_KNOWN: &str = "fields_known";

/// 탐색기의 보기 — 사람이 마지막으로 고른 것(moai-2bzp). **낱말로 든다** — 무슨 낱말이 있는지는
/// 탐색기가 안다. 이 모듈이 조각의 타입을 알면 설정 파일의 모양이 화면 코드에 매인다. 없는 키는
/// `None` 이고, 그 자리는 탐색기의 처음값이 선다.
///
/// ```toml
/// [tui]
/// hidden = ["done"]
/// hide_deferred = false
/// detail = true
/// sort = "updated"
/// sort_reversed = false
/// fields = ["id", "priority", "tally", "assignee"]
/// fields_known = ["id", "priority", "assignee", "created", "updated", "tally", "tags", "names", "branch"]
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Look {
    pub hidden: Option<Vec<String>>,
    pub hide_deferred: Option<bool>,
    pub sort: Option<String>,
    pub sort_reversed: Option<bool>,
    pub fields: Option<Vec<String>>,
    /// **적은 쪽이 알던 열 전부**(moai-3fnf 리뷰, 사용자 결정 2026-09-15). `fields` 는 켠 것만 담아
    /// "안 적혔다" 가 "껐다" 와 "그 열을 몰랐다" 둘 다를 뜻했다 — 그래서 새 열이 옛 설정을 가진
    /// 사람에게 영영 안 떴다. 이 목록에 없는 열은 **탐색기의 기본값**으로 선다. 옛 설정에는 이 키가
    /// 없으니 새 열이 기본대로 서고, 한 번 적히고 나면 끈 열은 끈 채로 남는다.
    pub fields_known: Option<Vec<String>>,
    /// 오른쪽 상세 칸이 보이나(moai-ymnu).
    pub detail: Option<bool>,
}

/// 설정에서 보기만 읽는다. 파일이 없으면 빈 `Look` 이고 문제도 아니다. 깨진 파일은 까닭 한 줄 —
/// 탐색기는 그래도 처음값으로 뜬다. **시험만 부른다** — 띄우는 길은 [`read`] 한 번으로 층과 보기를 함께 얻는다.
#[cfg(test)]
pub fn read_look(path: Option<&Path>) -> (Look, Vec<String>) {
    let reg = read(path);
    (reg.look, reg.look_problems)
}

fn look_words(t: &dyn toml_edit::TableLike, key: &str, problems: &mut Vec<String>) -> Option<Vec<String>> {
    let a = look_one(t, key, "낱말 배열이어야", Item::as_array, problems)?;
    Some(
        a.iter()
            .filter_map(|v| {
                let w = v.as_str().map(String::from);
                if w.is_none() {
                    problems.push(format!("`{TUI}.{key}` 의 `{}` 는 낱말이 아니다 — 건너뛴다", v.to_string().trim()));
                }
                w
            })
            .collect(),
    )
}

/// 값 하나를 읽는다 — 없으면 `None`, 모양이 틀리면 `None` 과 까닭 한 줄(`want` 는 `낱말이어야` 처럼
/// "한다" 앞에 올 말). 낱말·참거짓·낱말 배열이 저마다 같은 틀을 적고 있었다(moai-u8cs, 배열은 moai-y61p 단계 리뷰).
/// `pick` 은 표 안의 값을 빌려 낼 수 있다(`Item::as_array`) — 그래서 수명을 표에 묶는다.
fn look_one<'a, T>(
    t: &'a dyn toml_edit::TableLike,
    key: &str,
    want: &str,
    pick: impl Fn(&'a Item) -> Option<T>,
    problems: &mut Vec<String>,
) -> Option<T> {
    let item = t.get(key)?;
    let v = pick(item);
    if v.is_none() {
        problems.push(format!("`{TUI}.{key}` 는 {want} 한다 — 지금은 {}", item.type_name()));
    }
    v
}

/// 낱말 배열 하나에 이 세션이 바꾼 만큼만 옮긴다(`Doc::merge_look`). 바뀐 것이 있으면 참.
///
/// 배열이면 **제자리에서** 고친다 — 뺀 낱말(겹쳐 적힌 것까지)을 빼고 더한 낱말을 끝에 더한다. 낱말이
/// 아닌 원소(새 바이너리의 모양)와 여러 줄로 벌인 모양은 그대로다. 배열이 아니거나 없으면 이 세션이 그
/// 키를 바꿨으니 `new` 를 새로 적는다.
fn merge_words(t: &mut dyn toml_edit::TableLike, key: &str, base: Option<&[String]>, new: Option<&[String]>) -> bool {
    if base == new {
        return false;
    }
    let Some(new) = new else {
        return t.remove(key).is_some();
    };
    if t.get(key).and_then(Item::as_array).is_none() {
        return put_value(t, key, Some(new.iter().map(String::as_str).collect::<toml_edit::Array>().into()));
    }
    let base = base.unwrap_or_default();
    let words = t.get_mut(key).and_then(Item::as_array_mut).expect("방금 배열인 것을 봤다");
    let before = words.len();
    words.retain(|v| !v.as_str().is_some_and(|w| base.iter().any(|b| b == w) && !new.iter().any(|n| n == w)));
    let mut changed = words.len() != before;
    for w in new {
        if base.contains(w) || words.iter().any(|v| v.as_str() == Some(w.as_str())) {
            continue;
        }
        words.push(w.as_str());
        changed = true;
    }
    changed
}

/// 값 하나를 적는다(`Doc::merge_look`). 같은 값이면 안 적고, 바뀐 것이 있으면 참. `None` 이면 키를 지운다.
///
/// **키는 안 건드리고 값만 바꾼다.** 키 위의 주석은 키의 꾸밈에 붙어 있어 `Table::insert` 로 갈아 끼우면
/// 지워진다(키 모양을 새로 짓는다). 값 뒤의 주석은 있던 값의 꾸밈에 붙어 있어 옮겨 단다 — `set_hue` 와
/// 같은 까닭이다.
fn put_value(t: &mut dyn toml_edit::TableLike, key: &str, v: Option<toml_edit::Value>) -> bool {
    let Some(mut v) = v else {
        return t.remove(key).is_some();
    };
    if !t.contains_key(key) {
        t.insert(key, Item::Value(v));
        return true;
    }
    let item = t.get_mut(key).expect("방금 있는 것을 봤다");
    if let Some(old) = item.as_value() {
        let same = match (old, &v) {
            (toml_edit::Value::String(a), toml_edit::Value::String(b)) => a.value() == b.value(),
            (toml_edit::Value::Boolean(a), toml_edit::Value::Boolean(b)) => a.value() == b.value(),
            _ => false,
        };
        if same {
            return false;
        }
        *v.decor_mut() = old.decor().clone();
    }
    *item = Item::Value(v);
    true
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

/// 항목 표 하나에서 정한 색을 읽는다. 없거나 `auto` 면 `None`.
fn entry_hue(t: &Table) -> Result<Option<Hue>, String> {
    match t.get(COLOR) {
        None if t.contains_key(COLOUR) => Err(format!("`{COLOUR}` 는 모르는 키다 — `{COLOR}` 로 적는다")),
        None => Ok(None),
        Some(item) => match item.as_str() {
            Some(s) => hue_choice(s).map_err(|e| format!("`{COLOR}`: {e}")),
            None => Err(format!("`{COLOR}` 는 문자열이어야 한다 — 지금은 {}", item.type_name())),
        },
    }
}

/// 사람이 적은 색 낱말 → 정한 색. `auto` 는 정하지 않은 것(`None`)이다.
///
/// 설정 읽기와 `moai project color` 가 **같은 자로 잰다** — 명령이 받은 값을 읽기가 틀렸다고
/// 하거나 그 반대면, 고친 대로 적었는데 또 알림이 선다. 받는 이름은 `style` 이 댄다.
pub fn hue_choice(word: &str) -> Result<Option<Hue>, String> {
    if word == AUTO {
        return Ok(None);
    }
    Hue::named(word).map(Some).ok_or_else(|| {
        format!("{word:?} 는 프로젝트 색이 아니다 — {} 중 하나, 또는 {AUTO}", Hue::names().join("·"))
    })
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
///
/// 푼 철자는 **글자로 정리하기 전의** 경로에서도 얻는다. `link/..` 을 등록할 때
/// [`resolve_dir`] 는 링크를 먼저 풀고 `..` 을 따르지만(링크 대상의 부모), 글자
/// 정리는 `..` 이 링크를 먼저 지워 다른 디렉터리가 된다 — 그 철자로는 못 뺀다.
///
/// **사라진 디렉터리도 위쪽의 링크는 푼다.** 등록은 푼 경로로 적히므로, 조상에
/// 링크가 있는 자리(macOS 의 `/tmp` → `/private/tmp`, 링크로 건 홈)에서 디렉터리가
/// 사라지면 글자 철자로도 통째 `canonicalize` 로도 안 맞는다 — 아직 있는 가장 깊은
/// 조상을 풀고 남은 조각을 붙인다.
///
/// **준 철자 그대로도 댄다.** 손으로 적은 `/w/a/../b` 는 글자 정리로도 링크 풀기로도
/// 그 철자가 안 나온다 — TUI 의 `d` 는 층의 줄에 적힌 철자를 그대로 주는데, 그것으로
/// 못 찾으면 줄은 남은 채 "이미 목록에 없다" 고 말한다. 맨 뒤에 붙여 대표 철자
/// (`out[0]`, 글자로 정리한 것)는 그대로 둔다. **이 목록은 `rm`·`color` 가 함께
/// 쓴다** — 한쪽에만 철자를 더하면 `rm` 이 빼는 줄을 `color` 가 "등록돼 있지 않다"
/// 고 거절하고, 거절문이 시키는 `add` 가 같은 디렉터리의 둘째 줄을 만든다.
pub fn spellings(input: &Path, cwd: &Path) -> Vec<PathBuf> {
    let joined = cwd.join(input);
    let lexical = lexical(&joined);
    let mut out = vec![lexical.clone()];
    let more = [std::fs::canonicalize(&joined).ok(), real_prefix(&lexical), Some(joined)];
    for one in more.into_iter().flatten() {
        if !out.contains(&one) {
            out.push(one);
        }
    }
    out
}

/// 같은 디렉터리인가. 철자가 같으면 그만이고, 아니면 링크를 풀어 견준다.
///
/// **등록은 푼 경로로 적히지만**(`resolve_dir`) 손으로 적은 줄이나 옛 바이너리가 적은
/// 줄은 링크 철자일 수 있다. 등록 목록에 이미 있는지를 묻는 자리는 전부 이것을 지난다 —
/// 글자로만 견주면 `/w/link` 가 적힌 목록에 `/w/real` 이 또 실려 같은 저장소가 두 줄로
/// 선다(TUI 의 고르기 창은 이미 링크를 풀어 `✓ 등록됨` 을 달아 놓고 있다).
pub fn same_dir(a: &Path, b: &Path) -> bool {
    a == b || matches!((std::fs::canonicalize(a), std::fs::canonicalize(b)), (Ok(x), Ok(y)) if x == y)
}

/// 아직 있는 가장 깊은 조상을 풀고 남은 조각을 그대로 붙인다. `..` 이 없는
/// (글자로 정리한) 경로를 받는다 — 남은 조각에 `..` 이 있으면 풀린 뒤의 뜻이 달라진다.
fn real_prefix(p: &Path) -> Option<PathBuf> {
    let mut rest = Vec::new();
    let mut cur = p;
    loop {
        if let Ok(real) = std::fs::canonicalize(cur) {
            return Some(rest.iter().rev().fold(real, |acc, c| acc.join(c)));
        }
        rest.push(cur.file_name()?);
        cur = cur.parent()?;
    }
}

/// 목록에 보일 이름 — 디렉터리 이름이고, **겹치는 것끼리만** 위 조각을 하나씩
/// 더 붙인다. `repo/apps/a` 와 `lib/a` 는 `apps/a`·`lib/a` 로, 겹치지 않는
/// `argos` 는 `argos` 그대로 선다.
///
/// **파일에 적지 않는 파생값이다.** 이름은 목록 전체에서 정해져, 프로젝트 하나를
/// 더하면 옆 줄의 이름이 바뀔 수 있다 — 적어 두면 더할 때마다 남의 줄을 고쳐
/// 써야 한다. 파일 시스템을 안 보는 순수 함수라 `ls`·한눈 보기·TUI 가 같은 자를 쓴다.
///
/// 조각을 다 붙여도 겹치면(한쪽이 다른 쪽의 꼬리일 때) 짧은 쪽이 먼저 바닥나
/// 멈추고 긴 쪽이 계속 자라므로 끝난다. 경로는 [`Doc::projects`] 가 이미
/// 한 번씩만 내므로 끝까지 겹치는 둘은 없다.
pub fn names(projects: &[Project]) -> Vec<String> {
    let parts: Vec<Vec<String>> = projects
        .iter()
        .map(|p| {
            p.path
                .components()
                .filter_map(|c| match c {
                    Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
                    _ => None,
                })
                .collect()
        })
        .collect();
    let name = |i: usize, k: usize| -> String {
        let c = &parts[i];
        match c.len() {
            // 뿌리(`/`)는 조각이 없다. 경로 그대로가 이름이다.
            0 => projects[i].path.display().to_string(),
            n => c[n - k.min(n)..].join("/"),
        }
    };
    let mut depth = vec![1usize; projects.len()];
    loop {
        let now: Vec<String> = (0..projects.len()).map(|i| name(i, depth[i])).collect();
        let mut grew = false;
        for i in 0..projects.len() {
            let clash = now.iter().enumerate().any(|(j, n)| j != i && *n == now[i]);
            if clash && depth[i] < parts[i].len() {
                depth[i] += 1;
                grew = true;
            }
        }
        if !grew {
            return now;
        }
    }
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

    /// 시험 하나의 임시 디렉터리. **놓을 때 지운다** — 이름에 pid 가 들어 돌 때마다 새로
    /// 서므로, 안 지우면 `cargo test` 한 번마다 시험 수만큼 `/tmp` 에 쌓인다(층·등록 시험의
    /// `Scratch` 와 같다). 패닉으로 끝나도 `Drop` 이 돈다.
    struct Scratch(PathBuf);

    impl std::ops::Deref for Scratch {
        type Target = Path;
        fn deref(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// **돌려받은 것을 묶어 둔다** — `scratch(..).canonicalize()` 처럼 곧바로 흘리면
    /// 그 줄 끝에서 디렉터리가 지워진다.
    fn scratch(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("moai-user-config-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
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
        assert_eq!(reg.projects, [Project { path: "/good".into(), hue: None }]);
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

    /// **색이 틀린 항목은 알리되 서 있다** — 경로로 고른 색으로. 틀린 철자 `colour` 도
    /// 알린다. `auto` 는 정하지 않은 것이라 알릴 것이 없다.
    #[test]
    fn a_bad_colour_is_reported_and_the_project_still_stands() {
        let src = "[[project]]\npath = \"/ok\"\ncolor = \"green\"\n\n\
                   [[project]]\npath = \"/red\"\ncolor = \"red\"\n\n\
                   [[project]]\npath = \"/num\"\ncolor = 3\n\n\
                   [[project]]\npath = \"/typo\"\ncolour = \"blue\"\n\n\
                   [[project]]\npath = \"/auto\"\ncolor = \"auto\"\n\n\
                   [[project]]\npath = \"/case\"\ncolor = \"Green\"\n";
        let (projects, problems) = Doc::parse(src).unwrap().projects();
        let got: Vec<(&str, Option<&str>)> =
            projects.iter().map(|p| (p.path.to_str().unwrap(), p.hue.map(Hue::name))).collect();
        assert_eq!(
            got,
            [("/ok", Some("green")), ("/red", None), ("/num", None), ("/typo", None), ("/auto", None), ("/case", None)]
        );
        assert_eq!(problems.len(), 4, "{problems:#?}");
        assert!(problems[0].starts_with("2번째") && problems[0].contains("cyan·green·blue"), "{problems:#?}");
        assert!(problems[1].contains("문자열"), "{problems:#?}");
        assert!(problems[2].contains("`colour` 는 모르는 키다"), "{problems:#?}");
        assert!(problems[3].contains("\"Green\""), "{problems:#?}");
        assert!(problems.iter().all(|p| p.contains("지금은 경로로 고른 색을 쓴다") && !p.contains('\n')), "{problems:#?}");

        // `color` 와 `colour` 가 함께 있으면 `color` 가 서고, `colour` 는 조용히 버리지 않고 알린다.
        let both = "[[project]]\npath = \"/both\"\ncolor = \"green\"\ncolour = \"blue\"\n";
        let (projects, problems) = Doc::parse(both).unwrap().projects();
        assert_eq!(projects[0].hue, Hue::named("green"));
        assert_eq!(problems.len(), 1, "{problems:#?}");
        assert!(problems[0].contains("`colour` 는 모르는 키다"), "{problems:#?}");
    }

    /// 색을 정했다 `auto` 로 되돌리면 처음 바이트로 돌아온다. 같은 색을 다시 정하면 파일을
    /// 안 건드린다. 모르는 키·주석과 **값 뒤의 주석**은 살아남는다. 등록 안 된 경로는 0 이다.
    #[test]
    fn setting_a_colour_and_back_to_auto_restores_the_bytes() {
        let d = scratch("hue");
        let path = d.join("config.toml");
        let src = "# 내 설정\n[[project]]\npath = \"/a\"   # 일\nalias = \"일\"\n\n[[project]]\npath = \"/b\"\n\n[ui]\nx = 1\n";
        std::fs::write(&path, src).unwrap();
        let green = Hue::named("green");

        assert_eq!(update(&path, |doc| Ok(doc.set_hue(&["/a".into()], green))).unwrap(), 1);
        let after = std::fs::read_to_string(&path).unwrap();
        assert!(after.contains("color = \"green\"") && after.contains("alias = \"일\"") && after.contains("[ui]"), "{after}");
        assert_eq!(read(Some(&path)).projects[0].hue, green);
        assert_eq!(read(Some(&path)).projects[1].hue, None, "남의 줄에 색이 붙었다");

        let before = std::fs::metadata(&path).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert_eq!(update(&path, |doc| Ok(doc.set_hue(&["/a".into()], green))).unwrap(), 1);
        assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), before, "같은 색인데 다시 썼다");

        assert_eq!(update(&path, |doc| Ok(doc.set_hue(&["/a".into()], None))).unwrap(), 1);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), src, "auto 로 되돌렸는데 바이트가 다르다");
        assert_eq!(update(&path, |doc| Ok(doc.set_hue(&["/없음".into()], green))).unwrap(), 0);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), src);

        // 값을 바꿀 때 그 값 뒤의 주석은 들고 간다.
        std::fs::write(&path, "[[project]]\npath = \"/a\"\ncolor = \"blue\"  # 회사 것\n").unwrap();
        update(&path, |doc| Ok(doc.set_hue(&["/a".into()], Hue::named("cyan")))).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "[[project]]\npath = \"/a\"\ncolor = \"cyan\"  # 회사 것\n");
    }

    /// 명령이 받는 낱말과 읽기가 받는 낱말이 같다.
    #[test]
    fn hue_choice_takes_palette_names_and_auto_only() {
        assert_eq!(hue_choice("blue").unwrap(), Hue::named("blue"));
        assert_eq!(hue_choice(AUTO).unwrap(), None);
        let e = hue_choice("magenta").unwrap_err();
        assert!(e.contains("\"magenta\"") && e.contains("cyan·green·blue") && e.contains(AUTO), "{e}");
    }

    /// **보기는 `[tui]` 에 적히고 도로 읽히며, 남의 키·주석·등록은 그대로다**(moai-2bzp). 파일이 이미 그
    /// 보기면 파일을 안 건드린다 — 토글마다 적으므로 헛 쓰기가 없어야 한다.
    #[test]
    fn a_look_round_trips_and_leaves_the_rest_alone() {
        let d = scratch("look");
        let path = d.join("config.toml");
        std::fs::write(&path, "# 내 설정\n[[project]]\npath = \"/a\"\n\n[tui]\nextra = 1  # 남의 키\n").unwrap();
        let look = Look {
            hidden: Some(vec!["done".into(), "review".into()]),
            hide_deferred: Some(true),
            sort: Some("updated".into()),
            sort_reversed: Some(false),
            fields: Some(vec!["id".into(), "assignee".into()]),
            fields_known: None,
            detail: Some(false),
        };
        update(&path, |doc| doc.merge_look(&Look::default(), &look)).unwrap();
        let (back, problems) = read_look(Some(&path));
        assert_eq!((back, problems), (look.clone(), Vec::new()));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# 내 설정") && text.contains("path = \"/a\"") && text.contains("extra = 1  # 남의 키"), "{text}");

        // 이 세션이 바꾼 것이 파일에 이미 있으면(옆에서 같게 적었으면) 파일을 안 건드린다.
        let before = std::fs::metadata(&path).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        update(&path, |doc| doc.merge_look(&Look::default(), &look)).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), before, "같은 보기를 다시 적었다");

        // 없는 값으로 바꾼 키는 지운다.
        update(&path, |doc| doc.merge_look(&look, &Look { sort: Some("title".into()), ..Look::default() })).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("hidden") && text.contains("sort = \"title\"") && text.contains("extra = 1"), "{text}");
    }

    /// **한 번의 읽기가 등록과 보기를 함께 낸다**(moai-u8cs) — 보기의 까닭은 따로 읽던 때의 글 그대로 보기에만
    /// 서고, 못 읽었거나 깨진 파일의 까닭은 층(`problems`)과 보기(`look_problems`) 둘 다에 선다. **글자로 견준다**
    /// (moai-y61p 단계 리뷰) — `read_look` 과 견주면 `read` 를 저 자신과 견주는 셈이라 아무것도 못 잡는다.
    #[test]
    fn one_read_gives_the_projects_and_the_look() {
        let d = scratch("one-read");
        let path = d.join("config.toml");
        std::fs::write(&path, "[[project]]\npath = \"/a\"\n\n[tui]\nsort = \"title\"\nhide_deferred = 3\n").unwrap();
        let reg = read(Some(&path));
        assert_eq!(reg.projects.len(), 1);
        assert_eq!(reg.look.sort.as_deref(), Some("title"));
        assert_eq!(reg.look_problems, [format!("{}: `tui.hide_deferred` 는 true·false 여야 한다 — 지금은 integer", path.display())]);
        assert!(reg.problems.is_empty(), "보기의 까닭이 층으로 샜다: {:?}", reg.problems);

        std::fs::write(&path, "[[project]\n").unwrap();
        let broken = read(Some(&path));
        assert_eq!(broken.problems.len(), 1, "{broken:?}");
        assert_eq!(broken.look_problems, broken.problems, "깨진 파일의 까닭이 층과 보기에 같게 안 섰다");
        // 못 여는 자리(디렉터리)도 같다 — 없는 파일(NotFound)만 문제가 아니다.
        let unreadable = read(Some(&d));
        assert!(unreadable.problems.len() == 1 && unreadable.problems[0].starts_with(&format!("{}: ", d.display())), "{unreadable:?}");
        assert_eq!(unreadable.look_problems, unreadable.problems, "못 읽은 파일의 까닭이 층과 보기에 같게 안 섰다");
        assert_eq!(read(None).look_problems, Vec::<String>::new(), "자리를 모르는 것은 보기의 문제가 아니다");
    }

    /// **언어는 `[i18n] lang` 에서 읽는다**(moai-slfv) — 사람의 설정이지 프로젝트의 것이 아니다.
    /// 없거나 틀린 값이면 `None` 이고, 고르는 쪽이 영어로 떨어진다. 여기서 멈추지 않는 것은
    /// 글자 하나 때문에 도구가 안 도는 셈이기 때문이다.
    #[test]
    fn the_language_is_read_from_the_i18n_table() {
        let lang = |src: &str| Doc::parse(src).unwrap().lang();
        assert_eq!(lang("[i18n]\nlang = \"ko\"\n"), (Some("ko".to_string()), vec![]));
        assert_eq!(lang("[[project]]\npath = \"/a\"\n"), (None, vec![]), "표가 없으면 없는 것이다");
        // **틀린 값은 알린다** — 조용히 영어가 되면 고친 설정이 왜 안 듣는지 알 길이 없다.
        assert_eq!(lang("[i18n]\nlang = 3\n").1, ["`i18n.lang` 은 낱말이어야 한다 — 지금은 integer"]);
        assert_eq!(lang("i18n = 3\n").1, ["`i18n` 은 `[i18n]` 표여야 한다 — 지금은 integer"]);
        assert_eq!(lang("[i18n]\nlang = \"kr\"\n").1, ["`i18n.lang` 은 en·ko·zh·ja·es 중 하나다 — \"kr\""]);
        assert!(lang("[i18n]\nlang = \"kr\"\n").0.is_none(), "모르는 코드를 값으로 들였다");
        let reg = read(None);
        assert_eq!(reg.lang, None, "자리를 모르면 언어도 없다");
    }

    /// **틀린 보기 키는 알리고 나머지는 읽는다**(moai-2bzp). `tui` 가 표가 아니면 읽기는 비고 쓰기는 멈춘다.
    #[test]
    fn a_bad_look_key_is_reported_and_the_rest_still_reads() {
        let doc = Doc::parse("[tui]\nhidden = \"done\"\nsort = 3\nfields = [\"id\", 7]\nhide_deferred = true\n").unwrap();
        let (look, problems) = doc.look();
        assert_eq!(look.hidden, None);
        assert_eq!(look.sort, None);
        assert_eq!(look.fields, Some(vec!["id".to_string()]));
        assert_eq!(look.hide_deferred, Some(true));
        // 까닭 글은 판독기를 한 틀(`look_one`)로 모으기 전의 글 그대로다 — 글자로 박는다(moai-y61p 단계 리뷰).
        assert_eq!(
            problems,
            [
                "`tui.hidden` 는 낱말 배열이어야 한다 — 지금은 string",
                "`tui.sort` 는 낱말이어야 한다 — 지금은 integer",
                "`tui.fields` 의 `7` 는 낱말이 아니다 — 건너뛴다",
            ]
        );

        let title = Look { sort: Some("title".into()), ..Look::default() };
        let mut odd = Doc::parse("tui = 3\n").unwrap();
        assert_eq!(odd.look().1.len(), 1);
        assert!(odd.merge_look(&Look::default(), &title).is_err());
        assert!(!odd.changed());

        // 읽히는 인라인 표는 쓰기도 받는다.
        let mut inline = Doc::parse("tui = { sort = \"created\" }\n").unwrap();
        assert_eq!(inline.look().0.sort.as_deref(), Some("created"));
        inline.merge_look(&Look::default(), &title).unwrap();
        assert!(inline.render().contains("sort = \"title\""), "{}", inline.render());
    }

    /// **보기는 이 세션이 바꾼 만큼만 적힌다**(moai-2kyl 단계 리뷰). 같은 설정에서 뜬 두 탐색기가 저마다
    /// 다른 것을 눌러도 둘 다 남고, 안 바꾼 키는 주석·여러 줄 배열·모르는 모양까지 그대로다. 바꾼 값도
    /// 값 뒤 주석은 들고 간다.
    #[test]
    fn a_look_merge_keeps_what_others_wrote_and_the_comments() {
        let d = scratch("look-merge");
        let path = d.join("config.toml");
        std::fs::write(
            &path,
            "[tui]\n# 끝난 일은 늘 숨긴다\nhidden = [\n  \"done\",\n]\nsort = \"updated\"  # 새것 먼저\nfields = [\"id\", { name = \"estimate\" }]\nwidth = { list = 40 }\n",
        )
        .unwrap();
        // 둘 다 같은 파일을 읽고 떴다 — 화면의 말로 든 보기다.
        let base = Look {
            hidden: Some(vec!["done".into()]),
            hide_deferred: Some(false),
            sort: Some("updated".into()),
            sort_reversed: Some(false),
            fields: Some(vec!["id".into()]),
            fields_known: None,
            detail: Some(true),
        };
        let a = Look { fields: Some(vec!["id".into(), "assignee".into()]), ..base.clone() };
        let b = Look { hide_deferred: Some(true), ..base.clone() };
        update(&path, |doc| doc.merge_look(&base, &a)).unwrap();
        update(&path, |doc| doc.merge_look(&base, &b)).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let (back, _) = read_look(Some(&path));
        assert_eq!(back.fields, Some(vec!["id".to_string(), "assignee".to_string()]), "옆 탐색기가 켠 열을 지웠다\n{text}");
        assert_eq!(back.hide_deferred, Some(true), "{text}");
        for kept in ["# 끝난 일은 늘 숨긴다", "hidden = [\n  \"done\",\n]", "sort = \"updated\"  # 새것 먼저", "{ name = \"estimate\" }", "width = { list = 40 }"] {
            assert!(text.contains(kept), "안 바꾼 `{kept}` 가 달라졌다\n{text}");
        }

        // 바꾼 것은 값만 바뀐다 — 뒤 주석과 키 위 주석은 남고, 낱말 배열은 뺀 낱말만 빠진다.
        let c = Look { hidden: Some(Vec::new()), sort: Some("title".into()), ..b.clone() };
        update(&path, |doc| doc.merge_look(&b, &c)).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("sort = \"title\"  # 새것 먼저") && text.contains("# 끝난 일은 늘 숨긴다"), "{text}");
        assert_eq!(read_look(Some(&path)).0.hidden, Some(Vec::new()), "{text}");

        // **`fields_known` 은 더하기만 한다**(moai-6bc0 단계 리뷰) — 새 바이너리가 적어 둔 열 이름을
        // 이쪽이 지우면, 그쪽의 다음 실행이 제가 적어 둔 열을 "몰랐던 열" 로 읽어 사람이 끈 것을
        // 도로 켠다. 이 바이너리가 아는 목록은 세션 내내 같은 값이라 base 와 견줄 자가 없다.
        let newer = Look { fields_known: Some(vec!["id".into(), "estimate".into()]), ..c.clone() };
        update(&path, |doc| doc.merge_look(&c, &newer)).unwrap();
        let older = Look { fields_known: Some(vec!["id".into(), "priority".into()]), ..c.clone() };
        update(&path, |doc| doc.merge_look(&newer, &older)).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let known = read_look(Some(&path)).0.fields_known.unwrap_or_default();
        assert!(known.contains(&"estimate".to_string()), "옆 바이너리가 아는 열을 지웠다\n{text}");
        assert!(known.contains(&"priority".to_string()), "이 바이너리가 아는 열을 안 적었다\n{text}");
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
        assert_eq!(read(Some(&path)).projects, [Project { path: "/b".into(), hue: None }]);
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
        let s = scratch("resolve");
        let d = s.canonicalize().unwrap();
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
            // `link/..` 은 링크 대상의 부모로 등록된다 — 뺄 때도 그 철자가 나와야 한다.
            std::fs::create_dir_all(d.join("else/real")).unwrap();
            std::fs::create_dir_all(d.join("cwd")).unwrap();
            std::os::unix::fs::symlink(d.join("else/real"), d.join("cwd/up")).unwrap();
            let registered = resolve_dir(Path::new("up/.."), &d.join("cwd")).unwrap();
            assert_eq!(registered, d.join("else"));
            assert!(spellings(Path::new("up/.."), &d.join("cwd")).contains(&registered), "등록한 철자로 못 뺀다");
            // 사라진 디렉터리라도 위쪽 링크는 푼다 — 등록은 푼 경로로 적혔다.
            assert_eq!(spellings(Path::new("link/gone"), &d), [d.join("link/gone"), d.join("real/gone")]);
        }
        // 사라진 디렉터리도 글자로는 찾는다. 준 철자 그대로도 뒤에 선다 — 손으로
        // `/w/a/../b` 라 적힌 줄은 그 철자로만 찾을 수 있다(`rm`·`color` 가 함께 쓴다).
        assert_eq!(spellings(Path::new("gone/../gone2"), &d), [d.join("gone2"), d.join("gone/../gone2")]);
        assert_eq!(spellings(Path::new("gone/../gone2"), &d)[0], d.join("gone2"), "대표 철자가 밀렸다");
        assert_eq!(lexical(Path::new("/../a")), PathBuf::from("/a"));
    }

    /// 이름은 디렉터리 이름이고, 겹치는 것끼리만 위 조각이 붙는다.
    #[test]
    fn names_grow_only_where_they_clash() {
        let ps = |paths: &[&str]| paths.iter().map(|p| Project { path: p.into(), hue: None }).collect::<Vec<_>>();
        assert_eq!(names(&ps(&["/w/argos", "/r/apps/a", "/r/libs/a"])), ["argos", "apps/a", "libs/a"]);
        // 둘째 조각까지 같으면 셋째까지. 겹치지 않는 줄은 안 자란다.
        assert_eq!(names(&ps(&["/x/apps/a", "/y/apps/a", "/y/b"])), ["x/apps/a", "y/apps/a", "b"]);
        // 한쪽이 다른 쪽의 꼬리면 짧은 쪽이 바닥나고 긴 쪽이 자란다.
        assert_eq!(names(&ps(&["/a", "/z/a"])), ["a", "z/a"]);
        assert_eq!(names(&ps(&["/", "/a"])), ["/", "a"]);
        assert!(names(&[]).is_empty());
    }

    /// 설정 파일이 링크면 링크는 링크로 남고 가리키는 파일이 고쳐진다.
    #[cfg(unix)]
    #[test]
    fn a_symlinked_config_stays_a_symlink() {
        let s = scratch("symlink");
        let d = s.canonicalize().unwrap();
        std::fs::create_dir_all(d.join("dots")).unwrap();
        std::fs::write(d.join("dots/config.toml"), "# dotfiles\n").unwrap();
        let link = d.join("config.toml");
        std::os::unix::fs::symlink(d.join("dots/config.toml"), &link).unwrap();

        assert!(update(&link, |doc| doc.add(Path::new("/a"))).unwrap());
        assert!(std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink(), "링크를 보통 파일로 갈아끼웠다");
        assert_eq!(read(Some(&d.join("dots/config.toml"))).projects, [Project { path: "/a".into(), hue: None }]);
        // **락 파일은 양쪽에 선다.** 한때 준 철자 곁에만 두어 dotfiles 저장소를 깨끗이
        // 했는데, 그러면 같은 파일을 두 철자로 부른 둘이 서로 다른 락을 잡아 아무도
        // 막지 않았다(아래 시험이 그것을 잰다). 추적 안 된 파일 하나가 조용한 손실보다 싸다.
        assert!(d.join("config.toml.lock").exists(), "준 철자 곁의 락이 없다");
        assert!(d.join("dots/config.toml.lock").exists(), "푼 자리의 락이 없다 — 두 철자가 서로를 안 막는다");
    }

    /// **한 파일을 두 철자로 불러도 서로를 막는다.** 설정이 링크면(dotfiles 저장소가 흔히
    /// 그렇게 건다) 한쪽은 `~/.config/…`, 한쪽은 `~/dotfiles/…` 로 같은 파일을 고친다 —
    /// 준 철자 곁의 락만 잡으면 둘이 다른 파일을 잡아 나중에 `rename` 한 쪽이 앞의 등록을
    /// 지운다. 재 보면 스무 개 중 열 개가 사라졌다.
    #[cfg(unix)]
    #[test]
    fn two_spellings_of_one_config_still_lock_each_other() {
        let s = scratch("two-spellings");
        let d = s.canonicalize().unwrap();
        std::fs::create_dir_all(d.join("dots")).unwrap();
        let real = d.join("dots/config.toml");
        std::fs::write(&real, "").unwrap();
        let link = d.join("config.toml");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let (threads, each) = (8, 5);
        std::thread::scope(|s| {
            for t in 0..threads {
                // 짝수 갈래는 링크 철자로, 홀수 갈래는 푼 철자로 — 같은 파일이다.
                let path = if t % 2 == 0 { link.clone() } else { real.clone() };
                s.spawn(move || {
                    for i in 0..each {
                        let dir = PathBuf::from(format!("/w/t{t}-{i}"));
                        update(&path, |doc| doc.add(&dir)).unwrap();
                    }
                });
            }
        });
        assert_eq!(read(Some(&real)).projects.len(), threads * each, "동시 등록이 서로를 지웠다");
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
