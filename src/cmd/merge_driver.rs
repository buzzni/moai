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
//! ## 못 하는 것은 통째로 넘긴다
//!
//! 어느 쪽이든 못 읽는 줄이 있거나 한 파일 안에 같은 id 가 두 번 있으면 id 로 짝지을
//! 수가 없다. 그때는 세 파일을 통째로 충돌 표식 안에 넣고 사람에게 준다 — 읽은 것만
//! 골라 쓰면 못 읽은 줄이 그 자리에서 사라진다.

use super::{Ctx, Fail, R};
use crate::cli::MergeDriverArgs;
use crate::model::Issue;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// git 설정에 심는 이름. `.gitattributes` 의 `merge=moai` 가 이것을 가리킨다.
pub const DRIVER: &str = "moai";

/// 한 이슈를 푼 결과.
enum Settled {
    /// 이 줄로 쓴다. 지워진 것이면 `None`.
    Line(Option<Issue>),
    /// 사람이 푼다. 두 쪽의 줄을 그대로 든다 — 고쳐 적으면 사람이 보는 글이
    /// 제가 친 것과 달라진다.
    Clash { ours: Option<String>, theirs: Option<String> },
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
    let read = |p: &Path| {
        std::fs::read_to_string(p).map_err(|e| Fail::new(format!("{}: {e}", p.display())))
    };
    // **셋 다 먼저 읽는다.** `ours` 는 답을 쓸 자리이기도 해서, 쓰기 시작한 뒤에
    // theirs 를 못 읽으면 지운 것도 안 쓴 것도 아닌 파일이 남는다.
    let (o, a, b) = (read(base)?, read(ours)?, read(theirs)?);
    let marker = args.marker_size.unwrap_or(7).max(1);
    let what = args.path.as_deref().unwrap_or(".moai/issues.jsonl");

    let merged = match (keyed(&o), keyed(&a), keyed(&b)) {
        (Some(o), Some(a), Some(b)) => by_issue(&o, &a, &b, marker),
        // id 로 못 짝짓는다 — 통째로 넘긴다.
        _ => Some(whole(&a, &b, marker)),
    };
    let (text, clashes) = match merged {
        Some((text, clashes)) => (text, clashes),
        None => return Err(Fail::new("병합 결과를 짓지 못했다")),
    };
    std::fs::write(ours, &text).map_err(|e| Fail::new(format!("{}: {e}", ours.display())))?;

    if ctx.json {
        #[derive(serde::Serialize)]
        struct Out<'a> {
            path: &'a str,
            conflicts: &'a [String],
        }
        let out = super::json_line(&Out { path: what, conflicts: &clashes })?;
        return match clashes.is_empty() {
            true => Ok(out),
            // 종료 코드가 git 에게 "충돌" 이다. JSON 은 이미 찍었다.
            false => Err(clash_fail(&clashes, what)),
        };
    }
    if !clashes.is_empty() {
        return Err(clash_fail(&clashes, what));
    }
    Ok(Vec::new())
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

/// 파일 하나를 id → 줄로. **못 읽는 줄이 하나라도 있거나 같은 id 가 두 번 있으면
/// `None`** — 그때는 id 로 짝짓는 것 자체가 거짓이 된다.
fn keyed(src: &str) -> Option<BTreeMap<String, Value>> {
    let src = src.strip_prefix('\u{feff}').unwrap_or(src);
    let mut out = BTreeMap::new();
    for line in src.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(line).ok()?;
        // 읽히기는 해도 이슈가 아닌 줄(배열·수·id 없는 객체)은 못 짝짓는다.
        let id = v.get("id")?.as_str()?.to_string();
        // 줄이 이슈로 읽히는지도 여기서 본다 — 안 읽히면 쓰는 자리에서 잃는다.
        serde_json::from_value::<Issue>(v.clone()).ok()?;
        if out.insert(id, v).is_some() {
            return None;
        }
    }
    Some(out)
}

/// id 마다 3-way 로 풀고, 푼 것과 못 푼 것을 한 파일로 짓는다.
fn by_issue(
    o: &BTreeMap<String, Value>,
    a: &BTreeMap<String, Value>,
    b: &BTreeMap<String, Value>,
    marker: usize,
) -> Option<(String, Vec<String>)> {
    let ids: BTreeSet<&String> = o.keys().chain(a.keys()).chain(b.keys()).collect();
    let mut out = String::new();
    let mut clashes = Vec::new();
    for id in ids {
        match settle(o.get(id), a.get(id), b.get(id)) {
            Settled::Line(None) => {}
            Settled::Line(Some(i)) => {
                out.push_str(&serde_json::to_string(&i).ok()?);
                out.push('\n');
            }
            Settled::Clash { ours, theirs } => {
                clashes.push(id.clone());
                out.push_str(&marked(ours.as_deref(), theirs.as_deref(), marker));
            }
        }
    }
    Some((out, clashes))
}

/// 못 짝지을 때. **읽은 것만 골라 쓰지 않는다** — 못 읽은 줄이 그 자리에서 사라진다.
fn whole(a: &str, b: &str, marker: usize) -> (String, Vec<String>) {
    let strip = |s: &str| s.strip_suffix('\n').unwrap_or(s).to_string();
    (marked(Some(&strip(a)), Some(&strip(b)), marker), vec!["(파일 전체)".into()])
}

/// git 이 쓰는 것과 같은 모양의 충돌 표식. 한쪽이 지운 것이면 그 칸이 빈다.
fn marked(ours: Option<&str>, theirs: Option<&str>, marker: usize) -> String {
    let mut s = String::new();
    let bar = |c: char| c.to_string().repeat(marker);
    s.push_str(&format!("{} ours\n", bar('<')));
    if let Some(l) = ours {
        s.push_str(l);
        s.push('\n');
    }
    s.push_str(&format!("{}\n", bar('=')));
    if let Some(l) = theirs {
        s.push_str(l);
        s.push('\n');
    }
    s.push_str(&format!("{} theirs\n", bar('>')));
    s
}

/// 이슈 하나를 푼다.
///
/// 지우기는 git 의 3-way 와 같은 뜻으로 읽는다 — 한쪽이 지우고 다른 쪽이 안 건드렸으면
/// 지운 것이고, 지운 쪽과 고친 쪽이 맞서면 사람이 푼다. **둘 다 새로 세운 같은 id** 는
/// 내용이 같을 때만 지나간다: 랜덤 id 가 부딪친 것이라면 두 이슈는 서로 다른 일이고,
/// 하나를 골라 주면 나머지 하나가 통째로 사라진다.
fn settle(o: Option<&Value>, a: Option<&Value>, b: Option<&Value>) -> Settled {
    let line = |v: &Value| serde_json::to_string(v).unwrap_or_default();
    let clash = |a: Option<&Value>, b: Option<&Value>| Settled::Clash {
        ours: a.map(&line),
        theirs: b.map(&line),
    };
    match (o, a, b) {
        (_, None, None) => Settled::Line(None),
        // 한쪽만 건드렸다.
        (o, Some(x), None) | (o, None, Some(x)) if o == Some(x) => Settled::Line(None),
        (None, Some(x), None) | (None, None, Some(x)) => issue(x).map_or_else(|| clash(a, b), |i| Settled::Line(Some(i))),
        // 지운 쪽과 고친 쪽이 맞선다.
        (Some(_), _, None) | (Some(_), None, _) => clash(a, b),
        (o, Some(x), Some(y)) => {
            if x == y {
                return issue(x).map_or_else(|| clash(a, b), |i| Settled::Line(Some(i)));
            }
            if o == Some(x) {
                return issue(y).map_or_else(|| clash(a, b), |i| Settled::Line(Some(i)));
            }
            if o == Some(y) {
                return issue(x).map_or_else(|| clash(a, b), |i| Settled::Line(Some(i)));
            }
            // **둘 다 새로 세운 같은 id 는 안 섞는다** — 서로 다른 일이다.
            let Some(o) = o else { return clash(a, b) };
            match fields(o, x, y) {
                Some(v) => issue(&v).map_or_else(|| clash(a, b), |i| Settled::Line(Some(i))),
                None => clash(a, b),
            }
        }
    }
}

/// 쓰기 전에 이슈로 한 번 읽는다 — **읽기는 관대하고 쓰기는 엄하다.** 여기서 안 읽히는
/// 줄을 내보내면 그 줄은 다음 `moai` 가 못 읽는 줄로 만나고, 병합이 그것을 만든 것이 된다.
///
/// 표준형으로 맞춘 뒤 돌려준다. 맞추지 않으면 병합 직후의 파일이 다음 쓰기에서 통째로
/// 헛 diff 를 낸다(멱등성).
fn issue(v: &Value) -> Option<Issue> {
    let mut i: Issue = serde_json::from_value(v.clone()).ok()?;
    i.normalize();
    Some(i)
}

/// 둘 다 고친 줄을 **필드마다** 3-way 로 푼다. 한 필드라도 못 풀면 `None` — 줄 하나가
/// 이슈 하나라, 반만 푼 줄은 뜻이 없다.
fn fields(o: &Value, a: &Value, b: &Value) -> Option<Value> {
    let (o, a, b) = (o.as_object()?, a.as_object()?, b.as_object()?);
    let keys: BTreeSet<&String> = o.keys().chain(a.keys()).chain(b.keys()).collect();
    let mut out = Map::new();
    for k in keys {
        let (ov, av, bv) = (o.get(k), a.get(k), b.get(k));
        let picked = match (ov, av, bv) {
            _ if av == bv => av.cloned(),
            _ if ov == av => bv.cloned(),
            _ if ov == bv => av.cloned(),
            // 둘 다 다르게 바꿨다.
            _ => settle_field(k, ov, av, bv)?,
        };
        if let Some(v) = picked {
            out.insert(k.clone(), v);
        }
    }
    Some(Value::Object(out))
}

/// 둘 다 다르게 바꾼 필드의 규칙. **아는 것만 푼다** — 모르면 `None` 이고, 그러면
/// 사람이 푼다. 여기에 필드를 더하는 것은 "이 필드는 말없이 골라도 잃는 것이 없다" 를
/// 주장하는 일이라, 그 까닭을 함께 적는다.
fn settle_field(key: &str, o: Option<&Value>, a: Option<&Value>, b: Option<&Value>) -> Option<Option<Value>> {
    match key {
        // **집합이라 정확히 3-way 로 푼다.** 합집합으로 두면 한쪽이 뺀 태그가 되살아나고,
        // 되살아난 태그는 아무 자취가 없다. 더한 것은 더하고 뺀 것은 뺀다.
        "tags" | "blocked_by" => {
            let set = |v: Option<&Value>| -> Option<BTreeSet<String>> {
                match v {
                    None => Some(BTreeSet::new()),
                    Some(v) => v.as_array()?.iter().map(|x| Some(x.as_str()?.to_string())).collect(),
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
            Some((!out.is_empty()).then(|| Value::Array(out)))
        }
        // 마지막으로 쓴 때. 둘 다 썼으면 늦은 쪽이 참이다.
        "updated_at" | "done_at" => later(a, b, true),
        // **처음 때는 이른 쪽이다.** `started_at` 은 한 번 적고 안 덮는 값이고
        // (moai-38mh), `created_at` 이 갈린 것은 같은 id 를 두 번 세운 자국이라
        // 이른 쪽이 그 줄의 나이다. `status_since` 는 여기 올 때 두 쪽의 칸이 이미
        // 같으므로(다르면 위에서 충돌이다) 같은 칸에 먼저 든 때가 참이다.
        "created_at" | "started_at" | "status_since" => later(a, b, false),
        // **미루기는 둘을 한 쌍으로 고른다**(moai-l11z) — `planned_at` 이 미루거나 도로
        // 집은 때라, 늦게 친 쪽의 `deferred_at` 이 지금 상태다. 따로 고르면 "미뤘는데
        // 미룬 때가 없다" 같은 줄이 나온다. 그 쌍을 고르는 자리는 `deferred_at` 이
        // 아니라 여기 하나다.
        "planned_at" => later(a, b, true),
        _ => None,
    }
}

/// 두 시각 중 늦은/이른 쪽. 시각은 RFC3339 문자열이라 사전순이 곧 시간순이다.
/// 한쪽이 없으면 있는 쪽이다 — 없는 시각은 "모른다" 이지 "0" 이 아니다.
fn later(a: Option<&Value>, b: Option<&Value>, latest: bool) -> Option<Option<Value>> {
    match (a.and_then(Value::as_str), b.and_then(Value::as_str)) {
        (Some(x), Some(y)) => {
            let pick = if (x > y) == latest { x } else { y };
            Some(Some(Value::String(pick.to_string())))
        }
        (Some(x), None) => Some(Some(Value::String(x.to_string()))),
        (None, Some(y)) => Some(Some(Value::String(y.to_string()))),
        (None, None) => Some(None),
    }
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
    let driver = format!("{cmd} merge-driver %O %A %B %L %P");
    let name = format!("merge.{DRIVER}.name");
    let key = format!("merge.{DRIVER}.driver");
    // **저장소 지역 설정에 적는다.** 사람의 전역 설정에 남기면 이 저장소를 지운 뒤에도
    // 없는 바이너리를 가리키는 줄이 남는다. 자리는 지금 선 곳이 정한다 — git 이 제
    // 규칙으로 저장소를 찾으니, 저장소 밖이면 git 이 제 말로 거절한다.
    let here = std::env::current_dir().map_err(|e| Fail::new(e.to_string()))?;
    for (k, v) in [(name.as_str(), "moai issues.jsonl — 이슈마다 3-way"), (key.as_str(), driver.as_str())] {
        crate::git::run(&here, &["config", "--local", k, v]).map_err(|e| Fail::new(e.to_string()))?;
    }
    if ctx.json {
        #[derive(serde::Serialize)]
        struct Out<'a> {
            driver: &'a str,
            attribute: String,
        }
        return super::json_line(&Out {
            driver: &driver,
            attribute: format!(".moai/issues.jsonl merge={DRIVER}"),
        });
    }
    Ok(vec![
        format!("{key} = {driver}"),
        format!("`.gitattributes` 의 `.moai/issues.jsonl merge={DRIVER}` 가 이것을 부른다 — 없으면 `moai init` 이 넣는다"),
        "클론마다 한 번씩 친다. 안 친 클론은 git 의 기본 머지가 돈다".into(),
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

    fn merge(o: &str, a: &str, b: &str) -> (String, Vec<String>) {
        let (o, a, b) = (keyed(o).unwrap(), keyed(a).unwrap(), keyed(b).unwrap());
        by_issue(&o, &a, &b, 7).unwrap()
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

    /// **못 읽는 줄이 있으면 id 로 안 짝짓는다.** 읽은 것만 골라 쓰면 못 읽은 줄이
    /// 그 자리에서 사라진다 — 그것이 이 도구가 못 견디는 바로 그 손실이다.
    #[test]
    fn an_unreadable_line_hands_the_whole_file_over() {
        assert!(keyed("{망가진 줄\n").is_none());
        assert!(keyed(&format!("{}\n{}\n", line("argos-0001", ""), line("argos-0001", ""))).is_none(), "같은 id 두 줄");
        let (text, clashes) = whole("a\n", "b\n", 7);
        assert_eq!(clashes, ["(파일 전체)"]);
        assert!(text.contains("<<<<<<< ours\na\n=======\nb\n>>>>>>> theirs\n"), "{text}");
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
}
