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
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    root.hash(&mut h);
    format!("moai-{prefix}-{:04x}", h.finish() % 0x1_0000)
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
    if exe.contains(['"', '\\', '$', '`']) {
        // 따옴표를 깨는 경로는 아예 안 쓴다. PATH 에 기대는 편이 낫다.
        return format!("command -v moai >/dev/null 2>&1 && moai hook {event} || exit 0");
    }
    format!("[ -x \"{exe}\" ] && \"{exe}\" hook {event} || exit 0")
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
    author: Option<&str>,
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
    files.push((PathBuf::from(".claude-plugin/plugin.json"), plugin_json(exe, author, &version)));
    files.push((
        PathBuf::from(".claude-plugin/marketplace.json"),
        marketplace_json(prefix, root),
    ));
    files
}

fn version_of(files: &[(PathBuf, String)], exe: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for (path, body) in files {
        path.hash(&mut h);
        body.hash(&mut h);
    }
    exe.hash(&mut h);
    HOOKS.hash(&mut h);
    WATCHED.hash(&mut h);
    // semver 세 자리에 나눠 담는다. `claude` 가 판을 semver 로 읽는다.
    let n = h.finish();
    format!("{}.{}.{}", n % 1000, (n / 1000) % 1000, (n / 1_000_000) % 1000)
}

fn plugin_json(exe: &str, author: Option<&str>, version: &str) -> String {
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
    let mut manifest = serde_json::json!({
        "name": "moai",
        "description": "이 저장소의 이슈 트래커. 보드를 세션에 싣고, 새 이슈가 집고 있는 단위 밖으로 새는 것을 막는다.",
        "version": version,
        "hooks": hooks,
    });
    if let Some(who) = author {
        manifest["author"] = serde_json::json!({ "name": who });
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn tree_of(exe: &str, skill: &str) -> BTreeMap<String, String> {
        tree_for("t", exe, skill)
    }

    fn tree_for(prefix: &str, exe: &str, skill: &str) -> BTreeMap<String, String> {
        tree_at(prefix, Path::new("/repo"), exe, skill)
    }

    fn tree_at(prefix: &str, root: &Path, exe: &str, skill: &str) -> BTreeMap<String, String> {
        tree(prefix, root, exe, Some("레이븐 (raven@example.com)"), skill, "참고")
            .into_iter()
            .map(|(p, b)| (p.display().to_string(), b))
            .collect()
    }

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
        assert!(!cmd.contains('"'), "{cmd}");
        assert!(cmd.contains("command -v moai"), "{cmd}");
    }
}
