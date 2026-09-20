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
use crate::store::{Lock, dir_of, lock_beside};
use crate::style::Hue;
use std::collections::BTreeMap;
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
    /// 그 언어를 읽다 만난 까닭(리뷰 moai-80qw). **`problems` 에도 같이 든다** — 대는 자리가
    /// 둘이라서다. `.moai` 밖의 한눈 보기는 `problems` 로 대지만, 저장소 **안**의 `moai status`
    /// 는 등록 목록을 아예 안 읽으므로(`cmd::status` 의 결정 3) 이 자리로만 닿는다. 하나로
    /// 줄이면 두 화면 중 하나가 이 줄을 잃고, 그러면 틀린 설정이 조용히 영어가 된다 —
    /// 그것을 막자는 것이 [`Doc::lang`] 의 까닭이었다.
    pub lang_problems: Vec<String>,
    /// 적어 둔 탐색기 보기와 그것을 읽다 만난 까닭(moai-2bzp). **같은 파싱에서 함께 읽는다**(moai-u8cs) —
    /// 탐색기를 띄우면 층과 보기가 저마다 파일을 읽고 파싱해 한 번 띄울 때 설정을 두세 번 읽었다.
    /// 까닭을 `problems` 와 따로 드는 것은 대는 자리가 달라서다 — 층의 문제는 층이, 보기의 문제는 알림이 댄다.
    pub look: Look,
    /// 보기·읽음을 읽다 만난 까닭. `problems`(층이 대는 것)와 따로 든다 — 대는 자리가 다르다. 파일을 못 읽었거나
    /// 깨진 까닭은 여기 없다 — 층만 댄다(moai-5jsn).
    pub look_problems: Vec<String>,
    /// 이슈 id → **내가 마지막으로 본 줄의 `updated_at`**(RFC3339, moai-50mn — 옛 바이너리는 본 때를 적었다,
    /// moai-lyc1). 여기 없는 줄은 한 번도 안 본 것이다.
    /// 트래커가 아니라 내 설정에 드는 까닭: 읽음은 사람마다 다른 값이라 `.moai/issues.jsonl` 에
    /// 적으면 읽기만 해도 남과 부딪히고, 남의 읽음이 내 diff 에 섞인다(사용자 결정 2026-09-15).
    pub read: BTreeMap<String, String>,
    /// 읽다 만난 탈. 멀쩡하면 `None` 이다. 까닭 글은 `problems` 에 있고 여기는 **그 탈의 갈래**뿐이다
    /// — 글로 가르면 말이 바뀔 때마다 가르는 쪽이 따라 깨진다.
    ///
    /// **없는 파일도 갈래는 선다**([`Trouble::Gone`], moai-po6v) — `problems` 는 비는데 여기는 `Some`
    /// 이다. 그 둘을 한 물음으로 읽으면 안 된다: 사람에게 댈 까닭은 없지만("아직 아무것도 등록 안
    /// 했다" 는 정상이다) "없다" 와 "읽었더니 비었다" 를 가르는 자는 여기뿐이라, `trouble.is_none()`
    /// 을 "성한 설정" 으로 읽는 새 길은 끊긴 마운트를 빈 설정으로 들인다.
    pub trouble: Option<Trouble>,
}

/// 설정을 읽다 만난 탈의 갈래([`Registry::trouble`], moai-9p7v). **가르는 잣대는 다시 읽어 볼 값이다**
/// (moai-po6v) — 어느 단계에서 졌는가가 아니다. 한때 `read_to_string` 이 진 것은 모두 [`Trouble::Reading`]
/// 이라, 권한·디렉터리·UTF-8 아닌 바이트처럼 다시 해도 같은 것까지 걸음마다 다시 읽었다.
///
/// **들고 있던 것은 어느 갈래에서도 둔다**(사용자 결정 2026-09-19). 빈 것으로 갈아 끼우면 탐색기의 층이
/// 사라지고 읽음이 통째로 [NEW] 로 서는데, 그것을 막을 길이 도구 안에 없다(`SPC r` 은 moai-en4u 가
/// 걷었다). 까닭은 배너가 한 줄로 댄다. 갈리는 것은 **다시 읽는 때**뿐이고 그것이 [`Trouble::again`] 이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trouble {
    /// 파일을 못 읽었고 **잠깐일 것이다** — NFS 의 `ESTALE`·`EIO`, `Interrupted`. 다음 걸음에 다시 읽으면
    /// 지나간다.
    Reading,
    /// 파일을 못 읽었고 **다시 해도 같다** — 권한, 디렉터리를 설정 자리로 준 것, 심링크 고리,
    /// UTF-8 이 아닌 바이트. 이 가운데 권한만은 고쳐도 파일의 표식이 안 바뀌어(`chmod` 은 고친 때도
    /// 길이도 안 바꾼다) 표식으로는 영영 못 벗어난다 — 그래서 시계로 다시 본다. 층의 못 읽는 줄이
    /// 같은 까닭으로 시계에 걸린 그 자다(`Layer::stale`).
    Unreadable,
    /// 글이 깨졌다 — 파싱이 졌다. 사람이 고칠 때까지 다시 읽어도 같고, 고치면 파일이 바뀌어 표식이
    /// 그것을 낸다. 까닭을 대는 자리다.
    Broken,
    /// 파일이 없다. **사람에게 댈 까닭이 아니다** — 아직 아무것도 등록 안 한 사람의 정상이라 `problems`
    /// 에 한 줄도 안 선다. 그래도 갈래로 두는 것은 탐색기 때문이다(moai-po6v): autofs·sshfs 홈이 끊겨
    /// 설정이 사라진 자리를 "읽었더니 비었다" 로 들면 층도 읽음도 빈 채로 갈린다. 돌아오면 표식이
    /// 바뀌므로(`store::stamp` 은 없는 파일에 `None`) 다시 읽는 때는 그것이 정한다.
    Gone,
}

/// 진 읽기를 **언제 다시 해 볼 것인가**([`Trouble`], moai-po6v). 갈래마다 벗어나는 길이 다르다 —
/// 한 가지로 재면 잠깐의 실패가 영영 안 풀리거나(다시 안 읽는다), 다시 해도 같은 것을 걸음마다
/// 다시 읽는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Again {
    /// 다음 걸음에. 잠깐이라 곧 지나간다.
    Step,
    /// 시계로. 고쳐도 파일의 표식이 안 바뀌는 탈이라 표식만 보면 영영 못 벗어난다.
    Clock,
    /// 스스로는 안 한다. 고치면 파일이 바뀌어 표식이 그것을 낸다.
    Never,
}

impl Trouble {
    /// 이 탈을 언제 다시 읽어 볼 것인가. **이것이 갈래를 가르는 잣대다** — 갈래를 더하는 사람은
    /// 여기부터 답한다.
    pub fn again(self) -> Again {
        match self {
            Trouble::Reading => Again::Step,
            Trouble::Unreadable => Again::Clock,
            Trouble::Broken | Trouble::Gone => Again::Never,
        }
    }
}

/// 읽기가 진 까닭을 **다시 해 볼 값**으로 가른다(moai-po6v). 여기 안 든 갈래는 잠깐으로 본다 —
/// 모르는 것을 "다시 해도 같다" 로 두면 고쳐도 안 풀리는 쪽이 기본이 되고, 잠깐의 실패가 영영 안
/// 풀린다. 틀리는 쪽을 고르자면 헛 읽기 몇 번이 싸다.
pub fn unreadable(e: &std::io::Error) -> bool {
    use std::io::ErrorKind as E;
    matches!(e.kind(), E::PermissionDenied | E::IsADirectory | E::NotADirectory | E::InvalidData | E::InvalidInput)
        || Some(ELOOP) == e.raw_os_error()
}

/// 심링크 고리(`ELOOP`) — 고친 것이 파일을 바꾸므로 표식이 낸다. **번호로 든다**: 이것을 낱말로 드는
/// `ErrorKind::FilesystemLoop` 은 아직 안 여물었다(`io_error_more`, rust#86442 — 1.97 에서도 E0658 이다).
///
/// **아는 기계는 다 적는다**(리뷰) — 못 맞추면 `Trouble::Reading` 으로 떨어지고 그 갈래는
/// [`Again::Step`] 이라, 고리 하나가 "헛 읽기 몇 번" 이 아니라 **걸음마다 영영** 다시 읽는다.
/// 안드로이드는 cfg 에서 `linux` 가 아니고(번호는 같다), 애플과 BSD 는 62 로 한자다.
#[cfg(any(target_os = "linux", target_os = "android"))]
const ELOOP: i32 = 40;
#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]
const ELOOP: i32 = 62;
#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
)))]
const ELOOP: i32 = i32::MIN;

/// 설정을 관대하게 읽는다. 파일이 없으면 빈 목록이고 **사람에게 댈 까닭도 아니다** — 아직
/// 아무것도 등록하지 않은 사람의 정상이라 `problems` 에 한 줄도 안 선다. 다만 갈래는 남긴다
/// ([`Trouble::Gone`], moai-po6v): "없다" 와 "읽었더니 비었다" 를 못 가르면 끊긴 마운트가 빈 설정으로
/// 들어와, 탐색기가 들고 있던 층과 읽음을 빈 것으로 갈아 끼운다.
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
    // 못 읽은 것과 깨진 것을 가른다(moai-9p7v) — 앞의 것만 다시 해 볼 값이 있다([`Trouble`]).
    let parsed = match std::fs::read_to_string(path) {
        // **없는 파일은 까닭을 안 댄다**(아직 아무것도 등록 안 한 사람의 정상) — 갈래만 남긴다.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            reg.trouble = Some(Trouble::Gone);
            return reg;
        }
        Err(e) if unreadable(&e) => Err((e.to_string(), Trouble::Unreadable)),
        Err(e) => Err((e.to_string(), Trouble::Reading)),
        Ok(src) => Doc::parse(&src).map_err(|e| (e, Trouble::Broken)),
    };
    match parsed {
        Ok(doc) => {
            let (projects, problems) = doc.projects();
            reg.projects = projects;
            reg.problems = problems.into_iter().map(at).collect();
            let (lang, lang_problems) = doc.lang();
            reg.lang = lang;
            reg.lang_problems = lang_problems.into_iter().map(at).collect();
            reg.problems.extend(reg.lang_problems.iter().cloned());
            let (look, problems) = doc.look();
            reg.look = look;
            reg.look_problems = problems.into_iter().map(at).collect();
            let (read, problems) = doc.read_marks();
            reg.read = read;
            reg.look_problems.extend(problems.into_iter().map(at));
        }
        // 못 읽었거나 깨진 까닭은 **층만 댄다**(moai-5jsn). 보기에도 실으면 탐색기가 같은 파싱 오류를 층 없음
        // 배너와 보기 알림으로 두 번 댔다. 층은 늘 댄다 — 밖에서는 층 화면이, 안에서는 층을 못 세운 배너
        // (`App::attach_layer`)가. 보기는 처음값으로 뜨고, 그 뒤의 저장은 제 거절(`broken`)을 따로 댄다.
        Err((e, trouble)) => {
            reg.problems = vec![at(e)];
            reg.trouble = Some(trouble);
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
    let dir = dir_of(path);
    std::fs::create_dir_all(dir).map_err(|e| err(dir, e))?;
    let lock = Lock::acquire(&lock_beside(path))?;

    // 설정 파일이 심볼릭 링크면(dotfiles 저장소가 흔히 그렇게 건다) **링크가 가리키는
    // 파일을** 고친다. 링크 자리에 `rename` 하면 링크가 보통 파일로 갈아끼워져
    // dotfiles 쪽은 옛 내용에 멈추고, 사람은 그것을 모른다. 푸는 것은 락 **안에서**
    // 한다: 밖에서 풀면 그 사이에 파일이 링크로 갈아끼워질 수 있다. 가리키는 파일이 아직
    // 없어도 링크를 따라간다([`resolve_config`]).
    let resolved = resolve_config(path)?;
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
    //
    // **이미 쥔 파일이면 둘째는 없다**(moai-2l74). 링크가 파일이 아니라 위 디렉터리에 걸렸거나(`~/.config` 가
    // dotfiles 로 걸렸다, macOS 의 `/var` → `/private/var`) 락 파일 자체가 링크면(stow·rcm 은 dotfiles 안에 선 락까지
    // 건다) 두 철자의 락은 한 파일이라, 다시 잡으면 제가 쥔 락을 제가 기다리다 `locked` 로 물러난다 — 파일이 선
    // 뒤의 모든 쓰기가 그렇게 멈췄다. 견주는 것은 경로 글자가 아니라 **파일**이다([`Lock::holds`]) — 하드 링크나
    // 대소문자를 안 가르는 볼륨의 철자도 글자로는 못 가른다. 파일로 못 견주는 곳(unix 밖)은 위 디렉터리를 푼 철자로
    // 견준다. 파일이 아직 없으면(`resolved` 가 없다) 곁의 락이 곧 그 자리의 락이다.
    let held = |r: &Path| {
        lock.holds(&lock_beside(r)).unwrap_or_else(|| {
            std::fs::canonicalize(dir).ok().zip(path.file_name()).is_some_and(|(d, name)| d.join(name) == r)
        })
    };
    let _real_lock = match resolved.as_deref() {
        Some(r) if !held(r) => Some(Lock::acquire(&lock_beside(r))?),
        _ => None,
    };
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
    // **손으로 고칠 거절에는 어느 파일인지 붙인다**(moai-gmdu 에픽 리뷰) — 설정의 자리는 환경(`MOAI_CONFIG`·
    // XDG)이 골라 사람이 모를 수 있다. 문서(`Doc`)는 제 자리를 모르고 여기는 안다: 깨진 설정을 대는 위의
    // 거절문과 같은 모양으로 한 곳에서 붙인다. 부르는 쪽마다 붙이게 두면 붙인 곳(`project color`)과 잊은
    // 곳(`project add`·`rm`·`read`·탐색기)이 갈린다.
    let out = f(&mut doc).map_err(|e| match e.code {
        code::BROKEN => Fail::coded(format!("{}: {}", path.display(), e.message), e.code),
        _ => e,
    })?;
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

/// 손으로 적은 모양을 덮지 않는 거절(목록의 모양·`set_hue`·`merge_look`·읽음). **코드는 `broken` 이다**
/// (moai-3owm, 사용자 결정 2026-09-18) — 깨진 설정에 안 쓰는 것([`update`])과 같은 갈래다. 둘 다 사람이 설정 파일을
/// 손으로 고쳐야 쓴다는 뜻이라, 받는 쪽이 I/O 실패(`error`)와 가를 수 있어야 한다. 거절마다 한 코드를 내야 해서 한
/// 곳에 둔다. 어느 파일인지는 [`update`] 가 붙인다 — 문서는 제 자리를 모른다.
pub(crate) fn refuse(message: String) -> Fail {
    Fail::coded(message, code::BROKEN)
}

/// 설정 파일이 실제로 선 자리(`update`). 있으면 링크를 다 푼 경로다. **없는데 링크면 링크를 따라간 자리**다 — dotfiles 는
/// 링크를 먼저 걸고 파일은 첫 쓰기에 생기기도 하는데, `canonicalize` 는 없는 파일에서 실패해 그대로 두면 준 철자로
/// 떨어진다. 그러면 첫 쓰기가 링크 자리에 `rename` 해 링크를 보통 파일로 갈아끼우고(dotfiles 쪽 파일은 영영 안
/// 생긴다), 가리키는 철자로 쓰는 쪽과는 서로 다른 락을 잡는다. 링크가 아닌 없는 파일이면 `None` 이다 — 준 철자에
/// 새로 만든다.
///
/// 링크가 가리키는 자리의 디렉터리가 없으면 **쓰지 않는다**(`broken`). 아직 안 받은 dotfiles 저장소 자리에 디렉터리를
/// 지으면 뒤의 `git clone` 이 거기서 멈추고, 링크를 갈아끼우면 위의 손실이다 — 사람이 그 자리를 세울 때까지 멈춘다.
fn resolve_config(path: &Path) -> R<Option<PathBuf>> {
    match std::fs::canonicalize(path) {
        Ok(real) => return Ok(Some(real)),
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => {}
    }
    // 링크의 사슬을 끝까지 따라간다. 상대 대상은 그 링크가 든 디렉터리에서 잰다. 고리는 `canonicalize` 가 이미
    // `NotFound` 가 아닌 것으로 댔다 — 여기의 횟수 제한은 그래도 끝나게 하는 울타리다.
    let mut at = path.to_path_buf();
    for _ in 0..40 {
        let Ok(to) = std::fs::read_link(&at) else { break };
        at = dir_of(&at).join(to);
    }
    if at == path {
        return Ok(None);
    }
    match (std::fs::canonicalize(dir_of(&at)), at.file_name()) {
        (Ok(dir), Some(name)) => Ok(Some(dir.join(name))),
        _ => Err(refuse(format!(
            "{} 는 {} 를 가리키는데 그 디렉터리가 없다 — 링크를 갈아끼우지 않도록 쓰지 않는다. 그 자리를 세우거나 링크를 고친다",
            path.display(),
            at.display()
        ))),
    }
}

/// 읽어 들인 설정 문서. 모르는 키·표·주석은 문서가 들고 있다가 그대로 낸다.
pub struct Doc {
    doc: DocumentMut,
    bom: bool,
    /// 원문이 줄바꿈 없이 끝났나. 라이브러리는 렌더할 때마다 키·값 줄 끝에 줄바꿈을 세워
    /// 내는데(고친 것이 없어도 `a = 1` 이 `a = 1\n` 로 나온다), 그러면 손으로 적은 파일이
    /// 더했다 빼는 것만으로 한 바이트 자란다(moai-r9qa). 쓰기는 무엇이든(등록·색·보기·읽음)
    /// 이 렌더를 지나므로 여기서 한 번 뗀다. 끝 모양은 사람이 정한 것이라 그대로 돌려준다 —
    /// BOM 을 들고 가는 것과 같은 까닭이다.
    no_eol: bool,
    /// 원문의 줄 끝이 `\r\n` 쪽인가(moai-lb0u). 라이브러리는 렌더할 때 줄 끝을 `\n` 으로 접어(여러 줄 문자열
    /// 안만 빼고), CRLF 로 적은 설정이 등록·색·보기·읽음 한 번에 모든 줄이 바뀌었다. **많은 쪽을 따른다**
    /// (사용자 결정 2026-09-18) — 모두 CRLF 인 파일은 바이트가 지켜지고, 섞인 파일은 한 가지로 선다. 줄마다
    /// 지키는 것은 라이브러리가 접어 못 한다. 같으면 LF 다.
    ///
    /// **여러 줄 문자열 안의 줄바꿈은 줄 끝이 아니라 값이다**(moai-gmdu 에픽 리뷰) — 세지도 고치지도 않는다
    /// ([`multiline_strings`]). 세면 LF 파일에 붙여 넣은 CRLF 문자열 하나가 파일 전체를 CRLF 로 뒤집고, 고치면
    /// 모르는 키의 값이 바뀌고 이 도구가 적은 경로(줄바꿈이 든 디렉터리 이름)가 다른 디렉터리가 된다.
    crlf: bool,
    dirty: bool,
}

impl Doc {
    /// 파싱한다. TOML 로 못 읽는 것만 거절한다.
    ///
    /// **`project` 의 모양은 여기서 안 본다**(moai-aguj). 여기서 거절하면 `project = [{ … }]` 하나로 파일
    /// 전체가 깨진 것이 되어, 그 키와 상관없는 보기·읽음·언어까지 못 읽고 못 적는다 — 토글마다 '보기를
    /// 설정에 못 적었다' 가 섰다. 엄함은 지금 쓰는 줄에 대한 것이라, 모양은 그 키를 읽고 쓰는 자리
    /// ([`Doc::projects`]·[`Doc::add`]·[`Doc::remove`]·[`Doc::set_hue`])가 잰다.
    pub fn parse(src: &str) -> Result<Doc, String> {
        let (bom, body) = match src.strip_prefix('\u{feff}') {
            Some(rest) => (true, rest),
            None => (false, src),
        };
        let parsed = toml_edit::Document::parse(body).map_err(|e| {
            // 라이브러리의 오류는 여러 줄 그림이다. 한 줄로 접어야 목록 속 한 줄로 선다.
            e.to_string().split_whitespace().collect::<Vec<_>>().join(" ")
        })?;
        let no_eol = !body.is_empty() && !body.ends_with('\n');
        let (mut crlf_lines, mut lines) = (0, 0);
        for (outside, _) in split_strings(body, &multiline_strings(&parsed)) {
            crlf_lines += outside.matches("\r\n").count();
            lines += outside.matches('\n').count();
        }
        let crlf = crlf_lines > lines - crlf_lines;
        Ok(Doc { doc: parsed.into_mut(), bom, no_eol, crlf, dirty: false })
    }

    /// 고친 것이 있나 — [`update`] 가 쓸지 가르는 깃발 그대로다.
    pub fn changed(&self) -> bool {
        self.dirty
    }

    /// 뿌리 표 — **모르는 키를 읽는 문**이다. 읽음 파일([`crate::read_marks::Sheet`])이 제 키(`path`·
    /// `read`)를 여기서 본다.
    ///
    /// **문서를 통째로 내주지 않는다.** 바이트를 지키는 것([`Doc::render`] 의 BOM·끝 줄바꿈·CRLF)과
    /// 고친 것을 세는 깃발이 이 타입에 매여 있어, 밖에서 `DocumentMut` 을 쥐면 그 둘이 함께 샌다.
    pub(crate) fn root(&self) -> &Table {
        self.doc.as_table()
    }

    /// [`Doc::root`] 의 고칠 수 있는 판. **고쳤으면 [`Doc::touched`] 로 알린다** — 그 깃발이 곧
    /// [`update`] 가 파일을 쓸지를 가른다. 안 알리면 고친 것이 말없이 버려진다.
    pub(crate) fn root_mut(&mut self) -> &mut Table {
        self.doc.as_table_mut()
    }

    /// 고쳤다고 알린다 — [`Doc::root_mut`] 로 손댄 쪽이 부른다.
    pub(crate) fn touched(&mut self) {
        self.dirty = true;
    }

    /// 뿌리의 `table` 표에서 키들을 뺀다 — 실제로 빠진 수를 돌려준다.
    ///
    /// **맨 `TableLike::remove` 로 빼지 않는다**(리뷰). 그쪽은 키 위의 주석을 **빈 줄 너머까지** 함께
    /// 가져가, 읽음을 한 번 걷는 것이 사람이 그 파일에 적어 둔 글과 앞 줄의 꼬리를 말없이 지운다.
    /// [`drop_key`] 가 그 자를 이미 들고 있고(moai-liij·moai-bx7g), 보기를 뺄 때(`Doc::merge_look`)가
    /// 그것을 쓴다 — 같은 파일을 고치는 자가 둘로 갈리면 한쪽만 주석을 지킨다.
    pub(crate) fn drop_keys(&mut self, table: &str, keys: &[String]) -> usize {
        let Some(t) = self.doc.get_mut(table).and_then(Item::as_table_like_mut) else {
            return 0;
        };
        let mut left = String::new();
        let gone = keys.iter().filter(|k| drop_key(t, k, &mut left)).count();
        // 끝 줄을 지워 표 밖으로 나갈 주석(moai-liij) — `merge_look` 과 같은 자다.
        if !left.is_empty() {
            self.put_after(table, left);
        }
        self.dirty |= gone > 0;
        gone
    }

    pub fn render(&self) -> String {
        let mut body = self.doc.to_string();
        // 끝 줄바꿈은 줄 끝을 고르기 **전에** 뗀다 — 라이브러리는 꾸밈의 `\r` 을 걷어 그리므로 끝은 늘 `\n` 하나다.
        if self.no_eol && body.ends_with('\n') {
            body.pop();
        }
        if self.crlf {
            body = crlf_outside_strings(&body);
        }
        if self.bom { format!("\u{feff}{body}") } else { body }
    }

    /// `project` 가 표 배열이 아니면 그 까닭 — 읽기는 알리고 쓰기는 멈춘다. 그 키가 무엇인지 모르는 채로
    /// 항목을 더하거나 빼면 남의 값을 덮는다.
    fn odd_projects(&self) -> Option<String> {
        let item = self.doc.get(PROJECT).filter(|i| !i.is_array_of_tables())?;
        Some(format!("`{PROJECT}` 는 `[[{PROJECT}]]` 표 배열이어야 한다 — 지금은 {}", item.type_name()))
    }

    /// 목록을 고치는 자리. 없으면 `None`, 모양이 틀리면 거절(`broken`) — [`Doc::odd_projects`].
    fn tables_mut(&mut self) -> R<Option<&mut ArrayOfTables>> {
        if let Some(why) = self.odd_projects() {
            return Err(refuse(format!("{why} — 목록을 고치지 않는다. 손으로 고친다")));
        }
        Ok(self.doc.get_mut(PROJECT).and_then(Item::as_array_of_tables_mut))
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
        let mut problems: Vec<String> = self.odd_projects().into_iter().collect();
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
    ///
    /// **맞은 줄의 `color` 가 표 모양이면(`color.x = 1`·`[project.color]`·`{ … }`·배열) 하나도
    /// 안 바꾸고 거절한다**(moai-r9qa). 색이든 `auto` 든 그 자리를 덮으면 무엇을 적어 둔 것인지
    /// 모르는 채 사라진다. 겹친 줄 중 하나만 그래도 멈추는 까닭은, 앞의 것만 바꾸면 읽기가 보는
    /// 색과 남은 줄이 어긋나서다. 낱값(`color = 3`)은 읽기가 틀린 색이라 대는 값이라 고쳐 쓴다.
    /// 엄함은 지금 쓰는 줄에 대한 것이라 맞지 않은 줄은 안 본다 — [`crate::read_marks::Sheet::mark`] 와 같은 자다.
    /// 거절문은 어느 줄인지 경로로 댄다 — 겹쳐 적은 줄 중 뒤의 것일 수 있어 준 철자만으로는 못 찾는다.
    ///
    /// **값만 바꾼다**([`put_value`]) — `color` 위의 주석·값 뒤의 주석·키 모양은 그대로다.
    /// `Table::insert` 로 갈아 끼우면 키를 새로 지어 그 위의 주석이 말없이 사라진다.
    pub fn set_hue(&mut self, any_of: &[PathBuf], hue: Option<Hue>) -> R<usize> {
        let Some(aot) = self.tables_mut()? else {
            return Ok(0);
        };
        let mine = |t: &Table| entry_path(t).is_ok_and(|p| any_of.contains(&p));
        let odd = aot.iter().filter(|t| mine(t)).find_map(|t| {
            let c = t.get(COLOR).filter(|c| !plain(c, false))?;
            Some((entry_path(t).ok()?, c.type_name()))
        });
        if let Some((path, shape)) = odd {
            return Err(refuse(format!(
                "{} 의 `{COLOR}` 가 색 낱말이 아니라({shape}) 덮지 않는다 — 손으로 고친다",
                path.display()
            )));
        }
        let (mut hit, mut changed) = (0, false);
        let mut kept: Vec<(isize, String)> = Vec::new();
        for t in aot.iter_mut().filter(|t| mine(t)) {
            hit += 1;
            let mut left = String::new();
            changed |= put_value(t, COLOR, hue.map(|h| toml_edit::Value::from(h.name())), &mut left);
            if !left.is_empty()
                && let Some(at) = t.position()
            {
                kept.push((at, left));
            }
        }
        self.dirty |= changed;
        // `color` 가 끝 줄이었으면 빈 줄로 떨어진 그 위 주석은 표 뒤로 나간다(moai-liij).
        self.put_all_before_next(kept);
        Ok(hit)
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
        // 목록은 **모양을 재는 길로만** 연다([`Doc::tables_mut`]) — 있는지 볼 때도 더할 때도. 모양만 재고 값을 버리는
        // 줄을 따로 두면 안 쓰는 줄로 읽혀 걷히기 쉽고, 걷히면 아래의 새 목록이 모양이 틀린 `project` 를 덮는다.
        if self.tables_mut()?.is_some_and(|aot| aot.iter().any(|t| entry_path(t).is_ok_and(|p| p == dir))) {
            return Ok(false);
        }
        let mut t = self.new_table();
        t.insert(PATH, toml_edit::value(text));
        match self.tables_mut()? {
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

    /// 문서에 더할 새 표. **주석만 있는 설정이면 그 주석을 머리에 두고 그 뒤에 선다**([`Doc::lift_head_comment`])
    /// — 표를 세우는 쓰기는 모두 여기를 지난다(등록·보기·읽음). 한 곳만 옮기면 주석만 있던 설정에 처음 적는 것이
    /// 무엇이냐에 따라 머리 주석이 새 표 밑으로 밀린다(moai-gmdu 에픽 리뷰).
    ///
    /// 머리 주석과 새 표 사이에 빈 줄 하나 — 주석이 새 표의 것으로 읽히지 않게. 빈 줄은 **새 표의 머리에**
    /// 붙인다. 다시 읽으면 주석과 빈 줄이 새 표의 머리로 붙지만, 빈 줄로 떨어진 주석은 `remove` 가 남긴다
    /// — 도로 빼면 처음 바이트로 돌아온다. 주석이 이미 빈 줄로 끝나도 하나 더 둔다: `remove` 는 빈 줄
    /// 하나를 표의 것으로 보고 함께 빼므로, 안 두면 도로 뺄 때 사람이 둔 빈 줄이 빠진다.
    pub(crate) fn new_table(&mut self) -> Table {
        let mut t = Table::new();
        if self.lift_head_comment() {
            t.decor_mut().set_prefix("\n");
        }
        t
    }

    /// 키도 표도 없이 주석만 있는 설정이면 그 주석을 **문서 머리로** 옮긴다(moai-bx7g).
    ///
    /// 라이브러리는 마지막 키·표 뒤의 글을 끝 글로 들고 맨 끝에 그린다. 주석만 있는 파일에서는 그 글이
    /// 전부 끝 글이라, 표 하나를 더하면 머리 주석이 `[[project]]` 밑으로 밀린다. 머리로 옮기면 새 표가
    /// 그 뒤에 서고, 그 표를 도로 빼면(`remove`) 머리만 남아 **처음 바이트로 돌아온다** — 새 표의 머리에
    /// 붙이면 표와 함께 주석이 지워진다.
    ///
    /// 키나 표가 있는 파일의 끝 글은 옮기지 않는다 — 앞의 것에 붙은 꼬리인지 뒤에 올 것의 머리인지 모르고,
    /// 옮기면 도로 뺄 때 처음 바이트로 못 돌아온다. 끝에 남는 것은 전과 같다.
    ///
    /// 옮겼으면 참 — 부르는 쪽이 새 표 앞에 빈 줄을 둔다. **문서를 고치는 부름이다** — 묻는 이름으로 두면 조건에
    /// 끼워 읽다가 옮김을 놓친다.
    fn lift_head_comment(&mut self) -> bool {
        let tail = self.doc.trailing().as_str().unwrap_or_default();
        if !self.doc.is_empty() || tail.trim().is_empty() {
            return false;
        }
        let mut head = tail.to_string();
        // 줄바꿈 없이 끝난 주석 뒤에 표 머리가 붙으면 그 줄이 주석이 된다. 도로 빼면 이 줄바꿈이 끝에 남지만
        // 끝 줄바꿈 모양은 `render` 가 원문대로 돌려놓는다.
        if !head.ends_with('\n') {
            head.push('\n');
        }
        self.doc.set_trailing("");
        self.doc.decor_mut().set_prefix(head);
        true
    }

    /// 경로가 `any_of` 중 하나와 같은 항목을 **모두** 뺀다. 뺀 수를 낸다.
    ///
    /// 한 경로를 여러 철자로 받는 까닭은 [`spellings`] 에 있다. 못 읽는 항목은
    /// 건드리지 않는다 — 무엇을 가리키는지 모르는 줄을 지우면 되돌릴 수 없다.
    /// 목록에서만 뺀다. 그 디렉터리의 `.moai` 는 이 모듈이 모른다.
    ///
    /// **뺀 표 머리의 주석 중 빈 줄로 떨어진 윗부분은 남긴다**(moai-bx7g, 사용자 결정 2026-09-18). 라이브러리는
    /// 표 앞의 주석·빈 줄을 통째로 그 표의 머리로 들어, 표를 빼면 함께 사라진다. 바로 위에 붙은 주석은 그 표의
    /// 것이지만, 빈 줄 너머의 것은 파일 머리나 앞 것의 꼬리다 — 주석만 있던 설정에 `add` 한 뒤 도로 빼면 머리
    /// 주석이 사라지던 자리다. 남긴 글은 그 표 뒤에 그려지던 것의 앞에 선다 — 글의 차례가 안 바뀐다.
    pub fn remove(&mut self, any_of: &[PathBuf]) -> R<usize> {
        let Some(aot) = self.tables_mut()? else {
            return Ok(0);
        };
        let before = aot.len();
        let mut kept: Vec<(isize, String)> = Vec::new();
        aot.retain(|t| {
            let gone = entry_path(t).is_ok_and(|p| any_of.contains(&p));
            if gone && let (Some(at), Some(head)) = (t.position(), detached(prefix_of(t.decor()))) {
                kept.push((at, head));
            }
            !gone
        });
        let removed = before - aot.len();
        if removed > 0 {
            self.dirty = true;
        }
        self.put_all_before_next(kept);
        Ok(removed)
    }

    /// 남긴 글들(`(그 표의 위치, 글)`)을 저마다 [`Doc::put_before_next`] 한다. **뒤의 것부터 앞에 붙인다** — 같은
    /// 자리 앞에 둘이 서면 파일에 있던 차례대로 선다(`Doc::remove`·`Doc::set_hue`).
    fn put_all_before_next(&mut self, mut kept: Vec<(isize, String)>) {
        kept.sort_by_key(|(at, _)| std::cmp::Reverse(*at));
        for (at, text) in kept {
            self.put_before_next(at, text);
        }
    }

    /// 파일 차례로 `at` 다음에 그려지는 표의 머리 앞에 `text` 를 붙인다. 뒤에 표가 없으면 끝 글 앞이다.
    ///
    /// 차례는 **파일에서 읽은 위치**로 잰다 — 읽은 표는 모두 위치를 들고, 라이브러리가 그 차례로 그린다.
    /// 점 키·암묵 표는 머리를 안 그려 붙일 자리가 아니다.
    fn put_before_next(&mut self, at: isize, text: String) {
        let mut all = Vec::new();
        header_positions(self.doc.as_table(), &mut all);
        let next = all.into_iter().filter(|p| *p > at).min();
        match next.and_then(|p| header_at(self.doc.as_table_mut(), p)) {
            Some(t) => keep_in_front(t.decor_mut(), &text),
            None => {
                let was = self.doc.trailing().as_str().unwrap_or_default().to_string();
                self.doc.set_trailing(text + &was);
            }
        }
    }

    /// 뿌리의 `key` 표 **다음에 그려지는 것** 앞에 `text` 를 붙인다 — 그 표의 끝 줄을 지워 표 밖으로 나갈 글이다
    /// (`Doc::merge_look`). 머리를 그리는 표(`[tui]`)면 [`Doc::put_before_next`] 다.
    ///
    /// **뿌리에 점 키로 적은 표(`tui.sort = …`)는 위치가 없다** — 파서가 위치 없이 짓고, 그 줄은 뿌리 몸 안에 그려진다.
    /// 뿌리에서 그 다음에 그려지는 줄 앞에, 없으면 첫 표 머리 앞에 선다(뿌리 몸은 표 머리보다 먼저 그려진다). 이것을
    /// 건너뛰면 설정 머리 주석(첫 줄의 머리다)이 `tui.x` 를 지우는 저장 한 번에 사라진다. 인라인 표(`tui = { … }`)는
    /// 여기 안 온다 — [`drop_key`] 의 주석처럼 그 안의 끝 줄 주석은 전처럼 키와 함께 빠진다.
    fn put_after(&mut self, key: &str, text: String) {
        let Some(t) = self.doc.get(key).and_then(Item::as_table) else {
            return;
        };
        if let Some(at) = t.position() {
            return self.put_before_next(at, text);
        }
        if !t.is_dotted() {
            return;
        }
        let from = self.doc.iter().position(|(k, _)| k == key).map_or(0, |i| i + 1);
        match line_key(self.doc.as_table_mut(), from) {
            Some(mut n) => keep_in_front(n.leaf_decor_mut(), &text),
            None => self.put_before_next(isize::MIN, text),
        }
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
        let mut look = Look {
            hidden: look_words(t, HIDDEN, &mut problems),
            hide_deferred: look_one(t, HIDE_DEFERRED, "true·false 여야", Item::as_bool, &mut problems),
            sort: look_one(t, SORT, "낱말이어야", word, &mut problems),
            sort_reversed: look_one(t, SORT_REVERSED, "true·false 여야", Item::as_bool, &mut problems),
            fields: look_words(t, FIELDS, &mut problems),
            fields_known: look_words(t, FIELDS_KNOWN, &mut problems),
            detail: look_one(t, DETAIL, "true·false 여야", Item::as_bool, &mut problems),
        };
        // **차례를 못 읽었으면 방향도 버린다**(moai-ys7c) — 둘은 한 벌이다. `sort = 3` 을 없는 키로 넘기고
        // 방향만 내면, 탐색기가 처음 차례(우선순위)에 그 방향을 입혀 아무도 안 고른 거꾸로가 선다. 모르는
        // 낱말(`sort = "nope"`)은 탐색기가 같은 까닭으로 방향을 두고(`App::apply_look`), 모양이 틀린 것은
        // 여기서 막는다. 파일의 둘은 그대로 남는다 — 이 세션이 차례를 고르기 전까지는 안 건드린다.
        if t.contains_key(SORT) && look.sort.is_none() {
            look.sort_reversed = None;
        }
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
    /// - **바꿀 키가 표 모양이면 그 키만 안 적는다**(moai-j7r3, moai-jr3z) — 낱값으로 덮으면 무엇을 적어 둔 것인지
    ///   사라진다. 나머지 키는 적고, 건너뛴 키의 까닭을 낸다
    ///
    /// `None` 으로 바꾼 키는 지운다. 파일이 이미 그렇게 적혀 있으면(옆에서 같게 적었으면) 아무것도 안
    /// 한다 — 헛 쓰기가 없다. **`tui` 가 표가 아니면 적지 않는다** — 무엇인지 모르는 값을 덮으면 되돌릴
    /// 수 없다.
    pub fn merge_look(&mut self, base: &Look, new: &Look) -> R<Vec<String>> {
        if base == new {
            return Ok(Vec::new());
        }
        match self.doc.get(TUI) {
            None => {
                let t = self.new_table();
                self.doc.insert(TUI, Item::Table(t));
            }
            // 읽기(`look`)가 받는 모양은 쓰기도 받는다 — `tui = { … }` 인라인 표도 표다.
            Some(item) if item.is_table_like() => {}
            Some(item) => {
                return Err(refuse(format!(
                    "`{TUI}` 가 `[{TUI}]` 표가 아니라({}) 보기를 적지 않는다 — 손으로 고친다",
                    item.type_name()
                )));
            }
        }
        // 이 세션이 바꾼 키. **거절을 재는 자리와 적는 자리가 같은 깃발을 본다**(moai-gmdu 에픽 리뷰) — 조건을
        // 두 번 적으면 적는 쪽에만 키를 더하는 날 거절이 그 키를 못 보고, 손으로 적은 표 모양을 낱값으로 덮는다.
        let mut hidden = base.hidden != new.hidden;
        let mut hide_deferred = base.hide_deferred != new.hide_deferred;
        let mut sort = (&base.sort, base.sort_reversed) != (&new.sort, new.sort_reversed);
        let mut fields = base.fields != new.fields;
        // **`fields_known` 도 `base != new` 로 잰다**(moai-fdq2). 한때 이 키만 "적을 것이 있으면 늘
        // 본다"(`is_some()`)였다 — 더하기로만 적는 키라 `base` 를 안 쓰는 것과 같은 결로 둔 것인데,
        // 그 깃발은 **늘 참**이라 건너뛴 키를 든 것으로 옮기는 길(`App::save_look` 의 `saved = look`,
        // moai-jr3z)에서 혼자 샜다: 손으로 적은 표 모양 앞에서 같은 거절 알림이 토글마다 다시 서서
        // 사람이 방금 띄운 말을 덮었다. `save_look` 의 글은 "알림은 이번 한 번" 이라고 적혀 있다.
        //
        // **`base` 를 안 쓰는 것은 아래의 `merge_words` 지 이 깃발이 아니다** — 거기 `None` 을 넘기는
        // 것은 남이 적어 둔 이름을 빼지 않으려는 것이고, 여기서 재는 것은 *이 세션이 적을 것이
        // 남았는가* 다. 탐색기의 `new` 는 세션 내내 같은 값(아는 열 전부)이라, 한 번 적히고 나면
        // `saved` 가 그것을 들어 둘이 같아진다 — 그때부터 이 키는 안 본다.
        let mut known = base.fields_known != new.fields_known;
        let mut detail = base.detail != new.detail;
        // **이 세션이 적을 키가 손으로 적은 표 모양이면 그 키만 안 적는다**(moai-j7r3, moai-jr3z) — 무엇을 덮지
        // 않는가는 `set_hue` 와 같은 자다([`plain`]). `sort.by = "title"`·`[tui.sort]`·`sort = { … }` 은 무엇을
        // 적어 둔 것인지 모르는 채 낱값으로 덮이면 사라진다(`put_value` 는 값이 아닌 자리를 그대로 갈아 끼운다).
        // 낱값의 틀린 값(`sort = 3`·`hidden = "done"`)은 읽기가 까닭을 대는 값이라 고쳐 쓴다. 안 바꿀 키는 안 본다.
        //
        // **나머지 키는 적는다**(사용자 결정 2026-09-18). 처음에는 `set_hue`·읽음처럼 하나도 안 적었는데,
        // 탐색기는 거절된 차이를 다음 저장에 또 실어 그 세션의 숨김·상세·열이 하나도 안 적혔다 — 화면에는 선 채
        // 다음 실행에서 사라졌다. 보기는 키마다 따로 사는 값이라 한 키가 다른 키의 저장을 막을 까닭이 없다.
        // 차례와 방향은 한 벌이라 하나가 표 모양이면 둘 다 건너뛴다. 건너뛴 키의 까닭을 낸다.
        let t = self.doc.get(TUI).and_then(Item::as_table_like).expect("방금 표로 섰다");
        // **파일에 그 키가 없으면 이 세션이 이미 적었어도 다시 적는다**(리뷰) — 위의 깃발은 *이 세션이
        // 적을 것이 남았는가* 를 재는데, 그것만으로는 **파일이 밑에서 바뀐 판**을 못 본다. `saved` 는
        // 한 번 적고 나면(또는 거절돼 건너뛰고 나면, moai-jr3z) 세션 내내 `new` 와 같아, 그사이 누가
        // 이 키를 지우거나 설정을 통째로 갈아 끼우면 다음 토글이 `fields` 만 적고 `fields_known` 은
        // 빼놓는다 — 다음 실행이 그 빈자리를 `Field::BEFORE_KNOWN` 어휘로 읽어 사람이 끈 열을 도로
        // 켠다(`turning_off_a_new_column_survives_a_trip_through_the_file` 가 막는 그 해다). 걷은
        // `is_some()` 이 값싸게 해 주던 일이 이것 하나라, 그것만 도로 든다. 더하기로만 적는 키라
        // (`merge_words` 에 `None` 을 준다) 다시 적어도 남의 이름은 그대로다.
        known |= !t.contains_key(FIELDS_KNOWN);
        let mut skipped = Vec::new();
        let mut odd = |keys: &[&str], words: bool, go: &mut bool| {
            if !*go {
                return;
            }
            for key in keys {
                if let Some(item) = t.get(key).filter(|i| !plain(i, words)) {
                    skipped.push(format!(
                        "`{TUI}.{key}` 가 손으로 적은 모양이라({}) 그 키는 적지 않았다 — 손으로 고친다",
                        item.type_name()
                    ));
                    *go = false;
                    return;
                }
            }
        };
        odd(&[HIDDEN], true, &mut hidden);
        odd(&[HIDE_DEFERRED], false, &mut hide_deferred);
        odd(&[SORT, SORT_REVERSED], false, &mut sort);
        odd(&[FIELDS], true, &mut fields);
        odd(&[FIELDS_KNOWN], true, &mut known);
        odd(&[DETAIL], false, &mut detail);
        let t = self.doc.get_mut(TUI).and_then(Item::as_table_like_mut).expect("방금 표로 섰다");
        let mut changed = false;
        let mut left = String::new();
        if hidden {
            changed |= merge_words(t, HIDDEN, base.hidden.as_deref(), new.hidden.as_deref(), &mut left);
        }
        if hide_deferred {
            changed |= put_value(t, HIDE_DEFERRED, new.hide_deferred.map(toml_edit::Value::from), &mut left);
        }
        if sort {
            changed |= put_value(t, SORT, new.sort.as_deref().map(toml_edit::Value::from), &mut left);
            changed |= put_value(t, SORT_REVERSED, new.sort_reversed.map(toml_edit::Value::from), &mut left);
        }
        if fields {
            changed |= merge_words(t, FIELDS, base.fields.as_deref(), new.fields.as_deref(), &mut left);
        }
        // **`fields_known` 은 빼지 않고 더하기만 한다**(moai-6bc0 단계 리뷰) — `base` 를 비워 두는 까닭이다.
        // 이 키는 사람이 고른 것이 아니라 *적는 쪽이 아는 열 전부*라, 여기 있는데 이 바이너리가 모르는
        // 이름은 **새 바이너리가 적어 둔 것**이다. 그것을 빼면 그쪽의 다음 실행이 제가 적어 둔 열을
        // "몰랐던 열" 로 읽어 사람이 끈 것을 도로 켠다 — 낱말 배열을 합치는 까닭(`남이 더한 낱말은
        // 남는다`)이 여기서는 더 세게 걸린다.
        if known {
            changed |= merge_words(t, FIELDS_KNOWN, None, new.fields_known.as_deref(), &mut left);
        }
        if detail {
            changed |= put_value(t, DETAIL, new.detail.map(toml_edit::Value::from), &mut left);
        }
        self.dirty |= changed;
        // 끝 줄을 지워 표 밖으로 나갈 주석(moai-liij).
        if !left.is_empty() {
            self.put_after(TUI, left);
        }
        Ok(skipped)
    }

    /// **옛** `[read]` — 이슈 id → 마지막으로 본 줄의 도장(moai-50mn·moai-lyc1). 이 바이너리는 여기 다시
    /// 안 적고 겹쳐 보기만 한다(moai-bwce, 사용자 결정 3) — 적는 자리는 [`crate::read_marks`] 다.
    ///
    /// **읽는 자는 하나다**([`crate::read_marks::read_table`], 리뷰) — 옛 표와 새 읽음 파일이 같은 모양이라
    /// 둘이 저마다 읽으면 모양이 자라는 날 한쪽만 따라가고, 그 한쪽은 남은 읽음을 조용히 버린다.
    pub fn read_marks(&self) -> (BTreeMap<String, String>, Vec<String>) {
        crate::read_marks::read_table(self.root())
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
/// # 적는 쪽이 아는 열 전부 — 바이너리를 따라 자란다. `fields_known = []` 의 뜻을 얼린 목록과 다른 목록이다.
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
    ///
    /// **빈 목록은 "2026-09-20 에 있던 아홉 열을 알았다" 다**(moai-4qkj·moai-4gy5, 사용자 결정
    /// 2026-09-18) — 바이너리는 늘 이름을 다 적으니 `[]` 는 손으로 적은 것이고, **그 아홉 열에 대해서는**
    /// `fields` 가 곧 켠 열이다. `fields` 에 적힌 열은 이 목록과 상관없이 켜진다 — 적은 쪽이 그 열을
    /// 알았다는 뜻이다(`App::apply_look`).
    ///
    /// **그 아홉은 날짜로 얼렸다.** 읽는 쪽이 아는 열로 읽으면 `[]` 의 뜻이 바이너리를 따라 움직여, 파일을
    /// 먼저 만진 바이너리가 어느 쪽이냐에 따라 같은 파일이 다른 화면을 낸다. 얼린 뒤로는 어느 쪽이 먼저
    /// 만지든 한 자리로 모인다. 그 아홉을 들고 있는 것은 탐색기(`crate::tui::view::Field::EMPTY_KNOWN`)다
    /// — 무슨 낱말이 있는지를 아는 쪽이 거기라, 이 모듈은 규칙만 적고 목록은 안 든다(위의 `Look` 글).
    ///
    /// **대가는 적어 둔다**(moai-8mq9.p38 리뷰): 2026-09-20 뒤에 생긴 열은 `[]` 로 끌 수 없다. 그 열은
    /// 늘 기본값으로 서고, 끄려면 `fields_known` 에 이름을 다 적어야 한다. 읽기는 이때 아무 말도 안 한다.
    pub fields_known: Option<Vec<String>>,
    /// 오른쪽 상세 칸이 보이나(moai-ymnu).
    pub detail: Option<bool>,
}

/// 설정에서 보기만 읽는다. 파일이 없으면 빈 `Look` 이고 문제도 아니다. 깨진 파일도 까닭 없이 빈 `Look` 이다 —
/// 그 까닭은 층이 댄다(moai-5jsn). 탐색기는 처음값으로 뜬다. **시험만 부른다** — 띄우는 길은 [`read`] 한 번으로 층과 보기를 함께 얻는다.
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
fn merge_words(
    t: &mut dyn toml_edit::TableLike,
    key: &str,
    base: Option<&[String]>,
    new: Option<&[String]>,
    left: &mut String,
) -> bool {
    if base == new {
        return false;
    }
    let Some(new) = new else {
        return drop_key(t, key, left);
    };
    if t.get(key).and_then(Item::as_array).is_none() {
        return write_value(t, key, new.iter().map(String::as_str).collect::<toml_edit::Array>().into());
    }
    let base = base.unwrap_or_default();
    let words = t.get_mut(key).and_then(Item::as_array_mut).expect("방금 배열인 것을 봤다");
    let mut changed =
        drop_elements(words, |v| v.as_str().is_some_and(|w| base.iter().any(|b| b == w) && !new.iter().any(|n| n == w))) > 0;
    for w in new {
        if base.contains(w) || words.iter().any(|v| v.as_str() == Some(w.as_str())) {
            continue;
        }
        push_word(words, w);
        changed = true;
    }
    changed
}

/// 고쳐 써도 되는 자리인가 — **낱값**(문자열·수·참거짓·때)이면 참이다(`Doc::set_hue` moai-r9qa, `Doc::merge_look`
/// moai-j7r3 — 한 자를 둘이 쓴다). 표·점 키·인라인 표는 모양째 사람의 것이라 덮지 않는다. 배열은 `arrays` 일 때만
/// 받는다 — 낱말 배열 키(`hidden`·`fields`)는 배열이 제 모양이다. 틀린 낱값(`color = 3`·`sort = 3`)은 읽기가 까닭을
/// 대는 값이라 고쳐 쓴다. [`crate::read_marks::Sheet::mark`] 는 이보다 엄하다 — 때를 적은 낱말이 아니면 모두 거절한다(moai-j038.vna).
fn plain(item: &Item, arrays: bool) -> bool {
    matches!(item, Item::Value(v) if !v.is_inline_table() && (arrays || !v.is_array()))
}

/// 값 하나를 적거나(`Some`, [`write_value`]) 키를 지운다(`None`, [`drop_key`] — 표 밖으로 내보낼 주석은 `left` 에
/// 쌓는다). 바뀐 것이 있으면 참(`Doc::merge_look`·`Doc::set_hue`).
fn put_value(t: &mut dyn toml_edit::TableLike, key: &str, v: Option<toml_edit::Value>, left: &mut String) -> bool {
    match v {
        Some(v) => write_value(t, key, v),
        None => drop_key(t, key, left),
    }
}

/// 값 하나를 적는다([`put_value`]·[`crate::read_marks::Sheet::mark`]·`merge_words`). 같은 값이면 안 적고, 바뀐 것이 있으면 참.
///
/// **키는 안 건드리고 값만 바꾼다.** 키 위의 주석은 키의 꾸밈에 붙어 있어 `Table::insert` 로 갈아 끼우면
/// 지워진다(키 모양을 새로 짓는다). 값 뒤의 주석은 있던 값의 꾸밈에 붙어 있어 옮겨 단다.
pub(crate) fn write_value(t: &mut dyn toml_edit::TableLike, key: &str, mut v: toml_edit::Value) -> bool {
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

/// 남긴 글 `text` 를 다음 것의 머리 `was` 앞에 붙인 새 머리.
///
/// **남긴 글과 그 다음 것 사이에 빈 줄을 지킨다**(moai-gmdu 에픽 리뷰). 빈 줄 하나는 뺀 것과 함께
/// 빠졌다 — 다음 것이 뺀 것에 빈 줄 없이 붙어 있었으면 남긴 글이 그것의 머리에 붙고, 그것을 뺄 때 함께
/// 지워진다. 남긴 까닭이 그것을 안 지우려는 것이었다.
fn keep_before(text: &str, was: &str) -> String {
    let gap = if blank_lines(was).next() == Some(0) { "" } else { "\n" };
    format!("{text}{gap}{was}")
}

/// 키 하나를 지운다(`put_value`·`merge_words` 의 `None`). 지웠으면 참.
///
/// **키 위의 주석 중 빈 줄로 떨어진 윗부분은 남긴다**(moai-liij) — 표를 뺄 때의 자([`Doc::remove`], moai-bx7g)를
/// 키에도 댄다. 바로 위에 붙은 주석은 그 키의 것이라 함께 빠지고, 빈 줄 너머의 것은 앞 것의 꼬리나 밑의 것들의
/// 머리다. 남긴 글은 같은 표에서 **다음에 그려지는 줄**의 머리 앞에 선다([`line_key`] — 점 키 줄도 줄이다). 다음
/// 줄이 없으면 그 글은 표 밖으로 나가야 하는데 표는 제 끝 글을 안 들어, `left` 에 쌓아 부르는 쪽이 표 다음에
/// 그려지는 것 앞에 붙인다(`Doc::put_before_next`). 나중에 쌓이는 것일수록 파일에서 앞이라 앞에 붙인다 — 끝 줄을
/// 빼면 그 앞 줄이 끝 줄이 된다.
///
/// 표준 표(`[tui]`·`[[project]]`)의 키 머리는 줄 머리에서 시작한다. 인라인 표(`tui = { … }`)는 앞 쉼표 바로
/// 뒤에서 시작해 첫 줄이 앞 줄의 끝인데, 여기는 그것을 안 가른다 — 그 안의 주석은 TOML 1.1 모양이다.
fn drop_key(t: &mut dyn toml_edit::TableLike, key: &str, left: &mut String) -> bool {
    let Some(at) = t.iter().position(|(k, _)| k == key) else {
        return false;
    };
    let head = t.key(key).and_then(|k| detached(prefix_of(k.leaf_decor())));
    t.remove(key);
    let Some(head) = head else { return true };
    match line_key(t, at) {
        Some(mut n) => keep_in_front(n.leaf_decor_mut(), &head),
        None => *left = keep_before_end(&head, left),
    }
    true
}

/// 표 몸에서 `from` 번째 것부터 보아 **처음 그려지는 줄**의 키. 점 키 줄(`meta.x = 1`)은 줄 머리를 맨 끝 조각의
/// 꾸밈에 들고(toml_edit 의 `encode_key_path`) 몸 안 제자리에 그려지므로 그 조각까지 내려간다. 하위 표·표 배열은
/// 몸 뒤에 제 머리로 그려져 몸의 줄이 아니다 — 그 머리는 [`Doc::put_before_next`] 가 맡는다.
fn line_key(t: &mut dyn toml_edit::TableLike, from: usize) -> Option<toml_edit::KeyMut<'_>> {
    let name = t.iter().skip(from).find(|(_, v)| draws_line(v)).map(|(k, _)| k.to_string())?;
    if t.get(&name).and_then(Item::as_table_like).is_some_and(|d| d.is_dotted()) {
        return line_key(t.get_mut(&name)?.as_table_like_mut()?, 0);
    }
    t.key_mut(&name)
}

/// 표 몸에 줄을 하나라도 그리는가 — 값 키(낱값·배열·인라인 표)이거나 그런 것을 든 점 키.
fn draws_line(item: &Item) -> bool {
    match item.as_table_like() {
        Some(d) if d.is_dotted() => d.iter().any(|(_, v)| draws_line(v)),
        _ => item.is_value(),
    }
}

/// 배열 끝에 낱말 하나를 더한다(`merge_words`) — **여러 줄로 벌린 모양을 지킨다**(moai-n3ku).
///
/// 끝 원소와 `]` 사이의 글은 둘로 갈린다. 첫 줄(줄 끝 주석과 그 줄바꿈)은 **끝 원소의 줄 끝**이고, 그
/// 뒤는 `]` 앞 글이다. 더하면 끝 원소 뒤에 쉼표가 서므로 그 줄 끝을 새 원소의 머리로 옮긴다 — 그래야
/// 쉼표가 주석 앞에 서고, 새 원소가 앞 원소와 같은 들여쓰기로 제 줄에 선다. 안 옮기던 판은 `toml_edit`
/// 이 그 글을 끝 원소의 꼬리로 들어, 쉼표가 줄 머리에 서고(`"done"\n, "review"]`) `]` 가 끌려 올라왔다.
/// 줄 끝 주석이 있으면 쉼표가 그 주석 뒤로 가 파일이 통째로 안 읽혔다.
///
/// 들여쓰기는 **줄을 여는 원소 가운데 마지막 것**의 것이다 — 한 줄에 여럿이 선 배열(`[\n "a", "b"\n]`)의
/// 끝 원소는 제 줄을 안 열어 들여쓰기를 모른다. 그런 배열에 더한 낱말은 제 줄에 서되 들여쓰기가 없다 —
/// 본뜰 원소가 없으니 지어내지 않는다.
///
/// **빈 배열도 여러 줄일 수 있다**(리뷰) — 마지막 낱말을 뺀 자리가 `[\n]` 이라, 원소가 없다고 한 줄로
/// 보면 `SPC v` 를 껐다 켜는 것만으로 `[` 줄에 원소가 붙는다(`["done"\n]`). 원소가 없을 때 `]` 앞 글은
/// 배열의 꼬리(`Array::trailing`)에 통째로 있으므로 그것을 줄 끝으로 읽고, 들여쓰기는 거기 선 주석에서
/// 든다. 진짜 한 줄 배열(`[]`·`["a"]`)은 옮길 줄 끝이 없어 `toml_edit` 의 기본 모양(`, "새것"`)이 곧
/// 제 모양이다.
fn push_word(words: &mut toml_edit::Array, word: &str) {
    let last = words.len().checked_sub(1);
    let tail = last
        .and_then(|i| words.get(i).expect("차례 안이다").decor().suffix())
        .and_then(|r| r.as_str())
        .unwrap_or_default();
    let after = format!("{tail}{}", words.trailing().as_str().unwrap_or_default());
    let (line_end, rest) = first_line(&after);
    if line_end.is_empty() {
        words.push(word);
        return;
    }
    let indent = (0..words.len())
        .rev()
        .find_map(|i| {
            let p = prefix_of(words.get(i).expect("차례 안이다").decor());
            p.rfind('\n').map(|at| p[at + 1..].to_string())
        })
        // 본뜰 원소가 없다 — 빈 여러 줄 배열이면 `]` 앞 글의 들여쓰기가 그 배열의 것이다.
        .unwrap_or_else(|| rest.chars().take_while(|c| *c == ' ' || *c == '\t').collect());
    let (head, trailing) = (format!("{line_end}{indent}"), format!("\n{rest}"));
    if let Some(i) = last {
        words.get_mut(i).expect("차례 안이다").decor_mut().set_suffix("");
    }
    words.push_formatted(toml_edit::Value::from(word).decorated(&head, ""));
    words.set_trailing(trailing);
}

/// 배열에서 `gone` 인 원소를 뺀다(`merge_words`). 뺀 수를 낸다.
///
/// **원소 위의 주석 중 빈 줄로 떨어진 윗부분은 남긴다**(moai-liij) — [`drop_key`] 와 같은 자다. 원소의 머리는 앞
/// 원소의 쉼표 바로 뒤에서 시작해 **첫 줄바꿈까지는 앞 줄의 끝**이다(앞 원소의 줄 끝 주석, `[` 줄의 주석) — 그것은
/// 원소와 함께 빼지 않고, 빈 줄은 그 뒤에서만 잰다. 거꾸로 뒤 자리(다음 원소의 머리, 끝 원소면 `]` 앞 글)의 첫
/// 줄은 **뺀 원소의 줄 끝**이라 원소와 함께 빠진다 — 둘을 뒤집으면 남는 원소가 제 주석을 잃고 뺀 원소의 주석을
/// 단다. 원소가 앞 원소와 한 줄에 있었으면 그 줄이 남으므로 뒤 자리를 안 건드린다([`splice_mid_line`]).
///
/// 남긴 글은 다음 원소 앞에, 끝 원소면 닫는 `]` 앞에 선다. 뒤에서부터 빼 여럿이면 파일에 있던 차례대로 선다.
/// 끝 쉼표가 없는 배열의 끝 원소는 `]` 앞 글을 제 꼬리(suffix)에 들고 있어 그것도 뒤 자리로 친다.
fn drop_elements(words: &mut toml_edit::Array, gone: impl Fn(&toml_edit::Value) -> bool) -> usize {
    let mut n = 0;
    for i in (0..words.len()).rev() {
        let v = words.get(i).expect("차례 안이다");
        if !gone(v) {
            continue;
        }
        let prefix = prefix_of(v.decor()).to_string();
        let suffix = v.decor().suffix().and_then(|r| r.as_str()).unwrap_or_default().to_string();
        words.remove(i);
        n += 1;
        match words.get_mut(i) {
            Some(next) => {
                let slot = splice_mid_line(&prefix, prefix_of(next.decor()), i == 0, false);
                next.decor_mut().set_prefix(slot);
            }
            None => {
                let trailing = words.trailing().as_str().unwrap_or_default();
                let after = if words.trailing_comma() { trailing.to_string() } else { format!("{suffix}{trailing}") };
                let slot = splice_mid_line(&prefix, &after, i == 0, true);
                words.set_trailing(slot);
            }
        }
    }
    n
}

/// 머리가 **줄 가운데서** 시작하는 것(배열 원소 — 앞 쉼표 바로 뒤에서 시작한다)을 뺀 뒤 뒤 자리에 설 글. `gone` 은
/// 뺀 것의 머리, `after` 는 뒤 자리의 지금 글, `first` 는 뺀 것이 첫 원소였나, `closing` 은 뒤 자리가 닫는 괄호 앞
/// 글인가다.
///
/// - `gone` 에 줄바꿈이 없으면 뺀 것이 앞 것과 한 줄에 있었다 — 그 줄은 남으니 `after` 그대로다. 다만 첫 원소면
///   앞이 `[` 라, 그 줄에 이어 선 뒤 원소가 자리를 이어받는다(안 그러면 `["a", "b"]` 가 `[ "b"]` 가 된다)
/// - 있으면 `gone` 의 첫 줄(앞 줄의 끝)은 남기고 `after` 의 첫 줄(뺀 것의 줄 끝)은 버린다. `after` 에 줄바꿈이
///   없으면 뒤 것이 뺀 것과 한 줄이었다 — 그 줄을 이어받아 뺀 것의 들여쓰기에 선다
/// - 그 사이에 `gone` 에서 빈 줄로 떨어진 글([`detached`])을 남긴다. 닫는 괄호 앞이면 빈 줄을 안 둔다([`keep_before_end`])
fn splice_mid_line(gone: &str, after: &str, first: bool, closing: bool) -> String {
    let (end, rest) = first_line(gone);
    if end.is_empty() {
        return if first && !after.contains('\n') { gone.to_string() } else { after.to_string() };
    }
    let (after_end, after_rest) = first_line(after);
    let tail = if after_end.is_empty() { &rest[rest.rfind('\n').map_or(0, |i| i + 1)..] } else { after_rest };
    let kept = match detached(rest) {
        None => tail.to_string(),
        Some(head) if closing => keep_before_end(&head, tail),
        Some(head) => keep_before(&head, tail),
    };
    format!("{end}{kept}")
}

/// 글의 첫 줄(첫 줄바꿈까지, 줄바꿈이 없으면 빈 글)과 나머지.
fn first_line(text: &str) -> (&str, &str) {
    text.split_at(text.find('\n').map_or(0, |i| i + 1))
}

/// 꾸밈의 머리 글. 없으면 빈 글이다.
fn prefix_of(decor: &toml_edit::Decor) -> &str {
    decor.prefix().and_then(|r| r.as_str()).unwrap_or_default()
}

/// 남긴 글을 그 꾸밈의 머리 앞에 붙인다([`keep_before`]).
fn keep_in_front(decor: &mut toml_edit::Decor, text: &str) {
    let head = keep_before(text, prefix_of(decor));
    decor.set_prefix(head);
}

/// 닫는 글(`]` 앞·표 밖으로 나갈 글) 앞에 남긴 글을 붙인다 — 뒤가 비었거나 빈칸뿐이면 빈 줄 없이, 아니면
/// [`keep_before`] 대로.
fn keep_before_end(text: &str, was: &str) -> String {
    if was.trim().is_empty() { format!("{text}{was}") } else { keep_before(text, was) }
}

/// 머리 글 중 **마지막 빈 줄 앞까지**(moai-bx7g) — 그 다음 것에 붙지 않은 주석이다. 빈 줄 하나는 다음 것과
/// 함께 빠진다. 빈 줄이 없거나 그 앞에 주석이 없으면 `None`. 빈 줄이 무엇인지는 [`blank_lines`] 가 잰다.
fn detached(prefix: &str) -> Option<String> {
    let head = &prefix[..blank_lines(prefix).last()?];
    (!head.trim().is_empty()).then(|| head.to_string())
}

/// 글 속 빈 줄이 시작하는 자리들. 빈 줄은 줄바꿈으로 끝나고 **빈칸·탭뿐인 줄**이다 — 눈에는 `\n` 하나와 같은
/// 빈 줄이라, 그것을 못 알아보면 빈 줄로 떨어진 주석이 표와 함께 사라진다(moai-gmdu 에픽 리뷰). CRLF 의 `\r`
/// 도 여기서 걷힌다(moai-lb0u).
fn blank_lines(text: &str) -> impl Iterator<Item = usize> + '_ {
    let mut at = 0;
    text.split_inclusive('\n').filter_map(move |line| {
        let start = at;
        at += line.len();
        (line.ends_with('\n') && line.trim().is_empty()).then_some(start)
    })
}

/// 줄바꿈이 든 문자열 값(여러 줄 문자열)이 글에서 차지한 자리들, 앞에서부터(moai-gmdu 에픽 리뷰). 그 안의 줄바꿈은
/// 값이라 줄 끝으로 세거나 고치지 않는다([`Doc`] 의 `crlf`). 키는 여러 줄로 못 적으니 값만 본다.
fn multiline_strings(doc: &toml_edit::Document<&str>) -> Vec<std::ops::Range<usize>> {
    struct Found<'a> {
        raw: &'a str,
        at: Vec<std::ops::Range<usize>>,
    }
    impl<'doc> toml_edit::visit::Visit<'doc> for Found<'_> {
        fn visit_string(&mut self, node: &'doc toml_edit::Formatted<String>) {
            if let Some(span) = node.span().filter(|s| self.raw.get(s.clone()).is_some_and(|t| t.contains('\n'))) {
                self.at.push(span);
            }
        }
    }
    let mut found = Found { raw: doc.raw(), at: Vec::new() };
    toml_edit::visit::Visit::visit_table(&mut found, doc.as_table());
    found.at.sort_by_key(|s| s.start);
    found.at
}

/// 글을 여러 줄 문자열 밖과 안으로 가른다 — 차례대로 `(밖, 그 뒤의 문자열)` 이고 마지막 문자열은 빈다.
fn split_strings<'a>(text: &'a str, strings: &[std::ops::Range<usize>]) -> Vec<(&'a str, &'a str)> {
    let mut out = Vec::with_capacity(strings.len() + 1);
    let mut at = 0;
    for s in strings {
        out.push((&text[at..s.start], &text[s.clone()]));
        at = s.end;
    }
    out.push((&text[at..], ""));
    out
}

/// 여러 줄 문자열 **밖의** 줄바꿈을 모두 CRLF 로 그린다(moai-lb0u). 문자열 자리는 그린 글을 다시 읽어 잰다 —
/// 라이브러리가 그린 글이라 읽힌다(못 읽으면 통째로 고친다). 여러 줄 문자열이 없으면 다시 읽지 않는다.
fn crlf_outside_strings(text: &str) -> String {
    let crlf = |s: &str| s.replace("\r\n", "\n").replace('\n', "\r\n");
    if !text.contains("\"\"\"") && !text.contains("'''") {
        return crlf(text);
    }
    let strings = toml_edit::Document::parse(text).map(|d| multiline_strings(&d)).unwrap_or_default();
    split_strings(text, &strings).into_iter().map(|(outside, string)| crlf(outside) + string).collect()
}

/// 머리를 그리는 표들의 위치 — 점 키·암묵 표는 머리가 없어 뺀다.
fn header_positions(t: &Table, out: &mut Vec<isize>) {
    for (_, item) in t.iter() {
        let subs: Vec<&Table> = match item {
            Item::Table(s) if !s.is_dotted() && !s.is_implicit() => vec![s],
            Item::Table(s) => {
                header_positions(s, out);
                continue;
            }
            Item::ArrayOfTables(a) => a.iter().collect(),
            _ => continue,
        };
        for s in subs {
            out.extend(s.position());
            header_positions(s, out);
        }
    }
}

/// 위치가 `at` 인, 머리를 그리는 표.
fn header_at(t: &mut Table, at: isize) -> Option<&mut Table> {
    for (_, item) in t.iter_mut() {
        let subs: Vec<&mut Table> = match item {
            Item::Table(s) => vec![s],
            Item::ArrayOfTables(a) => a.iter_mut().collect(),
            _ => continue,
        };
        for s in subs {
            if s.position() == Some(at) && !s.is_dotted() && !s.is_implicit() {
                return Some(s);
            }
            if let Some(found) = header_at(s, at) {
                return Some(found);
            }
        }
    }
    None
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
    use crate::scratch::Scratch;
    use std::collections::HashMap;

    /// **돌려받은 것을 묶어 둔다** — `scratch(..).canonicalize()` 처럼 곧바로 흘리면
    /// 그 줄 끝에서 디렉터리가 지워진다.
    fn scratch(name: &str) -> Scratch {
        Scratch::new(&format!("user-config-{name}"))
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

    /// **못 읽은 것과 깨진 것을 가른다**(moai-9p7v). 가르는 값은 갈래([`Trouble`])뿐이고 까닭 글은 둘 다
    /// `problems` 에 선다. 멀쩡한 파일은 탈이 아니다 — 적힌 값의 모양이 틀린 것(`project = "/a"`)도
    /// 읽기는 된 것이라 탈이 아니다. 없는 파일은 갈래는 서되 **까닭은 안 댄다**(moai-po6v).
    ///
    /// 어느 갈래를 **언제 다시 읽는가**는 이웃(`the_trouble_says_how_reading_again_could_ever_win`)이 잰다.
    #[test]
    #[cfg(unix)]
    fn a_file_that_cannot_be_read_is_told_apart_from_a_broken_one() {
        use std::os::unix::fs::PermissionsExt;
        let d = scratch("trouble");
        let path = d.join("config.toml");

        std::fs::write(&path, "[[project]]\npath = \"/a\"\n").unwrap();
        assert_eq!(read(Some(&path)).trouble, None);
        assert!(read(Some(&d.join("없음/config.toml"))).problems.is_empty(), "없는 파일로 잔소리를 했다");
        assert_eq!(read(None).trouble, None, "자리를 모르는 것은 읽다 만난 탈이 아니다");

        std::fs::write(&path, "project = \"/a\"\n").unwrap();
        assert_eq!(read(Some(&path)).trouble, None, "읽고 파싱까지 된 것은 탈이 아니다");

        std::fs::write(&path, "[[project]\npath = \"/a\"\n").unwrap();
        let reg = read(Some(&path));
        assert_eq!(reg.trouble, Some(Trouble::Broken));
        assert_eq!(reg.problems.len(), 1, "{reg:?}");

        // **권한은 잠깐이 아니다**(moai-po6v) — 한때 `read_to_string` 이 진 것은 모두 `Reading` 이라
        // 걸음마다 다시 읽었다. 갈래를 가르는 잣대는 단계가 아니라 다시 읽어 볼 값이다
        // (`the_trouble_says_how_reading_again_could_ever_win`).
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
        let reg = read(Some(&path));
        assert_eq!(reg.trouble, Some(Trouble::Unreadable), "{reg:?}");
        assert_eq!(reg.problems.len(), 1, "{reg:?}");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    }

    /// **가르는 잣대는 다시 읽어 볼 값이다**(moai-po6v, 사용자 결정 2026-09-19) — 단계(읽기냐 파싱이냐)가
    /// 아니다. 한때 `read_to_string` 이 진 것은 모두 `Reading` 이라, 권한·디렉터리·UTF-8 아닌 바이트처럼
    /// 다시 해도 같은 것까지 걸음마다 다시 읽었다. 갈래마다 **어떻게 벗어나는지가 다르다**:
    ///
    /// - `Reading`(잠깐의 `EIO`·`ESTALE`)은 다음 걸음에 지나간다
    /// - `Unreadable`(권한)은 고쳐도 파일의 표식이 안 바뀌어 시계로만 다시 본다
    /// - `Broken`·`Gone` 은 고치면 파일이 바뀌어 표식이 그것을 낸다
    #[test]
    #[cfg(unix)]
    fn the_trouble_says_how_reading_again_could_ever_win() {
        use std::os::unix::fs::PermissionsExt;
        let d = scratch("trouble-again");
        let path = d.join("config.toml");

        std::fs::write(&path, "[[project]]\npath = \"/a\"\n").unwrap();
        assert_eq!(read(Some(&path)).trouble, None, "성한 파일에 탈이 섰다");

        std::fs::write(&path, "[[project]\npath = ").unwrap();
        assert_eq!(read(Some(&path)).trouble.map(Trouble::again), Some(Again::Never), "깨진 글은 고치면 파일이 바뀐다");

        // UTF-8 이 아닌 바이트 — 읽기가 지지만 다시 해도 같다.
        std::fs::write(&path, [0xff, 0xfe, 0x00, 0x41]).unwrap();
        let reg = read(Some(&path));
        assert_eq!(reg.trouble, Some(Trouble::Unreadable), "{reg:?}");
        assert_eq!(reg.problems.len(), 1, "까닭을 한 줄로 안 댔다 — {reg:?}");

        // 디렉터리를 설정 자리로 준 것도 마찬가지다.
        assert_eq!(read(Some(&d)).trouble, Some(Trouble::Unreadable), "디렉터리를 잠깐의 실패로 읽었다");

        // 권한은 고쳐도 표식이 안 바뀌어, 갈래는 같아도 벗어나는 길이 시계다.
        std::fs::write(&path, "[[project]]\npath = \"/a\"\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
        let reg = read(Some(&path));
        assert_eq!(reg.trouble.map(Trouble::again), Some(Again::Clock), "권한을 걸음마다 다시 읽는다 — {reg:?}");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

        // **파일이 없는 것은 사람에게 댈 까닭이 아니다** — 아직 아무것도 등록 안 한 사람의 정상이다.
        // 다만 탐색기는 그것을 알아야 한다: 들고 있던 층과 읽음을 빈 것으로 갈아 끼우지 않으려면
        // "없다" 와 "읽었더니 비었다" 를 갈라야 한다(autofs·sshfs 홈이 끊긴 자리).
        let reg = read(Some(&d.join("없음/config.toml")));
        assert_eq!(reg.trouble, Some(Trouble::Gone), "사라진 파일을 읽었더니 빈 것으로 읽었다");
        assert!(reg.problems.is_empty(), "없는 파일로 잔소리를 했다 — {reg:?}");
        assert_eq!(read(None).trouble, None, "자리를 모르는 것은 읽다 만난 탈이 아니다");
    }

    /// 깨진 파일은 읽기에서 알리고 계속, 쓰기에서 멈춘다. 파일은 한 글자도 안 바뀐다. `project` 가 표 배열이
    /// 아닌 것도 목록을 고치는 쓰기는 멈춘다 — 그 키가 무엇인지 모르는 채로 더하면 남의 값을 덮는다.
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

            let file = std::fs::canonicalize(&path).unwrap().display().to_string();
            for e in [
                update(&path, |doc| doc.add(Path::new("/b"))).unwrap_err(),
                update(&path, |doc| doc.remove(&["/a".into()])).unwrap_err(),
                update(&path, |doc| doc.set_hue(&["/a".into()], Hue::named("green"))).unwrap_err(),
            ] {
                assert_eq!(e.code, code::BROKEN, "{src:?} → {e}");
                // 손으로 고치라는 거절은 **어느 파일인지** 댄다(moai-gmdu 에픽 리뷰) — 설정의 자리는 환경이 골라
                // 사람이 모를 수 있다. 목록의 모양 거절(문서가 내는 것)도 깨진 파일 거절(`update` 가 내는 것)과 같다.
                assert!(e.message.starts_with(&format!("{file}: ")), "{src:?} → {e}");
            }
            assert_eq!(std::fs::read_to_string(&path).unwrap(), src, "깨진 파일을 덮어썼다");
        }
    }

    /// **`project` 의 모양은 그 키를 쓰는 자리만 막는다**(moai-aguj). `project = [{ … }]` 하나로 보기·읽음·
    /// 언어까지 못 읽고 못 적던 것 — 엄함은 지금 쓰는 줄에 대한 것이다.
    #[test]
    fn an_odd_project_key_blocks_only_the_project_list() {
        let d = scratch("odd-project");
        let path = d.join("config.toml");
        let src = "project = [{ path = \"/a\" }]\n\n[i18n]\nlang = \"en\"\n\n[tui]\nsort = \"title\"\n\n[read]\n\"m-0001\" = \"T\"\n";
        std::fs::write(&path, src).unwrap();
        let reg = read(Some(&path));
        assert!(reg.projects.is_empty(), "{reg:?}");
        assert!(reg.problems.len() == 1 && reg.problems[0].contains("[[project]]"), "{reg:?}");
        assert!(reg.look_problems.is_empty(), "보기가 목록의 모양 때문에 못 읽혔다 — {reg:?}");
        assert_eq!((reg.look.sort.as_deref(), reg.lang.as_deref()), (Some("title"), Some("en")));
        assert_eq!(reg.read.get("m-0001").map(String::as_str), Some("T"));

        // 보기는 적힌다. 목록은 그대로다.
        update(&path, |doc| doc.merge_look(&reg.look, &Look { sort: Some("created".into()), ..reg.look.clone() })).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("project = [{ path = \"/a\" }]\n"), "{text}");
        assert_eq!(read(Some(&path)).look.sort.as_deref(), Some("created"), "{text}");
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
        update(&path, |doc| doc.remove(&["/b".into()])).unwrap();
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

        assert_eq!(update(&path, |doc| doc.set_hue(&["/a".into()], green)).unwrap(), 1);
        let after = std::fs::read_to_string(&path).unwrap();
        assert!(after.contains("color = \"green\"") && after.contains("alias = \"일\"") && after.contains("[ui]"), "{after}");
        assert_eq!(read(Some(&path)).projects[0].hue, green);
        assert_eq!(read(Some(&path)).projects[1].hue, None, "남의 줄에 색이 붙었다");

        let before = std::fs::metadata(&path).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert_eq!(update(&path, |doc| doc.set_hue(&["/a".into()], green)).unwrap(), 1);
        assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), before, "같은 색인데 다시 썼다");

        assert_eq!(update(&path, |doc| doc.set_hue(&["/a".into()], None)).unwrap(), 1);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), src, "auto 로 되돌렸는데 바이트가 다르다");
        assert_eq!(update(&path, |doc| doc.set_hue(&["/없음".into()], green)).unwrap(), 0);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), src);

        // 값을 바꿀 때 그 값 뒤의 주석도, 키 위의 주석과 키 모양도 들고 간다 — 값만 바꾼 것이다.
        std::fs::write(&path, "[[project]]\npath = \"/a\"\n# 회사 색\ncolor   = \"blue\"  # 회사 것\n").unwrap();
        update(&path, |doc| doc.set_hue(&["/a".into()], Hue::named("cyan"))).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "[[project]]\npath = \"/a\"\n# 회사 색\ncolor   = \"cyan\"  # 회사 것\n"
        );
    }

    /// 손으로 적은 표 모양 `color`(점 키·하위 표)는 색으로도 `auto` 로도 **덮지 않는다** —
    /// 무엇을 뜻했는지 모르는 값을 지우면 되돌릴 수 없다. 겹쳐 적은 줄 중 하나만 그래도
    /// 하나도 안 바꾼다. 문자열이 아닌 값(`color = 3`)은 틀린 색이라 고쳐 쓴다.
    #[test]
    fn a_hand_written_table_colour_is_refused_not_overwritten() {
        let d = scratch("hue-table");
        let path = d.join("config.toml");
        for src in [
            "[[project]]\npath = \"/a\"\ncolor.x = 1  # 내 것\n",
            "[[project]]\npath = \"/a\"\n\n[project.color]\nx = 1\n",
            "[[project]]\npath = \"/a\"\ncolor = { x = 1 }\n",
            "[[project]]\npath = \"/a\"\ncolor = [\"green\"]\n",
            "[[project]]\npath = \"/a\"\n\n[[project.color]]\nx = 1\n",
            "[[project]]\npath = \"/a\"\ncolor = \"red\"\n\n[[project]]\npath = \"/a\"\ncolor.x = 1\n",
        ] {
            std::fs::write(&path, src).unwrap();
            for hue in [Hue::named("green"), None] {
                let e = update(&path, |doc| doc.set_hue(&["/a".into()], hue)).unwrap_err();
                // 어느 줄인지 경로로 댄다 — 사람이 그 줄을 찾아 고친다.
                assert!(e.message.contains("/a 의 `color`") && e.message.contains("손으로"), "{}", e.message);
                // 손으로 고칠 거절은 깨진 설정과 같은 코드다(moai-3owm) — I/O 실패(`error`)와 갈린다.
                assert_eq!(e.code, code::BROKEN);
                assert_eq!(std::fs::read_to_string(&path).unwrap(), src, "표 모양 color 를 덮었다");
            }
            // 남의 줄 것은 막지 않는다 — 엄함은 지금 쓰는 줄에 대한 것이다.
            assert_eq!(update(&path, |doc| doc.set_hue(&["/b".into()], Hue::named("green"))).unwrap(), 0);
        }

        std::fs::write(&path, "[[project]]\npath = \"/a\"\ncolor = 3\n").unwrap();
        assert_eq!(update(&path, |doc| doc.set_hue(&["/a".into()], Hue::named("green"))).unwrap(), 1);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "[[project]]\npath = \"/a\"\ncolor = \"green\"\n");
    }

    /// 끝 줄바꿈 없이 손으로 적은 파일도 더했다 빼거나 색을 정했다 되돌리면 처음 바이트다 —
    /// 줄바꿈이 하나씩 붙으면 dotfiles 저장소에 헛 변경이 선다. 줄바꿈으로 끝나는 파일은 그대로다.
    #[test]
    fn a_file_without_a_final_newline_round_trips() {
        let d = scratch("no-eol");
        let path = d.join("config.toml");
        for src in [
            "theme = \"dark\"",
            "[[project]]\npath = \"/a\"",
            "[[project]]\npath = \"/a\"\n",
            "[[project]]\npath = \"/a\"   # 끝 주석",
            "[[project]]\npath = \"/a\"\n\n[ui]\nx = 1",
            "\u{feff}theme = \"dark\"",
        ] {
            std::fs::write(&path, src).unwrap();
            assert!(update(&path, |doc| doc.add(Path::new("/z"))).unwrap());
            let added = std::fs::read_to_string(&path).unwrap();
            assert_eq!(read(Some(&path)).projects.last().map(|p| p.path.clone()), Some("/z".into()), "{added}");
            assert_eq!(added.ends_with('\n'), src.ends_with('\n'), "끝 줄바꿈 모양이 바뀌었다\n{added:?}");
            assert_eq!(update(&path, |doc| doc.remove(&["/z".into()])).unwrap(), 1);
            assert_eq!(std::fs::read_to_string(&path).unwrap(), src, "add/rm 왕복");

            if src.contains("/a") {
                assert_eq!(update(&path, |doc| doc.set_hue(&["/a".into()], Hue::named("green"))).unwrap(), 1);
                assert_eq!(update(&path, |doc| doc.set_hue(&["/a".into()], None)).unwrap(), 1);
                assert_eq!(std::fs::read_to_string(&path).unwrap(), src, "color 왕복");
            }
        }
    }

    /// **줄 끝은 원문의 많은 쪽을 따른다**(moai-lb0u, 사용자 결정 2026-09-18). 모두 CRLF 인 파일은 쓰기를
    /// 지나도 CRLF 이고 더했다 빼면 처음 바이트다. 섞인 파일은 많은 쪽 하나로 선다.
    #[test]
    fn line_endings_follow_the_most_lines() {
        let d = scratch("crlf");
        let path = d.join("config.toml");
        let all = "# 머리\r\n[tui]\r\nsort = \"title\"\r\n\r\n[[project]]\r\npath = \"/a\"\r\n";
        for src in [all, all.trim_end(), "\u{feff}# 머리\r\n\r\n"] {
            std::fs::write(&path, src).unwrap();
            assert!(update(&path, |doc| doc.add(Path::new("/z"))).unwrap());
            let added = std::fs::read_to_string(&path).unwrap();
            assert!(!added.replace("\r\n", "").contains('\n'), "LF 로 접힌 줄이 있다\n{added:?}");
            assert_eq!(update(&path, |doc| doc.remove(&["/z".into()])).unwrap(), 1);
            assert_eq!(std::fs::read_to_string(&path).unwrap(), src, "add/rm 왕복");
        }
        // 보기·색도 같은 렌더를 지난다.
        std::fs::write(&path, all).unwrap();
        update(&path, |doc| doc.merge_look(&Look::default(), &Look { detail: Some(true), ..Look::default() })).unwrap();
        update(&path, |doc| doc.set_hue(&["/a".into()], Hue::named("green"))).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.replace("\r\n", "").contains('\n'), "{text:?}");

        // 섞였으면 많은 쪽이다 — 같으면 LF.
        for (src, crlf) in [("a = 1\r\nb = 2\r\nc = 3\n", true), ("a = 1\r\nb = 2\nc = 3\n", false), ("a = 1\r\nb = 2\n", false)] {
            std::fs::write(&path, src).unwrap();
            assert!(update(&path, |doc| doc.add(Path::new("/z"))).unwrap());
            let text = std::fs::read_to_string(&path).unwrap();
            let lf = text.matches('\n').count() - text.matches("\r\n").count();
            assert!(if crlf { lf == 0 } else { !text.contains('\r') }, "{src:?} → {text:?}");
        }
    }

    /// **여러 줄 문자열 안의 줄바꿈은 줄 끝이 아니라 값이다**(moai-gmdu 에픽 리뷰) — 세지도 고치지도 않는다. 고치면
    /// 모르는 키의 값이 바뀌고, 세면 LF 파일에 붙여 넣은 CRLF 문자열 하나가 파일 전체를 CRLF 로 뒤집는다.
    #[test]
    fn newlines_inside_multiline_strings_are_values_not_line_endings() {
        let memo = |doc: &Doc| doc.doc.get("memo").and_then(Item::as_str).map(String::from);
        // CRLF 파일의 LF 문자열 — 줄은 CRLF 로 서고 값은 그대로다.
        let src = "a = 1\r\nb = 2\r\nmemo = \"\"\"\nx\ny\"\"\"\r\nc = '''\nz'''\r\n";
        let mut doc = Doc::parse(src).unwrap();
        doc.add(Path::new("/z")).unwrap();
        let out = doc.render();
        let again = Doc::parse(&out).unwrap();
        assert_eq!(memo(&again).as_deref(), Some("x\ny"), "{out:?}");
        assert_eq!(again.doc.get("c").and_then(Item::as_str), Some("z"), "{out:?}");
        assert!(!out.replace("\r\n", "").replace("\"\"\"\nx\ny", "").replace("'''\nz", "").contains('\n'), "LF 로 선 줄이 있다 {out:?}");

        // LF 파일에 붙여 넣은 CRLF 문자열 — 파일은 LF 그대로고, 더했다 빼면 처음 바이트다.
        let src = "a = 1\nmemo = \"\"\"\r\n1\r\n2\r\n3\r\n4\r\n\"\"\"\n";
        let mut doc = Doc::parse(src).unwrap();
        assert!(!doc.crlf, "문자열 안의 CRLF 를 줄 끝으로 셌다");
        doc.add(Path::new("/z")).unwrap();
        let mut doc = Doc::parse(&doc.render()).unwrap();
        doc.remove(&["/z".into()]).unwrap();
        assert_eq!(doc.render(), src);

        // 이 도구가 적은 경로도 — 줄바꿈이 든 디렉터리 이름은 CRLF 파일에서도 그 이름으로 남아 도로 뺄 수 있다.
        let mut doc = Doc::parse("a = 1\r\nb = 2\r\n").unwrap();
        doc.add(Path::new("/tmp/a\nb")).unwrap();
        let mut doc = Doc::parse(&doc.render()).unwrap();
        assert_eq!(doc.projects().0, [Project { path: "/tmp/a\nb".into(), hue: None }]);
        assert_eq!(doc.remove(&["/tmp/a\nb".into()]).unwrap(), 1);
        assert_eq!(doc.render(), "a = 1\r\nb = 2\r\n");
    }

    /// **주석만 있는 설정의 머리 주석은 머리에 남는다**(moai-bx7g) — 라이브러리가 그 글을 끝 글로 들어
    /// 새 `[[project]]` 가 그 앞에 섰다. 도로 빼면 처음 바이트로 돌아온다.
    #[test]
    fn a_head_comment_stays_above_the_first_project() {
        let d = scratch("head-comment");
        let path = d.join("config.toml");
        for (src, want) in [
            ("# 내 설정\n", "# 내 설정\n\n[[project]]\npath = \"/z\"\n"),
            ("# 내 설정\n\n", "# 내 설정\n\n\n[[project]]\npath = \"/z\"\n"),
            ("# 내 설정", "# 내 설정\n\n[[project]]\npath = \"/z\""),
            ("# a\n\n# b\n", "# a\n\n# b\n\n[[project]]\npath = \"/z\"\n"),
            ("\u{feff}# 내 설정\n", "\u{feff}# 내 설정\n\n[[project]]\npath = \"/z\"\n"),
        ] {
            std::fs::write(&path, src).unwrap();
            assert!(update(&path, |doc| doc.add(Path::new("/z"))).unwrap());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), want, "{src:?}");
            assert!(update(&path, |doc| doc.add(Path::new("/y"))).unwrap());
            assert_eq!(read(Some(&path)).projects.len(), 2);
            assert_eq!(update(&path, |doc| doc.remove(&["/y".into(), "/z".into()])).unwrap(), 2);
            assert_eq!(std::fs::read_to_string(&path).unwrap(), src, "add/rm 왕복");
        }

        // 손으로 적은 파일도 같다 — 빈 줄로 떨어진 주석은 남고 바로 위에 붙은 주석은 표와 함께 빠진다.
        // 남은 글은 뺀 표 뒤에 그려지던 것 앞에 선다.
        for (src, gone, want) in [
            ("# 목록\n\n# a 는 일\n[[project]]\npath = \"/a\"\n", "/a", "# 목록\n"),
            (
                "# 목록\n\n[[project]]\npath = \"/a\"\n\n[[project]]\npath = \"/b\"\n",
                "/a",
                "# 목록\n\n[[project]]\npath = \"/b\"\n",
            ),
            (
                "# 목록\n\n[[project]]\npath = \"/a\"\n\n[tui]\nsort = \"title\"\n# 꼬리\n",
                "/a",
                "# 목록\n\n[tui]\nsort = \"title\"\n# 꼬리\n",
            ),
            (
                "[tui]\nsort = \"title\"\n\n# 여기부터 목록\n\n[[project]]\npath = \"/a\"\n",
                "/a",
                "[tui]\nsort = \"title\"\n\n# 여기부터 목록\n",
            ),
            ("# 붙은 주석\n[[project]]\npath = \"/a\"\n", "/a", ""),
            // 빈칸·탭뿐인 줄도 빈 줄이다(moai-gmdu 에픽 리뷰) — 눈에는 같은 빈 줄이다.
            ("# 목록\n  \n[[project]]\npath = \"/a\"\n", "/a", "# 목록\n"),
            ("# 목록\n\t\r\n# a 는 일\n[[project]]\npath = \"/a\"\n", "/a", "# 목록\n"),
            // 뒤의 표가 뺀 표에 빈 줄 없이 붙어 있었으면 남긴 글과 그 표 사이에 빈 줄을 둔다(moai-gmdu 에픽
            // 리뷰) — 안 두면 남긴 글이 그 표의 머리에 붙어, 그 표를 뺄 때 함께 지워진다.
            (
                "# 목록\n\n[[project]]\npath = \"/a\"\n# b 는 일\n[[project]]\npath = \"/b\"\n",
                "/a",
                "# 목록\n\n# b 는 일\n[[project]]\npath = \"/b\"\n",
            ),
            (
                "# 목록\n\n[[project]]\npath = \"/a\"\n[[project]]\npath = \"/b\"\n",
                "/a",
                "# 목록\n\n[[project]]\npath = \"/b\"\n",
            ),
        ] {
            std::fs::write(&path, src).unwrap();
            assert_eq!(update(&path, |doc| doc.remove(&[gone.into()])).unwrap(), 1, "{src:?}");
            let left = std::fs::read_to_string(&path).unwrap();
            assert_eq!(left, want, "{src:?}");
            // 남긴 주석은 남은 표를 마저 빼도 남는다 — 한 번 남긴 것이 다음 `remove` 에 지워지면 남긴 뜻이 없다.
            if left.contains("/b") {
                assert_eq!(update(&path, |doc| doc.remove(&["/b".into()])).unwrap(), 1, "{src:?}");
                assert!(std::fs::read_to_string(&path).unwrap().starts_with("# 목록\n"), "{src:?} → 둘째 rm 이 남긴 주석을 지웠다");
            }
        }
    }

    /// **표를 세우는 쓰기는 모두 머리 주석을 머리에 둔다**(moai-gmdu 에픽 리뷰) — 등록만 옮기면 주석만 있던 설정에
    /// 처음 적는 것이 보기(`[tui]`)나 읽음(`[read]`)일 때 머리 주석이 새 표 밑으로 밀린다.
    #[test]
    fn a_head_comment_stays_above_the_first_table() {
        let look = Look { detail: Some(true), ..Look::default() };
        let mut doc = Doc::parse("# 내 설정\n").unwrap();
        doc.merge_look(&Look::default(), &look).unwrap();
        assert_eq!(doc.render(), "# 내 설정\n\n[tui]\ndetail = true\n");
        // 보기를 먼저 적고 등록했다 빼도 머리 주석은 머리에 있다.
        let mut doc = Doc::parse(&doc.render()).unwrap();
        doc.add(Path::new("/z")).unwrap();
        let mut doc = Doc::parse(&doc.render()).unwrap();
        doc.remove(&["/z".into()]).unwrap();
        assert_eq!(doc.render(), "# 내 설정\n\n[tui]\ndetail = true\n");
    }

    /// 머리로 옮기는 것은 **키도 표도 없는** 설정의 주석뿐이다 — 키나 표가 있는 파일의 끝 글은 앞의 것의 꼬리일
    /// 수 있어 그 자리에 둔다. 빈칸뿐인 설정은 옮길 주석이 없어 더했다 빼면 처음 바이트다(moai-gmdu 에픽 리뷰 —
    /// 두 조건을 지키는 시험이 없었다).
    #[test]
    fn only_a_comment_only_config_lifts_its_comment() {
        let mut doc = Doc::parse("[[project]]\npath = \"/a\"\n# 꼬리\n").unwrap();
        doc.add(Path::new("/z")).unwrap();
        let added = doc.render();
        assert!(added.starts_with("[[project]]\npath = \"/a\"\n") && added.ends_with("# 꼬리\n"), "{added:?}");
        for src in ["\n\n", "  \n"] {
            let mut doc = Doc::parse(src).unwrap();
            doc.add(Path::new("/z")).unwrap();
            let mut doc = Doc::parse(&doc.render()).unwrap();
            doc.remove(&["/z".into()]).unwrap();
            assert_eq!(doc.render(), src, "{src:?}");
        }
    }

    /// **남긴 글은 글의 차례를 안 바꾼다**(moai-bx7g) — 뺀 표 **바로 다음에** 그려지던 것 앞에, 여럿이면 파일에
    /// 있던 차례로, 파일 끝이면 끝 글 **앞에** 선다(moai-gmdu 에픽 리뷰 — 차례를 지키는 시험이 없었다).
    #[test]
    fn kept_heads_keep_the_order_of_the_text() {
        let src = "# 일\n\n[[project]]\npath = \"/w\"\n\n# 집\n\n[[project]]\npath = \"/h\"\n\n[tui]\nsort = \"title\"\n";
        let mut doc = Doc::parse(src).unwrap();
        assert_eq!(doc.remove(&["/w".into()]).unwrap(), 1);
        assert_eq!(doc.render(), "# 일\n\n# 집\n\n[[project]]\npath = \"/h\"\n\n[tui]\nsort = \"title\"\n");

        // 한 번에 둘을 빼면(링크 철자와 푼 철자) 남긴 글은 파일에 있던 차례대로 선다.
        let mut doc = Doc::parse(src).unwrap();
        assert_eq!(doc.remove(&["/w".into(), "/h".into()]).unwrap(), 2);
        assert_eq!(doc.render(), "# 일\n\n# 집\n\n[tui]\nsort = \"title\"\n");

        let mut doc = Doc::parse("[tui]\nsort = \"title\"\n\n# 목록\n\n[[project]]\npath = \"/a\"\n# 파일 끝\n").unwrap();
        assert_eq!(doc.remove(&["/a".into()]).unwrap(), 1);
        assert_eq!(doc.render(), "[tui]\nsort = \"title\"\n\n# 목록\n# 파일 끝\n");
    }

    /// **키를 지울 때도 빈 줄로 떨어진 위 주석은 남는다**(moai-liij) — 표를 뺄 때의 자(moai-bx7g)와 같다. 바로
    /// 위에 붙은 주석은 그 키의 것이라 함께 빠진다. 남은 글은 다음 키 앞에, 끝 키였으면 표 다음 것 앞에 선다.
    #[test]
    fn dropping_a_key_keeps_the_comment_a_blank_line_away() {
        let hue = |src: &str| {
            let mut doc = Doc::parse(src).unwrap();
            assert_eq!(doc.set_hue(&["/a".into()], None).unwrap(), 1);
            doc.render()
        };
        // gmdu 에픽 리뷰가 본 자리 — 그 주석은 밑의 `name` 의 머리이기도 했다.
        assert_eq!(
            hue("[[project]]\npath = \"/a\"\n\n# --- hand tweaks below ---\n\ncolor = \"cyan\"\nname = \"일\"\n"),
            "[[project]]\npath = \"/a\"\n\n# --- hand tweaks below ---\n\nname = \"일\"\n"
        );
        // 끝 키면 표 다음 것 앞에, 파일 끝이면 끝 글 앞에 선다.
        assert_eq!(
            hue("[[project]]\npath = \"/a\"\n# 색\n\ncolor = \"cyan\"\n\n[tui]\nsort = \"title\"\n"),
            "[[project]]\npath = \"/a\"\n# 색\n\n[tui]\nsort = \"title\"\n"
        );
        assert_eq!(hue("[[project]]\npath = \"/a\"\n# 색\n\ncolor = \"cyan\"\n# 끝\n"), "[[project]]\npath = \"/a\"\n# 색\n# 끝\n");
        // 바로 위에 붙은 주석은 그 키의 것이다.
        assert_eq!(hue("[[project]]\npath = \"/a\"\n\n# 색\ncolor = \"cyan\"\n"), "[[project]]\npath = \"/a\"\n");

        // 보기 키도 같다. 끝 키 둘을 한 번에 지우면 남은 글은 파일에 있던 차례로 선다.
        let src = "[tui]\n# 위\n\nsort = \"title\"\n# 차례\n\nsort_reversed = true\n# 방향\n\ndetail = false\n\n[x]\n";
        let base = Look { sort: Some("title".into()), sort_reversed: Some(true), detail: Some(false), ..Look::default() };
        let mut doc = Doc::parse(src).unwrap();
        doc.merge_look(&base, &Look { sort: None, sort_reversed: None, detail: None, ..base.clone() }).unwrap();
        assert_eq!(doc.render(), "[tui]\n# 위\n\n# 차례\n\n# 방향\n\n[x]\n");
    }

    /// **배열 원소를 뺄 때도 빈 줄로 떨어진 위 주석은 남는다**(moai-liij). 원소의 머리는 앞 쉼표 바로 뒤에서
    /// 시작해 첫 줄바꿈은 앞 줄의 끝이다 — 빈 줄로 세면 붙은 주석까지 남긴다.
    #[test]
    fn dropping_an_element_keeps_the_comment_a_blank_line_away() {
        let hide = |src: &str, gone: &[&str]| {
            let words = |w: &[&str]| Some(w.iter().map(|s| s.to_string()).collect::<Vec<_>>());
            let all = ["todo", "done", "review"];
            let base = Look { hidden: words(&all), ..Look::default() };
            let kept: Vec<&str> = all.iter().copied().filter(|w| !gone.contains(w)).collect();
            let mut doc = Doc::parse(src).unwrap();
            doc.merge_look(&base, &Look { hidden: words(&kept), ..base.clone() }).unwrap();
            doc.render()
        };
        let src = "[tui]\nhidden = [\n  \"todo\",\n  # 끝난 것\n\n  # 끝\n  \"done\",\n  # 리뷰\n\n  \"review\",\n]\n";
        assert_eq!(hide(src, &["done"]), "[tui]\nhidden = [\n  \"todo\",\n  # 끝난 것\n\n  # 리뷰\n\n  \"review\",\n]\n");
        assert_eq!(hide(src, &["review"]), "[tui]\nhidden = [\n  \"todo\",\n  # 끝난 것\n\n  # 끝\n  \"done\",\n  # 리뷰\n]\n");
        assert_eq!(hide(src, &["done", "review"]), "[tui]\nhidden = [\n  \"todo\",\n  # 끝난 것\n\n  # 리뷰\n]\n");
        // 한 줄 배열은 그대로 한 줄이다.
        assert_eq!(hide("[tui]\nhidden = [\"todo\", \"done\", \"review\"]\n", &["done"]), "[tui]\nhidden = [\"todo\", \"review\"]\n");
    }

    /// **원소를 빼도 남는 원소의 줄은 그대로다**(moai-1upp 에픽 리뷰). 원소 머리의 첫 줄은 앞 원소의 줄 끝(그 줄 끝 주석,
    /// `[` 줄의 주석)이라 남고, 뒤 자리의 첫 줄은 뺀 원소의 줄 끝이라 함께 빠진다 — 뒤집으면 남는 원소가 제 주석을
    /// 잃고 뺀 원소의 주석을 달았다. 남긴 글은 제 줄에 서고, 끝 쉼표 없는 배열의 `]` 도 제 줄에 남는다.
    #[test]
    fn dropping_an_element_keeps_the_neighbours_line() {
        let hide = |src: &str, all: &[&str], gone: &[&str]| {
            let words = |w: &[&str]| Some(w.iter().map(|s| s.to_string()).collect::<Vec<_>>());
            let kept: Vec<&str> = all.iter().copied().filter(|w| !gone.contains(w)).collect();
            let base = Look { hidden: words(all), ..Look::default() };
            let mut doc = Doc::parse(src).unwrap();
            doc.merge_look(&base, &Look { hidden: words(&kept), ..base.clone() }).unwrap();
            doc.render()
        };
        let (two, three) = (["done", "review"], ["todo", "done", "review"]);
        let src = "[tui]\nhidden = [\n  \"done\",    # 끝난 일\n  \"review\",  # 리뷰 중\n]\n";
        assert_eq!(hide(src, &two, &["review"]), "[tui]\nhidden = [\n  \"done\",    # 끝난 일\n]\n");
        assert_eq!(hide(src, &two, &["done"]), "[tui]\nhidden = [\n  \"review\",  # 리뷰 중\n]\n");
        let src = "[tui]\nhidden = [\n  \"todo\",\n  # 남길 것\n\n  \"done\", # 끝\n  \"review\",\n]\n";
        assert_eq!(hide(src, &three, &["done"]), "[tui]\nhidden = [\n  \"todo\",\n  # 남길 것\n\n  \"review\",\n]\n");
        // `[` 줄의 주석은 첫 원소를 빼도 남는다.
        let src = "[tui]\nhidden = [  # 안 볼 칸\n  \"done\",\n  \"review\",\n]\n";
        assert_eq!(hide(src, &two, &["done"]), "[tui]\nhidden = [  # 안 볼 칸\n  \"review\",\n]\n");
        // 끝 쉼표가 없으면 `]` 앞 글이 끝 원소의 꼬리에 있다 — 남긴 글도 `]` 도 제 줄에 선다.
        let src = "[tui]\nhidden = [\n  \"todo\",\n  # 남길 것\n\n  \"done\"\n]\n";
        assert_eq!(hide(src, &["todo", "done"], &["done"]), "[tui]\nhidden = [\n  \"todo\"\n  # 남길 것\n]\n");
        // 뒤 원소가 뺀 원소와 한 줄이었으면 그 줄을 이어받는다. 한 줄에 둘이 있던 줄은 남는다.
        let src = "[tui]\nhidden = [\n  # 남길 것\n\n  \"todo\", \"done\", \"review\"\n]\n";
        assert_eq!(hide(src, &three, &["todo"]), "[tui]\nhidden = [\n  # 남길 것\n\n  \"done\", \"review\"\n]\n");
        let src = "[tui]\nhidden = [\n  \"todo\", \"done\", # 둘\n  \"review\",\n]\n";
        assert_eq!(hide(src, &three, &["done"]), "[tui]\nhidden = [\n  \"todo\", # 둘\n  \"review\",\n]\n");
    }

    /// **낱말을 더해도 여러 줄로 벌린 모양은 그대로다**(moai-n3ku, [`push_word`]). 끝 원소와 `]` 사이 글의 첫
    /// 줄은 그 원소의 줄 끝이라 쉼표 앞에 남고, 더한 낱말은 앞 원소와 같은 들여쓰기로 제 줄에 선다.
    /// 옮기지 않던 판은 쉼표가 줄 머리에 서고 `]` 가 끌려 올라왔으며(`"done"\n, "review"]`), 줄 끝 주석이
    /// 있으면 쉼표가 그 주석 뒤로 가 파일이 통째로 안 읽혔다.
    #[test]
    fn adding_a_word_keeps_the_multiline_shape() {
        let show = |src: &str, base: &[&str], new: &[&str]| {
            let words = |w: &[&str]| Some(w.iter().map(|s| s.to_string()).collect::<Vec<_>>());
            let b = Look { hidden: words(base), ..Look::default() };
            let mut doc = Doc::parse(src).unwrap();
            doc.merge_look(&b, &Look { hidden: words(new), ..b.clone() }).unwrap();
            doc.render()
        };
        let (two, three) = (["todo", "done"], ["todo", "done", "review"]);
        // 끝 쉼표가 없는 배열 — `]` 앞 글이 끝 원소의 꼬리에 있다.
        let src = "[tui]\nhidden = [\n  \"todo\",\n  \"done\"\n]\n";
        assert_eq!(show(src, &two, &three), "[tui]\nhidden = [\n  \"todo\",\n  \"done\",\n  \"review\"\n]\n");
        // 끝 쉼표가 있으면 그대로 두고 그 아래에 선다.
        let src = "[tui]\nhidden = [\n  \"todo\",\n  \"done\",\n]\n";
        assert_eq!(show(src, &two, &three), "[tui]\nhidden = [\n  \"todo\",\n  \"done\",\n  \"review\",\n]\n");
        // 끝 원소의 줄 끝 주석은 그 줄에 남고 쉼표가 그 앞에 선다 — 뒤로 가면 쉼표가 주석에 먹힌다.
        let src = "[tui]\nhidden = [\n  \"todo\",\n  \"done\"  # 끝\n]\n";
        assert_eq!(show(src, &two, &three), "[tui]\nhidden = [\n  \"todo\",\n  \"done\",  # 끝\n  \"review\"\n]\n");
        let src = "[tui]\nhidden = [\n  \"todo\",\n  \"done\", # 끝\n]\n";
        assert_eq!(show(src, &two, &three), "[tui]\nhidden = [\n  \"todo\",\n  \"done\", # 끝\n  \"review\",\n]\n");
        // `]` 앞 제 줄에 선 주석은 `]` 앞에 그대로 남는다.
        let src = "[tui]\nhidden = [\n  \"todo\",\n  \"done\"\n  # 끝에\n]\n";
        assert_eq!(show(src, &two, &three), "[tui]\nhidden = [\n  \"todo\",\n  \"done\",\n  \"review\"\n  # 끝에\n]\n");
        // 여럿을 한 번에 더해도 줄마다 선다. 들여쓰기는 줄을 연 마지막 원소의 것이다.
        let src = "[tui]\nhidden = [\n    \"todo\"\n]\n";
        assert_eq!(show(src, &["todo"], &three), "[tui]\nhidden = [\n    \"todo\",\n    \"done\",\n    \"review\"\n]\n");
        let src = "[tui]\nhidden = [\n  \"todo\", \"done\"\n]\n";
        assert_eq!(show(src, &two, &three), "[tui]\nhidden = [\n  \"todo\", \"done\",\n  \"review\"\n]\n");
        // 한 줄 배열과 빈 배열은 한 줄 그대로다.
        assert_eq!(show("[tui]\nhidden = [\"todo\", \"done\"]\n", &two, &three), "[tui]\nhidden = [\"todo\", \"done\", \"review\"]\n");
        assert_eq!(show("[tui]\nhidden = []\n", &[], &["todo"]), "[tui]\nhidden = [\"todo\"]\n");
        // **빈 여러 줄 배열도 여러 줄이다**(리뷰) — 마지막 낱말을 뺀 자리가 이 모양이라, 껐다 켜는 것만으로
        // 여기를 지난다. 본뜰 원소가 없어 들여쓰기는 `]` 앞 글의 주석에서 들고, 그것도 없으면 안 짓는다.
        assert_eq!(show("[tui]\nhidden = [\n]\n", &[], &["todo", "done"]), "[tui]\nhidden = [\n\"todo\",\n\"done\"\n]\n");
        let src = "[tui]\nhidden = [\n  # 아직 없다\n]\n";
        assert_eq!(show(src, &[], &["todo"]), "[tui]\nhidden = [\n  \"todo\"\n  # 아직 없다\n]\n");
    }

    /// **켰다 끄면 옛 바이트로 돌아온다**(moai-n3ku). 더할 때 모양이 뒤집히면 뺄 때 그 모양을 되돌리지
    /// 못해, 토글마다 설정 파일이 헛 diff 를 냈다.
    ///
    /// 마지막 줄의 빈 배열 둘은 **뺀 자리에서 나는 모양**이다(리뷰) — 낱말이 하나뿐인 배열을 끄면 `[\n]`
    /// 이 되고, 다시 켜는 것이 곧 이 왕복이다.
    #[test]
    fn adding_a_word_and_dropping_it_again_restores_the_bytes() {
        for (src, base, more) in [
            ("[tui]\nhidden = [\n  \"todo\",\n  \"done\"\n]\n", &["todo", "done"][..], &["todo", "done", "review"][..]),
            ("[tui]\nhidden = [\n  \"todo\",\n  \"done\",\n]\n", &["todo", "done"], &["todo", "done", "review"]),
            ("[tui]\nhidden = [\n  \"todo\",\n  \"done\"  # 끝\n]\n", &["todo", "done"], &["todo", "done", "review"]),
            ("[tui]\nhidden = [\n  \"todo\",\n  \"done\", # 끝\n]\n", &["todo", "done"], &["todo", "done", "review"]),
            ("[tui]\nhidden = [\n  \"todo\",\n  \"done\"\n  # 끝에\n]\n", &["todo", "done"], &["todo", "done", "review"]),
            ("[tui]\nhidden = [\"todo\", \"done\"]\n", &["todo", "done"], &["todo", "done", "review"]),
            ("[tui]\nhidden = [\n]\n", &[], &["todo"]),
            ("[tui]\nhidden = [\n  # 아직 없다\n]\n", &[], &["todo"]),
        ] {
            let words = |w: &[&str]| Some(w.iter().map(|s| s.to_string()).collect::<Vec<_>>());
            let (base, more) =
                (Look { hidden: words(base), ..Look::default() }, Look { hidden: words(more), ..Look::default() });
            let mut doc = Doc::parse(src).unwrap();
            doc.merge_look(&base, &more).unwrap();
            let added = doc.render();
            let mut doc = Doc::parse(&added).unwrap();
            doc.merge_look(&more, &base).unwrap();
            assert_eq!(doc.render(), src, "켰다 끈 뒤 — 켠 모양은 {added:?}");
        }
    }

    /// **남긴 글은 점 키 줄 앞에도 선다**(moai-1upp 에픽 리뷰). 점 키(`meta.x = 1`)는 표 몸 안 제자리에 그려지고 그 줄의
    /// 머리를 맨 끝 조각이 든다 — 건너뛰면 남긴 글이 점 키 줄 밑으로 내려가 글의 차례가 바뀌었다. 뿌리에 점 키로 적은
    /// `tui.x` 는 위치가 없어 남긴 글이 버려졌다 — 설정 머리 주석이 그 첫 줄의 머리다.
    #[test]
    fn a_kept_comment_stays_above_a_dotted_key() {
        let mut doc = Doc::parse("[[project]]\npath = \"/a\"\n# --- 손본 것 ---\n\ncolor = \"cyan\"\nmeta.x = 1\nname = \"일\"\n").unwrap();
        assert_eq!(doc.set_hue(&["/a".into()], None).unwrap(), 1);
        assert_eq!(doc.render(), "[[project]]\npath = \"/a\"\n# --- 손본 것 ---\n\nmeta.x = 1\nname = \"일\"\n");

        let hidden = Look { hidden: Some(vec!["done".into()]), ..Look::default() };
        let mut doc = Doc::parse("[tui]\n# 보기\n\nhidden = [\"done\"]\nsort.by = \"title\"\n").unwrap();
        doc.merge_look(&hidden, &Look::default()).unwrap();
        assert_eq!(doc.render(), "[tui]\n# 보기\n\nsort.by = \"title\"\n");

        let sort = Look { sort: Some("title".into()), ..Look::default() };
        for (src, want) in [
            ("# 내 설정\n\ntui.sort = \"title\"\n\n[[project]]\npath = \"/a\"\n", "# 내 설정\n\n[[project]]\npath = \"/a\"\n"),
            ("# 내 설정\n\ntui.sort = \"title\"\n", "# 내 설정\n"),
            ("# 내 설정\n\ntui.sort = \"title\"\ni18n.lang = \"ko\"\n", "# 내 설정\n\ni18n.lang = \"ko\"\n"),
        ] {
            let mut doc = Doc::parse(src).unwrap();
            doc.merge_look(&sort, &Look::default()).unwrap();
            assert_eq!(doc.render(), want, "{src:?}");
        }
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
    /// 서고, 못 읽었거나 깨진 파일의 까닭은 층(`problems`)에만 선다(moai-5jsn). **글자로 견준다**
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
        assert!(broken.look_problems.is_empty(), "깨진 파일의 까닭을 보기에도 실었다 — 탐색기가 두 번 댄다");
        // 못 여는 자리(디렉터리)도 같다 — 없는 파일(NotFound)만 문제가 아니다.
        let unreadable = read(Some(&d));
        assert!(unreadable.problems.len() == 1 && unreadable.problems[0].starts_with(&format!("{}: ", d.display())), "{unreadable:?}");
        assert!(unreadable.look_problems.is_empty(), "못 읽은 파일의 까닭을 보기에도 실었다");
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

        // **까닭은 두 자리에 든다**(리뷰 moai-80qw) — `.moai` 밖의 한눈 보기가 대는 `problems`
        // 와, 저장소 안의 `status` 가 대는 `lang_problems`. 한쪽만 채우면 나머지 화면에서
        // 오타가 조용히 영어가 된다.
        let d = scratch("lang-problems");
        let path = d.join("config.toml");
        std::fs::write(&path, "[i18n]\nlang = \"kr\"\n").unwrap();
        let bad = read(Some(&path));
        let said = format!("{}: `i18n.lang` 은 en·ko·zh·ja·es 중 하나다 — \"kr\"", path.display());
        assert_eq!(bad.lang_problems, vec![said.clone()]);
        assert!(bad.problems.contains(&said), "밖에서 대는 자리에 안 섰다: {:?}", bad.problems);
        assert_eq!(bad.lang, None, "틀린 값을 들였다");
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

        // 차례를 못 읽으면 방향도 안 낸다(moai-ys7c) — 낱값이든 표 모양이든 같다. 까닭은 차례 하나만 선다.
        for sort in ["sort = 3", "sort = { by = \"title\" }", "sort.by = \"title\""] {
            let doc = Doc::parse(&format!("[tui]\n{sort}\nsort_reversed = true\n")).unwrap();
            let (look, problems) = doc.look();
            assert_eq!((look.sort, look.sort_reversed), (None, None), "{sort}");
            assert_eq!(problems.len(), 1, "{sort}: {problems:?}");
        }
        // 차례가 없으면 방향은 그대로 읽힌다 — 처음 차례에 입힐 방향이다.
        let doc = Doc::parse("[tui]\nsort_reversed = true\n").unwrap();
        assert_eq!(doc.look().0.sort_reversed, Some(true));

        let title = Look { sort: Some("title".into()), ..Look::default() };
        let mut odd = Doc::parse("tui = 3\n").unwrap();
        assert_eq!(odd.look().1.len(), 1);
        assert_eq!(odd.merge_look(&Look::default(), &title).unwrap_err().code, code::BROKEN);
        assert!(!odd.changed());

        // 읽히는 인라인 표는 쓰기도 받는다.
        let mut inline = Doc::parse("tui = { sort = \"created\" }\n").unwrap();
        assert_eq!(inline.look().0.sort.as_deref(), Some("created"));
        inline.merge_look(&Look::default(), &title).unwrap();
        assert!(inline.render().contains("sort = \"title\""), "{}", inline.render());
    }

    /// **바꿀 보기 키가 손으로 적은 표 모양이면 그 키만 안 적는다**(moai-j7r3, moai-jr3z) — 무엇을 덮지 않는가는
    /// `set_hue` 와 같은 자다(`plain`). 나머지 바뀐 키는 적고, 건너뛴 키는 까닭 한 줄로 낸다. 낱값의 틀린 값은
    /// 고쳐 쓰고, 이 세션이 안 바꾸는 키는 모양이 어떻든 막지 않는다.
    #[test]
    fn a_hand_written_table_look_key_is_refused_not_overwritten() {
        let title = Look { sort: Some("title".into()), ..Look::default() };
        let hide = Look { hidden: Some(vec!["done".into()]), ..Look::default() };
        let fields = Look { fields: Some(vec!["id".into()]), ..Look::default() };
        let deferred = Look { hide_deferred: Some(true), ..Look::default() };
        let detail = Look { detail: Some(true), ..Look::default() };
        // 탐색기는 저장마다 `fields_known` 을 싣는다 — 다른 키 하나만 바꿔도 이 키를 본다.
        let toggle = Look { fields_known: Some(vec!["id".into()]), ..detail.clone() };
        for (src, new, key) in [
            ("[tui]\nsort.by = \"created\"\n", &title, "sort"),
            ("[tui]\nsort = { by = \"created\" }\n", &title, "sort"),
            ("[tui]\nsort = [\"created\"]\n", &title, "sort"),
            ("[tui]\nsort_reversed.x = true\n", &title, "sort_reversed"),
            ("[tui.sort]\nby = \"created\"\n", &title, "sort"),
            ("[tui]\nhidden = { done = true }\n", &hide, "hidden"),
            ("[tui]\nhidden.done = true\n", &hide, "hidden"),
            // 거절을 재는 키마다 하나씩 — 한 줄이 빠져도 여기서 드러난다(moai-gmdu 에픽 리뷰).
            ("[tui.fields]\nid = true\n", &fields, "fields"),
            ("[tui]\nhide_deferred.x = true\n", &deferred, "hide_deferred"),
            ("[tui]\ndetail = { open = true }\n", &detail, "detail"),
            ("[tui]\nfields_known = { id = true }\n", &toggle, "fields_known"),
        ] {
            let mut doc = Doc::parse(src).unwrap();
            let skipped = doc.merge_look(&Look::default(), new).unwrap();
            assert!(skipped.len() == 1 && skipped[0].contains(&format!("`tui.{key}`")), "{src}: {skipped:?}");
            if std::ptr::eq(new, &toggle) {
                // 다른 키는 적힌다 — 한 키가 다른 키의 저장을 막지 않는다.
                let text = doc.render();
                assert!(text.contains("detail = true") && text.contains("fields_known = { id = true }"), "{text}");
            } else {
                assert!(!doc.changed(), "{src}");
                assert_eq!(doc.render(), src);
            }
        }
        // 차례가 표 모양이어도 숨김은 적힌다 — 차례와 방향은 한 벌로 건너뛴다.
        let mut doc = Doc::parse("[tui]\nsort.by = \"created\"\nsort_reversed = false\n").unwrap();
        let both = Look { sort_reversed: Some(true), hidden: hide.hidden.clone(), ..title.clone() };
        let skipped = doc.merge_look(&Look::default(), &both).unwrap();
        assert_eq!(skipped.len(), 1, "{skipped:?}");
        assert_eq!(doc.render(), "[tui]\nsort.by = \"created\"\nsort_reversed = false\nhidden = [\"done\"]\n");
        // 낱값의 틀린 값은 읽기가 까닭을 대는 값이다 — 고쳐 쓴다.
        let mut doc = Doc::parse("[tui]\nsort = 3\nhidden = \"done\"\n").unwrap();
        doc.merge_look(&Look::default(), &Look { hidden: hide.hidden.clone(), ..title.clone() }).unwrap();
        assert_eq!(doc.render(), "[tui]\nsort = \"title\"\nhidden = [\"done\"]\n");
        // 안 바꾸는 키는 모양이 어떻든 막지 않는다.
        let mut doc = Doc::parse("[tui]\nsort.by = \"created\"\n").unwrap();
        doc.merge_look(&Look::default(), &hide).unwrap();
        assert!(doc.render().contains("sort.by = \"created\""), "{}", doc.render());
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

    /// **읽음도 준 키만 손댄다**(moai-50mn) — `merge_look` 과 같은 약속이라 같은 자로 시험한다.
    /// **옛 `[read]` 는 읽기만 한다**(moai-bwce, 사용자 결정 3) — 이 바이너리가 다시 안 적으므로 쓰는
    /// 자는 [`crate::read_marks`] 로 옮겼다. 여기 남는 것은 겹쳐 볼 때의 읽기다: 관대하게 읽고, 표가
    /// 아니면 까닭 한 줄로 댄다. 인라인 표(`read = { … }`)도 읽는다.
    #[test]
    fn the_old_read_table_is_still_read_leniently() {
        let d = scratch("read-marks");
        let path = d.join("config.toml");
        let src = "[read]\n# 남이 적어 둔 것\n\"m-0001\" = \"2026-09-01T00:00:00Z\"  # 뒤 주석\nm-0002 = \"2026-09-02T00:00:00Z\"\n";
        std::fs::write(&path, src).unwrap();
        assert_eq!(
            read(Some(&path)).read,
            [
                ("m-0001".to_string(), "2026-09-01T00:00:00Z".to_string()),
                ("m-0002".to_string(), "2026-09-02T00:00:00Z".to_string()),
            ]
            .into()
        );

        let odd = Doc::parse("read = 3\n").unwrap();
        assert_eq!(odd.read_marks().1.len(), 1, "표가 아닌 것을 까닭 없이 지나쳤다");
        assert!(odd.read_marks().0.is_empty());

        let inline = Doc::parse("read = { \"m-0001\" = \"T\" }\n").unwrap();
        assert_eq!(inline.read_marks().0.len(), 1);

        // 때가 아닌 값은 그 줄만 건너뛰고 까닭을 댄다.
        let bad = Doc::parse("[read]\nm-0001 = 3\n").unwrap();
        assert!(bad.read_marks().0.is_empty() && bad.read_marks().1.len() == 1);
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
        assert_eq!(update(&path, |doc| doc.remove(&["/없음".into()])).unwrap(), 0);
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
        assert_eq!(update(&path, |doc| doc.remove(&["/x".into(), "/a".into()])).unwrap(), 2);
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

    /// **링크가 위 디렉터리에 걸렸으면 락은 하나다**(moai-2l74). 그때 두 철자의 락은 같은 파일이라, 둘 다
    /// 잡으면 제가 쥔 락을 제가 기다리다 `locked` 로 물러난다 — `~/.config` 가 dotfiles 로 걸렸거나 macOS 의
    /// `/var` 밑이면 쓰기마다 그랬다. 파일이 없던 첫 쓰기와 파일이 선 뒤의 쓰기가 다른 길이라 둘 다 잰다.
    /// 기다린 것은 `unwrap` 이 잡는다 — 락은 기다리다 `locked` 로 물러나지 늦게 잡히지 않는다(`store::Lock`).
    #[cfg(unix)]
    #[test]
    fn a_config_under_a_linked_directory_locks_once() {
        let s = scratch("linked-dir");
        let d = s.canonicalize().unwrap();
        std::fs::create_dir_all(d.join("dots")).unwrap();
        std::os::unix::fs::symlink(d.join("dots"), d.join("cfg")).unwrap();
        let path = d.join("cfg/config.toml");
        assert!(update(&path, |doc| doc.add(Path::new("/a"))).unwrap(), "첫 쓰기");
        assert!(update(&path, |doc| doc.add(Path::new("/b"))).unwrap(), "파일이 선 뒤의 쓰기");
        assert_eq!(read(Some(&path)).projects.len(), 2);
    }

    /// **락 파일 자체가 링크여도 락은 하나다**(moai-2l74 에픽 리뷰). stow·rcm 은 dotfiles 안의 파일을 하나씩 거는데,
    /// 설정 곁의 락도 dotfiles 안에 서니(`a_symlinked_config_stays_a_symlink`) 그것까지 걸린다. 철자로 견주면 둘이
    /// 달라 보여 한 파일을 두 번 잡고 쓰기마다 `locked` 로 물러났다 — 파일로 견준다. 하드 링크도 같다.
    #[cfg(unix)]
    #[test]
    fn a_linked_lock_file_is_locked_once() {
        let s = scratch("linked-lock");
        let d = s.canonicalize().unwrap();
        std::fs::create_dir_all(d.join("dots")).unwrap();
        std::fs::create_dir_all(d.join("cfg")).unwrap();
        std::fs::write(d.join("dots/config.toml"), "").unwrap();
        std::fs::write(d.join("dots/config.toml.lock"), "").unwrap();
        std::os::unix::fs::symlink(d.join("dots/config.toml"), d.join("cfg/config.toml")).unwrap();
        std::os::unix::fs::symlink(d.join("dots/config.toml.lock"), d.join("cfg/config.toml.lock")).unwrap();
        let path = d.join("cfg/config.toml");
        assert!(update(&path, |doc| doc.add(Path::new("/a"))).unwrap(), "락 파일이 링크");
        std::fs::remove_file(d.join("cfg/config.toml.lock")).unwrap();
        std::fs::hard_link(d.join("dots/config.toml.lock"), d.join("cfg/config.toml.lock")).unwrap();
        assert!(update(&path, |doc| doc.add(Path::new("/b"))).unwrap(), "락 파일이 하드 링크");
        assert_eq!(read(Some(&d.join("dots/config.toml"))).projects.len(), 2);
        assert!(std::fs::symlink_metadata(&path).unwrap().file_type().is_symlink());
    }

    /// **가리키는 파일이 아직 없는 링크도 링크로 남는다**(moai-2l74 에픽 리뷰). 링크를 먼저 걸고 파일은 첫 쓰기에
    /// 생기는 dotfiles 에서, 풀리지 않는 링크를 준 철자로 두면 첫 쓰기가 링크 자리에 `rename` 해 링크를 보통 파일로
    /// 갈아끼웠다 — dotfiles 쪽은 영영 비고, 가리키는 철자로 쓰는 쪽과는 다른 락을 잡는다. 가리키는 자리의
    /// 디렉터리가 없으면 쓰지 않는다 — 아직 안 받은 저장소 자리에 디렉터리를 짓지 않는다.
    #[cfg(unix)]
    #[test]
    fn a_link_to_a_missing_config_stays_a_link() {
        let s = scratch("dangling");
        let d = s.canonicalize().unwrap();
        std::fs::create_dir_all(d.join("dots")).unwrap();
        std::fs::create_dir_all(d.join("cfg")).unwrap();
        let link = d.join("cfg/config.toml");
        std::os::unix::fs::symlink("../dots/config.toml", &link).unwrap();
        assert!(update(&link, |doc| doc.add(Path::new("/a"))).unwrap());
        assert!(std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink(), "링크를 보통 파일로 갈아끼웠다");
        assert_eq!(read(Some(&d.join("dots/config.toml"))).projects, [Project { path: "/a".into(), hue: None }]);
        assert!(d.join("dots/config.toml.lock").exists(), "가리키는 자리의 락이 없다 — 두 철자가 서로를 안 막는다");

        let gone = d.join("cfg/gone.toml");
        std::os::unix::fs::symlink(d.join("nowhere/config.toml"), &gone).unwrap();
        let e = update(&gone, |doc| doc.add(Path::new("/a"))).unwrap_err();
        assert_eq!(e.code, code::BROKEN, "{e}");
        assert!(std::fs::symlink_metadata(&gone).unwrap().file_type().is_symlink());
        assert!(!d.join("nowhere").exists(), "없는 자리에 디렉터리를 지었다");
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
