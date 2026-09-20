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
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// git 설정에 심는 이름. `.gitattributes` 의 `merge=moai` 가 이것을 가리킨다.
pub const DRIVER: &str = "moai";

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
    let what = args.path.as_deref().unwrap_or(".moai/issues.jsonl");

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
    std::fs::write(ours, &text).map_err(|e| Fail::new(format!("{}: {e}", ours.display())))?;

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
    Fail::coded(
        format!(
            "{what}: {}건은 사람이 푼다 — {}",
            clashes.len(),
            clashes.join(" ")
        ),
        super::code::BROKEN,
    )
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
    let clash = || Settled::Clash {
        ours: a.map(|r| r.raw.to_string()),
        theirs: b.map(|r| r.raw.to_string()),
    };
    let keep = |v: &Value| {
        issue(v).map_or_else(clash, |i| {
            Settled::Line(Some(serde_json::to_string(&i).expect("Issue 는 언제나 직렬화된다")))
        })
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
/// 그 두 상태를 같게 만든다. 세 마디다.
///
/// 1. 드라이버가 0 으로 끝나면 그것으로 끝이다.
/// 2. 비영인데 `%A` 에 표식이 있으면 **답을 못 지어 사람에게 넘긴 것**이므로 그대로 비영이다.
/// 3. 비영인데 표식이 없으면 그 명령은 답을 안 쓴 것이다 — `git merge-file` 로 다시 합친다.
///    이 길은 표식을 남기므로, 최악이 "조용한 손실" 에서 "안 심은 클론과 같음" 으로 내려온다.
///
/// 표식을 `<<<<<<<` 일곱 자로 찾는 것은 git 의 기본값이다. 저장소가 `conflict-marker-size` 를
/// 그보다 짧게 잡아 못 찾으면 3번으로 내려가는데, 그것도 표식이 서는 길이라 잃는 것은 없다.
///
/// 경로는 `shell_word` 로 감싼다 — 빈칸이 든 경로는 첫 낱말에서 끊기고, 그때 껍데기가 내는
/// 것은 1번도 2번도 아닌 3번이다. `%A %O %B` 는 git 이 제가 지은 임시 파일 이름으로 바꾸므로
/// 그대로 둔다. `%P` 는 git 이 이미 따옴표로 싸서 넣으니 덧싸지 않는다.
fn driver_command(cmd: &str) -> String {
    let q = crate::text::shell_word(cmd);
    format!(
        "if {q} merge-driver %O %A %B %L %P; then exit 0; fi; \
         grep -q '^<<<<<<<' %A && exit 1; \
         exec git merge-file -L ours -L base -L theirs %A %O %B"
    )
}

/// 심어 둔 드라이버가 **못 도는** 상태의 알림(moai-2ewr).
///
/// **안 심은 것은 말하지 않는다.** `moai-w8so` 가 임시 저장소에서 둘을 나란히 쟀다 — 안 심은
/// 클론에서는 `.gitattributes` 의 `merge=moai` 가 그냥 무시되고 git 의 기본 머지가 돌며 표식도
/// 선다. 그 상태를 조르면 드라이버를 안 쓰기로 한 클론을 영영 조르는 셈이다(`agents_stale` 이
/// `missing` 을 안 말하는 것과 같은 까닭).
///
/// 해로운 것은 **심어 놓고 그 명령이 못 도는 자리**다. 사람은 이슈마다 푸는 것이 돈다고 믿는데
/// 실제로는 `driver_command` 의 셋째 마디로 내려앉아 기본 머지가 돌고, 그 사실이 어느 화면에도
/// 안 선다. 이 저장소에서 그 자리는 가깝다 — `--install` 의 기본값은 지금 도는 바이너리의 절대
/// 경로이고, 워크트리에서 치면 그 워크트리의 `target/` 이 적히는데 `--local` 은 클론이 함께
/// 쓰는 자리라 그 워크트리를 지우는 순간 모든 체크아웃이 그 상태가 된다.
///
/// **모르면 입을 다문다.** 설정을 못 읽었거나 적힌 줄이 이 도구가 지은 모양이 아니면 아무 말도
/// 안 한다 — 남이 손으로 적은 줄을 "썩었다" 고 부르면 걷을 길이 없는 알림이 선다.
pub fn notice(root: &Path, chdir: bool) -> Option<crate::report::Warning> {
    let key = format!("merge.{DRIVER}.driver");
    let planted = crate::git::run(root, &["config", "--get", &key]).ok()?;
    let planted = planted.trim();
    if planted.is_empty() {
        return None;
    }
    let word = planted_word(planted)?;
    if runnable(&word) {
        return None;
    }
    Some(crate::report::Warning::merge_driver_rotten(&word, crate::cmd::init::away_root(root, chdir).as_deref()))
}

/// 심어 둔 줄에서 **실제로 부르는 명령**을 떼어 낸다. 모양이 이 도구가 지은 것이 아니면 `None`.
///
/// [`driver_command`] 가 지은 줄은 `if <명령> merge-driver …` 로 선다. 빈칸이 든 경로는
/// `shell_word` 가 홑따옴표로 싸는데, 그 안에 홑따옴표가 또 들면(`'/a/it'\''s'`) 떼어 낸 조각이
/// 실제 경로가 아니다 — 그때는 재지 않는다. `$'…'` 도 같다.
fn planted_word(planted: &str) -> Option<String> {
    let rest = planted.strip_prefix("if ").unwrap_or(planted);
    match rest.strip_prefix('\'') {
        Some(quoted) => {
            let end = quoted.find('\'')?;
            // 이어 붙인 따옴표(`'…'\''…'`)면 여기서 끊은 것이 경로가 아니다.
            quoted[end + 1..].starts_with(' ').then(|| quoted[..end].to_string())
        }
        // `$'…'` 은 풀지 않는다 — 푸는 규칙을 여기 또 쓰면 `shell_word` 와 둘이 어긋난다.
        None if rest.starts_with('$') => None,
        None => rest.split_whitespace().next().map(str::to_string),
    }
}

/// 그 명령을 껍데기가 실제로 부를 수 있는가. 자리에 `/` 가 들면 그 파일을, 아니면 `PATH` 를 본다.
fn runnable(cmd: &str) -> bool {
    if cmd.contains('/') {
        return is_exe(Path::new(cmd));
    }
    let Some(path) = std::env::var_os("PATH") else { return false };
    std::env::split_paths(&path).any(|dir| is_exe(&dir.join(cmd)))
}

fn is_exe(p: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(p) else { return false };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        meta.is_file() && meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    meta.is_file()
}

/// `.git/config` 에 드라이버를 심는다.
///
/// **저장소마다 한 번씩 쳐야 한다** — git 은 드라이버 명령을 설정에서만 읽고 설정은
/// 커밋되지 않는다. 안 심은 클론에서는 `.gitattributes` 의 `merge=moai` 가 그냥
/// 무시되고 git 의 기본 텍스트 머지가 돈다. 즉 **안 심으면 지금까지와 똑같다.**
fn install(ctx: &Ctx, as_command: Option<&str>) -> R<Vec<String>> {
    // 기본은 지금 도는 이 바이너리의 절대 경로다. 이 도구는 전역 설치를 안 하므로
    // `moai` 라고만 적으면 PATH 에 없는 기계에서 조용히 안 돈다. PATH 에 둔 사람은
    // `--as moai` 로 고른다.
    let cmd = match as_command {
        Some(c) => c.to_string(),
        None => std::env::current_exe()
            .map_err(|e| Fail::new(format!("이 바이너리의 자리를 모른다: {e}")))?
            .display()
            .to_string(),
    };
    let driver = driver_command(&cmd);
    let name = format!("merge.{DRIVER}.name");
    let key = format!("merge.{DRIVER}.driver");
    // **속 병합에는 이 드라이버를 쓰지 않는다**(`merge.<이름>.recursive`). 갈래가 엇갈린
    // 이력에서 git 은 공통 조상 여럿을 먼저 합쳐 가상 조상을 짓는데, 그 자리에 이 드라이버를
    // 쓰면 충돌 표식이 `%O` 로 들어와 다음 판에서 못 읽는 줄이 된다 — 그러면 이슈마다 푼 것이
    // 통째로 날아가고 사람이 파일 전체를 손으로 푼다. 세션 여럿이 `develop` 을 서로 받는
    // 저장소에서 엇갈린 이력은 드문 일이 아니다. `binary` 는 표식을 안 남기고 이쪽 것을
    // 그대로 두어 `%O` 가 계속 JSONL 로 읽힌다.
    let recursive = format!("merge.{DRIVER}.recursive");
    // **저장소 지역 설정에 적는다.** 사람의 전역 설정에 남기면 이 저장소를 지운 뒤에도
    // 없는 바이너리를 가리키는 줄이 남는다. 자리는 지금 선 곳이 정한다 — git 이 제
    // 규칙으로 저장소를 찾으니, 저장소 밖이면 git 이 제 말로 거절한다.
    let here = std::env::current_dir().map_err(|e| Fail::new(e.to_string()))?;
    for (k, v) in [
        (name.as_str(), "moai issues.jsonl — 이슈마다 3-way"),
        (key.as_str(), driver.as_str()),
        (recursive.as_str(), "binary"),
    ] {
        crate::git::run(&here, &["config", "--local", k, v]).map_err(|e| Fail::new(e.to_string()))?;
    }
    if ctx.json {
        #[derive(serde::Serialize)]
        struct Planted<'a> {
            driver: &'a str,
            attribute: String,
        }
        return super::json_line(&Planted {
            driver: &driver,
            attribute: format!(".moai/issues.jsonl merge={DRIVER}"),
        });
    }
    Ok(vec![
        format!("{key} = {driver}"),
        format!("`.gitattributes` 의 `.moai/issues.jsonl merge={DRIVER}` 가 이것을 부른다 — 없으면 `moai init` 이 넣는다"),
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
        assert!(
            !text.contains("deferred_at"),
            "늦게 친 쪽은 도로 집었는데 미뤄 둔 채로 섰다\n{text}"
        );
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

    /// **심는 줄은 세 마디다** — 돌면 그것으로 끝, 표식을 남기고 실패했으면 그대로 넘김,
    /// 아무것도 안 쓰고 실패했으면 `git merge-file`. 셋째 마디가 없으면 경로가 썩은 순간
    /// git 이 표식 없는 파일을 남기고, `git add` 한 번에 저쪽이 사라진다.
    #[test]
    fn the_planted_line_falls_back_to_gits_own_merge() {
        let line = driver_command("/w/moai");
        assert!(line.starts_with("if /w/moai merge-driver %O %A %B %L %P; then exit 0; fi;"), "{line}");
        assert!(line.contains("grep -q '^<<<<<<<' %A && exit 1"), "표식이 선 실패를 내려앉혔다\n{line}");
        assert!(line.ends_with("exec git merge-file -L ours -L base -L theirs %A %O %B"), "{line}");
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
        // 홑따옴표가 든 경로는 `shell_query` 가 이어 붙여 싼다 — 떼어 낸 조각이 경로가 아니다.
        assert_eq!(word("/w/it's/moai"), None);
        // 제어문자가 든 경로는 `$'…'` 다. 푸는 규칙을 여기 또 쓰지 않는다.
        assert_eq!(word("/w/a\tb/moai"), None);
        // 이 도구가 지은 모양이 아니면 재지 않는다 — 다만 옛 판(맨 명령)은 그대로 읽는다.
        assert_eq!(planted_word("/w/moai merge-driver %O %A %B %L %P").as_deref(), Some("/w/moai"));
    }

    /// **부를 수 있는지는 파일로 잰다.** 있고 없음이 아니라 실행할 수 있는가다 — 권한을 잃은
    /// 파일은 git 이 부르지 못하고, 그때가 바로 표식 없이 끝나던 자리다.
    #[cfg(unix)]
    #[test]
    fn runnable_reads_the_file_not_just_its_name() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = std::env::temp_dir().join(format!("moai-runnable-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("moai");
        std::fs::write(&p, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(!runnable(&p.display().to_string()), "실행 권한이 없는 파일을 돈다고 했다");
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(runnable(&p.display().to_string()));
        assert!(!runnable(&dir.display().to_string()), "디렉터리를 명령으로 읽었다");
        assert!(!runnable(&dir.join("없다").display().to_string()));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// **빈칸이 든 경로를 감싼다.** 안 감싸면 첫 낱말에서 끊겨 늘 내려앉는 길로만 가고,
    /// 그 저장소는 드라이버를 심고도 안 심은 것과 같아진다.
    #[test]
    fn the_planted_line_quotes_a_path_with_a_space() {
        let line = driver_command("/w/My Work/moai");
        assert!(line.starts_with("if '/w/My Work/moai' merge-driver "), "{line}");
        // 자리표시자는 감싸지 않는다 — git 이 제 임시 파일 이름으로 바꾼다. `%P` 는 git 이 이미 싼다.
        assert!(!line.contains("\"%A\"") && !line.contains("'%A'"), "자리표시자를 덧쌌다\n{line}");
    }
}
