//! svn 의 `--xml` 을 읽는 작은 판독기(moai-0d0s.gst).
//!
//! **크레이트를 들이지 않는다** — svn 이 내는 XML 은 요소·속성·글과 다섯 개체, 숫자 참조뿐이다.
//! DTD·사용자 개체·CDATA 는 읽지 않는다: `<!DOCTYPE …>` 는 통째로 건너뛰고, 그 안에서 정의한
//! 개체는 **펼치지 않고** 글자 그대로 둔다. 서버가 보낸 글이라 잘려 오거나 모르는 꼴이어도
//! 패닉하지 않는다 — 읽은 데까지 닫아 돌려준다.

/// 요소 하나. 글과 자식 요소를 차례대로 든다.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Node {
    Element(Element),
    Text(String),
}

impl Element {
    /// 이 이름의 속성 값 — 순서에 기대지 않는다(svn 은 매번 순서를 바꿔 낸다).
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    /// 이 이름의 바로 밑 자식들.
    pub fn children<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Element> + 'a {
        self.children.iter().filter_map(move |n| match n {
            Node::Element(e) if e.name == name => Some(e),
            _ => None,
        })
    }

    /// 이 이름의 첫 자식.
    pub fn child(&self, name: &str) -> Option<&Element> {
        self.children.iter().find_map(|n| match n {
            Node::Element(e) if e.name == name => Some(e),
            _ => None,
        })
    }

    /// 밑에 든 글 전부를 이어 붙인 것.
    pub fn text(&self) -> String {
        let mut out = String::new();
        self.collect(&mut out);
        out
    }

    fn collect(&self, out: &mut String) {
        for n in &self.children {
            match n {
                Node::Text(t) => out.push_str(t),
                Node::Element(e) => e.collect(out),
            }
        }
    }
}

/// 글 전체를 읽어 이름 없는 뿌리 요소 밑에 담는다. 잘린 글은 읽은 데까지 닫는다.
pub(crate) fn parse(text: &str) -> Element {
    let mut stack: Vec<Element> = vec![Element::default()];
    let mut rest = text;
    while !rest.is_empty() {
        let Some(at) = rest.find('<') else {
            push_text(&mut stack, rest);
            break;
        };
        push_text(&mut stack, &rest[..at]);
        rest = &rest[at..];
        if let Some(after) = rest.strip_prefix("<?") {
            rest = skip_past(after, "?>");
        } else if let Some(after) = rest.strip_prefix("<!--") {
            rest = skip_past(after, "-->");
        } else if let Some(after) = rest.strip_prefix("<![CDATA[") {
            // 읽지 않는 꼴이다 — 글로 들이지 않고 건너뛴다.
            rest = skip_past(after, "]]>");
        } else if let Some(after) = rest.strip_prefix("<!") {
            rest = skip_declaration(after);
        } else if let Some(after) = rest.strip_prefix("</") {
            let (name, next) = take_name(after);
            rest = skip_past(next, ">");
            close(&mut stack, name);
        } else {
            let Some((element, closed, next)) = open_tag(&rest[1..]) else {
                // 머리가 다 오기 전에 끊겼다 — 여기서 멈춘다.
                break;
            };
            rest = next;
            if closed {
                attach(&mut stack, element);
            } else {
                stack.push(element);
            }
        }
    }
    while stack.len() > 1 {
        let e = stack.pop().expect("more than the root");
        attach(&mut stack, e);
    }
    stack.pop().unwrap_or_default()
}

fn push_text(stack: &mut [Element], raw: &str) {
    if raw.is_empty() {
        return;
    }
    if let Some(top) = stack.last_mut() {
        top.children.push(Node::Text(unescape(raw)));
    }
}

fn attach(stack: &mut [Element], e: Element) {
    if let Some(top) = stack.last_mut() {
        top.children.push(Node::Element(e));
    }
}

/// 닫는 꼬리. 이름이 열린 요소의 어느 것과 맞으면 그것까지 닫고, 아무것과도 안 맞으면 버린다.
fn close(stack: &mut Vec<Element>, name: &str) {
    let Some(depth) = stack.iter().rposition(|e| e.name == name) else { return };
    if depth == 0 {
        return;
    }
    while stack.len() > depth {
        let e = stack.pop().expect("deeper than depth");
        attach(stack, e);
    }
}

/// `end` 를 지난 자리. 없으면 빈 글 — 잘린 글의 끝이다.
fn skip_past<'a>(s: &'a str, end: &str) -> &'a str {
    s.find(end).map_or("", |i| &s[i + end.len()..])
}

/// `<!DOCTYPE … [ … ]>` 처럼 대괄호를 품은 선언을 건너뛴다.
fn skip_declaration(s: &str) -> &str {
    let mut depth = 0usize;
    for (i, c) in s.char_indices() {
        match c {
            '[' => depth += 1,
            ']' => depth = depth.saturating_sub(1),
            '>' if depth == 0 => return &s[i + 1..],
            _ => {}
        }
    }
    ""
}

fn is_name(c: char) -> bool {
    !(c.is_whitespace() || matches!(c, '>' | '/' | '=' | '<' | '"' | '\''))
}

fn take_name(s: &str) -> (&str, &str) {
    let end = s.find(|c: char| !is_name(c)).unwrap_or(s.len());
    (&s[..end], &s[end..])
}

/// 여는 머리 `name a="1" b='2'>` 나 `…/>` 를 읽는다. 끝까지 안 왔으면 `None`.
fn open_tag(s: &str) -> Option<(Element, bool, &str)> {
    let (name, mut rest) = take_name(s);
    let mut element = Element { name: name.to_owned(), ..Element::default() };
    loop {
        rest = rest.trim_start();
        if let Some(after) = rest.strip_prefix("/>") {
            return Some((element, true, after));
        }
        if let Some(after) = rest.strip_prefix('>') {
            return Some((element, false, after));
        }
        let c = rest.chars().next()?;
        if !is_name(c) {
            // 모르는 글자 하나 — 건너뛰고 계속 읽는다.
            rest = &rest[c.len_utf8()..];
            continue;
        }
        let (key, after) = take_name(rest);
        rest = after.trim_start();
        let Some(after) = rest.strip_prefix('=') else {
            element.attrs.push((key.to_owned(), String::new()));
            continue;
        };
        rest = after.trim_start();
        let quote = rest.chars().next()?;
        if quote == '"' || quote == '\'' {
            let body = &rest[1..];
            let end = body.find(quote)?;
            element.attrs.push((key.to_owned(), unescape(&body[..end])));
            rest = &body[end + 1..];
        } else {
            let end = rest.find(|c: char| c.is_whitespace() || c == '>' || c == '/').unwrap_or(rest.len());
            element.attrs.push((key.to_owned(), unescape(&rest[..end])));
            rest = &rest[end..];
        }
    }
}

/// 다섯 개체와 숫자 참조만 푼다. 모르는 개체는 글자 그대로 둔다 — DTD 가 정의한 것이라도.
pub(crate) fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let Some(end) = rest[1..].find(';').map(|i| i + 1).filter(|&i| i <= 12) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let name = &rest[1..end];
        let known = match name {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => name
                .strip_prefix("#x")
                .or_else(|| name.strip_prefix("#X"))
                .map(|h| u32::from_str_radix(h, 16))
                .or_else(|| name.strip_prefix('#').map(str::parse::<u32>))
                .and_then(Result::ok)
                .and_then(char::from_u32),
        };
        match known {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entities_numbers_and_korean_survive() {
        let doc = parse(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<log><logentry revision=\"7\"><msg>a &amp; b &lt;c&gt; &quot;q&quot; &apos;s&apos;&#10;둘째 줄 &#x41;</msg></logentry></log>",
        );
        let entry = doc.child("log").and_then(|l| l.child("logentry")).expect("entry");
        assert_eq!(entry.attr("revision"), Some("7"));
        assert_eq!(entry.child("msg").map(Element::text).as_deref(), Some("a & b <c> \"q\" 's'\n둘째 줄 A"));
    }

    #[test]
    fn attributes_in_any_order_and_quotes() {
        for text in [
            "<path copyfrom-path=\"/trunk\" copyfrom-rev=\"2\" action=\"A\">/branches/x</path>",
            "<path\n   action='A'\n   copyfrom-rev=\"2\"\n   copyfrom-path=\"/trunk\">/branches/x</path>",
        ] {
            let doc = parse(text);
            let p = doc.child("path").expect("path");
            assert_eq!(p.attr("copyfrom-path"), Some("/trunk"));
            assert_eq!(p.attr("copyfrom-rev"), Some("2"));
            assert_eq!(p.attr("action"), Some("A"));
            assert_eq!(p.text(), "/branches/x");
        }
    }

    #[test]
    fn paths_with_escaped_names() {
        let doc =
            parse("<status><target path=\".\"><entry path=\"a &amp; b/&quot;한글&quot;.txt\"/></target></status>");
        let entry = doc.child("status").and_then(|s| s.child("target")).and_then(|t| t.child("entry"));
        assert_eq!(entry.and_then(|e| e.attr("path")), Some("a & b/\"한글\".txt"));
    }

    #[test]
    fn truncated_and_odd_input_never_panics() {
        let full = "<?xml version=\"1.0\"?><info><entry path=\"한글\" revision=\"3\"><url>file:///r/트렁크</url><!-- c --></entry></info>";
        // 글자 경계마다 잘라 본다 — 어디서 끊겨도 패닉하지 않는다.
        for (i, _) in full.char_indices() {
            let _ = parse(&full[..i]);
        }
        let cut = parse("<info><entry path=\"x\" revision=\"3\"><url>file:///r");
        let entry = cut.child("info").and_then(|i| i.child("entry")).expect("entry closes");
        assert_eq!(entry.child("url").map(Element::text).as_deref(), Some("file:///r"));
        for odd in [
            "<",
            "</",
            "<a",
            "<a b",
            "<a b=",
            "<a b=\"",
            "&",
            "&#;",
            "&#xZZ;",
            "&#1114112;",
            "<a></b></a>",
            "</x>",
            "<![CDATA[x",
        ] {
            let _ = parse(odd);
        }
    }

    #[test]
    fn user_entities_are_not_expanded() {
        let doc = parse("<!DOCTYPE log [<!ENTITY boom \"BOOM\">]><log><msg>&boom;&amp;</msg></log>");
        assert_eq!(doc.child("log").and_then(|l| l.child("msg")).map(Element::text).as_deref(), Some("&boom;&"));
    }
}
