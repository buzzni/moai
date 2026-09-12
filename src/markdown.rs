//! 본문 마크다운을 **표면 중립 블록**으로 접는다.
//!
//! 터미널도 ratatui 도 모른다. `nav` 와 같은 자리의 순수 모듈이라 TTY 없이
//! 시험된다.
//!
//! # 색이 아니라 뜻을 낸다
//!
//! [`Role`] 은 `Strong`·`Code` 같은 **뜻**이지 색이 아니다. 여기서 색을 정하면
//! `anstyle`(CLI)과 ratatui(탐색기) 중 하나를 골라야 하고, 그 순간 이 모듈이
//! 한쪽 표면에 묶인다. 뜻을 색으로 옮기는 일은 표면이 각자 한다 — `색이 혼자
//! 뜻을 지지 않는다`는 규칙도 그쪽에서 지킨다.
//!
//! # 원문을 잃지 않는다
//!
//! 그린 글은 기호가 지워져 되돌릴 수 없다. 본문을 긁어 붙이거나 마크다운을
//! 고쳐야 할 때가 있으므로 표면은 원문 보는 길을 함께 준다 — `--json` 의
//! `body` 는 **언제나 원문**이다.

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

/// 글 한 조각이 지는 뜻.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Plain,
    Strong,
    Emphasis,
    Code,
    /// 링크의 **글**. 주소는 따로 낸다 — 터미널에서 주소는 대개 방해다.
    Link,
    /// 제목. 수준은 들여쓰기가 말한다.
    Heading,
    /// 글머리·인용 막대·가로줄 같은 **표시**. 글이 아니라 짜임새다.
    Mark,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub role: Role,
}

/// 목록의 한 줄. 깊이를 들고 있어 겹친 목록도 한 벌로 그린다 —
/// 재귀 구조로 두면 그리는 쪽이 그 재귀를 두 번 짜야 한다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub depth: u8,
    pub spans: Vec<Span>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Heading { level: u8, spans: Vec<Span> },
    Para(Vec<Span>),
    List { ordered: bool, items: Vec<Item> },
    /// 들여쓴 코드와 ``` 코드 둘 다 여기 온다.
    Code { lang: Option<String>, lines: Vec<String> },
    Quote(Vec<Span>),
    Table { head: Vec<Vec<Span>>, rows: Vec<Vec<Vec<Span>>> },
    Rule,
}

/// 본문을 블록으로. **마크다운이 아닌 글도 그대로 통과한다** — 이 저장소의
/// 본문은 마크다운을 조금 쓰는 산문이지 마크다운 문서가 아니다.
pub fn parse(src: &str) -> Vec<Block> {
    // 표는 크레이트 기능이 아니라 파서 옵션이다. 이 저장소 본문이 표를 쓴다.
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    Fold::default().run(Parser::new_ext(src, opts))
}

/// 이벤트를 받아 블록을 쌓는 자리.
///
/// **상태 기계를 하나만 둔다.** 블록마다 따로 접으면 "지금 굵게인가" 같은 것을
/// 여러 곳이 저마다 세게 되고, 반드시 한 곳이 어긋난다.
#[derive(Default)]
struct Fold {
    out: Vec<Block>,
    /// 지금 모으는 조각들. 문단·제목·인용·표 칸이 모두 여기로 온다.
    spans: Vec<Span>,
    /// 겹친 강조를 센다 — `**굵게 `코드`**` 처럼 겹칠 수 있다.
    strong: u32,
    emphasis: u32,
    link: u32,
    /// 목록은 끝날 때 한 벌로 낸다. 안쪽 목록이 바깥 것을 끊지 않게 한다.
    list: Option<(bool, Vec<Item>)>,
    depth: u8,
    code: Option<(Option<String>, String)>,
    quote: bool,
    heading: Option<u8>,
    table: Option<(Vec<Vec<Span>>, Vec<Vec<Vec<Span>>>)>,
    in_head: bool,
    row: Vec<Vec<Span>>,
}

impl Fold {
    fn run(mut self, events: Parser) -> Vec<Block> {
        for e in events {
            self.one(e);
        }
        self.out
    }

    fn role(&self) -> Role {
        // 겹쳤을 때의 차례: 코드가 가장 세고, 그다음 굵게. 하나만 고를 수 있는
        // 자리라 **더 좁은 뜻**을 남긴다.
        if self.link > 0 {
            Role::Link
        } else if self.strong > 0 {
            Role::Strong
        } else if self.emphasis > 0 {
            Role::Emphasis
        } else {
            Role::Plain
        }
    }

    fn push(&mut self, text: &str, role: Role) {
        if text.is_empty() {
            return;
        }
        // 같은 뜻이 이어지면 한 조각으로 잇는다. 파서는 줄 단위로 쪼개 주는데,
        // 그대로 두면 그리는 쪽이 조각마다 헛되이 칠한다.
        match self.spans.last_mut() {
            Some(last) if last.role == role => last.text.push_str(text),
            _ => self.spans.push(Span { text: text.to_string(), role }),
        }
    }

    fn take(&mut self) -> Vec<Span> {
        std::mem::take(&mut self.spans)
    }

    fn one(&mut self, e: Event) {
        match e {
            Event::Start(t) => self.start(t),
            Event::End(t) => self.end(t),
            Event::Text(t) => match &mut self.code {
                Some((_, buf)) => buf.push_str(&t),
                None => {
                    let role = self.role();
                    self.push(&t, role);
                }
            },
            Event::Code(t) => self.push(&t, Role::Code),
            // 줄바꿈은 한 칸으로. 문단 안의 줄 나눔은 그리는 쪽이 폭을 보고
            // 다시 접으므로, 여기서 원문의 줄을 지키면 두 번 접힌다.
            Event::SoftBreak | Event::HardBreak => {
                let role = self.role();
                self.push(" ", role);
            }
            Event::Rule => self.out.push(Block::Rule),
            _ => {}
        }
    }

    fn start(&mut self, t: Tag) {
        match t {
            Tag::Strong => self.strong += 1,
            Tag::Emphasis => self.emphasis += 1,
            Tag::Link { .. } => self.link += 1,
            Tag::Heading { level, .. } => self.heading = Some(level as u8),
            Tag::BlockQuote(_) => self.quote = true,
            Tag::CodeBlock(kind) => {
                let lang = match kind {
                    CodeBlockKind::Fenced(l) if !l.is_empty() => Some(l.to_string()),
                    _ => None,
                };
                self.code = Some((lang, String::new()));
            }
            Tag::List(first) => match self.list {
                // 안쪽 목록. 바깥 것을 끊지 않고 깊이만 는다.
                //
                // **모으던 글을 먼저 매듭짓는다.** 안 그러면 바깥 줄의 글이
                // `spans` 에 남아 있다가 안쪽 첫 줄에 이어 붙는다 — `- 둘` 과
                // `  - 둘의 속` 이 `둘둘의 속` 한 줄이 되고 바깥 줄은 사라진다.
                Some(_) => {
                    let spans = self.take();
                    let depth = self.depth;
                    if let Some((_, items)) = &mut self.list
                        && !spans.is_empty()
                    {
                        items.push(Item { depth, spans });
                    }
                    self.depth = self.depth.saturating_add(1);
                }
                None => self.list = Some((first.is_some(), Vec::new())),
            },
            Tag::Table(_) => self.table = Some((Vec::new(), Vec::new())),
            Tag::TableHead => self.in_head = true,
            _ => {}
        }
    }

    fn end(&mut self, t: TagEnd) {
        match t {
            TagEnd::Strong => self.strong = self.strong.saturating_sub(1),
            TagEnd::Emphasis => self.emphasis = self.emphasis.saturating_sub(1),
            TagEnd::Link => self.link = self.link.saturating_sub(1),
            TagEnd::Heading(_) => {
                let level = self.heading.take().unwrap_or(1);
                let spans = self.take();
                self.out.push(Block::Heading { level, spans });
            }
            TagEnd::CodeBlock => {
                if let Some((lang, buf)) = self.code.take() {
                    let lines = buf.lines().map(str::to_string).collect();
                    self.out.push(Block::Code { lang, lines });
                }
            }
            TagEnd::Item => {
                let spans = self.take();
                let depth = self.depth;
                if let Some((_, items)) = &mut self.list
                    && !spans.is_empty()
                {
                    items.push(Item { depth, spans });
                }
            }
            TagEnd::List(_) => {
                if self.depth > 0 {
                    self.depth -= 1;
                } else if let Some((ordered, items)) = self.list.take() {
                    self.out.push(Block::List { ordered, items });
                }
            }
            TagEnd::BlockQuote(_) => self.quote = false,
            TagEnd::Paragraph => {
                let spans = self.take();
                if spans.is_empty() {
                    return;
                }
                // 목록 안의 문단은 그 줄의 몫이다 — `Item` 이 받아 간다.
                if self.list.is_some() {
                    self.spans = spans;
                } else if self.quote {
                    self.out.push(Block::Quote(spans));
                } else {
                    self.out.push(Block::Para(spans));
                }
            }
            TagEnd::TableCell => {
                let cell = self.take();
                self.row.push(cell);
            }
            TagEnd::TableHead => {
                self.in_head = false;
                let row = std::mem::take(&mut self.row);
                if let Some((head, _)) = &mut self.table {
                    *head = row;
                }
            }
            TagEnd::TableRow => {
                let row = std::mem::take(&mut self.row);
                if let Some((_, rows)) = &mut self.table
                    && !row.is_empty()
                {
                    rows.push(row);
                }
            }
            TagEnd::Table => {
                if let Some((head, rows)) = self.table.take() {
                    self.out.push(Block::Table { head, rows });
                }
            }
            _ => {}
        }
    }
}

/// 조각들을 폭에 맞춰 접는다. **접힌 줄도 조각의 열이다** — 글자마다 무슨
/// 뜻인지를 잃지 않아야 표면이 칠할 수 있다.
///
/// 넘치면 띄어쓴 자리에서 끊고, 그럴 자리가 없으면 글자에서 끊는다. 한글은
/// 띄어쓰기 없이 길게 이어지고 한 글자가 두 칸이라 낱말 단위로만 접으면 한 줄이
/// 통째로 넘치고, 늘 글자에서 끊으면 영문 낱말이 가운데서 잘린다. 둘 다 본다.
///
/// **접는 것이 칠하는 것보다 먼저다.** 칠한 뒤 접으면 이스케이프가 폭에 세어져
/// 줄이 짧아지고, 끊긴 자리에 색이 열린 채로 남는다.
pub fn wrap_spans(spans: &[Span], max: usize) -> Vec<Vec<Span>> {
    if max == 0 {
        return vec![Vec::new()];
    }
    // 글자마다 뜻을 달아 둔다. 접는 자리는 조각 경계와 무관하게 정해진다.
    let chars: Vec<(char, Role)> = spans
        .iter()
        .flat_map(|s| s.text.chars().map(|c| (c, s.role)))
        .collect();

    let mut lines: Vec<Vec<(char, Role)>> = Vec::new();
    let mut line: Vec<(char, Role)> = Vec::new();
    let mut w = 0usize;
    let mut space: Option<usize> = None;

    for &(c, role) in &chars {
        let cw = crate::text::width(c.encode_utf8(&mut [0u8; 4]));
        if w + cw > max && !line.is_empty() {
            match space {
                Some(at) if at > 0 => {
                    let mut rest: Vec<(char, Role)> = line.split_off(at);
                    while rest.first().is_some_and(|(c, _)| *c == ' ') {
                        rest.remove(0);
                    }
                    trim_end(&mut line);
                    lines.push(std::mem::take(&mut line));
                    w = rest.iter().map(|(c, _)| crate::text::width(c.encode_utf8(&mut [0u8; 4]))).sum();
                    line = rest;
                }
                _ => {
                    lines.push(std::mem::take(&mut line));
                    w = 0;
                }
            }
            space = None;
        }
        if c == ' ' {
            space = Some(line.len());
        }
        line.push((c, role));
        w += cw;
    }
    trim_end(&mut line);
    if !line.is_empty() {
        lines.push(line);
    }
    if lines.is_empty() {
        lines.push(Vec::new());
    }
    // 이어지는 같은 뜻을 한 조각으로 되묶는다.
    lines.into_iter().map(regroup).collect()
}

fn trim_end(line: &mut Vec<(char, Role)>) {
    while line.last().is_some_and(|(c, _)| *c == ' ') {
        line.pop();
    }
}

fn regroup(chars: Vec<(char, Role)>) -> Vec<Span> {
    let mut out: Vec<Span> = Vec::new();
    for (c, role) in chars {
        match out.last_mut() {
            Some(last) if last.role == role => last.text.push(c),
            _ => out.push(Span { text: c.to_string(), role }),
        }
    }
    out
}

// ── 줄로 펴기 ─────────────────────────────────────────────────────────
//
// **여기가 두 표면의 계약이다.** 글머리를 무엇으로 쓸지, 인용을 어떻게 물릴지,
// 코드에 백틱을 남길지는 뜻에 관한 결정이지 색에 관한 결정이 아니다. 표면마다
// 정하면 CLI 와 탐색기가 같은 본문을 다르게 그리고, 둘을 나란히 놓고 보는
// 사람이 어느 쪽을 믿을지 정해야 한다. 색으로 옮기는 일만 표면이 한다.

/// 글머리. 겹친 목록도 같은 것을 쓴다 — 깊이는 들여쓰기가 말한다.
const BULLET: &str = "•";

fn mark(t: impl Into<String>) -> Span {
    Span { text: t.into(), role: Role::Mark }
}

/// 블록들을 폭에 맞춰 **줄**로 편다. 줄 하나는 조각의 열이고, 글머리·막대·
/// 들여쓰기도 조각으로 들어간다. 빈 줄은 빈 열이다.
pub fn layout(blocks: &[Block], width: usize) -> Vec<Vec<Span>> {
    let mut out: Vec<Vec<Span>> = Vec::new();
    for (n, b) in blocks.iter().enumerate() {
        if n > 0 {
            out.push(Vec::new());
        }
        lay_one(&mut out, b, width);
    }
    out
}

fn lay_one(out: &mut Vec<Vec<Span>>, b: &Block, width: usize) {
    match b {
        Block::Heading { level, spans } => {
            // 수준은 **들여쓰기**가 말한다. `#` 을 남기면 걷어낸 보람이 없고,
            // 굵게만으로는 2단계와 3단계가 같아 보인다.
            let indent = "  ".repeat((*level as usize).saturating_sub(1));
            let spans: Vec<Span> = spans
                .iter()
                .map(|s| match s.role {
                    Role::Plain => Span { text: s.text.clone(), role: Role::Heading },
                    _ => s.clone(),
                })
                .collect();
            flow(out, &spans, &indent, &indent, width);
        }
        Block::Para(spans) => flow(out, &marked(spans), "", "", width),
        Block::Quote(spans) => {
            // 인용은 **색이 아니라 세로줄**로 말한다.
            flow(out, &marked(spans), "│ ", "│ ", width);
        }
        Block::List { ordered, items } => {
            for (n, it) in items.iter().enumerate() {
                let pad = "  ".repeat(it.depth as usize);
                let bullet =
                    if *ordered { format!("{}. ", n + 1) } else { format!("{BULLET} ") };
                // 이어지는 줄은 글머리 폭만큼 물려 쓴다 — 안 그러면 둘째 줄이
                // 다음 항목처럼 보인다.
                let hang = format!("{pad}{}", " ".repeat(crate::text::width(&bullet)));
                flow(out, &marked(&it.spans), &format!("{pad}{bullet}"), &hang, width);
            }
        }
        Block::Code { lines, .. } => {
            // 코드는 접지 않는다. 접으면 그 줄이 더는 그 코드가 아니다.
            for l in lines {
                out.push(vec![mark("    "), Span { text: l.clone(), role: Role::Code }]);
            }
        }
        Block::Rule => out.push(vec![mark("─".repeat(width.min(40)))]),
        Block::Table { head, rows } => lay_table(out, head, rows, width),
    }
}

/// 칸 사이. 세로줄은 **표시**라 글과 다른 뜻을 진다.
const CELL_GAP: &str = " │ ";

/// 표를 칸 맞춰 편다.
///
/// 좁아서 다 안 들어가면 **칸을 줄이되 버리지는 않는다.** 오른쪽 칸을 조용히
/// 떨어뜨리면 보는 쪽은 그 칸이 애초에 없는 줄 안다. 줄인 자리에는 `…` 가
/// 남아 잘렸다는 것이 보인다 — 목록의 긴 제목을 다루는 방식과 같다.
fn lay_table(
    out: &mut Vec<Vec<Span>>,
    head: &[Vec<Span>],
    rows: &[Vec<Vec<Span>>],
    width: usize,
) {
    let cols = head.len().max(rows.iter().map(Vec::len).max().unwrap_or(0));
    if cols == 0 {
        return;
    }
    fn cell_at(r: &[Vec<Span>], n: usize) -> &[Span] {
        r.get(n).map(Vec::as_slice).unwrap_or(&[])
    }
    let span_width = |c: &[Span]| c.iter().map(|s| crate::text::width(&s.text)).sum::<usize>();

    // 있는 대로의 폭. 한글이 두 칸이라 **표시 폭**으로 잰다.
    // **표시를 붙인 뒤의 폭**으로 잰다. 백틱을 빼고 재면 칸이 좁게 잡혀
    // 멀쩡한 표가 늘 잘린다.
    let mut w: Vec<usize> = (0..cols)
        .map(|n| {
            std::iter::once(span_width(&marked(cell_at(head, n))))
                .chain(rows.iter().map(|r| span_width(&marked(cell_at(r, n)))))
                .max()
                .unwrap_or(0)
        })
        .collect();

    // 넘치면 **가장 넓은 칸부터** 한 칸씩 줄인다. 좁은 칸(`판`·`1.0`)은 그대로
    // 남으므로, 줄어드는 것은 늘 설명처럼 긴 칸이다.
    let gaps = crate::text::width(CELL_GAP) * (cols - 1);
    let budget = width.saturating_sub(gaps).max(cols);
    while w.iter().sum::<usize>() > budget {
        let Some(widest) = (0..cols).max_by_key(|&n| (w[n], std::cmp::Reverse(n))) else { break };
        if w[widest] <= 1 {
            break;
        }
        w[widest] -= 1;
    }

    let row = |r: &[Vec<Span>], head: bool| -> Vec<Span> {
        let mut line = Vec::new();
        for n in 0..cols {
            if n > 0 {
                line.push(mark(CELL_GAP));
            }
            // 표 안에서도 코드는 백틱을 남긴다 — 칸이 위치를 말해 줄 뿐,
            // 그 글이 코드라는 것은 색만으로는 색을 끈 터미널에서 사라진다.
            let cell = fit(&marked(cell_at(r, n)), w[n], head);
            let pad = w[n].saturating_sub(span_width(&cell));
            line.extend(cell);
            // 마지막 칸은 채우지 않는다 — 오른쪽에 뜻 없는 공백이 남는다.
            if n + 1 < cols && pad > 0 {
                line.push(mark(" ".repeat(pad)));
            }
        }
        line
    };

    out.push(row(head, true));
    // 구분줄. 테두리를 두르지 않는 이 저장소의 표 모양과 같게 가볍게 둔다.
    out.push({
        let mut line = Vec::new();
        for n in 0..cols {
            if n > 0 {
                line.push(mark("─┼─"));
            }
            line.push(mark("─".repeat(w[n])));
        }
        line
    });
    out.extend(rows.iter().map(|r| row(r, false)));
}

/// 칸 하나를 그 폭에 맞춘다. 넘치면 `…` 로 잘린다.
///
/// **잘림 표시는 한 번만 붙인다.** 조각마다 `text::clip` 을 부르면 조각마다
/// `…` 가 붙어 `터미널용 마크다……` 처럼 표시가 겹친다.
fn fit(cell: &[Span], max: usize, head: bool) -> Vec<Span> {
    let as_head = |s: &Span| match (head, s.role) {
        (true, Role::Plain) => Span { text: s.text.clone(), role: Role::Heading },
        _ => s.clone(),
    };
    let total: usize = cell.iter().map(|s| crate::text::width(&s.text)).sum();
    if total <= max {
        return cell.iter().map(as_head).collect();
    }
    // `…` 한 칸을 남겨 두고 폭으로만 자른다.
    let room = max.saturating_sub(1);
    let mut out: Vec<Span> = Vec::new();
    let mut used = 0;
    for s in cell {
        if used >= room {
            break;
        }
        let piece = cut(&s.text, room - used);
        used += crate::text::width(&piece);
        if !piece.is_empty() {
            out.push(Span { text: piece, role: as_head(s).role });
        }
    }
    out.push(mark("…"));
    out
}

/// 표시 폭으로만 자른다 — 표시는 붙이지 않는다.
fn cut(text: &str, max: usize) -> String {
    let mut out = String::new();
    let mut w = 0;
    for c in text.chars() {
        let cw = crate::text::width(c.encode_utf8(&mut [0u8; 4]));
        if w + cw > max {
            break;
        }
        out.push(c);
        w += cw;
    }
    out
}

/// 코드 조각에 백틱을 되돌려 놓는다.
///
/// **색을 끄면 색으로만 표시한 것은 그냥 글이 된다** — `0.2` 가 판인지 숫자인지
/// 구별할 길이 사라진다. 굵게·기울임은 글맛이지 낱말의 정체가 아니라 색과 함께
/// 사라져도 되지만, 코드는 그렇지 않다.
fn marked(spans: &[Span]) -> Vec<Span> {
    spans
        .iter()
        .map(|s| match s.role {
            Role::Code => Span { text: format!("`{}`", s.text), role: s.role },
            _ => s.clone(),
        })
        .collect()
}

fn flow(out: &mut Vec<Vec<Span>>, spans: &[Span], first: &str, hang: &str, width: usize) {
    let budget = width.saturating_sub(crate::text::width(first)).max(8);
    for (n, line) in wrap_spans(spans, budget).into_iter().enumerate() {
        let lead = if n == 0 { first } else { hang };
        let mut row = Vec::new();
        if !lead.is_empty() {
            row.push(mark(lead));
        }
        row.extend(line);
        out.push(row);
    }
}

#[cfg(test)]
mod real {
    /// **이 저장소의 진짜 본문**으로 돌려 본다. 합성 예제는 제가 만든 모양만
    /// 시험하므로, 실제로 쓰는 글에서 블록이 어떻게 나오는지는 따로 봐야 한다.
    /// `cargo test -- --ignored --nocapture real` 로 부른다.
    #[test]
    #[ignore]
    fn fold_every_body_in_this_repo() {
        let load = crate::store::parse_issues(
            &std::fs::read_to_string(".moai/issues.jsonl").unwrap_or_default(),
        );
        let (mut bodies, mut blocks) = (0, 0);
        let mut kinds = std::collections::BTreeMap::new();
        for i in load.issues.iter().filter(|i| i.body.is_some()) {
            bodies += 1;
            for b in super::parse(i.body.as_deref().unwrap_or_default()) {
                blocks += 1;
                let k = match b {
                    super::Block::Heading { .. } => "제목",
                    super::Block::Para(_) => "문단",
                    super::Block::List { .. } => "목록",
                    super::Block::Code { .. } => "코드",
                    super::Block::Quote(_) => "인용",
                    super::Block::Table { .. } => "표",
                    super::Block::Rule => "줄",
                };
                *kinds.entry(k).or_insert(0) += 1;
            }
        }
        println!("본문 {bodies}개 → 블록 {blocks}개");
        for (k, n) in kinds {
            println!("  {k} {n}");
        }
        assert!(blocks > 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(t: &str) -> Span {
        Span { text: t.into(), role: Role::Plain }
    }
    fn strong(t: &str) -> Span {
        Span { text: t.into(), role: Role::Strong }
    }
    fn code(t: &str) -> Span {
        Span { text: t.into(), role: Role::Code }
    }

    /// 마크다운을 안 쓴 줄은 손대지 않는다. 본문 대부분이 그렇다.
    #[test]
    fn plain_prose_passes_through() {
        assert_eq!(parse("그냥 한 줄이다."), vec![Block::Para(vec![plain("그냥 한 줄이다.")])]);
    }

    /// 굵게와 코드가 **뜻으로** 나온다. 기호는 사라진다.
    #[test]
    fn emphasis_and_code_become_roles() {
        let got = parse("**굵게** 와 `코드` 가 섞인 줄");
        assert_eq!(
            got,
            vec![Block::Para(vec![
                strong("굵게"),
                plain(" 와 "),
                code("코드"),
                plain(" 가 섞인 줄"),
            ])]
        );
    }

    /// 목록은 깊이를 들고 나온다. 이 저장소 본문이 겹친 목록을 쓴다.
    #[test]
    fn lists_carry_their_depth() {
        let got = parse("- 하나\n- 둘\n  - 둘의 속\n");
        let Some(Block::List { ordered, items }) = got.first() else {
            panic!("목록이 아니다 — {got:?}");
        };
        assert!(!ordered);
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].depth, 0);
        assert_eq!(items[2].depth, 1, "겹친 목록의 깊이를 잃었다");
        assert_eq!(items[2].spans, vec![plain("둘의 속")]);
    }

    #[test]
    fn ordered_lists_are_marked_as_such() {
        let got = parse("1. 하나\n2. 둘\n");
        let Some(Block::List { ordered, items }) = got.first() else {
            panic!("{got:?}");
        };
        assert!(ordered);
        assert_eq!(items.len(), 2);
    }

    /// 들여쓴 코드와 ``` 코드가 같은 블록으로 온다. 본문은 둘 다 쓴다.
    #[test]
    fn both_code_shapes_land_in_one_block() {
        let fenced = parse("```rust\nlet x = 1;\n```\n");
        assert_eq!(
            fenced,
            vec![Block::Code { lang: Some("rust".into()), lines: vec!["let x = 1;".into()] }]
        );
        let indented = parse("    moai status\n    moai ready\n");
        assert_eq!(
            indented,
            vec![Block::Code {
                lang: None,
                lines: vec!["moai status".into(), "moai ready".into()],
            }]
        );
    }

    #[test]
    fn headings_keep_their_level() {
        let got = parse("## 설계\n");
        assert_eq!(got, vec![Block::Heading { level: 2, spans: vec![plain("설계")] }]);
    }

    #[test]
    fn quotes_and_rules_survive() {
        assert_eq!(parse("> 인용한 줄\n"), vec![Block::Quote(vec![plain("인용한 줄")])]);
        assert_eq!(parse("---\n"), vec![Block::Rule]);
    }

    /// 표는 머리와 줄이 나뉘어 나온다. 칸 맞추는 일은 그리는 쪽이 한다.
    #[test]
    fn tables_split_into_head_and_rows() {
        let got = parse("| 후보 | 판 |\n|---|---|\n| termimad | 0.35 |\n");
        let Some(Block::Table { head, rows }) = got.first() else {
            panic!("표가 아니다 — {got:?}");
        };
        assert_eq!(head.len(), 2);
        assert_eq!(head[0], vec![plain("후보")]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][1], vec![plain("0.35")]);
    }

    /// 접은 줄은 **어느 것도** 폭을 넘지 않는다. 한글이 두 칸이라 글자 수로
    /// 세면 여기서 걸린다.
    #[test]
    fn wrapping_never_exceeds_the_width() {
        let texts = [
            "짧다",
            "띄어쓰기 없이 아주 길게 이어지는 한글 문장이 여기 들어간다",
            "a fairly long ascii sentence that needs to be folded somewhere",
            "섞인 mixed 문장 with both 스크립트 in it",
        ];
        for t in texts {
            for max in 4..40 {
                for line in wrap_spans(&[plain(t)], max) {
                    let w: usize =
                        line.iter().map(|s| crate::text::width(&s.text)).sum();
                    assert!(w <= max, "{t:?} @ {max} → {line:?} ({w}칸)");
                }
            }
        }
    }

    /// 접어도 **뜻을 잃지 않는다.** 접는 자리가 조각 한가운데일 수 있다.
    #[test]
    fn wrapping_keeps_the_roles() {
        let spans = vec![plain("앞 "), strong("아주 긴 굵은 글이 여기 이어진다"), plain(" 뒤")];
        let lines = wrap_spans(&spans, 12);
        assert!(lines.len() > 1, "안 접혔다 — {lines:?}");
        // 굵은 글은 어느 줄에 걸리든 굵은 채로 남는다
        let bold: String = lines
            .iter()
            .flatten()
            .filter(|s| s.role == Role::Strong)
            .map(|s| s.text.as_str())
            .collect();
        assert_eq!(bold.replace(' ', ""), "아주긴굵은글이여기이어진다");
    }

    /// 될 수 있으면 낱말 가운데서 안 끊는다.
    #[test]
    fn wrapping_prefers_spaces() {
        let flat = |spans: &[Span]| spans.iter().map(|s| s.text.as_str()).collect::<String>();
        let lines = wrap_spans(&[plain("alpha beta")], 7);
        assert_eq!(lines.iter().map(|l| flat(l)).collect::<Vec<_>>(), ["alpha", "beta"]);
        // 낱말 하나가 폭보다 길면 그때는 글자에서 끊는다
        let lines = wrap_spans(&[plain("alphabetagamma")], 6);
        assert_eq!(lines.iter().map(|l| flat(l)).collect::<Vec<_>>(), ["alphab", "etagam", "ma"]);
    }

    /// **두 표면이 같은 줄을 받는다.** 줄로 펴는 일이 여기 하나에 있는 이유다 —
    /// 표면마다 글머리와 들여쓰기를 따로 정하면 CLI 와 탐색기가 같은 본문을
    /// 다르게 그리고, 둘을 나란히 놓고 보는 사람이 어느 쪽을 믿을지 정해야 한다.
    #[test]
    fn layout_is_what_both_surfaces_share() {
        let blocks = parse("**굵게**\n\n- 하나\n  - 속\n\n> 인용\n\n    코드\n");
        let lines = layout(&blocks, 40);
        let flat: Vec<String> =
            lines.iter().map(|l| l.iter().map(|s| s.text.as_str()).collect()).collect();

        assert!(flat.iter().any(|l| l.contains("• 하나")), "글머리가 없다 — {flat:?}");
        assert!(flat.iter().any(|l| l.starts_with("  • 속")), "겹친 목록이 안 물렸다 — {flat:?}");
        assert!(flat.iter().any(|l| l.starts_with("│ 인용")), "인용 막대가 없다 — {flat:?}");
        assert!(flat.iter().any(|l| l.contains("    코드")), "코드 들여쓰기가 없다 — {flat:?}");
        // 표시는 글과 **다른 뜻**을 진다 — 표면이 달리 칠할 수 있어야 한다.
        assert!(
            lines.iter().flatten().any(|s| s.role == Role::Mark),
            "글머리가 글과 같은 뜻으로 나왔다"
        );
    }

    /// 이어지는 줄은 글머리 폭만큼 물린다 — 안 그러면 둘째 줄이 다음 항목처럼 보인다.
    #[test]
    fn wrapped_list_items_hang_under_their_bullet() {
        let blocks = parse("- 아주 길어서 반드시 접히고도 남을 한 줄이 여기 들어간다\n");
        let flat: Vec<String> = layout(&blocks, 20)
            .iter()
            .map(|l| l.iter().map(|s| s.text.as_str()).collect())
            .collect();
        assert!(flat.len() > 1, "안 접혔다 — {flat:?}");
        assert!(flat[0].starts_with("• "), "{flat:?}");
        assert!(flat[1].starts_with("  ") && !flat[1].starts_with("• "), "안 물렸다 — {flat:?}");
    }

    /// 표는 **칸이 맞는다.** 한글이 두 칸이라 글자 수로 맞추면 여기서 어긋난다.
    #[test]
    fn table_columns_line_up_by_display_width() {
        let blocks = parse("| 후보 | 판 |\n|---|---|\n| termimad | 0.35 |\n| 한글이름 | 1.0 |\n");
        let lines: Vec<String> = layout(&blocks, 60)
            .iter()
            .map(|l| l.iter().map(|s| s.text.as_str()).collect())
            .collect();
        assert_eq!(lines.len(), 4, "머리·구분줄·두 줄이어야 한다 — {lines:?}");

        // 둘째 칸이 모든 줄에서 같은 자리에서 시작한다. 구분줄은 `┼` 를 쓴다.
        let starts: Vec<usize> = lines
            .iter()
            .map(|l| {
                let head = l.split(['│', '┼']).next().unwrap_or_default();
                crate::text::width(head)
            })
            .collect();
        assert!(starts.windows(2).all(|w| w[0] == w[1]), "칸이 어긋났다 — {starts:?} {lines:?}");
    }

    /// 좁으면 **칸을 줄이되 잘렸다고 말한다.** 오른쪽 칸을 조용히 버리면
    /// 표를 보는 사람이 그 칸이 없는 줄 안다.
    #[test]
    fn a_narrow_table_shrinks_and_says_so() {
        let blocks = parse(
            "| 후보 | 무엇 |\n|---|---|\n| termimad | 터미널용 마크다운 렌더러인데 설명이 아주 길다 |\n",
        );
        let lines: Vec<String> = layout(&blocks, 28)
            .iter()
            .map(|l| l.iter().map(|s| s.text.as_str()).collect())
            .collect();
        for l in &lines {
            assert!(crate::text::width(l) <= 28, "{l:?} ({}칸)", crate::text::width(l));
        }
        // 칸은 둘 다 남아 있고, 잘린 자리에 표시가 있다
        // 칸은 둘 다 남는다 — 구분줄은 `┼`, 나머지는 `│`.
        assert!(
            lines.iter().all(|l| l.contains('│') || l.contains('┼')),
            "칸을 버렸다 — {lines:?}"
        );
        assert!(lines.iter().any(|l| l.contains('…')), "잘렸는데 표시가 없다 — {lines:?}");
        assert!(!lines.iter().any(|l| l.contains("……")), "잘림 표시가 겹쳤다 — {lines:?}");
    }

    /// 빈 본문이 무너지지 않는다.
    #[test]
    fn an_empty_body_is_no_blocks() {
        assert!(parse("").is_empty());
        assert!(parse("\n\n").is_empty());
    }

    /// 문단은 문단끼리 나뉜다 — 한 덩어리로 뭉치면 그릴 때 줄 사이가 사라진다.
    #[test]
    fn blank_lines_separate_paragraphs() {
        let got = parse("첫 문단\n\n둘째 문단\n");
        assert_eq!(got.len(), 2, "{got:?}");
    }
}
