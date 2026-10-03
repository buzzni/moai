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
use crate::i18n::{fill, say};
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

fn place(ctx: &Ctx) -> R<Place> {
    let repo = super::open_repo(ctx)?;
    // **선 체크아웃에 심는다**(리뷰 moai-71ht.jlh) — 트래커만 루트로 옮겨 간다(`Repo::here`).
    // `repo.root` 로 심던 판은 워크트리에서 친 `skill install` 이 루트의 `.claude/` 를 고쳐,
    // 이 가지에서 고친 훅은 이 가지에서 한 번도 안 돌고 남의 체크아웃만 더럽혔다.
    let root = repo.here().to_path_buf();
    let exe = std::env::current_exe().map_err(|e| Fail::new(e.to_string()))?;
    let on_path = which("moai");
    let exe = skill::exe_name(&exe, on_path.as_deref());
    let prefix = repo.config.prefix.clone();
    // **누구인지 묻지 않는다.** 심는 것은 이력이 남는 일이 아니라 설정이다.
    let files = plant(&prefix, &root, &exe);
    Ok(Place { dir: root.join(skill::DIR), market: skill::market(&prefix, &root), root, prefix, exe, on_path, files })
}

/// 훅에 이 실행 파일을 적었을 때 심을 트리.
fn plant(prefix: &str, root: &Path, exe: &str) -> Vec<(PathBuf, String)> {
    skill::tree(prefix, root, exe, &crate::guide::skill(), &crate::guide::reference(), &crate::guide::supervise())
}

pub fn install(ctx: &Ctx, scope: &str, dry_run: bool) -> R<Vec<String>> {
    let Place { root, dir, market, exe, files, .. } = place(ctx)?;
    // **같은 이름이 남의 저장소를 가리키면 등록하지 않는다.** 덮어쓰면 그
    // 저장소의 규칙이 이쪽에 걸린다 — 조용히 엉뚱해지는 쪽이라 더 나쁘다.
    // 연습도 같은 답을 낸다. 진짜 실행이 건너뛸 등록을 연습이 약속하면 안 된다.
    let clash = clash_of(&market, &dir);
    // **옛 판이 곁에 깐 것을 걷는다**(moai-vtfu). 범위는 등록하기 **전의** 장부로 잰다 — 옛 moai 가 서
    // 있던 자리가 그것을 함께 깐 자리다. 등록 뒤에 재면 이번 `--scope` 로 처음 서는 범위가 섞여, 사람이 그
    // 범위에 손으로 깐 것까지 이번 판에 걷는다. **막는 것은 이번 판뿐이다** — 다음 `install` 에서는 그
    // 범위에도 moai 가 서 있어, 거기 선 두 플러그인을 누가 깔았든 moai 의 것으로 읽고 걷는다([`retire`] 의
    // 대리). 이름이 남의 저장소를 가리키면 `uninstall` 처럼 아무것도 안 부른다.
    let target = format!("moai@{market}");
    let mut retiring = if clash.is_none() {
        retire(&root, &target, &scopes_of(&installs_here(&target, &root)))
    } else {
        Retired::default()
    };

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
                "retired": retiring.json(),
                "kept": retiring.kept,
            }));
        }
        let lang = ctx.lang();
        let mut out = vec![fill(say(lang, "skill.plan_head"), &[("dir", &dir.display().to_string())])];
        for (path, body) in &files {
            let n = body.lines().count().to_string();
            out.push(format!("  {:<44} {}", path.display(), fill(say(lang, "skill.plan_lines"), &[("n", &n)])));
        }
        out.push(String::new());
        out.push(match &clash {
            Some(other) => fill(
                say(lang, "skill.plan_register_skipped"),
                &[("market", &market), ("at", &other.display().to_string())],
            ),
            None => fill(
                say(lang, "skill.plan_register"),
                &[("cmd", &format!("claude plugin install moai@{market} --scope {scope} -y"))],
            ),
        });
        for step in &retiring.steps {
            out.push(fill(say(lang, "skill.plan_retire"), &[("cmd", &step.shown())]));
        }
        out.extend(retiring.kept_lines(lang));
        return Ok(out);
    }

    // **덮어쓰기만 한다.** 남은 파일을 치우는 것은 다음 설치의 몫이다.
    for (path, body) in &files {
        let at = dir.join(path);
        if let Some(parent) = at.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Fail::new(format!("{}: {e}", parent.display())))?;
        }
        std::fs::write(&at, body).map_err(|e| Fail::new(format!("{}: {e}", at.display())))?;
    }

    // **`--json` 보다 먼저 푼다** — 아래 두 갈래가 다 이것을 쓴다. 기계 출력으로 빠지는 판은
    // 이 글들을 안 지으므로 값이 새지 않는다(`register` 의 걸음 이름은 사람 화면에만 선다).
    let lang = ctx.lang();
    let steps = match &clash {
        Some(other) => vec![(
            fill(say(lang, "skill.market_taken"), &[("market", &market), ("at", &other.display().to_string())]),
            false,
        )],
        None => register(lang, &root, &dir, &market, scope, known_at(&market).is_some()),
    };

    // **등록이 안 됐으면 비영으로 끝낸다.** 파일은 심었어도 훅은 안 선다 —
    // `uninstall` 이 같은 반쪽 상태를 실패로 끝내는 것과 같은 셈이다. 종료 코드만
    // 보는 쪽이 "심었다" 로 읽으면 규칙 없이 세션이 돈다.
    let registered = steps.iter().all(|(_, ok)| *ok);
    if !registered {
        super::note_partial();
    }
    // **옛 판이 곁에 깐 것을 걷는 일은 등록과 따로 센다**(사용자 결정, moai-vtfu.dvk). 못 걷어도 moai 의
    // 훅은 선다 — 종료 코드는 등록 결과만 따른다. 하나가 실패해도 다음 것을 부르고, 못 걷은 것은 손으로 칠
    // 줄로 낸다. 다시 부르면 같은 장부로 범위를 재어 남은 것을 또 걷는다.
    retiring.call(&root);

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
            "retired": retiring.json(),
            "kept": retiring.kept,
        }));
    }

    let mut out = vec![fill(say(lang, "skill.planted"), &[("dir", &dir.display().to_string())])];
    out.push(fill(say(lang, "skill.hook_call"), &[("cmd", &format!("{exe} hook <event>"))]));
    for (what, ok) in &steps {
        out.push(format!("  {} {what}", if *ok { "·" } else { "!" }));
    }
    for step in &retiring.steps {
        out.push(match step.ok {
            Some(true) => fill(say(lang, "skill.retired"), &[("id", step.id), ("scope", &step.scope)]),
            _ => fill(say(lang, "skill.retire_failed"), &[("id", step.id), ("cmd", &step.shown())]),
        });
    }
    out.extend(retiring.kept_lines(lang));
    if registered {
        out.push(String::new());
        out.push(say(lang, "skill.reopen_claude").to_string());
    } else if clash.is_some() {
        // **하지 말라던 것을 일러 주지 않는다.** 여기서 `marketplace add` 를
        // 내면, 시킨 대로 한 사람이 남의 저장소 등록을 이쪽으로 돌려놓는다 —
        // 이 가드가 막으려던 바로 그 일이다.
        out.push(String::new());
        // `--scope` 를 바꿔 보라고 하지 않는다 — 이름은 기계 하나에서 전역이라,
        // 어느 범위로 심어도 같은 자리에서 걸려 아무것도 안 바뀐다.
        out.push(fill(say(lang, "skill.market_drop_first"), &[("market", &market)]));
        out.push(say(lang, "skill.market_one_place").to_string());
    } else {
        out.push(String::new());
        out.push(say(lang, "skill.register_by_hand").to_string());
        // **절대 경로를 낸다.** 저장소 뿌리를 기준으로 한 `./.claude/moai-plugin` 은
        // 하위 디렉터리에서 부른 사람이 그 자리에서 치면 없는 디렉터리를 가리킨다.
        // 빈칸 든 경로를 그대로 내면 친 줄이 인자 둘로 갈린다 — 따옴표로 싼다.
        out.push(format!(
            "  claude plugin marketplace add {} --scope {scope}",
            crate::text::shell_word(&dir.display().to_string())
        ));
        out.push(format!("  claude plugin install moai@{market} --scope {scope} -y"));
    }
    Ok(out)
}

/// 무엇이 어디 심겼는지 보인다. **읽기만 하고, 무엇이 어긋나도 0 이다.**
///
/// 어긋남을 비영 종료로 알리면 에이전트가 이것을 "실패" 로 읽는다 —
/// `moai status` 가 아무것도 막지 않는 것과 같은 까닭이다.
pub fn status(ctx: &Ctx) -> R<Vec<String>> {
    let Place { root, dir, market, prefix, exe, on_path, files } = place(ctx)?;
    let want = skill::version_in(&files).unwrap_or_default();
    let listed = known_at(&market);
    let clash = listed.clone().filter(|other| !crate::user_config::same_dir(other, &dir));
    let installs = installs_here(&format!("moai@{market}"), &root);
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

    let lang = ctx.lang();
    let mark = |ok: bool| if ok { "·" } else { "!" };
    // 줄 하나 — `  <글리프> <이름 칸><나머지>`. **이름 칸은 표시 폭으로 맞춘다**([`label`]).
    let row = |ok: bool, name: &str, rest: &str| format!("  {} {}{rest}", mark(ok), label(name));
    let mut out = vec![format!("moai skill — {}", dir.display())];
    let market_row = say(lang, "skill.row_market");
    out.push(match (&listed, &clash) {
        (_, Some(other)) => row(
            false,
            market_row,
            &fill(say(lang, "skill.market_elsewhere"), &[("market", &market), ("at", &other.display().to_string())]),
        ),
        (Some(_), None) => row(true, market_row, &fill(say(lang, "skill.market_listed"), &[("market", &market)])),
        (None, _) => row(false, market_row, &fill(say(lang, "skill.market_unlisted"), &[("market", &market)])),
    });
    // **등록이 남의 자리를 가리키면 다시 심으라고 하지 않는다.** `install` 은 그때
    // 등록을 건너뛰어, 시킨 대로 해도 아무것도 안 바뀐다 — 어느 줄에서 일러 주든
    // 같은 덫이다. 빠져나갈 길은 이 한 줄에만 댄다.
    if clash.is_some() {
        out.push(fill(say(lang, "skill.market_escape_here"), &[("market", &market)]));
    }
    let install_row = say(lang, "skill.row_install");
    if installs.is_empty() {
        // 막힌 자리에서는 심으라고 하지 않는다 — 위의 한 줄이 빠져나갈 길을 이미 댔다.
        let said = match clash.is_some() {
            true => say(lang, "skill.no_install"),
            false => say(lang, "skill.no_install_plant"),
        };
        out.push(row(false, install_row, said));
    }
    for (i, (copy, want)) in installs.iter().zip(copies.iter().zip(&wants)) {
        let current = i.version == *want;
        let found = copy.is_some();
        let advice = if clash.is_some() {
            String::new()
        } else {
            fill(say(lang, "skill.install_advice"), &[("scope", &i.scope)])
        };
        let head = fill(say(lang, "skill.install_at"), &[("scope", &i.scope), ("version", &i.version)]);
        let tail = if !found {
            fill(say(lang, "skill.install_copy_gone"), &[("at", &i.install_path), ("advice", &advice)])
        } else if current {
            String::new()
        } else {
            fill(say(lang, "skill.install_stale"), &[("want", want), ("advice", &advice)])
        };
        out.push(row(current && found, install_row, &format!("{head}{tail}")));
    }
    let hook_row = say(lang, "skill.row_hook");
    if let Some(hook) = &hooked {
        out.push(match &hook_path {
            Some(path) if path.as_os_str() == hook.as_str() => row(true, hook_row, hook),
            Some(path) => row(true, hook_row, &format!("{hook} → {}", path.display())),
            None => row(false, hook_row, &format!("{hook}  {}", say(lang, "skill.hook_unrunnable"))),
        });
        if *hook != exe {
            out.push(fill(say(lang, "skill.hook_is_another_moai"), &[("exe", &exe)]));
        }
    }
    let claude_said = match claude {
        true => say(lang, "skill.claude_on_path"),
        false => say(lang, "skill.claude_off_path"),
    };
    out.push(row(claude, "claude", claude_said));
    if stale > 0 {
        // **치우지 않는다.** 캐시는 `claude` 의 것이고, 돌고 있는 세션이 그중
        // 하나를 물고 있을 수 있다. 수만 비춘다.
        out.push(fill(say(lang, "skill.stale_copies"), &[("n", &stale.to_string())]));
    }
    Ok(out)
}

/// 이름 칸을 **표시 폭**으로 맞춘다(moai-uzgp). 한글은 한 글자가 두 칸이라 `{:<14}` 로 맞추면
/// 말마다 이 표가 어긋난다 — `text::width` 가 CLI 표와 탐색기가 함께 쓰는 자다.
fn label(what: &str) -> String {
    const WIDE: usize = 14;
    format!("{what}{}", " ".repeat(WIDE.saturating_sub(crate::text::width(what))))
}

/// `claude` 에서 이 저장소의 등록을 걷어낸다. **파일은 남긴다.**
pub fn uninstall(ctx: &Ctx, dry_run: bool) -> R<Vec<String>> {
    let Place { root, dir, market, .. } = place(ctx)?;
    let clash = clash_of(&market, &dir);
    let target = format!("moai@{market}");
    let installs = installs_here(&target, &root);
    let scopes = scopes_of(&installs);

    // **남의 등록이면 아무것도 부르지 않는다.** 같은 이름이 다른 저장소를
    // 가리키는데 걷으면, 그 저장소의 규칙이 말없이 사라진다.
    let mut plan: Vec<Vec<String>> = Vec::new();
    // `plan` 의 앞 몇 걸음이 moai 플러그인을 범위마다 걷는 것인가 — 뒤는 마켓플레이스 지우기다.
    let mut unplug = 0;
    // 옛 판이 곁에 깐 것을 걷는 걸음과, 사용자 범위라 두고 가는 것([`retire`]).
    let mut retiring = Retired::default();
    if clash.is_none() {
        for &scope in &scopes {
            plan.push(argv(&["plugin", "uninstall", &target, "--scope", scope]));
        }
        unplug = plan.len();
        if known_at(&market).is_some() {
            // 범위를 안 주면 모든 범위에서 걷는다.
            plan.push(argv(&["plugin", "marketplace", "remove", &market]));
        }
        // **moai 를 걷는 그 범위로 잰다** — 장부를 다시 읽어 재던 판은 두 읽기 사이에 `claude` 가 장부를
        // 고쳐 쓰면(옆 세션의 `skill install --scope user`) moai 를 안 걷는 범위의 것까지 걷을 수 있었다.
        retiring = retire(&root, &target, &scopes);
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
    // **옛 판이 곁에 깐 것은 moai 의 걸음과 따로 센다**([`install`] 과 같은 셈). moai 플러그인을 다 걷었으면 부르고 —
    // 마켓플레이스 지우기가 실패했어도 부른다. 다시 부르면 moai 의 설치로 범위를 재는데 그때는 그 설치가 이미
    // 없다 — 하나가 실패해도 다음 것을 부르고, 실패는 moai 의 결과를 뒤집지 않는다. 못 걷은 것은 손으로 칠 줄로
    // 낸다. moai 플러그인을 못 걷었으면 안 부른다 — 다시 부르면 남은 moai 설치로 범위를 재어 함께 걷는다.
    let unplugged = !dry_run && claude && steps.len() >= unplug && steps[..unplug].iter().all(|(_, ok)| *ok);
    if unplugged {
        retiring.call(&root);
    }
    let failed = steps.iter().any(|(_, ok)| !ok) || clash.is_some() || (!dry_run && !claude && !plan.is_empty());
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
            "retired": retiring.json(),
            "kept": retiring.kept,
        }));
    }

    let lang = ctx.lang();
    if let Some(other) = &clash {
        return Ok(vec![
            fill(say(lang, "skill.remove_elsewhere"), &[("market", &market), ("at", &other.display().to_string())]),
            say(lang, "skill.remove_elsewhere_there").to_string(),
        ]);
    }
    if plan.is_empty() {
        return Ok(vec![fill(say(lang, "skill.nothing_to_remove"), &[("market", &market)])]);
    }
    if dry_run || !claude {
        let mut out = vec![match dry_run {
            true => say(lang, "skill.plan_calls").to_string(),
            false => say(lang, "skill.no_claude_by_hand").to_string(),
        }];
        out.extend(plan.iter().map(|a| format!("  {}", shown(a))));
        out.extend(retiring.steps.iter().map(|step| format!("  {}", step.shown())));
        out.extend(retiring.kept_lines(lang));
        return Ok(out);
    }
    // 한 걸음이라도 실패했으면 "걷었다" 고 말하지 않는다 — 종료 코드만 비영이고
    // 첫 줄이 성공이면 사람은 첫 줄을 믿는다.
    let mut out = vec![match failed {
        true => fill(say(lang, "skill.removed_partly"), &[("market", &market)]),
        false => fill(say(lang, "skill.removed"), &[("market", &market)]),
    }];
    let failed_tail = say(lang, "skill.step_failed");
    let skipped_tail = say(lang, "skill.step_not_called");
    for (cmd, ok) in &steps {
        out.push(format!("  {} {cmd}{}", if *ok { "·" } else { "!" }, if *ok { "" } else { failed_tail }));
    }
    for a in &plan[steps.len()..] {
        out.push(format!("  - {}{skipped_tail}", shown(a)));
    }
    // 안 부른 걸음(`ok` 가 없다)은 moai 를 다 못 걷어 안 부른 것이다.
    for step in &retiring.steps {
        out.push(match step.ok {
            Some(true) => format!("  · {}", step.shown()),
            Some(false) => format!("  ! {}  {}", step.shown(), say(lang, "skill.step_failed_retry")),
            None => format!("  - {}{skipped_tail}", step.shown()),
        });
    }
    out.extend(retiring.kept_lines(lang));
    out.push(String::new());
    out.push(say(lang, "skill.reopen_to_finish").to_string());
    out.push(fill(say(lang, "skill.files_left"), &[("dir", skill::DIR)]));
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
    let out = Command::new("sh").args(["-c", "command -v -- \"$1\"", "sh", name]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let found = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!found.is_empty()).then(|| crate::path::real(Path::new(&found)))
}

/// 훅이 부르는 것이 **실제로 도는 파일.** 이름이면 PATH 에서 찾은 것이고, 경로면
/// 실행할 수 있을 때만 그 자리다.
///
/// 실행할 수 있는가는 [`super::runnable`] 하나가 답한다 — `is_file` 만 보던 판은 실행 권한이
/// 빠진 파일을 "있다" 고 했다. 이름으로 적힌 훅은
/// 찾은 자리를 **함께 보인다** — PATH 의 `moai` 가 남의 moai 여도 훅은 돈다.
///
/// **훅 줄의 앞문과 같은 셈은 아니다**(리뷰 moai-j4ie). 그쪽은 `command -v` 로 묻는데 그 답이
/// 껍데기마다 다르다 — dash 는 있는지만, bash 는 `access(X_OK)` 까지 본다. 여기는 늘 뒤엣것으로
/// 재고, 훅 줄은 그 갈림을 `[ -e ]` 로 메워 두 껍데기에서 같은 자리로 온다(`skill::command`).
fn runs(exe: &str, on_path: Option<&Path>) -> Option<PathBuf> {
    if exe.contains('/') {
        let path = Path::new(exe);
        super::runnable(path).then(|| path.to_path_buf())
    } else if exe == "moai" {
        on_path.map(Path::to_path_buf)
    } else {
        which(exe)
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
    known_at(market).filter(|other| !crate::user_config::same_dir(other, dir))
}

/// 설치 id(`moai@<market>`·옛 판이 곁에 깐 것) 하나의 설치 중 **이 저장소에 드는 것**. `status` 와 걷는
/// 둘(`uninstall`·[`retire`])이 한 자로 잰다.
fn installs_here(id: &str, root: &Path) -> Vec<skill::Install> {
    ledger("installed_plugins.json")
        .map(|l| skill::installs_of(&l, id, |p| crate::user_config::same_dir(Path::new(p), root)))
        .unwrap_or_default()
}

/// 이 저장소 말고 **다른 저장소의 moai**(`moai@<다른 이름>`)가 사용자 범위에 깔려 있는가 — 그러면 사용자
/// 범위의 곁의 것은 그 저장소도 쓴다. 장부를 못 읽으면 모른다 — 두고 가는 쪽으로 읽는다.
fn other_user_moai(target: &str) -> bool {
    let Some(ledger) = ledger("installed_plugins.json") else { return true };
    let Some(plugins) = ledger.get("plugins").and_then(|p| p.as_object()) else { return true };
    plugins.iter().any(|(key, rows)| {
        key.starts_with("moai@")
            && key != target
            && rows
                .as_array()
                .is_some_and(|rows| rows.iter().any(|r| r.get("scope").and_then(|s| s.as_str()) == Some("user")))
    })
}

/// 설치본 곁에 남은 옛 판 디렉터리의 수.
fn stale_copies(installs: &[skill::Install]) -> usize {
    let Some(parent) = installs.first().and_then(|i| Path::new(&i.install_path).parent().map(Path::to_path_buf)) else {
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

/// 옛 판이 moai 곁에 함께 깔던 한국어 글쓰기 플러그인 둘의 설치 id(moai-lr1s 가 깔았다).
///
/// **이제는 깔지 않고 걷는다**(사용자 결정 moai-vtfu, 2026-10-03). 글은 기본으로 쓰고, moai 가 깐 것은 moai
/// 가 거둔다 — `install` 과 `uninstall` 이 [`retire`] 로 같은 범위를 걷는다. 마켓플레이스(`@` 뒤)는 두고
/// 간다: `marketplace remove` 는 기계 하나 전체에 걸려, 다른 저장소나 사람이 그것으로 깐 것까지 끊는다.
const RETIRED: [&str; 2] = ["korean-skills@korean-skills", "humanize-korean@im-not-ai"];

/// [`RETIRED`] 를 걷는 걸음 하나 — 무엇을, 어느 범위에서, 그리고 부른 결과.
struct Retire {
    id: &'static str,
    scope: String,
    /// **부르지 않았으면 `None` 이다**(연습, `claude` 가 없거나 moai 를 다 못 걷어 안 부른 `uninstall`).
    /// 결과를 걸음 곁의 목록에 따로 두던 판은 차례로 짝지어, 걸음 하나를 건너뛰는 날이 오면 뒤의 결과가
    /// 모두 남의 id 에 붙을 꼴이었다.
    ok: Option<bool>,
}

impl Retire {
    /// 부를 인자 — 부르는 것과 손으로 칠 줄이 이 하나에서 나온다.
    fn argv(&self) -> Vec<String> {
        argv(&["plugin", "uninstall", self.id, "--scope", &self.scope])
    }

    /// 손으로 칠 한 줄 — 연습과 실패가 같은 글을 낸다.
    fn shown(&self) -> String {
        shown(&self.argv())
    }
}

/// 걷을 것과, 다른 저장소도 쓰는 줄이라 두고 가는 것.
#[derive(Default)]
struct Retired {
    steps: Vec<Retire>,
    /// 사용자 범위라 두고 가는 설치 id — 사람 출력은 손으로 걷는 줄을 함께 댄다.
    kept: Vec<&'static str>,
}

impl Retired {
    /// 걸음마다 `claude` 를 부르고 결과를 그 걸음에 적는다 — **하나가 실패해도 다음 것을 부른다.**
    fn call(&mut self, root: &Path) {
        for step in &mut self.steps {
            step.ok = Some(run(root, &step.argv()));
        }
    }

    /// `--json` 의 `retired` — 걸음마다 명령과 결과. 부르지 않은 걸음은 `ok` 가 `null` 이다.
    fn json(&self) -> Vec<serde_json::Value> {
        self.steps
            .iter()
            .map(|step| serde_json::json!({"id": step.id, "scope": step.scope, "command": step.shown(), "ok": step.ok}))
            .collect()
    }

    /// 두고 가는 것마다 한 줄 — 연습·실행·`uninstall` 이 같은 글을 낸다.
    fn kept_lines(&self, lang: crate::i18n::Lang) -> impl Iterator<Item = String> + '_ {
        self.kept.iter().map(move |id| fill(say(lang, "skill.kept_for_others"), &[("id", id)]))
    }
}

/// 장부에 적힌 범위를 한 번씩 — 적힌 차례대로.
///
/// **범위마다 한 번만 부른다.** 장부에 같은 범위 줄이 겹치면 같은 걷기를 두 번 부르고, 둘째는 이미 걷힌
/// 것이라 실패한다 — `uninstall` 에서는 그 실패가 "실패하면 멈춘다" 에 걸려 마켓플레이스가 안 지워진다.
/// 장부는 `claude` 의 것이라 고치지 않고, 읽은 쪽에서 겹침을 걷는다.
fn scopes_of(installs: &[skill::Install]) -> Vec<&str> {
    let mut scopes: Vec<&str> = Vec::new();
    for i in installs {
        if !scopes.contains(&i.scope.as_str()) {
            scopes.push(&i.scope);
        }
    }
    scopes
}

/// 옛 판이 곁에 깐 것을 걷는 걸음(사용자 결정 moai-vtfu.dvk). `scopes` 는 **이 저장소의 moai(`target`)가
/// 서 있는 범위**고, 부르는 쪽이 저 쓸 값을 그대로 건넨다 — `uninstall` 은 moai 를 걷는 그 범위를, `install`
/// 은 등록하기 전의 범위를.
///
/// 장부는 누가 깔았는지 안 적으니 **그 범위·자리의 설치**를 "moai 가 깐 것" 으로 읽는다 — 옛 `install` 이
/// 바로 그 자리에 깔았다. 대리라서 그 자리에 사람이 손으로 깐 것도 moai 의 것으로 읽힌다.
///
/// **사용자 범위의 설치는 다른 저장소의 moai 가 사용자 범위에 서 있으면 둔다** — 그 줄은 기계에 하나라 그
/// 저장소의 옛 판도 그것을 함께 깔았다. 가리지 않던 판은 한 저장소의 걷기로 다른 저장소의 두 플러그인까지
/// 지웠다(리뷰 moai-5wk4.76z).
fn retire(root: &Path, target: &str, scopes: &[&str]) -> Retired {
    let shared = other_user_moai(target);
    let mut out = Retired::default();
    for id in RETIRED {
        let theirs = installs_here(id, root);
        for scope in scopes {
            if !theirs.iter().any(|i| i.scope == *scope) {
                continue;
            }
            if *scope == "user" && shared {
                out.kept.push(id);
            } else {
                out.steps.push(Retire { id, scope: scope.to_string(), ok: None });
            }
        }
    }
    out
}

/// `claude` 에 등록한다. **사람의 `settings.json` 은 우리가 안 건드린다** —
/// `claude` 가 제 손으로 두 키만 넣는다.
fn register(
    lang: crate::i18n::Lang,
    root: &Path,
    dir: &Path,
    market: &str,
    scope: &str,
    listed: bool,
) -> Vec<(String, bool)> {
    let mut steps = Vec::new();
    let dir = dir.display().to_string();
    if !listed {
        steps.push((
            say(lang, "skill.step_market_added").to_string(),
            run(root, &argv(&["plugin", "marketplace", "add", &dir, "--scope", scope])),
        ));
    } else {
        steps.push((
            say(lang, "skill.step_market_updated").to_string(),
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
    steps.push((say(lang, "skill.step_plugin_registered").to_string(), installed || updated));
    steps
}

/// `claude` 를 **저장소 뿌리에서** 부른다. `local`·`project` 범위는 `claude` 가
/// 제 자리로 프로젝트를 정하므로, 하위 디렉터리에서 부르면 엉뚱한 프로젝트에
/// 적거나 못 찾는다.
fn run(root: &Path, args: &[String]) -> bool {
    Command::new("claude").args(args).current_dir(root).output().map(|o| o.status.success()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// **글마다 제 자리에 선다.** `skill::tree` 는 같은 `&str` 셋을 자리로 받아, 여기서 둘을
    /// 바꿔 적어도 컴파일되고 판도 제 자신과 맞는다 — 커밋된 트리 시험은 `tree_named` 를
    /// 따로 불러 이 길을 안 지난다. 바뀌면 frontmatter 없는 참고 문서가 감독 스킬 자리에
    /// 서서, 모든 저장소에서 그 스킬이 조용히 안 뜬다.
    #[test]
    fn each_text_is_planted_at_its_own_path() {
        let files: BTreeMap<String, String> = plant("t", Path::new("/repo"), "/bin/moai")
            .into_iter()
            .map(|(p, b)| (p.display().to_string(), b))
            .collect();
        for (path, head) in [
            ("skills/moai/SKILL.md", "---\nname: moai\n"),
            ("skills/moai/references/commands.md", "# Every command"),
            ("skills/moai-supervise/SKILL.md", "---\nname: moai-supervise\n"),
        ] {
            assert!(files[path].starts_with(head), "{path} 에 엉뚱한 글이 섰다");
        }
    }
}
