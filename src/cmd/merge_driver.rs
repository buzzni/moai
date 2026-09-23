//! `.moai/issues.jsonl` 의 git 커스텀 머지 드라이버.
//!
//! 한 줄이 이슈 하나고 id 로 정렬돼 있다. git 의 기본 텍스트 머지는 그것을 모르고 줄
//! 자리로만 재므로, **서로 다른 이슈**를 고친 두 가지가 그 줄들이 이웃이라는 이유로
//! 충돌한다. 여기서는 줄을 id 로 짝지어 이슈마다 3-way 로 푼다.
//!
//! **`merge=union` 은 쓰지 않는다.** union 은 같은 id 를 가진 줄 둘을 조용히 남기고,
//! 그건 데이터 손상이다 — 옛 moai 복잡도의 발원지가 거기였다(CLAUDE.md).
//!
//! ## 무엇을 스스로 풀고 무엇을 사람에게 넘기나
//!
//! 가르는 자는 하나다 — **조용한 손실이 이 도구가 못 견디는 유일한 실패 모드다.**
//! 그래서 "늦게 쓴 쪽이 이긴다" 로 줄을 통째로 고르지 않는다. 그렇게 하면 먼저 쓴
//! 쪽의 고침이 아무 자취 없이 사라진다. 대신 **필드마다** 3-way 로 본다.
//!
//! - 한쪽만 바꾼 필드 → 바꾼 쪽
//! - 둘 다 같은 값으로 바꾼 필드 → 그 값
//! - 둘 다 다르게 바꾼 필드 → [`settle`] 이 아는 필드면 그 규칙, 아니면 **충돌**
//!
//! 모르는 필드(`rest`)도 같은 자로 잰다 — 새 바이너리가 쓴 필드를 옛 바이너리가
//! 병합하다 지우면 그것도 조용한 손실이다.
//!
//! ## 못 읽는 줄은 들고 가고, 못 짝지을 때만 통째로 넘긴다
//!
//! 이슈로 안 읽히는 줄도 **JSON 과 `id` 까지 읽히면 id 로 짝지어**(moai-1a55.4oh) 이슈마다
//! 3-way 로 풀고, 그 답은 원문 그대로 파일 뒤에 붙인다 — `store::render_issues` 가 두는 자리와
//! 같다. id 조차 못 읽는 줄은 [`unkeyed`] 가 줄마다 센 수로 푼다. 그 한 줄로 파일 전체를
//! 충돌로 넘기면 드라이버를 심은 저장소가 안 심은 저장소보다 합치기 어려워지고, CLAUDE.md 가
//! 막은 자리가 바로 그것이다("남의 낡은 줄 하나가 모든 쓰기를 막으면 되돌릴 방법이 도구 밖에만
//! 남는다").
//!
//! 통째로 넘기는 것은 **짝지을 수가 없을 때** 둘이다 — 한 파일에 **읽히는** 줄이 같은 id 로
//! 둘 있을 때(그때는 id 로 짝짓는 것 자체가 거짓이다), 그리고 글자가 깨져 줄로도 못 나눌 때.
//! 답을 못 짓는 갈래도 **표식을 쓰고 나서** 비영으로 끝낸다: 표식 없이 실패하면 git 은 `%A` 를
//! 그대로 둔 채 "충돌" 이라고만 말하고, 사람은 그 파일을 열어 보고 이미 풀린 줄 알아
//! `git add` 한 번으로 저쪽을 통째로 버린다.

use super::{Ctx, Fail, R};
use crate::cli::MergeDriverArgs;
use crate::i18n::{fill, say};
use crate::model::Issue;
use crate::store::Repo;
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// git 설정에 심는 이름. `.gitattributes` 의 `merge=moai` 가 이것을 가리킨다.
pub const DRIVER: &str = "moai";

/// 그 이름이 걸리는 파일 — 저장소 뿌리에서 본 자리. [`declared`] 가 git 에게 이 자리의 `merge`
/// 속성을 묻고, `cmd::init` 의 `GITATTRIBUTES` 가 그 줄을 쓴다. **둘이 갈리면 알림이 영영 안
/// 서거나 영영 안 걷힌다** — `the_declared_path_is_the_one_init_writes` 가 그것을 맨다.
pub(crate) const SNAPSHOT: &str = ".moai/issues.jsonl";

/// 이 드라이버를 부르는 부명령. **적는 쪽과 재는 쪽이 한 글을 쓴다**(`driver_key` 와 같은 까닭,
/// 리뷰 moai-vbmn.spv) — [`driver_command`] 가 심는 줄, [`planted_word`] 가 찾는 표식,
/// [`probe`] 가 재려고 부르는 이름이 모두 이것이다.
///
/// **이름을 바꾸는 날 나는 일은 `Alien` 이 아니라 침묵이다.** 이미 심긴 줄은 옛 이름을 들고
/// 있어 [`planted_word`] 의 표식에 안 걸리고, `notice` 는 `?` 에서 `None` 으로 끝난다 — 그
/// 클론은 새 바이너리에 없는 부명령을 부르는 줄을 든 채 매 병합이 `git merge-file` 로 내려앉는데
/// 아무 말도 안 듣는다. 안 조르는 것이 목적인 도구에서 **가장 나쁜 쪽**이다. 셋을 한 글에 매어
/// 두는 것은 그 날을 한 군데서 처리하기 위함이고, 실제로 그 날을 잡는 자는 `planted_word` 의
/// 시험에 손으로 박힌 옛 줄(`/w/moai merge-driver %O %A %B %L %P`)이다.
const SUB: &str = "merge-driver";

/// 한 줄과 그 줄을 읽은 값.
///
/// **원문을 곁에 든다.** 충돌 표식에 넣는 것은 사람이 실제로 커밋한 그 바이트여야 한다 —
/// `Value` 를 다시 직렬화하면 키 차례가 바뀐다(`serde_json` 의 `Map` 은 `BTreeMap` 이라
/// 이름 차례로 선다). 그러면 사람이 보는 줄이 `git diff` 가 보여 준 줄과 다르고, 그 줄을
/// 골라 두면 다음 쓰기가 표준 차례로 되써서 통째로 헛 diff 를 낸다(멱등성).
struct Row<'s> {
    raw: &'s str,
    v: Value,
    /// `Issue` 로 읽히는가. **거짓이어도 짝은 짓는다**(moai-1a55.4oh) — JSON 과 `id` 까지만
    /// 읽히면 그 줄이 어느 이슈의 것인지는 알고, 그것이 3-way 에 필요한 전부다. 값을 못
    /// 지으므로 [`settle`] 은 그 줄을 원문 그대로 실어 나른다([`Settled::Opaque`]).
    read: bool,
}

/// 파일 하나를 id 로 짝지은 결과.
struct Keyed<'s> {
    /// **id 하나에 줄 여럿**(moai-2m94). 여느 때는 하나지만, 손으로 푼 충돌이 같은 id 를 두 번
    /// 남기면 둘이 선다. 밀어내지 않는 것은 어느 줄이 어느 칸에 서는지가 **그 파일에 무엇이 또
    /// 있는가**에 달리는 순간 세 쪽이 서로 다르게 갈리고, 그러면 [`settle`] 과 [`unkeyed`] 가
    /// 같은 줄을 서로 다른 자로 풀기 때문이다 — 한쪽이 지운 줄이 말없이 되살아나던 자리다.
    by_id: BTreeMap<String, Vec<Row<'s>>>,
    /// **id 조차 못 읽는 줄**의 원문 그대로. `store::render_issues` 의 `opaque` 와 같은 자리고,
    /// 열쇠가 없어 이슈마다 풀 수가 없다 — [`unkeyed`] 가 줄마다 센 수로 3-way 를 돌린다.
    ///
    /// **여기 있는 줄에는 id 가 없다.** `Issue` 로만 안 읽히는 줄은 `by_id` 에 서고 [`Row::read`]
    /// 가 거짓이다(moai-1a55.4oh).
    opaque: Vec<&'s str>,
}

/// 한 이슈를 푼 결과.
enum Settled {
    /// 이 줄로 쓴다. 지워진 것이면 `None`.
    ///
    /// **`Issue` 가 아니라 다 쓴 글로 든다.** 이 자리에서 쓸 것은 `store::render_issues` 와
    /// 같은 한 줄뿐이라 값을 들고 다닐 까닭이 없고, `Issue` 를 담으면 갈래끼리 크기가
    /// 크게 벌어져(`clippy::large_enum_variant`) id 마다 그 큰 쪽만큼 옮긴다.
    Line(Option<String>),
    /// 못 읽는 줄을 **원문 그대로** 들고 간다. `Issue` 로 못 읽으니 표준형으로 맞출 값도
    /// 검사할 값도 없고, 남이 남긴 그 줄에 손대는 것은 `store::with_write` 도 안 하는 일이다.
    ///
    /// **[`Settled::Line`] 과 갈라 두는 것은 자리 때문이다.** `store::render_issues` 는 못
    /// 읽는 줄을 파일 끝에 모아 쓰므로, 여기서 id 차례로 끼워 넣으면 병합 직후의 파일이 다음
    /// 쓰기에서 통째로 헛 diff 를 낸다(멱등성).
    Opaque(String),
    /// 사람이 푼다. 두 쪽의 줄을 **원문 그대로** 든다 — 고쳐 적으면 사람이 보는 글이
    /// 제가 친 것과 달라진다([`Row`]).
    Clash { ours: Option<String>, theirs: Option<String> },
}

/// `--json` 이 내는 줄. **충돌일 때도 낸다** — 어느 id 를 사람이 봐야 하는지 말하는
/// 유일한 자리라, 종료 코드만 남기고 버리면 기계가 한국말 문장을 긁어야 한다.
#[derive(serde::Serialize)]
struct Out<'a> {
    path: &'a str,
    conflicts: &'a [String],
}

pub fn run(ctx: &Ctx, args: MergeDriverArgs) -> R<Vec<String>> {
    if args.install {
        return install(ctx, args.as_command.as_deref());
    }
    let (Some(base), Some(ours), Some(theirs)) = (&args.base, &args.ours, &args.theirs) else {
        // **여기까지가 사람이 친 길이다**(moai-uzgp) — 자리를 안 주고 부를 수 있는 것은 사람뿐이라,
        // 이 거절문만 화면 말을 안다. 아래 git 이 부르는 길은 그대로 둔다.
        return Err(Fail::coded(say(ctx.lang(), "refuse.driver_places"), super::code::BAD_INPUT));
    };
    // **셋 다 먼저, 바이트로 읽는다.** `ours` 는 답을 쓸 자리이기도 해서, 쓰기 시작한 뒤에
    // theirs 를 못 읽으면 지운 것도 안 쓴 것도 아닌 파일이 남는다.
    //
    // **글자가 깨진 파일도 두 쪽을 다 보여 준다.** 여기서 그냥 실패하면 git 은 `%A` 를 손대지
    // 않은 채 "충돌" 이라고만 말하는데, 그 파일에는 표식이 없어 사람이 열어 보고 "이미 풀렸다"
    // 로 읽는다 — `git add` 한 번에 저쪽이 통째로 사라진다. 조용한 손실이 이 도구가 못 견디는
    // 유일한 실패 모드라, 답을 못 짓는 갈래도 **표식을 쓰고 나서** 비영으로 끝낸다.
    let read = |p: &Path| std::fs::read(p).map_err(|e| Fail::new(format!("{}: {e}", p.display())));
    let (o, a, b) = (read(base)?, read(ours)?, read(theirs)?);
    let marker = args.marker_size.unwrap_or(7).max(1);
    let what = args.path.as_deref().unwrap_or(SNAPSHOT);

    fn utf8(v: &[u8]) -> Option<&str> {
        std::str::from_utf8(v).ok()
    }
    let (text, clashes) = match (utf8(&o), utf8(&a), utf8(&b)) {
        (Some(o), Some(a), Some(b)) => {
            let (t, c) = plan(o, a, b, marker);
            (t.into_bytes(), c)
        }
        // 글자가 깨진 줄은 id 로 짝지을 수가 없다 — 두 쪽을 통째로 넘긴다.
        _ => whole_bytes(&a, &b, marker),
    };
    // **`%A` 는 temp+rename 으로 갈아끼운다**(`store::write_atomic`, 리뷰 moai-h6aq.cx8). 맨
    // `fs::write` 는 자르고 나서 쓰므로, 그 사이에 죽으면(ENOSPC·OOM kill) `%A` 가 잘린 파일로
    // 남는다. 심는 줄은 "드라이버가 `%A` 를 안 건드렸다" 를 보고 git 의 기본 머지로 내려앉는데
    // ([`driver_command`]), 잘린 파일은 건드린 것도 안 건드린 것도 아니라 그 판정이 거짓이 된다.
    // 여기서 원자적으로 쓰면 `%A` 는 늘 **받은 그대로**이거나 **다 쓴 답**이고, 판정이 참이 된다.
    crate::store::write_atomic(ours, &text)?;

    if clashes.is_empty() {
        return match ctx.json {
            true => super::json_line(&Out { path: what, conflicts: &clashes }),
            false => Ok(Vec::new()),
        };
    }
    // 종료 코드가 git 에게 "충돌" 이다. **`--json` 은 답을 먼저 낸다** — `json_line` 은 찍지
    // 않고 줄을 돌려주고 `main` 은 `Err` 에서 그 줄을 버리므로(`main::fail`), 그냥 `Err` 로
    // 나가면 `conflicts` 가 어디에도 안 선다. `note_partial` 이 "다 낸 뒤에 비영" 을 맡는다.
    if ctx.json {
        super::note_partial();
        return super::json_line(&Out { path: what, conflicts: &clashes });
    }
    Err(clash_fail(&clashes, what))
}

/// 셋 다 글자로 읽히는 판.
///
/// **못 읽는 줄로는 파일 전체를 넘기지 않는다**(moai-1a55.4oh). 두 쪽의 못 읽는 줄이 하나라도
/// 다르면 여기서 [`whole`] 로 갔는데, 그 한 줄 때문에 나머지 천 줄이 이슈마다 풀리는 것을
/// 잃었다. 새 바이너리가 쓴 `kind` 를 옛 바이너리가 못 읽는 판이 바로 그것이고, 그 줄을
/// 한쪽에서 고치는 것이 곧 두 쪽을 다르게 만드는 일이라 스스로 빠져나올 수도 없었다 —
/// `moai status` 는 못 읽는 줄을 치명으로 센다. [`keyed`] 가 id 까지 읽히는 줄을 짝지어
/// [`settle`] 로 보내고, id 조차 못 읽는 줄은 [`unkeyed`] 가 센 수로 3-way 한다.
///
/// 남은 [`whole`] 갈래는 **읽히는 줄이 같은 id 로 둘 있는** 판 하나다. 그때는 id 로 짝짓는
/// 것 자체가 거짓이라 풀 자가 없다.
fn plan(o: &str, a: &str, b: &str, marker: usize) -> (String, Vec<String>) {
    match (keyed(o), keyed(a), keyed(b)) {
        (Some(o), Some(x), Some(y)) => by_issue(&o, &x, &y, marker, a.len()),
        _ => whole(a, b, marker),
    }
}

/// **이 줄은 화면 말을 모른다**(moai-uzgp, 2026-09-21 사용자 결정). git 이 병합마다 부르는 길이라
/// 여기서 사용자 설정을 열면 병합마다 그 파일이 열리고, 저장 계층을 화면 말에서 떼어 둔 결정의
/// 까닭이 바로 이 길이다. 명령줄로 친 길(`--install`·자리를 안 준 부름)만 제 말로 편다.
fn clash_fail(clashes: &[String], what: &str) -> Fail {
    Fail::coded(format!("{what}: {}건은 사람이 푼다 — {}", clashes.len(), clashes.join(" ")), super::code::BROKEN)
}

/// 파일 하나를 id → 줄로. **읽히는 줄이 같은 id 로 둘 있으면 `None`** — 그때는 id 로
/// 짝짓는 것 자체가 거짓이 된다.
///
/// **못 읽는 줄도 id 까지 읽히면 짝짓는다**(moai-1a55.4oh). 짝지어야 한쪽만 건드린 판과 둘 다
/// 건드린 판이 갈리고, 뒤엣것만 사람에게 간다. `Issue` 로 읽히는지는 [`Row::read`] 가 말하고
/// 짝짓기를 막지 않는다.
///
/// 버리지 않는 것은 그 줄이 사라지면 조용한 손실이라서고, 파일 전체를 넘기지 않는 것은
/// CLAUDE.md 가 정한 자리라서다 — *"그 엄함은 지금 쓰는 줄에 대한 것이지 파일 전체에 대한
/// 것이 아니다. 남의 낡은 줄 하나가 모든 쓰기를 막으면 되돌릴 방법이 도구 밖에만 남는다."*
/// `store::with_write` 가 못 읽는 줄을 들고 다시 쓰므로 그런 줄은 저장소에 오래 남는데, 그 한
/// 줄로 모든 병합을 파일째 충돌로 넘기면 드라이버를 심은 저장소가 안 심은 저장소보다 합치기
/// 어려워진다.
fn keyed(src: &str) -> Option<Keyed<'_>> {
    let src = src.strip_prefix('\u{feff}').unwrap_or(src);
    let mut by_id: BTreeMap<String, Vec<Row<'_>>> = BTreeMap::new();
    let mut opaque = Vec::new();
    for line in src.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let Some((id, r)) = row(line) else {
            opaque.push(line);
            continue;
        };
        let slot = by_id.entry(id).or_default();
        // 읽히는 줄이 같은 id 로 둘이다 — 여기서 하나를 고르면 다른 이슈가 통째로 사라진다.
        if r.read && slot.iter().any(|x| x.read) {
            return None;
        }
        // **못 읽는 줄이 낀 겹침은 밀어내지 않고 쌓는다**(moai-2m94). 한 줄을 `opaque` 로
        // 밀어내면 그 줄이 어느 칸에 서는지가 그 파일의 다른 줄에 달려, 세 쪽이 서로 다르게
        // 갈린다. [`settle`] 이 그 id 를 통째로 사람에게 넘긴다.
        slot.push(r);
    }
    Some(Keyed { by_id, opaque })
}

/// 줄 하나를 id 와 값으로. **JSON 과 `id` 까지 읽히면 짝짓는다** — `Issue` 로도 읽히는지는
/// [`Row::read`] 에 적어 두고 짝짓기를 막지 않는다(moai-1a55.4oh).
///
/// `None` 은 열쇠가 없다는 뜻 하나다: 글자가 JSON 이 아니거나, 이슈가 아닌 줄(배열·수·id 없는
/// 객체)이다. 그런 줄은 [`unkeyed`] 가 센 수로 푼다.
fn row(line: &str) -> Option<(String, Row<'_>)> {
    let v: Value = serde_json::from_str(line).ok()?;
    let id = v.get("id")?.as_str()?.to_string();
    // **`&Value` 에서 바로 읽는다** — `from_value` 는 통째로 복사한 뒤 읽어, 줄마다 트리
    // 하나를 더 짓는다. 값은 아래 [`shaped`] 가 다시 쓰므로 여기서는 읽히는지만 본다.
    let read = Issue::deserialize(&v).is_ok();
    Some((id, Row { raw: line, v, read }))
}

/// id 마다 3-way 로 풀고, 푼 것과 못 푼 것을 한 파일로 짓는다.
///
/// `hint` 는 답의 크기 짐작(이쪽 파일의 길이)이다 — 없으면 1MB 짜리 스냅샷에서 버퍼가
/// 열일곱 번 자라며 이미 쓴 것을 그만큼 옮긴다.
fn by_issue(o: &Keyed<'_>, a: &Keyed<'_>, b: &Keyed<'_>, marker: usize, hint: usize) -> (String, Vec<String>) {
    let ids: BTreeSet<&String> = o.by_id.keys().chain(a.by_id.keys()).chain(b.by_id.keys()).collect();
    let mut out = String::with_capacity(hint);
    let mut carried = Vec::new();
    let mut clashes = Vec::new();
    for id in ids {
        fn at<'r, 's>(k: &'r Keyed<'s>, id: &str) -> Option<&'r [Row<'s>]> {
            k.by_id.get(id).map(Vec::as_slice)
        }
        match settle(at(o, id), at(a, id), at(b, id)) {
            Settled::Line(None) => {}
            Settled::Line(Some(line)) => {
                out.push_str(&line);
                out.push('\n');
            }
            // 짝은 졌지만 `Issue` 로 못 읽는 줄이다 — 아래 못 읽는 줄 칸으로 미룬다.
            Settled::Opaque(line) => carried.push(line),
            Settled::Clash { ours, theirs } => {
                clashes.push(id.clone());
                out.push_str(&marked(ours.as_deref(), theirs.as_deref(), marker));
            }
        }
    }
    // **못 읽는 줄은 뒤에 원문 그대로 붙는다** — `store::render_issues` 가 두는 자리와 같다.
    // 짝지은 것이 id 차례로 먼저 서고 열쇠 없는 것이 뒤따르는데, 그 다음 쓰기가 읽은 차례를
    // 그대로 되쓰므로 이 차례가 곧 고정점이다(멱등성).
    for line in carried {
        out.push_str(&line);
        out.push('\n');
    }
    for line in unkeyed(&o.opaque, &a.opaque, &b.opaque) {
        out.push_str(line);
        out.push('\n');
    }
    (out, clashes)
}

/// **id 조차 못 읽는 줄**을 3-way 로 푼다. 열쇠가 없어 이슈마다 풀 수가 없으니, [`settle_field`]
/// 가 `tags` 에 쓰는 것과 같은 셈을 **줄마다 센 수**로 돌린다 — 더한 것은 더하고 뺀 것은 뺀다.
///
/// 남길 수는 `o + 더한 벌 - 뺀 벌`이고, **더한 벌도 뺀 벌도 두 쪽 중 큰 쪽 하나만 센다.**
/// 두 쪽이 똑같이 한 벌을 더했으면 그것은 한 결정이지 둘이 아니고, 똑같이 한 벌을 뺀 것도
/// 마찬가지다 — `settle_field` 가 `tags` 를 집합으로 셀 때 저절로 서는 성질을 벌 수로 옮긴
/// 것이다. **벌 수를 세는 것은 집합으로 세면 똑같은 줄 여러 벌 중 하나가 말없이 사라지기
/// 때문이다** — 사라진 것이 못 읽는 줄이라 아무 표면에도 안 뜬다.
///
/// 처음 판은 `min(max(a, b), a + b - o)` 였는데(리뷰 moai-1a55.67v), 그 꼴은 더하기를 `max` 로
/// 한 번만 세면서 빼기는 `a + b - o` 로 두 번 셌다. 두 벌 있던 줄을 두 쪽이 한 벌씩 지우면
/// (`o=2, a=1, b=1`) 남는 수가 0 이 되어, 두 쪽이 다 남기려던 벌이 표식도 경고도 없이 사라졌다.
///
/// **여기에는 충돌이 없다.** 열쇠가 없는 줄에서 "고쳤다" 는 "지우고 새로 더했다" 와 구별되지
/// 않아, 둘 다 고친 판을 가려낼 자가 없다. 그 판에서는 두 쪽이 다 남아 `moai status` 가 못
/// 읽는 줄 둘로 세고 사람이 하나를 지운다 — 한쪽을 골라 버리는 것보다 낫고, 그것이 이 자리에서
/// 열쇠 없이 지킬 수 있는 전부다. id 가 읽히는 줄은 [`keyed`] 가 짝지어 그 판을 [`settle`] 로
/// 보내고, 거기서는 표식이 그 줄에만 선다.
///
/// **차례는 바탕을 따른다** — 바탕에 섰던 줄이 그 차례대로 먼저 서고, 이쪽이 더한 줄, 저쪽이
/// 더한 줄이 뒤따른다. 한쪽이 제 못 읽는 줄의 차례만 바꿔 두었으면 그 바꿈은 여기서 되물러진다.
/// 잃는 것은 없고(줄은 다 선다) 다음 쓰기의 고정점도 그대로지만, 아무도 안 건드린 줄이 머지
/// 커밋에 한 번 뜬다.
fn unkeyed<'s>(o: &[&'s str], a: &[&'s str], b: &[&'s str]) -> Vec<&'s str> {
    fn tally<'s>(v: &[&'s str]) -> BTreeMap<&'s str, usize> {
        let mut m = BTreeMap::new();
        for l in v {
            *m.entry(*l).or_insert(0) += 1;
        }
        m
    }
    let (co, ca, cb) = (tally(o), tally(a), tally(b));
    let at = |m: &BTreeMap<&'s str, usize>, l: &'s str| m.get(l).copied().unwrap_or(0);
    let mut want = BTreeMap::new();
    // 두 쪽 다 없는 줄은 셀 것이 없다 — 둘이 함께 지웠거나 처음부터 없던 줄이다.
    for l in ca.keys().chain(cb.keys().filter(|l| !ca.contains_key(*l))) {
        let (n, x, y) = (at(&co, l), at(&ca, l), at(&cb, l));
        // **더하기도 빼기도 큰 쪽 하나만 센다.** 둘이 똑같이 한 벌을 더한 것을 두 번 세면
        // 같은 줄이 둘로 서고, 똑같이 한 벌을 뺀 것을 두 번 세면 두 쪽이 다 남기려던 벌이
        // 사라진다 — 뒤엣것이 조용한 손실이다.
        let added = x.saturating_sub(n).max(y.saturating_sub(n));
        let gone = n.saturating_sub(x).max(n.saturating_sub(y));
        want.insert(*l, (n + added).saturating_sub(gone));
    }
    let mut out = Vec::new();
    for l in o.iter().chain(a).chain(b) {
        if let Some(n) = want.get_mut(*l).filter(|n| **n > 0) {
            *n -= 1;
            out.push(*l);
        }
    }
    out
}

/// 못 짝지을 때. **읽은 것만 골라 쓰지 않는다** — 못 읽은 줄이 그 자리에서 사라진다.
fn whole(a: &str, b: &str, marker: usize) -> (String, Vec<String>) {
    let (v, clashes) = whole_bytes(a.as_bytes(), b.as_bytes(), marker);
    (String::from_utf8(v).expect("조각이 다 UTF-8 이었다"), clashes)
}

/// 같은 일을 바이트로. **글자가 깨진 파일도 이 길로 두 쪽이 다 사람에게 간다.**
fn whole_bytes(a: &[u8], b: &[u8], marker: usize) -> (Vec<u8>, Vec<String>) {
    fn strip(s: &[u8]) -> &[u8] {
        s.strip_suffix(b"\n").unwrap_or(s)
    }
    (marked_bytes(Some(strip(a)), Some(strip(b)), marker), vec!["(파일 전체)".into()])
}

/// git 이 쓰는 것과 같은 모양의 충돌 표식. 한쪽이 지운 것이면 그 칸이 빈다.
fn marked(ours: Option<&str>, theirs: Option<&str>, marker: usize) -> String {
    let v = marked_bytes(ours.map(str::as_bytes), theirs.map(str::as_bytes), marker);
    String::from_utf8(v).expect("조각이 다 UTF-8 이었다")
}

fn marked_bytes(ours: Option<&[u8]>, theirs: Option<&[u8]>, marker: usize) -> Vec<u8> {
    let len = ours.map_or(0, <[u8]>::len) + theirs.map_or(0, <[u8]>::len);
    let mut s = Vec::with_capacity(len + 3 * marker + 18);
    let side = |s: &mut Vec<u8>, bar: u8, head: &[u8], body: Option<&[u8]>| {
        s.extend(std::iter::repeat_n(bar, marker));
        s.extend_from_slice(head);
        s.push(b'\n');
        if let Some(l) = body {
            s.extend_from_slice(l);
            s.push(b'\n');
        }
    };
    side(&mut s, b'<', b" ours", ours);
    side(&mut s, b'=', b"", None);
    // 가른 줄 다음이 저쪽이다 — 머리와 몸을 한 함수가 쓰므로 차례가 갈릴 수 없다.
    if let Some(l) = theirs {
        s.extend_from_slice(l);
        s.push(b'\n');
    }
    s.extend(std::iter::repeat_n(b'>', marker));
    s.extend_from_slice(b" theirs\n");
    s
}

/// 이슈 하나를 푼다.
///
/// 지우기는 git 의 3-way 와 같은 뜻으로 읽는다 — 한쪽이 지우고 다른 쪽이 안 건드렸으면
/// 지운 것이고, 지운 쪽과 고친 쪽이 맞서면 사람이 푼다. **둘 다 새로 세운 같은 id** 는
/// 내용이 같을 때만 지나간다: 랜덤 id 가 부딪친 것이라면 두 이슈는 서로 다른 일이고,
/// 하나를 골라 주면 나머지 하나가 통째로 사라진다.
///
/// **답이 한쪽 가지에 이미 서 있으면 검사를 건너뛴다**([`take`], moai-0k2e, 2026-09-23
/// 사용자 결정 — 그 결정과 뒤에 뒤집힌 절반이 `moai show moai-0k2e` 의 이력에 있다). 양쪽이 안
/// 건드린 줄, 한쪽만 건드린 줄, 둘이 똑같이 고친 줄이 그것이다 — 이번 머지가 고른 값이
/// 아니라 이미 커밋된 값이라, `validate_fields` 가 잴 것이 없다. 검사가 붙는 곳은
/// [`fields`] 가 필드마다 골라 **새로 조립한** 줄 하나뿐이고([`issue`]), 그것이 `issue` 의
/// 주석이 대는 유일한 까닭이다.
///
/// 이 갈래가 없던 동안, 오늘 규칙이 거절하는 값을 든 줄이 하나라도 있으면 양쪽이 그 줄을
/// 안 건드려도 `.moai/issues.jsonl` 이 **머지마다** 충돌했다 — 표식 안의 두 쪽이 글자째
/// 같아 사람이 고를 것도 없고, 그 값을 손으로 지우기 전에는 영영 되풀이됐다. CLAUDE.md 의
/// *"그 엄함은 지금 쓰는 줄에 대한 것이지 파일 전체에 대한 것이 아니다"* 가 [`keyed`] 의
/// 못 읽는 줄에만 서고 읽히는 줄에는 안 서 있던 자리다.
///
/// **건너뛰는 것은 검사뿐이고 표준형 맞추기는 그대로다**([`shaped`], 리뷰 moai-85o3.p4w).
/// 처음 고친 판은 원문 바이트를 그대로 냈는데, 그 절반은 `max` 리뷰의 실측을 받아 같은 날
/// 사용자가 뒤집었다.
/// `store::with_write` 가 서 있는 자리가 바로 거기다 — 그쪽은 읽은 줄을 **모두**
/// `Issue::normalize` 하고 검사만 이번에 바뀐 줄에 건다. 꼴까지 건너뛴 판을 재 보니 넷이
/// 무너졌다: 접히지 않은 `"#Bug"` 가 `moai show -t bug` 에서 빠지고, `"started_at":""` 이
/// `move_to` 에 "이미 적혔다" 로 읽혀 다음 `mv` 가 시작을 영영 안 적고(moai-38mh),
/// `"due_on":""` 이 기한 경고를 영영 조용하게 두고(moai-tfcp), 겹친 키를 든 줄이
/// `store::parse_issues`(줄을 통째로 `Issue` 로 읽는다)에 못 읽는 줄로 가 `moai status` 가
/// 비영으로 끝난다 — [`row`] 는 `Value` 를 거쳐 읽어 겹친 키를 마지막 것으로 접으므로 둘의
/// 문턱이 다르다. 그리고 그렇게 지나간 값은 **다음 머지에서 새 충돌을 짓는다**:
/// 표준형이 아닌 값은 `Value` 로도 다른 값이라 [`fields`] 까지 떨어지는데, `priority` 처럼
/// [`settle_field`] 가 모르는 필드면 그 줄이 통째로 사람에게 간다.
fn settle(o: Option<&[Row<'_>]>, a: Option<&[Row<'_>]>, b: Option<&[Row<'_>]>) -> Settled {
    // **원문을 그대로 든다**([`Row`]). 한 쪽이 그 id 로 줄을 여럿 들면 그 줄들이 다 선다.
    // 매개변수를 안 받는 것은 여덟 자리 중 한 곳에서 두 쪽을 뒤집어 넘기는 실수가
    // 컴파일되지 않게 하려는 것이다.
    let side = |v: Option<&[Row<'_>]>| {
        v.map(|rs| rs.iter().map(|r| r.raw).collect::<Vec<_>>().join("\n"))
    };
    let clash = || Settled::Clash { ours: side(a), theirs: side(b) };
    // **한 쪽이라도 같은 id 로 둘을 들면 그 id 는 사람이 푼다**(moai-2m94). 읽히는 줄 둘은
    // [`keyed`] 가 이미 걸렀으니 여기 오는 것은 못 읽는 줄이 낀 판이고, 그 둘 중 무엇이 "그
    // 이슈" 인지 고를 자가 없다 — 고르면 다른 하나가, 또는 한쪽의 지우기가 말없이 사라진다.
    // 그런 파일은 이미 `moai status` 가 치명으로 센다(`Ids standing twice`·`Unreadable rows`).
    if [o, a, b].iter().any(|v| v.is_some_and(|rs| rs.len() > 1)) {
        return clash();
    }
    let (o, a, b) = (o.and_then(<[Row<'_>]>::first), a.and_then(<[Row<'_>]>::first), b.and_then(<[Row<'_>]>::first));
    let render = |i: &Issue| Settled::Line(Some(serde_json::to_string(i).expect("Issue 는 언제나 직렬화된다")));
    // **이미 선 줄은 검사만 건너뛴다**([`shaped`]). 꼴은 `store` 가 쓰는 것과 같은 표준형이다.
    // **`Issue` 로 안 읽히는 줄은 원문 그대로 간다**(moai-1a55.4oh) — 지을 값이 없다.
    let take = |r: &Row<'_>| match r.read {
        true => render(&shaped(&r.v).expect("`Row::read` 가 참인 줄이다")),
        false => Settled::Opaque(r.raw.to_string()),
    };
    // **이 머지가 새로 지은 줄은 검사까지 지난다**([`issue`]).
    let built = |v: &Value| issue(v).map_or_else(clash, |i| render(&i));
    // 값으로 견준다 — 같은 뜻의 줄이 바이트로 다를 수 있다(키 차례·모르는 필드).
    let same = |p: Option<&Row<'_>>, q: &Row<'_>| p.is_some_and(|p| p.v == q.v);
    match (o, a, b) {
        (_, None, None) => Settled::Line(None),
        // 한쪽만 건드렸다.
        (o, Some(x), None) | (o, None, Some(x)) if same(o, x) => Settled::Line(None),
        (None, Some(x), None) | (None, None, Some(x)) => take(x),
        // 지운 쪽과 고친 쪽이 맞선다.
        (Some(_), _, None) | (Some(_), None, _) => clash(),
        (o, Some(x), Some(y)) => {
            // 둘이 같거나 저쪽이 안 건드렸으면 이쪽 값이 답이다.
            if x.v == y.v || same(o, y) {
                return take(x);
            }
            if same(o, x) {
                return take(y);
            }
            // **둘 다 새로 세운 같은 id 는 안 섞는다** — 서로 다른 일이다.
            let Some(o) = o else { return clash() };
            fields(&o.v, &x.v, &y.v).map_or_else(clash, |v| built(&v))
        }
    }
}

/// 이슈로 읽고 **표준형으로 맞추기만 한다** — 검사는 [`issue`] 가 얹는다.
///
/// 이 갈래를 [`settle`] 의 모든 답이 지난다. 맞추지 않으면 병합 직후의 파일이 다음 쓰기에서
/// 통째로 헛 diff 를 낸다(멱등성). `store::with_write` 도 읽은 줄을 모두 `normalize` 한 뒤
/// 되쓰므로, 여기서 같은 자를 대야 병합 결과가 그 쓰기의 고정점이 된다 — 무엇이 무너지는지는
/// [`settle`] 의 주석에 넷으로 적어 두었다.
///
/// **못 읽는 줄은 여기 안 온다**([`settle`] 의 `take` 가 [`Row::read`] 를 먼저 본다). 그래서
/// `None` 은 그 갈래에서 일어나지 않고, [`fields`] 가 조립한 줄에서만 뜻이 선다.
fn shaped(v: &Value) -> Option<Issue> {
    let mut i = Issue::deserialize(v).ok()?;
    i.normalize();
    Some(i)
}

/// **[`fields`] 가 새로 조립한 줄에만 부른다**([`settle`]). 한쪽 가지에 이미 선 줄은
/// 이 머지가 고른 값이 아니라 [`shaped`] 까지만 지난다 — 그 갈래가 이 함수의 범위다.
///
/// 쓰기 전에 이슈로 한 번 읽는다 — **읽기는 관대하고 쓰기는 엄하다.** 여기서 안 읽히는
/// 줄을 내보내면 그 줄은 다음 `moai` 가 못 읽는 줄로 만나고, 병합이 그것을 만든 것이 된다.
///
/// **검사도 `store::with_write` 와 같은 것을 건다**(`Issue::validate_fields`). 필드마다
/// 따로 고른 값들이 줄 하나로 서면서 도구가 절대 안 쓰는 짝이 될 수 있다 — 한쪽이
/// `kind` 를 마일스톤으로 바꾸고 다른 쪽이 `milestone` 을 붙이면 각 필드는 한쪽만 고친
/// 것이라 말없이 합쳐지는데, 그 줄은 `moai edit`·`mv`·`defer` 가 모두 거절한다. 여기서
/// 거르면 그 줄은 사람에게 가고, 넘기면 병합이 도구로 못 만지는 줄을 만든 것이 된다.
/// **이 한 판이 검사를 두는 까닭 전부**라, 이 함수가 딴 데서 불리면 그 까닭이 없는 곳에
/// 검사가 서고 남의 낡은 줄이 머지마다 충돌한다(moai-0k2e).
///
/// **칸 이름은 안 본다.** 그것만 `.moai/config.toml` 을 읽어야 하는데 이 명령은 설정을
/// 안 지난다(`cmd/mod.rs`). 설정으로 칸을 고친 저장소의 옛 줄을 병합이 막지 않는 쪽이
/// 맞기도 하다 — CLAUDE.md 의 "엄함은 지금 쓰는 줄에 대한 것" 과 같은 자리다.
fn issue(v: &Value) -> Option<Issue> {
    let i = shaped(v)?;
    i.validate_fields().ok()?;
    Some(i)
}

/// 둘 다 고친 줄을 **필드마다** 3-way 로 푼다. 한 필드라도 못 풀면 `None` — 줄 하나가
/// 이슈 하나라, 반만 푼 줄은 뜻이 없다.
fn fields(o: &Value, a: &Value, b: &Value) -> Option<Value> {
    let (o, a, b) = (o.as_object()?, a.as_object()?, b.as_object()?);
    let forced = coupled(o, a, b);
    let keys: BTreeSet<&String> = o.keys().chain(a.keys()).chain(b.keys()).collect();
    let mut out = Map::new();
    for k in keys {
        let picked = match forced.get(k.as_str()) {
            // **짝으로 움직이는 필드는 한 쪽에서 통째로 온다**([`coupled`]).
            Some(v) => v.clone(),
            None => {
                let (ov, av, bv) = (o.get(k), a.get(k), b.get(k));
                match (ov, av, bv) {
                    _ if av == bv => av.cloned(),
                    _ if ov == av => bv.cloned(),
                    _ if ov == bv => av.cloned(),
                    // 둘 다 다르게 바꿨다.
                    _ => settle_field(k, ov, av, bv)?,
                }
            }
        };
        if let Some(v) = picked {
            out.insert(k.clone(), v);
        }
    }
    Some(Value::Object(out))
}

/// **짝으로 움직이는 필드** — 이끄는 필드가 어느 쪽에서 왔으면 따르는 필드도 그 쪽에서 온다.
///
/// [`fields`] 는 키를 하나씩 보므로, 한 결정이 남긴 두 값을 각자 풀면 서로 어긋난 줄이 나온다.
/// 실제로 나온 두 가지가 이것이다.
///
/// - `status_since` 는 **지금 선 `status` 에 든 때**다. 칸을 한쪽에서 가져오고 시각을 다른
///   쪽에서 가져오면, 그 칸에 들기도 전의 때가 붙는다 — 한쪽이 칸을 왕복해 두면(`todo` →
///   `in_progress` → `todo`) 그 왕복 시각이 다른 쪽이 옮긴 칸의 나이가 되어, 방치 경고와
///   `--stale` 과 보드의 나이가 한꺼번에 거짓을 말한다.
/// - `deferred_at` 은 `planned_at` 이 적은 그 결정의 결과다(moai-l11z). 따로 고르면 한쪽의
///   `defer --undo` 가 아무 자취 없이 되물러진 채 그 `planned_at` 만 남는다 — 미뤄 둔 줄이
///   도로 집힌 때를 제 시각으로 들고, `ready` 에서도 보드에서도 빠진다.
///
/// 이끄는 필드가 셋 다 다를 때는 `status` 면 손을 안 댄다([`settle_field`] 가 그 줄을 사람에게
/// 넘긴다). `planned_at` 은 **늦게 친 쪽**이 지금 상태라, 그 쪽을 골라 `deferred_at` 을 함께
/// 데려온다 — 그래서 `settle_field` 에 `planned_at` 갈래가 따로 없다.
///
/// **둘 다 표준형이고 초가 갈릴 때만 늦은 쪽을 고른다**(moai-1a55.kh2). 한쪽이라도 [`time`] 이
/// 못 읽으면 손을 떼고 `settle_field` 에 넘겨 그 줄이 사람에게 간다. 못 읽는 시각을 "모른다" 로 세어 다른
/// 쪽을 고르면 그 쪽의 `planned_at` 과 `deferred_at` 이 통째로 이기고, **진 쪽의 미루기가 자취
/// 없이 사라진다** — 손으로 푼 줄의 `+09:00` 하나면 `2026-09-19T23:00:00+09:00`(14:00Z)이
/// `2026-09-19T04:00:00Z` 에 지고 종료 코드는 0 이었다. 조용한 손실이 이 도구가 못 견디는
/// 유일한 실패 모드다.
///
/// **[`later`] 보다 한 칸 더 엄하다**(리뷰 moai-1a55.67v). 그쪽은 없는 시각을 "모른다" 로 읽어
/// 있는 쪽을 고르고 못 읽는 시각만 사람에게 넘기는데, 여기서는 **없는 것도 함께 넘긴다.**
/// `updated_at` 이 없는 것은 모르는 것이지만 `planned_at` 이 없는 것은 *미뤄 두지 않았다* 는
/// 결정이라, 있는 쪽을 골라 주면 그 결정이 `deferred_at` 과 함께 자취 없이 사라진다. 두 필드의
/// "없음" 이 서로 다른 뜻인 자리라 규칙도 갈린다 — 한 함수로 합칠 때 이 차이를 먼저 인자로
/// 드러내야 한다.
fn coupled<'m>(
    o: &Map<String, Value>,
    a: &'m Map<String, Value>,
    b: &'m Map<String, Value>,
) -> BTreeMap<&'static str, Option<Value>> {
    let mut out = BTreeMap::new();
    for (lead, follows) in [("status", ["status_since"]), ("planned_at", ["deferred_at"])] {
        let (ol, al, bl) = (o.get(lead), a.get(lead), b.get(lead));
        if al == bl {
            continue; // 이끌 것이 없다 — 따르는 필드도 여느 규칙으로 푼다
        }
        let side = if ol == al {
            b
        } else if ol == bl {
            a
        } else if lead == "planned_at" {
            match (time(al), time(bl)) {
                (Some(x), Some(y)) if y > x => b,
                (Some(x), Some(y)) if x > y => a,
                // **한쪽이라도 못 읽으면 말없이 고르지 않는다.** 없는 시각도 못 읽는 시각도
                // 여기서는 같다 — 둘 다 "견줄 수 없다" 이고, 여기 오는 것은 두 쪽이 **다 바꾼**
                // 판뿐이라 한쪽을 고르면 다른 쪽 결정이 통째로 사라진다.
                //
                // **글자가 다른데 초가 같은 것도 여기로 온다**(리뷰 moai-1a55.67v).
                // [`crate::model::parse_rfc3339`] 은 꼴만 재고 날짜의 뜻은 안 재서 `2026-02-30`
                // 과 `2026-03-02` 가, `23:59:60` 과 다음 날 `00:00:00` 이 같은 초로 읽힌다 —
                // 늦은 쪽이 없으니 고를 자도 없는데, 앞선 판은 그때 이쪽을 골라 저쪽의
                // `deferred_at` 을 통째로 버렸다.
                _ => continue,
            }
        } else {
            continue;
        };
        out.insert(lead, side.get(lead).cloned());
        for f in follows {
            out.insert(f, side.get(f).cloned());
        }
    }
    out
}

/// 둘 다 다르게 바꾼 필드의 규칙. **아는 것만 푼다** — 모르면 `None` 이고, 그러면
/// 사람이 푼다. 여기에 필드를 더하는 것은 "이 필드는 말없이 골라도 잃는 것이 없다" 를
/// 주장하는 일이라, 그 까닭을 함께 적는다.
fn settle_field(key: &str, o: Option<&Value>, a: Option<&Value>, b: Option<&Value>) -> Option<Option<Value>> {
    match key {
        // **집합이라 정확히 3-way 로 푼다.** 합집합으로 두면 한쪽이 뺀 태그가 되살아나고,
        // 되살아난 태그는 아무 자취가 없다. 더한 것은 더하고 뺀 것은 뺀다.
        "tags" | "blocked_by" => {
            // **태그는 표기를 먼저 맞춘다**(`model::normalize_tag`) — `#Bug` 와 `bug` 를 다른
            // 것으로 세면 한쪽이 뺀 태그가 다른 쪽의 다른 표기로 되살아난다(`Issue::normalize`
            // 는 이 셈이 끝난 뒤에야 접으므로 여기서 접지 않으면 늦다). 막음 id 는 접지 않는다.
            let fold = |s: &str| match key {
                "tags" => crate::model::normalize_tag(s),
                _ => s.to_string(),
            };
            let set = |v: Option<&Value>| -> Option<BTreeSet<String>> {
                match v {
                    None => Some(BTreeSet::new()),
                    Some(v) => v.as_array()?.iter().map(|x| Some(fold(x.as_str()?))).collect(),
                }
            };
            let (o, a, b) = (set(o)?, set(a)?, set(b)?);
            let mut out = o.clone();
            out.extend(a.difference(&o).cloned());
            out.extend(b.difference(&o).cloned());
            for gone in o.difference(&a).chain(o.difference(&b)) {
                out.remove(gone);
            }
            let out: Vec<Value> = out.into_iter().map(Value::String).collect();
            Some((!out.is_empty()).then_some(Value::Array(out)))
        }
        // 마지막으로 쓴 때. 둘 다 썼으면 늦은 쪽이 참이다.
        //
        // `status_since` 도 여기다 — 여기 오는 것은 두 쪽의 **칸이 같은** 판뿐이고([`coupled`] 가
        // 칸이 갈린 판을 먼저 집어 간다), 그때 참은 그 칸에 마지막으로 든 때다. 이른 쪽을 고르면
        // 둘이 각자 닫은 줄에서 `status_since` 가 `done_at` 보다 앞서, 도구가 절대 안 쓰는 짝이 선다.
        "updated_at" | "done_at" | "status_since" => later(a, b, true),
        // **처음 때는 이른 쪽이다.** `started_at` 은 한 번 적고 안 덮는 값이고(moai-38mh),
        // `created_at` 이 갈린 것은 같은 id 를 두 번 세운 자국이라 이른 쪽이 그 줄의 나이다.
        "created_at" | "started_at" => later(a, b, false),
        _ => None,
    }
}

/// 두 시각 중 늦은/이른 쪽. 한쪽이 없으면 있는 쪽이다 — 없는 시각은 "모른다" 이지 "0" 이 아니다.
///
/// **둘 다 표준형이어야 고른다.** 사전순이 곧 시간순인 것은 `parse_rfc3339` 가 받는 스무 자
/// `…Z` 꼴에서만 참이다. 손으로 푼 줄이나 다른 도구가 적은 `+09:00` 을 바이트로 견주면
/// 실제로 늦은 쪽을 버린다(`12:00+09:00` 은 `04:00Z` 보다 이른 시각인데 바이트로는 뒤에 선다).
/// 글이 아닌 값도 같다 — 못 읽으면 말없이 고르지 않고 사람에게 넘긴다.
fn later(a: Option<&Value>, b: Option<&Value>, latest: bool) -> Option<Option<Value>> {
    fn read(v: Option<&Value>) -> Option<Option<&str>> {
        match v {
            None => Some(None),
            Some(v) => v.as_str().filter(|s| crate::model::parse_rfc3339(s).is_some()).map(Some),
        }
    }
    match (read(a)?, read(b)?) {
        (Some(x), Some(y)) => {
            let pick = if (x > y) == latest { x } else { y };
            Some(Some(Value::String(pick.to_string())))
        }
        (Some(x), None) | (None, Some(x)) => Some(Some(Value::String(x.to_string()))),
        (None, None) => Some(None),
    }
}

/// 표준형 시각만 초로. [`coupled`] 가 두 `planned_at` 중 늦은 쪽을 고를 때 쓴다.
fn time(v: Option<&Value>) -> Option<i64> {
    crate::model::parse_rfc3339(v?.as_str()?)
}

/// 설정에 적는 한 줄을 짓는다. **못 돌면 git 의 기본 머지로 내려앉는다.**
///
/// git 은 드라이버 명령을 `sh -c` 로 돌리고, **비영으로 끝난 것을 "충돌" 로 읽으면서 `%A` 를
/// 그대로 병합 결과로 삼는다.** 그래서 명령이 아예 못 돌면(적힌 경로가 사라졌다, PATH 에 없다,
/// 실행 권한이 없다) 파일은 이쪽 것 그대로인데 표식이 없고, 그것을 연 사람은 "이미 풀렸다" 로
/// 읽어 `git add` 한 번에 저쪽을 통째로 버린다. 안 심은 클론이 도리어 안전한 자리다 — 거기서는
/// git 의 기본 머지가 적어도 표식을 남긴다(`moai-w8so` 의 실측).
///
/// 그 두 상태를 같게 만든다. 가르는 자는 **드라이버가 `%A` 를 건드렸는가** 하나다.
///
/// 1. 드라이버가 0 으로 끝나면 그것으로 끝이다.
/// 2. 비영인데 `%A` 가 **달라졌으면** 답을 쓰고 나서 실패한 것 — 표식을 쓰고 사람에게 넘긴
///    갈래다(`run` 은 그 길에서만 비영으로 끝난다). 그대로 비영이다.
/// 3. 비영인데 `%A` 가 **받은 그대로면** 그 명령은 답을 안 쓴 것이다(경로가 사라졌다, PATH 에
///    없다, 실행 권한이 없다, 옛 바이너리라 이 명령을 모른다, 저쪽 파일을 못 읽었다) —
///    `git merge-file` 로 다시 합친다. 이 길은 표식을 남기므로, 최악이 "조용한 손실" 에서
///    "안 심은 클론과 같음" 으로 내려온다.
///
/// **`%A` 의 내용을 보고 가르지 않는다**(리뷰 moai-h6aq.cx8). 표식을 `grep` 으로 찾던 판은 두
/// 군데서 틀렸다. 하나, `.moai/issues.jsonl` 에 **이미** `<<<<<<<` 줄이 들어 있으면(못 읽는 줄은
/// `keyed` 가 그대로 들고 가므로 한 번 들어오면 오래 남는다) 명령이 아예 못 돈 판도 2번으로
/// 읽혀, 표식 없는 이쪽 파일이 그대로 남는 옛 버그가 되돌아온다. 둘, 저장소가
/// `conflict-marker-size` 를 7 보다 짧게 잡으면 드라이버가 쓴 표식을 못 찾아 3번으로 내려가고,
/// 그러면 `git merge-file` 이 **표식이 든 `%A`** 를 이쪽 것으로 알고 다시 합쳐 표식이 겹친다.
/// 앞뒤를 `cmp` 로 견주면 둘 다 안 생긴다 — 파일에 무엇이 들었는지가 아니라 이번 판이 무엇을
/// 했는지를 재기 때문이다.
///
/// 그 견줌이 서려면 `%A` 가 **받은 그대로**이거나 **다 쓴 답**이어야 한다. `run` 이 `%A` 를
/// `store::write_atomic` 으로 갈아끼우는 까닭이 그것이다 — 잘린 파일이 남으면 3번이 그것을
/// 이쪽 것으로 알고 합쳐, 지워진 줄이 "이쪽이 지웠다" 로 읽혀 **0 으로** 끝난다.
///
/// `--marker-size=%L` 을 준다. 안 주면 내려앉은 판만 일곱 자를 써서, 저장소가 고른 너비와
/// 드라이버가 쓰는 너비와 이것이 셋으로 갈린다.
///
/// 베낀 자리(`%A.ours`)는 어느 갈래에서나 지운다. `cp` 자체가 실패하면(그럴 자리가 거의 없지만)
/// `[ -f ]` 가 거짓이 되어 3번으로 간다 — 잃는 쪽이 아니라 **표식이 서는 쪽**으로 기운다.
///
/// 경로는 `shell_word` 로 감싼다 — 빈칸이 든 경로는 첫 낱말에서 끊기고, 그때 껍데기가 내는
/// 것은 1번도 2번도 아닌 3번이다. `%A %O %B` 는 git 이 제가 지은 임시 파일 이름으로 바꾸므로
/// 그대로 둔다. `%P` 는 git 이 이미 따옴표로 싸서 넣으니 덧싸지 않는다.
fn driver_command(cmd: &str) -> String {
    let q = crate::text::shell_word(cmd);
    format!(
        "cp %A %A.ours; \
         if {q} {SUB} %O %A %B %L %P; then rm -f %A.ours; exit 0; fi; \
         if [ -f %A.ours ] && ! cmp -s %A %A.ours; then rm -f %A.ours; exit 1; fi; \
         rm -f %A.ours; \
         exec git merge-file --marker-size=%L -L ours -L base -L theirs %A %O %B"
    )
}

/// 심은 줄이 앉는 설정 키. **적는 쪽과 재는 쪽이 한 글을 쓴다** — 따로 지으면 `merge.<이름>.*`
/// 를 손볼 때 한쪽만 안 고쳐져도 컴파일은 되고, 그때 알림만 조용해진다(리뷰 moai-h6aq.cx8).
fn driver_key() -> String {
    format!("merge.{DRIVER}.driver")
}

/// 심어 둔 드라이버가 **못 도는** 상태와, 심어야 하는데 **안 심은** 상태의 알림(moai-2ewr·moai-9khu).
///
/// 해로운 첫째는 **심어 놓고 그 명령이 못 도는 자리**다. 사람은 이슈마다 푸는 것이 돈다고 믿는데
/// 실제로는 `driver_command` 의 셋째 마디로 내려앉아 기본 머지가 돌고, 그 사실이 어느 화면에도
/// 안 선다. 이 저장소에서 그 자리는 가깝다 — `--install` 의 기본값은 지금 도는 바이너리의 절대
/// 경로이고, 워크트리에서 치면 그 워크트리의 `target/` 이 적히는데 `--local` 은 클론이 함께
/// 쓰는 자리라 그 워크트리를 지우는 순간 모든 체크아웃이 그 상태가 된다.
///
/// **안 심은 것도 말한다 — 저장소가 `merge=moai` 를 걸어 뒀을 때만**(moai-9khu, 2026-09-20 사용자
/// 결정). 앞 판은 입을 다물었다. `moai-w8so` 가 임시 저장소에서 둘을 나란히 재어 안 심은 클론이
/// 무해함을 보였기 때문이다 — `.gitattributes` 의 `merge=moai` 는 그냥 무시되고 git 의 기본 머지가
/// 돌며 표식도 선다. 그 실측은 그대로 서 있고 **갈래도 그대로 알림**이지만, 무해한 것과 말할 값이
/// 없는 것은 다르다: 클론마다 한 번 쳐야 하는 명령이라 새 사용자가 정확히 밟는 자리고, 이슈마다
/// 푸는 값을 잃는 줄 모르고 잃는다. 조르는 범위는 **저장소가 스스로 걸어 둔 선언**으로 좁힌다
/// ([`declared`]) — 드라이버를 안 쓰기로 한 저장소는 그 선언이 없으니 이 줄을 아예 안 본다. 선언을
/// 커밋해 두고 안 심기로 한 **클론**은 이 줄을 계속 보는데, 그 대가는 `gitattributes_rules` 와
/// `agents_stale` 이 이미 치르는 것과 같고 걷는 길은 한 줄이다.
///
/// **모르면 입을 다문다.** 설정을 못 읽었거나(저장소 밖이다, git 이 없다) 적힌 줄이 이 도구가
/// 지은 모양이 아니면 아무 말도 안 한다 — 남이 손으로 적은 줄을 "썩었다" 고 부르면 걷을 길이 없는
/// 알림이 선다. 안 심은 것과 못 읽은 것을 가르는 자는 `--default` 다: 맨 `--get` 은 키가 없을
/// 때도 비영으로 끝나, 저장소 밖과 안 심은 클론이 한 답으로 왔다.
///
/// **없는 키와 비워 둔 키도 가른다**([`UNSET`], 리뷰 moai-vbmn.spv). `--default ''` 로 받던 판은
/// 둘을 한 답으로 묶어, 손으로 비운 `driver =` 한 줄을 "안 심었다 — 병합이 기본 머지로
/// 내려앉는다" 로 읽었다. 그 상태는 내려앉지 않는다: git 이 빈 명령을 돌려 실패하고 **표식 없이
/// 이쪽 것만** 남긴다 — 이 도구가 막으려는 조용한 손실 그 자체라, 무해하다는 낱말을 붙일 수 없다.
/// 무엇이라 부를지는 아직 정한 낱말이 없으니 그 판은 **입을 다문다**(앞 판과 같은 자리다).
///
/// **이 저장소에 심은 줄만 본다**(`--local`, 리뷰 moai-h6aq.cx8). 맨 `--get` 은 system·global 까지
/// 훑어, 사람이 한때 `git config --global merge.moai.driver` 를 적어 뒀으면 심은 적 없는 저장소마다
/// — git 저장소가 아닌 `.moai` 자리까지 — 이 알림이 서고, 힌트를 따라 쳐도 그것은 `--local` 에
/// 적으니 영영 안 걷힌다. `install` 이 적는 자리가 `--local` 이므로 재는 자리도 거기다.
///
/// **선언이 넷 전부의 막이다**(moai-0j7c). 앞 판은 [`declared`] 로 `absent` 하나만 막아,
/// `.gitattributes` 에서 `merge=moai` 를 걷은 저장소에서도 rotten·alien·stale 이 그대로 섰다.
/// 거기서 병합은 어차피 늘 기본 머지였으니 "병합이 기본 머지로 내려앉는다" 가 공허하고, 걷는
/// 길은 `--uninstall` 이 없어 `.git/config` 를 손으로 여는 것뿐이다 — 걷을 길 없는 알림을
/// 안 세우는 자리가 여기다. **선언을 걷는 것이 진짜 탈출구가 된다**(moai-47bt 가 찾던 것).
/// 덤으로 안 쓰기로 한 저장소에서는 [`probe`] 가 프로세스를 아예 안 띄운다.
///
/// **[`Repo`] 를 통째로 받는다**(moai-8nkl). 자를 섞던 판은 여기 [`Repo::here`] 하나만 들어와,
/// 선언은 **워크트리의 가지**에서 재고 심은 줄은 클론 전체(`--local`)에서 쟀다. 합쳐질 트래커가
/// 사는 곳은 [`Repo::root`] 이므로 병합에서 실제로 걸리는 선언도 그쪽 것이다 — 루트가 `merge=moai`
/// 를 걸고 딸린 워크트리의 가지가 그것을 안 들면, 루트에서는 알림이 서고 워크트리 안에서는 같은
/// 명령이 아무 말도 안 했다. 이 저장소는 일을 모두 워크트리에서 하므로 세션이 보는 쪽이 늘 침묵이다.
///
/// 나머지 셋은 [`Repo::here`] 다. [`probe`] 는 git 이 드라이버를 **부르는** 자리에서 재야 하고,
/// `--local` 은 어느 체크아웃에서 물어도 같은 파일(`$GIT_COMMON_DIR/config`)이며, 힌트의 자리
/// ([`away_root`](crate::cmd::init::away_root))는 사람이 선 곳을 가리켜야 한다.
pub fn notice(repo: &Repo, chdir: bool) -> Option<crate::report::Warning> {
    notice_at(repo.here(), &repo.root, chdir)
}

/// [`notice`] 의 속 — 자를 둘로 받는다. 부르는 자리가 [`Repo`] 를 못 쥐는 `init --check` 만
/// 여기로 들어온다([`state_at`]).
fn notice_at(here: &Path, tracker: &Path, chdir: bool) -> Option<crate::report::Warning> {
    // **먼저 묻는다.** 뒤로 미루면 안 쓰기로 한 저장소에서도 설정을 읽고 `probe` 가 뜬다.
    declared(tracker).then(|| told(here, chdir)).flatten()
}

/// [`notice_at`] 에서 **선언을 이미 확인한 뒤**의 몸통 — 심긴 줄을 재어 할 말을 고른다.
///
/// [`state_at`] 도 여기로 든다. 갈라 둔 까닭은 그쪽이 "선언이 없다"(`off`)와 "할 말이
/// 없다"(`current`)를 **두 낱말로** 가려야 해서다 — [`notice_at`] 은 둘 다 `None` 이라 답만
/// 보고는 못 가린다. 재는 자리를 이렇게 나누지 않고 그쪽에서 [`declared`] 를 한 번 더 부르던
/// 판은 `moai init --check` 한 번에 `git check-attr` 를 두 번 띄웠고, "선언이 없다" 의 뜻이
/// 두 자리에 따로 적혀 손으로 맞춰야 했다.
fn told(here: &Path, chdir: bool) -> Option<crate::report::Warning> {
    /// 키가 없을 때 git 이 돌려줄 글. **값이 될 수 없는 것이라야 한다** — 심는 줄은 껍데기 명령이고
    /// 제어문자 하나만 든 줄은 그 무엇도 아니다.
    const UNSET: &str = "\u{1}";
    let root = here;
    let key = driver_key();
    let planted = match crate::git::run(root, &["config", "--local", "--get", "--default", UNSET, &key]) {
        Ok(v) => v,
        // **`--default` 가 없는 git 에서도 셋은 말한다**(리뷰 moai-vbmn.spv). 그 옵션은 2.18
        // 부터고, 없는 git 은 `unknown option` 으로 비영이라 `.ok()?` 가 알림 식구를 통째로
        // 삼켰다 — 이 에픽 전에는 `rotten` 과 `stale` 이 거기서도 돌았으니, 정확도를 더하려다
        // 있던 말을 잃는 꼴이다. 맨 `--get` 으로 한 번 더 묻는다: 키가 없을 때의 비영은
        // 예전처럼 입을 다무는 길이라, 옛 git 은 "안 심었다" 하나만 못 말하고 나머지는 산다.
        // 이 저장소가 판을 가정하지 않고 내려앉는 자리는 `worktree::table` 이 이미 그 꼴이다.
        Err(_) => crate::git::run(root, &["config", "--local", "--get", &key]).ok()?,
    };
    let planted = planted.trim();
    let away = crate::cmd::init::away_root(root, chdir);
    // 키는 있는데 비웠다 — 안 심은 것이 아니다. 위의 글이 까닭을 적는다.
    if planted.is_empty() {
        return None;
    }
    if planted == UNSET {
        // **"안 심었다" 는 어느 자리에도 없다는 말이다**(리뷰 moai-vbmn.spv). 심는 자리는
        // `--local` 이지만 git 이 드라이버를 **찾는** 자리는 system·global 까지고, `--global` 에
        // 한 번 적어 둔 사람의 병합에서는 실제로 그 줄이 돈다 — 잰 것이다. `--local` 만 보고
        // 조르면 "병합이 기본 머지로 내려앉는다" 가 통째로 거짓말이 되고, 힌트를 따라 쳐 봐야
        // 이미 도는 것을 한 번 더 적는 일이다.
        //
        // **넓게 묻는 것은 조르기 직전뿐이다.** 위의 `--local` 은 그대로다 — 넓은 자리의 줄은
        // 재지도 고치라고 하지도 않는다(남이 적은 줄을 "썩었다" 고 안 부르는 moai-h6aq.cx8 의
        // 규칙이 그 자리다). 여기서 넓은 자리는 **입을 다물 까닭**으로만 쓴다.
        return (!planted_anywhere(root)).then(|| crate::report::Warning::merge_driver_absent(away.as_deref()));
    }
    let word = planted_word(planted)?;
    match probe(root, &word) {
        // 그 **자리**가 못 돈다 — 비었거나, 권한이 없거나, 로더가 못 띄웠다(126·127).
        Probe::Dead => Some(crate::report::Warning::merge_driver_rotten(&word, away.as_deref())),
        // 돌기는 도는데 이 명령을 모른다 — **그 이름의 다른 도구**가 그 자리에 있다.
        Probe::Alien => Some(crate::report::Warning::merge_driver_alien(&word, away.as_deref())),
        // **못 잰 것은 말하지 않는다**(리뷰 moai-vbmn.spv). 기계가 막았거나 제 시간에 안 끝났거나
        // 신호에 맞아 죽은 판이다 — 무엇이 어긋났는지 말할 수 없으니 `모르면 입을 다문다` 다.
        Probe::Unknown => None,
        // **줄의 모양도 본다**(moai-h54i). 명령이 도는 것과 그 줄이 지금 판인 것은 다른 말이다 —
        // 내려앉는 마디가 없던 판에 심은 클론은 그 마디 없이 그대로 돌고, 적힌 경로가 사라지는 날
        // 표식 없이 저쪽을 버린다. 다시 심는 것은 사람이 치는 `--install` 하나뿐이고 설정은
        // 커밋되지 않으니, 말하지 않으면 그 클론은 영영 옛 줄을 든다.
        //
        // **고칠 명령은 같은 명령으로 다시 심는다**(`--as`). 맨 `--install` 은 지금 도는 바이너리로
        // 바꿔 적는데, 그것이 워크트리의 `target/` 이면 고치라는 말이 도리어 썩은 자리를 심는다.
        Probe::Runs => (planted != driver_command(&word))
            .then(|| crate::report::Warning::merge_driver_stale(&word, away.as_deref())),
    }
}

/// 어느 자리에든 이 드라이버가 적혀 있는가 — system·global·local 을 **git 이 푸는 대로.**
///
/// [`notice`] 가 "안 심었다" 를 말하기 직전에만 묻는다(리뷰 moai-vbmn.spv). 심는 자리(`--local`)와
/// git 이 **찾는** 자리는 다르고, "없다" 는 말은 뒤의 것이라야 참이다. 못 물어봤으면 **있다고
/// 친다** — 위의 셋과 반대로 기우는 자리다. 여기서 틀리는 값은 조르지 않는 쪽이라야 한다.
///
/// **`GIT_CONFIG_GLOBAL`·`GIT_CONFIG_SYSTEM` 으로 옮긴 자리도 본다**(moai-b5np). 앞 판은 그 둘을
/// 걷고 물어(`git_leaks::REPO`), 그것으로 전역 설정을 딴 파일에 둔 사람에게 **실제로 도는**
/// 드라이버를 "안 심었다" 고 했다 — dotfile 관리기·CI 이미지·컨테이너 래퍼가 밟는 자리다.
///
/// **넓히는 것은 이 물음 하나다**([`crate::git::run_reading_user_config`]). `GIT_DIR` 무리는 그대로
/// 걷는다: 훅이 준 환경이 어느 저장소를 여는지를 바꾸지 못하게 하는 것이 걷기가 선 까닭이고,
/// `tests/cli.rs` 의 격리는 그 셋을 `/dev/null` 로 채워 두어 그대로 선다.
///
/// **`--show-scope` 로 한 번에 받지 않는다**(moai-433r, 2026-09-21 사용자 결정). 그 옵션은 값과
/// 자리를 한 번에 내어 이 부름을 [`notice`] 의 첫 물음에 합칠 수 있지만, git 2.26 부터라 지금
/// 바닥(`--default`, 2.18)을 올린다. **줄이는 부름은 조르기 직전 한 번뿐이다** — 여기는
/// `--local` 이 비었을 때만 돌고, 그 갈래는 어차피 [`probe`] 를 안 띄운다. 바닥을 올리거나
/// 길을 하나 더 두는 값이 그보다 크다.
fn planted_anywhere(root: &Path) -> bool {
    crate::git::run_reading_user_config(root, &["config", "--get", "--default", "", &driver_key()])
        .map_or(true, |v| !v.trim().is_empty())
}

/// 이 저장소가 스냅샷에 `merge=moai` 를 걸어 뒀는가 — [`notice`] 네 갈래 전부의 막이다.
///
/// **딸린 파일을 손으로 읽지 않는다.** 규칙은 `.gitattributes` 하나에만 있는 것이 아니다 —
/// `.git/info/attributes` 와 위 디렉터리의 파일, 그리고 패턴끼리의 우선순위까지 git 의 규칙이다.
/// 그것을 여기 다시 적으면 git 이 실제로 무엇을 쓰는지와 이 판정이 갈리고, 갈리는 쪽은 늘
/// 이쪽이다. `check-attr` 에게 물으면 git 이 제 규칙으로 답한다.
///
/// `-z` 로 받는다 — 맨 출력은 `<경로>: merge: <값>` 이라 경로에 `: ` 가 들면 자를 자리가 갈린다.
/// 못 물어봤으면(저장소 밖이다, git 이 없다) **거짓이다**: 모르면 입을 다문다.
fn declared(root: &Path) -> bool {
    let Ok(out) = crate::git::run(root, &["check-attr", "-z", "merge", "--", SNAPSHOT]) else {
        return false;
    };
    let mut f = out.split('\0');
    f.next() == Some(SNAPSHOT) && f.next() == Some("merge") && f.next() == Some(DRIVER)
}

/// 심어 둔 줄에서 **실제로 부르는 명령**을 떼어 낸다. 모양이 이 도구가 지은 것이 아니면 `None`.
///
/// **표식은 `" {SUB} %O %A %B"` 다** — 이 도구가 지은 줄은 명령 바로 뒤에 그것이 선다. 앞에 무엇이 붙든
/// (지금은 `cp %A %A.ours; if `, 옛 판은 아무것도 없다) 그 자리 앞의 낱말 하나가 명령이다.
/// 재는 쪽이 앞머리를 세지 않으니 [`driver_command`] 를 고쳐도 여기가 안 따라 낡는다.
///
/// **표식이 없으면 재지 않는다**(리뷰 moai-h6aq.cx8). 앞 판은 낱말 하나만 떼어, 사람이 손으로
/// 적은 `python3 tools/merge.py %O %A %B` 를 `python3` 으로 읽고 그것이 이 프로세스의 PATH 에
/// 없으면 "썩었다" 고 불렀다 — 남의 줄을 덮으라는 힌트가 함께 서는데 걷을 길이 없다.
///
/// 빈칸이 든 경로는 `shell_word` 가 홑따옴표로 싸는데, 그 안에 홑따옴표가 또 들면
/// (`'/a/it'\''s'`) 떼어 낸 조각이 실제 경로가 아니다 — 그때는 재지 않는다. `$'…'` 도 같다.
fn planted_word(planted: &str) -> Option<String> {
    // 심은 줄에서 명령 바로 뒤에 서는 글. 이 도구가 지은 줄인지를 이것으로 가른다.
    let call = format!(" {SUB} %O %A %B");
    let head = planted.split_once(call.as_str())?.0;
    let word = match head.strip_suffix('\'') {
        Some(inner) => {
            let start = inner.rfind('\'')?;
            // 이어 붙인 따옴표(`'…'\''…'`)와 `$'…'` 은 앞이 빈칸으로 안 끝난다. `$'…'` 을
            // 풀지 않는 것은 푸는 규칙을 여기 또 쓰면 `shell_word` 와 둘이 어긋나서다.
            let (before, word) = (&inner[..start], &inner[start + 1..]);
            if !before.is_empty() && !before.ends_with(' ') {
                return None;
            }
            word
        }
        None => head.rsplit(' ').next()?,
    };
    (!word.is_empty()).then(|| word.to_string())
}

/// 심어 둔 명령을 실제로 불러 본 결과.
///
/// **셋을 가른다.** 고칠 명령이 비슷해도 무엇이 어긋났는지가 다르고, 뭉치면 그 중 하나는 반드시
/// 거짓말이 된다(`agents_stale` 을 둘로 가른 것과 같은 까닭).
enum Probe {
    /// 이 명령을 알고 0 으로 끝났다 — 도움말이 제 이름을 댄다([`Said`]).
    Runs,
    /// 돌기는 도는데 `merge-driver` 를 모른다 — 그 이름의 **다른 도구**가 그 자리에 있거나,
    /// 모르는 인자를 흘려 듣고 0 을 내는 래퍼다(moai-wwbi).
    Alien,
    /// 띄우지도 못했다 — 자리가 비었거나, 권한이 없다.
    Dead,
    /// **재지 못했다**(리뷰 moai-vbmn.spv). 기계가 막았거나(ETXTBSY·메모리·fd), 제 시간에 안
    /// 끝났거나, 신호에 맞아 죽었다 — 그 바이너리가 무엇인지 아무것도 말해 주지 않는 판이다.
    ///
    /// **넷째가 필요한 까닭.** 이것을 `Dead` 나 `Alien` 에 섞으면 그 낱말이 거짓말이 된다. 이
    /// 저장소에서 심는 자리는 대개 `target/release/moai` 인데, 3~7분짜리 LTO 링크가 그 자리를
    /// 다시 쓰는 동안 exec 은 ETXTBSY 로 막히고(멀쩡한 자리다), 16GiB cgroup 이 찬 날에는
    /// OOM 킬러가 자식을 SIGKILL 한다(멀쩡한 바이너리다). 둘 다 "안 돈다"·"딴 도구가 선다" 로
    /// 서면 사람이 없는 문제를 찾으러 간다.
    Unknown,
}

/// 재는 부름을 기다리는 한도. 넘기면 죽이고 [`Probe::Unknown`] 이다.
///
/// **알림 하나 때문에 `moai status` 가 멈추지 않는다.** 여기 도는 것은 이 도구가 안 쓴 명령이고,
/// `moai status` 는 세션이 여는 화면이자 훅의 `UserPromptSubmit` 보드다(`cmd/hook.rs`) — 그 명령
/// 하나가 잠들면 사람의 다음 프롬프트가 글자 한 줄 없이 함께 잠든다. `--help` 는 이 기계에서
/// 9ms 였으니 2초는 느린 기계와 찬 캐시에도 넉넉하고, 넘긴 판은 **어차피 말할 것이 없다.**
const PROBE_BUDGET: std::time::Duration = std::time::Duration::from_secs(2);

/// 그 명령이 이 드라이버를 아는가 — **파일을 보지 않고 실제로 불러서 잰다**(moai-zdw4,
/// 2026-09-20 사용자 결정).
///
/// 앞 판은 `metadata` 로 "그 자리에 실행할 수 있는 파일이 있는가" 만 봤고, 그래서 둘을 놓쳤다.
///
/// - `--as moai` 로 심었는데 그 이름의 **다른 도구**가 PATH 에 선 판. 파일도 있고 실행 권한도
///   있으니 조용한데 그 명령은 `merge-driver` 를 모른다 — 매 병합이 말없이 내려앉는다. 이름을
///   `moai` 그대로 두고 배포하기로 한 뒤(2026-09-20) 받는 쪽에서 가까워진 자리다
/// - `mode & 0o111` 은 "아무나 돌릴 수 있는가" 지 "내가 돌릴 수 있는가" 가 아니다. 남의 소유
///   0o700 이나 `noexec` 에 얹힌 파일은 통과하는데 껍데기는 126 을 낸다
///
/// 부르는 것은 `--help` 다. **답을 안 바꾸는 부름이라야 한다** — 재자고 병합을 돌릴 수는 없고,
/// 이 도구의 `--help` 는 어느 갈래에서도 파일을 안 건드린다.
///
/// **끝난 꼴과 함께 stdout 도 본다**(moai-wwbi, 2026-09-21 사용자 결정). 앞 판이 잰 것은 "0 으로
/// 끝났는가" 뿐이라, 모르는 인자를 흘려 듣고 0 을 내는 래퍼·busybox 꼴 디스패처가 그대로
/// 지나갔다 — `#!/bin/sh` 와 `exit 0` 두 줄짜리를 `--as` 로 심으니 `moai status` 가 조용했고,
/// 그 저장소의 매 병합은 말없이 내려앉는다. 도움말이 제 이름을 대는지까지 본다([`Said`]).
/// 그만큼 심는 줄의 규약이 하나 늘고(`merge-driver --help` 에 그 낱말이 서야 한다), 매는 자는
/// `the_planted_command_must_name_the_subcommand_in_its_help` 다.
///
/// **값은 프로세스 하나**다 — `moai status` 한 번에 릴리스로 4ms 쯤, 워크트리의 dev 바이너리를
/// 심었으면 9~11ms 다(2026-09-20 이 기계에서 잰 값). **어느 저장소냐로 비율이 갈린다**: 이슈와
/// 워크트리가 쌓인 이 저장소의 `status` 는 90~126ms 라 10% 아래지만, 이 알림이 겨누는 **새
/// 클론**의 `status` 는 13ms 라 같은 부름이 30% 다(리뷰 moai-vbmn.spv 의 실측 — 앞 글은 7% 만
/// 적어 겨누는 쪽을 빠뜨렸다). 훅의 보드는 세션당 한 번이고(`once_per_session`) 한눈 보기는
/// 이것을 아예 안 부른다. 그만큼을 내고 사는 것은 "심었는데 안 돈다" 를 **틀리게 말하지 않는
/// 것**이다.
///
/// **표준 입력을 끊는다.** 물려주면 그 명령이 stdin 을 읽는 순간 `moai status` 가 사람의 터미널을
/// 붙들고 영영 안 끝난다. 표준 오류는 버리고, 표준 출력은 **파일로 받는다**([`Said`]) — 사람의
/// 화면에는 안 나가되 도움말은 읽어야 하고, 파이프로 받으면 한도가 새기 때문이다.
///
/// **그래도 시간을 잰다**([`PROBE_BUDGET`], 리뷰 moai-vbmn.spv). 입을 끊는 것은 막히는 길 하나를
/// 막을 뿐이다 — `sleep`·락 대기·시작할 때 망을 한 번 타는 래퍼는 stdin 과 무관하게 잠든다.
/// 잰 판: `--as` 로 `sleep 30` 짜리를 심으면 `moai status` 가 글자 한 줄 없이 영영 안 끝났다.
/// 앞 판의 `metadata` 는 잠들 길이 거의 없었으니 여기가 **새로 생긴 자리**고, 알림 하나가 세션이
/// 여는 화면을 붙드는 것은 값이 맞지 않는다.
///
/// **한도가 덮는 것은 기다림이지 띄우기가 아니다**(리뷰 moai-vbmn.spv가 짚었다). 시계는
/// [`reaped`] 안에서 시작하는데 `spawn` 은 그 앞에 있고, 죽은 NFS·autofs 에 얹힌 파일은 `spawn`
/// 안에서 멈춘다 — 그 한 갈래는 여전히 안 덮인다. 덮으려면 띄우는 일까지 딴 실로 보내야 하고,
/// 그것은 알림 하나에 실을 하나 더 다는 값이라 여기서는 안 한다. **죽이는 것도 바로 아래
/// 자식까지다** — 껍데기 래퍼를 심었으면 그 안의 일은 한도를 넘어도 계속 돈다. 프로세스 그룹을
/// 만들어 한꺼번에 죽이려면 `libc` 가 드는데, 이 저장소는 그 무게를 이 알림에 안 낸다.
///
/// **그 둘은 열어 두기로 정했다**(moai-2k61, 2026-09-21 사용자 결정). 실을 하나 다는 값도,
/// 이 저장소에 없던 의존을 알림 하나 때문에 들이는 값도 지금 막는 것보다 크다 — 머리 기사인
/// 멈춤(`moai status` 가 영영 안 끝나던 것)은 [`PROBE_BUDGET`] 이 이미 막았고, 남은 둘은 죽은
/// NFS 에 얹힌 파일과 래퍼의 손자다. **여는 조건은 그 둘이 실제로 사람을 붙드는 것이 보일
/// 때다** — 다시 재는 사람이 여기서부터 읽는다.
///
/// **그 결정의 전제 하나는 그 뒤에 바뀌었다**(moai-dhx9, 2026-09-23) — `libc` 가 유닉스에서
/// 직접 의존으로 섰다(`Cargo.toml` 의 `[target.'cfg(unix)'.dependencies]`, 거기 측정값이 적혀
/// 있다). 그러니 다시 열 때 견줄 것은 "없던 의존을 들이는 비용" 이 아니라 실을 하나 다는 비용
/// 하나다. 결정 자체는 그대로 선다 — 바뀐 것은 값을 매기는 셈이다.
///
/// **그 결정은 "한도가 아이를 잰다" 위에 선다**(리뷰). 도움말을 파이프로 받던 판은 아이를 거둔
/// 뒤에 그 파이프를 읽어, 손자가 쓰기 끝을 쥐고 있으면 한도 **밖에서** 영영 멈췄다 — 위 문단이
/// "이미 막았다" 고 적은 바로 그 멈춤이 손자 하나로 되살아난다. 그래서 도움말은 [`Said`] 로
/// 받는다: 받는 자리를 파일로 두면 손자는 다시 "새는 프로세스" 일 뿐 멈춤이 아니다.
///
/// **가르는 것은 넷이다.** 못 띄운 까닭과 끝난 꼴을 둘 다 본다 — 뭉치면 그 중 하나는 반드시
/// 거짓말이 된다.
///
/// | 본 것 | 갈래 |
/// |---|---|
/// | 0 으로 끝났고 도움말이 제 이름을 댄다 | `Runs` |
/// | 0 으로 끝났는데 제 이름을 안 댄다 — 모르는 인자를 흘려 듣는 래퍼다 | `Alien` |
/// | 자리가 없다·권한이 없다 | `Dead` |
/// | 126·127 로 끝났다 — 껍데기와 로더가 "못 돌렸다" 고 낼 때 쓰는 값이다 | `Dead` |
/// | 그 밖의 비영 | `Alien` |
/// | 그 밖의 못 띄움·시간 넘김·신호에 맞아 죽음 | `Unknown`(입을 다문다) |
///
/// **`ENOEXEC` 가 `Unknown` 인 것은 일부러다.** git 은 심은 줄을 `sh -c` 로 돌리고, 껍데기는
/// 샤뱅 없는 파일을 제 스크립트로 읽어 **멀쩡히 돌린다** — 여기서 곧바로 exec 하는 이쪽만 막힌다.
/// 그것을 `Dead` 로 읽으면 git 이 잘 돌리는 래퍼를 "안 돈다" 고 부르게 되고, 걷을 길이 없다.
/// 깨진 ELF 도 같은 errno 라 함께 입을 다무는데, 못 가르는 둘 중 **조르지 않는 쪽**으로 기운다.
///
/// **상대 경로는 `root` 에 붙인다**(리뷰 moai-h6aq.cx8). git 은 드라이버를 그 워크트리 꼭대기에서
/// 돌리는데, 프로세스의 현재 자리로 풀던 판은 저장소의 아래 디렉터리에서 친 `moai status` 가
/// 멀쩡한 `--as bin/moai` 를 "썩었다" 고 불렀다. 절대 경로면 `join` 이 그대로 낸다. 자리에
/// 구분자가 없으면 이름이므로 `PATH` 에 맡긴다 — 그 `PATH` 는 이 프로세스의 것이고 git 이 병합에서
/// 쓸 것과 꼭 같지는 않지만, 이름으로 심은 것은 그만큼만 믿는 것이 맞다.
fn probe(root: &Path, cmd: &str) -> Probe {
    // **유닉스가 아니면 재지 않는다**(리뷰 moai-vbmn.spv 가 짚은, 조용히 사라진 결정). 앞 판은
    // `\` 와 `PATHEXT` 규칙을 여기 다시 적지 않으려고 입을 다물었고, 부르는 쪽으로 옮겨도 그
    // 까닭은 그대로다 — 러스트의 `Command` 는 이름에 `.exe` 만 붙이는데 git 은 심은 줄을
    // `sh -c` 로 돌려 `moai.cmd` 도 찾아낸다. 그 판을 `Dead` 로 읽으면 git 이 잘 부르는 드라이버에
    // 걷을 길 없는 알림이 선다. **줄의 모양은 그대로 본다** — 여기서 지나가는 것은 "돌 수
    // 있는가" 하나뿐이고, 그것이 이 에픽 전에 `runnable` 이 하던 일이다.
    #[cfg(not(unix))]
    {
        let _ = (root, cmd);
        return Probe::Runs;
    }
    #[cfg(unix)]
    {
        use std::io::ErrorKind;
        use std::process::{Command, Stdio};
        let has_dir = cmd.contains('/') || cmd.contains(std::path::MAIN_SEPARATOR);
        let program = if has_dir { root.join(cmd) } else { std::path::PathBuf::from(cmd) };
        // **못 열면 안 잰다** — 도움말을 못 받아 놓고 "제 이름을 안 댔다" 로 읽으면 멀쩡한
        // 드라이버가 `Alien` 이 된다. 못 잰 것은 말하지 않는 자리다.
        let Some((said, sink)) = Said::new() else { return Probe::Unknown };
        let spawned = Command::new(program)
            .args([SUB, "--help"])
            .current_dir(root)
            .stdin(Stdio::null())
            .stdout(Stdio::from(sink))
            .stderr(Stdio::null())
            .spawn();
        let mut child = match spawned {
            Ok(c) => c,
            // 이 둘만 그 **자리**에 대한 말이다.
            Err(e) if matches!(e.kind(), ErrorKind::NotFound | ErrorKind::PermissionDenied) => return Probe::Dead,
            // 나머지는 기계가 막은 것이다 — 멀쩡한 자리를 "썩었다" 고 부르지 않는다.
            Err(_) => return Probe::Unknown,
        };
        let Some(st) = reaped(&mut child) else { return Probe::Unknown };
        match st.code() {
            // **0 으로 끝난 것만으로는 모자라다**(moai-wwbi, 2026-09-21 사용자 결정). 모르는 인자를
            // 흘려 듣고 0 을 내는 래퍼(`#!/bin/sh` 두 줄짜리)는 `merge-driver` 를 모른 채 그대로
            // 지나갔고, 그 저장소의 매 병합이 말없이 내려앉는다. 도움말에 제 이름이 서는지까지 본다.
            Some(0) if said.read().contains(SUB) => Probe::Runs,
            Some(0) => Probe::Alien,
            // 껍데기와 로더의 낱말이다 — 공유 라이브러리를 못 찾은 판, exec 에 막힌 래퍼. 그 이름의
            // 딴 도구가 아니라 그 자리가 못 도는 것이다.
            Some(126 | 127) => Probe::Dead,
            Some(_) => Probe::Alien,
            // 신호에 맞아 죽었다(유닉스에서만 선다) — OOM 킬러가 가장 흔하다.
            None => Probe::Unknown,
        }
    }
}

/// 재는 동안 아이가 쓴 stdout 을 받아 둘 자리 — **파이프가 아니라 파일이다**(리뷰).
///
/// 도움말이 제 이름을 대는지를 보려면([`probe`], moai-wwbi) 아이의 stdout 을 읽어야 하는데,
/// 파이프로 받으면 [`PROBE_BUDGET`] 이 **더는 이 부름의 상한이 아니다**. 둘이 샌다.
///
/// - 아이가 파이프 한 통(리눅스 64KiB)을 채우면 쓰기에서 막힌다. [`reaped`] 는 그동안 아이만
///   보고 있으니 한도를 다 쓰고 죽인 뒤 `Unknown` 을 내 — 멀쩡한 드라이버가 매 `moai status`
///   마다 2초를 먹고 알림은 통째로 사라진다.
/// - 더 나쁜 쪽: 아이가 쓰기 끝을 물려준 **손자**가 살아 있으면 아이를 거둔 뒤의 `read_to_end`
///   가 영영 안 끝난다. 한도는 아이까지만 재므로(moai-2k61 이 열어 둔 그 구멍) 그 멈춤을
///   아무것도 안 막는다 — 껍데기 shim 이 뒤에 일을 하나 띄우는 것은 흔한 모양이다.
///
/// 파일로 받으면 쓰는 쪽이 안 막히고, 읽는 쪽은 손자를 안 기다린다. 한도는 다시 [`reaped`]
/// 하나가 쥔다. **값은 자리 하나 열고 지우는 것**이라 프로세스 하나 띄우는 값에 묻힌다.
struct Said(std::path::PathBuf);

impl Said {
    /// 자리를 하나 열어 **읽을 쪽과 아이에게 물려줄 쪽**을 함께 낸다. 못 열면 `None`.
    fn new() -> Option<(Said, std::fs::File)> {
        use std::sync::atomic::{AtomicU64, Ordering};
        // 한 판에서 여러 번 잰다(`chosen_command` 가 `install` 과 `plant_for_init` 에서).
        // pid 만으로는 그 둘이 같은 자리를 쓴다.
        static NTH: AtomicU64 = AtomicU64::new(0);
        let at = std::env::temp_dir().join(format!(
            "moai-probe.{}.{}",
            std::process::id(),
            NTH.fetch_add(1, Ordering::Relaxed)
        ));
        let file = std::fs::File::create(&at).ok()?;
        Some((Said(at), file))
    }

    /// 아이가 쓴 글 — **못 읽은 판은 안 본 것과 같다**(빈 글). 0 으로 끝났는데 제 이름을 못
    /// 댄 명령이고, 그 자리의 낱말은 `Alien` 이다.
    fn read(&self) -> String {
        std::fs::read(&self.0).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default()
    }
}

impl Drop for Said {
    /// **재고 나면 치운다.** 손자가 아직 그 fd 를 쥐고 있어도 유닉스에서는 지워진다.
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// [`PROBE_BUDGET`] 안에 끝나면 그 끝을, 아니면 죽이고 거둔 뒤 `None`.
///
/// **거두고 간다.** 죽이기만 하고 두면 `moai status` 가 좀비를 남긴 채 끝난다.
///
/// **기다리는 칸을 늘려 간다**(리뷰 moai-vbmn.spv). 한 칸을 2ms 로 못박던 판은 3.8ms 에 끝나는
/// 부름을 다음 2ms 자리까지 올림해, 잰 값이 판마다 2~5ms 씩 늘었다 — 자는 동안 끝난 것을
/// 모르고 더 자기 때문이다. 0.2ms 에서 시작해 갑절로 늘리면 빠른 판은 거의 안 자고, 느린 판은
/// 10ms 칸으로 자 [`PROBE_BUDGET`] 까지 깨는 횟수가 이백 번을 안 넘는다.
fn reaped(child: &mut std::process::Child) -> Option<std::process::ExitStatus> {
    use std::time::{Duration, Instant};
    const FLOOR: Duration = Duration::from_micros(200);
    const CEIL: Duration = Duration::from_millis(10);
    let deadline = Instant::now() + PROBE_BUDGET;
    let mut nap = FLOOR;
    loop {
        match child.try_wait() {
            Ok(Some(st)) => return Some(st),
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(nap);
                nap = (nap * 2).min(CEIL);
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

/// 아무도 안 주면 심을 명령 — **`PATH` 의 `moai` 가 같은 판이면 그쪽, 아니면 지금 이
/// 바이너리**(moai-bq6w, 2026-09-21 사용자 결정).
///
/// 심는 자리(`--local`)는 클론이 함께 쓰는 `$GIT_COMMON_DIR/config` 인데, 이 저장소의 절차는
/// 일을 모두 워크트리에서 하므로 지금 도는 바이너리는 대개 `<워크트리>/target/release/moai` 다.
/// 그 워크트리를 지우는 날 **모든 체크아웃**이 죽은 경로를 든다 — "한 번도 안 심었다" 가
/// "전부 썩었다" 로 바뀌는 자리고, `merge_driver_absent` 의 힌트를 그대로 친 사람이 밟는다.
///
/// **고르는 자리가 하나다.** 힌트에 경고를 적는 길은 버렸다 — 에이전트는 명령만 읽고 친다.
/// 거절하는 길도 버렸다: `--as` 로 제 경로를 주는 사람까지 막고 게이트가 하나 는다.
///
/// **"같은 판" 은 바이트가 같은 것이다**(리뷰 moai-t6z9.bd6 9번). `--version` 으로 재던 판은
/// 헛돌았다 — 이 크레이트의 판은 온 이력에서 `0.1.0` 하나라, `~/.local/bin/moai` 에 깔린 **옛
/// 빌드**가 같은 글을 내고 그대로 심겼다. 그러면 병합 알고리즘을 고친 사람이 `init` 을 친 뒤로
/// 모든 병합이 **옛 알고리즘**으로 합쳐지는데 `status` 와 `init --check` 는 둘 다 "제대로 섰다"
/// 고 말한다(이 컨테이너에서 두 번 재현했다). 판을 가릴 자가 글에 없으니 파일로 가른다: 크기가
/// 다르면 거기서 끝이고, 같으면 바이트를 견준다. 심는 명령은 한 번 치는 것이라 그 값이 싸다.
///
/// **그래서 감싼 스크립트는 안 고른다.** `exec <진짜>` 하는 셸 래퍼는 바이트가 다르니 지금
/// 바이너리가 심긴다 — 틀리는 값은 **덜 이르는 쪽**이라야 하고, 그쪽은 이 워크트리를 지울 때만
/// 썩지만 앞쪽은 조용히 옛 알고리즘으로 합친다.
///
/// 바이트가 같아도 [`probe`] 가 `Runs` 라야 한다 — 그 자리가 실제로 돌아야 심는 뜻이 있다.
fn chosen_command(here: &Path) -> Result<String, String> {
    // **까닭은 자료로 낸다** — 부르는 두 자리(`install` 과 `plant_for_init`)가 저마다 제 말로
    // 편다(moai-uzgp). 여기서 글을 지으면 이 함수가 화면 말을 알아야 한다.
    let mine = std::env::current_exe().map_err(|e| e.to_string())?;
    let Some(found) = on_path(DRIVER) else { return Ok(mine.display().to_string()) };
    // 같은 파일이면 고를 것이 없다 — 이름으로 적으면 git 이 병합에서 쓸 `PATH` 에 기대는 것이
    // 하나 늘 뿐이다.
    //
    // **철자가 아니라 자리로 견준다**(리뷰). `current_exe` 는 리눅스에서 `/proc/self/exe` 라
    // 링크가 다 풀린 값인데 `PATH` 의 철자는 안 풀린 값이다 — `/usr/local/bin/moai` 가 같은
    // 파일을 가리키는 심볼릭 링크면 둘이 갈려, 같은 바이너리를 두 번 띄워 재고도 "딴것" 으로
    // 읽었다. 못 풀면 적힌 철자 그대로 견준다: 여기서 틀리는 값은 **덜 이르는 쪽**이다.
    // 푸는 자는 [`crate::store::real`] 하나다(moai-8csx) — 이 줄도 그 다섯과 같은 꼴이었는데
    // 목록에서 빠져, 나중에 정할 것(윈도의 `\\?\` 접두어)이 여기만 옛 답으로 남았다(리뷰).
    if crate::store::real(&found) == crate::store::real(&mine) {
        return Ok(mine.display().to_string());
    }
    let word = found.display().to_string();
    if same_bytes(&found, &mine) && matches!(probe(here, &word), Probe::Runs) {
        Ok(word)
    } else {
        Ok(mine.display().to_string())
    }
}

/// 두 파일이 **바이트째 같은가** — 못 읽으면 거짓이다(모르면 덜 이르는 쪽).
///
/// 크기를 먼저 본다: 다른 빌드는 거의 늘 크기가 다르고, 그때는 5MB 를 읽지 않는다.
fn same_bytes(a: &Path, b: &Path) -> bool {
    let size = |p: &Path| std::fs::metadata(p).ok().map(|m| m.len());
    match (size(a), size(b)) {
        (Some(x), Some(y)) if x == y => std::fs::read(a).ok().zip(std::fs::read(b).ok()).is_some_and(|(x, y)| x == y),
        _ => false,
    }
}

// **`--version` 으로 재던 자는 없앴다**(리뷰 moai-t6z9.bd6 9번). 이 크레이트의 판은 온 이력에서
// `0.1.0` 하나라 그 물음이 두 빌드를 못 갈랐고, 갈라야 할 바로 그 자리에서 옛 빌드를 "같은 판"
// 으로 읽었다. 지금 가르는 자는 [`same_bytes`] 다 — 부름 하나와 그 한도도 함께 없어졌다.

/// `PATH` 에서 그 이름의 **돌릴 수 있는 파일** 첫 자리. 껍데기가 찾는 차례를 그대로 따른다.
///
/// **디렉터리는 건너뛴다** — `moai` 라는 디렉터리가 앞자리에 있으면 껍데기도 그것을 안 쓴다.
///
/// **절대 경로인 자리만 본다**(리뷰). 여기서 고른 값은 `.git/config` 에 그대로 앉아 **병합
/// 때** 풀리는데, 그 자리는 이 프로세스가 선 곳이 아니라 워크트리 꼭대기다. POSIX 는 `PATH` 의
/// 빈 자리(`/usr/bin:` 의 끝, `:` 가 둘 붙은 가운데)를 "지금 자리" 로 읽으므로 그대로 이으면
/// `Path::new("").join("moai")` 가 **맨 `moai`** 가 된다 — 이 저장소가 전역 설치를 안 해 맨
/// 이름으로 심으면 안 된다고 적어 둔 바로 그 값이고(`--install` 의 도움말), 상대 자리(`.`·
/// `bin`)는 병합에서 딴 파일로 풀린다. 껍데기가 그 자리를 쓰는 것과 **심어 둘 값으로 쓰는 것**은
/// 다른 물음이다.
///
/// **돌릴 수 있는가는 [`super::runnable`] 하나가 답한다**(moai-p3kb, 리뷰). 여기 있던 셋째 벌은
/// `skill`·`tui` 의 것과 글자째 같았는데 모으는 판에서 빠져, 이식성 고침 하나가 저 둘에만 들 뻔했다.
/// 빈 자리를 거르는 **위의 한 줄이 이 자리만의 것**이고, 그것은 재는 자가 아니라 고르는 자에 붙는다.
fn on_path(name: &str) -> Option<std::path::PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).filter(|d| d.is_absolute()).map(|d| d.join(name)).find(|p| super::runnable(p))
}

/// 설정의 세 줄을 적는다 — 심은 드라이버 줄을 낸다.
///
/// **속 병합에는 이 드라이버를 쓰지 않는다**(`merge.<이름>.recursive`). 갈래가 엇갈린
/// 이력에서 git 은 공통 조상 여럿을 먼저 합쳐 가상 조상을 짓는데, 그 자리에 이 드라이버를
/// 쓰면 충돌 표식이 `%O` 로 들어와 다음 판에서 못 읽는 줄이 된다 — 그러면 이슈마다 푼 것이
/// 통째로 날아가고 사람이 파일 전체를 손으로 푼다. 세션 여럿이 `develop` 을 서로 받는
/// 저장소에서 엇갈린 이력은 드문 일이 아니다. `binary` 는 표식을 안 남기고 이쪽 것을
/// 그대로 두어 `%O` 가 계속 JSONL 로 읽힌다.
///
/// **저장소 지역 설정에 적는다.** 사람의 전역 설정에 남기면 이 저장소를 지운 뒤에도
/// 없는 바이너리를 가리키는 줄이 남는다. 자리는 부르는 쪽이 준다 — git 이 제 규칙으로
/// 저장소를 찾으니, 저장소 밖이면 git 이 제 말로 거절한다.
///
/// **드라이버를 맨 마지막에 적는다**(리뷰 moai-h6aq.cx8). 세 줄은 따로 적히고 되돌리는 길이
/// 없으니, 드라이버를 먼저 적었다가 `recursive` 에서 실패하면 그 저장소는 **드라이버는 서 있고
/// 속 병합 막이는 없는** 상태로 남는다 — 바로 위가 그것이 이슈마다 푼 것을 통째로 날린다고
/// 적어 둔 자리다. 차례를 뒤집으면 실패한 판은 안 심은 것과 같아진다.
fn plant_config(here: &Path, cmd: &str) -> Result<String, String> {
    let driver = driver_command(cmd);
    let name = format!("merge.{DRIVER}.name");
    let recursive = format!("merge.{DRIVER}.recursive");
    let key = driver_key();
    for (k, v) in [
        // **심는 값이라 영어 하나다**(moai-uzgp) — `.gitattributes` 의 블록과 같은 까닭이다.
        // 설정에 앉아 저장소가 함께 쓰는 글이라 이 세션이 고른 말을 따라가면 안 된다.
        (name.as_str(), "moai issues.jsonl — three-way per issue"),
        (recursive.as_str(), "binary"),
        (key.as_str(), driver.as_str()),
    ] {
        crate::git::run(here, &["config", "--local", k, v]).map_err(git_said)?;
    }
    Ok(driver)
}

/// git 이 댄 까닭만 — **moai 의 글은 안 섞는다**(리뷰).
///
/// 무엇을 못 했는지를 앞에 붙이는 것은 [`crate::view::git_trouble`] 이고 그것은 화면 말로 선다.
/// 그 글을 여기서 들면 못 심은 까닭이 영어 화면의 `init.driver_trouble` 한가운데에 남의 말 한
/// 문장으로 박히고, `--json` 의 `driver_trouble` 로도 그대로 나간다 — 저장 계층을 화면 말에서
/// 떼어 둔 결정이 글 한 줄로 도로 새는 자리다. git 의 stderr 는 git 의 말이라 이쪽이 고를 것이
/// 아니고, **가르는 자는 [`crate::git::Error::said`] 하나다**(리뷰) — `worktree::git` 도 같은 자다.
fn git_said(e: crate::git::Error) -> String {
    e.said()
}

/// `moai init` 이 심을 때 일어난 일(moai-08bo).
pub(crate) enum Planting {
    /// 이 저장소는 드라이버를 안 쓴다 — 선언이 없거나(`.gitattributes`), git 저장소가 아니다.
    Off,
    /// 이미 지금 판의 줄이 서 있다. 아무것도 안 썼다.
    Already,
    /// 심었다 — 적은 명령.
    Planted(String),
    /// 못 심었다 — git 이 댄 까닭.
    Failed(String),
}

/// **`moai init` 이 머지 드라이버까지 심는다**(moai-08bo, 2026-09-21 사용자 결정).
///
/// 앞 판의 `init` 은 `.gitattributes` 에 `merge=moai` 라는 **이름**만 쓰고 그 이름이 가리키는
/// **명령**(`.git/config` 의 `merge.moai.driver`)은 안 심었다 — 도구가 제 손으로 안 도는 절반을
/// 만들어 두고 나머지를 사람에게 치라고 한 자리다. 기본으로 돌게 한다.
///
/// **클론은 이것으로 안 덮인다.** `.moai` 가 이미 있는 저장소를 클론한 사람은 `init` 을 안 친다
/// — 그쪽은 [`notice`] 의 `merge_driver_absent` 가 맡는다. 만드는 사람 몫과 받는 사람 몫이다.
///
/// **다시 불러도 안전하다.** 죽은 경로가 적혀 있으면 지금 고른 것으로 고쳐 준다 — `init` 이
/// 원래 그런 명령이고, 워크트리를 지워 죽은 줄을 되살리는 길이 이것으로 하나 는다.
///
/// **도는 줄은 안 건드린다**(리뷰). 고쳐 주는 것은 **죽은 자리**이지 남이 일부러 고른 자리가
/// 아니다. 적힌 값이 지금 고를 것과 다르다는 이유만으로 덮던 판은 이랬다 — `--install --as
/// /usr/local/bin/moai` 로 오래 사는 자리를 박아 둔 클론에서 누가 워크트리의 바이너리로
/// `moai init` 을 한 번 치면, `PATH` 에 `moai` 가 없는 이 저장소에서는 [`chosen_command`] 가
/// `<워크트리>/target/release/moai` 를 내어 그 줄을 덮는다. `--local` 은 클론이 함께 쓰는
/// 자리라 그 한 번이 **모든 체크아웃**을 그 워크트리에 묶고, 워크트리를 지우는 날 전부
/// `merge_driver_rotten` 이 된다 — `--as` 가 막으려던 바로 그것을 `init` 이 만든다. 남이 적은
/// 줄을 "썩었다" 고 안 부르는 moai-h6aq.cx8 의 규칙과 한 자리다.
///
/// **줄의 모양만은 지금 판으로 맞춘다.** 도는 명령은 그대로 두고 앞뒤 마디만 다시 적는다 —
/// [`notice`] 가 `merge_driver_stale` 로 대는 것이 그 모양이라, 안 고치면 `init` 을 쳐도 그
/// 알림이 안 걷힌다.
///
/// **안 걸어 둔 저장소에는 안 심는다.** 선언을 읽는 자는 [`declared`] 하나고, 그래서 moai-47bt
/// 의 탈출구(`.gitattributes` 에 `-merge` 를 적는 것)가 여기에도 그대로 선다.
pub(crate) fn plant_for_init(root: &Path) -> Planting {
    if !declared(root) {
        return Planting::Off;
    }
    // **먼저 선 줄을 본다.** 여기서 물러나면 [`chosen_command`] 의 두 부름(`--version`·[`probe`])
    // 도 안 띄운다 — 이미 도는 저장소에서 `init` 이 가장 흔하게 지나는 길이다.
    let planted = crate::git::run(root, &["config", "--local", "--get", &driver_key()]).unwrap_or_default();
    let planted = planted.trim();
    if let Some(word) = planted_word(planted)
        && matches!(probe(root, &word), Probe::Runs)
    {
        if planted == driver_command(&word) {
            // 이미 지금 판의 줄이다 — `init` 은 한 일을 한 대로 말한다.
            return Planting::Already;
        }
        return match plant_config(root, &word) {
            Ok(_) => Planting::Planted(word),
            Err(why) => Planting::Failed(why),
        };
    }
    let cmd = match chosen_command(root) {
        Ok(c) => c,
        Err(why) => return Planting::Failed(why),
    };
    match plant_config(root, &cmd) {
        Ok(_) => Planting::Planted(cmd),
        Err(why) => Planting::Failed(why),
    }
}

/// 이 저장소의 드라이버가 어떤 자리에 있는가 — `moai init --check` 가 쓰는 한 낱말(moai-08bo).
///
/// **아무것도 안 쓴다.** `--check` 는 재기만 하는 명령이고, 심는 것은 `init` 과
/// `merge-driver --install` 둘뿐이다.
///
/// 낱말은 [`notice`] 의 갈래를 그대로 쓴다 — 두 화면이 같은 것을 다른 이름으로 부르면 받는
/// 쪽이 표를 둘 든다. 선언이 없으면 `off`, 알림이 없으면 `current` 다.
pub(crate) fn state_at(here: &Path, tracker: &Path, chdir: bool) -> &'static str {
    if !declared(tracker) {
        return "off";
    }
    // **[`told`] 로 든다** — [`notice_at`] 으로 들면 선언을 한 번 더 묻는다(같은 답이 나올
    // 물음에 `git check-attr` 를 한 번 더 띄운다).
    match told(here, chdir) {
        Some(w) => w.kind.trim_start_matches("merge_driver_"),
        None => "current",
    }
}

/// `.git/config` 에 드라이버를 심는다.
///
/// **저장소마다 한 번씩 쳐야 한다** — git 은 드라이버 명령을 설정에서만 읽고 설정은
/// 커밋되지 않는다. 안 심은 클론에서는 `.gitattributes` 의 `merge=moai` 가 그냥
/// 무시되고 git 의 기본 텍스트 머지가 돈다 — **병합은 안 심었을 때와 똑같다.** 달라진 것은
/// 화면뿐이다: 걸어 뒀는데 안 심은 클론을 [`notice`] 가 한 줄로 댄다(moai-9khu).
fn install(ctx: &Ctx, as_command: Option<&str>) -> R<Vec<String>> {
    let here = std::env::current_dir().map_err(|e| Fail::new(e.to_string()))?;
    // 사람이 준 것이 이긴다. 안 주면 [`chosen_command`] 가 고른다 — 이 도구는 전역 설치를
    // 안 하므로 맨 `moai` 라고 적으면 PATH 에 없는 기계에서 조용히 안 돈다.
    let cmd = match as_command {
        Some(c) => c.to_string(),
        None => chosen_command(&here)
            .map_err(|why| Fail::new(fill(say(ctx.lang(), "refuse.driver_no_own_path"), &[("why", &why)])))?,
    };
    let key = driver_key();
    let driver = plant_config(&here, &cmd).map_err(Fail::new)?;
    if ctx.json {
        #[derive(serde::Serialize)]
        struct Planted<'a> {
            driver: &'a str,
            attribute: String,
        }
        return super::json_line(&Planted { driver: &driver, attribute: format!("{SNAPSHOT} merge={DRIVER}") });
    }
    // **사람이 친 길만 화면 말을 안다**(moai-uzgp, 2026-09-21 사용자 결정). git 이 `%O %A %B` 로
    // 부르는 길은 사용자 설정을 안 연다 — 저장 계층을 화면 말에서 떼어 둔 결정과 같은 자리다.
    let lang = ctx.lang();
    Ok(vec![
        format!("{key} = {driver}"),
        fill(say(lang, "driver.installed_attribute"), &[("rule", &format!("{SNAPSHOT} merge={DRIVER}"))]),
        say(lang, "driver.installed_per_clone").to_string(),
        // **적은 자리가 사라져도 조용히 잃지는 않는다** — 심는 줄이 `git merge-file` 로
        // 내려앉으므로(`driver_command`) 최악이 안 심은 클론과 같아진다. 그래도 자리는
        // 지키는 편이 낫다: 내려앉은 판은 이슈마다 푼 것을 못 쓰고 사람 손으로 간다.
        // 딸린 워크트리에서 쳐도 이 줄은 **클론이 함께 쓰는** `.git/config` 에 앉으므로
        // (`--local` 은 공용 자리다), 그 워크트리를 지우면 클론 전체가 그 상태가 된다.
        say(lang, "driver.installed_falls_back").to_string(),
        say(lang, "driver.installed_worktree").to_string(),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: &str = "2026-09-01T00:00:00Z";
    const T1: &str = "2026-09-02T00:00:00Z";
    const T2: &str = "2026-09-03T00:00:00Z";

    fn line(id: &str, extra: &str) -> String {
        stamped(id, extra, T0)
    }

    /// `updated_at` 만 다른 같은 줄. 둘 다 고친 줄을 지을 때 쓴다 — 실제로 그렇게 온다.
    fn stamped(id: &str, extra: &str, at: &str) -> String {
        format!(
            "{{\"id\":\"{id}\",\"title\":\"{id} 제목\",\"status\":\"todo\",\
             \"created_at\":\"{T0}\",\"updated_at\":\"{at}\",\"status_since\":\"{T0}\"{extra}}}"
        )
    }

    /// 실제로 도는 길([`plan`])을 그대로 지난다 — 시험만 `by_issue` 를 바로 부르면 못 읽는
    /// 줄이나 겹친 id 를 어떻게 다루는지가 시험 밖에 남는다.
    fn merge(o: &str, a: &str, b: &str) -> (String, Vec<String>) {
        plan(o, a, b, 7)
    }

    /// **서로 다른 이슈를 고친 것은 충돌이 아니다.** git 의 기본 머지는 그 줄들이
    /// 이웃이라는 이유로 부딪치고, 그것이 이 드라이버가 있는 까닭이다.
    #[test]
    fn edits_to_different_issues_do_not_clash() {
        let o = format!("{}\n{}\n", line("argos-0001", ""), line("argos-0002", ""));
        let a = format!("{}\n{}\n", line("argos-0001", ",\"priority\":1"), line("argos-0002", ""));
        let b = format!("{}\n{}\n", line("argos-0001", ""), line("argos-0002", ",\"priority\":3"));
        let (text, clashes) = merge(&o, &a, &b);
        assert!(clashes.is_empty(), "{clashes:?}\n{text}");
        assert!(text.contains("\"priority\":1") && text.contains("\"priority\":3"), "{text}");
        assert_eq!(text.lines().count(), 2, "{text}");
    }

    /// 한쪽만 만든 줄은 그대로 선다. 랜덤 id 라 두 가지가 같은 자리에 줄을 끼우는 일이
    /// 흔한데, git 의 기본 머지는 그것을 매번 충돌로 낸다.
    #[test]
    fn new_issues_from_both_sides_are_kept() {
        let o = format!("{}\n", line("argos-0001", ""));
        let a = format!("{}\n{}\n", line("argos-0001", ""), line("argos-000a", ""));
        let b = format!("{}\n{}\n", line("argos-0001", ""), line("argos-000b", ""));
        let (text, clashes) = merge(&o, &a, &b);
        assert!(clashes.is_empty(), "{clashes:?}");
        assert_eq!(text.lines().count(), 3, "{text}");
        // id 차례로 선다 — 그래야 다음 쓰기가 헛 diff 를 안 낸다.
        let ids: Vec<&str> = text.lines().map(|l| &l[7..17]).collect();
        assert_eq!(ids, ["argos-0001", "argos-000a", "argos-000b"], "{text}");
    }

    /// 같은 이슈를 둘이 **다른 필드로** 고친 것은 풀린다. 이것이 "늦게 쓴 쪽이 이긴다"
    /// 와 갈리는 자리다 — 그 규칙이었으면 먼저 쓴 쪽의 태그가 자취 없이 사라진다.
    #[test]
    fn edits_to_different_fields_of_one_issue_are_merged() {
        let o = format!("{}\n", line("argos-0001", ""));
        let a = format!("{}\n", stamped("argos-0001", ",\"tags\":[\"bug\"]", T1));
        let b = format!("{}\n", stamped("argos-0001", ",\"priority\":1", T2));
        let (text, clashes) = merge(&o, &a, &b);
        assert!(clashes.is_empty(), "{clashes:?}\n{text}");
        assert!(text.contains("\"bug\"") && text.contains("\"priority\":1"), "{text}");
        // 마지막으로 쓴 때는 늦은 쪽이다.
        assert!(text.contains(&format!("\"updated_at\":\"{T2}\"")), "{text}");
    }

    /// **같은 필드를 다르게 고쳤으면 사람이 푼다.** 한쪽을 골라 주면 다른 쪽의 고침이
    /// 아무 자취 없이 사라진다 — 이 도구가 못 견디는 실패 모드가 그것 하나다.
    #[test]
    fn the_same_field_changed_twice_goes_to_a_person() {
        let o = format!("{}\n", line("argos-0001", ""));
        let a = o.replace("todo", "in_progress");
        let b = o.replace("todo", "review");
        let (text, clashes) = merge(&o, &a, &b);
        assert_eq!(clashes, ["argos-0001"], "{text}");
        assert!(text.contains("<<<<<<< ours") && text.contains(">>>>>>> theirs"), "{text}");
        assert!(text.contains("in_progress") && text.contains("review"), "두 쪽을 다 안 보여 준다\n{text}");
    }

    /// 태그는 합집합이 아니라 **정확히 3-way** 다. 합집합이면 한쪽이 뺀 태그가
    /// 되살아나고, 되살아난 것은 아무 자취가 없다.
    #[test]
    fn a_removed_tag_does_not_come_back() {
        let o = format!("{}\n", line("argos-0001", ",\"tags\":[\"bug\",\"parser\"]"));
        let a = format!("{}\n", line("argos-0001", ",\"tags\":[\"parser\"]"));
        let b = format!("{}\n", line("argos-0001", ",\"tags\":[\"bug\",\"parser\",\"tui\"]"));
        let (text, clashes) = merge(&o, &a, &b);
        assert!(clashes.is_empty(), "{clashes:?}");
        assert!(text.contains("parser") && text.contains("tui"), "{text}");
        assert!(!text.contains("bug"), "뺀 태그가 되살아났다\n{text}");
    }

    /// 한쪽이 지우고 다른 쪽이 안 건드렸으면 지운 것이다. 맞서면 사람이 푼다 —
    /// git 의 3-way 와 같은 뜻이다.
    #[test]
    fn a_delete_is_a_delete_unless_the_other_side_edited() {
        let o = format!("{}\n{}\n", line("argos-0001", ""), line("argos-0002", ""));
        let a = format!("{}\n", line("argos-0001", ""));
        let b = o.clone();
        let (text, clashes) = merge(&o, &a, &b);
        assert!(clashes.is_empty() && !text.contains("argos-0002"), "{text} {clashes:?}");

        let b = format!("{}\n{}\n", line("argos-0001", ""), line("argos-0002", ",\"priority\":1"));
        let (_, clashes) = merge(&o, &a, &b);
        assert_eq!(clashes, ["argos-0002"], "지운 쪽과 고친 쪽을 말없이 골랐다");
    }

    /// **둘이 같은 id 를 새로 세운 것은 안 섞는다.** 랜덤 id 가 부딪친 것이면 두 줄은
    /// 서로 다른 일이고, 섞으면 둘 다 아닌 이슈 하나가 남는다.
    #[test]
    fn two_new_issues_with_one_id_go_to_a_person() {
        let o = String::new();
        let a = format!("{}\n", line("argos-0001", ""));
        let b = format!("{}\n", line("argos-0001", ",\"priority\":1"));
        let (_, clashes) = merge(&o, &a, &b);
        assert_eq!(clashes, ["argos-0001"]);
    }

    /// 모르는 필드도 같은 자로 잰다 — 병합이 그것을 지우면 그것도 조용한 손실이다.
    #[test]
    fn an_unknown_field_survives_the_merge() {
        let o = format!("{}\n", line("argos-0001", ""));
        let a = format!("{}\n", line("argos-0001", ",\"from_the_future\":\"x\""));
        let b = format!("{}\n", line("argos-0001", ",\"priority\":1"));
        let (text, clashes) = merge(&o, &a, &b);
        assert!(clashes.is_empty(), "{clashes:?}");
        assert!(text.contains("from_the_future"), "{text}");
    }

    /// **못 읽는 줄은 들고 가되 짝짓지 않는다.** 버리면 조용한 손실이고, 그 한 줄로 파일
    /// 전체를 넘기면 드라이버를 심은 저장소가 안 심은 저장소보다 합치기 어려워진다 —
    /// `store::with_write` 가 그런 줄을 들고 다시 쓰므로 그것은 오래 남는 상태다(CLAUDE.md:
    /// *"남의 낡은 줄 하나가 모든 쓰기를 막으면 되돌릴 방법이 도구 밖에만 남는다"*).
    #[test]
    fn an_unreadable_line_is_carried_not_escalated() {
        let junk = "{망가진 줄";
        let o = format!("{}\n{}\n{junk}\n", line("argos-0001", ""), line("argos-0002", ""));
        let a = format!("{}\n{}\n{junk}\n", line("argos-0001", ",\"priority\":1"), line("argos-0002", ""));
        let b = format!("{}\n{}\n{junk}\n", line("argos-0001", ""), line("argos-0002", ",\"priority\":3"));
        let (text, clashes) = merge(&o, &a, &b);
        assert!(clashes.is_empty(), "낡은 줄 하나로 파일째 넘겼다 — {clashes:?}\n{text}");
        assert!(text.contains("\"priority\":1") && text.contains("\"priority\":3"), "{text}");
        // 원문 그대로, 그리고 `store::render_issues` 처럼 **뒤에** 선다.
        assert_eq!(text.lines().last(), Some(junk), "{text}");
    }

    /// **파일 전체를 넘기는 갈래는 하나만 남았다**(moai-1a55.4oh) — 읽히는 줄이 같은 id 로
    /// 둘 있는 판이다. 그때는 id 로 짝짓는 것 자체가 거짓이라 풀 자가 없다.
    #[test]
    fn what_cannot_be_keyed_hands_the_whole_file_over() {
        let twice = format!("{}\n{}\n", line("argos-0001", ""), line("argos-0001", ""));
        assert!(keyed(&twice).is_none(), "같은 id 두 줄");
        let o = format!("{}\n", line("argos-0001", ""));
        let (text, clashes) = plan(&o, &twice, &o, 7);
        assert_eq!(clashes, ["(파일 전체)"], "{text}");

        let (text, clashes) = whole("a\n", "b\n", 7);
        assert_eq!(clashes, ["(파일 전체)"]);
        assert_eq!(text, "<<<<<<< ours\na\n=======\nb\n>>>>>>> theirs\n", "{text}");
    }

    /// 새 바이너리가 쓴 `kind` 를 옛 바이너리는 못 읽는다. **그 한 줄이 파일 전체를 넘기지
    /// 않는다**(moai-1a55.4oh) — 넘기면 그때부터 머지마다 천 줄이 통째로 충돌하고, `moai status`
    /// 가 못 읽는 줄을 치명으로 세니 그 줄을 한쪽에서 고치는 것이 곧 두 쪽을 다르게 만드는
    /// 일이라 스스로 빠져나올 수도 없다.
    #[test]
    fn a_row_only_one_side_added_that_does_not_read_is_merged_in() {
        let spike = line("argos-0009", ",\"kind\":\"spike\"");
        assert!(row(&spike).is_some_and(|(_, r)| !r.read), "`kind` 를 읽어 버렸다 — 시험 줄이 낡았다");
        let o = format!("{}\n{}\n", line("argos-0001", ""), line("argos-0002", ""));
        let a = format!("{}\n{}\n", line("argos-0001", ",\"priority\":1"), line("argos-0002", ""));
        let b = format!("{o}{spike}\n");
        let (text, clashes) = merge(&o, &a, &b);
        assert!(clashes.is_empty(), "못 읽는 줄 하나로 파일째 넘겼다 — {clashes:?}\n{text}");
        assert!(text.contains("\"priority\":1"), "이쪽 고침이 사라졌다\n{text}");
        // 원문 그대로, 그리고 `store::render_issues` 처럼 **뒤에** 선다.
        assert_eq!(text.lines().last(), Some(spike.as_str()), "{text}");
    }

    /// **못 읽는 줄도 둘 다 고쳤으면 그 줄에만 표식이 선다**(moai-1a55.4oh). id 로 짝지었으니
    /// "한쪽만 건드렸다" 와 "둘 다 건드렸다" 가 갈리고, 옆 이슈는 그대로 이슈마다 풀린다.
    #[test]
    fn a_row_that_does_not_read_and_both_sides_changed_goes_to_a_person() {
        let spike = |extra: &str| line("argos-0009", &format!(",\"kind\":\"spike\"{extra}"));
        let o = format!("{}\n{}\n", line("argos-0001", ""), spike(""));
        let a = format!("{}\n{}\n", line("argos-0001", ",\"priority\":1"), spike(",\"tags\":[\"a\"]"));
        let b = format!("{}\n{}\n", line("argos-0001", ""), spike(",\"tags\":[\"b\"]"));
        let (text, clashes) = merge(&o, &a, &b);
        assert_eq!(clashes, ["argos-0009"], "못 읽는 줄의 두 고침 중 하나를 말없이 골랐다\n{text}");
        assert!(text.contains("\"priority\":1"), "옆 이슈까지 사람에게 넘겼다\n{text}");
        assert!(text.contains("\"a\"") && text.contains("\"b\""), "두 쪽을 다 안 보여 준다\n{text}");
    }

    /// **같은 id 를 든 줄이 둘이면 그 id 가 통째로 사람에게 간다**(moai-2m94, 리뷰
    /// moai-1a55.67v 의 6·7·13번). 한 줄을 열쇠 없는 칸으로 밀어냈을 때는 그 줄이 어느 칸에
    /// 서는지가 **그 파일에 무엇이 또 있는가**에 달려, 세 쪽이 서로 다르게 갈렸다. 그러면
    /// [`settle`] 과 [`unkeyed`] 가 같은 줄을 서로 다른 자로 풀어, 한쪽이 지운 줄이 말없이
    /// 되살아나거나 겹친 id 를 든 파일이 그대로 나왔다 — 둘 다 종료 코드 0 이었다.
    #[test]
    fn rows_sharing_one_id_go_to_a_person() {
        let junk = line("argos-0001", ",\"kind\":\"spike\"");
        let good = line("argos-0001", "");
        // 혼자면 제 id 로 짝지어진다 — 열쇠 없는 칸에는 id 가 읽히는 줄이 안 선다.
        let alone = format!("{junk}\n");
        let solo = keyed(&alone).expect("겹친 id 가 없다");
        assert!(!solo.by_id["argos-0001"][0].read, "`Issue` 로 읽어 버렸다 — 시험 줄이 낡았다");
        assert!(solo.opaque.is_empty(), "id 가 읽히는데 열쇠 없는 줄로 갔다");
        // 둘이 같은 id 를 들면 밀어내지 않고 쌓는다 — 파일 안의 차례가 달라도 셋이 같은 것을 본다.
        for src in [format!("{junk}\n{good}\n"), format!("{good}\n{junk}\n")] {
            let k = keyed(&src).expect("읽히는 줄은 하나뿐이다");
            assert_eq!(k.by_id["argos-0001"].len(), 2, "{src}");
            assert!(k.opaque.is_empty(), "겹친 줄 하나를 열쇠 없는 칸으로 밀어냈다\n{src}");
        }
        let both = format!("{good}\n{junk}\n");
        // 잰 판 하나 — 이쪽이 `junk` 를, 저쪽이 `good` 을 지웠다. 골라 주면 한쪽 지우기가 사라진다.
        let (text, clashes) = merge(&both, &format!("{good}\n"), &format!("{junk}\n"));
        assert_eq!(clashes, ["argos-0001"], "겹친 id 에서 한쪽 지우기를 말없이 되물렀다\n{text}");
        assert!(text.contains("\"title\":\"argos-0001 제목\""), "두 쪽을 다 안 보여 준다\n{text}");
        // 잰 판 둘 — 저쪽에만 같은 id 의 못 읽는 줄이 하나 더 있다.
        let one = format!("{good}\n");
        let (text, clashes) = merge(&one, &one, &both);
        assert_eq!(clashes, ["argos-0001"], "겹친 id 를 든 파일을 말없이 냈다\n{text}");
    }

    /// **id 조차 못 읽는 줄은 센 수로 3-way 한다**(moai-1a55.4oh). 한쪽이 더한 것은 더하고 한쪽이
    /// 지운 것은 뺀다 — `settle_field` 가 `tags` 에 쓰는 것과 같은 셈이다.
    #[test]
    fn lines_without_a_key_are_merged_by_their_count() {
        let (x, y, z) = ("{깨진 하나", "{깨진 둘", "{깨진 셋");
        // 이쪽은 `x` 를 지우고 `z` 를 더했다. 저쪽은 `y` 를 더했다.
        assert_eq!(unkeyed(&[x, y], &[y, z], &[x, y, y]), [y, y, z], "더한 것과 뺀 것이 안 맞는다");
        // 둘이 똑같은 줄을 각자 더했다 — 한 벌만 선다.
        assert_eq!(unkeyed(&[], &[x], &[x]), [x]);
        // 둘이 함께 지웠다.
        assert_eq!(unkeyed(&[x], &[], &[]), [] as [&str; 0]);
        // **여러 벌은 그 벌 수를 지킨다** — 집합으로 세면 그 중 하나가 말없이 사라진다.
        assert_eq!(unkeyed(&[x, x], &[x, x], &[x, x]), [x, x]);
        // **둘이 똑같이 한 벌씩 지운 것은 한 번 지운 것이다.** 두 번 빼면 두 쪽이 다 남기려던
        // 벌까지 사라지는데, 사라진 것이 못 읽는 줄이라 아무 표면에도 안 뜬다 — 더하기를
        // `max` 로 한 번만 세면서 빼기만 두 번 세면 그 비대칭이 곧 조용한 손실이다.
        assert_eq!(unkeyed(&[x, x], &[x], &[x]), [x], "둘이 똑같이 지운 한 벌을 두 번 뺐다");
        assert_eq!(unkeyed(&[x, x, x], &[x, x], &[x, x]), [x, x], "둘이 똑같이 지운 한 벌을 두 번 뺐다");
    }

    /// **그 셈이 실제로 [`by_issue`] 에 물려 있는지까지 본다**(리뷰 moai-1a55.67v). 위는
    /// [`unkeyed`] 를 바로 부르므로 세 인자를 바꿔 끼워도 초록이고, 열쇠 없는 줄이 두 쪽에서
    /// 다른 판을 `plan` 으로 지나 보는 시험이 하나도 없었다 — 파일 전체를 넘기던 갈래를 걷을 때
    /// 그 판을 재던 시험이 같이 걷혔다.
    #[test]
    fn lines_without_a_key_ride_the_whole_plan() {
        let (x, y, z) = ("{깨진 하나", "{깨진 둘", "{깨진 셋");
        let row = line("argos-0001", "");
        // 이쪽은 `x` 를 지우고 `z` 를 더했다. 저쪽은 `y` 를 더하고 이슈를 고쳤다.
        let o = format!("{row}\n{x}\n");
        let a = format!("{row}\n{z}\n");
        let b = format!("{}\n{x}\n{y}\n", line("argos-0001", ",\"priority\":1"));
        let (text, clashes) = merge(&o, &a, &b);
        assert!(clashes.is_empty(), "열쇠 없는 줄로 사람을 불렀다 — {clashes:?}\n{text}");
        assert!(text.contains("\"priority\":1"), "저쪽 고침이 사라졌다\n{text}");
        // 지운 것은 빠지고 더한 것은 든다. **읽히는 줄 뒤에 선다** — `store::render_issues` 자리다.
        assert_eq!(text.lines().skip(1).collect::<Vec<_>>(), [z, y], "{text}");
    }

    /// **충돌 표식에는 사람이 커밋한 그 줄이 그대로 선다.** 다시 지어 적으면 키가 이름
    /// 차례로 서(`Value` 는 `BTreeMap` 이다) 사람이 보는 줄이 `git diff` 가 보여 준 줄과
    /// 다르고, 그 줄을 골라 두면 다음 쓰기가 통째로 헛 diff 를 낸다.
    #[test]
    fn a_clash_shows_the_lines_as_they_were_written() {
        let o = format!("{}\n", line("argos-0001", ""));
        let (a, b) = (o.replace("todo", "in_progress"), o.replace("todo", "review"));
        let (text, clashes) = merge(&o, &a, &b);
        assert_eq!(clashes, ["argos-0001"]);
        for side in [&a, &b] {
            let raw = side.trim_end();
            assert!(text.contains(raw), "원문이 아니라 다시 지어 적었다\n{text}");
        }
        // `id` 가 맨 앞이다 — 이름 차례로 섰으면 `created_at` 이 앞선다.
        assert!(!text.contains("{\"created_at\""), "키 차례가 바뀌었다\n{text}");
    }

    /// **칸과 칸 시각은 한 쪽에서 같이 온다**([`coupled`]). 한쪽이 칸을 왕복해 두면
    /// (`todo`→`in_progress`→`todo`) 그 왕복 시각이 다른 쪽이 옮긴 칸의 나이가 되어,
    /// 방치 경고와 `--stale` 과 보드의 나이가 한꺼번에 거짓을 말한다.
    #[test]
    fn the_column_and_its_clock_come_from_one_side() {
        let o = format!("{}\n", line("argos-0001", ""));
        // 이쪽: 칸을 왕복해 `todo` 로 돌아왔다 — 칸은 그대로고 칸 시각만 T1 이다.
        let a = o.replace(&format!("\"status_since\":\"{T0}\""), &format!("\"status_since\":\"{T1}\""));
        // 저쪽: T2 에 review 로 옮겼다.
        let b = o
            .replace("todo", "review")
            .replace(&format!("\"status_since\":\"{T0}\""), &format!("\"status_since\":\"{T2}\""));
        let (text, clashes) = merge(&o, &a, &b);
        assert!(clashes.is_empty(), "{clashes:?}\n{text}");
        assert!(text.contains("\"status\":\"review\""), "{text}");
        assert!(
            text.contains(&format!("\"status_since\":\"{T2}\"")),
            "review 에 든 때가 아니라 남의 왕복 시각이 섰다\n{text}"
        );
    }

    /// **미루기와 그 시각은 한 쌍이다**(moai-l11z). 따로 고르면 한쪽의 `defer --undo` 가
    /// 아무 자취 없이 되물러진 채 그 `planned_at` 만 남아, 미뤄 둔 줄이 도로 집힌 때를
    /// 제 시각으로 들고 `ready` 에서도 보드에서도 빠진다.
    #[test]
    fn a_deferral_and_its_clock_come_from_one_side() {
        let o = format!("{}\n", line("argos-0001", ""));
        // 이쪽: T1 에 미뤘다.
        let a = format!("{}\n", line("argos-0001", &format!(",\"deferred_at\":\"{T1}\",\"planned_at\":\"{T1}\"")));
        // 저쪽: 미뤘다가 T2 에 도로 집었다 — `deferred_at` 이 없고 `planned_at` 만 남는다.
        let b = format!("{}\n", line("argos-0001", &format!(",\"planned_at\":\"{T2}\"")));
        let (text, clashes) = merge(&o, &a, &b);
        assert!(clashes.is_empty(), "{clashes:?}\n{text}");
        assert!(!text.contains("deferred_at"), "늦게 친 쪽은 도로 집었는데 미뤄 둔 채로 섰다\n{text}");
        assert!(text.contains(&format!("\"planned_at\":\"{T2}\"")), "{text}");
    }

    /// **못 읽는 계획 시각은 사람에게 간다**(moai-1a55.kh2). 못 읽는 쪽을 "모른다" 로 세어
    /// 다른 쪽을 고르면 `deferred_at` 이 그 쪽에서 통째로 따라와, **진 쪽의 미루기가 자취
    /// 없이 사라진다** — 표식도 경고도 없이 종료 코드 0 이었다.
    #[test]
    fn a_deferral_at_a_time_that_cannot_be_read_goes_to_a_person() {
        let o = format!("{}\n", line("argos-0001", &format!(",\"planned_at\":\"{T0}\"")));
        // 이쪽: 손으로 푼 줄이라 `+09:00` 이다. 14:00Z 로 저쪽보다 **늦은** 결정이다.
        let late = "2026-09-19T23:00:00+09:00";
        let a = format!("{}\n", line("argos-0001", &format!(",\"deferred_at\":\"{late}\",\"planned_at\":\"{late}\"")));
        // 저쪽: 표준형이지만 이른 시각이고, 미뤄 두지 않았다.
        let b = format!("{}\n", line("argos-0001", ",\"planned_at\":\"2026-09-19T04:00:00Z\""));
        let (text, clashes) = merge(&o, &a, &b);
        assert_eq!(clashes, ["argos-0001"], "못 읽는 시각에서 한쪽을 골랐다\n{text}");
        assert!(text.contains(late), "이쪽 원문이 표식 안에 안 섰다\n{text}");
    }

    /// **없는 계획 시각도 사람에게 간다**(리뷰 moai-1a55.67v). [`later`] 는 없는 시각을 "모른다"
    /// 로 읽어 있는 쪽을 고르지만, `planned_at` 이 없는 것은 *미뤄 두지 않았다* 는 결정이라 있는
    /// 쪽을 골라 주면 그 결정이 `deferred_at` 과 함께 사라진다 — `defer --undo` 와 `defer` 가
    /// 두 가지에서 맞서는 판이 이 저장소에서는 여느 일이다.
    #[test]
    fn a_row_with_no_planned_at_against_a_deferral_goes_to_a_person() {
        let defer = |at: &str| line("argos-0001", &format!(",\"deferred_at\":\"{at}\",\"planned_at\":\"{at}\""));
        let o = format!("{}\n", defer(T0));
        // 이쪽: 두 필드를 걷었다. 저쪽: T2 에 다시 미뤘다.
        let (a, b) = (format!("{}\n", stamped("argos-0001", "", T1)), format!("{}\n", defer(T2)));
        let (text, clashes) = merge(&o, &a, &b);
        assert_eq!(clashes, ["argos-0001"], "없는 시각을 '모른다' 로 세어 한쪽을 골랐다\n{text}");
        // **[`coupled`] 의 `continue` 는 이 없음에 기대고 있다.** `settle_field` 에 갈래가 생기면
        // 그 줄은 사람에게 안 가고 `deferred_at` 만 여느 규칙으로 갈려 나간다 — `coupled` 가
        // 막으려는 바로 그 어긋남이다.
        assert!(settle_field("planned_at", None, None, None).is_none(), "`settle_field` 에 planned_at 갈래가 생겼다");
        assert!(settle_field("deferred_at", None, None, None).is_none(), "`settle_field` 에 deferred_at 갈래가 생겼다");
    }

    /// **표준형이 아닌 시각은 말없이 고르지 않는다.** 사전순이 곧 시간순인 것은 스무 자
    /// `…Z` 꼴에서만 참이라, `+09:00` 을 바이트로 견주면 실제로 늦은 쪽을 버린다.
    #[test]
    fn a_timestamp_that_is_not_canonical_goes_to_a_person() {
        let o = format!("{}\n", line("argos-0001", ""));
        let a = format!("{}\n", stamped("argos-0001", ",\"tags\":[\"bug\"]", "2026-09-19T12:00:00+09:00"));
        let b = format!("{}\n", stamped("argos-0001", ",\"priority\":1", T2));
        let (text, clashes) = merge(&o, &a, &b);
        assert_eq!(clashes, ["argos-0001"], "못 읽는 시각으로 늦은 쪽을 골랐다\n{text}");
    }

    /// 푼 줄은 **표준형**이다. 안 맞추면 병합 직후의 파일이 다음 쓰기에서 통째로
    /// 헛 diff 를 낸다.
    #[test]
    fn the_merged_line_is_normalized() {
        let o = format!("{}\n", line("argos-0001", ""));
        let a = format!("{}\n", line("argos-0001", ",\"tags\":[\"#Bug\"]"));
        let b = format!("{}\n", line("argos-0001", ",\"priority\":1"));
        let (text, _) = merge(&o, &a, &b);
        assert!(text.contains("\"tags\":[\"bug\"]"), "태그 표기를 안 맞췄다\n{text}");
    }

    /// **필드마다 고른 값이 줄 하나로 서면서 도구가 안 쓰는 짝이 되면 사람에게 간다.**
    ///
    /// 각 필드는 한쪽만 고친 것이라 여느 규칙으로는 말없이 합쳐지는데, 그렇게 나온 줄은
    /// `moai edit`·`mv`·`defer` 가 모두 거절한다 — `store::with_write` 는 이번에 고치는
    /// 줄만 검사하므로 아무도 그 줄을 다시 안 본다. 병합이 도구로 못 만지는 줄을 만드는
    /// 셈이라, 여기서 걸러 사람에게 넘긴다.
    #[test]
    fn a_pair_of_fields_the_tool_would_refuse_goes_to_a_person() {
        let o = format!("{}\n", line("argos-0001", ""));
        let a = format!("{}\n", line("argos-0001", ",\"kind\":\"milestone\""));
        let b = format!("{}\n", line("argos-0001", ",\"milestone\":\"argos-00ms\""));
        let (text, clashes) = merge(&o, &a, &b);
        assert_eq!(clashes, ["argos-0001"], "도구가 못 만지는 줄을 말없이 지었다\n{text}");
        // 한쪽만 선 값은 그대로 지나간다 — 막는 것은 짝이지 필드가 아니다.
        let b = format!("{}\n", line("argos-0001", ",\"priority\":1"));
        let (_, clashes) = merge(&o, &a, &b);
        assert!(clashes.is_empty(), "{clashes:?}");
    }

    /// 오늘 규칙이 거절하는 값을 든 줄. **기한은 마일스톤 줄에만 선다**(`validate_fields`)
    /// — 손으로 푼 충돌이 남겼거나, 뒷날 기한을 넓힌 바이너리가 쓴 줄이 이 꼴로 온다.
    /// 키 차례도 일부러 표준형이 아니다 — 지나간 줄을 [`shaped`] 가 맞추는지까지 재려면
    /// 들어가는 줄이 표준형과 갈려 있어야 한다.
    fn stale(id: &str) -> String {
        format!(
            "{{\"title\":\"{id} 제목\",\"id\":\"{id}\",\"status\":\"todo\",\"due_on\":\"2026-09-20\",\
             \"created_at\":\"{T0}\",\"updated_at\":\"{T0}\",\"status_since\":\"{T0}\"}}"
        )
    }

    /// **양쪽이 안 건드린 줄은 검사하지 않는다**([`settle`], moai-0k2e). 앞 판은 어느
    /// 갈래에서나 `validate_fields` 를 걸어, 그런 줄 하나가 `.moai/issues.jsonl` 을
    /// **머지마다** 충돌로 만들었다.
    #[test]
    fn a_row_neither_side_touched_is_left_alone() {
        let old = stale("argos-0001");
        let o = format!("{old}\n{}\n", line("argos-0002", ""));
        let a = format!("{old}\n{}\n", line("argos-0002", ",\"priority\":1"));
        let b = format!("{old}\n{}\n", line("argos-0002", ""));
        let (text, clashes) = merge(&o, &a, &b);
        assert!(clashes.is_empty(), "{clashes:?}\n{text}");
        // 낡은 **값**은 그대로 지나간다 — 건너뛰는 것은 검사뿐이다.
        assert!(text.contains("\"due_on\":\"2026-09-20\""), "낡은 값이 사라졌다\n{text}");
        assert!(text.contains("\"priority\":1"), "이쪽 고침이 사라졌다\n{text}");
    }

    /// 한쪽만 건드린 줄도 **그 쪽 값 그대로**다. 오늘 규칙에 없는 값을 든 줄을 옛
    /// 바이너리가 그대로 옮겨 적을 수 있고, 그 줄을 고른 것은 이 머지가 아니라 그 가지다.
    ///
    /// **두 방향이 한 답을 낸다**(리뷰 moai-85o3.p4w). 원문을 그대로 내던 판은 `x.v == y.v`
    /// 이면서 바이트가 다를 때 늘 이쪽 것을 골라, `git merge side` 와 `git merge main` 이
    /// 아무도 안 건드린 줄에서 바이트가 다른 파일을 냈다.
    #[test]
    fn a_row_only_one_side_touched_keeps_that_sides_value() {
        let o = format!("{}\n", stale("argos-0001"));
        let a = o.replace("\"status\":\"todo\"", "\"status\":\"in_progress\"");
        let (ours, clashes) = merge(&o, &a, &o);
        assert!(clashes.is_empty(), "{clashes:?}\n{ours}");
        let (theirs, clashes) = merge(&o, &o, &a);
        assert!(clashes.is_empty(), "{clashes:?}\n{theirs}");
        assert_eq!(ours, theirs, "머지 방향이 답을 가른다");
        assert!(ours.contains("\"status\":\"in_progress\""), "고친 쪽 값이 안 섰다\n{ours}");
        assert!(ours.contains("\"due_on\":\"2026-09-20\""), "낡은 값이 사라졌다\n{ours}");
    }

    /// **지나 보내는 줄도 표준형으로 맞춘다**([`shaped`], 리뷰 moai-85o3.p4w). 건너뛰는 것은
    /// 검사뿐이고, 그 자리가 `store::with_write` 가 선 자리다 — 그쪽은 읽은 줄을 모두
    /// `normalize` 하고 검사만 이번에 바뀐 줄에 건다.
    ///
    /// 꼴까지 건너뛴 판에서 넷이 무너졌다. 접히지 않은 `"#Bug"` 는 `moai show -t bug` 에서
    /// 빠지고(`query` 는 찾는 쪽만 접는다), `"started_at":""` 은 `move_to` 에 "이미 적혔다"
    /// 로 읽혀 다음 `mv` 가 시작을 영영 안 적고(moai-38mh), `"priority":2` 는 다음 쓰기가
    /// 걷어 아무도 안 건드린 줄에 헛 diff 를 내며, 겹친 키를 든 줄은 `store::parse_issues`
    /// 에 못 읽는 줄로 가 `moai status` 를 비영으로 끝낸다 — [`row`] 는 `Value` 를 거쳐
    /// 읽어 겹친 키를 마지막 것으로 접으므로 둘의 문턱이 다르다.
    #[test]
    fn a_row_that_passes_through_is_still_shaped() {
        let junk = format!(
            "{{\"id\":\"argos-0001\",\"title\":\"첫 제목\",\"title\":\"나중 제목\",\"status\":\"todo\",\
             \"tags\":[\"#Bug\",\"Bug\"],\"priority\":2,\"started_at\":\"\",\
             \"created_at\":\"{T0}\",\"updated_at\":\"{T0}\",\"status_since\":\"{T0}\"}}"
        );
        let o = format!("{junk}\n{}\n", line("argos-0002", ""));
        let a = format!("{junk}\n{}\n", line("argos-0002", ",\"priority\":1"));
        let (text, clashes) = merge(&o, &a, &o);
        assert!(clashes.is_empty(), "{clashes:?}\n{text}");
        let row = text.lines().next().expect("줄이 없다");
        assert_eq!(row.matches("\"title\":").count(), 1, "겹친 키가 그대로 섰다\n{row}");
        assert!(row.contains("\"tags\":[\"bug\"]"), "태그를 안 접었다\n{row}");
        assert!(!row.contains("\"priority\""), "기본값을 안 걷었다\n{row}");
        assert!(!row.contains("\"started_at\""), "빈 시각을 안 걷었다\n{row}");
    }

    /// **경계는 여기다** — 둘이 이 줄을 서로 다른 필드로 고치면 그 줄은 어느 가지에도
    /// 없던 줄이라 [`issue`] 가 재고, 낡은 값에 걸려 사람에게 간다. 안 건드린 줄과 달리
    /// 이것은 한 번 풀면 끝이고, 도구가 못 만지는 줄을 병합이 짓지 않게 막는다.
    #[test]
    fn a_stale_row_both_sides_edited_still_goes_to_a_person() {
        let o = format!("{}\n", stale("argos-0001"));
        let a = o.replace("\"status\":\"todo\"", "\"status\":\"in_progress\"");
        let b = o.replace("{\"title\"", "{\"priority\":1,\"title\"");
        let (text, clashes) = merge(&o, &a, &b);
        assert_eq!(clashes, ["argos-0001"], "조립한 줄을 안 쟀다\n{text}");
    }

    /// **심는 줄은 세 마디다** — 돌면 그것으로 끝, `%A` 를 건드리고 실패했으면 그대로 넘김,
    /// 안 건드리고 실패했으면 `git merge-file`. 셋째 마디가 없으면 경로가 썩은 순간
    /// git 이 표식 없는 파일을 남기고, `git add` 한 번에 저쪽이 사라진다.
    ///
    /// **가르는 자가 `%A` 의 내용이 아니라 `cmp` 여야 한다**(리뷰 moai-h6aq.cx8) — 표식을
    /// `grep` 으로 찾으면 파일에 이미 있던 `<<<<<<<` 한 줄이 못 돈 판을 "사람에게 넘겼다" 로
    /// 읽히게 하고, `conflict-marker-size` 가 7 보다 짧으면 드라이버가 쓴 표식을 놓친다.
    #[test]
    fn the_planted_line_falls_back_to_gits_own_merge() {
        let line = driver_command("/w/moai");
        assert!(line.starts_with("cp %A %A.ours; if /w/moai merge-driver %O %A %B %L %P; then"), "{line}");
        assert!(line.contains("! cmp -s %A %A.ours; then rm -f %A.ours; exit 1"), "건드린 판을 내려앉혔다\n{line}");
        assert!(!line.contains("grep"), "`%A` 의 내용으로 갈랐다\n{line}");
        assert!(
            line.ends_with("exec git merge-file --marker-size=%L -L ours -L base -L theirs %A %O %B"),
            "내려앉은 판이 저장소가 고른 표식 너비를 안 쓴다\n{line}"
        );
        // 한 줄이어야 한다 — `git config` 의 값은 줄 하나다.
        assert_eq!(line.lines().count(), 1, "{line}");
    }

    /// **심어 둔 줄에서 부르는 명령을 떼어 낸다.** 모르는 모양이면 아무 말도 안 한다 —
    /// 남이 손으로 적은 줄을 "썩었다" 고 부르면 걷을 길이 없는 알림이 선다.
    #[test]
    fn the_planted_word_is_read_back_or_not_at_all() {
        let word = |cmd: &str| planted_word(&driver_command(cmd));
        assert_eq!(word("/w/moai").as_deref(), Some("/w/moai"));
        assert_eq!(word("/w/My Work/moai").as_deref(), Some("/w/My Work/moai"));
        assert_eq!(word("moai").as_deref(), Some("moai"), "PATH 의 낱말도 읽는다");
        // 홑따옴표가 든 경로는 `shell_word` 가 이어 붙여 싼다 — 떼어 낸 조각이 경로가 아니다.
        assert_eq!(word("/w/it's/moai"), None);
        // 제어문자가 든 경로는 `$'…'` 다. 푸는 규칙을 여기 또 쓰지 않는다.
        assert_eq!(word("/w/a\tb/moai"), None);
        // 빈 `--as` 는 `''` 로 심긴다. 아무 경로도 안 대는 알림을 세우지 않는다.
        assert_eq!(word(""), None);
        // 옛 판(맨 명령)은 그대로 읽는다 — 그 줄에도 `merge-driver %O %A %B` 가 선다.
        assert_eq!(planted_word("/w/moai merge-driver %O %A %B %L %P").as_deref(), Some("/w/moai"));
        // **이 도구가 지은 모양이 아니면 재지 않는다.** 남이 손으로 적은 줄이다.
        assert_eq!(planted_word("python3 tools/merge.py %O %A %B"), None);
        assert_eq!(planted_word("if command -v moai >/dev/null; then moai m %O %A %B; fi"), None);
        assert_eq!(planted_word(""), None);
    }

    /// **부를 수 있는지는 불러서 잰다**(moai-zdw4). 파일의 모드를 읽던 판은 "아무나 돌릴 수
    /// 있는가" 까지만 봤고, 자리는 멀쩡한데 **이 명령을 모르는** 바이너리를 못 갈랐다 — 이름을
    /// `moai` 그대로 두고 배포하는 마당에 그 자리가 가장 가깝다.
    ///
    /// **상대 경로는 뿌리에 붙는다** — git 이 드라이버를 워크트리 꼭대기에서 돌리기 때문이다.
    #[cfg(unix)]
    #[test]
    fn the_probe_calls_the_command_instead_of_reading_its_mode() {
        use std::os::unix::fs::PermissionsExt as _;
        // 자리는 `Scratch` 가 쥔다 — 끝에서 `remove_dir_all` 을 부르면 패닉한 판이 찌꺼기를 남긴다.
        let scratch = crate::scratch::Scratch::new("probe");
        let dir = scratch.path();
        // **돌릴 파일은 이 프로세스가 쓴 inode 를 안 쓴다**(리뷰 moai-vbmn.spv). 쓰기 fd 가 열린
        // 동안 옆 스레드의 시험이 fork 하면 그 자식이 fd 를 물려받고, 자식이 exec 할 때까지 그
        // inode 에 쓰는 이가 남아 Linux 가 이쪽 exec 을 ETXTBSY 로 거절한다 — `tests/cli.rs` 의
        // `place_exe` 가 복사 2,400번으로 잰 것이 그것이다(`fs::copy` 345번, `cp` 0번). 여기가
        // 쓴 것을 **이 프로세스에서 곧바로 부르는** 첫 자리라 같은 대처를 쓴다.
        let write = |name: &str, body: &str| {
            let src = dir.join(format!("{name}.src"));
            let p = dir.join(name);
            std::fs::write(&src, body).unwrap();
            std::fs::set_permissions(&src, std::fs::Permissions::from_mode(0o755)).unwrap();
            let out = std::process::Command::new("cp").arg(&src).arg(&p).output().unwrap();
            assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
            p
        };
        let at = |cmd: &std::path::Path| probe(dir, &cmd.display().to_string());

        // 이 명령을 아는 바이너리. `--help` 는 어느 갈래에서도 파일을 안 건드린다.
        // **제 이름을 댄다**(moai-wwbi) — 0 으로 끝나는 것만으로는 `Runs` 가 아니다.
        let ours = write(
            "moai",
            "#!/bin/sh\n[ \"$1\" = merge-driver ] && { echo 'Usage: moai merge-driver'; exit 0; }\nexit 2\n",
        );
        assert!(matches!(at(&ours), Probe::Runs));

        // **모르는 인자를 흘려 듣고 0 을 내는 래퍼**(moai-wwbi). 끝난 꼴만 보던 판이 그냥
        // 지나가던 자리다 — 그 명령은 이 부명령을 모르고 매 병합이 말없이 내려앉는다.
        let mute = write("삼킨다", "#!/bin/sh\nexit 0\n");
        assert!(matches!(at(&mute), Probe::Alien), "흘려 듣는 래퍼가 지나갔다");

        // **그 이름의 다른 도구.** 파일도 있고 돌기도 도는데 이 명령을 모른다 — 모드를 읽던
        // 판이 조용히 지나가던 자리다.
        let alien = write("옛moai", "#!/bin/sh\necho '모르는 부명령' >&2\nexit 2\n");
        assert!(matches!(at(&alien), Probe::Alien));

        // 띄우지도 못하는 셋. 자리가 비었다, 실행 권한이 없다, 디렉터리다.
        assert!(matches!(at(&dir.join("없다")), Probe::Dead));
        let dead = write("권한없다", "#!/bin/sh\nexit 0\n");
        std::fs::set_permissions(&dead, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(matches!(at(&dead), Probe::Dead));
        assert!(matches!(at(dir), Probe::Dead), "디렉터리를 명령으로 읽었다");

        // 상대 경로는 프로세스의 자리가 아니라 뿌리에서 푼다.
        assert!(matches!(probe(dir, "./moai"), Probe::Runs), "뿌리에 있는 상대 경로를 못 찾았다");
        assert!(matches!(probe(Path::new("/"), "./moai"), Probe::Dead), "엉뚱한 뿌리에서 찾아 냈다");

        // **표준 입력을 끊는다.** 물려주면 stdin 을 읽는 명령 하나가 `moai status` 를 영영
        // 붙든다. 읽은 것이 비었는지로 잰다 — 끊긴 자리는 곧바로 EOF 다.
        let seen = dir.join("본것");
        // 자리를 감싼다 — `Scratch` 의 이름에는 `ThreadId(3)` 의 괄호가 든다.
        let quoted = crate::text::shell_word(&seen.display().to_string());
        let reader = write("읽는다", &format!("#!/bin/sh\ncat > {quoted}\necho merge-driver\nexit 0\n"));
        assert!(matches!(at(&reader), Probe::Runs));
        assert_eq!(std::fs::read(&seen).unwrap(), Vec::<u8>::new(), "표준 입력을 물려줬다");

        // **못 잰 것은 넷째 갈래다**(리뷰 moai-vbmn.spv). 신호에 맞아 죽은 자식은 `Ok` 로 오고
        // `success()` 만 거짓이라, 뭉치던 판은 OOM 에 맞아 죽은 **제 바이너리**를 "그 이름의 다른
        // 도구" 라고 불렀다. 16GiB 상한에 세션 예닐곱이 도는 이 기계에서 가까운 자리다.
        let killed = write("맞아죽는다", "#!/bin/sh\nkill -9 $$\n");
        assert!(matches!(at(&killed), Probe::Unknown), "신호에 맞아 죽은 것을 딴 도구라고 했다");

        // 126·127 은 껍데기와 로더가 "못 돌렸다" 고 낼 때 쓰는 값이다 — 자리 이야기지 이름
        // 이야기가 아니다(공유 라이브러리를 못 찾은 판이 127 이다).
        for code in [126, 127] {
            let p = write(&format!("못돌린다{code}"), &format!("#!/bin/sh\nexit {code}\n"));
            assert!(matches!(at(&p), Probe::Dead), "{code} 을 딴 도구라고 했다");
        }

        // **시간을 잰다.** 입을 끊어도 잠드는 길은 남는다 — 여기가 막히면 `moai status` 와 훅의
        // 보드가 글자 한 줄 없이 영영 안 끝난다. 재는 것은 "제 시간에 돌아오는가" 하나다.
        let clock = std::time::Instant::now();
        // `exec` 로 갈아 끼운다 — 껍데기가 `sleep` 을 따로 띄우면 죽이는 것은 껍데기뿐이라
        // 시험이 끝난 뒤에도 30초짜리가 남는다(죽이기가 바로 아래 자식까지인 까닭, 위의 글).
        let slow = write("느리다", "#!/bin/sh\nexec sleep 30\n");
        assert!(matches!(at(&slow), Probe::Unknown), "늦은 것을 재고 말았다");
        assert!(clock.elapsed() < PROBE_BUDGET * 3, "한도를 안 지켰다 — {:?}", clock.elapsed());

        // **손자가 살아 있어도 제 시간에 돌아온다**(리뷰). 도움말을 파이프로 받던 판은 아이를
        // 거둔 **뒤에** 그 파이프를 읽어, 아이가 물려준 쓰기 끝을 손자가 쥐고 있으면 거기서
        // 영영 멈췄다 — [`PROBE_BUDGET`] 은 아이까지만 재므로 아무것도 안 막는 자리다. 뒤에
        // 일을 하나 띄우는 껍데기 shim(mise·asdf·direnv 꼴)이 그 모양이고, 그 판에서
        // `moai status` 는 글자 한 줄 없이 안 끝났다. 받는 자리를 파일로 두어 끊었다.
        let clock = std::time::Instant::now();
        let busy = write("손자를남긴다", "#!/bin/sh\necho 'Usage: moai merge-driver'\nsleep 30 &\nexit 0\n");
        assert!(matches!(at(&busy), Probe::Runs), "손자를 남긴 래퍼를 못 읽었다");
        assert!(clock.elapsed() < PROBE_BUDGET, "손자가 읽기를 붙들었다 — {:?}", clock.elapsed());

        // **재고 나면 자리를 안 남긴다** — 알림 하나가 `moai status` 마다 찌꺼기를 쌓으면 안 된다.
        let (said, sink) = Said::new().expect("받을 자리를 못 열었다");
        let at_file = said.0.clone();
        drop(sink);
        assert!(at_file.exists());
        drop(said);
        assert!(!at_file.exists(), "잰 뒤 자리가 남았다 — {}", at_file.display());
    }

    /// **빈칸이 든 경로를 감싼다.** 안 감싸면 첫 낱말에서 끊겨 늘 내려앉는 길로만 가고,
    /// 그 저장소는 드라이버를 심고도 안 심은 것과 같아진다.
    #[test]
    fn the_planted_line_quotes_a_path_with_a_space() {
        let line = driver_command("/w/My Work/moai");
        assert!(line.contains("if '/w/My Work/moai' merge-driver "), "{line}");
        // 자리표시자는 감싸지 않는다 — git 이 제 임시 파일 이름으로 바꾼다. `%P` 는 git 이 이미 싼다.
        assert!(!line.contains("\"%A\"") && !line.contains("'%A'"), "자리표시자를 덧쌌다\n{line}");
    }
}
