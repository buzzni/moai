//! 읽기 메서드 여섯(moai-5n6m.q16) — `status`·`ready`·`prime`·`show`·`stats`·`project.ls` — 과 `changes`(moai-5n6m.cb9).
//!
//! 답은 `moai -C <root> --json <명령>` 과 같다(`cmd::request::read` 가 같은 명령 함수를 돌린다). 이 층이 하는 일은
//! 둘이다 — 요청의 **이름 붙은 필드**를 CLI 가 받는 값(`cli::ShowArgs` 등)으로 바꾸고, 돌아온 줄을 JSON 으로 싼다.
//!
//! **argv 를 받지 않는다.** 필드는 CLI 의 플래그를 따라 이름이 붙고 형이 있다(`tree: bool`, `limit: int`,
//! `status: string|[string]`). 값은 clap 을 거치지 않고 구조체에 직접 담는다 — 값을 읽는 규칙(종류·`--sort`·열거형)은
//! CLI 와 같은 함수(`FromStr`·`cli::sort_fields`·`ValueEnum`)를, 범위(`limit`≥1, `last` 1~1000)는 같은 수를 쓴다.
//! 거르개 값 자체(`status=todo` 가 칸 이름인가)는 CLI 도 clap 이 아니라 명령이 보니 같은 자리에서 걸린다.
//!
//! **모르는 키는 `-32602` 이고 키 이름을 댄다** — 오타 난 거르개가 말없이 먹히면 틀린 목록이 맞는 것처럼 돌아온다.
//! `project`·`actor`·`lang`·`tz` 는 모든 메서드가 아는 키다.
//!
//!
//! **결과의 꼴** — 모든 메서드가 같은 봉투다.
//!
//! ```json
//! { "value": <CLI 의 --json stdout>, "messages": ["…"], "partial": false }
//! ```
//!
//! `messages` 는 CLI 가 stderr 에 냈을 줄(색 없이), `partial` 은 CLI 라면 1 로 끝났을 덜 받은 답이다. 봉투를 쓰는
//! 까닭: 결과를 CLI 값 그대로 두면 두 곁길을 실을 자리가 없다. `value` 는 CLI 의 stdout 이 한 줄이면 그 값, 아니면
//! 줄마다 파싱한 배열이다(여섯 명령은 `--json` 에서 한 줄이다). CLI 가 넘어졌으면 `-32000` 이고 `data.code` 는 CLI 의
//! `code`, `data.messages` 는 그 곁의 stderr 줄이다.

//! | 메서드 | 필드(전부 선택, `project` 만 필수) |
//! |---|---|
//! | `status`·`prime` | `worktree: bool` |
//! | `ready` | `worktree: bool`, `limit: int>=1` |
//! | `show` | `id: string`(이슈 id 또는 종류), `tree`·`removed`·`worktree`·`reverse: bool`, `fields: string\|[string]`, `sort: string`, `limit: int>=1`, `after: string`, 거르개 |
//! | `stats` | `by: string\|[string]`(축), `bucket: "day"\|"week"`, `last: int 1..=1000`, 거르개 |
//! | `project.ls` | 없음 (`project` 도 안 쓴다) |
//! | `changes` | `cursor: string`(`value` 는 `{cursor, changed, removed, reset}`, 견주는 목록은 `show` 의 `all: true`) |
//!
//! 거르개: `status`·`tag`·`no_tag`·`epic`·`milestone`·`parent`·`priority`·`assignee`·`since`·`created`·`done`·`filter`
//! 는 `string|number|[…]`(쉼표는 "또는", 배열은 "그리고", 담당만 배열도 "또는"), `type`·`grep: string`, `stale: int`,
//! `deferred`·`all`·`archived: bool`. `show --raw`·`--as-plan` 은 글을 내는 플래그라 없다. 모든 메서드가 `project`
//! (걸어 둔 프로젝트의 경로)·`actor`·`lang`·`tz` 를 받는다. 오류: `-32602`(필드·말·시간대), `-32000`(`data.code`:
//! `not_registered`·`not_a_project`·CLI 의 코드).

use super::project;
use super::rpc::{self, Methods, Reply, RequestEnv, RpcError};
use crate::cli::{AxisArg, BucketArg, FilterArgs, PageArgs, ShowArgs, StatsArgs, WorktreeArg};
use crate::cmd::request::{self, Read, Request};
use clap::ValueEnum as _;
use serde::Serialize;
use serde_json::{Map, Value, json};

pub fn register(methods: &mut Methods) {
    methods.register("status", |env, p| {
        serve(env, "status", p, true, |p| Ok(Read::Status { worktree: p.flag("worktree")? }))
    });
    methods.register("ready", |env, p| {
        serve(env, "ready", p, true, |p| {
            Ok(Read::Ready { worktree: p.flag("worktree")?, limit: p.count("limit", 1, i64::MAX)? })
        })
    });
    methods.register("prime", |env, p| {
        serve(env, "prime", p, true, |p| Ok(Read::Prime { worktree: p.flag("worktree")? }))
    });
    methods.register("show", |env, p| serve(env, "show", p, true, show));
    methods.register("stats", |env, p| serve(env, "stats", p, true, stats));
    methods.register("project.ls", |env, p| serve(env, "project.ls", p, false, |_| Ok(Read::ProjectLs)));
    // 기억은 데몬 하나에 하나다 — 연결 스레드가 모두 이 복제로 같은 것을 나눠 쓴다(`changes::Memory`).
    let memory = super::changes::Memory::new();
    methods.register("changes", move |env, p| changes(env, p, &memory));
}

/// `changes` — 데몬이 기억한 옛 스냅샷 뒤로 바뀐 id(moai-5n6m.cb9, moai-8ts9). 같은 봉투에 싼다: 클라이언트가 어느
/// 메서드든 `result.value` 를 벗겨 읽게 두려는 것이다. 견주는 목록은 [`whole`] — 클라이언트의 "전부 읽기" 와 같은
/// 요청을 `serve` 와 같은 사람·말·시간대·설정으로 돌린다. 읽는 규칙(커서·`reset`·기억)은 `changes` 모듈에 있다.
fn changes(env: &RequestEnv, params: Value, memory: &super::changes::Memory) -> Result<Reply, RpcError> {
    let mut p = Params::new("changes", params)?;
    let cursor = p.string("cursor")?;
    p.done()?;
    let reg = project::registry();
    let lang = project::lang(env, &reg)?;
    let zone = project::zone(env)?;
    let root = project::resolve(env, lang, &reg)?;
    let lang_asked = env.lang.is_some().then_some(lang);
    let whole = Request { root, read: whole()?, actor: env.actor.clone(), lang: lang_asked, zone, reg: Some(reg) };
    super::changes::handle(memory, whole, lang, cursor.as_deref())
}

/// "전부 읽기" — `show` 메서드에 `all: true` 하나만 준 것과 같은 읽기다. `changes` 가 견주는 목록이고, `reset` 을
/// 받은 클라이언트가 다시 읽는 목록이다. 같은 빌더(`show`)로 지어 두 길이 갈릴 자리를 없앤다.
pub(super) fn whole() -> Result<Read, RpcError> {
    let mut p = Params::new("show", json!({ "all": true }))?;
    let read = show(&mut p)?;
    p.done()?;
    Ok(read)
}

/// 한 메서드의 한 요청. 차례: 필드 읽기(`-32602`) → 말·시간대(`-32602`) → 프로젝트(`-32602`·`-32000`) → 돌리기.
fn serve(
    env: &RequestEnv,
    method: &str,
    params: Value,
    needs_project: bool,
    build: impl FnOnce(&mut Params) -> Result<Read, RpcError>,
) -> Result<Reply, RpcError> {
    let mut p = Params::new(method, params)?;
    let read = build(&mut p)?;
    p.done()?;
    let reg = project::registry();
    let lang = project::lang(env, &reg)?;
    let zone = project::zone(env)?;
    let root = match needs_project {
        true => project::resolve(env, lang, &reg)?,
        // `project.ls` 는 자리를 안 쓴다 — 설정을 읽을 뿐이다. 그래도 빈 자리는 두지 않는다: `side::cwd()` 가 `""` 를 내면
        // 그것을 잇거나 올라가는 길이 데몬이 선 자리에 기대 상대 경로를 댄다. CLI 의 `moai project ls` 가 데몬이 선
        // 자리에서 친 것과 같게 그 자리를 절대 경로로 준다.
        false => std::env::current_dir().unwrap_or_default(),
    };
    let lang_asked = env.lang.is_some().then_some(lang);
    // 푼 설정을 그대로 넘긴다 — 명령이 `Ctx::registry` 로 같은 파일을 다시 파지 않게.
    let answer =
        request::read(Request { root, read, actor: env.actor.clone(), lang: lang_asked, zone, reg: Some(reg) });
    match answer.out {
        Ok(lines) => {
            // 줄마다 JSON 인지만 보고 바이트는 그대로 싣는다 — CLI `--json` 의 키 차례가 답에도 선다(moai-3hgm.whp).
            let mut values = Vec::with_capacity(lines.len());
            for line in &lines {
                values.push(rpc::raw_line(line).ok_or_else(|| {
                    RpcError::new(rpc::code::INTERNAL, format!("{method} printed a line that is not JSON"))
                })?);
            }
            let value = if values.len() == 1 { values.remove(0) } else { rpc::reply(&values)? };
            envelope(&value, &answer.messages, answer.partial)
        }
        Err(fail) => {
            let mut error = RpcError::moai(fail.code, fail.message);
            if !answer.messages.is_empty() {
                error.data = Some(json!({ "code": fail.code, "messages": answer.messages }));
            }
            Err(error)
        }
    }
}

/// 모든 메서드가 쓰는 봉투. `value` 는 이미 쓰인 글이든 직렬화할 값이든 그 글이 그대로 선다.
pub(super) fn envelope(value: &impl Serialize, messages: &[String], partial: bool) -> Result<Reply, RpcError> {
    #[derive(Serialize)]
    struct Envelope<'a, V: Serialize + ?Sized> {
        messages: &'a [String],
        partial: bool,
        value: &'a V,
    }
    rpc::reply(&Envelope { messages, partial, value })
}

fn show(p: &mut Params) -> Result<Read, RpcError> {
    let page = PageArgs {
        sort: match p.string("sort")? {
            Some(raw) => Some(crate::cli::sort_fields(&raw).map_err(|e| p.bad("sort", e))?),
            None => None,
        },
        reverse: p.flag("reverse")?,
        limit: p.count("limit", 1, i64::MAX)?,
        after: p.string("after")?,
    };
    Ok(Read::Show(Box::new(ShowArgs {
        target: p.string("id")?,
        raw: false,
        tree: p.flag("tree")?,
        as_plan: false,
        removed: p.flag("removed")?,
        fields: p.strings("fields")?.into_iter().flat_map(|s| split(&s)).collect(),
        worktree: WorktreeArg { worktree: p.flag("worktree")? },
        filter: filter(p)?,
        page,
    })))
}

fn stats(p: &mut Params) -> Result<Read, RpcError> {
    let mut by = Vec::new();
    for word in p.strings("by")?.iter().flat_map(|s| split(s)) {
        // 글자 크기를 가린다 — clap 의 `value_enum` 이 그렇다(`--by STATUS` 는 CLI 에서 거절이다).
        by.push(AxisArg::from_str(&word, false).map_err(|_| p.bad("by", format!("unknown axis {word:?}")))?);
    }
    let bucket = match p.string("bucket")? {
        Some(word) => Some(BucketArg::from_str(&word, false).map_err(|_| p.bad("bucket", "use \"day\" or \"week\""))?),
        None => None,
    };
    Ok(Read::Stats(Box::new(StatsArgs { by, bucket, last: p.count("last", 1, 1000)?, filter: filter(p)? })))
}

/// 거르개 — 쉼표는 "또는", 배열(반복한 플래그)은 "그리고". 담당만 배열도 "또는"이다. 규칙은 명령이 읽는다.
fn filter(p: &mut Params) -> Result<FilterArgs, RpcError> {
    Ok(FilterArgs {
        status: p.strings("status")?,
        tag: p.strings("tag")?,
        no_tag: p.strings("no_tag")?,
        epic: p.strings("epic")?,
        milestone: p.strings("milestone")?,
        parent: p.strings("parent")?,
        priority: p.strings("priority")?,
        assignee: p.strings("assignee")?,
        kind: match p.string("type")? {
            Some(raw) => Some(raw.parse().map_err(|e: String| p.bad("type", e))?),
            None => None,
        },
        grep: p.string("grep")?,
        stale: p.int("stale")?,
        since: p.strings("since")?,
        created: p.strings("created")?,
        done: p.strings("done")?,
        deferred: p.flag("deferred")?,
        all: p.flag("all")?,
        archived: p.flag("archived")?,
        filter: p.strings("filter")?,
    })
}

fn split(s: &str) -> Vec<String> {
    s.split(',').map(str::to_string).collect()
}

/// 한 요청의 필드. 읽은 키는 지우고, 남은 키가 모르는 키다.
struct Params {
    method: String,
    map: Map<String, Value>,
}

impl Params {
    fn new(method: &str, params: Value) -> Result<Params, RpcError> {
        let mut map = match params {
            Value::Null => Map::new(),
            Value::Object(map) => map,
            _ => return Err(RpcError::invalid_params(format!("{method}: params must be an object"))),
        };
        // 모든 메서드가 아는 키 — 값은 `RequestEnv` 가 읽는다. **글이 아니면 거절한다** — `RequestEnv` 는 글만 읽어,
        // `actor: {name, email}` 이 말없이 빠지면 그 요청은 데몬의 `MOAI_ACTOR`·git 의 사람으로 내 것과 남의 것을 가른다.
        for key in ["project", "actor", "lang", "tz"] {
            if let Some(v) = map.remove(key)
                && !matches!(v, Value::String(_) | Value::Null)
            {
                return Err(RpcError::invalid_params(format!("{method}: params.{key}: must be a string")));
            }
        }
        Ok(Params { method: method.to_string(), map })
    }

    fn bad(&self, key: &str, why: impl std::fmt::Display) -> RpcError {
        RpcError::invalid_params(format!("{}: params.{key}: {why}", self.method))
    }

    /// 남은 키가 있으면 거절한다 — 가장 앞의 이름을 댄다.
    fn done(self) -> Result<(), RpcError> {
        match self.map.keys().next() {
            Some(key) => Err(RpcError::invalid_params(format!("{}: unknown param {key:?}", self.method))),
            None => Ok(()),
        }
    }

    fn flag(&mut self, key: &str) -> Result<bool, RpcError> {
        match self.map.remove(key) {
            None | Some(Value::Null) => Ok(false),
            Some(Value::Bool(b)) => Ok(b),
            Some(_) => Err(self.bad(key, "must be a boolean")),
        }
    }

    fn string(&mut self, key: &str) -> Result<Option<String>, RpcError> {
        match self.map.remove(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(s)) => Ok(Some(s)),
            Some(_) => Err(self.bad(key, "must be a string")),
        }
    }

    /// 낱말 하나 또는 낱말의 배열. 수는 글로 받는다(`priority: 1`).
    fn strings(&mut self, key: &str) -> Result<Vec<String>, RpcError> {
        let word = |v: &Value| match v {
            Value::String(s) => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        };
        match self.map.remove(key) {
            None | Some(Value::Null) => Ok(Vec::new()),
            Some(Value::Array(items)) => items
                .iter()
                .map(|v| word(v).ok_or_else(|| self.bad(key, "must be a string, a number or an array of them")))
                .collect(),
            Some(v) => {
                word(&v).map(|w| vec![w]).ok_or_else(|| self.bad(key, "must be a string, a number or an array of them"))
            }
        }
    }

    fn int(&mut self, key: &str) -> Result<Option<i64>, RpcError> {
        match self.map.remove(key) {
            None | Some(Value::Null) => Ok(None),
            Some(v) => v.as_i64().map(Some).ok_or_else(|| self.bad(key, "must be an integer")),
        }
    }

    fn count(&mut self, key: &str, min: i64, max: i64) -> Result<Option<usize>, RpcError> {
        match self.int(key)? {
            None => Ok(None),
            Some(n) if (min..=max).contains(&n) => Ok(Some(n as usize)),
            Some(n) if max == i64::MAX => Err(self.bad(key, format!("{n} is out of range (at least {min})"))),
            Some(n) => Err(self.bad(key, format!("{n} is out of range ({min} to {max})"))),
        }
    }
}
