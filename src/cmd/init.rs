//! `.moai/` 를 심는다. `store` 말고 파일을 만드는 유일한 곳이다.

use super::{Ctx, Fail, R};
use crate::cmd::merge_driver::Planting;
use crate::config::DEFAULT_STATUSES;
use crate::held::Fell;
use crate::i18n::{fill, say};
use crate::init_choice::{Choices as Choice, Flags, Guide, Plan, Tracking};
use crate::store::Elsewhere;
use std::path::Path;

/// 여는 마커의 **머리**. 뒤에 메타(`v:`·`hash:`)가 붙고 `-->` 로 닫힌다 — 머리로 찾아야
/// 메타가 없던 옛 맨 마커(`<!-- moai:begin -->`)도 같은 블록으로 알아본다. 머리가 마커가 되는
/// 것은 **줄 하나를 통째로 차지할 때뿐이다**([`is_begin`]).
const BEGIN: &str = "<!-- moai:begin";
const END: &str = "<!-- moai:end -->";

/// 블록을 여는 마커 한 줄 — `<!-- moai:begin v:<버전> hash:<8자> -->` (moai-leyw).
///
/// **해시는 블록 글 위의 것이다**, 파일 전체가 아니라. 블록 밖은 사람의 산문이라 그것이
/// 바뀌었다고 블록이 낡은 것이 아니다. 버전은 사람이 읽으라고 둔다 — 낡음을 가르는 것은
/// 글이다: 크레이트 버전은 안내 글이 바뀌어도 그대로일 때가 많다. 그래서 **글이 같으면 이미
/// 선 마커를 그대로 둔다**([`kept_marker`]) — 마커의 버전은 그 글을 처음 쓴 바이너리의 것이다.
fn begin_marker(block: &str) -> String {
    format!("{BEGIN} v:{} {} -->", env!("CARGO_PKG_VERSION"), hash_of(block))
}

/// 마커가 대는 해시 조각 `hash:<8자>`. 쓰는 곳([`begin_marker`])과 알아보는 곳([`kept_marker`])이
/// 같은 글자를 쓴다.
///
/// 해셔는 [`crate::text::fnv1a32`] 다(moai-2vrw) — `skill` 의 64비트와 나란히 한 자리에 있어야
/// "왜 std 해셔가 아닌가" 를 두 곳에 적지 않는다. 공개된 시험값은 그 자리에서 박혀 있다.
fn hash_of(block: &str) -> String {
    format!("hash:{:08x}", crate::text::fnv1a32(block.as_bytes()))
}

/// 여는 마커 줄인가. **줄머리에서 머리로 시작하고, 머리 바로 뒤가 띄어쓰기이고, `-->` 로
/// 끝나야 한다**(리뷰 moai-epb0.27f). 머리를 파일 어디서나 찾던 때는 산문이 마커를 적어 보인
/// 자리(`` `<!-- moai:begin v:… -->` 줄로 연다 ``)나 `<!-- moai:beginning -->` 이 블록의
/// 시작이 되어, 거기서 진짜 닫는 마커까지의 산문이 블록으로 먹혔다. 파일 첫 줄의 BOM 은 건넌다.
fn is_begin(line: &str) -> bool {
    line.trim_start_matches('\u{feff}')
        .trim_end()
        .strip_prefix(BEGIN)
        .is_some_and(|rest| rest.starts_with(' ') && rest.ends_with("-->"))
}

/// 닫는 마커 줄인가. 여는 쪽과 같이 줄 하나를 통째로 차지할 때만.
fn is_end(line: &str) -> bool {
    line.trim_end() == END
}

/// 머지 충돌 표시 줄인가 — `<<<<<<<`·`>>>>>>>` 에 가지 이름이 붙거나 아무것도 안 붙는다.
fn is_conflict(line: &str, mark: &str) -> bool {
    line.strip_prefix(mark).is_some_and(|rest| rest.starts_with(' ') || rest.trim_end().is_empty())
}

/// 파일에 선 관리 블록의 자리들, 위에서부터 — `(시작, 끝)` 은 여는 마커 줄의 첫 바이트와 닫는
/// 마커 줄(개행까지)의 끝이다. **쓰는 길([`with_block`])과 보는 길([`block_state`])이 이 하나로
/// 블록을 찾는다** — 따로 찾으면 `check` 가 `missing` 이라 한 파일의 블록을 `init` 이 갈아
/// 끼우거나, 그 반대가 된다.
///
/// - **짝은 닫는 마커에 가장 가까운 여는 마커다.** 닫는 줄을 잃은 여는 마커(머지·손질)부터 재면
///   그 뒤의 산문이 블록으로 먹힌다 — `init` 이 덧붙인 새 블록과 짝지어져 다음 `init` 이 그
///   사이를 지웠다. 짝 잃은 여는 마커는 산문으로 남는다.
/// - **여는 마커 바로 위가 머지 충돌의 첫 줄이면 거기부터 한 블록이다.** 마커에 해시가 실려,
///   두 가지가 저마다 안내를 고치고 `init` 하면 첫 줄에서 충돌한다. 그 충돌은 블록 안의 것이라
///   다시 심으면 풀린다 — `<<<<<<<` 를 산문으로 남기면 그 줄을 인 채 `current` 로 읽혔다.
///   충돌이 닫히기 전에 만난 여는 마커는 같은 블록의 다른 쪽이다.
/// - 둘째부터는 **겹친 블록**이다. 새 마커를 못 알아보는 옛 바이너리가 `init` 마다 덧붙인다.
fn blocks(text: &str) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    let mut open: Option<usize> = None;
    let mut conflict = false;
    let mut prev: Option<(usize, &str)> = None;
    let mut at = 0;
    for line in text.split_inclusive('\n') {
        if is_begin(line) {
            match prev {
                _ if open.is_some() && conflict => {}
                Some((p, l)) if is_conflict(l, "<<<<<<<") => {
                    open = Some(p);
                    conflict = true;
                }
                // BOM 은 산문 쪽에 둔다 — 갈아 끼우며 같이 지우면 파일 머리가 바뀐다.
                _ => {
                    open = Some(at + line.len() - line.trim_start_matches('\u{feff}').len());
                    conflict = false;
                }
            }
        } else if is_conflict(line, ">>>>>>>") {
            conflict = false;
        } else if is_end(line) {
            if let Some(start) = open.take() {
                found.push((start, at + line.len()));
            }
            conflict = false;
        }
        prev = Some((at, line));
        at += line.len();
    }
    found
}

/// 마커 사이만 갈아 끼운다. 사람이 쓴 산문은 **한 글자도 건드리지 않는다.**
///
/// 남의 파일에 제 것을 쓰는 도구는 이 약속을 지켜야만 신뢰를 얻는다.
///
/// 블록은 [`blocks`] 로 찾는다. 첫 블록을 갈아 끼우고 **겹친 블록은 지운다** — 그 사이 산문은
/// 두고, 빈 줄뿐이면 같이 걷는다. **줄 끝은 파일의 것을 따른다**: CRLF 로 체크아웃한 파일에 LF
/// 블록을 쓰면 늘 `stale` 이었고, 체크아웃하고 `init` 할 때마다 빈 줄이 하나씩 붙었다.
fn with_block(existing: &str, block: &str) -> String {
    debug_assert!(block.ends_with('\n'), "블록은 개행으로 끝나야 닫는 마커가 제 줄에 선다");
    let found = blocks(existing);
    let crlf = match found.first() {
        Some(&(start, _)) => existing[start..].split_inclusive('\n').next().is_some_and(|l| l.ends_with("\r\n")),
        None => existing.contains("\r\n"),
    };
    let nl = if crlf { "\r\n" } else { "\n" };
    let written = if crlf { block.replace('\n', "\r\n") } else { block.to_string() };
    let Some(&(start, stop)) = found.first() else {
        let body = format!("{}{nl}{written}{END}{nl}", begin_marker(block));
        if existing.trim().is_empty() {
            return body;
        }
        let mut out = existing.to_string();
        if !out.ends_with('\n') {
            out.push_str(nl);
        }
        out.push_str(nl);
        out.push_str(&body);
        return out;
    };
    let marker =
        kept_marker(&existing[start..stop], &written, block).map_or_else(|| begin_marker(block), str::to_string);
    let mut out = format!("{}{marker}{nl}{written}{END}{nl}", &existing[..start]);
    let mut at = stop;
    for &(s, e) in &found[1..] {
        let between = &existing[at..s];
        if !between.trim().is_empty() {
            out.push_str(between);
        }
        at = e;
    }
    out.push_str(&existing[at..]);
    // 파일 끝의 겹친 블록을 걷었으면 그것을 덧붙이며 끼운 빈 줄 하나도 걷는다 — 덧붙이기 전의
    // 파일로 돌아간다. 중간의 것은 둔다: 걷으면 앞뒤 산문이 한 문단으로 붙는다.
    if found.len() > 1 && at == existing.len() && out.ends_with(&format!("{nl}{nl}")) {
        out.truncate(out.len() - nl.len());
    }
    out
}

/// 이미 선 여는 마커를 그대로 둘 것인가 — **블록 글이 쓸 글과 같고, 마커가 그 글의 해시를 댈 때.**
///
/// 새로 쓰면 버전만 다른 블록이 글은 같은데 `stale` 이 되고, 버전이 다른 바이너리 둘이 한
/// 저장소에서 첫 줄을 번갈아 고쳐 헛 diff 를 낸다. 해시가 안 맞거나(손으로 고친 마커) 없으면
/// (옛 맨 마커) 새로 쓴다.
fn kept_marker<'a>(span: &'a str, written: &str, block: &str) -> Option<&'a str> {
    let first = span.split_inclusive('\n').next()?;
    let body = span.get(first.len()..span.rfind(END)?)?;
    let marker = first.trim_end();
    (is_begin(first) && body == written && marker.ends_with(&format!(" {} -->", hash_of(block)))).then_some(marker)
}

/// AGENTS.md 블록이 지금 바이너리가 쓸 글과 어떤가(moai-mstm).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BlockState {
    /// 다시 심어도 바이트가 같다.
    Current,
    /// 블록은 있는데 다시 심으면 바뀐다 — 다른 바이너리가 썼거나, 옛 맨 마커거나, 손으로
    /// 고쳤거나, 블록이 겹쳤거나, 마커에 머지 충돌이 남았다.
    Stale,
    /// 파일이 없거나 마커 한 쌍이 없다.
    Missing,
}

/// 파일 글(없는 파일은 빈 글)을 보고 블록의 상태를 가른다. **순수하다** — `init --check` 와
/// `status` 가 같은 자로 잰다.
///
/// **낡음을 가르는 것은 해시가 아니라 다시 심은 결과다.** 마커의 해시만 견주면 블록 안을
/// 손으로 고친 것을 못 본다(마커는 그대로다). [`with_block`] 이 그대로 돌려주면 `init` 이 할
/// 일이 없다는 뜻이라 그것이 곧 `current` 다 — 쓰는 길과 보는 길이 한 자를 쓰니 둘이 어긋날 수
/// 없다. 버전만 다른 블록은 [`kept_marker`] 가 마커를 두므로 다시 심어도 같아 `current` 다.
fn block_state(text: &str, block: &str) -> BlockState {
    if blocks(text).is_empty() {
        BlockState::Missing
    } else if with_block(text, block) == text {
        BlockState::Current
    } else {
        BlockState::Stale
    }
}

/// 낡은 까닭 둘(moai-mj45, 2026-09-15 사용자 결정). **마커가 제 블록 글의 해시를 대는지가 가른다.**
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stale {
    /// 마커가 제 글의 해시를 댄다 — **어떤 바이너리가 쓴 그대로다.** 그 바이너리가 이쪽보다
    /// 새것일 수 있다: main 을 받고 아직 다시 빌드 안 한 워크트리가 그렇다. 여기서 `init` 을
    /// 치면 새 안내가 옛 글로 되돌아가고, 그 되돌림이 다음 머지로 main 에 실린다.
    Binary,
    /// 해시가 제 글을 안 대거나(손으로 고쳤다) 아예 없다(옛 맨 마커). `init` 은 그 손질을 버린다.
    Edited,
}

/// 낡은 블록이 둘 중 어느 쪽인가. 파일에 선 **첫 블록**의 마커와 그 본문을 견준다.
///
/// 줄 끝은 파일의 것을 따르므로(CRLF) 본문을 LF 로 되돌려 잰다 — 쓸 때 해시는 LF 글 위에서
/// 났다([`begin_marker`]). 블록이 없으면 `Edited` 로 친다: 마커가 없으니 어느 바이너리가 썼다고
/// 말할 수 없고, 그때의 `init` 은 실제로 사람 글을 갈아 끼운다.
fn stale_kind(text: &str) -> Stale {
    let Some(&(start, stop)) = blocks(text).first() else { return Stale::Edited };
    let span = &text[start..stop];
    let Some(first) = span.split_inclusive('\n').next() else { return Stale::Edited };
    let body = span.get(first.len()..span.rfind(END).unwrap_or(span.len())).unwrap_or_default();
    let said = hash_of(&body.replace("\r\n", "\n"));
    if first.trim_end().ends_with(&format!(" {said} -->")) { Stale::Binary } else { Stale::Edited }
}

/// 이 저장소가 든 뿌리 파일 하나를 읽는다 — AGENTS.md·딸린 파일·CLAUDE.md 가 모두 이 하나로 읽는다.
/// **없는 파일만 `None` 이다** — 못 읽는 파일(권한·UTF-8 아님)을 빈 글로 치면 AGENTS.md 는 블록 하나로
/// 덮여 사람의 산문이 통째로 사라지고(moai-gq1c), 딸린 파일은 이미 있는 줄을 빠졌다고 한다.
///
/// **스냅샷과 같은 자로 읽는다**([`crate::held::read_inside`], moai-x0o7) — 체크아웃 안이고 `.git/` 밖인
/// 보통 파일을, 연 손잡이가 댄 크기까지만. 맨 `fs::read_to_string` 으로 읽던 판은 `mkfifo AGENTS.md`
/// 하나로 `moai status`·훅의 첫 보드·밖 한눈 보기가 쓰는 쪽을 영영 기다렸고, 커밋된
/// `AGENTS.md -> /dev/zero` 는 메모리를 다 썼다.
fn read_held(path: &Path, home: &crate::held::Home) -> Result<Option<String>, Fell> {
    match crate::held::read_inside(path, home) {
        Ok(t) => Ok(Some(t)),
        Err(Fell::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(fell) => Err(fell),
    }
}

/// 이 디렉터리의 AGENTS.md 를 읽는다([`read_held`]). 보는 길([`agents_state`])과 쓰는 길([`run`])이 이
/// 하나로 읽는다.
///
/// **안 읽기로 한 것도 `Err` 다**([`Fell::Unheld`]) — 보는 길은 상태를 지어내지 않고(`--check` 는 0 이 아니다),
/// 쓰는 길([`run`])은 그 파일만 못 건드린 자리로 대고 나머지를 심는다. 그 자리는 쓰기도 거절하므로
/// (`store::target_of`) 블록을 심을 길도 지킬 산문도 없어, 멈추는 것은 권한·UTF-8 로 못 읽은 것뿐이다. 거꾸로는 아니다 — 쓰기만 거절하는
/// 자리(락으로 풀리는 링크)는 읽히고, `init` 은 그 파일을 못 쓴 것으로 대고 이어 간다. 까닭은 부르는 쪽이
/// 고른 말로 편다([`agents_unread`]).
fn read_agents(root: &Path) -> Result<Option<String>, Fell> {
    read_held(&root.join("AGENTS.md"), &crate::held::Home::of(root))
}

/// 이 디렉터리의 AGENTS.md 를 못 읽은 한 줄 — `<자리>: <까닭>`([`read_agents`]).
fn agents_unread(lang: crate::i18n::Lang, root: &Path, fell: &Fell) -> String {
    unread_at(lang, root, "AGENTS.md", fell)
}

/// 이 디렉터리의 `name` 을 못 읽은 한 줄 — `<자리>: <까닭>`. [`agents_state`] 는 링크 모드에서 `.moai/guide.md` 도
/// 읽으므로 못 읽은 파일의 이름을 함께 낸다.
fn unread_at(lang: crate::i18n::Lang, root: &Path, name: &str, fell: &Fell) -> String {
    format!("{}: {}", root.join(name).display(), unread(lang, fell))
}

/// 저장소가 든 파일을 못 읽은 까닭 한 토막 — 안 읽기로 한 것은 고른 말로([`crate::held::said`]), io 가
/// 진 것은 운영체제가 낸 말 그대로다(앞 판들이 싣던 글이다).
fn unread(lang: crate::i18n::Lang, fell: &Fell) -> String {
    match fell {
        Fell::Unheld(why) => crate::held::said(lang, why),
        Fell::Io(e) => e.to_string(),
    }
}

/// 이 디렉터리의 AGENTS.md 를 읽어 [`block_state`] 로 가른다 — 그 상태와 **읽은 글**을 함께 낸다(없는 파일은
/// 빈 글). 없는 파일은 `missing` 이고, 못 읽는 파일(권한·UTF-8 아님, 안 읽기로 한 자리 — [`read_agents`])만
/// `Err` 다 — 그때는 상태를 지어내지 않는다.
///
/// **글을 함께 내는 것은 낡음의 갈래([`stale_kind`])를 같은 글에서 재려는 것이다**(리뷰 moai-x0o7.52k) — 갈래를
/// 재려고 다시 읽던 판은 파일을 두 번 열었고, 둘째 읽기가 지면 빈 글로 쳐 바이너리가 쓴 블록을 손으로 고쳤다고 댔다.
///
/// **못 읽은 자리는 그 파일의 이름과 함께 낸다**(리뷰 moai-zynt.63u) — 링크 모드는 `.moai/guide.md` 도 읽는데, 그
/// 실패를 AGENTS.md 의 것으로 대던 판은 멀쩡한 AGENTS.md 를 들여다보게 했다.
pub fn agents_state(root: &Path) -> Result<(BlockState, String), (&'static str, Fell)> {
    let text = read_agents(root).map_err(|fell| ("AGENTS.md", fell))?.unwrap_or_default();
    if !links_to_guide(&text) {
        return Ok((block_state(&text, &crate::guide::agents()), text));
    }
    // **링크 모드면 두 자리를 다 잰다**(moai-cbfz) — 링크 블록이 맞고, 그것이 가리키는 파일이 이 바이너리가
    // 쓸 전문과 같아야 맞다. 파일이 없어도 낡은 것이다: `init` 이 다시 쓴다.
    let link = block_state(&text, &crate::guide::agents_link());
    if link != BlockState::Current {
        return Ok((link, text));
    }
    let path = root.join(crate::guide::GUIDE_FILE);
    let guide = read_held(&path, &crate::held::Home::of(root)).map_err(|fell| (crate::guide::GUIDE_FILE, fell))?;
    let state = match guide.is_some_and(|g| same_guide(&g)) {
        true => BlockState::Current,
        false => BlockState::Stale,
    };
    Ok((state, text))
}

/// `.moai/guide.md` 의 글이 이 바이너리가 쓸 전문과 같은가 — **줄 끝은 안 가린다**(리뷰 moai-zynt.63u). 그 파일은
/// `.moai` 와 함께 커밋되어 `core.autocrlf` 체크아웃에서 CRLF 로 오는데, 바이트로 견주던 판은 늘 낡았다고 했고
/// `init` 이 LF 로 다시 써 작업 트리에 헛 diff 를 남겼다. 블록이 파일의 줄 끝을 따르는 것([`with_block`])과 같은 자다.
fn same_guide(text: &str) -> bool {
    text.replace("\r\n", "\n") == crate::guide::agents()
}

/// 낡았다고 잰 AGENTS.md 에서 낡은 것이 링크 블록이 아니라 `.moai/guide.md` 인가 — 블록은 맞는 링크다. 링크가
/// 전문의 해시를 들어([`crate::guide::agents_link`]) 다른 바이너리가 쓴 전문이면 링크부터 갈리므로, 여기 남는 것은
/// 이 바이너리의 링크 밑에서 파일만 다른 판 — 손으로 고쳤거나 없어진 것이다.
fn guide_file_stale(text: &str) -> bool {
    links_to_guide(text) && block_state(text, &crate::guide::agents_link()) == BlockState::Current
}

/// AGENTS.md 의 블록이 `.moai/guide.md` 를 가리키는 링크인가 — 그러면 안내는 파일 모드로 심긴 것이다.
/// 모드를 어디에도 적지 않고 이 글에서 읽는다. **링크의 끝줄([`crate::guide::GUIDE_MARK`])로 알아본다** — 경로가
/// 든 글인지로 가르던 판은 그 경로를 적은 손질 한 줄에 블록 모드를 링크로 갈아 끼웠다(리뷰 moai-zynt.63u).
fn links_to_guide(text: &str) -> bool {
    blocks(text).first().is_some_and(|&(start, stop)| {
        text[start..stop].lines().any(|l| l.trim_start().starts_with(crate::guide::GUIDE_MARK))
    })
}

/// 다시 부른 `init` 이 맞출 안내 — **서 있는 것을 맞춘다.** 블록이 링크면 파일, 블록이 있으면 블록이다. 블록이
/// 없을 때만 추적이 가른다: git 밖의 트래커는 커밋되는 AGENTS.md 를 안 건드리고, 커밋하는 트래커는 지금까지처럼 심는다.
///
/// **git 밖이어도 서 있는 블록은 맞춘다**(리뷰 moai-zynt.63u) — `--guide block` 으로 골라 심은 블록을 안 건드리던
/// 판은 그 블록이 낡으면 `status` 와 `--check` 가 `moai init` 을 대는데 그 `init` 은 "다 맞아 있다" 고 하고 말아,
/// 알림이 영영 안 걷혔다. 못 읽는 AGENTS.md 는 블록으로 친다: 그 갈래는 `run` 이 아무것도 쓰기 전에 제 말로 멈춘다.
fn guide_of(root: &Path, tracking: Tracking) -> Guide {
    match read_agents(root) {
        Ok(Some(text)) if links_to_guide(&text) => Guide::File,
        Ok(Some(text)) if !blocks(&text).is_empty() => Guide::Block,
        _ if !tracking.tracked() => Guide::None,
        _ => Guide::Block,
    }
}

/// 이 클론에만 둔 트래커가 무시 블록에 더하는 줄(moai-zynt.own). 훅·스킬의 플러그인 트리(`moai skill install`)와
/// `claude` 가 `--scope local` 등록을 적는 파일도 같이 막는다 — 그것만 남으면 `git status` 가 추적 안 된 파일로 비춘다.
const LOCAL_IGNORE: &str = "\
# This tracker stays in this clone - git does not track it.
/.moai/
/.claude/moai-plugin/
/.claude/settings.local.json
";

/// git 이 이 뿌리를 어디에 두는가 — 공통 디렉터리(`--git-common-dir`, 워크트리·서브모듈에서도 `info/exclude` 가
/// 사는 곳)와 **작업 트리 꼭대기에서 이 뿌리까지의 길**(`--show-prefix`, 꼭대기면 빈 글 — 모노레포의 한 칸이면
/// `svc/`), 그리고 **딸린 워크트리인가**(제 git 디렉터리가 공통 디렉터리와 갈린다 — 서브모듈은 둘이 같다). git
/// 저장소가 아니거나 `budget` 안에 답이 없으면 `None`.
struct GitPlace {
    dir: std::path::PathBuf,
    under: String,
    linked: bool,
}

fn git_place(root: &Path, budget: Option<std::time::Duration>) -> Option<GitPlace> {
    let args = ["rev-parse", "--git-dir", "--git-common-dir", "--show-prefix"];
    let out = crate::git::run_within(root, &args, budget)?.ok()?;
    let mut lines = out.lines();
    let (own, dir) = (lines.next()?.trim(), lines.next()?.trim());
    let under = lines.next().unwrap_or_default().to_string();
    if own.is_empty() || dir.is_empty() {
        return None;
    }
    let (own, dir) = (root.join(own), root.join(dir));
    let linked = crate::path::real(&own) != crate::path::real(&dir);
    Some(GitPlace { dir, under, linked })
}

/// 심긴 트래커를 git 이 추적하는가 — **git 에 묻는다**(`check-ignore`). 무시한 규칙이 작업 트리 안의 `.gitignore`
/// 의 것이면 그쪽이고, 그 밖의 자리(`info/exclude`, 사람의 전역 무시 파일)는 이 클론의 것으로 읽는다([`ignored_by`]).
/// git 저장소가 아니면 지금까지의 `init` 처럼 커밋으로 친다.
fn tracking_of(root: &Path) -> Tracking {
    tracking_within(root, None).unwrap_or(Tracking::Commit)
}

/// [`tracking_of`] 를 한도 안에서 — 마감을 넘기면 `None`(모른다). git 이 실패한 것(저장소가 아니다, 무시되지 않는다
/// — `check-ignore` 는 안 걸리면 1 로 끝난다)은 커밋으로 친다.
///
/// **디렉터리를 묻는다**(`.moai/` — 끝의 `/` 가 디렉터리라고 대므로 없는 자리에도 디렉터리 규칙이 걸린다, 리뷰
/// moai-zynt.63u). 안의 파일 하나(`config.toml`)를 묻던 판은 그 파일에만 걸린 남의 규칙(`config.toml`·`*.toml`)에
/// 트래커 통째를 git 밖으로 읽었고, 다시 부른 `init` 이 그 말을 따라 `/.moai/` 를 `.gitignore` 에 적었다. 커밋된
/// 트래커는 안에 추적되는 파일이 있어 무시 규칙이 있어도 git 이 디렉터리를 안 댄다.
fn tracking_within(root: &Path, budget: Option<std::time::Duration>) -> Option<Tracking> {
    let said = crate::git::run_within(root, &["check-ignore", "-v", ".moai/"], budget)?;
    let Ok(said) = said else { return Some(Tracking::Commit) };
    Some(ignored_by(&said))
}

/// `check-ignore -v` 가 댄 규칙이 무엇을 뜻하는가 — 한 줄은 `<자리>:<줄 번호>:<패턴>\t<경로>` 다.
///
/// - **되살리는 패턴(`!…`)이면 무시되지 않은 것이다**(리뷰 moai-zynt.63u). `-v` 는 그 줄도 대고 0 으로 끝난다 —
///   허용 목록 꼴의 `.gitignore`(`*` · `!*/`)가 그렇다. 그것을 무시로 읽던 판은 커밋할 트래커를 `.gitignore` 의 것으로
///   읽어 다시 부른 `init` 이 `/.moai/` 를 그 파일에 적었다
/// - 자리가 **작업 트리 안의 `.gitignore`** 면 그쪽이다. git 은 그 자리를 꼭대기에서 잰 상대 경로로 대므로 하위
///   디렉터리의 트래커는 `svc/.gitignore` 다(리뷰) — 이름이 `.gitignore` 와 같은지로 가르던 판은 그것을 이 클론의
///   것으로 읽었다. 이상한 글자가 든 경로는 따옴표로 싸여 온다
/// - 그 밖(`.git/info/exclude`, 딸린 워크트리가 대는 공통 디렉터리의 절대 경로, 사람의 전역 무시 파일)은 이 클론의 것이다
fn ignored_by(said: &str) -> Tracking {
    let Some((source, pattern)) = said.lines().next().and_then(rule_of) else { return Tracking::Commit };
    if pattern.starts_with('!') {
        return Tracking::Commit;
    }
    let source = Path::new(source.trim_matches('"'));
    match source.is_relative() && source.file_name().is_some_and(|n| n == ".gitignore") {
        true => Tracking::Gitignore,
        false => Tracking::Exclude,
    }
}

/// `check-ignore -v` 의 한 줄을 `(자리, 패턴)` 으로 — **자리에도 `:` 가 설 수 있어**(윈도의 `C:/…`) 숫자뿐인 칸을
/// 줄 번호로 찾는다. 모양이 안 맞으면 `None`.
fn rule_of(line: &str) -> Option<(&str, &str)> {
    let (head, _) = line.rsplit_once('\t')?;
    let mut from = 0;
    while let Some(i) = head[from..].find(':') {
        let colon = from + i;
        if let Some((number, pattern)) = head[colon + 1..].split_once(':')
            && !number.is_empty()
            && number.bytes().all(|b| b.is_ascii_digit())
        {
            return Some((&head[..colon], pattern));
        }
        from = colon + 1;
    }
    None
}

/// 플래그끼리 부딪힌 까닭 — 화면과 플래그가 같은 말을 한다.
fn clash_said(lang: crate::i18n::Lang, c: crate::init_choice::Conflict) -> String {
    match c {
        crate::init_choice::Conflict::HookWithoutSkill => say(lang, "refuse.init_hook_without_skill").to_string(),
    }
}

/// 이 클론에만 두려 할 때 못 두는 까닭. git 저장소가 아니면 둘 자리(`info/exclude`)가 없고, `.gitignore` 에 적어도
/// 막을 git 이 없다.
///
/// **딸린 워크트리면 막는 줄이 주 체크아웃까지 가린다**(리뷰 moai-zynt.63u) — `info/exclude` 는 모든 워크트리가 함께
/// 쓰고 `.gitignore` 의 줄은 머지로 돌아가, 거기 커밋된 트래커의 새 파일(새 사람의 저널)이 말없이 커밋에서 빠진다.
/// 워크트리 거절문이 대는 `MOAI_HERE=1 moai init` 이 그 자리로 가는 길이다. 처음 심을 때만 잰다 — 이미 선 트래커를
/// 맞추는 `init` 을 막으면 고칠 길이 도구 밖에만 남는다.
fn local_refusal(lang: crate::i18n::Lang, tracking: Tracking, place: Option<&GitPlace>) -> Option<String> {
    if tracking.tracked() {
        return None;
    }
    match place {
        None => Some(say(lang, "refuse.init_local_no_git").to_string()),
        Some(p) if p.linked => Some(say(lang, "refuse.init_local_linked").to_string()),
        Some(_) => None,
    }
}

/// **플래그로 준 칸이 그 자체로 틀렸으면 묻기 전에 거절한다**(리뷰 moai-zynt.63u). 화면은 플래그의 칸을 잠가
/// 못 고치게 하므로, 그 값이 틀린 채 화면을 열면 Enter 는 영영 같은 까닭으로 머물고 나갈 길은 Esc 하나다 —
/// 다 물어 놓고 거절하는 셈이다. 재는 것은 화면이 Enter 에서 재는 것과 같은 자다(접두어·git 밖·부딪힘). 부딪힘은
/// 안내까지 플래그로 정한 때만 본다 — 안내가 화면의 칸이면 사람이 거기서 고친다.
fn refuse_fixed(
    lang: crate::i18n::Lang,
    fixed: &Choice,
    defaults: &crate::init_choice::Defaults,
    place: Option<&GitPlace>,
) -> R<()> {
    if let Some(p) = &fixed.prefix {
        fresh_prefix(lang, p)?;
    }
    if let Some(why) = fixed.tracking.and_then(|t| local_refusal(lang, t, place)) {
        return Err(Fail::coded(why, super::code::BAD_INPUT));
    }
    let clash =
        fixed.guide.and_then(|_| crate::init_choice::conflict(fixed, &crate::init_choice::resolve(fixed, defaults)));
    match clash {
        Some(c) => Err(Fail::coded(clash_said(lang, c), super::code::BAD_INPUT)),
        None => Ok(()),
    }
}

/// `.gitignore` 에 심을 블록 — **트래커가 링크 너머에 살면 그 자리의 락과 임시 파일도 막는다**(moai-th3b).
///
/// 블록의 `.moai/lock`·`.moai/*.tmp.*` 는 링크 경로를 가리키는데, git 은 링크를 따라가지 않는다. 막을 자리는
/// 둘이고 막는 넓이가 다르다.
///
/// - **`.moai` 가 링크면**(`.moai -> tracker`) 락과 임시 파일이 모두 그 자리(`tracker/`)에 선다. `init` 이 갈아
///   끼우는 `AGENTS.md` 의 임시 파일도 거기 선다([`tmp_dir`]) — 그 자리는 moai 의 디렉터리라 블록의 `.moai/`
///   줄을 통째로 옮긴다
/// - **트래커 파일만 링크면**(`.moai/issues.jsonl -> ../shared/issues.jsonl`) 그 파일이 든 자리에는
///   `store::Repo::far_lock` 의 `lock` 과, `.moai/` 에서 못 갈아끼울 때 물러선 **그 파일 이름의** 임시 파일만
///   선다. 그 자리는 사람의 디렉터리라 `*.tmp.*` 를 그대로 옮기면 남의 파일(`shared/release.tmp.md`)까지
///   가려 `git add -A` 가 말없이 빠뜨린다(리뷰) — 그래서 `*` 를 그 파일 이름으로 좁힌다. 가리키는 파일이
///   뿌리에 있으면 그 자리는 뿌리다(`/lock`)
///
/// 줄을 손으로 적지 않고 블록의 `.moai/` 줄을 그 자리로 옮겨 적는다([`mirrored`]) — 블록이 바뀌는 날 옮긴
/// 줄도 같이 바뀐다. 줄로 못 거는 자리([`inside`] 가 거절하는 곳)는 뺀다 — 그 트래커는 `moai status` 의 링크
/// 알림이 비춘다(`.git/` 안은 그 앞에서 `status` 가 트래커를 못 읽는다고 멈춘다).
fn gitignore_for(root: &Path) -> std::borrow::Cow<'static, str> {
    let own = moai_moved(root);
    let mut lines = own.as_deref().map(|dir| mirrored(GITIGNORE, "", dir, None)).unwrap_or_default();
    if let Some(rel) = linked_snapshot(root) {
        let (dir, name) = rel.rsplit_once('/').unwrap_or(("", rel.as_str()));
        // 제자리(`.moai`)의 줄은 블록이 이미 막고, `.moai` 가 푼 자리의 줄은 바로 위에서 통째로 옮겼다.
        if dir != ".moai" && own.as_deref() != Some(dir) {
            lines.push_str(&mirrored(GITIGNORE, "", dir, Some(name)));
        }
    }
    if lines.is_empty() {
        return GITIGNORE.into();
    }
    format!("{GITIGNORE}{LINKED_IGNORE_COMMENT}{lines}").into()
}

/// [`gitignore_for`] 가 더하는 줄 위의 주석. **글을 고치지 않는다** — 까닭은 [`LINKED_COMMENT`] 와 같다.
const LINKED_IGNORE_COMMENT: &str = "\
# The tracker lives behind a link. Its lock and temporary files sit where the
# link points, so they are ignored there too.
";

/// 블록의 `.moai/<under>` 로 시작하는 줄을 **뿌리에 못박은 `/<dir>/` 로 옮겨 적는다** — `dir` 이 빈 글이면
/// 뿌리 바로 밑이다(`/lock`). 앞 `/` 는 [`attributes_for`] 의 줄과 같은 까닭이다 — 없으면 어느 깊이의 같은
/// 이름에도 걸린다. `name` 을 받으면 파일 이름 자리의 `*` 를 그 이름으로 좁힌다(`*.tmp.*` →
/// `issues.jsonl.tmp.*`) — 그 까닭은 [`gitignore_for`] 에 있다.
fn mirrored(block: &str, under: &str, dir: &str, name: Option<&str>) -> String {
    block
        .lines()
        .filter_map(|l| l.strip_prefix(".moai/"))
        .filter(|rest| rest.starts_with(under))
        .map(|rest| {
            let rest = match name.zip(rest.strip_prefix('*')) {
                Some((name, tail)) => format!("{name}{tail}"),
                None => rest.to_string(),
            };
            match dir {
                "" => format!("/{rest}\n"),
                dir => format!("/{dir}/{rest}\n"),
            }
        })
        .collect()
}

/// `.moai` 가 **딴 자리로 풀렸으면** 그 자리 — 뿌리 안의 경로로(뿌리 자체면 빈 글). 제자리거나 없거나 줄로
/// 못 거는 자리면 `None` 이다. 락과 임시 파일과 저널이 받은 철자(`.moai/…`)로 가는 곳이 여기다 —
/// [`gitignore_for`] 와 [`attributes_for`] 가 이 하나로 잰다.
fn moai_moved(root: &Path) -> Option<String> {
    inside(root, &std::fs::canonicalize(root.join(".moai")).ok()?).filter(|dir| dir != ".moai")
}

/// `.gitattributes` 에 심을 블록 — **트래커가 체크아웃 안의 파일을 가리키는 링크면 그 파일에도
/// `merge=moai` 를 건다**(moai-7myd).
///
/// git 은 링크를 링크 글로 담고, 줄이 사는 파일은 가리켜진 경로로 따로 담는다. 병합에서 실제로
/// 합쳐지는 것은 그 파일인데 선언은 링크 경로에만 걸려, 그 파일이 git 의 기본 글 병합을 받아
/// 충돌 표식을 든 JSONL 이 됐다 — 그런데 `init --check` 는 드라이버가 `current` 라고 했다.
/// 선언을 재는 [`crate::cmd::merge_driver`] 의 `declared` 는 그대로 링크 경로를 묻는다. 빠진 것은
/// 이 한 줄이고, 그것은 [`gaps_in`] 가 `gitattributes_rules` 로 비춘다.
///
/// **뿌리에 못박는다**(`/<경로>`). `/` 없는 한 조각 패턴은 어느 깊이의 같은 이름에도 걸린다.
/// 쓸 수 없는 경로는 줄을 안 세운다 — 뿌리 밖(위 디렉터리·체크아웃 밖, 쓰기가 거절하는 자리다)이거나
/// `.git/` 안이거나 패턴 글자·빈칸·제어 문자가 든 이름이다([`inside`]). 그 트래커는 `moai status` 의 링크
/// 알림이 비춘다(moai-jo3h) — `.git/` 안은 그 앞에서 `status` 가 트래커를 못 읽는다고 멈춘다.
///
/// **거는 속성은 스냅샷 줄의 것을 그대로 옮긴다**(리뷰) — 손으로 다시 적으면 그 줄이 바뀌는 날 병합이
/// 실제로 도는 이 파일만 옛 속성에 남는다. 스냅샷 줄이 드라이버를 거는지는
/// `the_declared_path_is_the_one_init_writes` 가 맨다.
///
/// **`.moai` 가 링크면 저널 줄도 그 자리로 옮긴다**(moai-th3b). 저널은 `.moai` 안에 살아 `.moai -> tracker`
/// 면 `tracker/journal/*.jsonl` 로 합쳐지는데, 블록의 `.moai/journal…` 줄은 거기 안 걸려 union 대신
/// 기본 병합을 받는다. 끝 조각만 링크인 경우에는 저널이 제자리라 옮길 것이 없다.
fn attributes_for(root: &Path) -> std::borrow::Cow<'static, str> {
    let mut out = String::new();
    if let Some(rel) = linked_snapshot(root) {
        let snapshot = crate::cmd::merge_driver::SNAPSHOT;
        let attrs = GITATTRIBUTES
            .lines()
            .rev()
            .find_map(|l| l.strip_prefix(snapshot).filter(|rest| rest.starts_with(char::is_whitespace)))
            .unwrap_or_default();
        out.push_str(&format!("{LINKED_COMMENT}/{rel}{attrs}\n"));
    }
    if let Some(dir) = moai_moved(root) {
        out.push_str(LINKED_JOURNAL_COMMENT);
        out.push_str(&mirrored(GITATTRIBUTES, "journal", &dir, None));
    }
    if out.is_empty() { GITATTRIBUTES.into() } else { format!("{GITATTRIBUTES}{out}").into() }
}

/// [`attributes_for`] 가 더하는 줄 위의 주석. **글을 고치지 않는다** — 줄 단위로 견주어 덧붙이므로
/// 고친 글은 이미 심은 저장소에 한 벌 더 붙는다(`GITATTRIBUTES` 위의 글과 같은 까닭이다).
const LINKED_COMMENT: &str = "\
# .moai/issues.jsonl is a link. git merges the file it points at, so that file
# carries the same attributes.
";

/// [`attributes_for`] 가 `.moai` 가 링크일 때 저널 줄 위에 더하는 주석. **글을 고치지 않는다.**
const LINKED_JOURNAL_COMMENT: &str = "\
# .moai is a link. The journal lives where it points, so it merges there with
# the same rule.
";

/// 이 뿌리의 트래커 줄이 **실제로 사는 파일** — 끝 조각의 링크 사슬을 따라가고([`crate::path::follow_links`])
/// 그 파일이 든 디렉터리를 통째로 푼다. 끝 파일은 없어도 된다. 사슬이 고리거나 그 디렉터리가 없으면
/// `None` 이다.
///
/// **디렉터리를 통째로 푸는 까닭**은 `.moai` 가 링크인 판이다(리뷰). 끝 조각만 보던 판은
/// `.moai -> tracker` 에서 링크를 못 보고 병합 줄도 알림도 안 세웠는데, git 은 거기서도 줄이 사는
/// `tracker/issues.jsonl` 을 그 경로로 합쳐 충돌 표식을 냈다.
///
/// **거는 쪽([`linked_snapshot`])과 비추는 쪽(`cmd::status` 의 링크 알림)이 이 하나로 푼다** — 둘이 따로
/// 풀던 판은 링크를 알아보는 길부터 갈렸다(리뷰).
pub(crate) fn tracker_file(root: &Path) -> Option<std::path::PathBuf> {
    let end = crate::path::follow_links(&root.join(crate::cmd::merge_driver::SNAPSHOT)).ok()?;
    Some(std::fs::canonicalize(crate::path::dir_of(&end)).ok()?.join(end.file_name()?))
}

/// 트래커가 링크면 **이 뿌리 안에서** 그것이 가리키는 파일의 경로(`shared/issues.jsonl`). 링크가 아니거나
/// [`attributes_for`] 가 줄로 못 거는 자리면 `None` 이다 — 푼 자리가 제자리(`.moai/issues.jsonl`)면 링크가
/// 아니다. 줄로 못 거는 이름은 [`inside`] 가 가른다.
fn linked_snapshot(root: &Path) -> Option<String> {
    inside(root, &tracker_file(root)?).filter(|rel| rel != crate::cmd::merge_driver::SNAPSHOT)
}

/// 푼 경로 `path` 를 **규칙 줄에 실을 수 있는** 뿌리 안의 경로로 — 뿌리 자체는 빈 글이다. 뿌리 밖이거나
/// `.git/` 안이거나 빈칸·패턴 글자·제어 문자가 든 이름이면 `None` 이다. 규칙 줄을 짓는 자리([`linked_snapshot`]·
/// [`moai_moved`])가 모두 이 하나로 잰다.
///
/// **제어 문자가 든 이름도 줄로 안 건다**(리뷰). 받은 저장소가 링크와 그 이름의 디렉터리를 커밋하면 그
/// 글자가 `init --check` 와 `init` 이 찍는 규칙 줄에 그대로 실려 터미널을 다시 칠한다.
///
/// **뿌리 자체는 받는다**(리뷰). 파일은 뿌리일 수 없어 빈 글이 안 나오지만 디렉터리는 뿌리일 수 있다 —
/// `.moai/issues.jsonl -> ../issues.jsonl` 이면 `store::Repo::far_lock` 이 뿌리에 `lock` 을 세운다. 빈 글을
/// 거절하던 때는 그 락에 줄이 안 서서 `git add -A` 가 그것을 담았고, `init --check` 는 빠진 것이 없다고 했다.
///
/// **git 의 자리(`.git/`)는 줄로 안 건다**(moai-x0o7.2q8) — 트래커 링크가 `.git/` 으로 풀리면 `init` 이
/// `/.git/<…>/issues.jsonl … merge=moai` 와 `/.git/<…>/lock` 을 심었다. git 은 제 자리를 담지도 합치지도
/// 않아 그 줄은 뜻이 없고, `.moai` 가 그리 가면 쓰기도 읽기도 이미 거절한다. 재는 자는 그 둘(`store::resolve`·
/// [`crate::held::place`])과 한 몸인 [`crate::held::into_git`] 이다 — 끝 이름도 세고(딸린 워크트리의 `.git` 은
/// 파일이다) 대소문자를 안 가린다.
fn inside(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(crate::path::real(root)).ok()?;
    if crate::held::into_git(rel) {
        return None;
    }
    let rel = rel.to_str()?;
    let plain = |c: char| !c.is_whitespace() && !c.is_control() && !"*?[]\\\"#!".contains(c);
    rel.chars().all(plain).then(|| rel.to_string())
}

/// 이 저장소의 딸린 파일에서 **빠진 규칙** — `(파일 이름, 알림의 갈래, 빠진 줄들)`, 빠진 것이
/// 있는 파일만. 재는 파일은 [`dotfiles`] 가 고른다 — 쓰는 길([`run`])이 쓰는 바로 그 목록이다.
///
/// **못 읽는 파일은 여기서 말하지 않는다.** 무엇이 들었는지 모르니 빠졌다고 할 수 없고 — 그 줄이
/// 이미 있을 수도 있다 — `init` 도 그 파일은 안 건드리므로 한 번 선 알림이 **영영 안 걷힌다.**
/// 그 자리는 `init` 이 제 이름으로 이미 말한다.
///
/// **없는 파일은 통째로 빠진 것이다.** `git add -A` 가 옆 워크트리를 담는 위험이 가장 큰 자리라
/// (moai-mxtb) 입을 다물면 안 된다. 갈림은 **오류의 갈래로** 짓는다 — `exists()` 로 물으면 못 읽는
/// 파일과 없는 파일이 정확히 거꾸로 선다.
///
/// **링크인 파일도 여기서 말하지 않는다**(moai-yke5) — git 은 2.32 부터 체크아웃 안의 링크인 딸린 파일을
/// 안 읽어 무엇이 들었든 규칙이 하나도 안 선다. 빠진 줄을 대면 `init` 이 못 걷는 알림이 영영 서므로 그
/// 파일은 [`linked_in`] 이 제 낱말로 말한다. 그보다 옛 git 은 그 링크를 읽지만 `init` 은 거기에도 안
/// 쓴다 — git 을 올리는 날 말없이 꺼지는 규칙이라, 보통 파일로 바꾸라는 말은 어느 버전에서나 맞다.
///
/// **읽는 자는 [`ensure_lines`] 와 같다**([`crate::held::read_inside`], moai-x0o7) — `mkfifo .gitignore` 하나로
/// `moai status` 가 멈추던 자리다. 안 읽기로 한 파일도 못 읽은 파일이라 말하지 않는다.
fn gaps_in(files: &[Dotfile]) -> Vec<(&'static str, &'static str, Vec<String>)> {
    files
        .iter()
        .filter(|d| !d.path.is_symlink())
        .filter_map(|d| {
            let Ok(text) = read_held(&d.path, &crate::held::Home::of(crate::path::dir_of(&d.path))) else {
                return None;
            };
            let text = text.unwrap_or_default();
            let missing: Vec<String> = missing_rules(&text, &d.block).into_iter().map(str::to_string).collect();
            (!missing.is_empty()).then_some((d.name, d.kind, missing))
        })
        .collect()
}

/// `init` 이 맞추는 딸린 파일 하나 — 이름(사람에게 대는 글), 자리, 그 안에 서야 할 블록, 알림의 갈래.
///
/// 읽고 쓸 때 견주는 뿌리는 **그 파일이 든 디렉터리다**(`crate::path::dir_of`) — `.git/info/exclude` 를 체크아웃을
/// 뿌리로 재면 읽는 자가 `.git/` 안이라 거절한다([`crate::held::Unheld::IntoGit`]). 쓰는 길([`ensure_lines`])도 그
/// 자를 쓴다.
struct Dotfile {
    name: &'static str,
    path: std::path::PathBuf,
    block: String,
    kind: &'static str,
}

/// 이 저장소에서 `init` 이 맞추는 딸린 파일들 — **추적 방식에 따라 갈린다**(moai-zynt.zhr). git 밖에 둔 트래커는
/// 병합 규칙이 할 일이 없어 `.gitattributes` 가 없고, 무시 블록은 고른 자리(`.git/info/exclude`·`.gitignore`)에서
/// 트래커를 막는 줄까지 함께 든다.
///
/// **쓰는 길([`run`])과 비추는 길([`dotfile_notice`]·[`check`])이 이 하나로 짓는다**(리뷰 moai-zynt.63u) — 이름·자리·
/// 블록을 `run` 이 제 `match` 셋으로 다시 짓던 판은, 하위 디렉터리의 `info/exclude` 처럼 한쪽만 고쳐질 자리를 남겼다.
/// `.git/info/exclude` 는 git 의 자리(`place`)를 알아야 서고, 모르면 목록에 없다.
fn dotfiles(root: &Path, tracking: Tracking, place: Option<&GitPlace>) -> Vec<Dotfile> {
    // 이름과 블록을 한 자리에서 짓는다 — 이름으로 블록을 고르는 `match` 를 따로 두던 판은 `_` 갈래가 모르는
    // 이름에 `.gitignore` 블록을 줬다(리뷰). `.gitattributes` 의 블록은 저장소마다 한 줄이 더 설 수 있다([`attributes_for`]).
    let attributes = tracking.tracked().then(|| Dotfile {
        name: ".gitattributes",
        path: root.join(".gitattributes"),
        block: attributes_for(root).into_owned(),
        kind: "gitattributes_rules",
    });
    let ignore = match tracking {
        Tracking::Commit => Some(Dotfile {
            name: ".gitignore",
            path: root.join(".gitignore"),
            block: gitignore_for(root).into_owned(),
            kind: "gitignore_rules",
        }),
        // 갈래를 커밋하는 `.gitignore` 와 가른다(리뷰 moai-zynt.63u) — 빠졌을 때의 결과가 "옆 워크트리가 딸려 간다" 가
        // 아니라 트래커와 그 파일이 커밋되는 것이라, 같은 갈래로 서면 엉뚱한 까닭을 댄다.
        Tracking::Gitignore => Some(Dotfile {
            name: ".gitignore",
            path: root.join(".gitignore"),
            block: local_block(root, ""),
            kind: "gitignore_local_rules",
        }),
        Tracking::Exclude => place.map(|p| Dotfile {
            name: ".git/info/exclude",
            path: p.dir.join("info").join("exclude"),
            block: local_block(root, &p.under),
            kind: "exclude_rules",
        }),
    };
    attributes.into_iter().chain(ignore).collect()
}

/// 이 클론에만 둔 트래커의 무시 블록 — `.gitignore` 의 블록에 트래커와 훅 자리([`LOCAL_IGNORE`])를 더한다.
/// `under` 는 그 블록이 설 파일의 뿌리에서 이 뿌리까지의 길이다([`anchored`]) — `.gitignore` 는 제 디렉터리가
/// 뿌리라 빈 글이다.
fn local_block(root: &Path, under: &str) -> String {
    let block = format!("{}{LOCAL_IGNORE}", gitignore_for(root));
    if under.is_empty() {
        return block;
    }
    block.lines().map(|l| format!("{}\n", anchored(l, under))).collect()
}

/// 무시 블록의 한 줄을 `under`(작업 트리 꼭대기에서 뿌리까지의 길, `svc/`) 밑으로 옮겨 적는다 — **`.git/info/exclude`
/// 의 줄은 작업 트리 꼭대기가 뿌리다**(리뷰 moai-zynt.63u). 하위 디렉터리(모노레포의 한 칸)에 심은 트래커의 `/.moai/`
/// 를 그대로 적던 판은 그 트래커를 못 막고 꼭대기의 `.moai/` 를 막았다 — 꼭대기에 커밋된 트래커가 있으면 새 저널
/// 파일이 말없이 무시됐고, 다시 부른 `init` 은 하위 트래커를 커밋으로 읽어 커밋되는 파일을 심었다.
///
/// 주석과 빈 줄은 그대로다. `/` 를 품은 줄은 제 자리에 못박힌 줄이라 길을 앞에 붙이고, 안 품은 줄(어느 깊이에나
/// 걸리는 줄)은 `**/` 를 끼워 그 넓이를 지킨다. 길에 든 패턴 글자는 `\` 로 막는다.
fn anchored(line: &str, under: &str) -> String {
    if under.is_empty() || line.trim().is_empty() || line.starts_with('#') {
        return line.to_string();
    }
    let (not, rule) = line.strip_prefix('!').map_or(("", line), |rest| ("!", rest));
    let any_depth = !rule.starts_with('/') && !rule.trim_end_matches('/').contains('/');
    let under: String =
        under.chars().flat_map(|c| ["*?[\\".contains(c).then_some('\\'), Some(c)].into_iter().flatten()).collect();
    format!("{not}/{under}{}{}", if any_depth { "**/" } else { "" }, rule.trim_start_matches('/'))
}

/// 비추는 길이 재는 딸린 파일 — 추적 방식(`check-ignore`)과 `info/exclude` 의 자리(`rev-parse`)를 git 에 묻는다.
///
/// `budget` 은 **둘이 나눠 쓰는** 한도다 — 알림만 주고(`moai status`·훅의 보드·한눈 보기), 마감 안에 답이 없으면
/// **아무 파일도 안 잰다**: 모르는 것을 커밋으로 치면 git 밖의 트래커에 `.gitattributes` 를 조른다. 죽은 마운트에 선
/// 프로젝트 하나가 한눈 보기를 붙들던 자리(moai-59k3.u09)라 머지 드라이버의 알림과 같은 한도를 쓴다. `rev-parse` 를
/// 한도 없이 묻던 판은 그 물음 하나가 멈추면 보드가 통째로 멈췄다(리뷰 moai-zynt.63u). 공통 디렉터리를 못 찾으면
/// 잴 자리가 없다 — 말하지 않는다. 읽는 길이라 `info/` 를 만들지 않는다.
fn dotfiles_within(root: &Path, budget: Option<std::time::Duration>) -> Vec<Dotfile> {
    let deadline = budget.map(|b| std::time::Instant::now() + b);
    let left = || deadline.map(|d| d.saturating_duration_since(std::time::Instant::now()));
    let Some(tracking) = tracking_within(root, left()) else { return Vec::new() };
    let place = match tracking {
        Tracking::Exclude => match git_place(root, left()) {
            Some(place) => Some(place),
            None => return Vec::new(),
        },
        Tracking::Commit | Tracking::Gitignore => None,
    };
    dotfiles(root, tracking, place.as_ref())
}

/// **링크인** 딸린 파일의 이름(moai-yke5). git 은 2.32 부터 체크아웃 안의 `.gitattributes`·`.gitignore` 가
/// 링크면 따라가지 않는다 — 가리키는 파일이 체크아웃 안이든 밖이든 그 규칙은 안 선다. `init` 은 그 파일에
/// 안 쓰고([`ensure_lines`]), 고치는 길은 보통 파일로 바꾸는 것 하나라 알림도 `moai init` 을 대지 않는다.
///
/// 링크인지는 **따라가지 않고** 잰다(`Path::is_symlink`) — 못 재면 링크가 아니고, 그 파일은 읽는 길이 제
/// 말로 댄다.
fn linked_in(files: &[Dotfile]) -> Vec<&'static str> {
    files.iter().filter(|d| d.path.is_symlink()).map(|d| d.name).collect()
}

/// 고칠 명령이 `-C <뿌리>` 를 대야 하는가 — 그렇다면 셸에 붙여 넣을 모양의 뿌리.
///
/// **부른 사람의 셸이 뿌리에 있지 않을 수 있을 때** 댄다 — 부른 자리가 뿌리가 아니거나
/// (`chdir` 이 아니어도) `-C` 로 옮겨 왔을 때. 재는 것은 뿌리의 파일인데 `init` 은 부른 자리에
/// 심는다: 하위 디렉터리에서, 또는 `moai -C <프로젝트> status` 를 본 셸에서 맨 `moai init` 을
/// 따라 치면 그 자리에 트래커가 하나 더 섰다.
///
/// **알림마다 따로 재지 않는다** — 한 화면에 서는 두 알림이 서로 다른 뿌리를 대면 그 중 하나는
/// 반드시 엉뚱한 곳을 가리킨다.
pub(crate) fn away_root(root: &Path, chdir: bool) -> Option<String> {
    let here = std::env::current_dir().ok();
    (chdir || here.as_deref() != Some(root)).then(|| crate::text::shell_word(&root.display().to_string()))
}

/// 빠진 규칙을 알리는 **알림**(moai-2f99, 2026-09-15 사용자 결정). **파일마다 하나씩 선다** —
/// `.gitignore` 에 `/.claude/worktrees/` 가 없는 것과 `.gitattributes` 에 `merge=union` 이 없는
/// 것은 결과가 아주 다르고, 한 줄로 뭉치면 그 중 한쪽이 반드시 거짓말이 된다.
///
/// `init` 은 한 번 말하고 만다 — 못 써서 건너뛴 저장소는 `/.claude/worktrees/` 없이 얼마든지 오래
/// 가고, 그 사이 `git add -A` 한 번이 옆 워크트리를 통째로 담는다(moai-mxtb). 조용히 이어지는
/// 위험이라 세션이 시작하는 화면에 선다 — 낡은 블록을 거기 둔 것과 같은 까닭이다.
///
/// **링크인 파일은 따로 선다**([`linked_in`]) — 고칠 길이 `moai init` 이 아니다. 둘 다 링크여도
/// **알림은 하나다**(리뷰) — 파일마다 세우면 같은 머리가 두 번 서고, 갈래로 알림을 찾는 쪽은 하나를 잃는다.
///
/// **한 번 재어 둘로 가른다**(리뷰 moai-zynt.63u) — 빠진 줄과 링크를 따로 재던 판은 같은 git 물음을 두 번 띄워
/// 한도도 두 번 썼고, 한쪽만 마감을 넘기면 두 답이 갈렸다.
pub fn dotfile_notice(root: &Path, chdir: bool) -> Vec<crate::report::Warning> {
    let files = dotfiles_within(root, Some(crate::cmd::merge_driver::PROBE_BUDGET));
    let (gaps, linked) = (gaps_in(&files), linked_in(&files));
    let away = if gaps.is_empty() { None } else { away_root(root, chdir) };
    gaps.into_iter()
        .map(|(_, kind, missing)| crate::report::Warning::dotfile_rules(kind, missing, away.as_deref()))
        .chain((!linked.is_empty()).then(|| crate::report::Warning::dotfile_linked(&linked)))
        .collect()
}

/// 이 저장소의 AGENTS.md 블록이 낡았다는 알림(moai-mj45). **`status` 와 훅의 보드가 이 하나를
/// 싣는다** — 훅의 보드는 `moai status` 와 같은 말을 해야 하고(`hook::board`), 낡은 안내를 모르고
/// 시작하는 것이 그 보드를 받는 새 세션이다.
///
/// `stale` 일 때만 선다(2026-09-14 사용자 결정) — `missing` 을 말하면 `--no-agents` 로 안 쓰기로
/// 한 저장소를 영영 조른다. 못 읽는 파일도 입을 다문다: 세션의 시작점이 안내 파일 하나로 실패해
/// 보이면 안 되고, 까닭은 `moai init --check` 가 댄다.
///
/// 고칠 명령이 `-C <뿌리>` 를 대야 하는지는 [`away_root`] 가 정한다.
pub fn agents_notice(root: &Path, chdir: bool) -> Option<crate::report::Warning> {
    let Ok((BlockState::Stale, text)) = agents_state(root) else { return None };
    // **어느 쪽 낡음인지까지 말한다**(2026-09-15 사용자 결정). 한 낱말로 뭉뚱그려 `moai init` 만
    // 대면, 아직 다시 빌드 안 한 바이너리를 든 세션이 그 말을 따라 새 안내를 옛 글로 되돌린다.
    // 링크가 맞는데 낡았으면 손댄 것은 `.moai/guide.md` 다 — 손질로 알린다: `init` 이 그것을 다시 쓴다.
    let edited = guide_file_stale(&text) || stale_kind(&text) == Stale::Edited;
    Some(crate::report::Warning::agents_stale(away_root(root, chdir).as_deref(), edited))
}

/// `moai init --check`. **아무것도 안 쓰고, 파일을 못 읽을 때만 0 이 아니다**(2026-09-14 사용자
/// 결정) — 낡음으로 비영 종료하면 에이전트가 실패로 읽고, 그러면 게이트다. `.moai` 가 없어도
/// 선다: 보는 것은 AGENTS.md 하나고, 심기 전에 부르는 것도 자연스럽다.
pub fn check(ctx: &Ctx) -> R<Vec<String>> {
    let root = std::env::current_dir().map_err(|e| Fail::new(e.to_string()))?;
    let (state, text) =
        agents_state(&root).map_err(|(name, fell)| Fail::new(unread_at(ctx.lang(), &root, name, &fell)))?;
    // 빠진 딸린 파일 규칙도 같은 자리에서 본다(moai-2f99) — `--check` 는 "무엇이 낡았나" 를 묻는
    // 자리고, 블록만이 아니라 딸린 파일도 `init` 이 맞추는 것이다. 추적 방식은 **한 번 묻고** 아래 블록 없음의
    // 말에도 쓴다 — 같은 물음을 세 번 띄우던 자리다(리뷰 moai-zynt.63u). 사람이 친 명령이라 한도 없이 기다린다.
    let tracking = tracking_of(&root);
    let place = match tracking {
        Tracking::Exclude => git_place(&root, None),
        Tracking::Commit | Tracking::Gitignore => None,
    };
    let files = dotfiles(&root, tracking, place.as_ref());
    let (gaps, linked) = (gaps_in(&files), linked_in(&files));
    // **`moai init` 을 대기 전에 그것이 여기 서는지 묻는다**(moai-nppo). 딸린 워크트리에서는 안 선다
    // (moai-mz0e 가 거절을 세웠다) — 그 갈래를 모르던 판은 여기서 `moai init` 을 세 줄로 권하고,
    // 따라 친 사람은 1 로 끝나는 명령을 받았다. 가르는 자는 [`crate::store::init_belongs_at`] 하나고
    // `moai project add|ls` 와 한눈 보기가 같은 자를 쓴다.
    let tracker_at = crate::store::init_belongs_at(&root);
    // **드라이버가 어디 서 있는지도 여기서 답한다**(moai-08bo). `init` 이 그것까지 심게 된
    // 뒤로 `--check` 의 물음은 "블록이 낡았나" 하나가 아니라 "`init` 이 맞추는 것이 다 맞아
    // 있나" 다. **아무것도 안 쓴다** — 재고 말하는 것이 이 플래그의 전부다.
    //
    // 선언을 읽는 자리는 **트래커가 사는 곳**이다(moai-8nkl) — 딸린 워크트리의 가지는 그것을
    // 안 들 수 있고, 병합에서 걸리는 것은 루트의 선언이다.
    //
    // **그 자리를 대는 자는 [`crate::store::Repo::opened_root`] 다**(리뷰) — `merge_driver::notice`
    // 가 쓰는 `Repo::root` 와 **같은 자**라야 두 화면이 같은 선언을 읽는다. 위의 `tracker_at`
    // ([`crate::store::init_belongs_at`])으로 대던 판은 그 자가 "여기 `.moai` 가 있으면 `None`"
    // 이라, `.moai` 를 커밋하는 저장소(이 저장소가 그렇다)의 워크트리에서는 늘 **워크트리의**
    // 선언을 읽었다 — moai-8nkl 이 없앤 갈림이 `--check` 에만 그대로 남는다. `tracker_at` 은
    // "어디서 `init` 을 쳐야 하나" 에 답하는 자라 물음이 다르고, 아래 줄들이 그쪽을 그대로 쓴다.
    let driver = crate::cmd::merge_driver::state_at(&root, &crate::store::Repo::opened_root(&root), ctx.chdir);
    if ctx.json {
        let mut v = serde_json::json!({ "agents": state, "driver": driver });
        // **git 밖에 둔 트래커면 그것을 댄다**(리뷰 moai-zynt.63u) — 그 트래커의 `missing` 은 `init` 이 일부러 안 쓴
        // 것이라, 키가 없으면 기계는 "`init` 이 심는다" 와 못 가른다. 커밋하는 트래커는 키가 없다 — 전부터 내던 줄이 그대로다.
        if !tracking.tracked() {
            v["tracking"] = serde_json::json!(tracking.word());
        }
        // **아닐 때는 키가 없다** — 늘 달면 전부터 내던 줄이 바뀐다(`missing` 과 같은 자).
        //
        // **못 싣는 경로에서도 키만 빠진다**(리뷰). `serde_json::json!(<식>)` 은 속으로
        // `to_value(..).unwrap()` 이라, UTF-8 이 아닌 경로에 `Path` 의 직렬화가 지면 그 `unwrap` 이
        // 101 로 터진다 — 여기는 `current_dir()` 에서 자리를 받으므로 사람이 그런 이름을 쓰면 닿는다.
        // 위의 "아닐 때는 키가 없다" 가 이미 받는 쪽의 약속이니, 못 싣는 판도 같은 답으로 접는다.
        if let Some(main) = &tracker_at
            && let Ok(at) = serde_json::to_value(main)
        {
            v["tracker_at"] = at;
        }
        // 링크인 딸린 파일(moai-yke5) — `missing` 과 같은 자로, 있을 때만 키가 선다.
        if !linked.is_empty() {
            v["linked"] = serde_json::json!(linked);
        }
        if !gaps.is_empty() {
            v["missing"] = serde_json::json!(
                gaps.iter().map(|(name, _, missing)| (*name, missing)).collect::<std::collections::BTreeMap<_, _>>()
            );
        }
        return super::json_line(&v);
    }
    let lang = ctx.lang();
    let mut out = vec![match state {
        BlockState::Current => say(lang, "init.block_current").to_string(),
        // **키는 낱말째 적는다** — 소스를 훑는 시험(`i18n::tests::keys_in`)은 `say(…, "키")`
        // 모양만 읽어, 키를 변수로 넘기면 그 눈에서 통째로 사라진다.
        // 링크 블록이 맞는데 낡았으면 낡은 것은 `.moai/guide.md` 다 — 블록의 해시로 가르면 "다른 바이너리" 로 읽힌다.
        BlockState::Stale if guide_file_stale(&text) => {
            fill(say(lang, "init.guide_file_stale"), &[("file", crate::guide::GUIDE_FILE)])
        }
        BlockState::Stale => match stale_kind(&text) {
            Stale::Binary => say(lang, "init.block_stale_binary").to_string(),
            Stale::Edited => say(lang, "init.block_stale_edited").to_string(),
        },
        // git 밖에 둔 트래커에서 블록이 없는 것은 `init` 이 일부러 안 쓴 것이다 — "심는다" 고 하면 거짓말이다.
        BlockState::Missing if !tracking.tracked() => say(lang, "init.block_missing_local").to_string(),
        BlockState::Missing => say(lang, "init.block_missing").to_string(),
    }];
    // **규칙끼리는 쉼표로 가른다** — `.gitattributes` 의 규칙은 제 안에 띄어쓰기를 여럿 들어
    // (`.moai/journal.jsonl  text eol=lf merge=union`) 띄어쓰기로 이으면 어디서 한 줄이 끝나는지
    // 안 보인다.
    for (name, _, missing) in &gaps {
        out.push(fill(
            say(lang, "init.rules_missing"),
            &[("name", name), ("n", &missing.len().to_string()), ("rules", &missing.join(", "))],
        ));
    }
    for name in &linked {
        out.push(fill(say(lang, "init.check_linked"), &[("name", name)]));
    }
    // **드라이버도 한 줄로 댄다**(moai-08bo). 안 쓰기로 한 저장소(`off`)와 이미 선 줄(`current`)은
    // 조용하다 — `--check` 가 대는 것은 **남은 일**이고, 그 둘은 남은 일이 아니다.
    if !matches!(driver, "off" | "current") {
        out.push(fill(say(lang, "init.check_driver"), &[("state", driver)]));
    }
    // **어디서 쳐야 하는지를 끝에 댄다**(moai-nppo). 위의 줄들이 저마다 대는 `moai init` 은
    // 여기서 1 로 끝나므로, 그 자리를 안 대면 세 줄이 통째로 못 따를 말이 된다. 줄들을 고쳐 쓰지 않고
    // 한 줄을 더하는 까닭은 낡음을 말하는 것과 어디서 고치는가가 다른 물음이어서다 — 딸린 파일이 다
    // 맞은 워크트리에서도 이 줄은 서고, 그때 위는 `current` 다.
    if let Some(main) = &tracker_at {
        // **이 셸이 무엇을 읽고 있는지는 손잡이가 켜졌을 때만 묻는다** — 꺼진 셸에서는 둘째 줄이
        // 아예 안 서므로 물어도 쓸 데가 없고, 이 물음은 조상을 훑는다.
        let here = crate::store::here_wanted();
        let reading = here.then(|| crate::store::tracker_in_use(&root)).flatten();
        out.extend(check_worktree(lang, main, away_root(&root, ctx.chdir).as_deref(), here, reading.as_deref()));
    }
    Ok(out)
}

/// `--check` 의 끝줄 — **어디서 `init` 을 쳐야 하는가**, 그리고 **이 셸이 갈렸는가**.
///
/// **손잡이를 켠 셸에서는 줄이 둘이다**(moai-ha0f, 2026-09-21 사용자 결정). 첫 줄이 대는 자리는
/// [`crate::store::init_belongs_at`] 이 내는 것이고 그 자는 `MOAI_HERE` 를 **안 본다**(moai-ko4y) —
/// 나중의 다른 부름이 어디서 서느냐에 답하는 자라, 그 부름의 셸이 손잡이를 켤지는 여기서 알 수 없다.
/// 그 결정을 그대로 두면 `MOAI_HERE=1` 인 셸에서 이 줄이 "여기 안 선다" 를 말하는데 **같은 셸의**
/// `moai init` 은 여기 심고 0 으로 끝난다 — 한 명령의 답이 둘로 갈린다. 둘째 줄이 그 갈림을 메운다:
/// 첫 줄은 손잡이 없는 셸에 대한 답으로 그대로 맞고, 둘째 줄이 이 셸에 대한 답을 댄다.
///
/// **대는 명령에 접두어를 박는다.** 손잡이는 **이 프로세스 하나**에 서므로, 접두어 없는 줄을 베껴
/// 딴 셸에 붙여 넣으면 그 줄은 [`run`] 의 거절로 1 로 끝난다 — moai-ko4y 가 표면을 물려받게 두지
/// 않은 까닭이 그것이고, 접두어를 박은 줄은 어느 셸에서도 같은 일을 한다.
///
/// **`-C` 를 붙이는 규칙은 한 자리다**([`crate::report::Warning::cli_hint`], 리뷰 moai-h6aq.cx8) —
/// 알림들과 [`run`] 의 거절문이 같은 글자를 내야 한다. 여기 두 줄도 그 자를 거친다.
///
/// **심는 것이 이 셸이 읽던 트래커를 가리면 대는 말이 바뀐다**(리뷰 moai-uocc.45o 의 2번,
/// 2026-09-21 사용자 결정). 딸린 워크트리의 **밑자리**(`<wt>/src/deep`)가 그 자리다 — 손잡이를 켠
/// 셸은 `<wt>/.moai` 를 읽는데([`crate::store::tracker_in_use`]), 거기 대고 "여기 심는다" 를 그대로
/// 내면 따라 친 사람이 `src/deep` 에 아무도 안 읽는 `.moai` 를 세운다. 그 뒤의 `add` 는 그리로 가고
/// 먼저 쓴 줄들은 사라진 것처럼 보이며, 그 파일은 병합에서 겨룬다 — moai-pk4x 가 `init` 에서 막는
/// 바로 그것을 안내가 권하게 된다.
///
/// **주 체크아웃을 가리는 것은 이 갈래가 아니다.** 손잡이를 켠 셸에서 워크트리 꼭대기에 심는 것도
/// 위의 트래커를 가리지만, 그것이 moai-ko4y 가 열어 둔 길이고 [`run`] 의 거절문도 그 값을 함께
/// 댄다(`refuse.init_worktree_here_note`). 가르는 자는 "읽고 있는 것이 첫 줄이 대는 자리와 같은가"
/// 하나다.
fn check_worktree(
    lang: crate::i18n::Lang,
    main: &Path,
    away: Option<&str>,
    here: bool,
    reading: Option<&Path>,
) -> Vec<String> {
    let there = crate::report::Warning::cli_hint(Some(&crate::text::shell_word(&main.display().to_string())), "init");
    let mut out = vec![fill(say(lang, "init.check_worktree"), &[("go", &there)])];
    if !here {
        return out;
    }
    match reading.filter(|at| !crate::user_config::same_dir(at, main)) {
        Some(at) => {
            let at = at.join(".moai").display().to_string();
            out.push(fill(say(lang, "init.check_worktree_shadow"), &[("at", &at)]));
        }
        None => {
            let go = format!("MOAI_HERE=1 {}", crate::report::Warning::cli_hint(away, "init"));
            out.push(fill(say(lang, "init.check_worktree_here"), &[("go", &go)]));
        }
    }
    out
}

/// `moai init --print`. **아무것도 안 쓰고 블록만 찍는다.**
///
/// Claude 의 훅이 없는 에이전트는 제 도구가 읽는 파일이 `AGENTS.md` 가 아닐 수
/// 있다 — 그 파일에 붙여 넣을 글을 여기서 받는다. 계약은 이 글 하나다: 명령
/// 전부와 `--json` 의 모양이 여기 있으므로, 붙여 넣는 쪽은 도구가 자란 뒤에도
/// 같은 자리에서 새 글을 받는다.
///
/// **새 명령(`onboard`)을 안 둔 것은 `--check` 와 같은 까닭이다**(moai-mstm) —
/// 쓰는 길과 보는 길이 한 이름에 있어야, 받아 간 글이 낡았을 때 무엇을 칠지
/// 안다. 글도 `init` 이 쓰는 그것 하나라, 둘이 갈라질 자리가 없다.
///
/// `.moai` 가 없어도 선다. 심기 전에 무엇이 붙는지 보는 것이 자연스럽다.
pub fn print(ctx: &Ctx) -> R<Vec<String>> {
    let block = crate::guide::agents();
    if ctx.json {
        return super::json_line(&serde_json::json!({ "agents": block }));
    }
    Ok(block.lines().map(str::to_string).collect())
}

// **여기 글을 고치면 이미 심은 저장소가 주석을 둘 든다.** [`ensure_lines`] 는 줄 단위로 견주어
// 없는 줄을 덧붙이므로, 주석 한 줄만 고쳐도 다음 `moai init` 이 옛 주석 밑에 새 주석을 붙인다
// (리뷰 moai-vbmn.spv 에서 실제로 났다). 그래서 이 블록의 글은 규칙이 바뀔 때만 손댄다 —
// 규칙 없이 글만 고치고 싶으면 그 전에 덧붙임을 막는 길부터 낸다(idea).
//
// **한국어에서 영어로 간 것이 그 한 번이다**(moai-9vwy, 2026-09-21 사용자 결정). 심은 파일은
// 사람의 저장소에 커밋되어 남으므로, 영어로 고른 사람의 저장소에 한국어 주석이 박히는 것을
// 그대로 둘 수 없었다. **값은 사용자가 알고 받았다** — 이미 심긴 저장소는 다음 `moai init` 에
// 옛 일곱 줄 밑에 새 줄을 받고, 그것은 사람이 손으로 지운다. 이 저장소의 `.gitattributes` 는
// 같은 커밋에서 손으로 갈았다.
//
// **말묶음에 안 넣는다.** 심는 파일은 화면이 아니라 저장소의 내용이고, `moai init` 이 심는
// `AGENTS.md` 블록이 영어 하나로 선 것과 같은 자리다(moai-54k2) — 읽는 사람이 저장소를
// 함께 쓰는 남들이라 화면 말 설정을 따라가면 한 파일이 사람마다 달라진다.
const GITATTRIBUTES: &str = "\
# moai — issue tracker
# The snapshot does not use merge=union. When two branches change the same
# issue, union quietly leaves two lines carrying one id, and that is data loss.
# merge=moai pairs the lines by id and resolves each issue 3-way. A real
# conflict — two people changing the same field — still reaches a person, and
# one line per issue makes that easy in practice.
# Install the driver once per clone with `moai merge-driver --install`. In a
# clone without it this word is ignored and git's default merge runs.
.moai/issues.jsonl   text eol=lf merge=moai
# The journal is append-only, order does not matter, and it is never read to
# compute state. union is right here.
# It is filed per writer, by email — `.moai/journal/you_example_com.jsonl`.
# Several files is the normal shape, and the old single file is still read as
# one of them, so both lines stand.
.moai/journal.jsonl  text eol=lf merge=union
.moai/journal/*.jsonl  text eol=lf merge=union
";

// **워크트리 자리도 막는다**(moai-mxtb, 사용자와 정함). 감독 일꾼 절차가 `.claude/worktrees/` 에
// 워크트리를 뜨는데, 그 자리가 안 막히면 `git add -A` 에 남의 가지 전체가 딸려 온다. 넣는 것은
// 그 자리 하나다 — `.claude/` 통째는 저장소가 커밋하는 설정·스킬·플러그인을 가린다. 끄는 길은
// 두지 않는다: 워크트리를 안 쓰는 저장소에는 빈 자리를 막는 줄일 뿐이다.
const GITIGNORE: &str = "\
# moai
.moai/lock
.moai/*.tmp.*
/.claude/worktrees/
";

/// **새로 심는 접두어의 최대 길이**(moai-f7xs). id 는 `<접두어>-<4자>` 이고 사람과
/// 에이전트가 명령마다 친다 — 접두어가 길면 그만큼 매번 손이 늘고, 목록·트리·탐색기의 id
/// 열이 넓어져 제목 몫이 준다. 8자면 id 13자·자식 id 17자이고, `backend`·`frontend`
/// 같은 흔한 한 낱말이 그대로 들어간다.
///
/// **검사는 새로 심을 때만 한다.** `Config::parse` 에 두면 이미 긴 접두어로 심긴 저장소가
/// 통째로 안 열린다(읽기는 관대하게) — 접두어는 나중에 못 바꾸는 값이라 알려 봐야 고칠
/// 길도 없다.
pub const PREFIX_MAX: usize = 8;

/// 긴 접두어의 짧은 후보. **모양이 맞는 접두어만 받는다**(`config::check_prefix`) — 그래야
/// 내는 것도 모양이 맞는다. [`PREFIX_MAX`] 이하면 그대로, 넘으면 뜻을 더 남기는 것부터:
/// 1. 하이픈을 빼서 들어가면 그것 — `moa-issue` → `moaissue`. 머리글자(`mi`)는 알아보기
///    어렵고 접두어는 나중에 못 바꾼다(리뷰 moai-f7xs.z1x, 사용자와 정함)
/// 2. 낱말이 둘 이상이면 머리글자 — `my-company-backend` → `mcb`
/// 3. 낱말이 하나면 앞에서 [`PREFIX_MAX`] 자
fn shorten(prefix: &str) -> String {
    if prefix.chars().count() <= PREFIX_MAX {
        return prefix.to_string();
    }
    let words: Vec<&str> = prefix.split('-').filter(|w| !w.is_empty()).collect();
    let joined: String = words.concat();
    if words.len() >= 2 && joined.chars().count() <= PREFIX_MAX {
        joined
    } else if words.len() >= 2 {
        words.iter().filter_map(|w| w.chars().next()).take(PREFIX_MAX).collect()
    } else {
        prefix.chars().take(PREFIX_MAX).collect()
    }
}

/// 디렉터리 이름에서 접두어를 만든다. 소문자·숫자·`-` 만 남긴다.
fn prefix_from(dir: &Path) -> Option<String> {
    let name = dir.file_name()?.to_str()?.to_ascii_lowercase();
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').to_string();
    (!out.is_empty()).then_some(out)
}

/// [`ensure_lines`] 가 한 일.
#[derive(Debug, PartialEq, Eq)]
enum Added {
    /// 빠진 줄을 덧붙였다. **규칙이 들었는가를 함께 든다**(리뷰 moai-hom6.qd9 3번) — 주석만
    /// 덧붙인 판을 "병합 규칙을 넣었다" 로 부르던 자리다. 심는 주석을 영어로 바꾼 뒤(moai-9vwy)
    /// 이미 심은 저장소의 첫 `moai init` 이 바로 그 판이라 모두가 한 번씩 거짓 줄을 받았다.
    Wrote { rules: bool },
    /// 이미 다 있었다 — 파일은 안 건드렸다.
    Already,
    /// 못 읽어서 안 건드렸다. 안에 든 것은 그 까닭이다.
    Unreadable(String),
    /// 링크라 안 건드렸다(moai-yke5). git 은 2.32 부터 체크아웃 안의 링크인 딸린 파일을 안 읽으므로,
    /// 덧붙이면 가리키는 파일만 바뀌고 규칙은 하나도 안 선다. 체크아웃 밖을 가리키면 덧붙이는 것 자체가
    /// 남의 파일(`~/.bashrc`)을 고치는 일이다. **손으로 더할 줄도 안 댄다** — 그 줄을 따라 적는 곳이 바로
    /// 그 링크다. 할 일은 보통 파일로 바꾸는 것 하나다.
    ///
    /// `to` 는 **한 줄로 걸러 둔** 링크 글이다 — 받은 저장소가 커밋한 글이라 ESC 나 줄바꿈이 들 수 있다.
    /// `outside` 는 그 링크를 끝까지 푼 자리가 체크아웃 밖인가(못 풀면 밖으로 친다) — 고칠 말이 갈린다
    /// ([`run`]): 안이면 그 내용을 옮겨 담고, 밖이면 옮겨 담는 순간 남의 파일이 커밋에 실린다.
    Linked { to: String, outside: bool },
    /// 읽기는 됐는데 못 썼다(읽기 전용 파일·체크아웃).
    ///
    /// **읽기 실패와 가르는 것은 말뿐이 아니다**(리뷰 moai-humk). 사람이 할 일이 다르고
    /// (인코딩 vs 권한), `Permission denied` 는 읽기에도 쓰기에도 똑같이 뜬다 — 까닭 글만
    /// 실으면 `--json` 을 읽는 쪽이 둘을 못 가른다. 그래서 갈래를 낱말로 함께 싣는다
    /// ([`Added::trouble`]). 둘 다 건너뛰고 나머지를 심는 것은 같다(moai-0dwc).
    ///
    /// **못 넣은 줄을 함께 든다.** 읽기는 됐으니 무엇이 빠졌는지 안다 — 못 읽은 자리처럼
    /// 블록을 통째로 내면 이미 있는 줄까지 손으로 붙여 넣게 되고, 그러면 같은 줄이 둘 선다.
    Unwritable { why: String, missing: Vec<String> },
}

impl Added {
    /// 못 건드린 `(갈래, 까닭)` — 건드렸으면 `None`. 갈래는 `--json` 이 그대로 싣는 낱말이다.
    fn trouble(&self) -> Option<(&'static str, &str)> {
        match self {
            Added::Wrote { .. } | Added::Already => None,
            Added::Unreadable(why) => Some(("unreadable", why)),
            Added::Unwritable { why, .. } => Some(("unwritable", why)),
            Added::Linked { to, .. } => Some(("linked", to)),
        }
    }

    /// 사람이 손으로 더할 줄. 못 쓴 자리는 **못 넣은 줄만** 안다(읽기는 됐다). 못 읽은 자리는
    /// 파일을 못 봤으니 블록을 통째로 낸다 — 주석과 빈 줄은 빼고, 규칙 줄만.
    fn hand<'a>(&'a self, block: &'a str) -> Vec<&'a str> {
        let rule = |l: &&str| !l.trim().is_empty() && !l.trim_start().starts_with('#');
        match self {
            Added::Unwritable { missing, .. } => missing.iter().map(String::as_str).filter(rule).collect(),
            Added::Wrote { .. } | Added::Already | Added::Unreadable(_) => block.lines().filter(rule).collect(),
            Added::Linked { .. } => Vec::new(),
        }
    }
}

/// 이미 있는 파일에는 **빠진 줄만** 덧붙인다. 남의 내용을 지우지 않는다.
///
/// **못 읽는 파일은 안 건드린다**(moai-gq1c). `unwrap_or_default` 로 빈 글로 치던 때는 CP949 로
/// 적힌 남의 `.gitignore` 가 moai 줄만 남기고 통째로 사라졌다 — 덧붙이는 자리라 더 나쁘다:
/// 사람은 제 줄이 그대로 있으리라 믿는다. 없는 파일만 빈 글이다.
///
/// **멈추지는 않는다**(2026-09-15 사용자 결정). AGENTS.md 는 도구가 쓴 블록을 통째로 갈아 끼우는
/// 자리라 멈추지만, 여기는 줄 몇 개를 덧붙이는 자리다 — 그 하나로 `.moai` 도 못 심고 AGENTS 블록도
/// 못 고치면 고칠 길이 도구 밖에만 남는다. 무엇을 손으로 더할지는 부르는 쪽([`run`])이 댄다.
///
/// **쓰기 실패도 같다**(moai-0dwc). 한때 읽기만 넘어가고 쓰기는 `Err` 로 끊었는데, 그 끊김은
/// `.moai/` 를 만든 **뒤에** 와서 `.gitattributes` 도 AGENTS 블록도 없는 반쯤 심긴 저장소를
/// 남겼다 — 읽기 전용 파일 하나가 `init` 을 통째로 막는 셈이기도 했다. **`Err` 를 안 낸다**:
/// 여기서 실패해도 심는 일은 이어져야 한다.
///
/// **덧붙이는 자리라 덧붙여 쓴다**(리뷰 moai-humk). 읽은 글에 빠진 줄을 이어 파일을 통째로
/// 다시 쓰던 때는 `fs::write` 가 **열면서 먼저 비웠다** — 디스크가 차거나(ENOSPC) 쓰다 죽으면
/// 남의 `.gitignore` 가 반쯤 잘린 채 남고, 위의 결정 때문에 그 실패가 0 으로 끝나 아무도 모른다.
/// `O_APPEND` 는 못 쓰면 한 글자도 안 바뀌고, 쓰다 끊겨도 남의 줄은 그대로다 — 이 함수가 내건
/// "남의 내용을 지우지 않는다" 를 실제로 지키는 것은 이쪽이다.
///
/// **링크인 파일에는 아예 안 쓴다**(moai-yke5). 받은 저장소가 커밋한 `.gitignore -> ~/.bashrc` 에 `init` 이
/// 줄을 덧붙이던 자리를 moai-wd44 가 "체크아웃 안에서만 따라간다" 로 좁혔는데, 딸린 파일은 git 이 읽어야
/// 뜻이 선다 — git 은 2.32 부터 링크인 딸린 파일을 안 읽어, 안을 가리키는 링크에 덧붙여도 규칙이 하나도
/// 안 선다. 앞 판은 그 자리를 "썼다" 고 했고 `--check` 도 빠진 것이 없다고 했다. 그래서 링크면 읽지도
/// 쓰지도 않고 [`Added::Linked`] 로 돌아간다. 아래 `append_inside` 에 넘기는 뿌리(이 파일이 든 자리,
/// [`plant`] 와 같은 자)는 이제 재고 난 뒤 링크로 바뀐 틈만 막는다.
///
/// **링크인지는 따라가지 않고 잰다**(`Path::is_symlink`) — `read_link` 가 되는지로 가르면 그것이 실패한
/// 링크(I/O 오류, 윈도의 다른 reparse point)가 아래로 떨어져 링크를 따라 덧붙인다(리뷰).
///
/// **읽는 자는 스냅샷과 같다**([`crate::held::read_inside`], moai-x0o7) — 맨 `fs::read_to_string` 은
/// `mkfifo .gitignore` 앞에서 쓰는 쪽을 영영 기다렸다. 보통 파일이 아니면 못 읽은 것으로 넘어간다. 그
/// 까닭은 고른 말로 싣는데, **말은 그때만 묻는다**(`lang`) — `--json` 의 흔한 길은 사용자 설정을 안 연다.
fn ensure_lines(path: &Path, block: &str, lang: impl FnOnce() -> crate::i18n::Lang) -> Added {
    if path.is_symlink() {
        // **링크 글은 한 줄로 걸러 싣는다**(리뷰) — `init` 의 한 줄로 그대로 터미널에 나가는데, 받은 저장소가
        // 커밋한 글이라 ESC 는 화면을 다시 칠하고 줄바꿈은 없는 줄을 지어낸다. `store::target_of` 가 거절문에
        // 싣는 링크 끝을 거르는 것과 같은 자다.
        let to = std::fs::read_link(path).map(|t| crate::text::one_line(&t.display().to_string())).unwrap_or_default();
        // **밖인지는 링크 글이 아니라 끝까지 푼 자리로 잰다**(리뷰) — 가운데 디렉터리가 링크면 안처럼 보이는
        // 글(`config/shared/.bashrc`)도 밖에 닿는다. 못 풀면(고리, 없는 디렉터리) 밖으로 친다: 그쪽 말은
        // "옮겨 담지 말고 링크를 걷어라" 라 틀려도 잃는 것이 없다.
        let checkout = crate::path::real(crate::path::dir_of(path));
        let outside = !crate::path::follow_links(path)
            .ok()
            .and_then(|end| std::fs::canonicalize(crate::path::dir_of(&end)).ok())
            .is_some_and(|dir| dir.starts_with(&checkout));
        return Added::Linked { to, outside };
    }
    let existing = match read_held(path, &crate::held::Home::of(crate::path::dir_of(path))) {
        Ok(t) => t.unwrap_or_default(),
        Err(fell) => return Added::Unreadable(unread(lang(), &fell)),
    };
    let missing = missing_lines(&existing, block);
    if missing.is_empty() {
        return Added::Already;
    }
    // 끝에 `\n` 이 없는 파일의 꼬리는 `append_inside` 가 채운다(moai-a65c) — 여기서도 채우면 빈 줄이
    // 하나 더 선다. `gap` 은 앞 규칙과 이 블록을 가르는 빈 줄이다.
    let gap = if existing.is_empty() { "" } else { "\n" };
    let tail = format!("{gap}{}\n", missing.join("\n"));
    match crate::store::append_inside(path, tail.as_bytes(), crate::path::dir_of(path)) {
        // 규칙은 [`missing_rules`] 와 **같은 자**로 가른다 — 주석이 아닌 줄이다.
        Ok(()) => Added::Wrote { rules: missing.iter().any(|l| !l.trim_start().starts_with('#')) },
        Err(e) => Added::Unwritable { why: e.message, missing: missing.iter().map(|l| (*l).to_string()).collect() },
    }
}

/// 이 파일에 아직 없는 `block` 의 줄들 — **주석도 든다.** 빈 줄만 뺀다.
///
/// **쓰는 자리가 주석을 빼면 안 된다**: `.gitattributes` 의 주석은 왜 `issues.jsonl` 에
/// `merge=union` 을 걸면 안 되는지 적은 유일한 자리다. 빼면 새 저장소가 그 까닭 없이 서고, 다음
/// 사람이 union 을 다시 건다.
fn missing_lines<'a>(existing: &str, block: &'a str) -> Vec<&'a str> {
    block.lines().filter(|l| !l.trim().is_empty() && !existing.lines().any(|e| covers(e, l))).collect()
}

/// 그 중 **규칙 줄만** — 주석은 규칙이 아니라 빠졌다고 세지 않는다.
///
/// **쓰는 길([`ensure_lines`])과 비추는 길([`gaps_in`])이 한 자에서 갈린다.** 따로 재면
/// `status` 가 빠졌다고 하는 줄을 `init` 이 이미 있다고 보거나 그 반대가 되고, 그러면 알림이 영영
/// 안 걷히거나 헛 알림이 선다 — 여기서는 `missing_rules ⊆ missing_lines` 라 그럴 수 없다: 규칙이
/// 빠졌으면 `init` 이 반드시 그것을 쓰고, `init` 이 할 일이 없으면 알림도 서지 않는다.
fn missing_rules<'a>(existing: &str, block: &'a str) -> Vec<&'a str> {
    missing_lines(existing, block).into_iter().filter(|l| !l.trim_start().starts_with('#')).collect()
}

/// AGENTS.md 를 갈아 끼운다. **못 쓰면 한 글자도 안 바뀐다.**
///
/// 먼저 열어 본다 — `rename` 은 파일의 권한을 안 보므로 그것만으로는 읽기 전용 AGENTS.md 를
/// 소리 없이 갈아 끼운다. 열리면 temp+rename 이다: `fs::write` 는 **열면서 먼저 비워**, 디스크가
/// 차거나(ENOSPC) 쓰다 죽으면 사람의 산문이 반쯤 잘린 채 남는다. 그 실패가 이제 `Err` 가 아니라
/// 한 줄 말로 끝나므로(moai-780n) 더 나쁘다 — **"못 썼다" 가 참이려면 못 쓴 자리에 옛 글이 그대로
/// 있어야 한다.** [`ensure_lines`] 가 `O_APPEND` 로 지키는 것과 같은 약속이고, 산문을 한 글자도
/// 안 건드린다는 [`with_block`] 의 약속을 실제로 지키는 것도 이쪽이다.
fn plant(path: &Path, text: &str) -> Result<(), String> {
    if path.exists() {
        std::fs::OpenOptions::new().write(true).open(path).map_err(|e| e.to_string())?;
    }
    let first = tmp_dir(path);
    // 링크는 저장소 안을 가리킬 때만 따라간다(moai-4oab) — 받은 저장소가 커밋한 `AGENTS.md` 링크가
    // 체크아웃 밖을 가리키면 `init` 이 그 파일을 고쳐 쓴다. 뿌리는 이 파일이 든 자리다.
    let checkout = crate::path::dir_of(path);
    match crate::store::write_atomic_in(path, text.as_bytes(), &first, checkout) {
        // `.moai/` 에서 못 갈아 끼웠으면 옆자리로 한 번 더 — 까닭은 [`tmp_dir`] 에 적었다. 실패한
        // 쪽은 임시 파일을 치우고 대상을 안 건드리므로 다시 써도 잃을 것이 없다.
        Err(_) if path.parent() != Some(first.as_path()) => {
            crate::store::write_atomic_inside(path, text.as_bytes(), checkout).map_err(|e| e.message)
        }
        done => done.map_err(|e| e.message),
    }
}

/// 뿌리 파일을 갈아 끼울 임시 파일의 자리 — **`.moai/`** 다(moai-3akx, 2026-09-18 사용자 결정).
///
/// 옆자리에 두면 쓰다 죽은 `init` 이 `AGENTS.md.tmp.…` 를 저장소 뿌리에 남기고, 심는
/// `.gitignore` 블록은 `.moai/*.tmp.*` 만 덮는다. 규칙을 더하는 길은 버렸다 — 이미 심긴
/// 저장소마다 "규칙이 빠졌다" 알림이 새로 선다. `.moai/` 는 `init` 이 이 쓰기보다 먼저 세운다.
///
/// **거기서 못 쓰면 [`plant`] 가 옆자리로 물러선다 — 미리 재지 않고 써 보고 물러선다.** 다른
/// 파일시스템이면 `rename` 이 `EXDEV` 로, 읽기 전용 `.moai/` 면 임시 파일 만들기가 막힌다. 장치
/// 번호로 미리 재던 때는 뒤의 것을 못 봐 쓸 수 있는 `AGENTS.md` 를 "못 썼다" 고 하며 그 파일을
/// 고치라고 했고, unix 밖에서는 재지도 못했다. 블록을 못 쓰는 것보다 찌꺼기가 남을 수 있는 쪽이
/// 낫다. `.moai` 가 디렉터리가 아니면 처음부터 옆자리다.
fn tmp_dir(path: &Path) -> std::path::PathBuf {
    let beside = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let moai = beside.join(".moai");
    if moai.is_dir() { moai } else { beside }
}

/// 이미 있는 줄 `have` 가 넣으려는 줄 `want` 를 **이미 막고 있는가**(moai-mxtb).
///
/// 글자가 같은 줄만 보면, `/.claude/worktrees` 를 이미 적은 저장소에 효과 없는
/// `/.claude/worktrees/` 가 하나 더 붙는다. 그래서 앞뒤 `/` 를 떼고 견주고, 윗 디렉터리를
/// 통째로 막은 줄(`.claude/`)도 그 밑의 줄을 막은 것으로 친다. **남의 줄은 안 바꾼다** —
/// 판정만 넓히고 쓰는 것은 빠진 줄을 덧붙이는 것뿐이다. 주석·빈 줄·`!` 되살림은 넓혀
/// 읽지 않는다.
fn covers(have: &str, want: &str) -> bool {
    let (have, want) = (have.trim(), want.trim());
    if have == want {
        return true;
    }
    if have.is_empty() || have.starts_with('#') || have.starts_with('!') || want.starts_with('#') {
        return false;
    }
    if decided(have, want) {
        return true;
    }
    // 끝 `/` 는 "디렉터리만" 이라는 뜻이다. 같은 이름끼리 견줄 때 `have` 만 디렉터리 전용이면
    // (`.moai/lock/`) 파일 `.moai/lock` 을 막지 못하니 덮은 것으로 치지 않는다.
    let dir_only = have.ends_with('/') && !want.ends_with('/');
    let bare = |s: &str| s.trim_start_matches('/').trim_end_matches('/').to_string();
    let (have, want) = (bare(have), bare(want));
    !have.is_empty() && ((have == want && !dir_only) || want.starts_with(&format!("{have}/")))
}

/// 같은 경로에 대해 사람이 **머지 속성을 스스로 정한** 줄인가(moai-47bt, 2026-09-21 사용자 결정).
///
/// 머지 드라이버를 안 쓰기로 한 저장소에 탈출구가 없던 자리다. 선언(`merge=moai`)을 지우면
/// `merge_driver_absent` 는 걷히지만 이번엔 "규칙이 빠졌다"(`gitattributes_rules`)가 `moai init`
/// 힌트와 함께 서고, 그 `init` 이 지운 줄을 다시 덧붙여 처음으로 돌아왔다. 어느 쪽을 따라도
/// 영영 졸리는 고리다.
///
/// **끊는 자는 git 의 어휘다.** 설정에 끄는 키를 두면 어휘가 하나 늘고 그 키는 다른 알림에도
/// 번진다. 대신 `.gitattributes` 에 사람이 같은 경로로 적어 둔 줄이 `merge` 를 **어떤 꼴로든**
/// 정하고 있으면(`-merge`·`!merge`·`merge=<다른 것>`·`merge=moai`) 그것을 결정으로 읽는다 —
/// 지우는 것이 아니라 **적는 것**이 탈출구라, 다음 사람이 그 줄에서 까닭을 읽는다.
///
/// **그 경로의 줄은 통째로 사람의 것이 된다** — `text eol=lf` 까지 그쪽이 정한다. 반만 물려받아
/// 뒤에 우리 줄을 덧붙이면 git 은 뒤엣것을 쓰므로 결정이 조용히 뒤집힌다.
///
/// 줄이 통째로 **없어진** 판은 그대로 빠진 것이다(`init` 이 다시 쓴다). 실수로 지운 것과 일부러
/// 정한 것을 가르는 자리가 여기고, 가르지 않으면 `.gitattributes` 를 잃은 저장소가 영영 조용해진다.
///
/// **스냅샷의 `merge=union` 은 결정으로 안 읽는다**(리뷰 moai-t6z9.bd6 4번). 그 한 값은 사람이
/// 고를 수 있는 것이 아니라 이 저장소가 **없애려고 다시 만들어진** 바로 그 실패다 — union 은 같은
/// id 를 가진 줄 둘을 조용히 남기고, 그것이 CLAUDE.md 가 이름 붙인 데이터 손상이다. 결정으로
/// 읽으면 `init` 이 규칙을 안 쓰고 [`crate::cmd::merge_driver::declared`] 도 `moai` 가 아니라며
/// 네 알림을 통째로 재우는데, 그 저장소는 그때부터 스냅샷을 union 으로 합친다. 탈출구는 그대로
/// 선다: `-merge`·`!merge`·`merge=<union 아닌 것>` 은 여전히 결정이다.
///
/// **저널은 반대다.** 그 줄에 걸리는 값이 union 이고(`GITATTRIBUTES`), 거기서는 union 이 맞다 —
/// 가르는 자는 낱말이 아니라 **그 경로에 무엇이 걸려야 하는가**다.
fn decided(have: &str, want: &str) -> bool {
    /// 이 줄이 `name` 속성을 정하고 있는가 — `name`·`-name`·`!name`·`name=<값>` 넷이다.
    fn sets(line: &str, name: &str) -> bool {
        line.split_whitespace().skip(1).any(|t| {
            let t = t.trim_start_matches(['-', '!']);
            t == name || t.strip_prefix(name).is_some_and(|rest| rest.starts_with('='))
        })
    }
    // 앞 `/` 는 뿌리에 못박는 표시일 뿐 가리키는 파일은 같다 — 위의 [`covers`] 와 같은 자다.
    fn path(l: &str) -> Option<&str> {
        l.split_whitespace().next().map(|p| p.trim_start_matches('/'))
    }
    // 스냅샷에 union 을 건 줄 — 결정이 아니라 고쳐야 할 줄이다. **스냅샷은 경로가 아니라 거는 값으로
    // 가른다** — 드라이버를 걸어야 하는 자리가 곧 스냅샷이다. 링크인 트래커가 가리키는 파일
    // (`/shared/issues.jsonl`, moai-7myd)도 그 자리라, `.moai/issues.jsonl` 로만 가르던 판은 거기 걸린
    // `merge=union` 을 결정으로 읽어 줄도 안 쓰고 알림도 재웠다 — 그 파일이 union 으로 합쳐졌다(리뷰).
    let moai = format!("merge={}", crate::cmd::merge_driver::DRIVER);
    let drives = |l: &str| l.split_whitespace().skip(1).any(|t| t == moai);
    let union_on_snapshot = |l: &str| drives(want) && l.split_whitespace().skip(1).any(|t| t == "merge=union");
    // **스냅샷에 적은 결정은 그 스냅샷이 가리키는 파일에도 선다**(리뷰). 링크인 트래커가 가리키는 파일의
    // 줄([`attributes_for`])은 스냅샷의 속성을 옮기려고 서는 것이라, 스냅샷에 `-merge` 로 드라이버를 안
    // 쓰기로 한 저장소에 그 줄을 요구하면 알림이 영영 서고 그 `init` 이 드라이버를 도로 건다 — 위의
    // 탈출구가 링크 하나로 닫혔다. 드라이버를 거는 줄은 결정이 아니라 우리 줄이다.
    let decided_on_snapshot =
        || drives(want) && path(have) == Some(crate::cmd::merge_driver::SNAPSHOT) && !drives(have);
    // **옮겨 적은 줄도 그 원래 줄에 적은 결정을 받는다**(리뷰) — 바로 위 스냅샷과 같은 까닭이다. `.moai` 가
    // 링크면 저널 줄이 그 자리로 옮겨 서는데([`mirrored`]), `.moai/journal…` 에 `-merge` 로 union 을 안 쓰기로
    // 한 저장소에 옮긴 줄을 요구하면 알림이 영영 서고 그 `init` 이 union 을 도로 건다. 원래 줄은 **이 블록이
    // 거는 `.moai/` 규칙 줄**뿐이다 — 사람이 적은 넓은 패턴(`.moai/*.jsonl`)은 git 에서도 저널에 안 걸린다.
    // 결정은 거는 값이 우리 줄과 다를 때다: 우리 줄을 그대로 적은 저장소는 옮긴 줄도 여전히 요구받는다.
    let decided_on_mirror = || {
        let (Some(from), Some(to)) = (path(have), path(want)) else { return false };
        let ours = GITATTRIBUTES.lines().filter(|l| !l.starts_with('#')).filter_map(path).any(|p| p == from);
        ours && from.strip_prefix(".moai/").is_some_and(|rest| to.ends_with(&format!("/{rest}")))
            && merges(have) != merges(want)
    };
    // 속성이 없는 줄은 `.gitignore` 의 줄이다 — 그쪽은 위의 자로만 잰다.
    (path(have) == path(want) || decided_on_snapshot() || decided_on_mirror())
        && sets(want, "merge")
        && sets(have, "merge")
        && !union_on_snapshot(have)
}

/// 이 `.gitattributes` 줄이 `merge` 를 정하는 낱말들 — `merge`·`-merge`·`!merge`·`merge=<값>`. 사람의 줄이
/// 우리 줄과 같은 값을 거는지 견줄 때 쓴다([`decided`]).
fn merges(line: &str) -> Vec<&str> {
    line.split_whitespace()
        .skip(1)
        .filter(|t| {
            let t = t.trim_start_matches(['-', '!']);
            t == "merge" || t.starts_with("merge=")
        })
        .collect()
}

/// 새로 심는 접두어를 잰다 — 플래그로 준 것도 선택 상자에 친 것도 이 하나로 잰다(moai-zynt.a7y).
///
/// **모양을 먼저 본다** — 길이를 먼저 보면 `MyCompanyBackend` 에 그 자체로 틀린 `MyCompan` 을 후보로
/// 대, 따라 친 쪽이 둘째 오류를 만났다(리뷰 moai-f7xs.z1x). 후보는 명령줄이 아니라 접두어만 댄다 —
/// `-C`·`--no-agents` 를 줬던 명령을 다시 짜서 대면 붙여 넣은 자리에 엉뚱하게 심는다.
fn fresh_prefix(lang: crate::i18n::Lang, p: &str) -> Result<(), Fail> {
    // **빈 접두어는 모양 검사를 지난다** — 걸릴 글자가 없다. 그대로 두면 선택 상자를 닫은 뒤에야 설정 검사가
    // "`prefix` 가 없다" 로 멈춰 화면에서 고른 것이 통째로 사라졌다(리뷰 moai-zynt.63u). 플래그(`moai init ''`)도 같은 말이다.
    if p.is_empty() {
        return Err(Fail::coded(say(lang, "refuse.init_empty_prefix"), super::code::BAD_INPUT));
    }
    crate::config::check_prefix(p).map_err(|e| Fail::new(crate::view::config_trouble(lang, &e)))?;
    if p.chars().count() > PREFIX_MAX {
        return Err(Fail::coded(
            format!(
                "{}\n      {}",
                fill(
                    say(lang, "refuse.init_prefix_too_long"),
                    &[("max", &PREFIX_MAX.to_string()), ("p", p), ("n", &p.chars().count().to_string())],
                ),
                fill(say(lang, "refuse.init_prefix_short"), &[("short", &shorten(p))]),
            ),
            super::code::BAD_INPUT,
        ));
    }
    Ok(())
}

/// 여러 줄 거절문을 선택 상자의 한 줄로 편다 — 이음 들여쓰기는 `moai: ` 머리에 맞춘 값이라 거기서는 뜻이 없다.
/// **한 줄 자리의 자([`crate::text::one_line`])를 지난다**(리뷰 moai-zynt.63u) — 이어 부른 명령이 진 말은 경로와
/// 사용자 설정의 글을 실어 오는데, 제어문자를 안 걷던 판은 그것을 터미널에 그대로 냈다.
fn one_line(message: &str) -> String {
    crate::text::one_line(&message.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" — "))
}

pub fn run(ctx: &Ctx, flags: &Flags, yes: bool) -> R<Vec<String>> {
    let root = std::env::current_dir().map_err(|e| Fail::new(e.to_string()))?;
    let dir = root.join(".moai");
    // **세우기 전에 한 번 묻는다**(moai-pjrr·moai-mz0e). 이미 여기 심겨 있으면 안 묻는다 — 그때 이
    // 명령이 하는 일은 딸린 파일을 다시 맞추는 것뿐이라 새 트래커가 서지 않는다.
    let elsewhere = if dir.exists() { None } else { crate::store::planted_elsewhere(&root) };
    // **거절하는 자리는 워크트리 하나다.** 위에서 찾은 것은 세우고 아래에서 알린다 — 가른 까닭은
    // [`crate::store::planted_elsewhere`] 에 있다.
    if let Some(Elsewhere::Worktree(main)) = &elsewhere {
        let there = main.clone();
        let lang = ctx.lang();
        // **여러 줄짜리 거절문의 줄바꿈과 이음 들여쓰기는 이 자리가 쥔다** — 말묶음의 값은 한 줄이라는
        // 계약이 있어(`i18n::tests::every_translation_keeps_the_places_english_marks` 가 제어 글자를
        // 막는다) `\n` 을 거기 담을 길이 아예 없고, 이음 여섯 칸은 `moai: ` 머리에 맞춘 값이라 옮기는
        // 사람이 셀 것이 아니다. **보고 줄의 두·네 칸은 반대로 말묶음이 쥔다**(`init.columns` 처럼) —
        // 그쪽은 stderr 머리와 무관한 이 화면만의 층이고, 저장소의 다른 `warn.*`·`tui.*` 키 서른한 개가
        // 이미 그 꼴이다. 두 규칙을 섞어 읽지 않는다.
        let why = format!("{}\n      {}", say(lang, "refuse.init_worktree"), there.join(".moai").display());
        // **친 대로 도로 낸다**(리뷰). 접두어를 빠뜨린 줄을 그대로 베끼면 디렉터리 이름에서 만든
        // 접두어가 서는데, 그것은 아래 갈래가 말하듯 **나중에 못 바꾼다** — 따라 친 한 줄이 그 저장소의
        // 모든 id 에 남는다. `--no-agents` 도 일부러 준 것이라 빼면 안 준 사람의 `AGENTS.md` 를 고친다.
        //
        // **`-C` 도 되살린다**(리뷰 둘째 판) — `-C` 는 [`crate::main`] 이 `set_current_dir` 로 따르므로
        // 여기의 "여기" 는 **`-C` 가 가리킨 자리**고 사람의 셸은 딴 데 있다. 빠뜨린 줄을 그대로 베끼면
        // 그 셸 자리에 트래커가 하나 더 선다 — 나머지를 친 대로 되살린 줄일수록 더 그대로 베낀다.
        // 재는 자는 [`away_root`] 하나다: 알림마다 따로 재면 한 화면의 두 줄이 다른 자리를 댄다.
        let at = away_root(&root, ctx.chdir).map(|r| format!(" -C {r}")).unwrap_or_default();
        // **깃발마다 짝을 적지 않는다**(리뷰). 경우를 손으로 다 적던 판은 `--no-driver` 가 늘 때
        // 그것만 빠졌고, 그 줄을 따라 친 사람은 안 심기로 한 드라이버를 심었다 — `--local` 은
        // 클론이 함께 쓰는 자리라 그 한 번이 **모든 체크아웃**에 앉는다. 깃발이 하나 늘면 줄도
        // 하나만 는다.
        //
        // 값을 받는 깃발은 정한 낱말로 되살린다 — `--no-agents` 는 `--guide none` 으로 섰다(`Flags`).
        let mut same = String::new();
        if let Some(p) = flags.prefix {
            same.push(' ');
            same.push_str(&crate::text::quoted(p));
        }
        let worded = [("--tracking", flags.tracking.map(Tracking::word)), ("--guide", flags.guide.map(Guide::word))];
        for (flag, word) in worded {
            if let Some(word) = word {
                same.push_str(&format!(" {flag} {word}"));
            }
        }
        let paired = [
            (flags.driver == Some(true), " --driver"),
            (flags.driver == Some(false), " --no-driver"),
            (flags.skill == Some(true), " --skill"),
            (flags.skill == Some(false), " --no-skill"),
            (flags.register == Some(true), " --register"),
            (flags.register == Some(false), " --no-register"),
        ];
        for (on, flag) in [(yes, " --yes")].into_iter().chain(paired) {
            if on {
                same.push_str(flag);
            }
        }
        // **빠져나가는 길의 값도 함께 댄다**(리뷰). `MOAI_HERE` 는 **이 프로세스 하나**에만 선다 —
        // 그것으로 세운 트래커는 그 뒤의 맨 `moai` 가 도로 루트의 것을 읽어 아무도 안 읽는다.
        // 대지 않으면 이 줄이 거절문이 막으려던 바로 그 자리로 사람을 데려간다.
        // **경로는 감싸서 낸다**(`crate::text::shell_word`, moai-0cl3) — 붙여 넣으면 도는
        // 글자여야 한다. 빈칸 하나가 `-C` 를 딴 자리로 보낸다. **`-C` 를 붙이는 규칙 자체는
        // `report::Warning::cli_hint` 하나다**(리뷰 moai-h6aq.cx8) — 알림과 `--check` 와 이 줄이
        // 같은 글자를 내야 한다. 한눈 보기(`view::unopened`·`cmd::project`)는 아직 제 손으로 짓는다.
        let there_go =
            crate::report::Warning::cli_hint(Some(&crate::text::shell_word(&there.display().to_string())), "init");
        let here_go = format!("MOAI_HERE=1 moai{at} init{same}");
        return Err(Fail::coded(
            format!(
                "{why}\n      {}\n      {}\n        {}",
                fill(say(lang, "refuse.init_worktree_there"), &[("go", &there_go)]),
                fill(say(lang, "refuse.init_worktree_here"), &[("go", &here_go)]),
                say(lang, "refuse.init_worktree_here_note"),
            ),
            super::code::ALREADY_EXISTS,
        ));
    }

    // 이미 심긴 곳에서 다시 부르면 **딸린 파일만 다시 맞춘다.**
    //
    // 도구가 자라면 `AGENTS.md` 블록은 반드시 낡는다. 그걸 다시 쓸 길이
    // 없으면 새 세션의 에이전트가 없는 명령을 쓰고 있는 명령을 모른다.
    // 명령을 하나 더 만드는 대신 `init` 이 그 일을 맡는다 — 이슈와 저널은
    // 손대지 않으므로 다시 불러도 잃을 것이 없다.
    let again = dir.exists();

    // **처음 심을 때 사람에게 묻는다**(moai-zynt.a7y). 플래그와 화면은 저마다 고른 것을 내고 같은
    // `resolve` 를 지난다 — 아래는 그 계획만 읽는다. 묻는 것은 사람이 보는 터미널에서뿐이다: 에이전트·
    // 스크립트·`--json` 은 지금까지와 같은 파일을 심는다(`init_choice::PLAIN`) — `--json` 의 줄만 고른 것을 대는
    // 키(`tracking`·`guide`·`guide_file`·`skill`·`project`)가 늘었다. 다시 부른
    // `init` 은 안 묻는다 — 접두어는 이미 못 바꾸고, 하는 일은 딸린 파일을 맞추는 것뿐이다. 워크트리
    // 거절은 위에서 이미 섰다 — 다 물어 놓고 거절하지 않는다.
    let mut fixed = Choice::from_flags(flags);
    // **다시 부른 `init` 의 추적은 git 에 묻는다**(moai-zynt.own) — 어디에도 적어 두지 않는다. 적어 두면
    // 사람이 무시 줄을 지운 날 적힌 값과 git 이 갈려, `init` 이 커밋되는 트래커에 드라이버를 안 심는다.
    // 바꾸는 길은 `init` 이 아니다: 커밋된 것을 빼거나(`git rm --cached`) 넣는 일은 사람의 결정이다.
    if again {
        let now = tracking_of(&root);
        if let Some(asked) = fixed.tracking.filter(|t| *t != now) {
            return Err(Fail::coded(
                fill(say(ctx.lang(), "refuse.init_tracking_fixed"), &[("now", now.word()), ("asked", asked.word())]),
                super::code::ALREADY_EXISTS,
            ));
        }
        fixed.tracking = Some(now);
        // 안내도 파일에서 읽는다 — 블록이 `.moai/guide.md` 를 가리키면 그 모드다. 그 밖에는 지금까지처럼
        // 블록을 맞추되, git 밖에 둔 트래커는 커밋되는 AGENTS.md 를 안 건드린다. 플래그로 주면 바꿔 심는다:
        // 블록과 링크는 둘 다 `init` 이 쥔 글이라 갈아 끼워도 잃을 것이 없다.
        if fixed.guide.is_none() {
            fixed.guide = Some(guide_of(&root, now));
        }
    }
    // **git 의 자리는 한 번 묻는다** — git 밖에 둘 수 있는가와 `.git/info/exclude` 의 자리(하위 디렉터리면 그 길까지)가
    // 이 하나에서 온다([`dotfiles`]).
    let place = git_place(&root, None);
    let plan = if !again
        && !yes
        && !ctx.json
        && !fixed.complete(&crate::init_choice::SCREEN)
        && crate::tui::init_screen::on_terminal()
    {
        let lang = ctx.lang();
        let suggested = prefix_from(&root).map(|full| shorten(&full)).unwrap_or_default();
        // git 저장소가 아니면 이 클론에만 둘 자리가 없고, 딸린 워크트리면 막는 줄이 주 체크아웃까지 가린다
        // ([`local_refusal`]) — 기본을 커밋 쪽으로 돌린다. 칸은 남겨 둔다: 고르면 심을 때 까닭을 댄다.
        let mut defaults = match &place {
            Some(p) if !p.linked => crate::init_choice::SCREEN,
            _ => crate::init_choice::Defaults { tracking: Tracking::Commit, ..crate::init_choice::SCREEN },
        };
        // **`--no-skill` 이면 git 밖의 안내 기본을 훅에서 거둔다**(리뷰 moai-zynt.63u) — 훅 안내는 설치가 있어야
        // 서서, 그대로 두면 화면이 처음부터 부딪히는 값을 골라 놓고 Enter 에서 거절한다. 사람이 훅을 고르면 그때
        // 같은 까닭으로 머문다.
        if fixed.skill == Some(false) {
            defaults.guide_local = Guide::None;
        }
        refuse_fixed(lang, &fixed, &defaults, place.as_ref())?;
        let fixed_flags = fixed.clone();
        let form = crate::init_choice::Form::new(fixed, defaults, suggested.clone());
        // 심으려는 접두어를 플래그로 준 것과 같은 잣대로 잰다. 거절문은 여러 줄이라 한 줄로 편다.
        let check = |plan: &Plan| {
            let prefix = match &plan.prefix {
                Some(p) => fresh_prefix(lang, p).err().map(|f| one_line(&f.message)),
                None if suggested.is_empty() => Some(say(lang, "refuse.init_no_prefix").to_string()),
                None => None,
            };
            let clash = crate::init_choice::conflict(&fixed_flags, plan).map(|c| clash_said(lang, c));
            prefix.or_else(|| local_refusal(lang, plan.tracking, place.as_ref())).or(clash)
        };
        // 머리에는 디렉터리 이름만 댄다 — 온 경로는 한 줄을 넘겨 잘리고, 셸이 이미 그 자리에 서 있다.
        let name =
            root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| root.display().to_string());
        match crate::tui::init_screen::ask(lang, &name, form, &check) {
            Ok(Some(plan)) => plan,
            Ok(None) => return Err(Fail::new(say(lang, "init.stopped"))),
            Err(why) => return Err(Fail::new(fill(say(lang, "init.no_screen"), &[("why", &why)]))),
        }
    } else {
        let plan = crate::init_choice::resolve(&fixed, &crate::init_choice::PLAIN);
        if let Some(why) = local_refusal(ctx.lang(), plan.tracking, place.as_ref()).filter(|_| !again) {
            return Err(Fail::coded(why, super::code::BAD_INPUT));
        }
        if let Some(c) = crate::init_choice::conflict(&fixed, &plan) {
            return Err(Fail::coded(clash_said(ctx.lang(), c), super::code::BAD_INPUT));
        }
        plan
    };
    // 훅으로 알리는 것도 AGENTS.md 를 안 건드린다 — 알리는 일은 훅(`hook::guided_board`)이 한다.
    let no_agents = matches!(plan.guide, Guide::None | Guide::Hook);
    let (prefix, no_driver) = (plan.prefix.as_deref(), !plan.driver);
    // 디렉터리 이름이 길어 줄였으면 그 원래 모양 — 무엇에서 줄였는지 말하려고 든다.
    let mut shortened: Option<String> = None;
    let prefix = match (prefix, again) {
        // 접두어는 나중에 못 바꾼다. 이미 발급된 id 가 전부 그것을 달고 있고,
        // 바꾸면 그 줄들이 제 접두어를 잃는다.
        (Some(p), true) => {
            let cur = crate::config::Config::load(&root).map_err(|e| Fail::config(&e, ctx.lang()))?.prefix;
            if p != cur {
                return Err(Fail::coded(
                    format!(
                        "{}\n      {}",
                        fill(say(ctx.lang(), "refuse.init_prefix_fixed"), &[("cur", &cur)]),
                        say(ctx.lang(), "refuse.init_prefix_fixed_why"),
                    ),
                    super::code::ALREADY_EXISTS,
                ));
            }
            cur
        }
        // **사람이 준 긴 접두어는 거절한다** — 쓰기는 엄하게. `init` 은 한 번 부르는 명령이라
        // 다시 부르는 비용이 작고, 거절문이 짧은 후보를 댄다. 이미 심긴 저장소의 긴 접두어는
        // 위 갈래가 그대로 받는다.
        (Some(p), false) => {
            fresh_prefix(ctx.lang(), p)?;
            p.to_string()
        }
        (None, true) => crate::config::Config::load(&root).map_err(|e| Fail::config(&e, ctx.lang()))?.prefix,
        // **디렉터리 이름에서 만든 것은 줄여서 쓴다** — 사람이 고른 이름이 아니라 거절할
        // 까닭이 없다. 줄였다는 것은 출력이 말한다.
        (None, false) => {
            let full =
                prefix_from(&root).ok_or_else(|| Fail::new(crate::i18n::say(ctx.lang(), "refuse.init_no_prefix")))?;
            let short = shorten(&full);
            if short != full {
                shortened = Some(full);
            }
            short
        }
    };

    // **심는 파일이라 영어 하나다**(moai-9vwy) — `GITATTRIBUTES` 와 같은 까닭이다. 이쪽은 처음
    // 지을 때만 써서(`!again`) 이미 심긴 저장소가 받는 것이 없다.
    let config = format!(
        "# moai — issue tracker settings for this repository\n\
         prefix = \"{prefix}\"\nstatuses = \"{DEFAULT_STATUSES}\"\n\
         # How a person is shown: full (`Name (email)`) · name · email\nnaming = \"full\"\n"
    );
    // 설정을 먼저 검사한다 — 접두어가 형식에 안 맞으면 파일을 만들기 전에 멈춘다.
    crate::config::Config::parse(&config).map_err(|e| Fail::new(crate::view::config_trouble(ctx.lang(), &e)))?;

    // AGENTS.md 도 **아무것도 심기 전에** 읽는다. 못 읽는 파일(UTF-8 아님·권한)을 빈 글로 치면
    // 블록 하나로 덮어써 사람의 산문이 통째로 사라졌다 — 멈추되, `.moai/` 를 만든 뒤에 멈추면
    // 반쯤 심긴 저장소가 남는다. `--check` 가 같은 파일에 같은 까닭을 댄다(`read_agents`).
    //
    // **안 읽기로 한 자리는 멈추지 않고 건너뛴다**(리뷰 moai-x0o7.52k 5번, 2026-10-03 밤 정함) — 밖·`.git` 으로
    // 가는 링크와 보통 파일이 아닌 것은 쓰기도 거절하는 자리라(`store::target_of`) 지킬 산문이 없다. 멈추던
    // 판은 하위 트래커의 `svc/AGENTS.md -> ../AGENTS.md` 에서 `moai init` 이 늘 1 로 끝나, 보드가 딸린 파일
    // 규칙을 고치라며 대는 `moai init` 이 막다른 길이 됐다. 그 파일은 못 건드린 자리로 이름과 까닭을 댄다.
    let agents_path = root.join("AGENTS.md");
    let mut agents_unheld = None;
    let agents_now = if no_agents {
        None
    } else {
        match read_agents(&root) {
            Ok(read) => Some(read.unwrap_or_default()),
            Err(Fell::Unheld(why)) => {
                agents_unheld = Some(why);
                None
            }
            Err(fell) => {
                let lang = ctx.lang();
                return Err(Fail::new(format!(
                    "{}\n      {}",
                    agents_unread(lang, &root, &fell),
                    say(lang, "refuse.init_agents_unreadable")
                )));
            }
        }
    };

    // 블록과 자리는 비추는 길([`dotfile_notice`]·[`check`])과 같은 자([`dotfiles`])가 짓는다 — 비추는 길이 요구하는
    // 줄과 여기서 쓰는 줄이 갈릴 자리가 없다.
    //
    // **이 클론에만 두면**(moai-zynt.own) 병합 규칙은 할 일이 없어 `.gitattributes` 가 목록에 없고, 무시 블록에
    // 트래커와 훅 자리를 더해 고른 파일에 쓴다. `.git/info/exclude` 를 고르면 커밋되는 파일은 하나도 안 바뀐다.
    let files = dotfiles(&root, plan.tracking, place.as_ref());
    let attributes = files.iter().find(|d| d.kind == "gitattributes_rules");
    let ignored = files.iter().find(|d| d.kind != "gitattributes_rules");
    let ignore_name = ignored.map_or(".gitignore", |d| d.name);
    let write = |d: &Dotfile| {
        // `.git/info/` 는 템플릿을 끈 저장소에 없다 — 만든다. 못 만들면 아래 쓰기가 제 까닭으로 진다.
        if d.kind == "exclude_rules" {
            let _ = std::fs::create_dir_all(crate::path::dir_of(&d.path));
        }
        ensure_lines(&d.path, &d.block, || ctx.lang())
    };
    // **git 밖에 두는 트래커는 무시 줄을 먼저 쓰고, 못 쓰면 아무것도 안 심는다**(리뷰 moai-zynt.63u). 그 줄이 이
    // 트래커가 git 밖이라는 유일한 기록이다 — `.moai/` 를 먼저 세우고 줄을 못 쓴 판은 커밋될 트래커를 남겼고, 시킨
    // 대로 다시 부른 `init` 은 git 에 물어 그것을 커밋으로 읽어 커밋되는 파일을 심었다. 커밋하는 저장소의 딸린
    // 파일은 지금까지처럼 못 써도 나머지를 심는다(moai-0dwc).
    let early = match ignored {
        Some(d) if !again && !plan.tracking.tracked() => Some(write(d)),
        _ => None,
    };
    if let (Some(d), Some((kind, why))) = (ignored, early.as_ref().and_then(Added::trouble)) {
        return Err(Fail::new(fill(
            say(ctx.lang(), "refuse.init_local_unwritten"),
            &[("name", d.name), ("kind", kind), ("why", why)],
        )));
    }

    if !again {
        std::fs::create_dir_all(&dir).map_err(|e| Fail::new(format!("{}: {e}", dir.display())))?;
        // **저널 파일은 안 짓는다**(moai-nzlo). 새 줄은 `.moai/journal/<메일>.jsonl` 로 가고 그
        // 자리는 첫 쓰기가 만든다 — 빈 `journal.jsonl` 을 심으면 이력이 거기 사는 것으로 읽히는데,
        // 그 파일은 이제 읽기만 하는 옛 자리다. 빈 디렉터리는 git 이 안 담으므로 미리 만들지도 않는다.
        for (name, body) in [("config.toml", config.as_str()), ("issues.jsonl", "")] {
            let p = dir.join(name);
            std::fs::write(&p, body).map_err(|e| Fail::new(format!("{}: {e}", p.display())))?;
        }
    }

    let attrs = attributes.map_or(Added::Already, &write);
    // 이 클론에만 둘 자리를 못 찾으면(`ignored` 가 없으면) 위의 거절(git 저장소가 아니다)이 이미 막았다.
    let ignore = early.unwrap_or_else(|| ignored.map_or(Added::Already, &write));
    // **선언을 쓴 바로 뒤에 그 이름이 가리키는 명령을 심는다**(moai-08bo, 2026-09-21 사용자 결정).
    // 앞 판은 이름만 쓰고 명령은 사람에게 치라고 했다 — 도구가 제 손으로 안 도는 절반이었다.
    // 무엇을 하고 안 하는지는 [`crate::cmd::merge_driver::plant_for_init`] 가 쥔다: 선언이 없는
    // 저장소와 git 저장소가 아닌 자리에는 안 심고, 이미 같은 줄이면 아무것도 안 쓴다.
    let planting = if no_driver { None } else { Some(crate::cmd::merge_driver::plant_for_init(&root)) };
    // 못 건드린 자리 — 이름과 까닭과 **손으로 더할 줄**을 함께 든다(moai-gq1c, moai-0dwc). 줄을 안
    // 대면 사람은 도구가 무엇을 넣으려 했는지 모른 채 파일만 고치게 된다. 못 읽은 것과 못 쓴 것을
    // 가르는 것은 **말뿐이다** — 사람이 할 일이 인코딩과 권한으로 갈린다. 링크는 줄 대신 고칠 말을 댄다
    // ([`Added::Linked`]).
    let untouched: Vec<(&str, &Added, &str)> = attributes
        .map(|d| (d.name, &attrs, d.block.as_str()))
        .into_iter()
        .chain(ignored.map(|d| (d.name, &ignore, d.block.as_str())))
        .filter(|(_, done, _)| done.trouble().is_some())
        .collect();

    // `AGENTS.md` **하나만** 쓴다. `CLAUDE.md` 에도 같은 것을 쓰면 곧 갈라지고,
    // 갈라진 두 벌 중 어느 것이 참인지 아무도 모른다.
    // **쓴 때만 `true` 다**(moai-knn0). 늘 참이던 때는 "블록을 맞췄다" 가 아무것도 안 쓴 자리에도
    // 서서 `이미 다 맞아 있다` 가 `--no-agents` 말고는 닿지 않았고, 낡았다는 알림을 보고 부른
    // 사람이 그 줄만으로는 무엇이 바뀌었는지 몰랐다. `gitattributes`·`gitignore` 가 이미 그 뜻이다.
    // **못 써도 끊지 않는다**(moai-780n, 2026-09-15 사용자 결정). 읽기 전용 파일 하나로 `?` 에
    // 끊기던 때는 `.moai/` 와 `.gitattributes` 는 이미 선 채 접두어도 딸린 파일 안내도 못 찍고
    // 끝났다 — 같은 실행이 만든 말이 통째로 삼켜졌다. **못 읽는 것과는 다르다**: 그쪽은 아무것도
    // 심기 전에 멈춰 남의 산문을 지키지만(위의 `read_agents`), 여기서 잃을 산문은 없다. 안 써진
    // 블록은 다음 `init` 이 채우고, 그 사이는 `init --check` 와 `status` 가 말한다.
    // 안 읽기로 한 AGENTS.md 는 위에서 건너뛰었다 — 못 읽은 자리로 선다. 그 까닭의 말은 그때만 묻는다
    // (`ensure_lines` 와 같다 — `--json` 의 흔한 길은 사용자 설정을 안 연다).
    let mut agents_trouble = agents_unheld.map(|why| Added::Unreadable(crate::held::said(ctx.lang(), &why)));
    let agents = match &agents_now {
        None => false,
        Some(existing) => {
            let block = match plan.guide {
                Guide::File => crate::guide::agents_link(),
                Guide::Block | Guide::Hook | Guide::None => crate::guide::agents(),
            };
            let next = with_block(existing, &block);
            if next == *existing {
                false
            } else {
                match plant(&agents_path, &next) {
                    Ok(()) => true,
                    Err(why) => {
                        agents_trouble = Some(Added::Unwritable { why, missing: Vec::new() });
                        false
                    }
                }
            }
        }
    };
    // **안내 전문은 `.moai/guide.md` 에 통째로 쓴다**(moai-cbfz). `init` 이 쥔 파일이라 블록처럼 마커가 없고,
    // 다시 쓰면 그대로 덮는다 — 사람이 고친 것은 `--check` 가 낡았다고 비춘다. 같으면 안 쓴다.
    //
    // **읽는 자는 AGENTS.md 와 같다**(리뷰 moai-zynt.63u) — 안 읽기로 한 자리(밖·`.git` 으로 가는 링크, FIFO 같은
    // 보통 파일이 아닌 것)는 쓰기도 거절하는 자리라 건드리지 않고 못 건드린 자리로 댄다. 읽기 실패를 "없음" 으로 접던
    // 판은 FIFO 를 쓰려고 열어 `init` 이 영영 멈췄다(moai-x0o7 이 AGENTS.md 에서 막은 그것이다). 그 밖에 못 읽은
    // 글(UTF-8 아님)은 `init` 이 쥔 파일이라 다시 쓴다. 줄 끝만 다른 글은 같은 글이다([`same_guide`]).
    let mut guide_trouble = None;
    let guide_file = plan.guide == Guide::File && {
        let path = root.join(crate::guide::GUIDE_FILE);
        match read_held(&path, &crate::held::Home::of(&root)) {
            Ok(Some(now)) if same_guide(&now) => false,
            Err(Fell::Unheld(why)) => {
                guide_trouble = Some(Added::Unreadable(crate::held::said(ctx.lang(), &why)));
                false
            }
            _ => match plant(&path, &crate::guide::agents()) {
                Ok(()) => true,
                Err(why) => {
                    guide_trouble = Some(Added::Unwritable { why, missing: Vec::new() });
                    false
                }
            },
        }
    };
    // 딸린 파일과 **한 자리에서 말한다** — 못 건드린 것은 이름·갈래·까닭으로 함께 선다. 블록은
    // 줄 몇 개가 아니라 통째로 갈아 끼우는 글이라 "손으로 더할 줄" 이 없다(빈 글을 넘긴다):
    // 사람이 할 일은 쓸 수 있게 고치고 다시 부르는 것(못 썼을 때)이나 그 자리에 보통 파일을 두는 것(안 읽었을
    // 때)뿐이고, 그 말은 [`hand`] 가 빈 자리에서 고른다.
    let untouched: Vec<(&str, &Added, &str)> = untouched
        .into_iter()
        .chain(agents_trouble.iter().map(|t| ("AGENTS.md", t, "")))
        .chain(guide_trouble.iter().map(|t| (crate::guide::GUIDE_FILE, t, "")))
        .collect();
    // **`agents` 가 아니라 "AGENTS.md 를 다뤘는가" 로 묻는다** — `agents` 는 이제 *쓴* 때만 참이라
    // (moai-knn0) 그것으로 물으면 블록이 이미 맞는 저장소에서는 이 안내가 영영 안 선다.
    //
    // **두 이름이 한 파일이면 가리킬 것이 없다**(moai-4oab 리뷰). `AGENTS.md -> CLAUDE.md` 면 블록은 이제
    // 링크 너머의 `CLAUDE.md` 에 들고, 거꾸로 건 `CLAUDE.md -> AGENTS.md` 도 한 파일이다 — 거기에
    // `@AGENTS.md` 를 넣으라는 말은 그 파일이 저를 부르게 하고, 블록에는 `AGENTS.md` 라는 글이 없어
    // `init` 마다 다시 선다. 이름이 아니라 푼 자리로 견준다.
    //
    // **못 읽는 `CLAUDE.md` 에는 말하지 않는다**(moai-x0o7) — 무엇이 들었는지 모르니 가리킴이 빠졌다고 할
    // 수 없다([`gaps_in`] 와 같은 자). 읽는 자는 스냅샷과 같다 — 맨 `fs::read_to_string` 은 FIFO 앞에서
    // 영영 멈췄다. 못 읽은 것을 빈 글로 치면 이제 안 읽는 밖의 링크에 가리킴이 이미 들었어도 "더하라" 고 한다.
    let claude = root.join("CLAUDE.md");
    let claude_needs_pointer = agents_now.is_some()
        && claude.exists()
        && crate::path::real(&agents_path) != crate::path::real(&claude)
        && read_held(&claude, &crate::held::Home::of(&root)).is_ok_and(|t| t.is_some_and(|t| !t.contains("AGENTS.md")));

    // **심은 뒤에 이어 부른다**(moai-785q) — 훅·스킬 설치와 프로젝트 목록. 제 명령을 그대로 부르고 그 말을
    // 보고에 얹는다: 같은 일을 두 벌 두면 한쪽만 고쳐진다. 실패해도 심은 것은 그대로 두고 0 으로 끝낸다 —
    // 머지 드라이버를 못 심었을 때와 같다. 설치는 `claude` 를 부르는 일이라 이 기계에 없을 수 있다.
    //
    // **그 명령이 세운 부분 실패 깃발은 거둔다**(리뷰 moai-zynt.63u) — `skill install` 은 등록을 못 하면 `Ok` 를
    // 내면서 깃발로 종료 코드를 1 로 만든다. 그대로 두면 다 심은 `init` 이 1 로 끝나, 종료 코드만 보는 쪽이 실패로
    // 읽는다. 못 한 것은 그 명령의 말(`!` 줄, `--json` 의 `registered: false`)이 이미 댄다. 이 명령이 앞서 세운
    // 깃발은 그대로 둔다.
    let partial = super::take_partial();
    let skilled = plan.skill.then(|| crate::cmd::skill::install(ctx, "local", false));
    let listed = plan.project.then(|| crate::cmd::project::add(ctx, &root));
    super::take_partial();
    if partial {
        super::note_partial();
    }

    if ctx.json {
        let mut v = serde_json::json!({
            "root": root.display().to_string(),
            "prefix": prefix,
            "created": !again,
            "tracking": plan.tracking.word(),
            "guide": plan.guide.word(),
            "guide_file": guide_file,
            "gitattributes": matches!(attrs, Added::Wrote { .. }),
            "gitignore": matches!(ignore, Added::Wrote { .. }),
            "agents": agents,
            // **늘 서는 키다**(moai-08bo). `--no-driver` 는 `off` 와 같은 낱말을 쓰지 않는다 —
            // 안 쓰기로 한 저장소와 이번 한 번만 건너뛴 것은 다음에 칠 명령이 다르다.
            "driver": match &planting {
                None => "skipped",
                Some(Planting::Off) => "off",
                Some(Planting::Already) => "current",
                Some(Planting::Planted(_)) => "planted",
                Some(Planting::Failed(_)) => "failed",
            },
        });
        // 심은 명령과 못 심은 까닭은 **있을 때만** 싣는다 — 늘 `null` 을 두면 대부분의 줄이
        // 헛 키를 든다(`shortened_from` 과 같은 자).
        match &planting {
            Some(Planting::Planted(cmd)) => v["driver_command"] = serde_json::json!(cmd),
            Some(Planting::Failed(why)) => v["driver_trouble"] = serde_json::json!(why),
            _ => {}
        }
        // 줄였을 때만 싣는다 — 늘 `null` 을 두면 줄이지 않은 대부분의 줄이 헛 키를 든다.
        if let Some(full) = &shortened {
            v["shortened_from"] = serde_json::json!(full);
        }
        // 이어 부른 명령은 그 명령의 `--json` 을 그대로 싣는다. 안 불렀으면 `false`, 실패했으면 그 코드와 말이다.
        for (key, done) in [("skill", &skilled), ("project", &listed)] {
            v[key] = match done {
                None => serde_json::json!(false),
                Some(Ok(lines)) => {
                    lines.first().and_then(|l| serde_json::from_str(l).ok()).unwrap_or(serde_json::Value::Bool(true))
                }
                Some(Err(f)) => serde_json::json!({ "code": f.code, "error": f.message }),
            };
        }
        // **위에 트래커가 있었다는 것은 기계에게도 말한다**(moai-pjrr). 사람에게는 알림 한 줄인데
        // 여기만 조용하면, 고리를 짜는 쪽은 제가 방금 둘째 트래커를 세웠다는 것을 어디서도 못 본다.
        // 싣는 것은 **그 트래커의 뿌리**다 — `root` 와 같은 자라 견주는 쪽이 꼴을 다시 안 배운다.
        if let Some(Elsewhere::Above(at)) = &elsewhere {
            v["above"] = serde_json::json!(at.display().to_string());
        }
        // **못 건드린 자리는 기계에게도 말한다.** `false` 만 보면 "이미 다 있었다" 와 구별이
        // 안 되고, 그 차이가 곧 사람이 손볼 것이 남았는지다.
        //
        // **까닭 옆에 갈래를 싣는다**(리뷰 moai-humk). `Permission denied` 는 못 읽을 때도 못 쓸
        // 때도 똑같이 떠서, 까닭 글만으로는 '다시 인코딩하라' 와 'chmod 하라' 가 안 갈린다 —
        // 사람 출력이 낱말로 가르는 것을 기계도 받아야 이 키가 제 몫을 한다. 키 이름도
        // `unreadable` 이 아니다: 못 쓴 자리까지 드는 자리라 그 이름은 이제 거짓말이다
        // (이번 에픽에서 난 키라 읽는 쪽이 아직 없다 — 고칠 자리는 지금뿐이다).
        if !untouched.is_empty() {
            v["untouched"] = serde_json::json!(
                untouched
                    .iter()
                    .filter_map(|(name, done, _)| done
                        .trouble()
                        .map(|(kind, why)| (*name, serde_json::json!({ "kind": kind, "why": why }))))
                    .collect::<std::collections::BTreeMap<_, _>>()
            );
        }
        return super::json_line(&v);
    }

    // **`--json` 을 지난 뒤에 푼다**(리뷰) — 값나가는 것은 되풀이가 아니라 **첫 부름**이다.
    // 두 번째부터의 `ctx.lang()` 은 이미 채워진 `OnceLock` 한 번 읽기지만, 첫 부름은 사용자
    // 설정을 열어 파싱한다. 그러니 기계 출력으로 빠지는 판(`super::json_line` 이 위에서 돌아간다)
    // 은 그 파일을 아예 안 연다. [`check`] 도 같은 자리에서 푼다. 위쪽 거절 갈래들이 `ctx.lang()`
    // 을 제자리에서 부르는 것도 같은 셈이다 — 거기까지 가면 어차피 사람에게 글을 내야 한다.
    let lang = ctx.lang();
    let mut out = if again {
        vec![fill(say(lang, "init.already"), &[("prefix", &prefix)])]
    } else {
        vec![
            fill(say(lang, "init.made"), &[("prefix", &prefix)]),
            fill(say(lang, "init.columns"), &[("columns", &DEFAULT_STATUSES.replace(',', " → "))]),
        ]
    };
    // **위에도 트래커가 있으면 세우고 나서 말한다**(moai-pjrr, 2026-09-20 사용자 결정 둘째 판).
    // 막지 않는 까닭은 [`crate::store::planted_elsewhere`] 에 있다 — 여기 심은 것은 이 밑에서 실제로 읽힌다.
    // 그래도 말은 해야 한다: 이 줄이 없으면 `<프로젝트>/src/deep` 에 선 사람이 옆 디렉터리와 다른
    // 파일을 쓰기 시작한 것을 모른 채 "왜 내 이슈가 안 보이나" 를 딴 데서 찾는다.
    if let Some(Elsewhere::Above(at)) = &elsewhere {
        out.push(fill(say(lang, "init.tracker_above"), &[("at", &at.join(".moai").display().to_string())]));
        out.push(say(lang, "init.tracker_above_note").to_string());
    }
    // 접두어는 나중에 못 바꾸므로 **지금** 말한다 — 이슈를 하나라도 만들면 되돌릴 길이 없다.
    if let Some(full) = &shortened {
        out.insert(1, fill(say(lang, "init.prefix_shortened"), &[("full", full), ("max", &PREFIX_MAX.to_string())]));
    }
    // **한 일을 한 대로 부른다** — 규칙을 넣은 판과 주석만 맞춘 판은 말이 다르다. 주석만 맞춘
    // 판도 "이미 다 맞아 있다" 는 아니다: 파일을 고쳤다.
    match attrs {
        Added::Wrote { rules: true } => out.push(say(lang, "init.wrote_gitattributes").to_string()),
        Added::Wrote { rules: false } => {
            out.push(fill(say(lang, "init.wrote_comments"), &[("name", ".gitattributes")]))
        }
        _ => {}
    }
    match (&ignore, plan.tracking.tracked()) {
        (Added::Wrote { rules: true }, true) => out.push(say(lang, "init.wrote_gitignore").to_string()),
        (Added::Wrote { rules: true }, false) => {
            out.push(fill(say(lang, "init.wrote_local"), &[("name", ignore_name)]))
        }
        (Added::Wrote { rules: false }, _) => {
            out.push(fill(say(lang, "init.wrote_comments"), &[("name", ignore_name)]))
        }
        _ => {}
    }
    // **한 일만 말한다** — 이미 서 있던 줄과 안 쓰기로 한 저장소는 조용하다. 못 심은 것은
    // 끊지 않고 말한다: `.git/config` 하나 때문에 트래커를 세운 말까지 삼키면 안 된다
    // (읽기 전용 AGENTS.md 를 `?` 로 끊지 않는 moai-780n 과 같은 자다).
    match &planting {
        Some(Planting::Planted(cmd)) => out.push(fill(say(lang, "init.planted_driver"), &[("cmd", cmd)])),
        Some(Planting::Failed(why)) => out.push(fill(say(lang, "init.driver_trouble"), &[("why", why)])),
        _ => {}
    }
    for (name, done, block) in &untouched {
        // 못 읽은 것과 못 쓴 것은 **사람이 할 일이 다르다** — 인코딩을 고칠 일과 권한을 열 일이다.
        // 갈래를 빠짐없이 적는다: `_` 로 받던 때는 갈래가 하나 느는 날 그것이 말없이 "못 읽어"
        // 로 서고 까닭 자리가 빈 채 나갔다(리뷰 moai-humk).
        let head = match done {
            Added::Unwritable { why, .. } => fill(say(lang, "init.unwritable"), &[("name", name), ("why", why)]),
            Added::Unreadable(why) => fill(say(lang, "init.unreadable"), &[("name", name), ("why", why)]),
            // 링크는 **손으로 더할 줄을 안 댄다**([`Added::Linked`]) — 따라 적는 곳이 그 링크다. 고칠 말은
            // 가리키는 곳에 따라 갈린다(리뷰): 체크아웃 안이면 그 내용을 보통 파일로 옮기고, 밖이면
            // (`~/.bashrc`) 옮겨 담는 순간 그 파일이 이 저장소의 커밋에 실리므로 링크만 걷는다.
            Added::Linked { to, outside } => {
                out.push(fill(say(lang, "init.linked"), &[("name", name), ("to", to)]));
                out.push(match outside {
                    true => say(lang, "init.linked_fix_outside").to_string(),
                    false => say(lang, "init.linked_fix").to_string(),
                });
                continue;
            }
            Added::Wrote { .. } | Added::Already => continue,
        };
        out.push(head);
        // **댈 줄이 없는 자리도 있다** — AGENTS.md 블록은 줄 몇 개가 아니라 통째로 갈아 끼우는
        // 글이라 손으로 옮겨 적을 것이 아니다. 그 자리는 상태를 보는 길을 대신 댄다(moai-780n).
        //
        // **못 읽은 AGENTS.md 는 고칠 말이 다르다**(리뷰 moai-x0o7.52k) — 여기 오는 것은 안 읽기로 한 자리뿐이고
        // (권한·UTF-8 로 못 읽은 것은 아무것도 심기 전에 멈췄다), 그 파일은 읽히고 쓸 수도 있어 "쓸 수 있게 고쳐라"
        // 는 헛걸음이다. 할 일은 그 자리에 보통 파일을 두거나 `--no-agents` 로 그대로 두는 것이다.
        if done.hand(block).is_empty() {
            out.push(match done {
                Added::Unreadable(_) => say(lang, "init.agents_unheld_fix").to_string(),
                _ => say(lang, "init.untouched_fix").to_string(),
            });
            continue;
        }
        out.push(say(lang, "init.untouched_hand").to_string());
        // **한 줄에 하나씩 낸다** — 쉼표로 이으면 붙여 넣은 것이 한 줄이 되어 규칙이 안 선다.
        // `.gitattributes` 는 더 나쁘다: `<패턴> text eol=lf, <패턴> …` 은 첫 패턴에 쓰레기
        // 속성을 달 뿐이라 `journal.jsonl` 이 `merge=union` 을 영영 못 받는다.
        // **못 쓴 자리는 빠진 줄만 낸다**([`Added::hand`]) — 파일을 읽었으니 이미 있는 줄을 안다.
        for line in done.hand(block) {
            out.push(format!("      {line}"));
        }
    }
    if agents {
        out.push(say(lang, "init.agents_synced").to_string());
    }
    if guide_file {
        out.push(fill(say(lang, "init.wrote_guide_file"), &[("file", crate::guide::GUIDE_FILE)]));
    }
    // 이어 부른 명령의 말은 그 이름 아래 들여 싣는다 — 어느 말이 어느 명령의 것인지 갈린다. 이름은 **사람이 그대로
    // 다시 칠 수 있는 줄이다** — 실패하면 "손으로 다시 부른다" 고 하므로, `-C` 로 불렀으면 그 자리를 붙인다(리뷰
    // moai-zynt.63u). 맨 `moai project add .` 를 대던 판은 따라 친 셸의 자리를 목록에 올렸다. `-C` 를 붙이는 규칙은
    // [`crate::report::Warning::cli_hint`] 하나다.
    let away = away_root(&root, ctx.chdir);
    let ran = [("skill install --scope local", &skilled), ("project add .", &listed)];
    for (tail, done) in ran {
        let cmd = crate::report::Warning::cli_hint(away.as_deref(), tail);
        match done {
            None => {}
            Some(Ok(lines)) => {
                out.push(fill(say(lang, "init.ran"), &[("cmd", &cmd)]));
                out.extend(lines.iter().filter(|l| !l.trim().is_empty()).map(|l| format!("    {l}")));
            }
            Some(Err(f)) => {
                out.push(fill(say(lang, "init.ran_failed"), &[("cmd", &cmd), ("why", &one_line(&f.message))]))
            }
        }
    }
    // **줄 수가 아니라 한 일로 묻는다.** 줄을 세던 때는 이 자리 위에 줄 하나를 더하는 것만으로
    // 이 안내가 말없이 사라졌다 — `moai-knn0` 전까지 `agents` 가 늘 참이라 실제로 그랬다.
    // `Already` 는 "다 있어서 안 건드렸다" 뿐이다 — 못 읽은 자리는 `Unreadable` 이라 여기서 걸린다.
    // **못 건드린 자리가 있으면 "다 맞아 있다" 가 아니다**(moai-780n) — 못 쓴 AGENTS.md 는 딸린
    // 파일이 둘 다 `Already` 여도 남은 일이다.
    let did_nothing = attrs == Added::Already
        && ignore == Added::Already
        && !agents
        && !guide_file
        && skilled.is_none()
        && listed.is_none()
        && untouched.is_empty()
        && !matches!(planting, Some(Planting::Planted(_) | Planting::Failed(_)));
    if again && did_nothing {
        out.push(say(lang, "init.all_current").to_string());
    }
    if claude_needs_pointer {
        out.push(String::new());
        out.push(say(lang, "init.claude_pointer").to_string());
    }
    if !again {
        out.push(String::new());
        out.push(say(lang, "init.next").to_string());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// [`ensure_lines`] 가 거절의 까닭을 펼 말 — 시험은 영어로 잰다.
    fn en() -> crate::i18n::Lang {
        crate::i18n::Lang::En
    }

    /// **선언을 거는 자리와 묻는 자리가 한 글을 쓴다**(moai-9khu). `.gitattributes` 에 쓰는 줄과
    /// `check-attr` 로 묻는 경로가 갈리면 "안 심었다" 알림이 영영 안 서거나(묻는 자리가 틀렸다)
    /// 영영 안 걷힌다(쓰는 자리가 틀렸다) — 둘 다 조용해서 아무도 모른다.
    ///
    /// **마지막 줄을 본다**(리뷰 moai-vbmn.spv). git 은 같은 경로에 걸린 규칙 중 **뒤엣것**을
    /// 쓰므로, 같은 경로를 **글자 그대로** 두 번 거는 날 첫 줄을 보던 판은 틀린 줄을 잰다.
    ///
    /// **낱말째 맞춘다.** `starts_with` 만으로는 `.moai/issues.jsonlX` 도 든다.
    ///
    /// **여기가 못 잡는 것**(리뷰 moai-vbmn.spv): 뒤에 붙는 것이 글자가 아니라 **패턴**이면
    /// (`.moai/*.jsonl merge=union`) 이 거르개에 안 걸려 그대로 푸른데, `check-attr` 은 `union` 을
    /// 답해 알림이 영영 안 선다. 글로 패턴을 푸는 규칙을 여기 또 쓰면 git 과 갈리고 갈리는 쪽은
    /// 늘 이쪽이라(`declared` 가 `check-attr` 에게 묻는 까닭과 같다), 그 갈래를 실제로 재는 자는
    /// 임시 저장소에 이 글을 깔고 `status` 를 부르는 `tests/cli.rs` 쪽이다.
    #[test]
    fn the_declared_path_is_the_one_init_writes() {
        let path = crate::cmd::merge_driver::SNAPSHOT;
        let rule = GITATTRIBUTES
            .lines()
            .rfind(|l| l.strip_prefix(path).is_some_and(|rest| rest.starts_with(char::is_whitespace)))
            .unwrap_or_else(|| panic!("{path} 에 거는 줄이 없다\n{GITATTRIBUTES}"));
        assert!(
            rule.contains(&format!("merge={}", crate::cmd::merge_driver::DRIVER)),
            "그 줄이 드라이버를 안 건다 — {rule}"
        );
    }

    /// **링크인 트래커가 가리키는 파일의 `merge=union` 도 결정이 아니다**(리뷰). 스냅샷을 경로로 알아보던
    /// 판은 그 줄을 사람의 결정으로 읽어 `merge=moai` 줄을 안 쓰고 알림도 재웠다 — 그 파일이 union 으로
    /// 합쳐져 한 id 가 두 줄로 섰다. 사람이 union 아닌 값을 적은 것은 그대로 결정이다.
    #[test]
    fn union_on_the_file_a_linked_tracker_points_at_is_not_a_decision() {
        let want = "/shared/issues.jsonl   text eol=lf merge=moai";
        assert_eq!(missing_rules("shared/issues.jsonl merge=union\n", want), [want], "union 을 결정으로 읽었다");
        assert!(missing_rules("shared/issues.jsonl -merge\n", want).is_empty(), "사람의 결정을 덮었다");
        // **스냅샷에 적은 결정은 가리키는 파일에도 선다** — 드라이버를 안 쓰기로 한 저장소가 링크 하나로
        // 다시 졸리면 안 된다. 스냅샷에 드라이버를 거는 우리 줄과 union 은 결정이 아니다.
        assert!(missing_rules(".moai/issues.jsonl   text eol=lf -merge\n", want).is_empty(), "스냅샷의 결정을 덮었다");
        let ours = ".moai/issues.jsonl   text eol=lf merge=moai\n";
        assert_eq!(missing_rules(ours, want), [want], "스냅샷에 건 우리 줄로 가리키는 파일까지 덮었다고 읽었다");
        assert_eq!(missing_rules(".moai/issues.jsonl merge=union\n", want), [want]);
        // 저널은 반대다 — 거기 걸리는 값이 union 이다.
        let journal = ".moai/journal/*.jsonl  text eol=lf merge=union";
        assert!(missing_rules(".moai/journal/*.jsonl merge=union\n", journal).is_empty());
    }

    /// **옮겨 적은 저널 줄도 원래 줄에 적은 결정을 받는다**(리뷰). `.moai -> tracker` 면 저널 줄이
    /// `/tracker/journal…` 로 옮겨 서는데, `.moai/journal…` 에 `-merge` 를 적은 저장소에 옮긴 줄을 요구하면
    /// 알림이 영영 서고 그 `init` 이 union 을 도로 건다 — 스냅샷에서 막은 고리가 저널로 다시 열린다.
    /// 우리 줄을 그대로 적은 저장소와, 블록이 거는 줄이 아닌 넓은 패턴은 결정이 아니다.
    #[test]
    fn a_decision_on_a_journal_line_carries_to_where_it_moved() {
        let file = "/tracker/journal.jsonl  text eol=lf merge=union";
        let split = "/tracker/journal/*.jsonl  text eol=lf merge=union";
        assert!(
            missing_rules(".moai/journal.jsonl  text eol=lf -merge\n", file).is_empty(),
            "옛 한 파일의 결정을 덮었다"
        );
        assert!(missing_rules(".moai/journal/*.jsonl  text eol=lf -merge\n", split).is_empty(), "저널의 결정을 덮었다");
        // 결정은 그 줄의 것만 옮긴다 — 한 파일에 적은 결정이 사람마다 갈린 저널까지 덮지 않는다.
        assert_eq!(missing_rules(".moai/journal.jsonl  text eol=lf -merge\n", split), [split]);
        let ours = ".moai/journal/*.jsonl  text eol=lf merge=union\n";
        assert_eq!(missing_rules(ours, split), [split], "우리 줄로 옮긴 줄까지 덮었다고 읽었다");
        let wide = ".moai/journal/*.jsonl  text eol=lf merge=union";
        assert_eq!(missing_rules(".moai/*.jsonl -merge\n", wide), [wide], "블록 밖의 넓은 패턴을 결정으로 읽었다");
    }

    /// **트래커가 링크인지는 푼 자리로 잰다**(리뷰) — 끝 조각이 링크인 판도, `.moai` 가 링크인 판도 줄은
    /// 딴 파일에 살고 git 은 그 파일을 그 경로로 합친다. 끝 조각만 보던 판은 뒤의 것을 링크가 아니라고
    /// 읽어 병합 줄을 안 세웠다. 뿌리 밖과, 규칙 줄에 못 싣는 이름(빈칸·패턴 글자·제어 문자)은 안 건다.
    #[cfg(unix)]
    #[test]
    fn the_file_a_tracker_lives_in_is_read_through_any_link_on_the_way() {
        use std::os::unix::fs::symlink;
        let s = crate::scratch::Scratch::new("init-linked-snapshot");
        let root = |name: &str| {
            let r = s.join(name);
            std::fs::create_dir_all(&r).unwrap();
            r
        };

        let plain = root("plain");
        std::fs::create_dir(plain.join(".moai")).unwrap();
        std::fs::write(plain.join(".moai/issues.jsonl"), "").unwrap();
        assert_eq!(linked_snapshot(&plain), None, "제자리의 트래커를 링크로 읽었다");
        assert_eq!(attributes_for(&plain), GITATTRIBUTES);

        let file = root("file");
        std::fs::create_dir_all(file.join(".moai")).unwrap();
        std::fs::create_dir_all(file.join("shared")).unwrap();
        symlink("../shared/issues.jsonl", file.join(".moai/issues.jsonl")).unwrap();
        assert_eq!(linked_snapshot(&file).as_deref(), Some("shared/issues.jsonl"));
        let rule = "/shared/issues.jsonl   text eol=lf merge=moai";
        assert!(attributes_for(&file).lines().any(|l| l == rule), "스냅샷 줄의 속성을 그대로 안 옮겼다");

        let dir = root("dir");
        std::fs::create_dir_all(dir.join("tracker")).unwrap();
        std::fs::write(dir.join("tracker/issues.jsonl"), "").unwrap();
        symlink("tracker", dir.join(".moai")).unwrap();
        assert_eq!(linked_snapshot(&dir).as_deref(), Some("tracker/issues.jsonl"), "`.moai` 가 링크인 판을 놓쳤다");

        // 제어 문자는 패턴 글자와 따로 잰다 — `ESC c` 는 터미널을 통째로 되돌리는데 `[` 도 `]` 도 안 든다.
        for (name, target) in [("space", "shared data"), ("glob", "sh*red"), ("control", "sh\u{1b}cared")] {
            let r = root(name);
            std::fs::create_dir_all(r.join(".moai")).unwrap();
            std::fs::create_dir_all(r.join(target)).unwrap();
            symlink(format!("../{target}/issues.jsonl"), r.join(".moai/issues.jsonl")).unwrap();
            assert_eq!(linked_snapshot(&r), None, "{name}: 규칙 줄에 못 싣는 이름을 걸었다");
        }
        let away = root("away");
        let out = root("out");
        std::fs::create_dir_all(out.join(".moai")).unwrap();
        symlink(away.join("issues.jsonl"), out.join(".moai/issues.jsonl")).unwrap();
        assert_eq!(linked_snapshot(&out), None, "뿌리 밖을 가리키는 링크에 줄을 걸었다");

        // **`.git/` 안은 줄로 안 건다**(moai-x0o7.2q8) — 끝 조각이 그리 가든 `.moai` 가 그리 가든, 딸린 파일의
        // 블록은 제자리의 것 그대로다.
        let git = root("git");
        std::fs::create_dir_all(git.join(".git/moai")).unwrap();
        std::fs::create_dir_all(git.join(".moai")).unwrap();
        symlink("../.git/moai/issues.jsonl", git.join(".moai/issues.jsonl")).unwrap();
        assert_eq!(linked_snapshot(&git), None, ".git 안을 가리키는 스냅샷에 줄을 걸었다");
        assert_eq!((attributes_for(&git), gitignore_for(&git)), (GITATTRIBUTES.into(), GITIGNORE.into()));
        let moved = root("git-moved");
        std::fs::create_dir_all(moved.join(".git/moai")).unwrap();
        std::fs::write(moved.join(".git/moai/issues.jsonl"), "").unwrap();
        symlink(".git/moai", moved.join(".moai")).unwrap();
        assert_eq!(moai_moved(&moved), None, ".git 안으로 간 `.moai` 에 줄을 걸었다");
        assert_eq!((attributes_for(&moved), gitignore_for(&moved)), (GITATTRIBUTES.into(), GITIGNORE.into()));
        // **끝 이름도 센다**(리뷰 moai-x0o7.52k) — 딸린 워크트리의 `.git` 은 디렉터리가 아니라 `gitdir:` 한 줄짜리
        // 파일이라, 트래커 링크가 그 파일 자체로 풀리면 `.git` 은 가운데가 아니라 끝 조각에만 선다.
        let gitfile = root("git-file");
        std::fs::write(gitfile.join(".git"), "gitdir: /x\n").unwrap();
        std::fs::create_dir_all(gitfile.join(".moai")).unwrap();
        symlink("../.git", gitfile.join(".moai/issues.jsonl")).unwrap();
        assert_eq!(linked_snapshot(&gitfile), None, "딸린 워크트리의 .git 파일로 풀리는 스냅샷에 줄을 걸었다");
    }

    /// **링크인 트래커는 락·임시 파일·저널의 규칙도 그 자리로 옮긴다**(moai-th3b). git 은 링크를 안 따라가
    /// `.moai/lock` 은 `tracker/lock` 에 안 걸린다. `.moai` 가 링크면 넷 다 옮기고, 끝 조각만 링크면
    /// `far_lock` 과 물러선 임시 파일이 서는 그 파일의 디렉터리에 **그 둘만** 옮긴다 — 저널은 제자리고, 그
    /// 자리의 남의 `*.tmp.*` 는 안 가린다(리뷰). 가리키는 파일이 뿌리에 있으면 줄도 뿌리에 선다(리뷰).
    #[cfg(unix)]
    #[test]
    fn a_linked_tracker_moves_its_lock_tmp_and_journal_rules_to_where_it_lives() {
        use std::os::unix::fs::symlink;
        let s = crate::scratch::Scratch::new("init-linked-dirs");
        let has = |block: &str, line: &str| block.lines().any(|l| l == line);

        let plain = s.join("plain");
        std::fs::create_dir_all(plain.join(".moai")).unwrap();
        assert_eq!(gitignore_for(&plain), GITIGNORE, "제자리 트래커에 줄을 더했다");

        let dir = s.join("dir");
        std::fs::create_dir_all(dir.join("tracker")).unwrap();
        std::fs::write(dir.join("tracker/issues.jsonl"), "").unwrap();
        symlink("tracker", dir.join(".moai")).unwrap();
        let (ignore, attrs) = (gitignore_for(&dir), attributes_for(&dir));
        assert!(has(&ignore, "/tracker/lock") && has(&ignore, "/tracker/*.tmp.*"), "{ignore}");
        assert!(has(&attrs, "/tracker/journal.jsonl  text eol=lf merge=union"), "{attrs}");
        assert!(has(&attrs, "/tracker/journal/*.jsonl  text eol=lf merge=union"), "{attrs}");
        assert!(has(&attrs, "/tracker/issues.jsonl   text eol=lf merge=moai"), "{attrs}");

        let file = s.join("file");
        std::fs::create_dir_all(file.join(".moai")).unwrap();
        std::fs::create_dir_all(file.join("shared")).unwrap();
        symlink("../shared/issues.jsonl", file.join(".moai/issues.jsonl")).unwrap();
        let (ignore, attrs) = (gitignore_for(&file), attributes_for(&file));
        assert!(has(&ignore, "/shared/lock") && has(&ignore, "/shared/issues.jsonl.tmp.*"), "{ignore}");
        assert!(!ignore.contains("/shared/*.tmp.*"), "사람의 디렉터리의 임시 파일을 통째로 가렸다: {ignore}");
        assert!(!attrs.contains("/shared/journal"), "제자리 저널을 옮겼다: {attrs}");
        assert!(!ignore.contains("/.moai/"), "제자리를 한 벌 더 적었다: {ignore}");

        let top = s.join("top");
        std::fs::create_dir_all(top.join(".moai")).unwrap();
        symlink("../issues.jsonl", top.join(".moai/issues.jsonl")).unwrap();
        let (ignore, attrs) = (gitignore_for(&top), attributes_for(&top));
        assert!(has(&ignore, "/lock") && has(&ignore, "/issues.jsonl.tmp.*"), "뿌리에 선 락을 안 막았다: {ignore}");
        assert!(has(&attrs, "/issues.jsonl   text eol=lf merge=moai"), "{attrs}");

        // 옮긴 줄을 실제로 git 이 읽는지 — 줄의 꼴이 아니라 결과로 잰다. git 은 `git::isolated` 로만 띄운다
        // (리뷰) — 돌리는 사람의 전역 설정·기본 무시 파일(`~/.config/git/ignore`)·틀 디렉터리가 새면 옮긴
        // 줄이 없어도 푸르거나 까닭 없이 붉다. `check-ignore` 는 걸린 것이 없으면 1 로 끝나 끝 코드는 안 본다.
        for at in [&dir, &file, &top] {
            crate::git::tests::run_git(at, None, &["init", "-q", "--template="]);
            std::fs::write(at.join(".gitignore"), &*gitignore_for(at)).unwrap();
            std::fs::write(at.join(".gitattributes"), &*attributes_for(at)).unwrap();
        }
        let git = |at: &Path, args: &[&str]| {
            let o = crate::git::isolated(at)
                .args(["-c", "core.excludesFile=/dev/null", "-c", "core.attributesFile=/dev/null"])
                .args(args)
                .output()
                .unwrap();
            String::from_utf8_lossy(&o.stdout).into_owned()
        };
        let ignored = |at: &Path, paths: &[&str]| {
            let mut args = vec!["check-ignore"];
            args.extend_from_slice(paths);
            git(at, &args).lines().count()
        };
        assert_eq!(ignored(&dir, &["tracker/lock", "tracker/issues.jsonl.tmp.1.0"]), 2);
        assert!(git(&dir, &["check-attr", "merge", "tracker/journal/a_b.jsonl"]).contains("merge: union"));
        assert_eq!(ignored(&file, &["shared/lock", "shared/issues.jsonl.tmp.1.0.00ab.1"]), 2);
        assert_eq!(ignored(&file, &["shared/release.tmp.md"]), 0, "사람의 파일을 가렸다");
        assert_eq!(ignored(&top, &["lock", "issues.jsonl.tmp.1.0"]), 2);
        assert_eq!(ignored(&top, &["notes.tmp.md"]), 0, "뿌리의 남의 임시 파일을 가렸다");
    }

    /// **링크인 딸린 파일에는 안 쓰고, 빠진 줄 대신 링크라고 말한다**(moai-yke5). git 은 2.32 부터 체크아웃
    /// 안의 링크인 `.gitignore`·`.gitattributes` 를 안 읽는다 — 안을 가리키는 링크에 덧붙이던 판은 규칙이
    /// 하나도 안 선 채 "썼다" 고 했고, 밖을 가리키는 링크는 `init` 이 못 걷는 알림을 영영 세웠다.
    ///
    /// **밖을 가리키는지는 푼 자리로 잰다**(리뷰) — 고칠 말이 갈린다: 밖(`~/.bashrc`)을 옮겨 담으라고 하면
    /// 그 파일이 커밋에 실린다. **링크 글은 한 줄로 걸러 싣는다**(리뷰) — 그 글이 터미널로 그대로 나간다.
    /// 둘 다 링크여도 알림은 하나다(리뷰).
    #[cfg(unix)]
    #[test]
    fn a_linked_dotfile_is_never_written_and_is_named_as_a_link() {
        use std::os::unix::fs::symlink;
        let s = crate::scratch::Scratch::new("init-linked-dotfile");
        let root = s.join("repo");
        std::fs::create_dir_all(root.join("conf")).unwrap();
        std::fs::write(root.join("conf/ignore"), "target/\n").unwrap();
        symlink("conf/ignore", root.join(".gitignore")).unwrap();
        std::fs::write(root.join(".gitattributes"), GITATTRIBUTES).unwrap();

        let linked = |to: &str, outside: bool| Added::Linked { to: to.to_string(), outside };
        assert_eq!(ensure_lines(&root.join(".gitignore"), GITIGNORE, en), linked("conf/ignore", false));
        assert_eq!(std::fs::read_to_string(root.join("conf/ignore")).unwrap(), "target/\n", "링크 너머에 썼다");
        let files = dotfiles_within(&root, None);
        assert_eq!(linked_in(&files), vec![".gitignore"]);
        assert!(gaps_in(&files).is_empty(), "링크를 빠진 줄로 말했다: {:?}", gaps_in(&files));
        let notes = dotfile_notice(&root, false);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].kind, "dotfile_linked");
        assert!(linked("", false).hand(GITIGNORE).is_empty(), "링크에 적을 줄을 댔다");

        // 체크아웃 밖 — 가운데 디렉터리가 링크라 글만으로는 안처럼 보이는 것도 밖이다.
        let home = s.join("home");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(home.join("rc"), "export TOKEN=x\n").unwrap();
        symlink(&home, root.join("conf/shared")).unwrap();
        std::fs::remove_file(root.join(".gitattributes")).unwrap();
        symlink("conf/shared/rc", root.join(".gitattributes")).unwrap();
        assert_eq!(ensure_lines(&root.join(".gitattributes"), GITATTRIBUTES, en), linked("conf/shared/rc", true));
        assert_eq!(std::fs::read_to_string(home.join("rc")).unwrap(), "export TOKEN=x\n", "체크아웃 밖에 썼다");
        let notes = dotfile_notice(&root, false);
        assert_eq!(notes.len(), 1, "링크 둘에 알림이 둘 섰다: {notes:?}");
        assert_eq!(notes[0].ids, [".gitattributes", ".gitignore"]);

        // 링크 글에 든 ESC 와 줄바꿈은 걷는다 — 화면을 다시 칠하거나 없는 줄을 지어내면 안 된다.
        std::fs::remove_file(root.join(".gitignore")).unwrap();
        symlink("conf/\u{1b}[2Jx\n  everything is already in line", root.join(".gitignore")).unwrap();
        let Added::Linked { to, .. } = ensure_lines(&root.join(".gitignore"), GITIGNORE, en) else {
            panic!("링크를 링크로 안 읽었다")
        };
        assert!(!to.chars().any(char::is_control), "링크 글의 제어 문자를 그대로 실었다: {to:?}");
    }

    /// **`\n` 없이 끝난 딸린 파일 뒤에도 빈 줄은 하나다**(moai-a65c). 꼬리를 채우는 것은 이제
    /// `store::append_inside` 하나다 — 여기도 채우던 판이 그대로 남으면 앞 규칙과 이 블록 사이에 빈 줄이 둘 선다.
    #[test]
    fn a_dotfile_without_a_trailing_newline_gets_one_blank_line() {
        let s = crate::scratch::Scratch::new("init-torn-dotfile");
        let root = s.join("repo");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join(".gitignore"), "target/").unwrap();
        assert!(matches!(ensure_lines(&root.join(".gitignore"), GITIGNORE, en), Added::Wrote { .. }));
        // 바이트째 견준다(리뷰) — 앞머리만 보던 판은 블록이 두 벌 서거나 줄이 빠지거나 끝에 빈 줄이 더 서도 푸르렀다.
        let text = std::fs::read_to_string(root.join(".gitignore")).unwrap();
        assert_eq!(text, format!("target/\n\n{GITIGNORE}"));
    }

    /// **손잡이를 켠 셸에서는 `--check` 의 끝줄이 둘이다**(moai-ha0f). 첫 줄은 손잡이 없는 셸에
    /// 대한 답이라 그대로 서고, 둘째 줄이 **이 셸**의 답을 댄다 — 한 줄만 두던 판은 `MOAI_HERE=1`
    /// 인 셸에서 "여기 안 선다, 주 체크아웃에서 쳐라" 를 냈는데 같은 셸의 `moai init` 은 여기 심고
    /// 0 으로 끝났다.
    ///
    /// **둘째 줄에 접두어가 박혀 있는가를 함께 잰다.** 손잡이는 이 프로세스 하나에 서므로 접두어
    /// 없는 줄은 베껴 딴 셸에 붙여 넣으면 1 로 끝난다 — 표면이 손잡이를 안 물려받는
    /// (moai-ko4y) 값을 이 접두어가 갚는다.
    ///
    /// **환경을 안 만진다**(`store::tests` 의 같은 자리 글). 단위 시험은 한 프로세스의 스레드로
    /// 나란히 돌아 `set_var` 로 켠 값을 옆 시험이 본다 — 갈림을 인자로 받는 까닭이 그것이고,
    /// 손잡이를 실제로 읽는 자리([`crate::store::here_wanted`])는 부르는 쪽 한 줄이다.
    #[test]
    fn a_split_shell_gets_a_second_line() {
        let main = Path::new("/main");
        let plain = check_worktree(crate::i18n::Lang::En, main, Some("/wt"), false, None);
        assert_eq!(plain.len(), 1, "손잡이 없는 셸에 줄이 둘 섰다 — {plain:?}");
        assert!(plain[0].contains("moai -C /main init"), "주 체크아웃을 안 댔다 — {}", plain[0]);

        let split = check_worktree(crate::i18n::Lang::En, main, Some("/wt"), true, Some(main));
        assert_eq!(split.len(), 2, "갈린 셸에 둘째 줄이 없다 — {split:?}");
        assert_eq!(split[0], plain[0], "첫 줄이 손잡이를 물려받았다 — {}", split[0]);
        assert!(split[1].contains("MOAI_HERE=1 moai -C /wt init"), "빠져나가는 길을 안 댔다 — {}", split[1]);

        // `-C` 없이 부른 판은 그 자리도 없다 — 붙는 규칙은 `cli_hint` 하나가 쥔다.
        let here = check_worktree(crate::i18n::Lang::En, main, None, true, None);
        assert!(here[1].contains("MOAI_HERE=1 moai init"), "맨 명령을 안 댔다 — {}", here[1]);
    }

    /// **읽고 있는 트래커를 가리라고 대지 않는다**(리뷰 moai-uocc.45o 의 2번, 2026-09-21 사용자
    /// 결정). 딸린 워크트리의 밑자리에서 손잡이를 켠 셸은 `<wt>/.moai` 를 읽는데, 거기 "여기
    /// 심는다" 를 그대로 내면 따라 친 사람이 `src/deep` 에 아무도 안 읽는 `.moai` 를 세우고 먼저 쓴
    /// 줄들은 사라진 것처럼 보인다 — `moai init` 이 막는 자리를 안내가 권하게 된다.
    ///
    /// **주 체크아웃을 가리는 것은 이 갈래가 아니다** — 위의 시험이 그쪽(`reading == main`)을 재고,
    /// 그때는 빠져나가는 길을 그대로 댄다.
    #[test]
    fn a_shell_that_already_reads_one_is_not_told_to_shadow_it() {
        let main = Path::new("/main");
        let said = check_worktree(crate::i18n::Lang::En, main, None, true, Some(Path::new("/main/wt")));
        assert_eq!(said.len(), 2, "밑자리에서 둘째 줄이 없다 — {said:?}");
        assert!(said[1].contains("/main/wt/.moai"), "읽고 있는 트래커를 안 댔다 — {}", said[1]);
        assert!(!said[1].contains("MOAI_HERE=1 moai init"), "가릴 명령을 그대로 댔다 — {}", said[1]);
    }

    /// 두 번 넣어도 블록은 하나고, 사람이 쓴 산문은 바이트 단위로 그대로다.
    #[test]
    fn the_block_is_replaced_never_repeated() {
        let mine = "# 우리 규약\n\n손으로 쓴 것.\n";
        let once = with_block(mine, "옛 내용\n");
        assert!(once.starts_with(mine), "{once:?}");
        assert_eq!(once.matches(BEGIN).count(), 1);

        let twice = with_block(&once, "새 내용\n");
        assert_eq!(twice.matches(BEGIN).count(), 1, "{twice:?}");
        assert!(twice.contains("새 내용") && !twice.contains("옛 내용"), "{twice:?}");
        assert!(twice.starts_with(mine), "산문을 건드렸다 — {twice:?}");

        // 한 번 더 넣어도 더는 안 바뀐다
        assert_eq!(with_block(&twice, "새 내용\n"), twice);
    }

    /// **커밋된 AGENTS.md 블록이 지금의 글과 같다.** `guide.rs` 만 고치고
    /// `moai init` 을 안 부르면 이 저장소의 에이전트가 옛 글을 배운다 —
    /// 탐색기가 idea 를 담게 된 뒤에도 "읽기 전용" 이라 적혀 있었다(`moai-ka9p`).
    /// 블록 밖의 산문은 사람의 것이라 보지 않는다.
    /// `moai status` 가 재는 함수 그대로 잰다 — 규칙을 여기 한 벌 더 적으면 둘이 갈라진다.
    #[test]
    fn the_checked_in_agents_block_matches_the_guide() {
        let got = agents_state(Path::new(env!("CARGO_MANIFEST_DIR")));
        assert!(
            matches!(got, Ok((BlockState::Current, _))),
            "AGENTS.md 블록이 guide.rs 의 글에서 낡았다 — `moai init` 을 다시 부른다: {got:?}"
        );
    }

    /// 블록을 찾는 자([`blocks`])가 기대는 것 — 안내 글이 개행으로 끝나고, 마커나 충돌 표시로
    /// 읽힐 줄을 안 든다. 들면 다시 심을 때마다 블록이 제 글 중간에서 잘린다.
    #[test]
    fn the_guide_block_ends_with_a_newline_and_holds_no_marker_lines() {
        let guide = crate::guide::agents();
        assert!(guide.ends_with('\n'));
        for line in guide.lines() {
            assert!(!is_begin(line) && !is_end(line), "마커로 읽힐 줄 — {line}");
            assert!(!is_conflict(line, "<<<<<<<") && !is_conflict(line, ">>>>>>>"), "충돌로 읽힐 줄 — {line}");
        }
    }

    /// 다시 심은 것을 한 번 더 심어도 같고, 그것이 `current` 다 — 아니면 `status` 가 `init` 을 권하고
    /// 그 `init` 이 또 무언가를 바꾸는 고리가 선다.
    fn settle(what: &str, before: &str, block: &str) -> String {
        let once = with_block(before, block);
        assert_eq!(with_block(&once, block), once, "{what}: 다시 심었더니 바뀌었다");
        assert_eq!(block_state(&once, block), BlockState::Current, "{what}: 심은 뒤에도 current 가 아니다\n{once}");
        once
    }

    /// **산문이 마커를 적어 보인 자리는 블록이 아니다**(리뷰 moai-epb0.27f). 머리를 파일 어디서나
    /// 찾던 때는 거기부터 진짜 닫는 마커까지를 먹어, `stale — 블록 밖의 산문은 안 건드린다` 를
    /// 말한 뒤 따라 친 `init` 이 산문을 지웠다.
    #[test]
    fn prose_that_quotes_the_marker_is_not_the_block() {
        let block = "## 안내\n본문\n";
        let fresh = with_block("", block);
        for prose in [
            "아래 `<!-- moai:begin v:… -->` 부터는 moai 가 관리한다.\n",
            "`<!-- moai:begin -->` 과 `<!-- moai:end -->` 사이는 관리된다.\n",
            "<!-- moai:beginning-of-team-notes -->\n",
            "<!-- moai:begin~end 사이는 moai 가 관리 -->\n",
        ] {
            let text = format!("# 규약\n\n{prose}\n## 사람 메모\n지우면 안 되는 산문\n\n{fresh}");
            assert_eq!(block_state(&text, block), BlockState::Current, "{prose}");
            assert_eq!(settle(prose, &text.replace("본문", "고친 본문"), block), text, "{prose}");
        }
    }

    /// **닫는 줄을 잃은 여는 마커는 뒤의 산문을 먹지 않는다.** 가장 먼저 만난 여는 마커로 짝을
    /// 짓던 때는 `missing` → `init` 이 블록을 덧붙임 → `stale` → 둘째 `init` 이 그 사이 산문을
    /// 지웠다.
    #[test]
    fn an_orphaned_begin_marker_never_eats_the_prose_after_it() {
        let block = "## 안내\n본문\n";
        let text = "# 우리 규약\n\n<!-- moai:begin -->\n반쯤 지운 옛 블록\n\n## 사람 메모\n지우면 안 되는 산문\n";
        assert_eq!(block_state(text, block), BlockState::Missing);
        let once = settle("짝 잃은 마커", text, block);
        assert!(once.starts_with(text), "산문을 건드렸다 — {once}");
    }

    /// **겹친 블록은 하나로 접는다.** 새 마커를 못 알아보는 옛 바이너리는 `init` 마다 맨 마커
    /// 블록을 덧붙인다 — 첫 블록만 보던 때는 그 파일이 `current` 였고 `init` 도 그대로 뒀다.
    /// 사이에 선 산문은 둔다.
    #[test]
    fn blocks_appended_by_an_older_binary_fold_into_one() {
        let block = "## 안내\n본문\n";
        let fresh = with_block("", block);
        let old = "\n<!-- moai:begin -->\n## 옛 안내\n<!-- moai:end -->\n";
        let piled = format!("{fresh}{old}{old}{old}");
        assert_eq!(block_state(&piled, block), BlockState::Stale);
        assert_eq!(settle("겹친 블록", &piled, block), fresh);

        let tailed = format!("{fresh}\n## 사람 꼬리\n산문\n{old}");
        assert_eq!(settle("꼬리 뒤에 겹친 블록", &tailed, block), format!("{fresh}\n## 사람 꼬리\n산문\n"));
    }

    /// **낡음은 두 얼굴이다**(moai-mj45, 2026-09-15 사용자 결정). 마커가 제 글의 해시를 대면 어떤
    /// 바이너리가 쓴 그대로고(그쪽이 더 새것일 수 있다), 안 대면 사람이 블록 안을 고친 것이다.
    /// 가르지 않으면 아직 다시 빌드 안 한 세션이 `moai init` 을 따라 쳐 새 안내를 옛 글로 되돌린다.
    #[test]
    fn a_stale_block_says_whether_a_binary_or_a_person_wrote_it() {
        let ours = "## 안내\n새 글\n";
        let theirs = with_block("", "## 안내\n남이 쓴 글\n");
        assert_eq!(block_state(&theirs, ours), BlockState::Stale);
        assert_eq!(stale_kind(&theirs), Stale::Binary, "마커가 제 글의 해시를 대는데 손질로 읽었다");

        // 블록 안을 한 글자 고치면 마커의 해시가 그 글을 더는 안 댄다.
        let edited = theirs.replace("남이 쓴 글", "사람이 고친 글");
        assert_eq!(stale_kind(&edited), Stale::Edited);
        // 옛 맨 마커는 댈 해시가 없다 — `init` 이 갈아 끼우면 그 안의 글은 사라진다.
        assert_eq!(stale_kind("<!-- moai:begin -->\n옛 글\n<!-- moai:end -->\n"), Stale::Edited);
        // CRLF 로 체크아웃한 파일도 같은 자로 잰다 — 해시는 LF 글 위에서 났다.
        assert_eq!(stale_kind(&theirs.replace('\n', "\r\n")), Stale::Binary);
    }

    /// **글이 같으면 버전만 다른 마커는 `current` 이고 그대로 둔다** — 낡음을 가르는 것은 글이다.
    /// 해시가 다른 글을 대거나(손으로 고친 마커), 해시가 없거나(옛 맨 마커), 글을 고쳤으면 `stale` 이다.
    #[test]
    fn only_the_text_decides_staleness_not_the_version() {
        let block = "## 안내\n본문\n";
        let fresh = with_block("", block);
        let older = fresh.replacen(&format!("v:{}", env!("CARGO_PKG_VERSION")), "v:0.0.1", 1);
        assert_ne!(older, fresh);
        assert_eq!(block_state(&older, block), BlockState::Current);
        assert_eq!(with_block(&older, block), older, "버전만 다른 마커를 고쳐 썼다");

        for (what, text) in [
            ("해시가 다른 글을 댄다", fresh.replacen(&hash_of(block), "hash:deadbeef", 1)),
            ("옛 맨 마커", format!("{BEGIN} -->\n{block}{END}\n")),
            ("글을 고쳤다", older.replace("본문", "고친 본문")),
        ] {
            assert_eq!(block_state(&text, block), BlockState::Stale, "{what}");
            assert_eq!(settle(what, &text, block), fresh, "{what}");
        }
    }

    /// **마커 줄에 난 머지 충돌은 다시 심으면 풀린다**(리뷰 moai-epb0.27f). 해시가 첫 줄에 실려
    /// 두 가지가 저마다 안내를 고치면 거기서 충돌한다 — 첫 여는 마커부터 갈아 끼우던 때는
    /// `<<<<<<< HEAD` 가 블록 위에 남은 채 `current` 로 읽혀 그대로 커밋됐다.
    #[test]
    fn a_merge_conflict_on_the_marker_is_healed_by_replanting() {
        let block = "## 안내\n본문\n";
        let fresh = with_block("", block);
        let (ours, theirs) = (begin_marker("우리 글\n"), begin_marker("남의 글\n"));
        for (what, text) in [
            (
                "마커 줄만",
                format!("# 산문\n\n<<<<<<< HEAD\n{ours}\n=======\n{theirs}\n>>>>>>> b\n{block}{END}\n꼬리\n"),
            ),
            (
                "마커와 첫 줄",
                format!(
                    "# 산문\n\n<<<<<<< HEAD\n{ours}\n## 우리\n=======\n{theirs}\n## 남\n>>>>>>> b\n본문\n{END}\n꼬리\n"
                ),
            ),
            (
                "diff3",
                format!(
                    "# 산문\n\n<<<<<<< HEAD\n{ours}\n||||||| base\n{}\n=======\n{theirs}\n>>>>>>> b\n{block}{END}\n꼬리\n",
                    begin_marker(block)
                ),
            ),
        ] {
            assert_eq!(block_state(&text, block), BlockState::Stale, "{what}");
            assert_eq!(settle(what, &text, block), format!("# 산문\n\n{fresh}꼬리\n"), "{what}");
        }
    }

    /// **줄 끝과 BOM 은 파일의 것을 따른다.** CRLF 로 체크아웃한 블록은 글이 같으면 `current` 고,
    /// LF 로 갈아 끼우던 때는 늘 `stale` 에 체크아웃·`init` 을 돌 때마다 빈 줄이 하나씩 붙었다.
    #[test]
    fn line_endings_and_a_bom_are_the_files_own() {
        let block = "## 안내\n본문\n";
        let fresh = with_block("", block);
        let crlf = format!("# 산문\r\n\r\n{}꼬리\r\n", fresh.replace('\n', "\r\n"));
        assert_eq!(block_state(&crlf, block), BlockState::Current);
        assert_eq!(settle("CRLF 고친 글", &crlf.replace("본문", "고친 본문"), block), crlf);

        let appended = settle("CRLF 에 덧붙임", "# 산문\r\n", block);
        assert!(!appended.replace("\r\n", "").contains('\n'), "LF 가 섞였다 — {appended:?}");

        let bom = format!("\u{feff}{fresh}");
        assert_eq!(block_state(&bom, block), BlockState::Current);
        assert_eq!(settle("BOM", &bom.replace("본문", "고친 본문"), block), bom);
    }

    #[test]
    fn an_empty_file_gets_just_the_block() {
        let got = with_block("", "내용\n");
        assert_eq!(got, format!("{}\n내용\n{END}\n", begin_marker("내용\n")));
    }

    /// **마커는 이 바이너리의 버전과 블록의 해시를 든다**(moai-leyw). 옛 맨 마커
    /// (`<!-- moai:begin -->`)도 알아보고 갈아 끼운다 — 못 알아보면 이미 심긴 저장소마다
    /// 블록이 둘 선다. 다시 넣어도 바이트가 같다.
    #[test]
    fn the_marker_carries_version_and_hash_and_replaces_a_bare_one() {
        let fnv1a = |s: &str| crate::text::fnv1a32(s.as_bytes());
        let first = with_block("", "내용\n").lines().next().unwrap().to_string();
        assert_eq!(first, format!("<!-- moai:begin v:{} hash:{:08x} -->", env!("CARGO_PKG_VERSION"), fnv1a("내용\n")));

        let old = "# 산문\n\n<!-- moai:begin -->\n옛 내용\n<!-- moai:end -->\n꼬리\n";
        let new = with_block(old, "내용\n");
        assert_eq!(new, format!("# 산문\n\n{first}\n내용\n{END}\n꼬리\n"));
        assert_eq!(with_block(&new, "내용\n"), new, "다시 넣었더니 바뀌었다");

        let changed = with_block(&new, "새 내용\n");
        assert_eq!(changed.matches(BEGIN).count(), 1, "{changed}");
        assert!(changed.contains(&format!("hash:{:08x} -->\n새 내용\n", fnv1a("새 내용\n"))), "{changed}");
    }

    #[test]
    fn makes_a_prefix_from_a_directory_name() {
        for (dir, want) in [
            ("/w/argos", Some("argos")),
            ("/w/moa-issue", Some("moa-issue")),
            ("/w/My_Project 2", Some("my-project-2")),
            ("/w/___", None),
        ] {
            assert_eq!(prefix_from(Path::new(dir)).as_deref(), want, "{dir}");
        }
    }

    /// **긴 접두어는 하이픈을 빼 들어가면 그것, 아니면 머리글자, 낱말이 하나면 앞 8자로
    /// 줄인다**(moai-f7xs). 8자 이하는 그대로.
    #[test]
    fn a_long_prefix_is_shortened_to_initials_or_cut() {
        for (full, want) in [
            ("argos", "argos"),
            ("backend8", "backend8"),
            ("moa-issue", "moaissue"),
            ("my-company-backend", "mcb"),
            ("my-2nd-project-x", "m2px"),
            ("2024-plan-final-draft", "2pfd"),
            ("supercalifragilistic", "supercal"),
            ("a-b-c-d-e-f-g-h-i-j", "abcdefgh"),
        ] {
            let got = shorten(full);
            assert_eq!(got, want, "{full}");
            assert!(got.chars().count() <= PREFIX_MAX, "{full} → {got}");
            // 줄인 것도 설정이 받는 접두어다.
            crate::config::Config::parse(&format!("prefix = \"{got}\"\n"))
                .unwrap_or_else(|e| panic!("{full} → {got}: {e:?}"));
        }
    }

    /// **밖을 가리키는 AGENTS.md 는 안 읽는다**(moai-x0o7) — 커밋된 `AGENTS.md -> /dev/zero` 가 `moai status`
    /// 에서 메모리를 다 쓰던 자리다. 끝없는 파일로는 재지 않는다: 고침이 없으면 시험이 메모리를 다 쓴다. 밖의
    /// 보통 파일에 **낡은 블록**을 담아 둔다 — 고침이 없으면 그것이 읽혀 `stale` 로 서고, 보드에는 남의 파일의
    /// 낡은 블록을 고치라는 알림이 선다(리뷰 moai-x0o7.52k — 지금의 블록을 담던 판은 고침이 없어도 알림이 안 서서
    /// 둘째 단언이 아무것도 못 쟀다). FIFO 앞에서 안 멈추는 것은 바이너리로 잰다(`tests/cli.rs` 의
    /// `a_fifo_in_place_of_agents_md_or_a_dotfile_stops_nothing`).
    #[cfg(unix)]
    #[test]
    fn an_agents_md_that_points_outside_is_not_read() {
        let s = crate::scratch::Scratch::new("init-agents-away");
        let away = crate::scratch::Scratch::new("init-agents-away-file");
        let file = away.join("AGENTS.md");
        let stale = with_block("", "## an older block\n");
        assert_eq!(block_state(&stale, &crate::guide::agents()), BlockState::Stale, "시험의 전제 — 블록이 낡지 않았다");
        std::fs::write(&file, stale).unwrap();
        std::os::unix::fs::symlink(&file, s.join("AGENTS.md")).unwrap();
        let got = agents_state(s.path());
        assert!(
            matches!(got, Err(("AGENTS.md", Fell::Unheld(crate::held::Unheld::Outside { .. })))),
            "밖을 가리키는 AGENTS.md 를 읽었다: {got:?}"
        );
        assert_eq!(agents_notice(s.path(), false), None);
        let said = agents_unread(en(), s.path(), &got.unwrap_err().1);
        assert!(said.contains("AGENTS.md: ") && said.contains("outside"), "{said}");
    }

    /// **뿌리 파일의 임시 파일은 `.moai/` 에 선다**(moai-3akx). 옆자리에 서면 쓰다 죽은 `init` 이
    /// `AGENTS.md.tmp.…` 를 뿌리에 남기고 심는 `.gitignore` 는 그것을 안 덮는다. 그 자리에서 실제로
    /// 써 보고, 쓴 뒤 `.moai/` 에도 뿌리에도 찌꺼기가 없는지 본다.
    #[test]
    fn root_files_are_swapped_through_a_temp_file_in_dot_moai() {
        let s = crate::scratch::Scratch::new("init-tmp");
        let agents = s.join("AGENTS.md");
        assert_eq!(tmp_dir(&agents), s.path(), ".moai 가 없으면 옆자리다");
        std::fs::create_dir(s.join(".moai")).unwrap();
        assert_eq!(tmp_dir(&agents), s.join(".moai"));
        // **정말 `.moai/` 를 거치는지** 옆자리를 막아 두고 본다 — 옆에 쓰는 `plant` 도 성공하면 둘 다
        // 찌꺼기를 안 남겨 아래의 단언만으로는 못 가른다. 막은 자리는 디렉터리라 파일을 못 만든다.
        // 첫 이름이 막히면 다음 이름으로 가므로(moai-ydm7.976) **이름을 다** 막는다.
        let beside: Vec<_> = crate::store::tmp_names("AGENTS.md").map(|n| s.join(n)).collect();
        for b in &beside {
            std::fs::create_dir(b).unwrap();
        }
        let wrote = plant(&agents, "글\n");
        for b in &beside {
            std::fs::remove_dir(b).unwrap();
        }
        wrote.expect("`.moai/` 를 안 거치고 옆자리에 썼다");
        assert_eq!(std::fs::read_to_string(&agents).unwrap(), "글\n");
        let left = |d: &Path| std::fs::read_dir(d).unwrap().map(|e| e.unwrap().file_name()).collect::<Vec<_>>();
        assert_eq!(left(&s.join(".moai")), Vec::<std::ffi::OsString>::new());
        assert_eq!(left(s.path()).len(), 2, "뿌리에는 .moai 와 AGENTS.md 뿐이다: {:?}", left(s.path()));

        // **`.moai/` 에 못 쓰면 옆자리로 물러서 쓴다.** 읽기 전용 `.moai/` 에서 쓸 수 있는 AGENTS.md 를
        // "못 썼다" 고 하며 그 파일을 고치라고 하던 자리다. 권한을 되돌린 뒤에 재야 `Drop` 이 치운다.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let moai = s.join(".moai");
            std::fs::set_permissions(&moai, std::fs::Permissions::from_mode(0o555)).unwrap();
            let wrote = plant(&agents, "둘째\n");
            std::fs::set_permissions(&moai, std::fs::Permissions::from_mode(0o755)).unwrap();
            wrote.unwrap();
            assert_eq!(std::fs::read_to_string(&agents).unwrap(), "둘째\n");
            assert_eq!(left(&moai), Vec::<std::ffi::OsString>::new());
            assert_eq!(left(s.path()).len(), 2, "옆자리로 물러선 임시 파일이 남았다: {:?}", left(s.path()));
        }
    }

    /// **`check-ignore -v` 의 한 줄을 그 뜻대로 읽는다**(리뷰 moai-zynt.63u) — 되살리는 패턴(`!…`)은 무시가 아니고,
    /// 하위 디렉터리의 `.gitignore` 는 꼭대기에서 잰 상대 경로로, 이상한 글자가 든 경로는 따옴표로 싸여 온다.
    #[test]
    fn a_check_ignore_line_reads_as_the_tracking_it_means() {
        let cases = [
            (".gitignore:3:/.moai/\t.moai/\n", Tracking::Gitignore),
            ("svc/.gitignore:1:/.moai/\t.moai/\n", Tracking::Gitignore),
            ("\"\\355\\225\\234/.gitignore\":1:/.moai/\t.moai/\n", Tracking::Gitignore),
            (".gitignore:2:!*/\t.moai/\n", Tracking::Commit),
            (".git/info/exclude:7:/.moai/\t.moai/\n", Tracking::Exclude),
            ("/home/u/.config/git/ignore:1:.moai/\t.moai/\n", Tracking::Exclude),
            ("/home/u/.gitignore:1:.moai/\t.moai/\n", Tracking::Exclude),
            ("", Tracking::Commit),
        ];
        for (said, want) in cases {
            assert_eq!(ignored_by(said), want, "{said:?}");
        }
        // 자리에 `:` 가 서도 줄 번호 칸으로 가른다.
        assert_eq!(rule_of("C:/u/ignore:4:a:b\t.moai/"), Some(("C:/u/ignore", "a:b")));
    }

    /// **`.git/info/exclude` 의 줄은 작업 트리 꼭대기가 뿌리다**(리뷰 moai-zynt.63u) — 하위 디렉터리의 트래커는 그
    /// 길을 머리에 단다. 패턴 글자가 든 길은 막고, 어느 깊이에나 걸리던 줄은 그 넓이를 지킨다.
    #[test]
    fn exclude_lines_are_anchored_under_the_subdirectory() {
        assert_eq!(anchored("/.moai/", "svc/"), "/svc/.moai/");
        assert_eq!(anchored(".moai/lock", "svc/"), "/svc/.moai/lock");
        assert_eq!(anchored("# moai", "svc/"), "# moai");
        assert_eq!(anchored("target", "svc/"), "/svc/**/target");
        assert_eq!(anchored("!/.moai/keep", "svc/"), "!/svc/.moai/keep");
        assert_eq!(anchored("/.moai/", "a[1]/"), "/a\\[1]/.moai/");
        assert_eq!(anchored("/.moai/", ""), "/.moai/");
    }

    /// **다른 바이너리가 쓴 안내 전문은 손질로 안 읽는다**(리뷰 moai-zynt.63u) — 링크가 전문의 해시를 들어, 다른
    /// 전문을 쓴 바이너리의 링크는 제 마커의 해시를 대는 낡은 블록이다(`Stale::Binary`). 이 바이너리의 링크 밑에서
    /// 파일만 다르면 손질이고, 줄 끝만 다른 전문은 같은 글이다.
    #[test]
    fn a_guide_another_binary_wrote_is_not_read_as_a_hand_edit() {
        let s = crate::scratch::Scratch::new("init-guide-skew");
        std::fs::create_dir_all(s.join(".moai")).unwrap();
        let ours = format!("hash:{:08x}", crate::text::fnv1a32(crate::guide::agents().as_bytes()));
        let theirs = crate::guide::agents_link().replace(&ours, "hash:00000000");
        assert_ne!(theirs, crate::guide::agents_link(), "시험의 전제 — 링크가 전문의 해시를 안 든다");
        std::fs::write(s.join("AGENTS.md"), with_block("", &theirs)).unwrap();
        std::fs::write(s.join(crate::guide::GUIDE_FILE), "an older guide\n").unwrap();
        let kind = |root: &Path| agents_notice(root, false).map(|w| w.kind);
        assert_eq!(kind(s.path()), Some("agents_stale"), "다른 바이너리가 쓴 것을 손질로 댔다");
        std::fs::write(s.join("AGENTS.md"), with_block("", &crate::guide::agents_link())).unwrap();
        assert_eq!(kind(s.path()), Some("agents_hand_edited"));
        std::fs::write(s.join(crate::guide::GUIDE_FILE), crate::guide::agents().replace('\n', "\r\n")).unwrap();
        assert_eq!(kind(s.path()), None, "CRLF 체크아웃을 낡았다고 했다");
    }

    /// **링크 모드는 링크의 끝줄로 알아본다**(리뷰 moai-zynt.63u) — 블록이 `.moai/guide.md` 라는 글을 품는다고 링크가
    /// 아니다. 그렇게 가르던 판은 그 경로를 적은 손질 한 줄에 블록 모드 저장소를 링크로 갈아 끼웠다.
    #[test]
    fn a_block_that_names_the_guide_file_is_still_a_block() {
        let noted = with_block("", &format!("{}see {}\n", crate::guide::agents(), crate::guide::GUIDE_FILE));
        assert!(!links_to_guide(&noted));
        assert!(links_to_guide(&with_block("", &crate::guide::agents_link())));
        assert!(!crate::guide::agents().contains(crate::guide::GUIDE_MARK), "전문이 링크의 표식을 품었다");
    }
}
