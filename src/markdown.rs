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
    /// 이 줄의 글머리. `Some(n)` 은 번호 매긴 목록의 *n*번째, `None` 은 점.
    ///
    /// **번호를 접는 여기서 센다.** 그리는 쪽이 평평한 열을 세면 겹친 목록의
    /// 줄까지 같이 세어 `1. 하나 / 2. 속 / 3. 둘` 이 된다. 원문이 `3.` 에서
    /// 시작하면 `3.` 으로 시작해야 하는 것도 원문을 아는 이곳만 안다.
    pub marker: Option<u64>,
    pub spans: Vec<Span>,
}

/// 표의 한 칸, 그리고 칸들로 된 한 줄.
///
/// **이름을 붙인다.** `Vec<Vec<Vec<Span>>>` 은 어느 겹이 줄이고 어느 겹이
/// 칸인지 괄호를 세어야 알 수 있고, 줄과 칸을 맞바꾼 코드가 그대로 컴파일된다.
pub type Cell = Vec<Span>;
pub type Row = Vec<Cell>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Heading { level: u8, spans: Vec<Span> },
    Para(Vec<Span>),
    /// 번호는 줄마다 [`Item::marker`] 가 든다 — 블록에 `ordered` 를 또 두면
    /// 둘이 어긋날 수 있고, 어긋나면 어느 쪽이 참인지 정할 길이 없다.
    ///
    /// `quote` 는 이 목록이 인용 몇 겹 안에 있는가다. **문단에만 막대를 달면
    /// 인용 속 목록이 막대를 잃고, 기호를 걷어낸 뒤라 남의 말이 제 말처럼
    /// 읽힌다** — `Quote` 를 따로 둔 까닭과 같은 까닭이다.
    List { quote: u8, items: Vec<Item> },
    /// 들여쓴 코드와 ``` 코드 둘 다 여기 온다.
    Code { quote: u8, lang: Option<String>, lines: Vec<String> },
    /// `quote` 는 겹친 인용의 깊이다. 하나로 못 박으면 인용 속 인용이 제
    /// 겹을 잃어, 어디까지가 누구 말인지 화면만 봐서는 못 가린다.
    Quote { quote: u8, spans: Vec<Span> },
    Table { head: Row, rows: Vec<Row> },
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
    /// 여는 링크마다 그 주소. 겹칠 수 있으므로 쌓아 둔다.
    urls: Vec<String>,
    /// 모인 목록 줄. 겹친 목록도 한 벌이라 평평한 열 하나다.
    list: Option<Vec<Item>>,
    /// 열려 있는 목록마다 (번호 매긴 목록인가, 다음 번호). 깊이는 이 높이다.
    ///
    /// **목록마다 센다.** 하나만 두면 번호 매긴 목록 안의 점 목록이 번호를
    /// 물려받고, 바깥 번호가 안쪽 줄까지 세어 건너뛴다.
    lists: Vec<(bool, u64)>,
    code: Option<(Option<String>, String)>,
    /// 겹친 인용을 센다. **불로 두면 안쪽 인용이 닫힐 때 바깥 것도 닫혀**,
    /// 그 뒤의 인용 줄이 막대를 잃고 남의 말이 제 말처럼 읽힌다.
    quote: u32,
    heading: Option<u8>,
    table: Option<(Row, Vec<Row>)>,
    row: Row,
    /// 블록 HTML 은 문단을 두르지 않고 줄마다 온다. 모아 두고 다른 것이 오면
    /// 한 덩이로 낸다 — 버리면 `<div>` 안의 글이 통째로 사라진다.
    html: String,
}

impl Fold {
    fn run(mut self, events: Parser) -> Vec<Block> {
        for e in events {
            self.one(e);
        }
        // 남은 것을 버리지 않는다. 빠뜨리면 마지막 덩이가 조용히 사라진다.
        // 파서는 늘 태그를 닫아 주지만, 안 닫는 판이 오는 날 **글이 사라지는
        // 것보다 문단이 하나 더 나오는 쪽이 싸다.**
        self.flush_html();
        self.close_list();
        let spans = self.take();
        if !spans.is_empty() {
            self.out.push(Block::Para(spans));
        }
        self.out
    }

    /// 모으던 글을 목록의 한 줄로 매듭짓는다. 글이 없으면 아무 일도 없다.
    fn flush_item(&mut self) {
        let spans = self.take();
        if spans.is_empty() {
            return;
        }
        let depth = self.lists.len().saturating_sub(1).min(u8::MAX as usize) as u8;
        let Some((ordered, next)) = self.lists.last_mut() else {
            // 열린 목록이 없으면 이것은 목록 줄이 아니다. **도로 놓는다** —
            // 여기서 버리면 문단이 될 글이 조용히 사라진다.
            self.spans = spans;
            return;
        };
        let marker = if *ordered {
            let at = *next;
            *next += 1;
            Some(at)
        } else {
            None
        };
        self.list.get_or_insert_with(Vec::new).push(Item { depth, marker, spans });
    }

    /// 여태 모인 목록 줄을 블록으로 낸다.
    ///
    /// **목록 안의 코드·표·제목을 그냥 쌓으면 목록보다 위에 얹힌다** — 나중에
    /// 나오는 `Block::List` 뒤가 아니라 앞에 앉으므로, 제 항목이 아닌 앞
    /// 문단에 붙은 것처럼 읽힌다. 목록을 여기서 끊어 내면 차례가 지켜진다.
    /// `lists` 는 건드리지 않으므로 끊긴 뒤에도 번호는 이어서 센다.
    fn close_list(&mut self) {
        self.flush_item();
        let Some(items) = &mut self.list else { return };
        let items = std::mem::take(items);
        if !items.is_empty() {
            let quote = self.quote_depth();
            self.out.push(Block::List { quote, items });
        }
    }

    /// 지금 인용 몇 겹 안인가. 막대는 이 수만큼 붙는다.
    fn quote_depth(&self) -> u8 {
        self.quote.min(u8::MAX as u32) as u8
    }

    /// 방금 닫힌 링크·그림의 주소를 글 뒤에 괄호로 붙인다. 터미널에서 주소가
    /// 방해가 되는 것은 맞지만, 없어서 못 찾는 것이 더 나쁘다.
    ///
    /// **글이 이미 주소면 붙이지 않는다.** `<https://a>` 같은 맨 주소는
    /// 글과 주소가 같은 것 하나라, 그대로 두면 `https://a (https://a)` 로
    /// 두 번 나오고 그 길이 때문에 줄이 한 번 더 접힌다.
    fn trail_url(&mut self) {
        let Some(url) = self.urls.pop() else { return };
        if url.is_empty() || self.spans.last().is_some_and(|s| s.text.ends_with(&url)) {
            return;
        }
        self.push(&format!(" ({url})"), Role::Mark);
    }

    fn flush_html(&mut self) {
        if self.html.is_empty() {
            return;
        }
        let text = std::mem::take(&mut self.html);
        let text = text.trim_matches('\n');
        if text.is_empty() {
            return;
        }
        // 목록 항목 안이면 **그 줄의 글**이다. 블록으로 내면 그 항목이 목록에서
        // 빠져 글머리를 잃고, 세 줄짜리 목록이 목록 둘로 쪼개진다.
        if !self.lists.is_empty() {
            let role = self.role();
            self.push(&text.replace('\n', " "), role);
            return;
        }
        // 산문이 아니므로 그대로 낸다. 접으면 태그가 글 사이에 섞인다.
        self.out.push(Block::Code {
            quote: self.quote_depth(),
            lang: None,
            lines: text.lines().map(str::to_string).collect(),
        });
    }

    /// 지금 모으는 글이 지는 뜻. 겹쳤으면 **더 좁은 것**을 남긴다 — 링크가
    /// 가장 좁고, 그다음 굵게, 그다음 기울임이다.
    ///
    /// 코드는 여기 없다. `Event::Code` 가 제 뜻을 곧바로 들고 오므로 셀 것이
    /// 없다 — 여기 적어 두면 없는 갈래를 찾게 만든다.
    fn role(&self) -> Role {
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
        // 모아 둔 블록 HTML 은 다른 것이 오는 순간 매듭짓는다.
        if !matches!(e, Event::Html(_)) {
            self.flush_html();
        }
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
            // **마크다운이 아닌 글도 사라지지 않는다.** CommonMark 는
            // `R<Vec<String>>` 의 `<String>` 을 inline HTML 로 읽는다 — 버리면
            // 본문에서 글자가 조용히 사라진다. 이 저장소 본문에 실제로 있었다.
            Event::InlineHtml(t) => {
                let role = self.role();
                self.push(&t, role);
            }
            Event::Html(t) => self.html.push_str(&t),
            // 줄바꿈은 한 칸으로. 문단 안의 줄 나눔은 그리는 쪽이 폭을 보고
            // 다시 접으므로, 여기서 원문의 줄을 지키면 두 번 접힌다.
            Event::SoftBreak | Event::HardBreak => {
                let role = self.role();
                self.push(" ", role);
            }
            Event::Rule => {
                self.close_list();
                self.out.push(Block::Rule);
            }
            _ => {}
        }
    }

    fn start(&mut self, t: Tag) {
        match t {
            Tag::Strong => self.strong += 1,
            Tag::Emphasis => self.emphasis += 1,
            // **주소를 들고 있는다.** 본문에서 되짚을 수 없는 것이 주소다 —
            // 링크 글만 남기면 어디를 가리켰는지는 `--raw` 밖에 길이 없다.
            Tag::Link { dest_url, .. } => {
                self.link += 1;
                self.urls.push(dest_url.to_string());
            }
            // **그림도 주소를 남긴다.** 그림은 터미널에 뜨지 않으므로 대체글만
            // 남기면 무엇을 가리켰는지 `--raw` 밖에 길이 없다 — 링크에 대고
            // 적어 둔 까닭 그대로다. 대체글은 제 둘레의 뜻을 그대로 쓴다:
            // 그림은 링크가 아니라 누를 데가 없다.
            Tag::Image { dest_url, .. } => self.urls.push(dest_url.to_string()),
            // **모으던 목록 줄을 먼저 매듭짓는다.** 이 세 블록은 저마다
            // `spans` 를 통째로 걷어 가므로, 목록 항목의 글이 남아 있으면 그
            // 글까지 같이 걷어 간다 — `- 하나` 뒤의 `## 제목` 이 `하나제목`
            // 한 줄이 되고 목록에서 그 항목이 사라진다.
            Tag::Heading { level, .. } => {
                self.close_list();
                self.heading = Some(level as u8);
            }
            Tag::BlockQuote(_) => {
                self.quote += 1;
                // **목록 항목 속의 인용에도 막대를 단다.** 항목 안의 인용은
                // 블록이 아니라 그 줄의 글로 이어지므로(아래 `TagEnd::Paragraph`
                // 의 목록 갈래), 그냥 두면 `- 목록 항목` 뒤에 곧바로 붙어
                // `목록 항목항목 속 인용` 처럼 없던 낱말이 생기고, 기호를
                // 걷어낸 뒤라 남의 말이 제 말처럼 읽힌다.
                if !self.lists.is_empty() {
                    if !self.spans.is_empty() {
                        self.push(" ", Role::Plain);
                    }
                    self.push(BAR, Role::Mark);
                }
            }
            Tag::CodeBlock(kind) => {
                self.close_list();
                let lang = match kind {
                    CodeBlockKind::Fenced(l) if !l.is_empty() => Some(l.to_string()),
                    _ => None,
                };
                self.code = Some((lang, String::new()));
            }
            Tag::List(first) => {
                // 안쪽 목록. 바깥 것을 끊지 않고 깊이만 는다.
                //
                // **모으던 글을 먼저 매듭짓는다.** 안 그러면 바깥 줄의 글이
                // `spans` 에 남아 있다가 안쪽 첫 줄에 이어 붙는다 — `- 둘` 과
                // `  - 둘의 속` 이 `둘둘의 속` 한 줄이 되고 바깥 줄은 사라진다.
                self.flush_item();
                self.list.get_or_insert_with(Vec::new);
                self.lists.push((first.is_some(), first.unwrap_or(1)));
            }
            Tag::Table(_) => {
                self.close_list();
                self.table = Some((Vec::new(), Vec::new()));
            }
            _ => {}
        }
    }

    fn end(&mut self, t: TagEnd) {
        match t {
            TagEnd::Strong => self.strong = self.strong.saturating_sub(1),
            TagEnd::Emphasis => self.emphasis = self.emphasis.saturating_sub(1),
            TagEnd::Link => {
                self.link = self.link.saturating_sub(1);
                self.trail_url();
            }
            TagEnd::Image => self.trail_url(),
            TagEnd::Heading(_) => {
                let level = self.heading.take().unwrap_or(1);
                let spans = self.take();
                self.out.push(Block::Heading { level, spans });
            }
            TagEnd::CodeBlock => {
                if let Some((lang, buf)) = self.code.take() {
                    let lines: Vec<String> = buf.lines().map(str::to_string).collect();
                    // 빈 ``` 울타리는 블록이 아니다. 내면 `layout` 이 그 앞에
                    // 빈 줄만 하나 놓아, 본문에 까닭 없는 틈이 벌어진다.
                    if !lines.is_empty() {
                        self.out.push(Block::Code { quote: self.quote_depth(), lang, lines });
                    }
                }
            }
            TagEnd::Item => self.flush_item(),
            TagEnd::List(_) => {
                self.flush_item();
                self.lists.pop();
                // 바깥 목록이 끝났을 때만 한 벌로 낸다. 안쪽에서 끊으면
                // 겹친 목록이 블록 여럿으로 쪼개져 사이에 빈 줄이 낀다.
                if self.lists.is_empty() {
                    self.close_list();
                    self.list = None;
                }
            }
            TagEnd::BlockQuote(_) => self.quote = self.quote.saturating_sub(1),
            TagEnd::Paragraph => {
                let spans = self.take();
                if spans.is_empty() {
                    return;
                }
                // 목록 안의 문단은 그 줄의 몫이다 — `Item` 이 받아 간다.
                if self.list.is_some() {
                    self.spans = spans;
                    // **문단 사이에 한 칸을 둔다.** 한 항목에 문단이 둘이면
                    // 그대로 이어 붙어 `첫 문단둘째 문단` 이 된다. 마지막
                    // 문단이면 접을 때 줄 끝에서 다시 지워진다.
                    self.push(" ", Role::Plain);
                } else if self.quote > 0 {
                    self.out.push(Block::Quote { quote: self.quote_depth(), spans });
                } else {
                    self.out.push(Block::Para(spans));
                }
            }
            TagEnd::TableCell => {
                let cell = self.take();
                self.row.push(cell);
            }
            TagEnd::TableHead => {
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

/// 인용 막대. **겹친 만큼 겹쳐 놓는다** — 한 겹만 놓으면 인용 속 인용이
/// 제 겹을 잃고, 어디까지가 누구 말인지 화면만 봐서는 못 가린다.
const BAR: &str = "│ ";

fn bars(depth: u8) -> String {
    BAR.repeat(depth as usize)
}

fn mark(t: impl Into<String>) -> Span {
    Span { text: t.into(), role: Role::Mark }
}

/// 조각 열의 표시 폭. **한 자리에서만 센다** — 칸을 채우는 쪽과 자르는 쪽이
/// 저마다 이 합을 적으면, 한 곳만 고쳐졌을 때 표의 칸이 어긋난다.
pub fn span_width(spans: &[Span]) -> usize {
    spans.iter().map(|s| crate::text::width(&s.text)).sum()
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
            // **`#` 를 남긴다.** 색을 끄면 `anstream` 이 굵게까지 걷어내므로,
            // 굵게에만 기대면 1단계 제목이 문단과 한 글자도 다르지 않다 —
            // 들여쓰기가 0이기 때문이다. 수준도 이것으로 읽힌다.
            let hash = format!("{} ", "#".repeat(*level as usize));
            // 제목 안의 코드도 백틱을 되돌려 받는다 — 여기서 `marked` 를
            // 빠뜨리면 `## `moai status`` 의 코드가 색만 남아, 색을 끈
            // 터미널에서 제목의 다른 낱말과 구별할 길이 사라진다.
            //
            // **한 번만 붙인다.** `marked` 는 백틱을 두르고도 뜻을 `Code` 로
            // 남기므로, 아래에서 또 부르면 `` ``moai status`` `` 가 된다.
            let spans: Vec<Span> = marked(spans)
                .into_iter()
                .map(|s| match s.role {
                    Role::Plain => Span { text: s.text, role: Role::Heading },
                    _ => s,
                })
                .collect();
            // 이어지는 줄은 `#` 폭만큼 물려 쓴다.
            let hang = " ".repeat(crate::text::width(&hash));
            flow(out, &spans, &hash, &hang, width);
        }
        Block::Para(spans) => flow(out, &marked(spans), "", "", width),
        Block::Quote { quote, spans } => {
            // 인용은 **색이 아니라 세로줄**로 말한다.
            let bar = bars((*quote).max(1));
            flow(out, &marked(spans), &bar, &bar, width);
        }
        Block::List { quote, items } => {
            let bar = bars(*quote);
            for it in items {
                let pad = "  ".repeat(it.depth as usize);
                let bullet = match it.marker {
                    Some(n) => format!("{n}. "),
                    None => format!("{BULLET} "),
                };
                // 이어지는 줄은 글머리 폭만큼 물려 쓴다 — 안 그러면 둘째 줄이
                // 다음 항목처럼 보인다.
                let hang = format!("{bar}{pad}{}", " ".repeat(crate::text::width(&bullet)));
                flow(out, &marked(&it.spans), &format!("{bar}{pad}{bullet}"), &hang, width);
            }
        }
        Block::Code { quote, lines, .. } => {
            // 코드는 접지 않는다. 접으면 그 줄이 더는 그 코드가 아니다.
            let lead = format!("{}    ", bars(*quote));
            for l in lines {
                out.push(vec![mark(lead.clone()), Span { text: l.clone(), role: Role::Code }]);
            }
        }
        Block::Rule => out.push(vec![mark("─".repeat(width.min(40)))]),
        Block::Table { head, rows } => lay_table(out, head, rows, width),
    }
}

/// 칸 사이. 세로줄은 **표시**라 글과 다른 뜻을 진다.
const CELL_GAP: &str = " │ ";
/// 좁을 때의 칸 사이. 공백을 버리고 세로줄만 남긴다.
const THIN_GAP: &str = "│";

/// 표를 포기하고 칸마다 한 줄로. **칸 사이가 칸보다 넓어지는 폭**에서는 어떤
/// 줄맞춤도 뜻이 없다 — 그래도 글자는 잃지 않는다.
fn lay_table_as_lines(out: &mut Vec<Vec<Span>>, head: &Row, rows: &[Row], width: usize) {
    for (n, r) in std::iter::once(head).chain(rows).enumerate() {
        if n > 0 {
            out.push(vec![mark("─".repeat(width.min(8)))]);
        }
        for cell in r.iter().filter(|c| !c.is_empty()) {
            flow(out, cell, "", "", width);
        }
    }
}

/// 표를 칸 맞춰 편다.
///
/// 좁아서 다 안 들어가면 **칸을 줄이되 버리지는 않는다.** 오른쪽 칸을 조용히
/// 떨어뜨리면 보는 쪽은 그 칸이 애초에 없는 줄 안다. 줄인 자리에는 `…` 가
/// 남아 잘렸다는 것이 보인다 — 목록의 긴 제목을 다루는 방식과 같다.
fn lay_table(out: &mut Vec<Vec<Span>>, head: &[Cell], rows: &[Row], width: usize) {
    let cols = head.len().max(rows.iter().map(Vec::len).max().unwrap_or(0));
    if cols == 0 {
        return;
    }
    // **표시는 한 번만 붙인다.** 재는 데 한 번 부르고 그리는 데 또 부르면
    // 칸마다 백틱 붙인 글이 두 벌 생기고, 이 셈은 프레임마다 돈다.
    let cells = |r: &[Cell]| -> Row {
        (0..cols).map(|n| marked(r.get(n).map(Vec::as_slice).unwrap_or(&[]))).collect()
    };
    let head = cells(head);
    let rows: Vec<Row> = rows.iter().map(|r| cells(r)).collect();

    // 있는 대로의 폭. 한글이 두 칸이라 **표시 폭**으로 잰다.
    // **표시를 붙인 뒤의 폭**으로 잰다. 백틱을 빼고 재면 칸이 좁게 잡혀
    // 멀쩡한 표가 늘 잘린다.
    let mut w: Vec<usize> = (0..cols)
        .map(|n| {
            std::iter::once(span_width(&head[n]))
                .chain(rows.iter().map(|r| span_width(&r[n])))
                .max()
                .unwrap_or(0)
        })
        .collect();

    // 넘치면 **가장 넓은 칸부터** 한 칸씩 줄인다. 좁은 칸(`판`·`1.0`)은 그대로
    // 남으므로, 줄어드는 것은 늘 설명처럼 긴 칸이다.
    // **칸 사이도 줄인다.** 칸을 1칸까지 좁혀도 `" │ "` 셋씩이 남아 준 폭을
    // 넘을 수 있다 — 그때는 가운뎃점만 남기고, 그래도 안 되면 표를 포기하고
    // 칸마다 한 줄로 떨어뜨린다. 넘치면 위젯이 다시 접어 칸 맞춤이 통째로
    // 무너지고, 그러면 표가 표인 값을 잃는다.
    // `cols` 는 "칸마다 최소 한 칸" 이다 — 칸 너비 1 × 칸 수.
    let gap = match cols + crate::text::width(CELL_GAP) * (cols - 1) <= width {
        true => CELL_GAP,
        false => THIN_GAP,
    };
    let gaps = crate::text::width(gap) * (cols - 1);
    // **칸 하나씩이라도 들어가야 표다.** 칸 사이만 견주면 1칸짜리 칸들이
    // 그 위로 더해져 그대로 넘친다.
    if cols + gaps > width {
        lay_table_as_lines(out, &head, &rows, width);
        return;
    }
    let budget = width - gaps;
    while w.iter().sum::<usize>() > budget {
        let Some(widest) = (0..cols).max_by_key(|&n| (w[n], std::cmp::Reverse(n))) else { break };
        if w[widest] <= 1 {
            break;
        }
        w[widest] -= 1;
    }

    let row = |r: &[Cell], is_head: bool| -> Vec<Span> {
        let mut line = Vec::new();
        for n in 0..cols {
            if n > 0 {
                line.push(mark(gap));
            }
            // 표 안에서도 코드는 백틱을 남긴다 — 칸이 위치를 말해 줄 뿐,
            // 그 글이 코드라는 것은 색만으로는 색을 끈 터미널에서 사라진다.
            let cell = fit(&r[n], w[n], is_head);
            let pad = w[n].saturating_sub(span_width(&cell));
            line.extend(cell);
            // 마지막 칸은 채우지 않는다 — 오른쪽에 뜻 없는 공백이 남는다.
            if n + 1 < cols && pad > 0 {
                line.push(mark(" ".repeat(pad)));
            }
        }
        line
    };

    out.push(row(&head, true));
    // 구분줄. 테두리를 두르지 않는 이 저장소의 표 모양과 같게 가볍게 둔다.
    out.push({
        let mut line = Vec::new();
        for (n, &cw) in w.iter().enumerate() {
            if n > 0 {
                line.push(mark(match gap {
                    CELL_GAP => "─┼─",
                    _ => "┼",
                }));
            }
            line.push(mark("─".repeat(cw)));
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
    // 머리 칸의 맨글은 제목이 된다. 조각을 베끼지 않고 **뜻만** 고른다.
    let as_head = |r: Role| if head && r == Role::Plain { Role::Heading } else { r };
    if span_width(cell) <= max {
        return cell
            .iter()
            .map(|s| Span { text: s.text.clone(), role: as_head(s.role) })
            .collect();
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
        // **빈 조각에서 멈춘다.** 다음 글자가 남은 칸보다 넓을 때 건너뛰면
        // 뒤 조각의 글이 칸 머리에 앉아, 그 칸이 그 글로 시작하는 것처럼
        // 보인다 — `가`xy`` 가 `` `… `` 로 나온다.
        if piece.is_empty() {
            break;
        }
        used += crate::text::width(&piece);
        out.push(Span { text: piece, role: as_head(s.role) });
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
    // 들여쓴 만큼을 뺀 몫이 글의 폭이다. **바닥은 두 칸**이다 — 한 칸으로 두면
    // 두 칸짜리 한글이 그 한 칸을 넘고, 넉넉히 여덟 칸으로 두면 좁은 패널에서
    // `들여쓰기 + 8` 이 폭을 넘어 위젯이 그 줄을 다시 접는다. 다시 접힌 줄은
    // 글머리 밑으로 물리지 않아 다음 항목처럼 보인다.
    // **앞머리를 뺀 뒤에 바닥을 걸지 않는다.** 뺄셈 뒤에 `max` 를 걸면 앞머리
    // 폭만큼 통째로 넘쳐, 깊이 물린 목록과 `###### ` 제목이 준 폭을 넘는다.
    // 앞머리 자체가 폭보다 넓을 수도 있어(폭 4에 `###### `) 잘라서라도 글
    // 한 칸을 남긴다 — 앞머리만 있는 줄은 아무것도 말하지 않는다.
    // 두 칸을 남긴다. 한 칸만 남기면 두 칸짜리 한글 한 자가 그 줄에서 넘친다 —
    // 글자를 버릴 수는 없으므로 앞머리를 그만큼 더 줄인다.
    let room = width.saturating_sub(2);
    let first = crate::text::clip(first, room);
    let hang = crate::text::clip(hang, room);
    let lead = crate::text::width(&first).max(crate::text::width(&hang));
    let budget = width.saturating_sub(lead).max(2);
    for (n, line) in wrap_spans(spans, budget).into_iter().enumerate() {
        let lead: &str = if n == 0 { &first } else { &hang };
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
                    super::Block::Quote { .. } => "인용",
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
        let Some(Block::List { items, .. }) = got.first() else {
            panic!("목록이 아니다 — {got:?}");
        };
        assert_eq!(items.len(), 3);
        assert!(items.iter().all(|i| i.marker.is_none()), "점 목록에 번호가 붙었다");
        assert_eq!(items[0].depth, 0);
        assert_eq!(items[2].depth, 1, "겹친 목록의 깊이를 잃었다");
        assert_eq!(items[2].spans, vec![plain("둘의 속")]);
    }

    /// 번호는 **목록마다** 센다. 겹친 줄을 같이 세면 바깥 번호가 건너뛰고,
    /// 번호 매긴 목록 안의 점 목록이 번호를 물려받는다.
    #[test]
    fn ordered_lists_number_each_level_on_its_own() {
        let got = parse("1. 하나\n2. 둘\n   - 속\n3. 셋\n");
        let Some(Block::List { items, .. }) = got.first() else {
            panic!("{got:?}");
        };
        let got: Vec<(u8, Option<u64>)> = items.iter().map(|i| (i.depth, i.marker)).collect();
        assert_eq!(got, [(0, Some(1)), (0, Some(2)), (1, None), (0, Some(3))], "{items:?}");
    }

    /// 원문이 `3.` 에서 시작하면 `3.` 으로 시작한다. 1부터 다시 세면 본문이
    /// 가리키는 단계 번호가 조용히 달라진다.
    #[test]
    fn an_ordered_list_keeps_the_number_it_starts_at() {
        let Some(Block::List { items, .. }) = parse("3. 셋\n4. 넷\n").first().cloned() else {
            panic!("목록이 아니다");
        };
        assert_eq!(items.iter().map(|i| i.marker).collect::<Vec<_>>(), [Some(3), Some(4)]);
    }

    /// 들여쓴 코드와 ``` 코드가 같은 블록으로 온다. 본문은 둘 다 쓴다.
    #[test]
    fn both_code_shapes_land_in_one_block() {
        let fenced = parse("```rust\nlet x = 1;\n```\n");
        assert_eq!(
            fenced,
            vec![Block::Code { quote: 0, lang: Some("rust".into()), lines: vec!["let x = 1;".into()] }]
        );
        let indented = parse("    moai status\n    moai ready\n");
        assert_eq!(
            indented,
            vec![Block::Code {
                quote: 0,
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
        assert_eq!(parse("> 인용한 줄\n"), vec![Block::Quote { quote: 1, spans: vec![plain("인용한 줄")] }]);
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

    /// **제목은 색을 꺼도 제목으로 남는다.** 색을 끄면 `anstream` 이 굵게까지
    /// 걷어내므로, 굵게에만 기대면 1단계 제목이 그냥 문단과 한 글자도 다르지
    /// 않게 된다 — 들여쓰기가 0이기 때문이다.
    #[test]
    fn a_heading_keeps_a_mark_that_survives_without_colour() {
        for src in ["# 큰 제목\n", "### 작은 제목\n"] {
            let flat: String = layout(&parse(src), 40)
                .iter()
                .flat_map(|l| l.iter().map(|s| s.text.clone()))
                .collect();
            assert!(flat.contains('#'), "제목 표시가 없다 — {flat:?}");
        }
    }

    /// **링크의 주소를 버리지 않는다.** 본문에서 되짚을 수 없는 것이 주소다.
    /// 모듈 문서가 "주소는 따로 낸다" 고 적고 있으니 실제로 내야 한다.
    #[test]
    fn a_link_keeps_its_address() {
        let flat: String = layout(&parse("근거는 [여기](https://example.com/a) 다\n"), 60)
            .iter()
            .flat_map(|l| l.iter().map(|s| s.text.clone()))
            .collect();
        assert!(flat.contains("여기"), "{flat:?}");
        assert!(flat.contains("https://example.com/a"), "주소를 버렸다 — {flat:?}");
    }

    /// **표는 준 폭을 넘지 않는다.** 칸 사이가 칸보다 넓어지는 좁은 폭에서도
    /// 그렇다 — 넘치면 위젯이 다시 접어 칸 맞춤이 통째로 무너진다.
    #[test]
    fn a_table_never_exceeds_the_width_it_was_given() {
        let wide = "| a | b | c | d | e | f |\n|---|---|---|---|---|---|\n| 1 | 2 | 3 | 4 | 5 | 6 |\n";
        for max in 4..40 {
            for line in layout(&parse(wide), max) {
                let w: usize = line.iter().map(|s| crate::text::width(&s.text)).sum();
                assert!(w <= max, "폭 {max} 에서 {w}칸 — {line:?}");
            }
        }
    }

    /// 어느 블록도 준 폭을 넘지 않는다. 들여쓴 목록·깊은 제목이 걸리던 자리다.
    #[test]
    fn no_block_exceeds_the_width_it_was_given() {
        let src = "###### 깊은 제목\n\n- 하나\n  - 둘\n    - 셋이 길게 이어진다\n\n> 인용한 줄\n";
        for max in 4..40 {
            for line in layout(&parse(src), max) {
                let w: usize = line.iter().map(|s| crate::text::width(&s.text)).sum();
                assert!(w <= max, "폭 {max} 에서 {w}칸 — {line:?}");
            }
        }
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

    fn flat(src: &str, w: usize) -> Vec<String> {
        layout(&parse(src), w)
            .iter()
            .map(|l| l.iter().map(|s| s.text.as_str()).collect())
            .collect()
    }

    /// **마크다운이 아닌 글자가 사라지지 않는다.** CommonMark 는 `<String>` 을
    /// inline HTML 로 읽고, 버리면 본문에서 낱말이 조용히 없어진다 — 이 저장소
    /// 본문의 `R<Vec<String>>` 이 실제로 그렇게 7자를 잃고 있었다.
    #[test]
    fn text_that_looks_like_html_is_not_swallowed() {
        assert_eq!(flat("cmd::run 이 R<Vec<String>> 을 낸다\n", 60), ["cmd::run 이 R<Vec<String>> 을 낸다"]);
        assert_eq!(flat("moai show <id> 를 쓴다\n", 60), ["moai show <id> 를 쓴다"]);
        // 블록 HTML 도 통째로 사라지지 않는다. 산문이 아니므로 그대로 낸다.
        let got = flat("<div>\n감춰진 글\n</div>\n", 60).join("\n");
        assert!(got.contains("감춰진 글"), "블록 HTML 안의 글이 사라졌다 — {got:?}");
    }

    /// 목록 안의 코드·표·제목은 **제 자리에** 남는다. 목록을 끝에서 한 벌로
    /// 내면서 그 안의 블록을 곧바로 쌓으면, 그 블록이 목록보다 **위**에 얹혀
    /// 앞 문단에 붙은 것처럼 읽힌다. 표는 더 나쁘다 — 항목의 글까지 첫 머리
    /// 칸으로 빨려 들어가 그 항목이 목록에서 사라진다.
    #[test]
    fn blocks_inside_a_list_keep_their_place() {
        let got = flat("- 첫 항목\n\n      moai status\n\n- 둘째 항목\n", 40);
        let at = |needle: &str| got.iter().position(|l| l.contains(needle));
        assert!(at("첫 항목") < at("moai status"), "코드가 목록 위로 올라갔다 — {got:?}");
        assert!(at("moai status") < at("둘째 항목"), "{got:?}");

        let got = flat("- 하나\n\n  | a | b |\n  |---|---|\n  | 1 | 2 |\n\n- 둘\n", 40).join("\n");
        assert!(got.contains("• 하나"), "표가 항목의 글을 빨아들였다 — {got:?}");
        assert!(!got.contains("하나a"), "{got:?}");

        let got = flat("- 하나\n\n  # 제목\n\n- 둘\n", 40).join("\n");
        assert!(!got.contains("하나제목"), "제목이 항목의 글과 붙었다 — {got:?}");
    }

    /// 겹친 인용이 닫혀도 **바깥 인용은 열린 채다.** 불 하나로 세면 안쪽이
    /// 닫힐 때 바깥도 닫혀, 그 뒤의 인용 줄이 막대를 잃는다 — 기호를 걷어낸
    /// 뒤라 남의 말이 제 말처럼 읽힌다.
    #[test]
    fn a_nested_quote_does_not_close_the_outer_one() {
        let got = flat("> 바깥\n>\n> > 안쪽\n>\n> 다시 바깥\n", 40);
        for l in got.iter().filter(|l| !l.is_empty()) {
            assert!(l.starts_with("│ "), "인용 막대를 잃었다 — {got:?}");
        }
    }

    /// 한 항목에 문단이 둘이면 **낱말이 붙지 않는다.** 그대로 이으면
    /// `첫 문단둘째 문단` 이 되어 없는 낱말이 생긴다.
    #[test]
    fn two_paragraphs_in_one_item_do_not_fuse() {
        assert_eq!(flat("- 첫 문단\n\n  둘째 문단\n", 40), ["• 첫 문단 둘째 문단"]);
        // 문단이 하나면 줄 끝에 군더더기가 남지 않는다.
        assert_eq!(flat("- 하나\n", 40), ["• 하나"]);
    }

    /// 번호는 **목록마다** 센다. 원문이 시작한 번호에서 시작하고, 겹친 점
    /// 목록은 번호를 물려받지 않는다.
    #[test]
    fn ordered_markers_survive_nesting_and_a_start_number() {
        assert_eq!(flat("1. 하나\n   - 속\n2. 둘\n", 40), ["1. 하나", "  • 속", "2. 둘"]);
        assert_eq!(flat("3. 셋\n4. 넷\n", 40), ["3. 셋", "4. 넷"]);
    }

    /// 칸이 좁아 첫 조각이 한 글자도 못 들어가면 **거기서 멈춘다.** 건너뛰면
    /// 뒤 조각의 글이 칸 머리에 앉아, 그 칸이 그 글로 시작하는 것처럼 보인다.
    #[test]
    fn a_clipped_cell_never_starts_with_a_later_span() {
        let cell = marked(&[plain("가나"), code("xy")]);
        let got: String = fit(&cell, 2, false).iter().map(|s| s.text.as_str()).collect();
        assert_eq!(got, "…", "뒤 조각이 칸 머리로 올라왔다 — {got:?}");
    }

    /// 제목 안의 코드도 백틱을 되돌려 받는다. 색을 끄면 색으로만 표시한 것은
    /// 그냥 글이 된다 — 제목도 예외가 아니다.
    #[test]
    /// **한 겹만 붙는다.** `contains` 로만 물으면 `` ``moai status`` `` 도
    /// 통과한다 — 실제로 `marked` 를 두 번 불러 그렇게 나가고 있었고, 그 시험이
    /// 초록이라 아무도 못 봤다. 줄 전체를 대고 잰다.
    fn code_in_a_heading_keeps_its_backticks() {
        assert_eq!(flat("## `moai status` 를 먼저\n", 40), ["## `moai status` 를 먼저"]);
    }

    /// **인용은 목록·코드 안에서도 막대를 지킨다.** 문단에만 막대를 달면
    /// 인용 속 목록과 코드가 막대를 잃고, 기호를 걷어낸 뒤라 남의 말이 제
    /// 말처럼 읽힌다 — 겹친 인용에 대고 적어 둔 것과 같은 까닭이다.
    #[test]
    fn a_quote_keeps_its_bar_around_lists_and_code() {
        let got = flat("> 인용 문단\n>\n> - 인용 속 목록\n>\n> ```\n> 인용 속 코드\n> ```\n", 40);
        for l in got.iter().filter(|l| !l.is_empty()) {
            assert!(l.starts_with("│ "), "인용 막대를 잃었다 — {got:?}");
        }
        // 겹친 인용은 겹친 만큼 막대를 쌓는다.
        assert_eq!(flat("> > 두 겹\n", 40), ["│ │ 두 겹"]);
    }

    /// 목록 항목 안의 인용은 그 줄에 이어지는데, **낱말이 붙지 않고 막대를
    /// 얻는다.** 그냥 이으면 `목록 항목항목 속 인용` 처럼 없던 낱말이 생긴다.
    #[test]
    fn a_quote_inside_a_list_item_neither_fuses_nor_loses_its_bar() {
        assert_eq!(
            flat("- 목록 항목\n  > 항목 속 인용\n- 다음 항목\n", 40),
            ["• 목록 항목 │ 항목 속 인용", "• 다음 항목"]
        );
    }

    /// **맨 주소는 한 번만 낸다.** `<https://a>` 는 글과 주소가 같은 것
    /// 하나라, 괄호로 또 붙이면 `https://a (https://a)` 로 두 번 나오고 그
    /// 길이 때문에 줄이 한 번 더 접힌다.
    #[test]
    fn a_bare_address_is_not_printed_twice() {
        assert_eq!(flat("근거는 <https://example.com/a> 다\n", 60), ["근거는 https://example.com/a 다"]);
        // 글이 따로 있으면 주소는 그대로 뒤에 붙는다.
        assert_eq!(
            flat("글은 [여기](https://example.com/b) 다\n", 60),
            ["글은 여기 (https://example.com/b) 다"]
        );
    }

    /// **그림도 주소를 남긴다.** 그림은 터미널에 뜨지 않으므로 대체글만
    /// 남기면 무엇을 가리켰는지 `--raw` 밖에 길이 없다 — 링크와 같은 까닭이다.
    #[test]
    fn an_image_keeps_its_address() {
        assert_eq!(
            flat("![그림](https://example.com/i.png) 이 온다\n", 60),
            ["그림 (https://example.com/i.png) 이 온다"]
        );
    }

    /// 빈 울타리는 블록이 아니다. 내면 `layout` 이 그 앞에 빈 줄만 하나 놓아
    /// 본문에 까닭 없는 틈이 벌어진다.
    #[test]
    fn an_empty_fence_makes_no_block() {
        assert_eq!(flat("문단\n\n```\n```\n\n뒤 문단\n", 40), ["문단", "", "뒤 문단"]);
    }

    /// 목록 항목 안의 `<div>` 도 그 줄의 글이다. 블록으로 내면 그 항목이
    /// 목록에서 빠져 글머리를 잃고, 세 줄이 목록 둘로 쪼개진다.
    #[test]
    fn html_inside_a_list_item_keeps_its_bullet() {
        assert_eq!(
            flat("- 하나\n- <div>속</div>\n- 둘\n", 40),
            ["• 하나", "• <div>속</div>", "• 둘"]
        );
    }

    /// **어떤 폭을 줘도 산문 줄은 그 폭을 넘지 않는다.** 들여쓴 만큼을 뺀 뒤에
    /// 넉넉한 바닥값을 얹으면 `들여쓰기 + 바닥값` 이 폭을 넘고, 그러면 그리는
    /// 쪽 위젯이 그 줄을 다시 접어 글머리 밑으로 물린 것이 풀린다.
    ///
    /// 코드와 표는 뺀다 — 접지 않기로 정한 자리다.
    #[test]
    fn laid_out_prose_never_exceeds_the_width_it_was_given() {
        let bodies = [
            "- 하나\n  - 둘의 속이 아주 길게 이어지고 또 이어진다\n",
            "###### 여섯 단계 제목이 길게 이어진다\n",
            "> 인용한 글이 아주 길게 이어지고 또 이어진다\n",
            "1. 하나\n   1. 속이 아주 길게 이어지고 또 이어진다\n",
        ];
        for body in bodies {
            for w in 0..40 {
                for line in layout(&parse(body), w) {
                    // 글머리·들여쓰기는 줄지 않는다. 넘지 않아야 하는 것은
                    // **글의 몫**이고, 그 바닥은 두 칸(한글 한 자)이다.
                    let lead = line
                        .first()
                        .filter(|s| s.role == Role::Mark)
                        .map_or(0, |s| crate::text::width(&s.text));
                    let text: String = line.iter().map(|s| s.text.as_str()).collect();
                    let got = crate::text::width(&text);
                    assert!(got <= w.max(lead + 2), "{body:?} @ {w} → {text:?} ({got}칸)");
                }
            }
        }
    }
}
