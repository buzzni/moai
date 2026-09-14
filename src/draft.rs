//! `moai add --from` 이 받는 마크다운을 읽는다. **순수 함수다** — 파일도
//! 터미널도 모른다.
//!
//! 사람이 "좋다" 고 한 순간 에이전트가 에픽 하나와 이슈 여섯을 **한 번의
//! 호출로** 만들 수 있어야 한다. 일곱 번 따로 부르면 에이전트가 중간에
//! 흘리고, 사람은 그 일곱 줄의 출력을 보며 지친다.
//!
//! 형식은 규칙 넷뿐이다.
//!
//! ```text
//! # 에픽 제목                     `#` 줄은 에픽
//! - [p1] 이슈 제목 #bug           `-` 줄은 바로 위 에픽의 이슈
//! - 이슈 제목                     [pN] 과 #태그 는 없어도 된다
//! - \[WIP] 제목 \#12              `[`·끝의 `#낱말` 이 제목이면 `\` 를 앞에 둔다
//! ```
//!
//! **이스케이프는 세 자리뿐이다** — 앞머리 `\[` 와 끝쪽에 이어진 `\#낱말`(moai-a5pz), 그리고 템플릿
//! 변수로 안 읽힐 글자 `\{{`(moai-xqxu). 오타(`[P1]`·`[x]`)는 조용히 제목이 되지 않고 전처럼 줄
//! 번호와 거절한다.
//!
//! **JSON 입력은 두지 않는다.** 에이전트가 Bash heredoc 안에서 중첩 JSON 을
//! 이스케이프하는 것이 마크다운을 쓰는 것보다 훨씬 잘 깨진다. 그리고 사람도
//! 이 마크다운은 손으로 쓴다.

use crate::model::{Issue, Kind, normalize_tag};

#[derive(Debug, PartialEq)]
pub struct Draft {
    pub kind: Kind,
    pub title: String,
    pub priority: Option<u8>,
    pub tags: Vec<String>,
    /// 몇 번째 에픽에 속하는가. 에픽 자신은 `None`.
    pub epic: Option<usize>,
}

/// 줄 끝의 `#태그` 들과 앞머리의 `[pN]` 을 떼어 낸다.
///
/// 태그는 **끝에서부터** 뗀다 — 제목 가운데의 `#` 은 제목의 일부다
/// (`--json 이 #1 에서 깨진다`).
fn split_parts(raw: &str, n: usize) -> Result<(String, Option<u8>, Vec<String>), String> {
    let mut rest = raw.trim();
    let mut priority = None;

    if let Some(after) = rest.strip_prefix('[') {
        let (tag, tail) = after
            .split_once(']')
            .ok_or_else(|| format!("{n}줄: `[` 를 닫지 않았다 — {raw:?}"))?;
        let p = tag
            .trim()
            .strip_prefix('p')
            .and_then(|d| d.parse::<u8>().ok())
            .filter(|p| *p <= crate::model::MAX_PRIORITY)
            .ok_or_else(|| {
                format!("{n}줄: `[{tag}]` 는 우선순위가 아니다. `[p0]`~`[p{}]`", crate::model::MAX_PRIORITY)
            })?;
        priority = Some(p);
        rest = tail.trim();
    }
    // `\[` 로 시작하면 우선순위가 아니라 제목의 `[` 다(moai-a5pz) — `[pN]` 을 뗀 **뒤에** 본다.
    // 떼는 것은 역슬래시 하나뿐이다.
    if rest.starts_with("\\[") {
        rest = &rest[1..];
    }

    let mut tags = Vec::new();
    while let Some((head, last)) = rest.rsplit_once(char::is_whitespace) {
        match last.strip_prefix('#') {
            Some(t) if !t.is_empty() => {
                tags.push(normalize_tag(t));
                rest = head.trim_end();
            }
            _ => break,
        }
    }
    tags.reverse();

    if rest.is_empty() {
        return Err(format!("{n}줄: 제목이 없다 — {raw:?}"));
    }
    // 제목 끝쪽에 이어진 `\#낱말` 은 태그가 아니라 제목의 `#낱말` 이다(moai-a5pz). 가운데
    // 것은 원래 태그로 안 읽히므로 이스케이프도 안 하고 풀지도 않는다 — render 와 같은 자리.
    let title = map_trailing_words(rest, |w| w.strip_prefix("\\#").filter(|t| !t.is_empty()).map(|t| format!("#{t}")));
    Ok((title, priority, tags))
}

/// 끝쪽에 이어진 낱말을 `f` 가 `None` 을 낼 때까지 바꾼다 — 이스케이프의 두 짝이 같이 쓴다.
///
/// 낱말은 **태그 고리와 같은 `char::is_whitespace` 로 가른다.** `' '` 로만 가르면
/// `메모\u{3000}#12` 의 끝 `#12` 를 render 는 낱말로 안 보고 parse 는 태그로 먹는다.
/// 가름자는 그대로 둔다 — 빈 낱말(겹친 공백)은 `f` 가 거절해 멈춘다.
fn map_trailing_words(s: &str, f: impl Fn(&str) -> Option<String>) -> String {
    let pieces: Vec<&str> = s.split_inclusive(char::is_whitespace).collect();
    let mut keep = pieces.len();
    let mut tail = Vec::new();
    for p in pieces.iter().rev() {
        let word = p.strip_suffix(char::is_whitespace).unwrap_or(p);
        match f(word) {
            Some(w) => tail.push(format!("{w}{}", &p[word.len()..])),
            None => break,
        }
        keep -= 1;
    }
    pieces[..keep].concat() + &tail.into_iter().rev().collect::<String>()
}

/// 템플릿의 `{{이름}}` 을 `vars` 로 채운다(moai-3faw). [`parse`] **앞에서** 글을 한 번 바꿀 뿐이라
/// 형식의 규칙은 채운 뒤의 글에 그대로 선다.
///
/// - 이름은 영문·숫자·`_`·`-` 만이고 `{{` 와 `}}` 사이에 빈칸이 없다. **그 모양이 아닌 `{{…}}` 는
///   변수가 아니라 글자다** — `{{ 빈칸 }}` 같은 글이 멀쩡한 계획을 거절시키지 않게
/// - **변수는 선언 없이 전부 필수다**(사람이 정했다). 못 채운 이름과 계획에 없는 `--var` 는
///   **전부** 대며 거절한다 — 반만 채운 계획이 조용히 만들어지거나 오타가 넘어가지 않게.
///   [`parse`] 처럼 한 번에 다 말한다
/// - 치환은 한 번뿐이다 — 값 안의 `{{…}}` 는 다시 펴지 않는다
/// - 변수도 `vars` 도 없으면 글을 그대로 돌려준다 — 여느 계획은 안 바뀐다
/// - **`\{{` 는 글자 `{{` 다**(moai-xqxu) — 역슬래시를 떼고 변수로 세지 않는다. [`render`] 가 제목의
///   `{{` 를 그렇게 써서, 되뽑은 계획이 채우지 않은 변수로 거절되지 않는다
/// - **값은 늘 제목 글자다**(사람이 정했다, 리뷰 moai-cypw.nn4) — 값이 제목 앞머리에 오면 `[` 를,
///   제목 끝쪽에 오면 `#낱말` 을 이스케이프해 넣어 우선순위·태그를 못 바꾼다([`escape_value`])
pub fn fill(src: &str, vars: &[(String, String)]) -> Result<String, String> {
    let is_name = |n: &str| !n.is_empty() && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    let mut out = String::with_capacity(src.len());
    let (mut missing, mut used): (Vec<&str>, Vec<&str>) = (Vec::new(), Vec::new());
    let mut rest = src;
    while let Some(open) = rest.find("{{") {
        if rest[..open].ends_with('\\') {
            out.push_str(&rest[..open - 1]);
            out.push_str("{{");
            rest = &rest[open + 2..];
            continue;
        }
        out.push_str(&rest[..open]);
        let after = &rest[open + 2..];
        match after.find("}}").map(|end| &after[..end]).filter(|n| is_name(n)) {
            Some(name) => {
                match vars.iter().find(|(k, _)| k == name) {
                    Some((_, value)) => {
                        let before = out.rsplit('\n').next().unwrap_or("");
                        let line_rest = after[name.len() + 2..].split('\n').next().unwrap_or("");
                        out.push_str(&escape_value(value, before, line_rest));
                        if !used.contains(&name) {
                            used.push(name);
                        }
                    }
                    None if !missing.contains(&name) => missing.push(name),
                    None => {}
                }
                rest = &after[name.len() + 2..];
            }
            // `{` 하나만 글자로 넘기고 다음 `{` 부터 다시 본다 — `{{` 를 통째로 넘기면
            // `{{{a}}}` 의 `{{a}}` 가 변수로 안 읽혀, 준 `--var a` 가 "계획에 없는 변수" 로 거절된다.
            None => {
                out.push('{');
                rest = &rest[open + 1..];
            }
        }
    }
    out.push_str(rest);

    let mut unknown: Vec<&str> = Vec::new();
    for (k, _) in vars {
        if !used.contains(&k.as_str()) && !unknown.contains(&k.as_str()) {
            unknown.push(k);
        }
    }
    let names = |v: &[&str]| v.iter().map(|n| format!("`{n}`")).collect::<Vec<_>>().join("·");
    let mut errors = Vec::new();
    if !missing.is_empty() {
        errors.push(format!("채우지 않은 변수 {} — `--var 이름=값` 으로 준다", names(&missing)));
    }
    if !unknown.is_empty() {
        errors.push(format!("계획에 없는 변수 {} — 이름이 맞는지 본다", names(&unknown)));
    }
    if !errors.is_empty() {
        return Err(errors.join("\n      "));
    }
    Ok(out)
}

/// `--var` 값을 **제목 글자로** 넣게 이스케이프한다(moai-xqxu). `before` 는 같은 줄에서 값 앞의 글,
/// `after` 는 값 뒤의 글이다.
///
/// - 값이 **제목 앞머리**에 오면(`- `·`* `·`# ` 글머리와, 있으면 `[pN]` 뒤) 앞의 `[` 를 `\[` 로 —
///   [`split_parts`] 가 우선순위로 읽는 자리다
/// - 값이 **제목 끝쪽**에 오면(뒤가 `#태그` 낱말뿐) 값의 끝 `#낱말` 들을 `\#낱말` 로 — 태그로 먹히는
///   자리다. 가운데 오는 값은 원래 태그로 안 읽혀 그대로 둔다
///
/// 형식의 이스케이프(`\[`·`\#`, moai-a5pz)를 그대로 쓴다 — 값만을 위한 문법을 따로 짓지 않는다.
fn escape_value(value: &str, before: &str, after: &str) -> String {
    let head = before.trim_start();
    let head = head.strip_prefix("- ").or_else(|| head.strip_prefix("* ")).or_else(|| head.strip_prefix("# "));
    let at_start = head.is_some_and(|h| {
        let h = h.trim_start();
        let h = match h.strip_prefix('[').and_then(|x| x.split_once(']')) {
            Some((p, tail)) if p.trim().strip_prefix('p').is_some_and(|d| d.parse::<u8>().is_ok()) => tail,
            _ => h,
        };
        h.trim().is_empty()
    });
    let at_end = after
        .split_whitespace()
        .all(|w| w.strip_prefix('#').is_some_and(|t| !t.is_empty()));
    // 값 앞뒤의 빈칸은 split_parts 가 깎는다 — 판정은 빈칸을 뗀 알맹이로 한다. 안 떼면 ` [p0] x` 가
    // `[` 로 시작하지 않는 것으로, `x #inj  ` 의 끝 낱말이 빈 낱말로 보여 이스케이프를 비껴간다.
    let core = value.trim();
    let lead = &value[..value.len() - value.trim_start().len()];
    let trail = &value[lead.len() + core.len()..];
    // 값 앞이 빈칸 없이 붙은 글이면 값의 첫 낱말은 앞 글과 한 낱말이라 태그로 안 읽힌다 —
    // 거기에 `\` 를 넣으면 `끝\#a` 처럼 제목에 역슬래시가 샌다.
    let glued = lead.is_empty() && !before.is_empty() && !before.ends_with(char::is_whitespace);
    let mut v = if at_end {
        let split = if glued { core.find(char::is_whitespace).unwrap_or(core.len()) } else { 0 };
        let (stuck, free) = core.split_at(split);
        stuck.to_string()
            + &map_trailing_words(free, |w| w.strip_prefix('#').filter(|t| !t.is_empty()).map(|t| format!("\\#{t}")))
    } else {
        core.to_string()
    };
    if at_start && v.starts_with('[') {
        v.insert(0, '\\');
    }
    format!("{lead}{v}{trail}")
}

/// 못 읽은 줄은 **전부** 모아 한 번에 말한다. 하나씩 고치게 하면 여섯 줄짜리
/// heredoc 을 여섯 번 다시 보낸다.
pub fn parse(src: &str) -> Result<Vec<Draft>, String> {
    let mut out: Vec<Draft> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    let mut epic: Option<usize> = None;

    for (i, line) in src.lines().enumerate() {
        let n = i + 1;
        let l = line.trim();
        if l.is_empty() {
            continue;
        }
        let (kind, body) = match l.strip_prefix("- ").or_else(|| l.strip_prefix("* ")) {
            Some(b) => (Kind::Issue, b),
            None => match l.strip_prefix("# ") {
                Some(b) => (Kind::Epic, b),
                None => {
                    errors.push(format!(
                        "{n}줄: `# 에픽` 도 `- 이슈` 도 아니다 — {l:?}"
                    ));
                    continue;
                }
            },
        };
        if kind == Kind::Issue && epic.is_none() {
            errors.push(format!(
                "{n}줄: 어느 에픽의 이슈인지 알 수 없다. 위에 `# 에픽 제목` 을 둔다 — {l:?}"
            ));
            continue;
        }
        match split_parts(body, n) {
            Err(e) => errors.push(e),
            Ok((title, priority, tags)) => {
                if kind == Kind::Epic {
                    epic = Some(out.len());
                }
                out.push(Draft {
                    kind,
                    title,
                    priority,
                    tags,
                    epic: (kind == Kind::Issue).then_some(epic).flatten(),
                });
            }
        }
    }

    if !errors.is_empty() {
        return Err(errors.join("\n      "));
    }
    if out.is_empty() {
        return Err("읽을 것이 없다. `# 에픽 제목` 과 `- 이슈 제목` 을 적는다".into());
    }
    Ok(out)
}

/// [`parse`] 의 반대. 에픽 하나와 그 멤버를 `add --from` 이 받는 마크다운으로
/// 되뽑는다 (`moai show <에픽> --as-plan`).
///
/// **이 파일에 둔다.** 형식을 아는 자리가 둘이 되면 되뽑은 것을 도로 넣을 때
/// 한쪽만 바뀐 형식에서 깨진다 — 짝을 같은 파일에 두고 되돌려 읽는 시험으로 묶는다.
///
/// - 형식이 받는 것만 낸다: 제목·`[pN]`·`#태그`. 막음(`blocked_by`)·담당·본문·
///   부모는 형식에 자리가 없어 빠진다 — 자식 이슈는 형제로 평평하게 선다. 없는 문법을 지어내면 `add --from` 이 그 줄을
///   거부해 되뽑은 것이 도로 안 들어간다 (막음은 moai-quxo 의 단계 의존과 함께)
/// - 우선순위는 **적힌 것만** 낸다. 기본값을 채워 쓰면 템플릿이 설정의 기본값을
///   박제한다
/// - 멤버는 받은 차례 그대로 적는다 — 차례를 정하는 것은 부르는 쪽이다
/// - 제목은 [`crate::text::one_line`] 을 지난다. 형식은 한 줄에 하나라 손으로 고친
///   파일의 줄바꿈이 든 제목은 다음 줄을 딴 이슈로 만들고, 제어문자는 이 글을
///   그대로 찍는 터미널을 다시 칠한다
pub fn render(epic: &Issue, members: &[&Issue]) -> String {
    std::iter::once(format!("# {}\n", body(epic)))
        .chain(members.iter().map(|m| format!("- {}\n", body(m))))
        .collect()
}

/// 글머리(`#`·`-`)를 뗀 한 줄 — [`split_parts`] 가 읽는 바로 그 자리.
///
/// **제목은 parse 가 달리 읽을 자리만 이스케이프한다**(moai-a5pz) — 앞머리 `[` 는 `\[`,
/// 끝쪽에 이어진 `#낱말` 은 `\#낱말`. 가운데 `#` 은 원래 태그로 안 읽혀 그대로 둔다.
fn body(i: &Issue) -> String {
    let mut s = String::new();
    if let Some(p) = i.priority {
        s.push_str(&format!("[p{p}] "));
    }
    // 제목의 `{{` 는 `\{{` 로 쓴다(moai-xqxu) — `add --from` 은 형식을 읽기 전에 채우기(`fill`)를 지나므로,
    // 안 쓰면 제목의 `{{version}}` 같은 글자가 채우지 않은 변수로 거절된다. 왼쪽부터 겹치지 않게
    // 바꾸면 `{{{` 도 `\{{{` 가 되어 채운 뒤 제자리로 돌아온다.
    let title = crate::text::one_line(&i.title).replace("{{", "\\{{");
    let title = map_trailing_words(&title, |w| {
        w.strip_prefix('#').filter(|t| !t.is_empty()).map(|t| format!("\\#{t}"))
    });
    if title.starts_with('[') {
        s.push('\\');
    }
    s.push_str(&title);
    // 태그의 `{{` 도 제목과 같이 `\{{` 로 쓴다(리뷰 moai-xqxu.fc3) — 안 쓰면 `#{{a}}` 가 채우지 않은
    // 변수로 거절돼 되뽑은 계획이 도로 안 들어간다.
    for t in &i.tags {
        s.push_str(&format!(" #{}", t.replace("{{", "\\{{")));
    }
    s
}

/// [`render`] 가 쓴 줄을 [`parse`] 가 **다르게 읽는** 줄의 id.
///
/// 앞머리 `[` 와 끝의 `#낱말` 은 이제 이스케이프로 도로 들어간다(moai-a5pz). 남는 것은
/// 이스케이프로도 못 담는 제목 — 원래 역슬래시로 시작하거나 끝의 `#낱말` 앞에 역슬래시를
/// 든 제목 같은 것이다. 견주는 제목은 `one_line` 을 지난 것이라 줄바꿈은 안 짚는다. **조용히 틀리지 않고 이름을 댄다**(moai-nb8d.j61). 되뽑은
/// 글은 사람이 다듬는 틀이라 거절하지는 않는다 — 안전망으로 남긴다(사람이 정했다).
///
/// 판정은 짝이 직접 한다 — 쓴 줄을 도로 읽어 견준다. 규칙을 따로 적으면 parse 가
/// 바뀌는 날 이쪽만 옛 규칙으로 남는다.
pub fn lossy<'a>(epic: &'a Issue, members: &[&'a Issue]) -> Vec<&'a str> {
    std::iter::once(epic)
        .chain(members.iter().copied())
        .filter(|i| {
            // `add --from` 이 도로 읽는 길 그대로 — 채우기(빈 변수)를 지난 뒤 형식을 읽는다(moai-xqxu).
            fill(&body(i), &[]).ok().and_then(|b| split_parts(&b, 0).ok())
                != Some((crate::text::one_line(&i.title), i.priority, i.tags.clone()))
        })
        .map(|i| i.id.as_str())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(src: &str) -> Vec<Draft> {
        parse(src).unwrap()
    }

    fn issue(title: &str, kind: Kind, priority: Option<u8>, tags: &[&str]) -> Issue {
        let mut i = Issue::new("x-1".into(), title.into(), kind, crate::model::Status::new("todo"), "2026-09-14T00:00:00Z");
        i.priority = priority;
        i.tags = tags.iter().map(|t| t.to_string()).collect();
        i
    }

    /// **되뽑은 것은 도로 들어가야 한다.** 이 짝이 어긋나면 `--as-plan` 은
    /// 템플릿이 아니라 손으로 고쳐야 쓸 수 있는 글이다.
    #[test]
    fn what_render_writes_parse_reads_back() {
        let epic = issue("릴리스", Kind::Epic, Some(1), &["release"]);
        let a = issue("--json 이 #1 에서 깨진다", Kind::Issue, Some(0), &["bug", "parser"]);
        let b = issue("태그도 우선순위도 없다", Kind::Issue, None, &[]);
        let md = render(&epic, &[&a, &b]);
        assert_eq!(md, "# [p1] 릴리스 #release\n- [p0] --json 이 #1 에서 깨진다 #bug #parser\n- 태그도 우선순위도 없다\n");
        let got = one(&md);
        assert_eq!(got[0], Draft { kind: Kind::Epic, title: "릴리스".into(), priority: Some(1), tags: vec!["release".into()], epic: None });
        assert_eq!(got[1], Draft { kind: Kind::Issue, title: a.title.clone(), priority: Some(0), tags: vec!["bug".into(), "parser".into()], epic: Some(0) });
        assert_eq!(got[2], Draft { kind: Kind::Issue, title: b.title.clone(), priority: None, tags: vec![], epic: Some(0) });
    }

    /// 손으로 고친 파일의 제목에 줄바꿈이나 ESC 가 들어도 한 줄에 하나로 선다.
    #[test]
    fn a_title_that_spans_lines_stays_one_line() {
        let epic = issue("가", Kind::Epic, None, &[]);
        let bad = issue("앞\n뒤\u{1b}[2J", Kind::Issue, None, &[]);
        let md = render(&epic, &[&bad]);
        assert_eq!(md, "# 가\n- 앞  뒤[2J\n");
        assert_eq!(one(&md).len(), 2, "줄바꿈이 이슈를 하나 더 만들었다");
    }

    /// **앞머리 `[` 와 끝의 `#낱말` 을 든 제목도 도로 들어간다**(moai-a5pz). 형식에
    /// 이스케이프가 없던 때는 `[WIP] 반쯤` 이 우선순위로 읽혀 거부되고 `이슈 #12` 는 끝
    /// 낱말이 태그로 먹혔다(리뷰 moai-nb8d.j61). render 는 필요한 자리만 `\[`·`\#` 로 쓰고
    /// parse 가 푼다 — render → parse 가 같은 계획이다.
    #[test]
    fn titles_with_a_leading_bracket_or_trailing_hashes_round_trip() {
        let epic = issue("[릴리스] 준비 #2", Kind::Epic, Some(2), &["release"]);
        let wip = issue("[WIP] 반쯤", Kind::Issue, None, &[]);
        let hash = issue("이슈 #12 를 고친다 #12", Kind::Issue, Some(1), &["bug"]);
        let two = issue("끝이 #a #b", Kind::Issue, None, &[]);
        let lone = issue("#12", Kind::Issue, None, &["x"]);
        let middle = issue("--json 이 #1 에서 깨진다", Kind::Issue, Some(0), &["bug"]);
        let all = [&wip, &hash, &two, &lone, &middle];
        let md = render(&epic, &all);
        assert_eq!(
            md,
            "# [p2] \\[릴리스] 준비 \\#2 #release\n- \\[WIP] 반쯤\n- [p1] 이슈 #12 를 고친다 \\#12 #bug\n- 끝이 \\#a \\#b\n- \\#12 #x\n- [p0] --json 이 #1 에서 깨진다 #bug\n",
            "가운데 # 은 이스케이프하지 않는다"
        );
        let got = one(&md);
        for (d, i) in got.iter().zip(std::iter::once(&epic).chain(all)) {
            assert_eq!((d.title.as_str(), d.priority, d.tags.as_slice()), (i.title.as_str(), i.priority, i.tags.as_slice()), "{md}");
        }
        assert!(lossy(&epic, &all).is_empty(), "되돌아 읽히는데 lossy 가 짚었다");

        // 태그 고리는 `' '` 가 아닌 공백(전각·NBSP)에서도 낱말을 가른다 — 이스케이프도 같은 자리를 본다.
        let wide = issue("메모\u{3000}#12", Kind::Issue, None, &["x"]);
        let nbsp = issue("[a]\u{a0}#1 #2", Kind::Issue, Some(1), &[]);
        assert_eq!(render(&epic, &[&wide, &nbsp]).lines().skip(1).collect::<Vec<_>>(), ["- 메모\u{3000}\\#12 #x", "- [p1] \\[a]\u{a0}\\#1 \\#2"]);
        assert!(lossy(&epic, &[&wide, &nbsp]).is_empty(), "전각 공백 뒤의 #낱말 을 태그로 먹었다");
    }

    /// 사람이 손으로 쓸 때도 같은 이스케이프를 받고, **오타는 여전히 거절한다** — `[P1]`
    /// 이 조용히 제목이 되면 우선순위를 잃은 것을 아무도 모른다(사람이 정했다, moai-a5pz).
    #[test]
    fn escapes_are_read_by_hand_and_typos_are_still_refused() {
        // 가운데 `#` 은 원래 태그로 안 읽혀 이스케이프가 필요 없다 — 끝쪽의 `\#` 만 푼다.
        let got = one("# 가\n- \\[x] 체크박스처럼 #todo\n- 우선 #1 과 \\#2\n- [p1] \\[p2] 는 제목\n");
        assert_eq!((got[1].title.as_str(), got[1].priority, got[1].tags.as_slice()), ("[x] 체크박스처럼", None, &["todo".to_string()][..]));
        assert_eq!((got[2].title.as_str(), got[2].tags.len()), ("우선 #1 과 #2", 0));
        assert_eq!((got[3].title.as_str(), got[3].priority), ("[p2] 는 제목", Some(1)), "우선순위 뒤의 이스케이프를 못 풀었다");
        let e = parse("# 가\n- [P1] 대문자\n- [x] 체크박스\n").unwrap_err();
        assert!(e.contains("2줄") && e.contains("3줄"), "오타를 제목으로 받았다 — {e}");
    }

    /// 이스케이프로도 못 담는 제목은 **여전히 이름이 불린다** — lossy 는 짝이 도로 읽어
    /// 견주는 안전망이다(사람이 정했다). 원래 역슬래시로 시작하는 제목은 이스케이프를 푸는
    /// 규칙과 부딪친다. 줄바꿈은 `one_line` 한 줄로 견주므로 여기서 안 짚는다 —
    /// `a_title_that_spans_lines_stays_one_line` 이 따로 본다.
    #[test]
    fn titles_even_escaping_cannot_hold_are_still_named() {
        let epic = issue("가", Kind::Epic, None, &[]);
        let mut slash = issue("\\[이미 역슬래시]", Kind::Issue, None, &[]);
        slash.id = "x-slash".into();
        let mut fine = issue("[WIP] 반쯤", Kind::Issue, None, &[]);
        fine.id = "x-fine".into();
        assert_eq!(lossy(&epic, &[&slash, &fine]), ["x-slash"]);
    }

    /// 멤버가 없는 에픽도 도로 들어가는 계획이다 — 에픽 줄 하나.
    #[test]
    fn an_empty_epic_is_one_line() {
        let md = render(&issue("빈 에픽", Kind::Epic, None, &[]), &[]);
        assert_eq!(md, "# 빈 에픽\n");
        assert_eq!(one(&md).len(), 1);
    }

    fn vars(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    /// **템플릿의 `{{이름}}` 은 `--var` 로 채운다**(moai-3faw). 채운 글은 여느 계획처럼 읽힌다.
    #[test]
    fn variables_are_filled_before_the_plan_is_read() {
        let tpl = "# 릴리스 {{version}}\n- [p1] {{version}} 태그를 단다 #release\n- {{channel}} 채널에 올린다\n";
        let got = fill(tpl, &vars(&[("version", "1.2"), ("channel", "stable")])).unwrap();
        assert_eq!(got, "# 릴리스 1.2\n- [p1] 1.2 태그를 단다 #release\n- stable 채널에 올린다\n");
        assert_eq!(one(&got)[1].title, "1.2 태그를 단다");
        // 값 안의 `{{…}}` 는 다시 펴지 않는다 — 한 번뿐이다.
        assert_eq!(fill("# {{a}}\n", &vars(&[("a", "{{a}}")])).unwrap(), "# {{a}}\n");
        // 겹친 중괄호 안의 이름도 변수다 — `{{{a}}}` 의 `{{a}}` 를 놓치면 준 `--var a` 가 거절된다.
        assert_eq!(fill("# {{{a}}} {{{{a}}}}\n", &vars(&[("a", "1")])).unwrap(), "# {1} {{1}}\n");
        // 닫히지 않은 `{{` 는 글자고, 그 뒤의 변수는 그대로 읽는다. 여러 바이트 글자 사이에서도.
        assert_eq!(fill("# 가{{나 {{a}}다{{\n", &vars(&[("a", "값")])).unwrap(), "# 가{{나 값다{{\n");
    }

    /// 변수가 없고 `--var` 도 없으면 글은 그대로다 — 여느 계획이 안 바뀐다. 이름 모양이 아닌
    /// `{{…}}` 는 변수가 아니라 글자다.
    #[test]
    fn a_plan_without_variables_is_untouched() {
        let plain = "# 가\n- --json 이 #1 에서 깨진다 #bug\n- {{ 빈칸 }} 과 {{}} 와 {{a b}} 는 글자\n";
        assert_eq!(fill(plain, &[]).unwrap(), plain);
    }

    /// **못 채운 변수와 파일에 없는 `--var` 는 이름을 전부 대며 거절한다**(사람이 정했다) — 반만
    /// 채운 계획이 조용히 만들어지거나 오타가 넘어가지 않게.
    #[test]
    fn missing_and_unknown_variables_are_refused_by_name() {
        let tpl = "# {{version}}\n- {{channel}} 과 {{version}} 과 {{owner}}\n";
        let e = fill(tpl, &vars(&[("version", "1")])).unwrap_err();
        assert!(e.contains("channel") && e.contains("owner"), "빠진 이름을 다 안 댔다 — {e}");
        assert_eq!(e.matches("version").count(), 0, "채운 것까지 댔다 — {e}");
        let e = fill(tpl, &vars(&[("version", "1"), ("channel", "c"), ("owner", "o"), ("verison", "2")])).unwrap_err();
        assert!(e.contains("verison"), "파일에 없는 --var 를 안 댔다 — {e}");
        assert!(fill("# 변수 없음\n", &vars(&[("x", "1")])).is_err(), "쓸 곳 없는 --var 를 받았다");
    }

    /// **`\{{` 는 변수가 아니라 글자 `{{` 다**(moai-xqxu) — 제목에 `{{이름}}` 문법 자체를 적을 수 있게.
    /// 변수로 세지 않으니 채우라고 거절하지도 않는다.
    #[test]
    fn an_escaped_brace_pair_is_text_not_a_variable() {
        assert_eq!(fill("# \\{{version}} 문법을 적는다\n", &[]).unwrap(), "# {{version}} 문법을 적는다\n");
        assert_eq!(
            fill("# {{v}} 과 \\{{v}}\n", &vars(&[("v", "1")])).unwrap(),
            "# 1 과 {{v}}\n",
            "이스케이프한 자리까지 채웠다"
        );
    }

    /// **되뽑은 계획의 제목에 든 `{{` 는 `\{{` 로 쓴다**(moai-xqxu) — 도로 넣을 때 채우지 않은 변수로
    /// 거절되지 않는다. 채우기를 지나 읽어도 같은 제목이다.
    #[test]
    fn titles_with_braces_round_trip_through_fill() {
        let epic = issue("{{version}} 문법", Kind::Epic, None, &[]);
        // 태그의 `{{` 도 되돌아온다(리뷰 moai-xqxu.fc3).
        let a = issue("겹친 {{{a}}} 와 {{ 빈칸 }}", Kind::Issue, Some(1), &["x", "{{t}}"]);
        let b = issue("{{", Kind::Issue, None, &[]);
        let md = render(&epic, &[&a, &b]);
        assert!(md.starts_with("# \\{{version}} 문법\n"), "{md}");
        let filled = fill(&md, &[]).expect("되뽑은 계획이 변수로 거절됐다");
        let got = one(&filled);
        for (d, i) in got.iter().zip([&epic, &a, &b]) {
            assert_eq!((d.title.as_str(), d.priority, d.tags.as_slice()), (i.title.as_str(), i.priority, i.tags.as_slice()), "{md}");
        }
        assert!(lossy(&epic, &[&a, &b]).is_empty(), "되돌아 읽히는데 lossy 가 짚었다");
    }

    /// **`--var` 값은 늘 제목 글자다**(사람이 정했다, 리뷰 moai-cypw.nn4) — 값이 우선순위나 태그를
    /// 바꾸지 못한다. 우선순위와 태그는 템플릿 파일에서만 정한다.
    #[test]
    fn a_value_cannot_set_priority_or_tags() {
        let tpl = "# 에픽 {{e}}\n- {{p}}\n- [p2] {{p}}\n- 끝에 {{t}}\n- 끝에 {{t}} #release\n- 가운데 {{t}} 뒤\n";
        let filled = fill(tpl, &vars(&[("e", "#1"), ("p", "[p0] 몰래"), ("t", "x #injected")])).unwrap();
        let got = one(&filled);
        let seen: Vec<(&str, Option<u8>, &[String])> = got.iter().map(|d| (d.title.as_str(), d.priority, d.tags.as_slice())).collect();
        assert_eq!(
            seen,
            [
                ("에픽 #1", None, &[][..]),
                ("[p0] 몰래", None, &[][..]),
                ("[p0] 몰래", Some(2), &[][..]),
                ("끝에 x #injected", None, &[][..]),
                ("끝에 x #injected", None, &["release".to_string()][..]),
                ("가운데 x #injected 뒤", None, &[][..]),
            ],
            "{filled}"
        );
        // 값 앞뒤의 빈칸이 이스케이프를 비껴가지 않고, 앞 글에 붙은 값에는 역슬래시가 새지 않는다.
        let filled = fill(
            "# e\n- {{a}}\n- {{b}}\n- 끝 {{c}}\n- 끝{{d}}\n",
            &vars(&[("a", " [p0] 몰래"), ("b", "\u{3000}[p0] 몰래"), ("c", "x #inj  "), ("d", "#a #b")]),
        )
        .unwrap();
        let got = one(&filled);
        let seen: Vec<(&str, Option<u8>, &[String])> = got.iter().map(|d| (d.title.as_str(), d.priority, d.tags.as_slice())).collect();
        assert_eq!(
            seen,
            [
                ("e", None, &[][..]),
                ("[p0] 몰래", None, &[][..]),
                ("[p0] 몰래", None, &[][..]),
                ("끝 x #inj", None, &[][..]),
                ("끝#a #b", None, &[][..]),
            ],
            "{filled}"
        );
    }

    #[test]
    fn reads_an_epic_and_its_issues() {
        let got = one("# 저장 계층\n- [p1] 원자적으로 쓴다 #enhancement\n- 잘린 줄을 복구한다 #bug\n");
        assert_eq!(got.len(), 3);
        assert_eq!(got[0], Draft { kind: Kind::Epic, title: "저장 계층".into(), priority: None, tags: vec![], epic: None });
        assert_eq!(got[1], Draft { kind: Kind::Issue, title: "원자적으로 쓴다".into(), priority: Some(1), tags: vec!["enhancement".into()], epic: Some(0) });
        assert_eq!(got[2].epic, Some(0));
    }

    /// 에픽이 둘이면 그다음 이슈는 **뒤엣것**에 붙는다.
    #[test]
    fn a_second_epic_takes_over() {
        let got = one("# 가\n- 첫째\n# 나\n- 둘째\n");
        assert_eq!(got[1].epic, Some(0));
        assert_eq!(got[3].epic, Some(2));
    }

    #[test]
    fn tags_come_off_the_end_only() {
        let got = one("# 가\n- --json 이 #1 에서 깨진다 #bug #parser\n");
        assert_eq!(got[1].title, "--json 이 #1 에서 깨진다", "제목 가운데의 # 을 태그로 봤다");
        assert_eq!(got[1].tags, ["bug", "parser"]);
    }

    #[test]
    fn tags_fold_like_everywhere_else() {
        assert_eq!(one("# 가\n- 제목 #Bug\n")[1].tags, ["bug"]);
    }

    #[test]
    fn blank_lines_and_star_bullets_are_fine() {
        let got = one("\n# 가\n\n* 별표도 받는다\n\n");
        assert_eq!(got.len(), 2);
        assert_eq!(got[1].title, "별표도 받는다");
    }

    /// 못 읽은 줄은 전부 모아 한 번에 말한다.
    #[test]
    fn every_refusal_names_its_line() {
        let e = parse("# 가\n이건 뭔가\n- [p9] 너무 큼\n- [열림] 숫자가 아님\n- \n").unwrap_err();
        for want in ["2줄", "3줄", "4줄", "5줄"] {
            assert!(e.contains(want), "{want} 가 없다 — {e}");
        }
    }

    /// 에픽 없이 시작하면 거부한다. 이 도구는 계획 없이 쌓이는 것을 막으려고
    /// 있고, 대량 생성이야말로 그 일이 제일 잘 일어나는 자리다.
    #[test]
    fn an_issue_without_an_epic_is_refused() {
        let e = parse("- 그냥 하나\n").unwrap_err();
        assert!(e.contains("어느 에픽의") && e.contains("1줄"), "{e}");
    }

    #[test]
    fn nothing_to_read_says_so() {
        assert!(parse("\n\n").unwrap_err().contains("읽을 것이 없다"));
    }
}
