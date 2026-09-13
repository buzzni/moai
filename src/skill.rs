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
/// 것을 여러 번 확인했다. 접힌 뒤에만 집고 있던 것을 싣는다. **`PreCompact`
/// 는 걸지 않는다** — `claude` 가 그 출력을 거절한다. 까닭은 `hook::Event` 에
/// 적혀 있다.
const HOOKS: &[(&str, &str, &str)] = &[
    ("SessionStart", "session-start", "moai 경고를 센다..."),
    ("UserPromptSubmit", "user-prompt-submit", "moai 보드를 읽는다..."),
    ("PreToolUse", "pre-tool-use", "moai 규칙을 본다..."),
    ("Stop", "stop", "moai 상태를 견준다..."),
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
    let exe = if quotable(exe) { exe } else { "moai" };
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
    let market = (PathBuf::from(".claude-plugin/marketplace.json"), marketplace_json(prefix, root));
    // 판은 **매니페스트를 뺀 트리 전부와 판 자리를 비운 매니페스트**에서 나온다.
    // 매니페스트 자신은 그 판을 담고 있으므로 그대로는 셈에 넣을 수 없다 —
    // 넣으면 해시가 제 꼬리를 문다. 그렇다고 매니페스트를 통째로 빼면 그 틀
    // (`timeout`·`description`)만 바꾼 판이 옛 판과 같아 `claude` 가 옛 복사를
    // 계속 쓴다. 훅 명령·실행 파일·이름표는 그 틀과 `marketplace.json` 에 이미
    // 들어 있어 따로 셈하지 않는다 — 따로 적은 목록은 틀이 자랄 때마다 어긋난다.
    let version = version_of(&[files.as_slice(), std::slice::from_ref(&market)].concat(), &plugin_json(exe, ""));
    files.push((PathBuf::from(".claude-plugin/plugin.json"), plugin_json(exe, &version)));
    files.push(market);
    files
}

fn version_of(files: &[(PathBuf, String)], template: &str) -> String {
    let mut all = String::new();
    for (path, body) in files {
        all.push_str(&path.to_string_lossy());
        all.push('\u{1}');
        all.push_str(body);
        all.push('\u{2}');
    }
    all.push_str(template);
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
    let shown = current.display().to_string();
    match on_path {
        Some(p) if p == current => "moai".to_string(),
        // 따옴표를 깨는 경로는 훅 한 줄에 못 적어 `command` 가 이름으로 바꿔 적는다.
        // **여기서 먼저 바꾼다** — 안 그러면 판·설치 출력·`status` 는 절대 경로를
        // 말하는데 훅은 PATH 의 `moai` 를 불러, 셋이 서로 다른 것을 가리킨다.
        _ if !quotable(&shown) => "moai".to_string(),
        _ => shown,
    }
}

/// 셸 한 줄의 따옴표 안에 그대로 적을 수 있는 경로인가.
fn quotable(exe: &str) -> bool {
    !exe.contains(['"', '\\', '$', '`'])
}

/// `claude` 가 장부에 적어 둔 설치 한 건.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Install {
    pub scope: String,
    pub version: String,
    pub install_path: String,
    pub project: Option<String>,
}

/// `installed_plugins.json` 에서 **이 저장소의** moai 설치를 고른다.
///
/// 마켓플레이스 이름이 저장소마다 달라(`market`) `moai@<이름>` 이면 이미 이
/// 저장소의 것이다. 그래도 `local`·`project` 는 `projectPath` 를 한 번 더
/// 본다 — 같은 이름이 옛 자리에 남아 있는 줄을 제 것으로 걷으면 남의 설정을
/// 건드린다. 자리를 견주는 법(심볼릭 링크 풀기)은 부르는 쪽이 준다.
pub fn installs(ledger: &serde_json::Value, market: &str, is_here: impl Fn(&str) -> bool) -> Vec<Install> {
    let key = format!("moai@{market}");
    let Some(rows) = ledger.get("plugins").and_then(|p| p.get(&key)).and_then(|r| r.as_array()) else {
        return Vec::new();
    };
    let text = |row: &serde_json::Value, k: &str| row.get(k).and_then(|v| v.as_str()).map(str::to_string);
    rows.iter()
        .filter_map(|row| {
            let scope = text(row, "scope")?;
            let project = text(row, "projectPath");
            if scope != "user" && !project.as_deref().is_some_and(&is_here) {
                return None;
            }
            Some(Install {
                scope,
                version: text(row, "version").unwrap_or_default(),
                install_path: text(row, "installPath").unwrap_or_default(),
                project,
            })
        })
        .collect()
}

/// 매니페스트가 훅으로 부르는 실행 파일. `command` 가 적는 모양
/// (`command -v -- "<exe>" …`)에서 따옴표 속을 꺼낸다.
///
/// **설치본의 매니페스트를 읽는다.** 저장소의 트리가 아니라 `claude` 가 복사해
/// 간 쪽이 실제로 불린다 — 둘이 어긋난 채로 저장소만 보면 멀쩡해 보인다.
pub fn hook_exe(plugin_json: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(plugin_json).ok()?;
    let cmd = v
        .get("hooks")?
        .as_object()?
        .values()
        .filter_map(|groups| groups.get(0)?.get("hooks")?.get(0)?.get("command")?.as_str())
        .next()?;
    let rest = cmd.strip_prefix("command -v -- \"")?;
    Some(rest[..rest.find('"')?].to_string())
}

/// 트리의 매니페스트에 적힌 판.
pub fn version_in(files: &[(PathBuf, String)]) -> Option<String> {
    let (_, body) = files.iter().find(|(p, _)| p.ends_with("plugin.json"))?;
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    v.get("version")?.as_str().map(str::to_string)
}

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
        use crate::hook::{guard_shell, Decision};

        let cfg = Config::parse("prefix = \"t\"\n").unwrap();
        let held = vec![epic_row(), held_row(), review_row("in_progress")];
        let root = Path::new("/repo");

        let mut checked = 0;
        for cmd in taught() {
            let should_deny = DENIED_WHILE_HELD.iter().any(|d| cmd.starts_with(d));
            // 훅이 실제로 부르는 그 차례로 본다 — 손으로 다시 짠 차례는 규칙이
            // 하나 늘 때 여기서 빠진다.
            let got = guard_shell(&held, &cfg, root, root, &cmd);
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

        // **아무것도 안 집은 채로도 쓰기 규칙에 안 걸린다.** 가르치는 명령은
        // 트래커를 만질 뿐 저장소 파일을 쓰지 않는다 — `< <리뷰 원문>` 같은
        // 자리표시자의 `>` 가 리다이렉션으로 읽히면 여기서 붉어진다.
        let idle = vec![epic_row()];
        for cmd in taught() {
            assert_eq!(
                guard_shell(&idle, &cfg, root, root, &cmd),
                Decision::Pass,
                "가르치는 명령이 아무것도 안 집은 채로 막힌다 — {cmd}"
            );
        }
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

    /// 심는 글과 AGENTS 블록이 실제로 가르치는 명령들. 자리표시자는 실제 값으로 바꾼다.
    ///
    /// **글자로 고르지 않는다.** 앞서 리뷰 절차를 `contains("review")` 로
    /// 골랐는데, 그 그물은 `moai show -s todo,review` 를 끌어오고 리뷰
    /// 토막의 문구가 바뀌면 조용히 아무것도 안 고른다.
    fn taught() -> Vec<String> {
        // AGENTS 블록도 같은 조각에서 나오므로 같이 본다.
        let texts = [crate::guide::skill(), crate::guide::reference(), crate::guide::agents()];
        texts
            .iter()
            .flat_map(|t| t.lines())
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

    /// **매니페스트의 틀도 판에 든다.** 딸린 파일과 훅 명령만 셈하던 판은
    /// `timeout` 이나 `description` 만 바꾸면 판이 그대로여서, `claude` 가 옛
    /// 복사를 계속 썼다. 셈에 넣는 틀이 실제로 심는 매니페스트와 판 한 자리만
    /// 다른지도 본다 — 다른 틀을 셈하면 이 시험은 통과해도 구멍은 그대로다.
    #[test]
    fn the_manifest_template_is_in_the_version() {
        let files = tree_of("/bin/moai", "# 스킬");
        let mut shipped: serde_json::Value =
            serde_json::from_str(&files[".claude-plugin/plugin.json"]).unwrap();
        shipped["version"] = "".into();
        assert_eq!(pretty(&shipped), plugin_json("/bin/moai", ""), "셈한 틀이 심는 매니페스트와 다르다");

        let rest: Vec<(PathBuf, String)> = files
            .into_iter()
            .filter(|(p, _)| p != ".claude-plugin/plugin.json")
            .map(|(p, b)| (PathBuf::from(p), b))
            .collect();
        let template = plugin_json("/bin/moai", "");
        let tweaked = template.replace("\"timeout\": 15", "\"timeout\": 30");
        assert_ne!(template, tweaked, "시험이 틀을 못 바꿨다");
        assert_ne!(version_of(&rest, &template), version_of(&rest, &tweaked), "틀이 바뀌었는데 판이 같다");
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

    /// **이 저장소의 설치만 고른다.** 같은 이름이 옛 자리에 남은 `local` 줄을
    /// 제 것으로 걷으면 남의 설정을 건드린다. `user` 는 자리가 없으니 이름으로
    /// 족하다.
    #[test]
    fn only_this_repos_installs_are_picked() {
        let ledger = serde_json::json!({"plugins": {
            "moai@m": [
                {"scope": "local", "projectPath": "/repo", "version": "1.2.3", "installPath": "/c/1.2.3"},
                {"scope": "local", "projectPath": "/old/repo", "version": "0.0.1", "installPath": "/c/0.0.1"},
                {"scope": "user", "version": "4.5.6", "installPath": "/c/4.5.6"},
            ],
            "moai@other": [{"scope": "user", "version": "9.9.9", "installPath": "/c/9"}],
        }});
        let got = installs(&ledger, "m", |p| p == "/repo");
        let versions: Vec<&str> = got.iter().map(|i| i.version.as_str()).collect();
        assert_eq!(versions, ["1.2.3", "4.5.6"]);
        assert!(installs(&serde_json::json!({}), "m", |_| true).is_empty(), "빈 장부에서 무언가 골랐다");
    }

    /// 매니페스트에서 훅이 부르는 실행 파일을 **심은 그대로** 꺼낸다 — 이름이든
    /// 절대 경로든.
    #[test]
    fn the_hook_exe_round_trips_through_the_manifest() {
        for exe in ["/repo/target/release/moai", "moai"] {
            let files = tree("t", Path::new("/repo"), exe, "# 스킬", "참고");
            let (_, manifest) = files.iter().find(|(p, _)| p.ends_with("plugin.json")).unwrap();
            assert_eq!(hook_exe(manifest).as_deref(), Some(exe));
            assert!(version_in(&files).is_some_and(|v| v.split('.').count() == 3));
        }
        assert_eq!(hook_exe("{ 깨진 json"), None);
    }

    /// 따옴표를 깨는 경로에는 절대 경로를 안 쓴다. 셸 한 줄이 깨지면 그
    /// 세션의 모든 도구 호출이 막힌다.
    #[test]
    fn a_hostile_path_falls_back_to_the_name() {
        let cmd = command("/tmp/\"moai\"", "stop");
        assert!(!cmd.contains("/tmp/"), "그 경로가 그대로 들어갔다 — {cmd}");
        assert!(cmd.contains("\"moai\" hook stop"), "이름으로도 안 부른다 — {cmd}");
        // **판과 출력도 같은 이름을 말한다.** 매니페스트만 이름으로 바꾸면 설치
        // 출력과 `status` 는 절대 경로를, 훅은 PATH 의 moai 를 가리킨다.
        assert_eq!(exe_name(Path::new("/tmp/we$ird/moai"), None), "moai");
        assert_eq!(exe_name(Path::new("/tmp/plain/moai"), None), "/tmp/plain/moai");
    }
}
