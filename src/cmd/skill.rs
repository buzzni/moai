//! `.claude/` 에 플러그인을 심고 `claude` 에 등록한다.
//!
//! **지우지 않는다.** 심은 것을 고칠 때도 덮어쓰기만 한다 — 돌고 있는 세션이
//! 물고 있는 훅 파일을 지우면 그 세션의 도구 호출이 전부 막힌다. 실제로 한 번
//! 그렇게 잠겼고, 껍데기를 만들 도구조차 그 훅에 막혀 세션을 다시 여는 것
//! 말고는 길이 없었다. **밖에서 심은 것을 안에서 걷어내지 않는다.**
//!
//! `claude` 를 못 찾아도 파일은 심는다. 등록만 사람이 한 줄 치면 된다 —
//! 절반을 해 놓고 아무 말 없이 실패하는 것이 제일 나쁘다.

use super::{Ctx, Fail, R};
use crate::skill;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn install(ctx: &Ctx, scope: &str, dry_run: bool) -> R<Vec<String>> {
    let repo = crate::store::Repo::discover()?;
    let root = repo.root.clone();
    let dir = root.join(skill::DIR);

    let exe = std::env::current_exe().map_err(|e| Fail::new(e.to_string()))?;
    let exe = skill::exe_name(&exe, on_path().as_deref());
    let market = skill::market(&repo.config.prefix, &root);
    // **누구인지 묻지 않는다.** 심는 것은 이력이 남는 일이 아니라 설정이다.
    let files = skill::tree(
        &repo.config.prefix,
        &root,
        &exe,
        &crate::guide::skill(),
        &crate::guide::reference(),
    );

    if dry_run {
        if ctx.json {
            return super::json_line(&serde_json::json!({
                "dir": dir.display().to_string(),
                "market": market,
                "scope": scope,
                "exe": exe,
                "dry_run": true,
                "files": files.iter().map(|(p, _)| p.display().to_string()).collect::<Vec<_>>(),
            }));
        }
        let mut out = vec![format!("심을 것 — {}", dir.display())];
        for (path, body) in &files {
            out.push(format!("  {:<44} {}줄", path.display(), body.lines().count()));
        }
        out.push(String::new());
        out.push(format!("등록: claude plugin install moai@{market} --scope {scope}"));
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

    // **같은 이름이 남의 저장소를 가리키면 등록하지 않는다.** 덮어쓰면 그
    // 저장소의 규칙이 이쪽에 걸린다 — 조용히 엉뚱해지는 쪽이라 더 나쁘다.
    let clash = known_at(&market).filter(|other| !same_dir(other, &dir));
    let steps = match &clash {
        Some(other) => vec![(
            format!("`{market}` 이 이미 {} 를 가리킨다 — 등록은 건너뛴다", other.display()),
            false,
        )],
        None => register(&dir, &market, scope, known_at(&market).is_some()),
    };

    if ctx.json {
        return super::json_line(&serde_json::json!({
            "dry_run": false,
            "dir": dir.display().to_string(),
            "market": market,
            "scope": scope,
            "exe": exe,
            "files": files.iter().map(|(p, _)| p.display().to_string()).collect::<Vec<_>>(),
            "registered": steps.iter().all(|(_, ok)| *ok),
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
    if steps.iter().all(|(_, ok)| *ok) {
        out.push(String::new());
        out.push("Claude 를 다시 열면 든다. 이미 열려 있는 세션은 옛 판을 계속 쓴다".into());
    } else if clash.is_some() {
        // **하지 말라던 것을 일러 주지 않는다.** 여기서 `marketplace add` 를
        // 내면, 시킨 대로 한 사람이 남의 저장소 등록을 이쪽으로 돌려놓는다 —
        // 이 가드가 막으려던 바로 그 일이다.
        out.push(String::new());
        out.push(format!("그 저장소를 이제 안 쓰면 `claude plugin marketplace remove {market}` 뒤에"));
        out.push("다시 부른다. 둘 다 쓰면 한쪽은 `--scope user` 로 심는다".into());
    } else {
        out.push(String::new());
        out.push("등록은 손으로 마친다:".into());
        out.push(format!("  claude plugin marketplace add ./{} --scope {scope}", skill::DIR));
        out.push(format!("  claude plugin install moai@{market} --scope {scope} -y"));
    }
    Ok(out)
}

/// PATH 에서 찾아지는 `moai`. 훅에 이름을 적어도 되는지를 이것이 정한다.
fn on_path() -> Option<PathBuf> {
    let out = Command::new("sh").arg("-c").arg("command -v moai").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let found = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!found.is_empty()).then(|| std::fs::canonicalize(&found).unwrap_or_else(|_| found.into()))
}

/// 이 이름의 마켓플레이스가 이미 가리키고 있는 자리.
///
/// `claude` 의 장부를 **읽기만** 한다. 쓰는 것은 `claude` 의 몫이다.
fn known_at(market: &str) -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let path = Path::new(&home).join(".claude/plugins/known_marketplaces.json");
    let text = std::fs::read_to_string(path).ok()?;
    let all: serde_json::Value = serde_json::from_str(&text).ok()?;
    let at = all.get(market)?.get("installLocation")?.as_str()?;
    Some(PathBuf::from(at))
}

fn same_dir(a: &Path, b: &Path) -> bool {
    let real = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    real(a) == real(b)
}

/// `claude` 에 등록한다. **사람의 `settings.json` 은 우리가 안 건드린다** —
/// `claude` 가 제 손으로 두 키만 넣는다.
fn register(dir: &Path, market: &str, scope: &str, listed: bool) -> Vec<(String, bool)> {
    let mut steps = Vec::new();
    if !listed {
        steps.push((
            "마켓플레이스를 알렸다".to_string(),
            run(&["plugin", "marketplace", "add", &dir.display().to_string(), "--scope", scope]),
        ));
    } else {
        steps.push((
            "마켓플레이스를 다시 읽혔다".to_string(),
            run(&["plugin", "marketplace", "update", market]),
        ));
    }
    let target = format!("moai@{market}");
    // 이미 심긴 곳에서는 `install` 이 아무것도 안 한다. 판이 바뀌었을 때
    // 새 복사를 뜨는 것은 `update` 뿐이라 둘 다 부른다.
    let installed = run(&["plugin", "install", &target, "--scope", scope, "-y"]);
    // **`-y` 를 준다.** `claude` 는 stdout 이 TTY 가 아니면 그것을 요구하고,
    // 여기서는 언제나 파이프다 — 없으면 이 갈래가 늘 실패한다.
    let updated = run(&["plugin", "update", &target, "--scope", scope, "-y"]);
    steps.push(("플러그인을 등록했다".to_string(), installed || updated));
    steps
}

fn run(args: &[&str]) -> bool {
    Command::new("claude").args(args).output().map(|o| o.status.success()).unwrap_or(false)
}
