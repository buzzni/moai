//! 요청의 `project` 를 걸어 둔 프로젝트의 자리로 푼다(moai-5n6m.q16).
//!
//! 데몬은 사용자당 하나이고 어느 프로젝트든 읽을 수 있다 — 그래서 요청이 대는 자리를 그대로 믿지 않는다. **걸어 둔
//! (`moai project add`) 프로젝트만 읽는다.** 웹 서버의 `project=` 와 같은 결정이고(`web::project_of`), 같은 견주기
//! ([`crate::user_config::same_dir`])와 같은 코드·글을 쓴다.

use super::rpc::{RequestEnv, RpcError};
use crate::fail::code;
use std::path::PathBuf;

/// 요청의 화면 말. 안 주면 CLI 와 같이 `MOAI_LANG`, 그다음 사용자 설정이다. **모르는 값은 `-32602`** — 말없이 영어로
/// 떨어지면 오타가 안 보인다.
pub fn lang(env: &RequestEnv, reg: &crate::user_config::Registry) -> Result<crate::i18n::Lang, RpcError> {
    match env.lang.as_deref() {
        Some(raw) => crate::i18n::Lang::parse(raw)
            .ok_or_else(|| RpcError::invalid_params(format!("unknown lang {raw:?} (en, ko, zh, ja, es)"))),
        None => Ok(crate::cmd::lang_of(reg)),
    }
}

/// 요청의 시간대. 안 주면 `None` — 읽는 입구가 CLI 와 같이 푼다. 모르는 값은 `-32602`.
pub fn zone(env: &RequestEnv) -> Result<Option<crate::tz::Zone>, RpcError> {
    env.tz
        .as_deref()
        .map(|name| {
            crate::tz::Zone::load(name).map_err(|_| {
                RpcError::invalid_params(format!("unknown tz {name:?} (a tz database name or POSIX rule)"))
            })
        })
        .transpose()
}

/// `project` 가 가리키는 걸어 둔 프로젝트의 디렉터리.
///
/// - 없다 → `-32602`
/// - 걸어 둔 목록에 없다(상대 경로 포함) → `-32000`, `data.code` 는 `not_registered`
/// - 걸어 두었는데 제 `.moai` 가 없다 → `-32000`, `not_a_project` (위로 찾아 부모의 트래커를 읽지 않는다)
///
/// 걸어 둔 항목에는 이름이 없다(`project ls` 의 이름은 경로에서 파생한 값이라 겹칠 수 있다) — 그래서 **경로로만**
/// 가린다. 설정은 요청마다 다시 읽는다 — 데몬이 도는 동안 `moai project add` 한 것이 바로 선다.
///
/// **돌려주는 자리는 링크를 푼 것이다**(moai-3hgm.k7s) — CLI 의 `-C <자리>` 는 그리로 옮겨 선 뒤 커널이 주는 물리
/// 경로(`current_dir`)에서 트래커를 찾으므로, 말에 대는 뿌리가 그 철자다. `project add` 는 푼 경로를 적지만 손으로
/// 고친 설정에는 링크 철자가 설 수 있다 — 그대로 넘기면 같은 요청의 말이 CLI 와 다른 자리를 댄다.
pub fn resolve(
    env: &RequestEnv,
    lang: crate::i18n::Lang,
    reg: &crate::user_config::Registry,
) -> Result<PathBuf, RpcError> {
    use crate::i18n::{fill, say};
    let value = env.project.as_deref().ok_or_else(|| RpcError::invalid_params("params.project is required"))?;
    let asked = std::path::Path::new(value);
    let found = match asked.is_absolute() {
        true => reg.projects.iter().map(|p| &p.path).find(|p| crate::user_config::same_dir(p, asked)),
        false => None,
    };
    let Some(dir) = found else {
        return Err(RpcError::moai(code::NOT_REGISTERED, fill(say(lang, "web.not_registered"), &[("path", value)])));
    };
    match crate::store::spot(&crate::store::Repo::opened_root(dir)) {
        crate::store::Spot::Absent => {
            Err(RpcError::moai(code::NOT_A_PROJECT, fill(say(lang, "web.not_a_project"), &[("path", value)])))
        }
        _ => Ok(crate::path::canonical(dir).unwrap_or_else(|_| dir.clone())),
    }
}

/// 설정을 읽는다 — 요청마다. 깨진 파일은 실패하지 않고 빈 목록이 된다(`user_config::read`).
pub fn registry() -> crate::user_config::Registry {
    crate::user_config::read(crate::user_config::path().as_deref())
}
