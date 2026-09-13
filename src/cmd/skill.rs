//! `.claude/` 에 플러그인을 심고 `claude` 에 등록한다. 심은 것을 보이고 걷어낸다.
//!
//! **지우지 않는다.** 심은 것을 고칠 때도 덮어쓰기만 한다 — 돌고 있는 세션이
//! 물고 있는 훅 파일을 지우면 그 세션의 도구 호출이 전부 막힌다. 실제로 한 번
//! 그렇게 잠겼고, 껍데기를 만들 도구조차 그 훅에 막혀 세션을 다시 여는 것
//! 말고는 길이 없었다. **밖에서 심은 것을 안에서 걷어내지 않는다** — 걷어낼
//! 때도 `claude` 의 등록만 걷고 파일은 남긴다.
//!
//! `claude` 를 못 찾아도 파일은 심는다. 등록만 사람이 한 줄 치면 된다 —
//! 절반을 해 놓고 아무 말 없이 실패하는 것이 제일 나쁘다.

use super::{Ctx, Fail, R};
use crate::skill;
use std::path::{Path, PathBuf};
use std::process::Command;

/// 세 명령이 같이 보는 자리.
struct Place {
    root: PathBuf,
    dir: PathBuf,
    market: String,
    prefix: String,
    exe: String,
    /// PATH 에서 찾아지는 `moai` — 훅이 이름으로 적혔을 때 실제로 불리는 것.
    on_path: Option<PathBuf>,
    files: Vec<(PathBuf, String)>,
}

fn place() -> R<Place> {
    let repo = crate::store::Repo::discover()?;
    let root = repo.root.clone();
    let exe = std::env::current_exe().map_err(|e| Fail::new(e.to_string()))?;
    let on_path = which("moai");
    let exe = skill::exe_name(&exe, on_path.as_deref());
    let prefix = repo.config.prefix.clone();
    // **누구인지 묻지 않는다.** 심는 것은 이력이 남는 일이 아니라 설정이다.
    let files = plant(&prefix, &root, &exe);
    Ok(Place {
        dir: root.join(skill::DIR),
        market: skill::market(&prefix, &root),
        root,
        prefix,
        exe,
        on_path,
        files,
    })
}

/// 훅에 이 실행 파일을 적었을 때 심을 트리.
fn plant(prefix: &str, root: &Path, exe: &str) -> Vec<(PathBuf, String)> {
    skill::tree(prefix, root, exe, &crate::guide::skill(), &crate::guide::reference())
}

pub fn install(ctx: &Ctx, scope: &str, dry_run: bool) -> R<Vec<String>> {
    let Place { root, dir, market, exe, files, .. } = place()?;
    // **같은 이름이 남의 저장소를 가리키면 등록하지 않는다.** 덮어쓰면 그
    // 저장소의 규칙이 이쪽에 걸린다 — 조용히 엉뚱해지는 쪽이라 더 나쁘다.
    // 연습도 같은 답을 낸다. 진짜 실행이 건너뛸 등록을 연습이 약속하면 안 된다.
    let clash = clash_of(&market, &dir);

    if dry_run {
        if ctx.json {
            return super::json_line(&serde_json::json!({
                "dir": dir.display().to_string(),
                "market": market,
                "scope": scope,
                "exe": exe,
                "dry_run": true,
                "files": files.iter().map(|(p, _)| p.display().to_string()).collect::<Vec<_>>(),
                "blocked_by": clash.as_ref().map(|p| p.display().to_string()),
            }));
        }
        let mut out = vec![format!("심을 것 — {}", dir.display())];
        for (path, body) in &files {
            out.push(format!("  {:<44} {}줄", path.display(), body.lines().count()));
        }
        out.push(String::new());
        out.push(match &clash {
            Some(other) => format!("등록: 건너뛴다 — `{market}` 이 이미 {} 를 가리킨다", other.display()),
            None => format!("등록: claude plugin install moai@{market} --scope {scope} -y"),
        });
        return Ok(out);
    }

    // **덮어쓰기만 한다.** 남은 파일을 치우는 것은 다음 설치의 몫이다.
    for (path, body) in &files {
        let at = dir.join(path);
        if let Some(parent) = at.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Fail::new(format!("{}: {e}", parent.display())))?;
        }
        std::fs::write(&at, body).map_err(|e| Fail::new(format!("{}: {e}", at.display())))?;
    }

    let steps = match &clash {
        Some(other) => vec![(
            format!("`{market}` 이 이미 {} 를 가리킨다 — 등록은 건너뛴다", other.display()),
            false,
        )],
        None => register(&root, &dir, &market, scope, known_at(&market).is_some()),
    };

    // **등록이 안 됐으면 비영으로 끝낸다.** 파일은 심었어도 훅은 안 선다 —
    // `uninstall` 이 같은 반쪽 상태를 실패로 끝내는 것과 같은 셈이다. 종료 코드만
    // 보는 쪽이 "심었다" 로 읽으면 규칙 없이 세션이 돈다.
    let registered = steps.iter().all(|(_, ok)| *ok);
    if !registered {
        super::note_partial();
    }

    if ctx.json {
        return super::json_line(&serde_json::json!({
            "dry_run": false,
            "dir": dir.display().to_string(),
            "market": market,
            "scope": scope,
            "exe": exe,
            "files": files.iter().map(|(p, _)| p.display().to_string()).collect::<Vec<_>>(),
            "registered": registered,
            // **못 한 까닭을 기계에도 준다.** 사람 출력에만 적어 두면 스크립트는
            // `registered: false` 만 보고 무엇을 해야 할지 모른다.
            "blocked_by": clash.as_ref().map(|p| p.display().to_string()),
        }));
    }

    let mut out = vec![format!("{} 에 심었다", dir.display())];
    out.push(format!("  훅이 부를 것: {exe} hook <event>"));
    for (what, ok) in &steps {
        out.push(format!("  {} {what}", if *ok { "·" } else { "!" }));
    }
    if registered {
        out.push(String::new());
        out.push("Claude 를 다시 열면 든다. 이미 열려 있는 세션은 옛 판을 계속 쓴다".into());
    } else if clash.is_some() {
        // **하지 말라던 것을 일러 주지 않는다.** 여기서 `marketplace add` 를
        // 내면, 시킨 대로 한 사람이 남의 저장소 등록을 이쪽으로 돌려놓는다 —
        // 이 가드가 막으려던 바로 그 일이다.
        out.push(String::new());
        // `--scope` 를 바꿔 보라고 하지 않는다 — 이름은 기계 하나에서 전역이라,
        // 어느 범위로 심어도 같은 자리에서 걸려 아무것도 안 바뀐다.
        out.push(format!("그 저장소를 이제 안 쓰면 `claude plugin marketplace remove {market}` 뒤에"));
        out.push("다시 부른다. 한 이름을 두 자리가 함께 쓸 수는 없다".into());
    } else {
        out.push(String::new());
        out.push("등록은 손으로 마친다:".into());
        // **절대 경로를 낸다.** 저장소 뿌리를 기준으로 한 `./.claude/moai-plugin` 은
        // 하위 디렉터리에서 부른 사람이 그 자리에서 치면 없는 디렉터리를 가리킨다.
        out.push(format!("  claude plugin marketplace add {} --scope {scope}", dir.display()));
        out.push(format!("  claude plugin install moai@{market} --scope {scope} -y"));
    }
    Ok(out)
}

/// 무엇이 어디 심겼는지 보인다. **읽기만 하고, 무엇이 어긋나도 0 이다.**
///
/// 어긋남을 비영 종료로 알리면 에이전트가 이것을 "실패" 로 읽는다 —
/// `moai status` 가 아무것도 막지 않는 것과 같은 까닭이다.
pub fn status(ctx: &Ctx) -> R<Vec<String>> {
    let Place { root, dir, market, prefix, exe, on_path, files } = place()?;
    let want = skill::version_in(&files).unwrap_or_default();
    let listed = known_at(&market);
    let clash = listed.clone().filter(|other| !same_dir(other, &dir));
    let installs = installs_here(&market, &root);
    let claude = which("claude").is_some();
    // 실제로 불리는 것은 `claude` 가 복사해 간 매니페스트다. **복사본이 사라진
    // 설치**(캐시를 치운 뒤)는 훅이 아예 안 실리므로, 판이 맞아도 따로 짚는다.
    let copies: Vec<Option<String>> = installs
        .iter()
        .map(|i| std::fs::read_to_string(Path::new(&i.install_path).join(".claude-plugin/plugin.json")).ok())
        .collect();
    let hooks: Vec<Option<String>> = copies.iter().map(|m| m.as_deref().and_then(skill::hook_exe)).collect();
    // **판은 그 설치의 훅이 부르는 실행 파일로 견준다.** 판에는 훅 명령이 들어가,
    // 지금 부른 moai 의 자리로 견주면 내용이 같아도 판이 달라 보인다 — 복사본이나
    // `cargo run` 으로 부른 것만으로 "다시 심는다" 가 떴고, 시킨 대로 하면 훅이 그
    // 복사본을 부르게 바뀌었다.
    let wants: Vec<String> = hooks
        .iter()
        .map(|h| match h.as_deref() {
            Some(h) if h != exe => skill::version_in(&plant(&prefix, &root, h)).unwrap_or_default(),
            _ => want.clone(),
        })
        .collect();
    let hooked = hooks.iter().flatten().next().cloned();
    let hook_path = hooked.as_deref().and_then(|h| runs(h, on_path.as_deref()));
    let stale = stale_copies(&installs);

    if ctx.json {
        let rows: Vec<_> = installs
            .iter()
            .zip(copies.iter().zip(&wants))
            .map(|(i, (copy, want))| {
                serde_json::json!({
                    "scope": i.scope,
                    "version": i.version,
                    "project": i.project,
                    "install_path": i.install_path,
                    "current": i.version == *want,
                    "copy_found": copy.is_some(),
                })
            })
            .collect();
        return super::json_line(&serde_json::json!({
            "dir": dir.display().to_string(),
            "written": dir.join(".claude-plugin/plugin.json").is_file(),
            "market": market,
            "listed": listed.as_ref().map(|p| p.display().to_string()),
            "blocked_by": clash.as_ref().map(|p| p.display().to_string()),
            "want_version": want,
            "exe": exe,
            "installs": rows,
            "hook_exe": hooked,
            "hook_exe_found": hooked.as_ref().map(|_| hook_path.is_some()),
            "hook_exe_path": hook_path.as_ref().map(|p| p.display().to_string()),
            "stale_copies": stale,
            "claude": claude,
        }));
    }

    let mark = |ok: bool| if ok { "·" } else { "!" };
    let mut out = vec![format!("moai skill — {}", dir.display())];
    out.push(match (&listed, &clash) {
        (_, Some(other)) => format!(
            "  ! 마켓플레이스  `{market}` 이 다른 자리({}) 를 가리킨다",
            other.display()
        ),
        (Some(_), None) => format!("  · 마켓플레이스  `{market}` 등록됨"),
        (None, _) => format!("  ! 마켓플레이스  `{market}` 등록 안 됨"),
    });
    // **등록이 남의 자리를 가리키면 다시 심으라고 하지 않는다.** `install` 은 그때
    // 등록을 건너뛰어, 시킨 대로 해도 아무것도 안 바뀐다 — 어느 줄에서 일러 주든
    // 같은 덫이다. 빠져나갈 길은 이 한 줄에만 댄다.
    if clash.is_some() {
        out.push(format!(
            "    그 자리를 이제 안 쓰면 `claude plugin marketplace remove {market}` 뒤에 `moai skill install`"
        ));
    }
    if installs.is_empty() {
        out.push(if clash.is_some() {
            "  ! 설치          없음".into()
        } else {
            "  ! 설치          없음 — `moai skill install` 로 심는다".into()
        });
    }
    for (i, (copy, want)) in installs.iter().zip(copies.iter().zip(&wants)) {
        let current = i.version == *want;
        let found = copy.is_some();
        let advice = if clash.is_some() { String::new() } else { format!(": moai skill install --scope {}", i.scope) };
        out.push(format!(
            "  {} 설치          {}  판 {}{}",
            mark(current && found),
            i.scope,
            i.version,
            if !found {
                format!("  — 설치본({})이 없다. 훅이 안 실린다{advice}", i.install_path)
            } else if current {
                String::new()
            } else {
                format!("  — 심을 판은 {want}. 다시 심는다{advice}")
            }
        ));
    }
    if let Some(hook) = &hooked {
        out.push(match &hook_path {
            Some(path) if path.as_os_str() == hook.as_str() => format!("  · 훅            {hook}"),
            Some(path) => format!("  · 훅            {hook} → {}", path.display()),
            None => format!("  ! 훅            {hook}  — 없거나 실행할 수 없다. 훅은 조용히 아무것도 안 한다"),
        });
        if *hook != exe {
            out.push(format!(
                "    지금 부른 moai({exe}) 는 훅이 부르는 것과 다르다 — 여기서 심으면 훅이 그것을 부르게 바뀐다"
            ));
        }
    }
    out.push(format!(
        "  {} claude        {}",
        mark(claude),
        if claude { "PATH 에 있다" } else { "PATH 에 없다 — 심을 수도 걷을 수도 없다" }
    ));
    if stale > 0 {
        // **치우지 않는다.** 캐시는 `claude` 의 것이고, 돌고 있는 세션이 그중
        // 하나를 물고 있을 수 있다. 수만 비춘다.
        out.push(format!("    옛 판 {stale}개가 claude 캐시에 남아 있다 (치우는 것은 claude 의 몫)"));
    }
    Ok(out)
}

/// `claude` 에서 이 저장소의 등록을 걷어낸다. **파일은 남긴다.**
pub fn uninstall(ctx: &Ctx, dry_run: bool) -> R<Vec<String>> {
    let Place { root, dir, market, .. } = place()?;
    let clash = clash_of(&market, &dir);
    let installs = installs_here(&market, &root);
    let target = format!("moai@{market}");

    // **남의 등록이면 아무것도 부르지 않는다.** 같은 이름이 다른 저장소를
    // 가리키는데 걷으면, 그 저장소의 규칙이 말없이 사라진다.
    let mut plan: Vec<Vec<String>> = Vec::new();
    if clash.is_none() {
        // **범위마다 한 번만 부른다.** 장부에 같은 범위 줄이 겹치면 같은 걷기를 두
        // 번 부르고, 둘째는 이미 걷힌 것이라 실패한다 — 그러면 아래의 "실패하면
        // 멈춘다" 에 걸려 마켓플레이스가 안 지워진다. 장부는 `claude` 의 것이라
        // 고치지 않고, 읽은 쪽에서 겹침을 걷는다.
        let mut scopes: Vec<&str> = Vec::new();
        for i in &installs {
            if !scopes.contains(&i.scope.as_str()) {
                scopes.push(&i.scope);
            }
        }
        for scope in scopes {
            plan.push(argv(&["plugin", "uninstall", &target, "--scope", scope]));
        }
        if known_at(&market).is_some() {
            // 범위를 안 주면 모든 범위에서 걷는다.
            plan.push(argv(&["plugin", "marketplace", "remove", &market]));
        }
    }
    let claude = which("claude").is_some();
    let mut steps: Vec<(String, bool)> = Vec::new();
    if !dry_run && claude {
        for a in &plan {
            let ok = run(&root, a);
            steps.push((shown(a), ok));
            // **실패하면 멈춘다.** 플러그인을 못 걷은 채 마켓플레이스를 지우면
            // 걷을 이름이 사라진 설치가 남고, 그때는 도구로 되돌릴 길이 없다.
            if !ok {
                break;
            }
        }
    }
    let failed = steps.iter().any(|(_, ok)| !ok)
        || clash.is_some()
        || (!dry_run && !claude && !plan.is_empty());
    if failed {
        super::note_partial();
    }

    if ctx.json {
        return super::json_line(&serde_json::json!({
            "dry_run": dry_run,
            "dir": dir.display().to_string(),
            "market": market,
            "planned": plan.iter().map(|a| shown(a)).collect::<Vec<_>>(),
            "steps": steps.iter().map(|(c, ok)| serde_json::json!({"command": c, "ok": ok})).collect::<Vec<_>>(),
            "removed": !dry_run && !failed && !plan.is_empty(),
            "blocked_by": clash.as_ref().map(|p| p.display().to_string()),
            "claude": claude,
        }));
    }

    if let Some(other) = &clash {
        return Ok(vec![
            format!("! `{market}` 은 다른 자리({}) 의 등록이다 — 건드리지 않는다", other.display()),
            "  그 저장소에서 `moai skill uninstall` 을 부른다".into(),
        ]);
    }
    if plan.is_empty() {
        return Ok(vec![format!("걷어낼 것이 없다 — `{market}` 은 claude 에 등록돼 있지 않다")]);
    }
    if dry_run || !claude {
        let mut out = vec![if dry_run {
            "부를 것:".to_string()
        } else {
            "! claude 를 PATH 에서 못 찾았다. 손으로 걷는다:".to_string()
        }];
        out.extend(plan.iter().map(|a| format!("  {}", shown(a))));
        return Ok(out);
    }
    // 한 걸음이라도 실패했으면 "걷었다" 고 말하지 않는다 — 종료 코드만 비영이고
    // 첫 줄이 성공이면 사람은 첫 줄을 믿는다.
    let mut out = vec![if failed {
        format!("! `{market}` 을 다 걷지 못했다")
    } else {
        format!("`{market}` 을 걷었다")
    }];
    for (cmd, ok) in &steps {
        out.push(format!("  {} {cmd}{}", if *ok { "·" } else { "!" }, if *ok { "" } else { "  — 실패" }));
    }
    for a in &plan[steps.len()..] {
        out.push(format!("  - {}  — 앞 걸음이 실패해 안 불렀다", shown(a)));
    }
    out.push(String::new());
    out.push("이미 열려 있는 Claude 세션은 옛 훅을 계속 부른다 — 다시 열어야 완전히 걷힌다".into());
    out.push(format!(
        "심은 파일({}/)은 남긴다. 돌고 있는 세션이 물고 있을 수 있다 — 세션을 닫은 뒤 지워도 된다",
        skill::DIR
    ));
    Ok(out)
}

fn argv(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| s.to_string()).collect()
}

fn shown(args: &[String]) -> String {
    format!("claude {}", args.join(" "))
}

/// PATH 에서 찾아지는 그 이름. 훅에 이름을 적어도 되는지, `claude` 를 부를 수
/// 있는지를 이것이 정한다.
fn which(name: &str) -> Option<PathBuf> {
    let out = Command::new("sh")
        .args(["-c", "command -v -- \"$1\"", "sh", name])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let found = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!found.is_empty()).then(|| std::fs::canonicalize(&found).unwrap_or_else(|_| found.into()))
}

/// 훅이 부르는 것이 **실제로 도는 파일.** 이름이면 PATH 에서 찾은 것이고, 경로면
/// 실행할 수 있을 때만 그 자리다 — 훅 명령의 `command -v … && "<exe>"` 와 같은 셈.
///
/// `is_file` 만 보던 판은 실행 권한이 빠진 파일을 "있다" 고 했다. 훅은 그때 권한
/// 오류를 `|| exit 0` 으로 삼켜 아무 말 없이 아무것도 안 한다. 이름으로 적힌 훅은
/// 찾은 자리를 **함께 보인다** — PATH 의 `moai` 가 남의 moai 여도 훅은 돈다.
fn runs(exe: &str, on_path: Option<&Path>) -> Option<PathBuf> {
    if exe.contains('/') {
        let path = Path::new(exe);
        runnable(path).then(|| path.to_path_buf())
    } else if exe == "moai" {
        on_path.map(Path::to_path_buf)
    } else {
        which(exe)
    }
}

fn runnable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

/// `claude` 가 플러그인 장부를 두는 자리. **`claude` 가 보는 곳을 그대로 본다** —
/// `CLAUDE_CODE_PLUGIN_CACHE_DIR`, 없으면 `CLAUDE_CONFIG_DIR/plugins`, 없으면
/// `~/.claude/plugins`. `HOME` 만 보던 판은 설정 디렉터리를 옮긴 사람에게 "걷어낼
/// 것이 없다" 고 말하고 0 으로 끝났다 — 훅은 그대로 살아 있는데.
fn plugins_dir() -> Option<PathBuf> {
    let set = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    set("CLAUDE_CODE_PLUGIN_CACHE_DIR")
        .or_else(|| set("CLAUDE_CONFIG_DIR").map(|d| d.join("plugins")))
        .or_else(|| set("HOME").map(|h| h.join(".claude/plugins")))
}

/// `claude` 의 장부 하나. **읽기만** 한다. 쓰는 것은 `claude` 의 몫이다.
fn ledger(name: &str) -> Option<serde_json::Value> {
    let text = std::fs::read_to_string(plugins_dir()?.join(name)).ok()?;
    serde_json::from_str(&text).ok()
}

/// 이 이름의 마켓플레이스가 이미 가리키고 있는 자리.
fn known_at(market: &str) -> Option<PathBuf> {
    let all = ledger("known_marketplaces.json")?;
    let at = all.get(market)?.get("installLocation")?.as_str()?;
    Some(PathBuf::from(at))
}

/// 같은 이름이 **남의 자리**를 가리키면 그 자리.
fn clash_of(market: &str, dir: &Path) -> Option<PathBuf> {
    known_at(market).filter(|other| !same_dir(other, dir))
}

fn installs_here(market: &str, root: &Path) -> Vec<skill::Install> {
    ledger("installed_plugins.json")
        .map(|l| skill::installs(&l, market, |p| same_dir(Path::new(p), root)))
        .unwrap_or_default()
}

/// 설치본 곁에 남은 옛 판 디렉터리의 수.
fn stale_copies(installs: &[skill::Install]) -> usize {
    let Some(parent) = installs
        .first()
        .and_then(|i| Path::new(&i.install_path).parent().map(Path::to_path_buf))
    else {
        return 0;
    };
    let live: Vec<&str> = installs.iter().map(|i| i.version.as_str()).collect();
    std::fs::read_dir(parent)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .filter(|e| !live.contains(&e.file_name().to_string_lossy().as_ref()))
                .count()
        })
        .unwrap_or(0)
}

fn same_dir(a: &Path, b: &Path) -> bool {
    let real = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    real(a) == real(b)
}

/// `claude` 에 등록한다. **사람의 `settings.json` 은 우리가 안 건드린다** —
/// `claude` 가 제 손으로 두 키만 넣는다.
fn register(root: &Path, dir: &Path, market: &str, scope: &str, listed: bool) -> Vec<(String, bool)> {
    let mut steps = Vec::new();
    let dir = dir.display().to_string();
    if !listed {
        steps.push((
            "마켓플레이스를 알렸다".to_string(),
            run(root, &argv(&["plugin", "marketplace", "add", &dir, "--scope", scope])),
        ));
    } else {
        steps.push((
            "마켓플레이스를 다시 읽혔다".to_string(),
            run(root, &argv(&["plugin", "marketplace", "update", market])),
        ));
    }
    let target = format!("moai@{market}");
    // 이미 심긴 곳에서는 `install` 이 아무것도 안 한다. 판이 바뀌었을 때
    // 새 복사를 뜨는 것은 `update` 뿐이라 둘 다 부른다.
    let installed = run(root, &argv(&["plugin", "install", &target, "--scope", scope, "-y"]));
    // **`-y` 를 준다.** `claude` 는 stdout 이 TTY 가 아니면 그것을 요구하고,
    // 여기서는 언제나 파이프다 — 없으면 이 갈래가 늘 실패한다.
    let updated = run(root, &argv(&["plugin", "update", &target, "--scope", scope, "-y"]));
    steps.push(("플러그인을 등록했다".to_string(), installed || updated));
    steps
}

/// `claude` 를 **저장소 뿌리에서** 부른다. `local`·`project` 범위는 `claude` 가
/// 제 자리로 프로젝트를 정하므로, 하위 디렉터리에서 부르면 엉뚱한 프로젝트에
/// 적거나 못 찾는다.
fn run(root: &Path, args: &[String]) -> bool {
    Command::new("claude")
        .args(args)
        .current_dir(root)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
