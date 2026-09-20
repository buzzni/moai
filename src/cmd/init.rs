//! `.moai/` 를 심는다. `store` 말고 파일을 만드는 유일한 곳이다.

use super::{Ctx, Fail, R};
use crate::config::DEFAULT_STATUSES;
use std::path::{Path, PathBuf};

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

/// AGENTS.md 를 읽는다. **없는 파일만 `None` 이다** — 못 읽는 파일(권한·UTF-8 아님)을 빈 글로 치면
/// `init` 이 블록 하나로 덮어써 사람의 산문이 통째로 사라졌다. 보는 길([`agents_state`])과 쓰는
/// 길([`run`])이 이 하나로 읽는다.
fn read_agents(path: &Path) -> Result<Option<String>, String> {
    match std::fs::read_to_string(path) {
        Ok(t) => Ok(Some(t)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// 이 디렉터리의 AGENTS.md 를 읽어 [`block_state`] 로 가른다. 없는 파일은 `missing` 이고, 못 읽는
/// 파일(권한·UTF-8 아님)만 `Err` 다 — 그때는 상태를 지어내지 않는다.
pub fn agents_state(root: &Path) -> Result<BlockState, String> {
    let text = read_agents(&root.join("AGENTS.md"))?.unwrap_or_default();
    Ok(block_state(&text, &crate::guide::agents()))
}

/// 이 저장소가 심는 딸린 파일 — `(이름, 규칙 블록, 알림의 갈래)`. **갈래를 표에 함께 둔다** —
/// 빠졌을 때의 결과가 파일마다 달라 화면의 낱말이 갈리고(`view::says`), 기계도 `kind` 로 그것을
/// 가른다. `agents_stale`·`agents_hand_edited` 가 이미 그 자리다.
const DOTFILES: [(&str, &str, &str); 2] =
    [(".gitattributes", GITATTRIBUTES, "gitattributes_rules"), (".gitignore", GITIGNORE, "gitignore_rules")];

/// 이 저장소의 딸린 파일에서 **빠진 규칙** — `(파일 이름, 알림의 갈래, 빠진 줄들)`, 빠진 것이
/// 있는 파일만.
///
/// **못 읽는 파일은 여기서 말하지 않는다.** 무엇이 들었는지 모르니 빠졌다고 할 수 없고 — 그 줄이
/// 이미 있을 수도 있다 — `init` 도 그 파일은 안 건드리므로 한 번 선 알림이 **영영 안 걷힌다.**
/// 그 자리는 `init` 이 제 이름으로 이미 말한다.
///
/// **없는 파일은 통째로 빠진 것이다.** `git add -A` 가 옆 워크트리를 담는 위험이 가장 큰 자리라
/// (moai-mxtb) 입을 다물면 안 된다. 갈림은 **오류의 갈래로** 짓는다 — `exists()` 로 물으면 못 읽는
/// 파일과 없는 파일이 정확히 거꾸로 선다.
pub fn dotfile_gaps(root: &Path) -> Vec<(&'static str, &'static str, Vec<&'static str>)> {
    DOTFILES
        .into_iter()
        .filter_map(|(name, block, kind)| {
            let text = match std::fs::read_to_string(root.join(name)) {
                Ok(t) => t,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
                Err(_) => return None,
            };
            let missing = missing_rules(&text, block);
            (!missing.is_empty()).then_some((name, kind, missing))
        })
        .collect()
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
pub fn dotfile_notice(root: &Path, chdir: bool) -> Vec<crate::report::Warning> {
    let gaps = dotfile_gaps(root);
    if gaps.is_empty() {
        return Vec::new();
    }
    let away = away_root(root, chdir);
    gaps.into_iter()
        .map(|(_, kind, missing)| crate::report::Warning::dotfile_rules(kind, &missing, away.as_deref()))
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
    if agents_state(root) != Ok(BlockState::Stale) {
        return None;
    }
    // **어느 쪽 낡음인지까지 말한다**(2026-09-15 사용자 결정). 한 낱말로 뭉뚱그려 `moai init` 만
    // 대면, 아직 다시 빌드 안 한 바이너리를 든 세션이 그 말을 따라 새 안내를 옛 글로 되돌린다.
    let text = read_agents(&root.join("AGENTS.md")).ok().flatten().unwrap_or_default();
    Some(crate::report::Warning::agents_stale(away_root(root, chdir).as_deref(), stale_kind(&text) == Stale::Edited))
}

/// `moai init --check`. **아무것도 안 쓰고, 파일을 못 읽을 때만 0 이 아니다**(2026-09-14 사용자
/// 결정) — 낡음으로 비영 종료하면 에이전트가 실패로 읽고, 그러면 게이트다. `.moai` 가 없어도
/// 선다: 보는 것은 AGENTS.md 하나고, 심기 전에 부르는 것도 자연스럽다.
pub fn check(ctx: &Ctx) -> R<Vec<String>> {
    let root = std::env::current_dir().map_err(|e| Fail::new(e.to_string()))?;
    let state = agents_state(&root).map_err(Fail::new)?;
    // 빠진 딸린 파일 규칙도 같은 자리에서 본다(moai-2f99) — `--check` 는 "무엇이 낡았나" 를 묻는
    // 자리고, 블록만이 아니라 딸린 파일도 `init` 이 맞추는 것이다.
    let gaps = dotfile_gaps(&root);
    if ctx.json {
        let mut v = serde_json::json!({ "agents": state });
        if !gaps.is_empty() {
            v["missing"] = serde_json::json!(
                gaps.iter().map(|(name, _, missing)| (*name, missing)).collect::<std::collections::BTreeMap<_, _>>()
            );
        }
        return super::json_line(&v);
    }
    let mut out = vec![match state {
        BlockState::Current => "AGENTS.md 블록: current — 이 바이너리가 쓸 글과 같다".into(),
        BlockState::Stale => {
            let text = read_agents(&root.join("AGENTS.md")).map_err(Fail::new)?.unwrap_or_default();
            match stale_kind(&text) {
                Stale::Binary =>
                    "AGENTS.md 블록: stale — 다른 바이너리가 쓴 그대로다. 그쪽이 더 새것일 수 있으니 다시 빌드해 보고 `moai init`"
                        .into(),
                Stale::Edited =>
                    "AGENTS.md 블록: stale — 블록 안을 손으로 고쳤다. `moai init` 은 그 손질을 버린다 (블록 밖의 산문은 안 건드린다)"
                        .into(),
            }
        }
        BlockState::Missing => {
            "AGENTS.md 블록: missing — `moai init` 이 심는다. `--no-agents` 로 안 쓰기로 했으면 그대로 둔다".into()
        }
    }];
    // **규칙끼리는 쉼표로 가른다** — `.gitattributes` 의 규칙은 제 안에 띄어쓰기를 여럿 들어
    // (`.moai/journal.jsonl  text eol=lf merge=union`) 띄어쓰기로 이으면 어디서 한 줄이 끝나는지
    // 안 보인다.
    for (name, _, missing) in &gaps {
        out.push(format!("{name}: 규칙 {}개 빠졌다 ({}) — `moai init`", missing.len(), missing.join(", ")));
    }
    Ok(out)
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

// **여기 글을 고치면 이미 심은 저장소가 주석을 둘 든다.** `ensure_lines` 는 줄 단위로 견주어
// 없는 줄을 덧붙이므로, 주석 한 줄만 고쳐도 다음 `moai init` 이 옛 주석 밑에 새 주석을 붙인다
// (리뷰 moai-vbmn.spv 에서 실제로 났다). 그래서 이 블록의 글은 규칙이 바뀔 때만 손댄다 —
// 규칙 없이 글만 고치고 싶으면 그 전에 덧붙임을 막는 길부터 낸다(idea).
const GITATTRIBUTES: &str = "\
# moai — 이슈 트래커
# 스냅샷에는 merge=union 을 쓰지 않는다. 두 브랜치가 같은 이슈를 고치면
# union 이 같은 id 를 가진 줄 두 개를 조용히 남기고, 그건 데이터 손상이다.
# merge=moai 는 줄을 id 로 짝지어 이슈마다 3-way 로 푼다. 같은 필드를 둘이
# 다르게 고친 진짜 충돌은 그대로 사람에게 온다 — 한 줄이 이슈 하나라 실제로 쉽다.
# 드라이버는 클론마다 `moai merge-driver --install` 로 심는다. 안 심은 클론에서는
# 이 낱말이 무시되고 git 의 기본 머지가 돈다.
.moai/issues.jsonl   text eol=lf merge=moai
# 저널은 추가 전용이고 순서가 무관하며 상태 계산에 읽히지 않는다. 여기선 맞다.
.moai/journal.jsonl  text eol=lf merge=union
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
    /// 빠진 줄을 덧붙였다.
    Wrote,
    /// 이미 다 있었다 — 파일은 안 건드렸다.
    Already,
    /// 못 읽어서 안 건드렸다. 안에 든 것은 그 까닭이다.
    Unreadable(String),
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
            Added::Wrote | Added::Already => None,
            Added::Unreadable(why) => Some(("unreadable", why)),
            Added::Unwritable { why, .. } => Some(("unwritable", why)),
        }
    }

    /// 사람이 손으로 더할 줄. 못 쓴 자리는 **못 넣은 줄만** 안다(읽기는 됐다). 못 읽은 자리는
    /// 파일을 못 봤으니 블록을 통째로 낸다 — 주석과 빈 줄은 빼고, 규칙 줄만.
    fn hand<'a>(&'a self, block: &'a str) -> Vec<&'a str> {
        let rule = |l: &&str| !l.trim().is_empty() && !l.trim_start().starts_with('#');
        match self {
            Added::Unwritable { missing, .. } => missing.iter().map(String::as_str).filter(rule).collect(),
            Added::Wrote | Added::Already | Added::Unreadable(_) => block.lines().filter(rule).collect(),
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
fn ensure_lines(path: &Path, block: &str) -> Added {
    use std::io::Write as _;
    let existing = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Added::Unreadable(e.to_string()),
    };
    let missing = missing_lines(&existing, block);
    if missing.is_empty() {
        return Added::Already;
    }
    let mut tail = String::new();
    if !existing.is_empty() && !existing.ends_with('\n') {
        tail.push('\n');
    }
    if !existing.is_empty() {
        tail.push('\n');
    }
    tail.push_str(&missing.join("\n"));
    tail.push('\n');
    let wrote =
        std::fs::OpenOptions::new().create(true).append(true).open(path).and_then(|mut f| f.write_all(tail.as_bytes()));
    match wrote {
        Ok(()) => Added::Wrote,
        Err(e) => Added::Unwritable { why: e.to_string(), missing: missing.iter().map(|l| (*l).to_string()).collect() },
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
/// **쓰는 길([`ensure_lines`])과 비추는 길([`dotfile_gaps`])이 한 자에서 갈린다.** 따로 재면
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
    match crate::store::write_atomic_in(path, text.as_bytes(), &first) {
        // `.moai/` 에서 못 갈아 끼웠으면 옆자리로 한 번 더 — 까닭은 [`tmp_dir`] 에 적었다. 실패한
        // 쪽은 임시 파일을 치우고 대상을 안 건드리므로 다시 써도 잃을 것이 없다.
        Err(_) if path.parent() != Some(first.as_path()) => {
            crate::store::write_atomic(path, text.as_bytes()).map_err(|e| e.message)
        }
        done => done.map_err(|e| e.message),
    }
}

/// 뿌리 파일을 갈아 끼울 임시 파일의 자리 — **`.moai/`** 다(moai-3akx, 2026-09-18 사용자 결정).
///
/// 옆자리에 두면 쓰다 죽은 `init` 이 `AGENTS.md.tmp.<pid>` 를 저장소 뿌리에 남기고, 심는
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
    // 끝 `/` 는 "디렉터리만" 이라는 뜻이다. 같은 이름끼리 견줄 때 `have` 만 디렉터리 전용이면
    // (`.moai/lock/`) 파일 `.moai/lock` 을 막지 못하니 덮은 것으로 치지 않는다.
    let dir_only = have.ends_with('/') && !want.ends_with('/');
    let bare = |s: &str| s.trim_start_matches('/').trim_end_matches('/').to_string();
    let (have, want) = (bare(have), bare(want));
    !have.is_empty() && ((have == want && !dir_only) || want.starts_with(&format!("{have}/")))
}

/// 이 자리에 트래커를 세우면 무엇이 어긋나는가(moai-pjrr·moai-mz0e) — 없으면 `None`.
///
/// 묻는 것은 **명령이 어느 트래커로 가는가** 하나인데, 자리마다 답이 달라 [`Elsewhere`] 로 가른다.
///
/// - **딸린 워크트리**([`Elsewhere::Worktree`]): 워크트리 안에서 친 `moai` 는 주 체크아웃의 트래커를
///   읽고 쓴다(moai-y7go). 여기 심은 `.moai` 는 **아무도 안 읽고**, 커밋되면 병합에서 겨룬다 —
///   그래서 거절한다
/// - **위에 트래커가 있는 하위 디렉터리**([`Elsewhere::Above`]): 여기 세우면 이 밑의 명령은 여기
///   것을 쓰고 옆 디렉터리는 위의 것을 쓴다. 심은 것이 **읽히기는 한다** — 그래서 알리기만 한다
///
/// **둘을 가른 것은 값이 다르기 때문이다**(2026-09-20 사용자 결정 둘째 판). 위로 찾기에는 경계를
/// 그을 자가 없다 — 천장을 두었다가 걷은 것이 같은 날의 moai-a2kn 이고, 그 결정은 "`여기서
/// moai init` 은 트래커를 하나 더 세운다" 를 **살아 있는 길**로 적었다. `~/.moai` 를 둔 사람의 새
/// 프로젝트마다 거절을 세우면 그 길이 막힌다. 워크트리는 다르다: 거기 심은 것은 어느 명령도 안 읽어,
/// 알림 한 줄로 두면 사람이 그것을 모른 채 커밋한다.
///
/// `MOAI_HERE` 는 둘 다 끈다 — 그 워크트리에서만 쓰는 트래커를 일부러 두는 길이다.
///
/// **위로 찾는 자는 `.moai` 가 디렉터리인가로 가른다** — [`crate::store`] 의 위로 찾기와 같은 자다.
/// `config.toml` 까지 봐야 트래커라고 세는 자리도 있지만([`crate::worktree::tracker_root`]), 여기서
/// 물어야 하는 것은 "명령이 어디로 가는가" 라 그쪽 자를 쓰면 설정이 빠진 `.moai` 위에서 둘이 갈린다.
///
/// **찾은 자리에서 한 번 더 옮김을 묻는다**(리뷰) — `.moai` 를 가진 조상을 찾았다고 거기가 끝이
/// 아니다. [`crate::store::Repo::find_from`] 은 그 자리에서 [`crate::worktree::tracker_root`] 로 한
/// 번 더 옮겨 가므로(moai-y7go), 묻지 않으면 도구가 **제가 안 읽는 트래커**를 댄다.
fn planted_elsewhere(root: &Path) -> Option<Elsewhere> {
    // **`MOAI_HERE` 가 이 물음을 통째로 끈다.** 아래 [`crate::store::Repo::opened_root`] 도 같은
    // 손잡이를 거치지만(`Repo::redirect`), 여기 한 줄로 세워야 두 갈래가 한 자로 꺼진다 — 그쪽에
    // 맡기던 판은 켠 것이 어느 갈래를 끄는지가 두 모듈을 오가야 보였다.
    if crate::store::here_wanted() {
        return None;
    }
    // **워크트리를 먼저 묻는다.** 워크트리의 루트는 조상이기도 해 아래 자가 같은 자리를 대는데,
    // 그때 대야 할 말은 "위에 있다" 가 아니라 "여기는 워크트리다" 다.
    let main = crate::store::Repo::opened_root(root);
    if main != root {
        return Some(Elsewhere::Worktree(main));
    }
    let mut at = root.to_path_buf();
    while at.pop() {
        if at.join(".moai").is_dir() {
            // **여기가 두 갈래를 가른다**(리뷰). 워크트리의 **밑자리**에서는 위의 물음이 안 선다 —
            // [`crate::worktree::main_root`] 는 밑길을 주 체크아웃에 그대로 비추므로
            // (`<wt>/src` → `<main>/src`), 거기 트래커가 없으면 "워크트리다" 가 아니라고 답한다.
            // 그대로 두던 판은 워크트리의 `.moai` 를 대며 `moai -C <워크트리> init` 을 시켰는데,
            // 그 자리의 명령은 모두 루트의 트래커를 쓰고 그 줄을 따라 친 사람은 병합에서 겨룰
            // 파일을 고쳤다.
            let main = crate::store::Repo::opened_root(&at);
            if main != at {
                return Some(Elsewhere::Worktree(main));
            }
            return Some(Elsewhere::Above(at));
        }
    }
    None
}

/// [`planted_elsewhere`] 가 찾은 자리 — **자리마다 값이 다르다.**
enum Elsewhere {
    /// 딸린 워크트리의 주 체크아웃. 여기 심은 트래커는 아무도 안 읽어 **거절한다.**
    Worktree(PathBuf),
    /// 위에서 찾은 트래커의 뿌리. 여기 세운 것도 읽히므로 **알리기만 한다.**
    Above(PathBuf),
}

pub fn run(ctx: &Ctx, prefix: Option<&str>, no_agents: bool) -> R<Vec<String>> {
    let root = std::env::current_dir().map_err(|e| Fail::new(e.to_string()))?;
    let dir = root.join(".moai");
    // **세우기 전에 한 번 묻는다**(moai-pjrr·moai-mz0e). 이미 여기 심겨 있으면 안 묻는다 — 그때 이
    // 명령이 하는 일은 딸린 파일을 다시 맞추는 것뿐이라 새 트래커가 서지 않는다.
    let elsewhere = if dir.exists() { None } else { planted_elsewhere(&root) };
    // **거절하는 자리는 워크트리 하나다.** 위에서 찾은 것은 세우고 아래에서 알린다 — 가른 까닭은
    // [`planted_elsewhere`] 에 있다.
    if let Some(Elsewhere::Worktree(main)) = &elsewhere {
        let there = main.clone();
        let why =
            format!("여기는 딸린 워크트리다 — 트래커는 주 체크아웃에 산다\n      {}", there.join(".moai").display());
        // **친 대로 도로 낸다**(리뷰). 접두어를 빠뜨린 줄을 그대로 베끼면 디렉터리 이름에서 만든
        // 접두어가 서는데, 그것은 아래 갈래가 말하듯 **나중에 못 바꾼다** — 따라 친 한 줄이 그 저장소의
        // 모든 id 에 남는다. `--no-agents` 도 일부러 준 것이라 빼면 안 준 사람의 `AGENTS.md` 를 고친다.
        //
        // **`-C` 도 되살린다**(리뷰 둘째 판) — `-C` 는 [`crate::main`] 이 `set_current_dir` 로 따르므로
        // 여기의 "여기" 는 **`-C` 가 가리킨 자리**고 사람의 셸은 딴 데 있다. 빠뜨린 줄을 그대로 베끼면
        // 그 셸 자리에 트래커가 하나 더 선다 — 나머지를 친 대로 되살린 줄일수록 더 그대로 베낀다.
        // 재는 자는 [`away_root`] 하나다: 알림마다 따로 재면 한 화면의 두 줄이 다른 자리를 댄다.
        let at = away_root(&root, ctx.chdir).map(|r| format!(" -C {r}")).unwrap_or_default();
        let same = match (prefix, no_agents) {
            (Some(p), true) => format!(" {} --no-agents", crate::text::quoted(p)),
            (Some(p), false) => format!(" {}", crate::text::quoted(p)),
            (None, true) => " --no-agents".to_string(),
            (None, false) => String::new(),
        };
        // **빠져나가는 길의 값도 함께 댄다**(리뷰). `MOAI_HERE` 는 **이 프로세스 하나**에만 선다 —
        // 그것으로 세운 트래커는 그 뒤의 맨 `moai` 가 도로 루트의 것을 읽어 아무도 안 읽는다.
        // 대지 않으면 이 줄이 거절문이 막으려던 바로 그 자리로 사람을 데려간다.
        return Err(Fail::coded(
            format!(
                "{why}
      거기를 맞추려면  moai -C {} init
      정말 여기 세우려면  MOAI_HERE=1 moai{at} init{same}
        그 트래커는 MOAI_HERE=1 을 준 명령만 읽는다 — 맨 moai 는 위의 것을 읽는다",
                // **경로는 감싸서 낸다**(`crate::text::shell_word`, moai-0cl3) — 붙여 넣으면 도는
                // 글자여야 한다. 한눈 보기가 같은 `moai -C … init` 을 내는 자리도 같은 자다
                // (`view::unopened`·`cmd::project`). 빈칸 하나가 `-C` 를 딴 자리로 보낸다.
                crate::text::shell_word(&there.display().to_string())
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
    // 디렉터리 이름이 길어 줄였으면 그 원래 모양 — 무엇에서 줄였는지 말하려고 든다.
    let mut shortened: Option<String> = None;
    let prefix = match (prefix, again) {
        // 접두어는 나중에 못 바꾼다. 이미 발급된 id 가 전부 그것을 달고 있고,
        // 바꾸면 그 줄들이 제 접두어를 잃는다.
        (Some(p), true) => {
            let cur = crate::config::Config::load(&root).map_err(Fail::new)?.prefix;
            if p != cur {
                return Err(Fail::coded(
                    format!(
                        "접두어는 `{cur}` 로 이미 정해졌다. 나중에 못 바꾼다 —\n      \
                         이미 발급된 id 가 전부 그것을 달고 있다"
                    ),
                    super::code::ALREADY_EXISTS,
                ));
            }
            cur
        }
        // **사람이 준 긴 접두어는 거절한다** — 쓰기는 엄하게. `init` 은 한 번 부르는 명령이라
        // 다시 부르는 비용이 작고, 거절문이 짧은 후보를 댄다. 이미 심긴 저장소의 긴 접두어는
        // 위 갈래가 그대로 받는다.
        // **모양을 먼저 본다** — 길이를 먼저 보면 `MyCompanyBackend` 에 그 자체로 틀린
        // `MyCompan` 을 후보로 대, 따라 친 쪽이 둘째 오류를 만났다(리뷰 moai-f7xs.z1x).
        // 후보는 명령줄이 아니라 접두어만 댄다 — `-C`·`--no-agents` 를 줬던 명령을 다시 짜서
        // 대면 붙여 넣은 자리에 엉뚱하게 심는다.
        (Some(p), false) => {
            crate::config::check_prefix(p).map_err(Fail::new)?;
            if p.chars().count() > PREFIX_MAX {
                return Err(Fail::coded(
                    format!(
                        "접두어는 {PREFIX_MAX}자까지다 — `{p}` 는 {}자다. id 를 칠 때마다 붙는다\n      \
                         짧은 후보: `{}`",
                        p.chars().count(),
                        shorten(p)
                    ),
                    super::code::BAD_INPUT,
                ));
            }
            p.to_string()
        }
        (None, true) => crate::config::Config::load(&root).map_err(Fail::new)?.prefix,
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

    let config = format!(
        "# moai — {}\nprefix = \"{prefix}\"\nstatuses = \"{DEFAULT_STATUSES}\"\n\
         # 화면이 사람을 내는 모양: full(`이름 (메일)`) · name · email\nnaming = \"full\"\n",
        "이 저장소의 이슈 트래커 설정"
    );
    // 설정을 먼저 검사한다 — 접두어가 형식에 안 맞으면 파일을 만들기 전에 멈춘다.
    crate::config::Config::parse(&config).map_err(Fail::new)?;

    // AGENTS.md 도 **아무것도 심기 전에** 읽는다. 못 읽는 파일(UTF-8 아님·권한)을 빈 글로 치면
    // 블록 하나로 덮어써 사람의 산문이 통째로 사라졌다 — 멈추되, `.moai/` 를 만든 뒤에 멈추면
    // 반쯤 심긴 저장소가 남는다. `--check` 가 같은 파일에 같은 까닭을 댄다(`read_agents`).
    let agents_path = root.join("AGENTS.md");
    let agents_now = if no_agents {
        None
    } else {
        let read = read_agents(&agents_path).map_err(|e| {
            Fail::new(format!(
                "{e}\n      못 읽는 AGENTS.md 는 덮어쓰지 않는다 — 읽히게 고치거나 `--no-agents` 로 부른다"
            ))
        })?;
        Some(read.unwrap_or_default())
    };

    if !again {
        std::fs::create_dir_all(&dir).map_err(|e| Fail::new(format!("{}: {e}", dir.display())))?;
        for (name, body) in [("config.toml", config.as_str()), ("issues.jsonl", ""), ("journal.jsonl", "")] {
            let p = dir.join(name);
            std::fs::write(&p, body).map_err(|e| Fail::new(format!("{}: {e}", p.display())))?;
        }
    }

    let attrs = ensure_lines(&root.join(".gitattributes"), GITATTRIBUTES);
    let ignore = ensure_lines(&root.join(".gitignore"), GITIGNORE);
    // 못 건드린 자리 — 이름과 까닭과 **손으로 더할 줄**을 함께 든다(moai-gq1c, moai-0dwc). 줄을 안
    // 대면 사람은 도구가 무엇을 넣으려 했는지 모른 채 파일만 고치게 된다. 못 읽은 것과 못 쓴 것을
    // 가르는 것은 **말뿐이다** — 사람이 할 일이 인코딩과 권한으로 갈린다.
    let untouched: Vec<(&str, &Added, &str)> =
        [(".gitattributes", &attrs, GITATTRIBUTES), (".gitignore", &ignore, GITIGNORE)]
            .into_iter()
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
    let mut agents_trouble = None;
    let agents = match &agents_now {
        None => false,
        Some(existing) => {
            let next = with_block(existing, &crate::guide::agents());
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
    // 딸린 파일과 **한 자리에서 말한다** — 못 건드린 것은 이름·갈래·까닭으로 함께 선다. 블록은
    // 줄 몇 개가 아니라 통째로 갈아 끼우는 글이라 "손으로 더할 줄" 이 없다(빈 글을 넘긴다):
    // 사람이 할 일은 쓸 수 있게 고치고 다시 부르는 것뿐이고, 그 말은 [`hand`] 가 빈 자리에서 낸다.
    let untouched: Vec<(&str, &Added, &str)> =
        untouched.into_iter().chain(agents_trouble.iter().map(|t| ("AGENTS.md", t, ""))).collect();
    // **`agents` 가 아니라 "AGENTS.md 를 다뤘는가" 로 묻는다** — `agents` 는 이제 *쓴* 때만 참이라
    // (moai-knn0) 그것으로 물으면 블록이 이미 맞는 저장소에서는 이 안내가 영영 안 선다.
    let claude_needs_pointer = agents_now.is_some()
        && root.join("CLAUDE.md").exists()
        && !std::fs::read_to_string(root.join("CLAUDE.md")).unwrap_or_default().contains("AGENTS.md");

    if ctx.json {
        let mut v = serde_json::json!({
            "root": root.display().to_string(),
            "prefix": prefix,
            "created": !again,
            "gitattributes": attrs == Added::Wrote,
            "gitignore": ignore == Added::Wrote,
            "agents": agents,
        });
        // 줄였을 때만 싣는다 — 늘 `null` 을 두면 줄이지 않은 대부분의 줄이 헛 키를 든다.
        if let Some(full) = &shortened {
            v["shortened_from"] = serde_json::json!(full);
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

    let mut out = if again {
        vec![format!("이미 심겨 있다. 접두어는 `{prefix}` 다 — 딸린 파일만 다시 맞춘다")]
    } else {
        vec![
            format!(".moai/ 를 만들었다. 접두어는 `{prefix}` 다"),
            format!("  칸: {}", DEFAULT_STATUSES.replace(',', " → ")),
        ]
    };
    // **위에도 트래커가 있으면 세우고 나서 말한다**(moai-pjrr, 2026-09-20 사용자 결정 둘째 판).
    // 막지 않는 까닭은 [`planted_elsewhere`] 에 있다 — 여기 심은 것은 이 밑에서 실제로 읽힌다.
    // 그래도 말은 해야 한다: 이 줄이 없으면 `<프로젝트>/src/deep` 에 선 사람이 옆 디렉터리와 다른
    // 파일을 쓰기 시작한 것을 모른 채 "왜 내 이슈가 안 보이나" 를 딴 데서 찾는다.
    if let Some(Elsewhere::Above(at)) = &elsewhere {
        out.push(format!("  위에도 트래커가 있다 — {}", at.join(".moai").display()));
        out.push("    이 밑의 명령은 여기 것을 쓴다. 위의 것으로 모으려면 방금 만든 .moai/ 를 지운다".into());
    }
    // 접두어는 나중에 못 바꾸므로 **지금** 말한다 — 이슈를 하나라도 만들면 되돌릴 길이 없다.
    if let Some(full) = &shortened {
        out.insert(
            1,
            format!(
                "  디렉터리 이름 `{full}` 이 {PREFIX_MAX}자를 넘어 줄였다 — 다른 것을 원하면 이슈를 만들기 전에 \
                 .moai/ 를 지우고 `moai init <접두어>`"
            ),
        );
    }
    if attrs == Added::Wrote {
        out.push("  .gitattributes 에 병합 규칙을 넣었다".into());
    }
    if ignore == Added::Wrote {
        out.push("  .gitignore 에 moai 가 쓰는 자리(lock·tmp·워크트리)를 넣었다".into());
    }
    for (name, done, block) in &untouched {
        // 못 읽은 것과 못 쓴 것은 **사람이 할 일이 다르다** — 인코딩을 고칠 일과 권한을 열 일이다.
        // 갈래를 빠짐없이 적는다: `_` 로 받던 때는 갈래가 하나 느는 날 그것이 말없이 "못 읽어"
        // 로 서고 까닭 자리가 빈 채 나갔다(리뷰 moai-humk).
        let head = match done {
            Added::Unwritable { why, .. } => format!("  {name} 에 못 썼다 — {why}"),
            Added::Unreadable(why) => format!("  {name} 를 못 읽어 안 건드렸다 — {why}"),
            Added::Wrote | Added::Already => continue,
        };
        out.push(head);
        // **댈 줄이 없는 자리도 있다** — AGENTS.md 블록은 줄 몇 개가 아니라 통째로 갈아 끼우는
        // 글이라 손으로 옮겨 적을 것이 아니다. 그 자리는 상태를 보는 길을 대신 댄다(moai-780n).
        if done.hand(block).is_empty() {
            out.push("    쓸 수 있게 고치고 다시 부른다 — `moai init --check` 가 블록 상태를 말한다".into());
            continue;
        }
        out.push("    손으로 더할 줄 (고치고 `moai init` 을 다시 불러도 된다):".into());
        // **한 줄에 하나씩 낸다** — 쉼표로 이으면 붙여 넣은 것이 한 줄이 되어 규칙이 안 선다.
        // `.gitattributes` 는 더 나쁘다: `<패턴> text eol=lf, <패턴> …` 은 첫 패턴에 쓰레기
        // 속성을 달 뿐이라 `journal.jsonl` 이 `merge=union` 을 영영 못 받는다.
        // **못 쓴 자리는 빠진 줄만 낸다**([`Added::hand`]) — 파일을 읽었으니 이미 있는 줄을 안다.
        for line in done.hand(block) {
            out.push(format!("      {line}"));
        }
    }
    if agents {
        out.push("  AGENTS.md 블록을 맞췄다".into());
    }
    // **줄 수가 아니라 한 일로 묻는다.** 줄을 세던 때는 이 자리 위에 줄 하나를 더하는 것만으로
    // 이 안내가 말없이 사라졌다 — `moai-knn0` 전까지 `agents` 가 늘 참이라 실제로 그랬다.
    // `Already` 는 "다 있어서 안 건드렸다" 뿐이다 — 못 읽은 자리는 `Unreadable` 이라 여기서 걸린다.
    // **못 건드린 자리가 있으면 "다 맞아 있다" 가 아니다**(moai-780n) — 못 쓴 AGENTS.md 는 딸린
    // 파일이 둘 다 `Already` 여도 남은 일이다.
    let did_nothing = attrs == Added::Already && ignore == Added::Already && !agents && untouched.is_empty();
    if again && did_nothing {
        out.push("  이미 다 맞아 있다".into());
    }
    if claude_needs_pointer {
        out.push(String::new());
        out.push("CLAUDE.md 가 있다. 그 안에 `@AGENTS.md` 한 줄을 넣으면 같이 읽힌다".into());
    }
    if !again {
        out.push(String::new());
        out.push("다음:  moai add '첫 이슈'".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// **위에 트래커가 있으면 그 자리를 찾아 낸다**(moai-pjrr). 막지는 않는다 — 여기 심은 것은 이
    /// 밑에서 읽히고, 위로 찾기에는 경계를 그을 자가 없다(같은 날 moai-a2kn 이 천장을 걷었다).
    /// 부르는 쪽이 이 값으로 알림 한 줄을 세운다.
    ///
    /// **`.moai` 가 디렉터리인가로 가른다** — 위로 찾는 [`crate::store`] 와 같은 자다. 그 자가
    /// 갈리면 여기서 지나간 자리를 명령이 잡는다.
    #[test]
    fn a_subdir_under_a_tracker_finds_the_one_above() {
        let s = crate::scratch::Scratch::new("init-above");
        let deep = s.join("src/deep");
        std::fs::create_dir_all(&deep).unwrap();
        assert!(planted_elsewhere(&deep).is_none(), "트래커가 없는데 자리를 댔다");

        std::fs::create_dir_all(s.join(".moai")).unwrap();
        match planted_elsewhere(&deep) {
            Some(Elsewhere::Above(at)) => assert_eq!(at, s.path(), "댄 자리가 트래커의 자리가 아니다"),
            Some(Elsewhere::Worktree(main)) => panic!("워크트리가 아닌데 워크트리라 했다 — {}", main.display()),
            None => panic!("위의 트래커를 못 봤다"),
        }

        // 그 자리 자신은 안 묻는다 — 여기 이미 심겨 있으면 `run` 이 딸린 파일만 다시 맞춘다.
        assert!(planted_elsewhere(s.path()).is_none(), "제 트래커를 남의 것으로 댔다");
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
        assert_eq!(
            agents_state(Path::new(env!("CARGO_MANIFEST_DIR"))),
            Ok(BlockState::Current),
            "AGENTS.md 블록이 guide.rs 의 글에서 낡았다 — `moai init` 을 다시 부른다"
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
                .unwrap_or_else(|e| panic!("{full} → {got}: {e}"));
        }
    }

    /// **뿌리 파일의 임시 파일은 `.moai/` 에 선다**(moai-3akx). 옆자리에 서면 쓰다 죽은 `init` 이
    /// `AGENTS.md.tmp.<pid>` 를 뿌리에 남기고 심는 `.gitignore` 는 그것을 안 덮는다. 그 자리에서 실제로
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
        let beside = s.join(format!("AGENTS.md.tmp.{}", std::process::id()));
        std::fs::create_dir(&beside).unwrap();
        let wrote = plant(&agents, "글\n");
        std::fs::remove_dir(&beside).unwrap();
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
}
