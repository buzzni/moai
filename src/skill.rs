//! Claude 에 심을 것을 **글로 만든다.** 순수 함수다 — 파일도 안 쓰고 명령도
//! 안 부른다. 쓰는 일과 `claude` 를 부르는 일은 `cmd/skill.rs` 가 한다.
//!
//! ## 왜 플러그인인가
//!
//! 훅을 사람의 `settings.json` 에 직접 써 넣는 길도 있다. 그 길을 안 가는
//! 까닭은 하나다 — **JSON 에는 마커를 못 넣는다.** `init` 은 AGENTS.md 의
//! 마커 사이만 갈아 끼우고 사람이 쓴 산문은 한 글자도 안 건드리는데,
//! `settings.json` 에서는 그 약속을 못 지킨다. 다시 쓰는 순간 남의 서식이
//! 사라진다.
//!
//! 플러그인으로 심으면 훅이 **우리 디렉터리 안**에 있고, 사람의 설정에는
//! `claude` 가 제 손으로 두 키(`extraKnownMarketplaces`·`enabledPlugins`)만
//! 넣는다. 우리는 남의 JSON 을 만지지 않는다.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// 심는 자리. 저장소 안이라 팀이 그대로 커밋할 수 있다.
pub const DIR: &str = ".claude/moai-plugin";
/// 마켓플레이스 이름. `claude plugin install moai@<이것>` 의 뒷부분이다.
///
/// **저장소마다 달라야 한다.** 이름은 기계 하나에서 전역이라, 고정 이름을 쓰면
/// 둘째 저장소가 `install` 할 때 첫째 저장소의 트리를 가져간다 — 실제로 시험
/// 저장소가 이 저장소의 플러그인을 설치하는 것을 보고 알았다. 조용히 엉뚱한
/// 규칙이 걸리는 쪽이라 눈치채기도 어렵다.
///
/// 접두어에 **저장소 자리**를 섞는다. 접두어만으로는 모자란다 — 접두어는
/// 디렉터리 이름에서 나오므로 `~/work/api` 와 `~/old/api` 가 같은 것을 쓴다.
/// 그때 뒤에 심은 쪽이 등록에 실패하고, 실패한 자리에서 할 수 있는 일이
/// 없다(접두어는 못 바꾼다). 자리를 섞으면 그 막다른 길이 사라진다.
pub fn market(prefix: &str, root: &Path) -> String {
    format!("moai-{prefix}-{:04x}", stable(root.to_string_lossy().as_bytes()) % 0x1_0000)
}

/// 손으로 적은 FNV-1a. **`DefaultHasher` 를 쓰지 않는다** — 그 알고리즘은
/// rustc 판 사이에 바뀌어도 된다고 문서가 밝혀 두었다. 이름과 판이 그것에
/// 기대면 컴파일러를 올린 날 이름이 바뀌고, 옛 등록은 `지우지 않는다` 는
/// 약속 때문에 그대로 남아 훅이 두 벌 돈다 — 보드도 거절문도 두 번이다.
fn stable(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// 훅이 걸리는 자리와 그때 부를 이벤트.
///
/// **`SessionStart` 는 보드를 안 싣는다** — 재개에서 그 출력이 대화에 안 붙는
/// 것을 여러 번 확인했다. 까닭은 `hook::Event` 에 적혀 있다.
const HOOKS: &[(&str, &str, &str)] = &[
    ("SessionStart", "session-start", "moai 경고를 센다..."),
    ("UserPromptSubmit", "user-prompt-submit", "moai 보드를 읽는다..."),
    ("PreToolUse", "pre-tool-use", "moai 규칙을 본다..."),
    ("Stop", "stop", "moai 상태를 견준다..."),
    ("PreCompact", "pre-compact", "집고 있던 일을 적어 둔다..."),
];

/// `PreToolUse` 가 볼 도구들. 규칙이 뜻을 두는 것만 적는다 — 전부 받으면
/// 읽기만 하는 호출까지 훅을 한 번씩 띄운다.
const WATCHED: &str = "Bash|Edit|Write|NotebookEdit|Skill";

/// 훅이 부를 명령.
///
/// **없으면 조용히 0 이다.** `cargo clean` 한 번이면 바이너리가 사라지는데,
/// 그때 훅이 "command not found" 를 매 세션 뱉으면 사람이 훅을 꺼 버린다 —
/// 꺼진 규칙은 없는 규칙이다. 한 번은 이 가드가 없어 세션 하나가 통째로
/// 잠겼다: 도구 호출마다 훅이 실패해 `Bash` 도 `Write` 도 안 돌았다.
fn command(exe: &str, event: &str) -> String {
    // 따옴표를 깨는 경로는 아예 안 쓴다. 셸 한 줄이 깨지면 그 세션의 모든
    // 도구 호출이 막힌다.
    let exe = if exe.contains(['"', '\\', '$', '`']) { "moai" } else { exe };
    // **`command -v` 로 본다. `[ -x ]` 가 아니다.** `[ -x "moai" ]` 는 PATH 를
    // 안 보고 `./moai` 를 본다 — PATH 에 moai 가 있는 남의 기계에서 훅이 전부
    // 조용히 `exit 0` 으로 빠지고, 그 모습은 "규칙이 통과했다" 와 똑같다.
    // 이 저장소에서 그 검사가 참으로 보였던 것도 하필 `moai` 라는 **디렉터리**가
    // 있어서였다. `command -v` 는 절대 경로도 이름도 옳게 가린다.
    format!("command -v -- \"{exe}\" >/dev/null 2>&1 && \"{exe}\" hook {event} || exit 0")
}

/// 심을 파일들. 경로는 `DIR` 부터의 상대다.
///
/// **판(version)은 내용의 해시다.** `claude plugin install` 은 `directory`
/// 원본이어도 제 캐시로 **복사**하고, 그 복사는 `plugin.json` 의 판이 달라질
/// 때만 새로 뜬다. 손으로 세는 판은 반드시 어긋난다 — 내용이 같으면 판도
/// 같아 헛 업데이트가 없고, 한 글자라도 다르면 반드시 달라진다.
///
/// 판이 **내려가도** `claude plugin update` 는 받는다 (`1.0.1 → 0.0.5` 로
/// 실제로 재 봤다). 그래서 해시를 그대로 판으로 쓸 수 있다.
pub fn tree(
    prefix: &str,
    root: &Path,
    exe: &str,
    skill: &str,
    reference: &str,
) -> Vec<(PathBuf, String)> {
    let mut files: Vec<(PathBuf, String)> = vec![
        (PathBuf::from("skills/moai/SKILL.md"), skill.to_string()),
        (PathBuf::from("skills/moai/references/commands.md"), reference.to_string()),
    ];
    // 판은 **딸린 파일과 훅 명령**에서 나온다. 매니페스트 자신은 그 판을 담고
    // 있으므로 셈에 넣을 수 없다 — 넣으면 해시가 제 꼬리를 문다.
    let version = version_of(&files, &format!("{exe}\u{1}{prefix}"));
    files.push((PathBuf::from(".claude-plugin/plugin.json"), plugin_json(exe, &version)));
    files.push((
        PathBuf::from(".claude-plugin/marketplace.json"),
        marketplace_json(prefix, root),
    ));
    files
}

fn version_of(files: &[(PathBuf, String)], exe: &str) -> String {
    let mut all = String::new();
    for (path, body) in files {
        all.push_str(&path.to_string_lossy());
        all.push('\u{1}');
        all.push_str(body);
        all.push('\u{2}');
    }
    all.push_str(exe);
    for (at, event, message) in HOOKS {
        all.push_str(at);
        all.push_str(event);
        all.push_str(message);
    }
    all.push_str(WATCHED);
    // semver 세 자리에 나눠 담는다. `claude` 가 판을 semver 로 읽는다.
    let n = stable(all.as_bytes());
    format!("{}.{}.{}", n % 1000, (n / 1000) % 1000, (n / 1_000_000) % 1000)
}

/// **누가 심었는지는 안 적는다.** 심는 사람마다 이 파일이 바뀌면, 팀이
/// 커밋해 두고 쓰는 트리가 사람이 바뀔 때마다 헛 diff 를 낸다. 그리고 그 값이
/// 판(해시)에 안 들어가면 내용이 달라졌는데 판은 그대로인 자리가 생긴다 —
/// `claude` 가 옛 복사를 그대로 쓴다. 안 적는 것이 둘 다 푼다.
fn plugin_json(exe: &str, version: &str) -> String {
    let mut hooks = BTreeMap::new();
    for (at, event, message) in HOOKS {
        let entry = serde_json::json!({
            "type": "command",
            "command": command(exe, event),
            "timeout": 15,
            "statusMessage": message,
        });
        let group = if *at == "PreToolUse" {
            serde_json::json!({ "matcher": WATCHED, "hooks": [entry] })
        } else {
            serde_json::json!({ "hooks": [entry] })
        };
        hooks.insert(*at, vec![group]);
    }
    let manifest = serde_json::json!({
        "name": "moai",
        "description": "이 저장소의 이슈 트래커. 보드를 세션에 싣고, 새 이슈가 집고 있는 단위 밖으로 새는 것을 막는다.",
        "version": version,
        "hooks": hooks,
    });
    pretty(&manifest)
}

fn marketplace_json(prefix: &str, root: &Path) -> String {
    pretty(&serde_json::json!({
        "$schema": "https://anthropic.com/claude-code/marketplace.schema.json",
        "name": market(prefix, root),
        "description": "moai 가 심는다. 손으로 고치면 다음 `moai skill install` 이 덮어쓴다.",
        "owner": { "name": "moai" },
        "plugins": [{
            "name": "moai",
            "description": "이 저장소의 할 일·규칙·보드.",
            "source": "./",
            "category": "productivity",
        }],
    }))
}

fn pretty(v: &serde_json::Value) -> String {
    let mut s = serde_json::to_string_pretty(v).unwrap_or_default();
    s.push('\n');
    s
}

/// 훅이 부를 실행 파일을 어떻게 적을까.
///
/// **PATH 에서 같은 파일이 찾아지면 이름만 적는다.** 그러면 심은 트리를 팀이
/// 그대로 커밋해도 남의 기계에서 산다. 아니면 절대 경로다 — 이 저장소처럼
/// PATH 의 `moai` 가 **다른** moai 인 곳에서는 이름을 적는 것이 남의 바이너리를
/// 부르는 일이 된다.
pub fn exe_name(current: &Path, on_path: Option<&Path>) -> String {
    match on_path {
        Some(p) if p == current => "moai".to_string(),
        _ => current.display().to_string(),
    }
}

/// 심는 글. **`init` 의 AGENTS 블록과 한 출처여야 한다** — 지금은 두 벌이고,
/// 합치는 것은 `moai-zy40` 이다.
///
/// **`cmd` 가 아니라 여기 둔다.** 이 글은 규칙이 실제로 요구하는 명령을
/// 가르치므로, 규칙과 어긋나면 에이전트가 거절문을 보기도 전에 틀린 길로
/// 간다 — 실제로 그렇게 틀린 적이 있다. 순수 모듈에 있어야 시험이
/// `hook` 의 판정에 이 글을 그대로 먹여 볼 수 있다.
pub const SKILL: &str = r#"---
name: moai
description: 이 저장소의 할 일·이슈·계획을 다룰 때 쓴다. "뭐부터 할까", "할 일 정리", "이슈 만들어", "진행 상황", "이거 나중에 하자", 기능 요청을 여러 갈래로 쪼갤 때, 또는 작업 중 지금 범위가 아닌 것이 떠올랐을 때. TodoWrite 나 마크다운 TODO 목록 대신 이것을 쓴다.
---

# moai — 이 저장소의 이슈 트래커

할 일은 `.moai/issues.jsonl` 에 있다. 승인 게이트가 없다 — 무엇이든 만들고
무엇이든 옮길 수 있다. 사람을 부르지 않는다.

    moai status                            보드 · 경고 · 흐름 (세션은 여기서 시작)
    moai ready                             지금 집을 수 있는 일
    moai show <id>                         본문·자식·이력. 왜 그렇게 정했는지가 여기 있다
    moai show -g <키워드>                  이미 적어 뒀는지 찾는다
    moai add "제목" -p 1 -t bug -e <에픽>  만들기
    moai mv <id> in_progress               집기  →  review  →  done
    moai note <id> "발견한 것"             다음 사람이 읽을 메모
    moai defer <id> -m "왜"                지금 안 할 일을 계획에서 뺀다

모든 명령에 `--json` 이 붙는다. 담당은 만든 사람이 저절로 맡는다.

## 갈림길 셋

**1. `add` 냐 `idea` 냐** — 가르는 것은 하나다. *지금 집을 것인가.*
집을 것이면 `moai add`, 나중에 볼 것이면 `moai idea add "떠오른 것"`.
idea 는 보드에도 `ready` 에도 안 들어 계획을 흐리지 않는다.
**적지 않고 넘어가는 것이 제일 나쁘다.**

**2. `defer` 냐 `done` 이냐** — 안 하기로 한 것을 `done` 으로 옮기지 않는다.
`moai defer <id> -m "왜"` 는 칸도 종류도 안 바꾸고, `--undo` 로 같은 줄이
그대로 돌아온다. idea 는 "아직 일이 아닌 것", defer 는 "일이지만 지금은 아닌 것".

**3. 에픽으로 쪼갤 만한가** — 파일 하나로 안 끝나는 요청이면 코드를 쓰기 전에
에픽 하나 + 이슈 3~7개로 쪼갠 안을 사람에게 **한 번** 보여주고 물어본다.
"좋다" 를 받으면 `moai add --from -` 로 한 번에 만든다 (`--dry-run` 으로 먼저 봐도 된다).

    moai add --from - <<'MD'
    # 에픽 제목
    - [p1] 첫 이슈 #enhancement
    - [p2] 둘째 이슈
    MD

## 훅이 실제로 보는 것 셋

**1. 집은 것 밖에 새 이슈를 세우지 않는다.** `in_progress` 인 이슈가 초점이다.
그 일을 하다 나온 것은 같은 에픽 안(`-e <에픽>`)이나 그 일의 자식
(`--parent <id>`)으로 만든다. 지금 할 일이 아니면 `moai idea add` 로 담는다 —
idea 는 이 규칙에서 언제나 자유롭고, `moai add --from` 도 그렇다 (거기서
만들어지는 것은 에픽과 그 자식들이라 그 자체로 한 단위다).

**2. 저장소를 고치기 전에 하나를 집는다.** `moai mv <id> in_progress`.
세는 것은 저장소 안의 일감뿐이다 — `.moai/`·`.claude/`·`target/` 과 저장소
밖(스크래치패드·임시 파일)은 안 센다. 계획에 없던 것이면 `moai add "제목"` 으로
세우고 그것을 집는다.

**3. 리뷰도 이슈다.** `/code-review` 를 부르기 전에 지금 보는 것에 매인 리뷰
이슈를 세운다.

    moai add "리뷰 — <무엇을 보는가>" -t review --parent <보는 이슈> -b "<무엇을 왜 보는가>"
    moai mv <id> in_progress                  리뷰를 시작할 때
    moai note <id> -b - < <리뷰 원문>         낸 글을 **그대로** 남긴다
    moai mv <id> done -m "<무엇을 반영하고 무엇을 넘겼나>"

관점(`-b`)과 닫는 한 줄(`-m`)은 규칙이 **실제로 요구한다.** 없이 부르면
막히고, 거절문이 고칠 명령을 함께 낸다. **사람을 부르지 않는다** — 그 명령을
그대로 부르면 지나간다.

**원문과 판단을 두 노트로 가른다** — 리뷰어가 한 말과 이쪽이 정한 것은 다른
글이다. 넘긴 것은 **이슈 번호와 함께** 적는다. "넘겼다" 만 적힌 줄은 아무도
다시 안 본다. 원문을 어디서 찾는지는 `references/commands.md` 에 있다.

## 세션을 닫기 전에

`moai status` 를 한 번 더 돌려 경고가 늘지 않았는지 본다. 경고는 막지 않는다 —
에픽 없는 이슈, 오래 멈춘 review, 한 번에 벌여 놓은 것, 쌓인 idea 를 비출 뿐이다.

전체 명령과 `--from` 문법은 `references/commands.md` 에 있다.
"#;

pub const REFERENCE: &str = r#"# 전체 명령

`moai --help` 와 `moai <명령> --help` 가 참이다. 이 파일은 그 요약이라
어긋나면 도움말이 이긴다.

## 묶음은 둘이다

    moai epic add "저장 계층"                      에픽
    moai milestone add "v0.1"                      마일스톤
    moai add "제목" -e <에픽> --milestone <마일스톤>
    moai show <에픽|마일스톤 id>                   그 밑에 무엇이 있는지
    moai show --milestone <id>                     그 마일스톤에 딸린 전부

**소속은 물려받는다.** 자식은 부모의 에픽을, 이슈는 제 에픽의 마일스톤을
물려받는다. 이슈마다 다시 적지 않는다 — 에픽을 옮기면 멤버가 따라온다.

## 거름망

쉼표는 "또는", 같은 플래그를 두 번 쓰면 "그리고" 다.

    moai show -s todo -t bug          todo 이면서 bug
    moai show -s todo,review          todo 또는 review
    moai show -e none                 에픽 없는 것
    moai show --deferred              미뤄 둔 것만
    moai show --stale 7               지금 칸에 이레 넘게 머문 것
    moai show --tree                  에픽 → 이슈 → 자식

## 한 번에 만들기

`#` 줄은 에픽, `-` 줄은 바로 위 에픽의 이슈다. `[pN]` 과 `#태그` 는 없어도 된다.

    moai add --from - <<'MD'
    # 저장 계층
    - [p1] 원자적으로 쓴다 #enhancement
    - 잘린 줄을 복구한다 #bug
    MD

`--dry-run` 이 heredoc 오타로 엉뚱한 여섯 개를 만드는 것을 막는다.

## 담아 둔 생각을 펼치기

    moai idea add "반짝 떠오른 것"
    moai idea ls
    moai idea promote <id> --from -    에픽과 이슈로 펼치고 그 생각을 닫는다

## 사람

이름과 메일은 `git config` 에서 온다. 없으면 `--user "이름 (메일)"` 이나
`MOAI_ACTOR` 로 준다. 남에게 맡기려면 `-a "이름 (메일)"`, 임자 없이 두려면
`-a none`.

## 리뷰가 낸 글을 찾는 법

리뷰 전문은 파일에 남아 있다. 끝났다는 알림에 `task-id` 가 실려 오고, 그것이
곧 파일 이름이다.

    ~/.claude/projects/<프로젝트>/<세션>/subagents/agent-<task-id>.jsonl

리뷰 전문은 **마지막 `text` 블록**이다.

    python3 -c "
    import json,sys
    t=[c['text'] for l in open(sys.argv[1])
       for c in json.loads(l).get('message',{}).get('content') or []
       if isinstance(c, dict) and c.get('type') == 'text']
    print(t[-1] if t else '')" <그 파일> | moai note <리뷰 id> -b -

**마지막 줄을 그냥 집지 않는다.** 한 턴의 블록이 줄마다 나뉘어 적히고 생각·
도구 호출도 섞여, 마지막 줄이 글이 아닐 때가 있다. 그러면 빈 글이 넘어가고
`moai note` 가 "메모가 비었다" 로 멈춘다 — 시끄럽게 멈추니 잃지는 않지만,
한 번에 되는 편이 낫다.

**요약만 적고 원문을 버리지 않는다.** 요약은 이쪽의 판단이고 원문은 리뷰어가
한 말이다. 판단은 다시 할 수 있지만 버린 원문은 못 되돌린다.

## 훅

    moai hook <event>    Claude 의 훅이 부른다. 사람이 손으로 부를 일은 없다

`moai skill install` 이 심은 플러그인이 이것을 부른다. 무엇이 어긋나도 종료
코드는 0 이다 — 훅이 시끄러우면 사람이 훅을 꺼 버리고, 꺼진 규칙은 없는
규칙이다.
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::model::{Issue, Kind, Status};

    fn tree_of(exe: &str, skill: &str) -> BTreeMap<String, String> {
        tree_for("t", exe, skill)
    }

    fn tree_for(prefix: &str, exe: &str, skill: &str) -> BTreeMap<String, String> {
        tree_at(prefix, Path::new("/repo"), exe, skill)
    }

    fn tree_at(prefix: &str, root: &Path, exe: &str, skill: &str) -> BTreeMap<String, String> {
        tree(prefix, root, exe, skill, "참고")
            .into_iter()
            .map(|(p, b)| (p.display().to_string(), b))
            .collect()
    }

    /// 초점이 있을 때 **막히는 것이 옳은** 명령들.
    ///
    /// 새 단위를 세우는 자리라 규칙 1 이 잡는 것이 뜻대로다. 목록으로 두는
    /// 까닭은 본문이 바뀔 때 **왜 막히는지 한 번 더 생각하게 하기 위해서**다 —
    /// 시험이 규칙을 다시 구현하면 규칙이 틀렸을 때 시험도 같이 틀린다.
    ///
    /// 목록은 **정확히** 적는다. `moai add "제목" -p 1 -t bug -e <에픽>` 처럼
    /// 앵커가 붙은 줄까지 접두로 싸잡으면, 지나가는 것이 맞는 명령을 시험이
    /// "막혀야 한다" 고 우긴다 — 처음 적을 때 실제로 그랬다.
    const DENIED_WHILE_HELD: &[&str] = &["moai epic add", "moai milestone add"];

    /// **심는 글이 가르치는 명령은 규칙에 막히면 안 된다.**
    ///
    /// 이 글은 에이전트가 **가장 먼저** 읽는 것이라, 규칙과 어긋나면 거절문을
    /// 보기도 전에 틀린 길로 간다. 실제로 `-b` 없는 `add` 와 `-m` 없는 `done`
    /// 을 가르치고 있었고, 그것을 잡은 것은 시험이 아니라 리뷰였다.
    ///
    /// **초점이 있는 상태로만 본다.** 앞선 판은 아무것도 안 집은 상태도 함께
    /// 봤는데, 그 상태에서는 `guard_create` 와 `guard_close` 가 명령을 읽기도
    /// 전에 통과한다 — 어떤 글자를 넣어도 초록이라 아무것도 지키지 못했다.
    /// 리뷰어가 나쁜 명령을 일부러 심어도 시험은 웃고 있었다.
    #[test]
    fn what_the_skill_teaches_actually_passes() {
        use crate::hook::{guard_close, guard_create, Decision};

        let cfg = Config::parse("prefix = \"t\"\n").unwrap();
        let held = vec![epic_row(), held_row(), review_row("in_progress")];

        let mut checked = 0;
        for cmd in taught() {
            let should_deny = DENIED_WHILE_HELD.iter().any(|d| cmd.starts_with(d));
            let got = match guard_create(&held, &cfg, &cmd) {
                Decision::Pass => guard_close(&held, &cfg, &cmd),
                deny => deny,
            };
            match (&got, should_deny) {
                (Decision::Pass, false) => {}
                (Decision::Deny(_), true) => {}
                (Decision::Pass, true) => {
                    panic!("막혀야 하는데 지나간다 — {cmd}")
                }
                (got, _) => panic!("가르치는 명령이 막힌다 — {cmd}\n{got:?}"),
            }
            checked += 1;
        }
        assert!(checked > 15, "가르치는 명령을 {checked}개밖에 못 찾았다");
    }

    /// **가르친 대로 세운 리뷰로 곧장 리뷰를 부를 수 있어야 한다.**
    ///
    /// 관점(`-b`)을 요구하는 것은 `guard_review` 인데 위 시험은 그것을 부르지
    /// 않는다. 그래서 가르치는 줄에서 `-b` 를 지워도 초록이었다 — 리뷰어가
    /// 실제로 지워 보고 알아냈다. 여기서 그 다리를 놓는다.
    #[test]
    fn a_review_made_as_taught_can_be_used_at_once() {
        use crate::hook::{guard_review, Decision};

        let cfg = Config::parse("prefix = \"t\"\n").unwrap();
        let made = taught()
            .into_iter()
            .find(|c| c.starts_with("moai add") && c.contains("-t review"))
            .expect("리뷰를 세우는 명령을 안 가르친다");

        // 그 명령이 만들 줄을 세운다 — 본문은 `-b` 가 있을 때만 붙는다.
        let mut review = review_row("in_progress");
        review.body = angle_of(&made);
        let all = vec![epic_row(), held_row(), review];

        assert_eq!(
            guard_review(&all, &cfg),
            Decision::Pass,
            "가르친 대로 세운 리뷰가 규칙 3 에 막힌다 — {made}"
        );
    }

    /// `-b "<글>"` 이 있으면 그 글. 없으면 `None` — 본문 없는 줄이 선다.
    fn angle_of(cmd: &str) -> Option<String> {
        let at = cmd.find("-b ")?;
        let rest = cmd[at + 3..].trim_start_matches('"');
        let end = rest.find('"').unwrap_or(rest.len());
        let body = rest[..end].trim();
        (!body.is_empty()).then(|| body.to_string())
    }

    /// 심는 글이 실제로 가르치는 명령들. 자리표시자는 실제 값으로 바꾼다.
    ///
    /// **글자로 고르지 않는다.** 앞서 리뷰 절차를 `contains("review")` 로
    /// 골랐는데, 그 그물은 `moai show -s todo,review` 를 끌어오고 리뷰
    /// 토막의 문구가 바뀌면 조용히 아무것도 안 고른다.
    fn taught() -> Vec<String> {
        SKILL
            .lines()
            .chain(REFERENCE.lines())
            .map(str::trim)
            .filter(|l| l.starts_with("moai ") && !l.contains("<명령>"))
            // 치트시트 줄은 **두 칸 이상 띄우고** 설명을 붙인다. 그 뒤는
            // 명령이 아니다 — 안 자르면 "집기 → review → done" 의 `done` 이
            // 인자로 읽혀, 시험이 제가 만든 허깨비를 잡는다.
            .map(|l| l.split("  ").next().unwrap_or(l).trim())
            .map(|l| {
                l.replace("<id>", "t-r")
                    .replace("<에픽>", "t-e")
                    .replace("<보는 이슈>", "t-1")
                    .replace("<리뷰 id>", "t-r")
                    .replace("<리뷰 원문>", "/tmp/review.txt")
                    .replace("<그 파일>", "/tmp/review.txt")
                    .replace("<키워드>", "파서")
                    .replace("<마일스톤>", "t-m")
            })
            .collect()
    }

    fn epic_row() -> Issue {
        Issue::new("t-e".into(), "에픽".into(), Kind::Epic, Status::new("todo"), NOW)
    }

    fn held_row() -> Issue {
        let mut i =
            Issue::new("t-1".into(), "집은 일".into(), Kind::Issue, Status::new("in_progress"), NOW);
        i.epic = Some("t-e".into());
        i
    }

    fn review_row(status: &str) -> Issue {
        let mut i =
            Issue::new("t-r".into(), "리뷰".into(), Kind::Issue, Status::new(status), NOW);
        i.epic = Some("t-e".into());
        i.tags = vec!["review".into()];
        i.body = Some("무엇을 왜 보는가".into());
        i
    }

    const NOW: &str = "2026-01-01T00:00:00Z";

    /// 심는 것은 넷이다 — 스킬, 참고, 그리고 매니페스트 둘.
    #[test]
    fn the_tree_has_what_claude_needs() {
        let files = tree_of("/bin/moai", "# 스킬");
        for want in [
            "skills/moai/SKILL.md",
            "skills/moai/references/commands.md",
            ".claude-plugin/plugin.json",
            ".claude-plugin/marketplace.json",
        ] {
            assert!(files.contains_key(want), "{want} 가 없다");
        }
    }

    /// **판은 내용에서 나온다.** 같은 내용이면 같은 판이라 헛 업데이트가 없고,
    /// 한 글자라도 다르면 반드시 달라진다 — 손으로 세면 반드시 어긋난다.
    #[test]
    fn the_version_is_the_content() {
        let a = tree_of("/bin/moai", "# 스킬");
        let b = tree_of("/bin/moai", "# 스킬");
        let c = tree_of("/bin/moai", "# 스킬 (고침)");
        let d = tree_of("/usr/local/bin/moai", "# 스킬");
        let version = |f: &BTreeMap<String, String>| {
            let v: serde_json::Value =
                serde_json::from_str(&f[".claude-plugin/plugin.json"]).unwrap();
            v["version"].as_str().unwrap().to_string()
        };
        assert_eq!(version(&a), version(&b), "같은 내용인데 판이 다르다");
        assert_ne!(version(&a), version(&c), "본문이 달라졌는데 판이 같다");
        assert_ne!(version(&a), version(&d), "부를 바이너리가 달라졌는데 판이 같다");
        // semver 세 자리여야 `claude` 가 읽는다.
        assert_eq!(version(&a).split('.').count(), 3, "{}", version(&a));
    }

    /// **마켓플레이스 이름은 저장소마다 다르다.** 이름은 기계 하나에서
    /// 전역이라, 고정 이름이면 둘째 저장소가 첫째의 트리를 가져간다 — 시험
    /// 저장소가 이 저장소의 플러그인을 설치하는 것을 실제로 보았다.
    #[test]
    fn two_repos_do_not_share_a_marketplace() {
        let one = tree_for("alpha", "/bin/moai", "# 스킬");
        let two = tree_for("beta", "/bin/moai", "# 스킬");
        let name = |f: &BTreeMap<String, String>| {
            let v: serde_json::Value =
                serde_json::from_str(&f[".claude-plugin/marketplace.json"]).unwrap();
            v["name"].as_str().unwrap().to_string()
        };
        assert!(name(&one).starts_with("moai-alpha-"), "{}", name(&one));
        assert_ne!(name(&one), name(&two));

        // **접두어가 같아도 자리가 다르면 이름이 다르다.** 접두어는 디렉터리
        // 이름에서 나오므로 `~/work/api` 와 `~/old/api` 가 쉽게 겹친다.
        let here = tree_at("api", Path::new("/work/api"), "/bin/moai", "# 스킬");
        let there = tree_at("api", Path::new("/old/api"), "/bin/moai", "# 스킬");
        assert_ne!(name(&here), name(&there), "접두어가 같다고 이름까지 같다");
    }

    /// **이름으로 적을 때도 PATH 를 본다.** `[ -x "moai" ]` 는 PATH 가 아니라
    /// `./moai` 를 보므로, PATH 에 moai 가 있는 남의 기계에서 훅이 전부 조용히
    /// 빠진다 — 그 모습은 "규칙이 통과했다" 와 구별되지 않는다. 이 저장소에서
    /// 그 검사가 참으로 보였던 것도 하필 `moai` 라는 디렉터리가 있어서였다.
    #[test]
    fn a_bare_name_is_looked_up_on_the_path() {
        let cmd = command("moai", "stop");
        assert!(cmd.contains("command -v"), "PATH 를 안 본다 — {cmd}");
        assert!(!cmd.contains("[ -x"), "상대 경로를 본다 — {cmd}");
    }

    /// 훅은 **없으면 조용히 0** 이다. 한 번은 이 가드가 없어 세션 하나가
    /// 통째로 잠겼다 — 훅이 실패하자 `Bash` 도 `Write` 도 안 돌았다.
    #[test]
    fn a_missing_binary_never_makes_noise() {
        let files = tree_of("/nowhere/moai", "# 스킬");
        let v: serde_json::Value =
            serde_json::from_str(&files[".claude-plugin/plugin.json"]).unwrap();
        let mut seen = 0;
        for (_, groups) in v["hooks"].as_object().unwrap() {
            for g in groups.as_array().unwrap() {
                for h in g["hooks"].as_array().unwrap() {
                    let cmd = h["command"].as_str().unwrap();
                    assert!(cmd.contains("exit 0"), "가드가 없다 — {cmd}");
                    // 경로가 따옴표에 싸이므로 `moai" hook ...` 모양이다.
                    assert!(cmd.contains("/nowhere/moai"), "엉뚱한 것을 부른다 — {cmd}");
                    assert!(cmd.contains("command -v"), "있는지부터 안 본다 — {cmd}");
                    assert!(cmd.contains(" hook "), "훅을 안 부른다 — {cmd}");
                    seen += 1;
                }
            }
        }
        assert_eq!(seen, HOOKS.len(), "훅 수가 안 맞는다");
    }

    /// 규칙이 뜻을 두는 도구만 본다. 전부 받으면 읽기만 하는 호출까지
    /// 훅을 한 번씩 띄운다.
    #[test]
    fn only_the_tools_the_rules_care_about_are_watched() {
        let files = tree_of("/bin/moai", "# 스킬");
        let v: serde_json::Value =
            serde_json::from_str(&files[".claude-plugin/plugin.json"]).unwrap();
        let matcher = v["hooks"]["PreToolUse"][0]["matcher"].as_str().unwrap();
        assert!(matcher.contains("Bash") && matcher.contains("Edit"), "{matcher}");
        assert!(v["hooks"]["Stop"][0].get("matcher").is_none(), "Stop 에 matcher 가 붙었다");
    }

    /// PATH 의 `moai` 가 **같은 파일일 때만** 이름으로 적는다.
    ///
    /// 이 저장소가 그 반례다 — PATH 의 `moai` 는 옛 moai 의 바이너리이고,
    /// 이름만 적으면 훅이 남의 도구를 부른다.
    #[test]
    fn the_name_is_used_only_when_path_agrees() {
        let here = Path::new("/repo/target/release/moai");
        assert_eq!(exe_name(here, Some(here)), "moai");
        assert_eq!(exe_name(here, Some(Path::new("/usr/bin/moai"))), "/repo/target/release/moai");
        assert_eq!(exe_name(here, None), "/repo/target/release/moai");
    }

    /// 따옴표를 깨는 경로에는 절대 경로를 안 쓴다. 셸 한 줄이 깨지면 그
    /// 세션의 모든 도구 호출이 막힌다.
    #[test]
    fn a_hostile_path_falls_back_to_the_name() {
        let cmd = command("/tmp/\"moai\"", "stop");
        assert!(!cmd.contains("/tmp/"), "그 경로가 그대로 들어갔다 — {cmd}");
        assert!(cmd.contains("\"moai\" hook stop"), "이름으로도 안 부른다 — {cmd}");
    }
}
