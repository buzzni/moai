//! 쓰기 메서드 일곱(moai-3hgm.k7s) — `add`·`mv`·`edit`·`note`·`defer`·`rm`·`link`.
//!
//! 답은 `moai -C <root> --json <명령>` 과 같다 — [`request::write`] 가 같은 명령 함수를 같은 쓰기 경로
//! (`store::with_write`)로 돌린다. 데몬에 따로 선 쓰기 길은 없다. 이 층이 하는 일은 읽기(`daemon::read`)와 같은 둘이다
//! — 요청의 **이름 붙은 필드**를 CLI 의 인자 구조체로 바꾸고, 돌아온 줄을 같은 봉투(`{exit, messages, partial, value}`)에
//! 싼다.
//!
//! - **argv 를 받지 않는다.** 필드는 CLI 의 긴 깃발 이름을 따르고 `-` 는 `_` 로 쓴다(`--from` → `from`, `--if-start` →
//!   `if_start`, `--match` → `match`). 자리 인자는 뜻으로 이름 짓는다 — 대상은 `id`, `mv` 가 갈 칸은 `to`(`from` 과
//!   짝이다), `note` 의 글은 `text`. 여럿을 받는 자리(`mv`·`defer`·`rm` 의 `id`, 태그, 막는 줄)는 `string|[string]`
//!   이다(`daemon::read` 의 거르개와 같은 꼴). CLI 가 쉼표로 가르는 깃발(`tag`·`untag`·`blocks`·`unblocks`)은 여기서도
//!   쉼표로 가른다
//! - **clap 을 안 지나므로 clap 이 하던 거절을 여기서 한다** — 꼭 줄 것(`id`·`to`), 함께 못 줄 것(`edit` 의 `shift*`,
//!   `rm` 의 `id`·`line`), 다른 것을 요구하는 것(`edit` 의 `if_*`), 값의 꼴(`if_start` 의 날, `shift` 의 폭, `priority`
//!   의 u8). 모두 `-32602` 이고 키 이름을 댄다. 값을 읽는 자는 CLI 의 것이다(`Kind::from_str`·`cli::expected_day`·
//!   `cli::SHIFT_DAYS`). clap 을 지난 뒤 **명령이** 하는 거절(없는 칸·우선순위 4 이상·빈 제목·없는 id)은 명령이
//!   그대로 한다 — `-32000` 에 CLI 의 `code` 다
//! - **글 자리는 글이다** — `body`·`msg`·`text` 의 `"-"` 도 그 글이다. 요청에는 stdin 이 없다(moai-3hgm.25e)
//! - **계획은 안 낸다** — `add` 의 `from`·`dry_run`·`var` 는 모르는 키다(`add --from`·`backlog promote` 는 다음 일).
//!   `quiet` 도 없다: `--json` 에서는 아무것도 안 바꾸는 깃발이라, 받으면 거짓말이다. `note` 의 `-b` 는 `text` 하나로
//!   든다 — 요청의 글은 둘 다 그냥 글이라 둘을 두면 같은 것에 이름이 둘이다
//! - **사람**은 요청의 `actor` → 그 프로젝트의 git 설정 → `no_actor`(`-32000`)다. 데몬의 `MOAI_ACTOR` 는 안 읽는다
//!   (`model::actor`). **남의 줄·담당 없는 줄을 시작 칸으로 옮겨도 막지 않는다** — CLI 가 stderr 에 내는 한 줄이
//!   `messages` 에 서고, `take: true` 가 `--take` 다. 훅 규칙 5 는 이 API 에 서지 않는다(사람의 결정, moai-3hgm)
//! - **멈추는 데몬은 돌던 쓰기를 끝까지 기다린다** — 일곱 다 [`Methods::register_write`] 로 단다(moai-3hgm.uz0)
//!
//! | 메서드 | 필드(`project` 필수) | CLI |
//! |---|---|---|
//! | `add` | `kind: "issue"\|"epic"\|"milestone"\|"backlog"`, `title`, `body`, `epic`, `milestone`, `parent`, `status`, `assignee`, `start`, `due: string`, `tag: string\|[string]`, `priority: int` | `moai [<kind>] add` |
//! | `mv` | `id: string\|[string]`·`to: string`(둘 다 필수), `msg`, `from: string`, `take: bool` | `moai mv <id>… <to>` |
//! | `edit` | `id`(필수), `title`, `body`, `epic`, `milestone`, `assignee`, `start`, `due`, `if_start`, `if_due: string`, `tag`·`untag: string\|[string]`, `priority`·`shift`·`shift_start`·`shift_due: int`, `if_unplanned: bool` | `moai edit` |
//! | `note` | `id`(필수), `text: string` | `moai note <id> <text>` |
//! | `defer` | `id: string\|[string]`(필수), `undo: bool`, `msg`, `from: string` | `moai defer` |
//! | `rm` | `id: string\|[string]` 또는 `line: int>=0`, `yes: bool`, `match: string` | `moai rm` |
//! | `link` | `id`(필수), `blocks`·`unblocks: string\|[string]` | `moai link` |

use super::read::{self, Asked, Params};
use super::rpc::{Methods, Reply, RequestEnv, RpcError};
use crate::cli::{AddArgs, DeferArgs, EditArgs, LinkArgs, MvArgs, NoteArgs, RmArgs};
use crate::cmd::request::{self, Write, WriteRequest};
use crate::model::Kind;
use serde_json::Value;

pub fn register(methods: &mut Methods) {
    methods.register_write("add", |env, p| serve(env, "add", p, add));
    methods.register_write("mv", |env, p| serve(env, "mv", p, mv));
    methods.register_write("edit", |env, p| serve(env, "edit", p, edit));
    methods.register_write("note", |env, p| serve(env, "note", p, note));
    methods.register_write("defer", |env, p| serve(env, "defer", p, defer));
    methods.register_write("rm", |env, p| serve(env, "rm", p, rm));
    methods.register_write("link", |env, p| serve(env, "link", p, link));
}

/// 한 메서드의 한 요청 — 읽기의 `serve` 와 같은 차례다: 필드(`-32602`) → 말·시간대·프로젝트([`Asked`]) → 돌리기 →
/// 봉투([`read::respond`]). 필드를 프로젝트보다 먼저 읽는다 — 틀린 요청은 설정 파일도 트래커도 안 연다.
fn serve(
    env: &RequestEnv,
    method: &str,
    params: Value,
    build: impl FnOnce(&mut Params) -> Result<Write, RpcError>,
) -> Result<Reply, RpcError> {
    let mut p = Params::new(method, params)?;
    let write = build(&mut p)?;
    p.done()?;
    let asked = Asked::of(env, true)?;
    let answer = request::write(WriteRequest {
        root: asked.root,
        write,
        actor: env.actor.clone(),
        lang: asked.lang_asked,
        zone: asked.zone,
        reg: Some(asked.reg),
    });
    read::respond(method, answer)
}

/// `moai add`·`moai <kind> add`. `kind` 는 종류를 박은 동사의 자리다(`cmd::typed` 가 넘기는 `add::run` 의 셋째 인자) —
/// 하나짜리 `add` 에서는 `--type` 과 같은 줄을 짓고, 둘이 갈리는 길(계획)은 안 낸다.
fn add(p: &mut Params) -> Result<Write, RpcError> {
    let kind = match p.string("kind")? {
        Some(raw) => Some(raw.parse::<Kind>().map_err(|e| p.bad("kind", e))?),
        None => None,
    };
    let args = AddArgs {
        title: p.string("title")?,
        epic: p.string("epic")?,
        milestone: p.string("milestone")?,
        tag: list(p, "tag")?,
        priority: priority(p)?,
        status: p.string("status")?,
        body: p.string("body")?,
        assignee: p.string("assignee")?,
        start: p.string("start")?,
        due: p.string("due")?,
        kind: None,
        parent: p.string("parent")?,
        from: None,
        dry_run: false,
        var: Vec::new(),
        quiet: false,
    };
    Ok(Write::Add { args: Box::new(args), kind })
}

/// `moai mv <id>… <to>` — 갈 칸을 따로 받는다. 한 줄에 이어 받으면 `to` 를 빠뜨린 요청의 마지막 id 가 칸으로 읽힌다.
fn mv(p: &mut Params) -> Result<Write, RpcError> {
    let mut args = ids(p, "id")?;
    args.push(needed(p, "to")?);
    Ok(Write::Mv(Box::new(MvArgs { args, msg: p.string("msg")?, from: p.string("from")?, take: p.flag("take")? })))
}

fn edit(p: &mut Params) -> Result<Write, RpcError> {
    let args = EditArgs {
        id: needed(p, "id")?,
        title: p.string("title")?,
        body: p.string("body")?,
        tag: list(p, "tag")?,
        untag: list(p, "untag")?,
        epic: p.string("epic")?,
        milestone: p.string("milestone")?,
        priority: priority(p)?,
        assignee: p.string("assignee")?,
        start: p.string("start")?,
        due: p.string("due")?,
        if_start: expected_day(p, "if_start")?,
        if_due: expected_day(p, "if_due")?,
        shift: shift(p, "shift")?,
        shift_start: shift(p, "shift_start")?,
        shift_due: shift(p, "shift_due")?,
        if_unplanned: p.flag("if_unplanned")?,
    };
    edit_rules(p, &args)?;
    Ok(Write::Edit(Box::new(args)))
}

/// `EditArgs` 에 clap 이 건 짝 규칙 — 날을 옮기는 셋(`shift*`)은 날을 주는 둘과도 서로도 함께 못 서고, `if_*` 는 날을
/// 줄 때만 뜻이 있다. **`cli::EditArgs` 의 `conflicts_with_all`·`requires = "dates"` 를 그대로 옮겼다** — 그쪽을 고치면
/// 여기도 고친다(`edit_refuses_what_clap_refuses` 가 두 쪽을 한 표로 잰다).
fn edit_rules(p: &Params, a: &EditArgs) -> Result<(), RpcError> {
    let given = [
        ("start", a.start.is_some()),
        ("due", a.due.is_some()),
        ("shift", a.shift.is_some()),
        ("shift_start", a.shift_start.is_some()),
        ("shift_due", a.shift_due.is_some()),
        ("if_start", a.if_start.is_some()),
        ("if_due", a.if_due.is_some()),
        ("if_unplanned", a.if_unplanned),
    ];
    let on = |key: &str| given.iter().any(|(k, v)| *k == key && *v);
    const APART: &[(&str, &[&str])] = &[
        ("shift", &["start", "due", "shift_start", "shift_due", "if_unplanned"]),
        ("shift_start", &["start", "due", "shift_due", "if_unplanned"]),
        ("shift_due", &["start", "due", "if_unplanned"]),
    ];
    for (key, others) in APART {
        if on(key)
            && let Some(other) = others.iter().find(|o| on(o))
        {
            return Err(p.bad(key, format!("cannot be used with params.{other}")));
        }
    }
    for key in ["if_start", "if_due", "if_unplanned"] {
        if on(key) && !(on("start") || on("due")) {
            return Err(p.bad(key, "needs params.start or params.due"));
        }
    }
    Ok(())
}

fn note(p: &mut Params) -> Result<Write, RpcError> {
    Ok(Write::Note(Box::new(NoteArgs { id: needed(p, "id")?, text: p.string("text")?, body: None })))
}

fn defer(p: &mut Params) -> Result<Write, RpcError> {
    Ok(Write::Defer(Box::new(DeferArgs {
        ids: ids(p, "id")?,
        undo: p.flag("undo")?,
        msg: p.string("msg")?,
        from: p.string("from")?,
    })))
}

/// `moai rm <id>…` 또는 `moai rm --line <n>` — clap 처럼 둘 가운데 하나만 받는다(`RmArgs` 의 `required_unless_present`·
/// `conflicts_with`).
fn rm(p: &mut Params) -> Result<Write, RpcError> {
    let ids = p.strings("id")?;
    let line = p.count("line", 0, i64::MAX)?;
    match (ids.is_empty(), line) {
        (true, None) => return Err(RpcError::invalid_params("rm: params.id or params.line is required")),
        (false, Some(_)) => return Err(p.bad("line", "cannot be used with params.id")),
        _ => {}
    }
    Ok(Write::Rm(Box::new(RmArgs { ids, line, yes: p.flag("yes")?, matches: p.string("match")? })))
}

fn link(p: &mut Params) -> Result<Write, RpcError> {
    Ok(Write::Link(Box::new(LinkArgs {
        id: needed(p, "id")?,
        blocks: list(p, "blocks")?,
        unblocks: list(p, "unblocks")?,
    })))
}

/// 꼭 줄 글 하나 — clap 의 자리 인자와 같다.
fn needed(p: &mut Params, key: &str) -> Result<String, RpcError> {
    p.string(key)?.ok_or_else(|| p.missing(key))
}

/// 하나 이상의 id — `mv`·`defer` 의 `required = true` 자리다.
fn ids(p: &mut Params, key: &str) -> Result<Vec<String>, RpcError> {
    let ids = p.strings(key)?;
    if ids.is_empty() {
        return Err(p.missing(key));
    }
    Ok(ids)
}

/// 쉼표로 가르는 목록 — CLI 의 `value_delimiter = ','` 와 같다. 배열은 깃발을 여러 번 준 것이다.
fn list(p: &mut Params, key: &str) -> Result<Vec<String>, RpcError> {
    Ok(p.strings(key)?.iter().flat_map(|s| read::split(s)).collect())
}

/// `-p` — clap 은 u8 로만 읽고, 0~3 은 쓰기가 잰다(`Issue::validate_fields`). 그래서 여기도 u8 밖만 거절하고 4~255 는
/// 명령에 넘긴다 — `-p 7` 과 같은 `code` 로 넘어진다.
fn priority(p: &mut Params) -> Result<Option<u8>, RpcError> {
    match p.int("priority")? {
        None => Ok(None),
        // 글은 이 자리가 실제로 거절하는 폭을 댄다 — 4~255 는 지나가 명령이 거절하므로 "0 to 3" 이라 대면 그 둘이 갈린다.
        Some(n) => u8::try_from(n).map(Some).map_err(|_| p.bad("priority", format!("{n} is out of range (0 to 255)"))),
    }
}

/// `--if-start`·`--if-due` — `YYYY-MM-DD` 나 `none`(`cli::expected_day`).
fn expected_day(p: &mut Params, key: &str) -> Result<Option<String>, RpcError> {
    match p.string(key)? {
        None => Ok(None),
        Some(raw) => crate::cli::expected_day(&raw).map(Some).map_err(|e| p.bad(key, e)),
    }
}

/// `--shift*` 의 날 수 — CLI 와 같은 폭(`cli::SHIFT_DAYS`).
fn shift(p: &mut Params, key: &str) -> Result<Option<i64>, RpcError> {
    let range = crate::cli::SHIFT_DAYS;
    match p.int(key)? {
        Some(n) if !range.contains(&n) => {
            Err(p.bad(key, format!("{n} is out of range ({} to {})", range.start(), range.end())))
        }
        n => Ok(n),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::rpc::code;
    use clap::Parser as _;
    use serde_json::json;

    /// 메서드 하나의 필드를 인자 구조체로 — 프로젝트는 안 푼다(그것은 `daemon` 통합 시험이 잰다).
    fn built(method: &str, params: Value) -> Result<Write, RpcError> {
        let build = match method {
            "add" => add,
            "mv" => mv,
            "edit" => edit,
            "note" => note,
            "defer" => defer,
            "rm" => rm,
            "link" => link,
            other => panic!("{other} 는 쓰기 메서드가 아니다"),
        };
        let mut p = Params::new(method, params)?;
        let write = build(&mut p)?;
        p.done()?;
        Ok(write)
    }

    /// 인자 구조체를 견줄 글로 — `Write` 는 `Debug` 가 없고, 담은 CLI 구조체는 있다.
    fn shape(write: &Write) -> String {
        match write {
            Write::Add { args, kind } => format!("add {kind:?} {args:?}"),
            Write::Mv(a) => format!("mv {a:?}"),
            Write::Edit(a) => format!("edit {a:?}"),
            Write::Note(a) => format!("note {a:?}"),
            Write::Defer(a) => format!("defer {a:?}"),
            Write::Rm(a) => format!("rm {a:?}"),
            Write::Link(a) => format!("link {a:?}"),
        }
    }

    /// CLI 가 같은 argv 에서 짓는 쓰기.
    fn parsed(argv: &[&str]) -> Write {
        let cli = crate::cli::Cli::try_parse_from(std::iter::once("moai").chain(argv.iter().copied()))
            .unwrap_or_else(|e| panic!("{argv:?}: {e}"));
        Write::of(cli.cmd.unwrap()).unwrap_or_else(|_| panic!("{argv:?} 는 쓰기 명령이 아니다"))
    }

    fn refused(method: &str, params: Value) -> RpcError {
        match built(method, params.clone()) {
            Ok(w) => panic!("{method} {params} 가 섰다: {}", shape(&w)),
            Err(e) => e,
        }
    }

    /// **필드가 CLI 의 그 깃발과 같은 인자 구조체를 짓는다** — 같은 argv 를 clap 으로 푼 것과 글자째 견준다. 이름을
    /// 잘못 이은 필드(`msg` 를 `body` 로)는 다른 구조체가 되어 붉다.
    #[test]
    fn each_field_builds_what_the_cli_flag_builds() {
        let cases: Vec<(&str, Value, Vec<&str>)> = vec![
            ("add", json!({ "title": "t" }), vec!["add", "t"]),
            (
                "add",
                json!({ "title": "t", "body": "b\n둘째", "epic": "x-e", "milestone": "x-m", "parent": "x-p",
                        "status": "review", "assignee": "none", "start": "2026-11-01", "due": "+3",
                        "tag": ["a,b", "c"], "priority": 1 }),
                vec![
                    "add",
                    "t",
                    "-b",
                    "b\n둘째",
                    "-e",
                    "x-e",
                    "--milestone",
                    "x-m",
                    "--parent",
                    "x-p",
                    "-s",
                    "review",
                    "-a",
                    "none",
                    "--start",
                    "2026-11-01",
                    "--due",
                    "+3",
                    "-t",
                    "a,b",
                    "-t",
                    "c",
                    "-p",
                    "1",
                ],
            ),
            ("add", json!({ "kind": "epic", "title": "e" }), vec!["epic", "add", "e"]),
            ("add", json!({ "kind": "milestone", "title": "v1" }), vec!["milestone", "add", "v1"]),
            ("add", json!({ "kind": "backlog", "title": "생각" }), vec!["backlog", "add", "생각"]),
            ("add", json!({ "kind": "issue", "title": "i" }), vec!["issue", "add", "i"]),
            // 글 자리의 `-` 는 글이다 — CLI 에서는 `--` 뒤에 줘야 제목이 된다.
            ("add", json!({ "title": "-x", "body": "-" }), vec!["add", "-b", "-", "--", "-x"]),
            ("add", json!({ "title": "t", "priority": 7 }), vec!["add", "t", "-p", "7"]),
            ("mv", json!({ "id": "x-a", "to": "done" }), vec!["mv", "x-a", "done"]),
            (
                "mv",
                json!({ "id": ["x-a", "x-b"], "to": "in_progress", "from": "todo", "take": true, "msg": "-" }),
                vec!["mv", "x-a", "x-b", "in_progress", "--from", "todo", "--take", "-m", "-"],
            ),
            ("edit", json!({ "id": "x-a", "title": "t" }), vec!["edit", "x-a", "--title", "t"]),
            (
                "edit",
                json!({ "id": "x-a", "body": "b", "tag": "a,b", "untag": ["c"], "epic": "none", "milestone": "x-m",
                        "priority": 0, "assignee": "me", "start": "+1", "due": "2026-12-01",
                        "if_start": "none", "if_due": "2026-11-30", "if_unplanned": true }),
                vec![
                    "edit",
                    "x-a",
                    "-b",
                    "b",
                    "-t",
                    "a,b",
                    "--untag",
                    "c",
                    "-e",
                    "none",
                    "--milestone",
                    "x-m",
                    "-p",
                    "0",
                    "-a",
                    "me",
                    "--start",
                    "+1",
                    "--due",
                    "2026-12-01",
                    "--if-start",
                    "none",
                    "--if-due",
                    "2026-11-30",
                    "--if-unplanned",
                ],
            ),
            ("edit", json!({ "id": "x-a", "shift": -3 }), vec!["edit", "x-a", "--shift", "-3"]),
            ("edit", json!({ "id": "x-a", "shift_start": 2 }), vec!["edit", "x-a", "--shift-start", "2"]),
            ("edit", json!({ "id": "x-a", "shift_due": 999999 }), vec!["edit", "x-a", "--shift-due", "999999"]),
            ("note", json!({ "id": "x-a", "text": "글" }), vec!["note", "x-a", "글"]),
            ("note", json!({ "id": "x-a" }), vec!["note", "x-a"]),
            ("defer", json!({ "id": "x-a" }), vec!["defer", "x-a"]),
            (
                "defer",
                json!({ "id": ["x-a", "x-b"], "msg": "다음", "from": "todo" }),
                vec!["defer", "x-a", "x-b", "-m", "다음", "--from", "todo"],
            ),
            ("defer", json!({ "id": "x-a", "undo": true }), vec!["defer", "x-a", "--undo"]),
            ("rm", json!({ "id": ["x-a", "x-b"] }), vec!["rm", "x-a", "x-b"]),
            ("rm", json!({ "line": 3 }), vec!["rm", "--line", "3"]),
            (
                "rm",
                json!({ "line": 3, "yes": true, "match": "1a2b3c4d" }),
                vec!["rm", "--line", "3", "--yes", "--match", "1a2b3c4d"],
            ),
            ("link", json!({ "id": "x-a", "blocks": "x-b,x-c" }), vec!["link", "x-a", "--blocks", "x-b,x-c"]),
            ("link", json!({ "id": "x-a", "unblocks": ["x-b"] }), vec!["link", "x-a", "--unblocks", "x-b"]),
        ];
        for (method, params, argv) in cases {
            let got = built(method, params.clone()).unwrap_or_else(|e| panic!("{method} {params}: {}", e.message));
            assert_eq!(shape(&got), shape(&parsed(&argv)), "{method} {params} ≠ moai {argv:?}");
        }
    }

    /// **clap 이 거절하는 것은 여기서도 `-32602` 다** — 같은 표의 argv 를 clap 에 주면 그쪽도 거절해야 한다. 한쪽만
    /// 거절하면 API 와 CLI 가 같은 부름을 다르게 받는다.
    #[test]
    fn edit_refuses_what_clap_refuses() {
        let cases: Vec<(Value, Vec<&str>, &str)> = vec![
            (json!({ "shift": 1, "start": "+1" }), vec!["--shift", "1", "--start", "+1"], "shift"),
            (json!({ "shift": 1, "due": "+1" }), vec!["--shift", "1", "--due", "+1"], "shift"),
            (json!({ "shift": 1, "shift_start": 1 }), vec!["--shift", "1", "--shift-start", "1"], "shift"),
            (json!({ "shift": 1, "shift_due": 1 }), vec!["--shift", "1", "--shift-due", "1"], "shift"),
            (json!({ "shift_start": 1, "start": "+1" }), vec!["--shift-start", "1", "--start", "+1"], "shift_start"),
            (
                json!({ "shift_start": 1, "shift_due": 1 }),
                vec!["--shift-start", "1", "--shift-due", "1"],
                "shift_start",
            ),
            (json!({ "shift_due": 1, "due": "+1" }), vec!["--shift-due", "1", "--due", "+1"], "shift_due"),
            (
                json!({ "shift_due": 1, "if_unplanned": true, "due": "+1" }),
                vec!["--shift-due", "1", "--if-unplanned", "--due", "+1"],
                "shift_due",
            ),
            (json!({ "if_start": "none" }), vec!["--if-start", "none"], "if_start"),
            (json!({ "if_due": "2026-11-01" }), vec!["--if-due", "2026-11-01"], "if_due"),
            (json!({ "if_unplanned": true }), vec!["--if-unplanned"], "if_unplanned"),
            (json!({ "if_due": "tomorrow", "due": "+1" }), vec!["--if-due", "tomorrow", "--due", "+1"], "if_due"),
            (json!({ "shift": 1_000_000 }), vec!["--shift", "1000000"], "shift"),
            (json!({ "shift_due": -1_000_000 }), vec!["--shift-due=-1000000"], "shift_due"),
            (json!({ "priority": 256 }), vec!["-p", "256"], "priority"),
            (json!({ "priority": -1 }), vec!["-p=-1"], "priority"),
        ];
        for (mut params, flags, key) in cases {
            params["id"] = json!("x-a");
            let e = refused("edit", params.clone());
            assert_eq!(e.code, code::INVALID_PARAMS, "{params}: {}", e.message);
            assert!(e.message.contains(&format!("params.{key}")), "{params}: 키를 안 댄다 — {}", e.message);
            let argv: Vec<&str> = ["moai", "edit", "x-a"].into_iter().chain(flags.iter().copied()).collect();
            assert!(crate::cli::Cli::try_parse_from(&argv).is_err(), "clap 은 {argv:?} 를 받는다 — 여기만 거절한다");
        }
    }

    /// 꼭 줄 것·함께 못 줄 것·형 — 메서드마다. 모르는 키는 그 이름을 댄다.
    #[test]
    fn missing_conflicting_mistyped_and_unknown_fields_are_invalid_params() {
        let cases: Vec<(&str, Value, &str)> = vec![
            ("add", json!({ "title": "t", "from": "-" }), "\"from\""),
            ("add", json!({ "title": "t", "dry_run": true }), "\"dry_run\""),
            ("add", json!({ "title": "t", "quiet": true }), "\"quiet\""),
            ("add", json!({ "title": "t", "type": "epic" }), "\"type\""),
            ("add", json!({ "title": "t", "kind": "story" }), "params.kind"),
            ("add", json!({ "title": "t", "kind": "Epic" }), "params.kind"),
            ("add", json!({ "title": 5 }), "params.title"),
            ("add", json!({ "title": "t", "tag": { "a": 1 } }), "params.tag"),
            ("add", json!({ "title": "t", "priority": "1" }), "params.priority"),
            ("add", json!({ "title": "t", "priority": 1.5 }), "params.priority"),
            ("mv", json!({ "to": "done" }), "params.id"),
            ("mv", json!({ "id": [], "to": "done" }), "params.id"),
            ("mv", json!({ "id": "x-a" }), "params.to"),
            ("mv", json!({ "id": "x-a", "to": "done", "take": "yes" }), "params.take"),
            ("mv", json!({ "id": "x-a", "to": "done", "status": "done" }), "\"status\""),
            ("edit", json!({ "title": "t" }), "params.id"),
            ("edit", json!({ "id": ["x-a"] }), "params.id"),
            ("note", json!({ "text": "t" }), "params.id"),
            ("note", json!({ "id": "x-a", "body": "t" }), "\"body\""),
            ("note", json!({ "id": "x-a", "text": ["t"] }), "params.text"),
            ("defer", json!({ "msg": "m" }), "params.id"),
            ("defer", json!({ "id": "x-a", "undo": 1 }), "params.undo"),
            ("rm", json!({}), "params.id"),
            ("rm", json!({ "id": "x-a", "line": 3 }), "params.line"),
            ("rm", json!({ "line": -1 }), "params.line"),
            ("rm", json!({ "id": "x-a", "argv": ["--yes"] }), "\"argv\""),
            ("link", json!({ "blocks": "x-b" }), "params.id"),
            ("link", json!({ "id": "x-a", "blocks": true }), "params.blocks"),
        ];
        for (method, params, named) in cases {
            let e = refused(method, params.clone());
            assert_eq!(e.code, code::INVALID_PARAMS, "{method} {params}: {}", e.message);
            assert!(e.message.contains(named), "{method} {params}: `{named}` 를 안 댄다 — {}", e.message);
        }
    }

    /// 일곱이 다 쓰기로 달린다 — 멈추는 데몬이 끝까지 기다리는 표시(moai-3hgm.uz0). 이름 표에 빠지면 읽기로 끊긴다.
    #[test]
    fn all_seven_are_registered_as_writes() {
        let mut methods = Methods::new();
        register(&mut methods);
        for name in ["add", "mv", "edit", "note", "defer", "rm", "link"] {
            let line = format!(r#"{{"jsonrpc":"2.0","id":1,"method":"{name}","params":{{}}}}"#);
            match methods.prepare(line.as_bytes()) {
                crate::daemon::rpc::Prepared::Call(call) => assert!(call.writes(), "{name} 가 읽기로 달렸다"),
                crate::daemon::rpc::Prepared::Answered(a) => panic!("{name} 가 안 달렸다: {a:?}"),
            }
        }
    }
}
