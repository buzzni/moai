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
//! 이슈로 안 읽히는 줄은 원문 그대로 뒤에 붙인다 — `store::render_issues` 가 두는 자리와
//! 같다. 그 한 줄로 파일 전체를 충돌로 넘기면 드라이버를 심은 저장소가 안 심은 저장소보다
//! 합치기 어려워지고, CLAUDE.md 가 막은 자리가 바로 그것이다("남의 낡은 줄 하나가 모든
//! 쓰기를 막으면 되돌릴 방법이 도구 밖에만 남는다").
//!
//! 통째로 넘기는 것은 **짝지을 수가 없을 때** 셋이다 — 한 파일에 같은 id 가 두 번 있을 때,
//! 못 읽은 줄이 두 쪽에서 다를 때(그 줄을 3-way 로 볼 자가 없다), 그리고 글자가 깨져
//! 줄로도 못 나눌 때. 답을 못 짓는 갈래도 **표식을 쓰고 나서** 비영으로 끝낸다: 표식 없이
//! 실패하면 git 은 `%A` 를 그대로 둔 채 "충돌" 이라고만 말하고, 사람은 그 파일을 열어 보고
//! 이미 풀린 줄 알아 `git add` 한 번으로 저쪽을 통째로 버린다.

use super::{Ctx, Fail, R};
use crate::cli::MergeDriverArgs;
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
}

/// 파일 하나를 id 로 짝지은 결과.
struct Keyed<'s> {
    by_id: BTreeMap<String, Row<'s>>,
    /// 이슈로 안 읽히는 줄의 **원문 그대로**. `store::render_issues` 의 `opaque` 와 같은 자리다.
    opaque: Vec<&'s str>,
}

/// 한 이슈를 푼 결과.
enum Settled {
    /// 이 줄로 쓴다. 지워진 것이면 `None`.
    ///
    /// **`Issue` 가 아니라 다 쓴 글로 든다.** 이 자리에서 쓸 것은 `store::render_issues` 와
    /// 같은 한 줄뿐이라 값을 들고 다닐 까닭이 없고, `Issue` 를 담으면 갈래 둘의 크기가
    /// 크게 벌어져(`clippy::large_enum_variant`) id 마다 그 큰 쪽만큼 옮긴다.
    Line(Option<String>),
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
        return Err(Fail::coded(
            "머지 드라이버는 `%O %A %B` 세 자리를 받는다. 심는 길은 `moai merge-driver --install`",
            super::code::BAD_INPUT,
        ));
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

/// 셋 다 글자로 읽히는 판. **못 읽은 줄이 두 쪽에서 같을 때만 id 로 짝짓는다** — 다르면
/// 그 줄을 3-way 로 볼 자가 없고, 읽은 것만 골라 쓰면 그 줄이 그 자리에서 사라진다.
fn plan(o: &str, a: &str, b: &str, marker: usize) -> (String, Vec<String>) {
    match (keyed(o), keyed(a), keyed(b)) {
        (Some(o), Some(x), Some(y)) if x.opaque == y.opaque => by_issue(&o, &x, &y, marker, a.len()),
        // 같은 id 가 두 번 있거나, 못 읽은 줄이 두 쪽에서 다르다 — 통째로 넘긴다.
        _ => whole(a, b, marker),
    }
}

fn clash_fail(clashes: &[String], what: &str) -> Fail {
    Fail::coded(format!("{what}: {}건은 사람이 푼다 — {}", clashes.len(), clashes.join(" ")), super::code::BROKEN)
}

/// 파일 하나를 id → 줄로. **같은 id 가 두 번 있으면 `None`** — 그때는 id 로 짝짓는 것
/// 자체가 거짓이 된다.
///
/// **못 읽는 줄은 원문 그대로 들고 간다**(`Keyed::opaque`). 버리지 않는 것은 그 줄이
/// 사라지면 조용한 손실이라서고, 파일 전체를 넘기지 않는 것은 CLAUDE.md 가 정한 자리라서다
/// — *"그 엄함은 지금 쓰는 줄에 대한 것이지 파일 전체에 대한 것이 아니다. 남의 낡은 줄 하나가
/// 모든 쓰기를 막으면 되돌릴 방법이 도구 밖에만 남는다."* `store::with_write` 가 못 읽는 줄을
/// 들고 다시 쓰므로 그런 줄은 저장소에 오래 남는데, 그 한 줄로 모든 병합을 파일째 충돌로
/// 넘기면 드라이버를 심은 저장소가 안 심은 저장소보다 합치기 어려워진다.
fn keyed(src: &str) -> Option<Keyed<'_>> {
    let src = src.strip_prefix('\u{feff}').unwrap_or(src);
    let mut by_id = BTreeMap::new();
    let mut opaque = Vec::new();
    for line in src.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let Some((id, r)) = row(line) else {
            opaque.push(line);
            continue;
        };
        if by_id.insert(id, r).is_some() {
            // 같은 id 가 두 번 있으면 id 로 짝짓는 것 자체가 거짓이다.
            return None;
        }
    }
    Some(Keyed { by_id, opaque })
}

/// 줄 하나를 id 와 값으로. **이슈로 안 읽히면 `None`** — 짝짓지 않고 원문으로 들고 간다.
/// 여기서 안 읽히는 줄을 짝지어 쓰면 그 줄은 다음 `moai` 가 못 읽는 줄로 만난다.
fn row(line: &str) -> Option<(String, Row<'_>)> {
    let v: Value = serde_json::from_str(line).ok()?;
    // 읽히기는 해도 이슈가 아닌 줄(배열·수·id 없는 객체)은 못 짝짓는다.
    let id = v.get("id")?.as_str()?.to_string();
    // **`&Value` 에서 바로 읽는다** — `from_value` 는 통째로 복사한 뒤 읽어, 줄마다 트리
    // 하나를 더 짓는다. 값은 아래 `issue()` 가 다시 쓰므로 여기서는 읽히는지만 본다.
    Issue::deserialize(&v).ok()?;
    Some((id, Row { raw: line, v }))
}

/// id 마다 3-way 로 풀고, 푼 것과 못 푼 것을 한 파일로 짓는다.
///
/// `hint` 는 답의 크기 짐작(이쪽 파일의 길이)이다 — 없으면 1MB 짜리 스냅샷에서 버퍼가
/// 열일곱 번 자라며 이미 쓴 것을 그만큼 옮긴다.
fn by_issue(o: &Keyed<'_>, a: &Keyed<'_>, b: &Keyed<'_>, marker: usize, hint: usize) -> (String, Vec<String>) {
    let ids: BTreeSet<&String> = o.by_id.keys().chain(a.by_id.keys()).chain(b.by_id.keys()).collect();
    let mut out = String::with_capacity(hint);
    let mut clashes = Vec::new();
    for id in ids {
        match settle(o.by_id.get(id), a.by_id.get(id), b.by_id.get(id)) {
            Settled::Line(None) => {}
            Settled::Line(Some(line)) => {
                out.push_str(&line);
                out.push('\n');
            }
            Settled::Clash { ours, theirs } => {
                clashes.push(id.clone());
                out.push_str(&marked(ours.as_deref(), theirs.as_deref(), marker));
            }
        }
    }
    // **못 읽은 줄은 뒤에 원문 그대로 붙는다** — `store::render_issues` 가 두는 자리와 같다.
    // 두 쪽이 같을 때만 여기 온다(`plan`).
    for line in &a.opaque {
        out.push_str(line);
        out.push('\n');
    }
    (out, clashes)
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
fn settle(o: Option<&Row<'_>>, a: Option<&Row<'_>>, b: Option<&Row<'_>>) -> Settled {
    // **원문을 그대로 든다**([`Row`]). 매개변수를 안 받는 것은 여덟 자리 중 한 곳에서
    // 두 쪽을 뒤집어 넘기는 실수가 컴파일되지 않게 하려는 것이다.
    let clash = || Settled::Clash { ours: a.map(|r| r.raw.to_string()), theirs: b.map(|r| r.raw.to_string()) };
    let keep = |v: &Value| {
        issue(v)
            .map_or_else(clash, |i| Settled::Line(Some(serde_json::to_string(&i).expect("Issue 는 언제나 직렬화된다"))))
    };
    match (o.map(|r| &r.v), a.map(|r| &r.v), b.map(|r| &r.v)) {
        (_, None, None) => Settled::Line(None),
        // 한쪽만 건드렸다.
        (o, Some(x), None) | (o, None, Some(x)) if o == Some(x) => Settled::Line(None),
        (None, Some(x), None) | (None, None, Some(x)) => keep(x),
        // 지운 쪽과 고친 쪽이 맞선다.
        (Some(_), _, None) | (Some(_), None, _) => clash(),
        (o, Some(x), Some(y)) => {
            if x == y {
                return keep(x);
            }
            if o == Some(x) {
                return keep(y);
            }
            if o == Some(y) {
                return keep(x);
            }
            // **둘 다 새로 세운 같은 id 는 안 섞는다** — 서로 다른 일이다.
            let Some(o) = o else { return clash() };
            fields(o, x, y).map_or_else(clash, |v| keep(&v))
        }
    }
}

/// 쓰기 전에 이슈로 한 번 읽는다 — **읽기는 관대하고 쓰기는 엄하다.** 여기서 안 읽히는
/// 줄을 내보내면 그 줄은 다음 `moai` 가 못 읽는 줄로 만나고, 병합이 그것을 만든 것이 된다.
///
/// 표준형으로 맞춘 뒤 돌려준다. 맞추지 않으면 병합 직후의 파일이 다음 쓰기에서 통째로
/// 헛 diff 를 낸다(멱등성).
///
/// **검사도 `store::with_write` 와 같은 것을 건다**(`Issue::validate_fields`). 필드마다
/// 따로 고른 값들이 줄 하나로 서면서 도구가 절대 안 쓰는 짝이 될 수 있다 — 한쪽이
/// `kind` 를 마일스톤으로 바꾸고 다른 쪽이 `milestone` 을 붙이면 각 필드는 한쪽만 고친
/// 것이라 말없이 합쳐지는데, 그 줄은 `moai edit`·`mv`·`defer` 가 모두 거절한다. 여기서
/// 거르면 그 줄은 사람에게 가고, 넘기면 병합이 도구로 못 만지는 줄을 만든 것이 된다.
///
/// **칸 이름은 안 본다.** 그것만 `.moai/config.toml` 을 읽어야 하는데 이 명령은 설정을
/// 안 지난다(`cmd/mod.rs`). 설정으로 칸을 고친 저장소의 옛 줄을 병합이 막지 않는 쪽이
/// 맞기도 하다 — CLAUDE.md 의 "엄함은 지금 쓰는 줄에 대한 것" 과 같은 자리다.
fn issue(v: &Value) -> Option<Issue> {
    let mut i = Issue::deserialize(v).ok()?;
    i.normalize();
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
                (Some(_), _) | (None, None) => a,
                (None, Some(_)) => b,
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
    /// 키가 없을 때 git 이 돌려줄 글. **값이 될 수 없는 것이라야 한다** — 심는 줄은 껍데기 명령이고
    /// 제어문자 하나만 든 줄은 그 무엇도 아니다.
    const UNSET: &str = "\u{1}";
    // **먼저 묻는다.** 뒤로 미루면 안 쓰기로 한 저장소에서도 설정을 읽고 `probe` 가 뜬다.
    if !declared(tracker) {
        return None;
    }
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
    /// 이 명령을 알고 0 으로 끝났다 — 도움말이 제 이름을 댄다([`spoke`]).
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
/// 그 저장소의 매 병합은 말없이 내려앉는다. 도움말이 제 이름을 대는지까지 본다([`spoke`]).
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
/// 붙들고 영영 안 끝난다. 표준 출력과 오류도 버린다: 재는 부름이 사람의 화면에 글을 쓰면 안 된다.
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
        let spawned = Command::new(program)
            .args([SUB, "--help"])
            .current_dir(root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let mut child = match spawned {
            Ok(c) => c,
            // 이 둘만 그 **자리**에 대한 말이다.
            Err(e) if matches!(e.kind(), ErrorKind::NotFound | ErrorKind::PermissionDenied) => return Probe::Dead,
            // 나머지는 기계가 막은 것이다 — 멀쩡한 자리를 "썩었다" 고 부르지 않는다.
            Err(_) => return Probe::Unknown,
        };
        let said = child.stdout.take();
        let Some(st) = reaped(&mut child) else { return Probe::Unknown };
        match st.code() {
            // **0 으로 끝난 것만으로는 모자라다**(moai-wwbi, 2026-09-21 사용자 결정). 모르는 인자를
            // 흘려 듣고 0 을 내는 래퍼(`#!/bin/sh` 두 줄짜리)는 `merge-driver` 를 모른 채 그대로
            // 지나갔고, 그 저장소의 매 병합이 말없이 내려앉는다. 도움말에 제 이름이 서는지까지 본다.
            Some(0) => {
                if spoke(said) {
                    Probe::Runs
                } else {
                    Probe::Alien
                }
            }
            // 껍데기와 로더의 낱말이다 — 공유 라이브러리를 못 찾은 판, exec 에 막힌 래퍼. 그 이름의
            // 딴 도구가 아니라 그 자리가 못 도는 것이다.
            Some(126 | 127) => Probe::Dead,
            Some(_) => Probe::Alien,
            // 신호에 맞아 죽었다(유닉스에서만 선다) — OOM 킬러가 가장 흔하다.
            None => Probe::Unknown,
        }
    }
}

/// 그 도움말이 **제 이름을 대는가** — [`probe`] 가 `Runs` 와 `Alien` 을 가르는 둘째 자(moai-wwbi).
///
/// **표식은 부명령 이름 하나다**([`SUB`]). 이 도구의 `merge-driver --help` 는 어느 판에서나
/// 쓰임새 줄에 그 낱말을 싣는다 — clap 이 짓는 줄이라 글을 고쳐도 남는다. 심는 줄의 규약이
/// 하나 느는 일이고(도움말에 그 낱말이 서야 한다), 그것을 매는 자가
/// `the_probe_marker_stands_in_the_help` 다.
///
/// **못 읽은 판은 안 본 것과 같다.** 파이프를 못 잡았거나 글자가 깨졌으면 표식이 없는 것으로
/// 친다 — 0 으로 끝났는데 제 이름을 못 댄 명령이고, 그 자리의 낱말은 `Alien` 이다.
fn spoke(said: Option<std::process::ChildStdout>) -> bool {
    use std::io::Read;
    let Some(mut out) = said else { return false };
    let mut buf = Vec::new();
    // **다 읽고 잰다.** 도움말은 몇 줄이라 파이프 하나에 든다. 파이프를 채우고도 안 끝나는
    // 명령은 여기서 막히는 것이 아니라 [`PROBE_BUDGET`] 에 걸려 `Unknown` 으로 간다 — 그쪽이
    // 이미 "못 잰 것은 말하지 않는다" 의 자리다.
    let _ = out.read_to_end(&mut buf);
    String::from_utf8_lossy(&buf).contains(SUB)
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
/// **"같은 판" 은 두 자로 잰다.** `--version` 이 같은 글을 내고(옛 moai 도 `moai` 라는 이름을
/// 쓴다 — 이름만으로는 못 가른다), [`probe`] 가 `Runs` 라야 한다. 둘 중 하나라도 어긋나면 지금
/// 바이너리를 적는다 — 틀리는 값은 **덜 이르는 쪽**이라야 한다.
fn chosen_command(here: &Path) -> Result<String, String> {
    let mine = std::env::current_exe().map_err(|e| format!("이 바이너리의 자리를 모른다: {e}"))?;
    let Some(found) = on_path(DRIVER) else { return Ok(mine.display().to_string()) };
    // 같은 파일이면 고를 것이 없다 — 이름으로 적으면 git 이 병합에서 쓸 `PATH` 에 기대는 것이
    // 하나 늘 뿐이다.
    if found == mine {
        return Ok(mine.display().to_string());
    }
    let word = found.display().to_string();
    let same = version_of(&found).is_some_and(|v| v == my_version());
    if same && matches!(probe(here, &word), Probe::Runs) { Ok(word) } else { Ok(mine.display().to_string()) }
}

/// 이 바이너리가 `--version` 으로 내는 글.
fn my_version() -> String {
    format!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
}

/// 그 명령이 `--version` 으로 내는 글 — 못 물어봤으면 `None`.
fn version_of(path: &Path) -> Option<String> {
    let out = std::process::Command::new(path)
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// `PATH` 에서 그 이름의 **돌릴 수 있는 파일** 첫 자리. 껍데기가 찾는 차례를 그대로 따른다.
///
/// **디렉터리는 건너뛴다** — `moai` 라는 디렉터리가 앞자리에 있으면 껍데기도 그것을 안 쓴다.
fn on_path(name: &str) -> Option<std::path::PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|d| d.join(name)).find(|p| runnable(p))
}

#[cfg(unix)]
fn runnable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn runnable(p: &Path) -> bool {
    p.is_file()
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
        (name.as_str(), "moai issues.jsonl — 이슈마다 3-way"),
        (recursive.as_str(), "binary"),
        (key.as_str(), driver.as_str()),
    ] {
        crate::git::run(here, &["config", "--local", k, v]).map_err(|e| e.to_string())?;
    }
    Ok(driver)
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
/// **안 걸어 둔 저장소에는 안 심는다.** 선언을 읽는 자는 [`declared`] 하나고, 그래서 moai-47bt
/// 의 탈출구(`.gitattributes` 에 `-merge` 를 적는 것)가 여기에도 그대로 선다.
pub(crate) fn plant_for_init(root: &Path) -> Planting {
    if !declared(root) {
        return Planting::Off;
    }
    let cmd = match chosen_command(root) {
        Ok(c) => c,
        Err(why) => return Planting::Failed(why),
    };
    let want = driver_command(&cmd);
    // 이미 같은 줄이면 아무것도 안 쓴다 — `init` 은 한 일을 한 대로 말한다.
    if crate::git::run(root, &["config", "--local", "--get", &driver_key()]).is_ok_and(|v| v.trim() == want) {
        return Planting::Already;
    }
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
    match notice_at(here, tracker, chdir) {
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
        None => chosen_command(&here).map_err(Fail::new)?,
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
    Ok(vec![
        format!("{key} = {driver}"),
        format!("`.gitattributes` 의 `{SNAPSHOT} merge={DRIVER}` 가 이것을 부른다 — 없으면 `moai init` 이 넣는다"),
        "클론마다 한 번씩 친다. 안 친 클론은 git 의 기본 머지가 돈다".into(),
        // **적은 자리가 사라져도 조용히 잃지는 않는다** — 심는 줄이 `git merge-file` 로
        // 내려앉으므로(`driver_command`) 최악이 안 심은 클론과 같아진다. 그래도 자리는
        // 지키는 편이 낫다: 내려앉은 판은 이슈마다 푼 것을 못 쓰고 사람 손으로 간다.
        // 딸린 워크트리에서 쳐도 이 줄은 **클론이 함께 쓰는** `.git/config` 에 앉으므로
        // (`--local` 은 공용 자리다), 그 워크트리를 지우면 클론 전체가 그 상태가 된다.
        "적은 자리가 사라지면 git 의 기본 머지로 내려앉는다 — 표식은 서지만 이슈마다 푸는 값은 잃는다".into(),
        "워크트리의 `target/` 을 가리키면 그 워크트리를 지울 때 같이 죽는다. 그때는 다시 치거나 `--as <늘 있는 자리>` 로 심는다".into(),
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

    /// **못 읽은 줄이 두 쪽에서 다르면 통째로 넘긴다** — 그 줄을 3-way 로 볼 자가 없다.
    /// 같은 id 가 두 번 있는 것도 같다: 그때는 id 로 짝짓는 것 자체가 거짓이다.
    #[test]
    fn what_cannot_be_keyed_hands_the_whole_file_over() {
        let twice = format!("{}\n{}\n", line("argos-0001", ""), line("argos-0001", ""));
        assert!(keyed(&twice).is_none(), "같은 id 두 줄");
        let o = format!("{}\n", line("argos-0001", ""));
        let a = format!("{o}{{이쪽이 못 읽는 줄\n");
        let b = format!("{o}{{저쪽이 못 읽는 줄\n");
        let (text, clashes) = plan(&o, &a, &b, 7);
        assert_eq!(clashes, ["(파일 전체)"], "{text}");
        assert!(text.contains("이쪽이 못 읽는 줄") && text.contains("저쪽이 못 읽는 줄"), "{text}");

        let (text, clashes) = whole("a\n", "b\n", 7);
        assert_eq!(clashes, ["(파일 전체)"]);
        assert_eq!(text, "<<<<<<< ours\na\n=======\nb\n>>>>>>> theirs\n", "{text}");
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
