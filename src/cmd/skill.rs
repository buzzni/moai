//! `.claude/` 에 플러그인을 심고 `claude` 에 등록한다. 심은 것을 보이고 걷어낸다.
//!
//! **지우지 않는다.** 심은 것을 고칠 때도 덮어쓰기만 한다 — 돌고 있는 세션이
//! 물고 있는 훅 파일을 지우면 그 세션의 도구 호출이 전부 막힌다. 실제로 한 번
//! 그렇게 잠겼고, 껍데기를 만들 도구조차 그 훅에 막혀 세션을 다시 여는 것
//! 말고는 길이 없었다. **밖에서 심은 것을 안에서 걷어내지 않는다** — 걷어낼
//! 때도 `claude` 의 등록만 걷고 파일은 남긴다. moai 가 제 손으로 고치는 사람의
//! 설정은 옛 판이 커밋된 설정에 적은 선언을 걷는 [`Undeclare`] 하나다.
//!
//! `claude` 를 못 찾아도 파일은 심는다. 등록만 사람이 한 줄 치면 된다 —
//! 절반을 해 놓고 아무 말 없이 실패하는 것이 제일 나쁘다.

use super::{Ctx, Fail, R};
use crate::cli::{Agent, Scope};
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
    let current = std::env::current_exe().map_err(|e| Fail::new(e.to_string()))?;
    // **푼 자리는 여기서 한 번 짓는다**(리뷰 moai-gu5m.ke0) — 부른 철자를 믿을지([`invoked`])와 PATH 의
    // `moai` 가 이 파일인지([`skill::exe_name`])를 같은 값으로 가른다. `which` 도 푼 자리를 내니, 둘을
    // 글자로 견주면 자리로 견준 것이다. `skill` 은 글만 짓는 모듈이라 파일 시스템을 거기서 안 본다.
    let resolved = crate::path::real(&current);
    let on_path = which("moai");
    let planted = std::fs::read_to_string(root.join(skill::DIR).join(".claude-plugin/plugin.json")).ok();
    let spelling = kept(planted.as_deref(), &resolved).unwrap_or_else(|| invoked(current, &resolved));
    let exe = skill::exe_name(&spelling, &resolved, on_path.as_deref());
    let prefix = repo.config.prefix.clone();
    // **누구인지 묻지 않는다.** 심는 것은 이력이 남는 일이 아니라 설정이다.
    let files = plant(&prefix, &root, &exe);
    Ok(Place { dir: root.join(skill::DIR), market: skill::market(&prefix, &root), root, prefix, exe, on_path, files })
}

/// 이 체크아웃에 이미 심긴 훅의 철자 — **같은 파일이면 그것을 잇는다**(사용자 결정 2026-10-03, 리뷰
/// moai-gu5m.ke0 3번).
///
/// 부른 철자만 따르면 같은 파일을 다른 철자로 불러 심을 때마다(`/tmp/cargo-target/…` 를 바로, 루트에
/// 건 `./moai` 링크로) 커밋된 `plugin.json` 의 철자가 바뀌어 diff 가 다시 났다. 판도 철자를 따라 바뀌는데
/// `status` 는 같은 파일이라 아무 말을 안 하니, 그 diff 는 예고 없이 섰다. 다른 파일이거나 처음 심을 때만
/// [`invoked`] 의 부른 철자다.
///
/// 심긴 철자도 [`skill::spelled`] 를 지난다 — 훅 한 줄에 못 적는 철자, 프로세스마다 다른 자리, 상대
/// 철자는 잇지 않는다.
fn kept(planted: Option<&str>, resolved: &Path) -> Option<PathBuf> {
    let hooked = skill::hook_exe(planted?)?;
    skill::spelled(Some(Path::new(&hooked)), None).filter(|p| crate::path::real(p) == resolved)
}

/// 훅에 적을 이 실행 파일의 철자 — **부른 철자다**(moai-gu5m, 사용자 결정 2026-10-03).
///
/// `current_exe` 는 리눅스에서 `/proc/self/exe` 라 링크가 다 풀린 값이다. `target/` 이
/// `/tmp/cargo-target/<이름>` 으로 가는 링크인 체크아웃에서(moai-c5xo) 그 값을 적으면, 루트에서 친
/// `skill install` 이 커밋된 `plugin.json` 을 `/tmp/…` 로 바꿔 작업 트리에 diff 가 남고, 훅 철자를
/// 글자로 견주는 `skill status` 는 같은 바이너리를 "훅이 부르는 것과 다르다" 고 했다.
///
/// **`argv[0]` 은 같은 파일일 때만 믿는다** — 그 값은 부른 쪽 마음대로라 제 자리를 대지 못한다.
/// [`skill::spelled`] 가 낸 철자가 푼 자리에서 `current_exe` 의 자리(`resolved`)와 갈리면 `current_exe` 를
/// 그대로 쓴다.
///
/// **상대 철자는 명령을 친 자리에 붙인다**([`crate::store::invoked_dir`], 리뷰 moai-gu5m.ke0) — 커널은 그
/// 철자를 `-C` 가 옮기기 전의 자리에서 찾았다. `main` 이 `-C` 를 따른 뒤의 자리에 붙이던 판은 없는 자리를
/// 짚어 늘 푼 철자로 떨어졌다 — 규약이 권하는 `moai -C <dir>` 꼴에서 `/tmp/…` 가 그대로 돌아왔다. 그 자리는
/// getcwd 라 링크가 풀려 있다 — 안 푸는 것은 `argv[0]` 이 적은 조각이다.
fn invoked(current: PathBuf, resolved: &Path) -> PathBuf {
    let argv0 = std::env::args_os().next().map(PathBuf::from);
    let cwd = crate::store::invoked_dir();
    match skill::spelled(argv0.as_deref(), cwd.as_deref()) {
        Some(spelled) if crate::path::real(&spelled) == resolved => spelled,
        _ => current,
    }
}

/// 훅에 이 실행 파일을 적었을 때 심을 트리.
fn plant(prefix: &str, root: &Path, exe: &str) -> Vec<(PathBuf, String)> {
    skill::tree(prefix, root, exe, &skill::skills())
}

/// 고른 에이전트를 심을 자리로 푼 것(moai-xs2h.ylx).
struct Chosen {
    /// Claude 의 플러그인을 심고 `claude` 에 등록하는가.
    claude: bool,
    /// [`skill::AGENTS_DIR`] 를 읽으려고 고른 에이전트 — 비면 그 자리를 안 심는다.
    shared: Vec<&'static str>,
    /// `auto` 가 PATH 에서 찾은 에이전트. `auto` 를 안 줬으면 `None` 이다.
    found: Option<Vec<&'static str>>,
}

/// `auto` 가 PATH 에서 찾는 것 — 에이전트와 그 실행 파일(사용자 결정 2026-10-04). Antigravity 의 CLI 는 `agy` 다.
/// 훅 한 줄과 `status` 가 이미 쓰는 [`which`] 로 묻는다 — 저장소의 표식(`.codex/` 등)은 새 저장소에 없어, 그것으로
/// 고르면 Codex 를 쓰는 사람이 처음 부를 때 아무것도 못 받는다.
const ON_PATH: [(Agent, &str); 3] = [(Agent::Claude, "claude"), (Agent::Codex, "codex"), (Agent::Antigravity, "agy")];

impl Chosen {
    /// **안 주면 `claude` 하나다**(사용자 결정 2026-10-04) — 지금 부르는 사람에게는 아무것도 안 바뀐다. `auto` 가
    /// 아무것도 못 찾아도 `claude` 다. 되풀이한 값은 하나로 접힌다.
    fn of(agents: &[Agent]) -> Chosen {
        let found: Option<Vec<Agent>> = agents
            .contains(&Agent::Auto)
            .then(|| ON_PATH.iter().filter(|(_, bin)| which(bin).is_some()).map(|(a, _)| *a).collect());
        let mut picked: Vec<Agent> = agents.iter().copied().filter(|a| *a != Agent::Auto).collect();
        picked.extend(found.iter().flatten());
        if picked.is_empty() {
            picked.push(Agent::Claude);
        }
        let named = |all: &[Agent]| -> Vec<&'static str> {
            [Agent::Claude, Agent::Codex, Agent::Antigravity]
                .into_iter()
                .filter(|a| all.contains(a))
                .map(Agent::as_str)
                .collect()
        };
        Chosen {
            claude: picked.contains(&Agent::Claude),
            shared: named(&picked).into_iter().filter(|a| *a != Agent::Claude.as_str()).collect(),
            found: found.as_deref().map(named),
        }
    }

    /// `--json` 의 `agents` — 고른 것 전부, 늘 같은 차례로.
    fn names(&self) -> Vec<&'static str> {
        self.claude.then_some(Agent::Claude.as_str()).into_iter().chain(self.shared.iter().copied()).collect()
    }

    /// `auto` 가 무엇을 찾았는지 한 줄 — `auto` 를 안 줬으면 없다. 고른 까닭이 화면에 없으면, PATH 가 다른 기계에서
    /// 같은 명령이 다른 것을 심는 것이 설명되지 않는다.
    fn found_line(&self, lang: crate::i18n::Lang) -> Option<String> {
        self.found.as_ref().map(|found| match found.is_empty() {
            true => say(lang, "skill.auto_none").to_string(),
            false => fill(say(lang, "skill.auto_found"), &[("agents", &found.join(", "))]),
        })
    }
}

/// 트리를 그 자리에 쓴다. **덮어쓰기만 한다**(이 모듈 머리). [`skill::AGENTS_DIR`] 는 `write_atomic_inside` 로 쓴다 —
/// 임시 파일을 갈아끼우니 쓰려고 열지 않아 그 자리에 선 FIFO 앞에서 멈추지 않고(보통 파일이 아닌 자리는 그 자리를
/// 대며 거절한다), 링크는 체크아웃 안을 가리킬 때만 따라간다. 그 자리는 이 판 전에 moai 가 한 번도 안 쓰던 곳이다.
fn write_shared(dir: &Path, files: &[(PathBuf, String)], root: &Path) -> R<()> {
    for (path, body) in files {
        let at = dir.join(path);
        if let Some(parent) = at.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Fail::new(format!("{}: {e}", parent.display())))?;
        }
        crate::store::write_atomic_inside(&at, body.as_bytes(), root)?;
    }
    Ok(())
}

/// 스킬을 고른 에이전트들에 심는다(moai-xs2h.ylx). Codex·Antigravity 는 [`skill::AGENTS_DIR`] 에 파일을 쓰는 것으로
/// 끝난다 — 등록이 없다. Claude 는 지금까지와 같다([`claude_install`]).
///
/// **`--scope` 는 Claude 의 등록 범위다.** Claude 를 안 고르고 준 범위는 아무것도 안 바꾸니 한 줄로 그렇게 말한다 —
/// 안 말하면 `--scope user` 로 모든 저장소에 심었다고 믿는다. 막지는 않는다.
pub fn install(ctx: &Ctx, scope: Option<Scope>, agents: &[Agent], dry_run: bool) -> R<Vec<String>> {
    let chosen = Chosen::of(agents);
    let place = place(ctx)?;
    let shared =
        (!chosen.shared.is_empty()).then(|| (place.root.join(skill::AGENTS_DIR), skill::agents_tree(&skill::skills())));
    // **Claude 보다 먼저 쓴다** — Claude 의 걸음은 `claude` 를 불러 반쪽으로 끝날 수 있고(비영), 파일 쓰기는 거기에
    // 안 기댄다. 못 쓰면 `claude` 를 부르기 전에 멈춘다.
    if let (Some((dir, files)), false) = (&shared, dry_run) {
        write_shared(dir, files, &place.root)?;
    }
    let (mut json, claude) = match chosen.claude {
        true => claude_install(ctx, place, scope.unwrap_or(Scope::Local).as_str(), dry_run)?,
        false => (serde_json::json!({ "dry_run": dry_run }), Vec::new()),
    };

    if ctx.json {
        if let Some(o) = json.as_object_mut() {
            o.insert("agents".into(), serde_json::json!(chosen.names()));
            o.insert("found".into(), serde_json::json!(chosen.found));
            o.insert("agents_dir".into(), serde_json::json!(shared.as_ref().map(|(d, _)| d.display().to_string())));
            let listed: Vec<String> =
                shared.iter().flat_map(|(_, f)| f.iter().map(|(p, _)| p.display().to_string())).collect();
            o.insert("agents_files".into(), serde_json::json!(listed));
        }
        return super::json_line(&json);
    }

    let lang = ctx.lang();
    let mut out: Vec<String> = chosen.found_line(lang).into_iter().collect();
    if let Some((dir, files)) = &shared {
        let at = dir.display().to_string();
        if dry_run {
            out.push(fill(say(lang, "skill.agents_plan"), &[("dir", &at)]));
            out.extend(plan_lines(lang, files));
        } else {
            out.push(fill(say(lang, "skill.agents_planted"), &[("dir", &at)]));
        }
        if scope.is_some() && !chosen.claude {
            out.push(fill(say(lang, "skill.scope_is_claudes"), &[("dir", &at)]));
        }
        if chosen.claude {
            out.push(String::new());
        }
    }
    out.extend(claude);
    Ok(out)
}

/// 연습이 심을 파일마다 내는 줄 — 두 트리가 같은 꼴로 선다.
fn plan_lines<'a>(lang: crate::i18n::Lang, files: &'a [(PathBuf, String)]) -> impl Iterator<Item = String> + 'a {
    files.iter().map(move |(path, body)| {
        let n = body.lines().count().to_string();
        format!("  {:<44} {}", path.display(), fill(say(lang, "skill.plan_lines"), &[("n", &n)]))
    })
}

/// Claude 의 플러그인을 심고 등록한다. `--json` 이면 값만, 아니면 사람의 줄만 낸다 — 고른 에이전트를 함께 싣는
/// 것은 [`install`] 이다.
fn claude_install(ctx: &Ctx, place: Place, scope: &str, dry_run: bool) -> R<(serde_json::Value, Vec<String>)> {
    let Place { root, dir, market, exe, files, .. } = place;
    // **같은 이름이 남의 저장소를 가리키면 등록하지 않는다.** 덮어쓰면 그
    // 저장소의 규칙이 이쪽에 걸린다 — 조용히 엉뚱해지는 쪽이라 더 나쁘다.
    // 연습도 같은 답을 낸다. 진짜 실행이 건너뛸 등록을 연습이 약속하면 안 된다.
    let clash = clash_of(&market, &dir);
    // **옛 판이 곁에 깐 것을 걷는다**(moai-vtfu). 설치는 등록하기 **전의** 장부로 잰다 — 등록이 설치본을 새
    // 판으로 바꾸면 그 범위의 moai 가 옛 판이라는 표식([`teaches_retired`])이 사라지고, 이번 `--scope` 로 처음
    // 서는 범위가 섞인다. 이름이 남의 저장소를 가리키면 `uninstall` 처럼 아무것도 안 부른다. 이번 `--scope` 밖에서
    // 옛 판으로 선 moai 는 걷기 전에 새 판으로 올린다([`Lift`]).
    let target = format!("moai@{market}");
    let mut retiring = if clash.is_none() {
        retire(&root, &target, &installs_here(&target, &root), Some(scope))
    } else {
        Retired::default()
    };

    if dry_run {
        if ctx.json {
            return Ok((
                serde_json::json!({
                    "dir": dir.display().to_string(),
                    "market": market,
                    "scope": scope,
                    "exe": exe,
                    "dry_run": true,
                    "files": files.iter().map(|(p, _)| p.display().to_string()).collect::<Vec<_>>(),
                    "blocked_by": clash.as_ref().map(|p| p.display().to_string()),
                    "lifted": retiring.lifted_json(),
                    "retired": retiring.json(),
                    "undeclared": retiring.undeclared_json(),
                    "kept": retiring.kept,
                }),
                Vec::new(),
            ));
        }
        let lang = ctx.lang();
        let mut out = vec![fill(say(lang, "skill.plan_head"), &[("dir", &dir.display().to_string())])];
        out.extend(plan_lines(lang, &files));
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
        for lift in &retiring.lifts {
            out.push(fill(say(lang, "skill.plan_lift"), &[("cmd", &lift.shown())]));
        }
        for step in &retiring.steps {
            out.push(fill(say(lang, "skill.plan_retire"), &[("cmd", &step.shown())]));
        }
        out.extend(retiring.undeclare.iter().map(|u| fill(say(lang, "skill.plan_retire"), &[("cmd", &u.what(lang))])));
        out.extend(retiring.kept_lines(lang));
        return Ok((serde_json::Value::Null, out));
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
    // 줄로 낸다. **등록이 안 됐으면 부르지 않는다**(사용자 결정 둘째 판) — 그때는 플러그인을 부르라는 옛 moai
    // 사본이 그대로 실려 있다. `uninstall` 이 moai 를 다 못 걷으면 안 부르는 것과 같은 셈이고, 설치본이 옛
    // 판으로 남으니 다시 부르면 또 걷는다. 이번 `--scope` 밖의 옛 판 범위는 걷기 전에 올리고, 못 올린 범위는
    // 안 걷는다([`Retired::call`]). 올리기도 걷기처럼 종료 코드를 안 바꾼다.
    if registered {
        retiring.call(&root);
    }

    if ctx.json {
        return Ok((
            serde_json::json!({
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
                "lifted": retiring.lifted_json(),
                "retired": retiring.json(),
                "undeclared": retiring.undeclared_json(),
                "kept": retiring.kept,
            }),
            Vec::new(),
        ));
    }

    let mut out = vec![fill(say(lang, "skill.planted"), &[("dir", &dir.display().to_string())])];
    out.push(fill(say(lang, "skill.hook_call"), &[("cmd", &format!("{exe} hook <event>"))]));
    for (what, ok) in &steps {
        out.push(format!("  {} {what}", if *ok { "·" } else { "!" }));
    }
    for lift in &retiring.lifts {
        out.push(match lift.ok {
            Some(true) => fill(say(lang, "skill.lifted"), &[("scope", &lift.scope)]),
            Some(false) => fill(say(lang, "skill.lift_failed"), &[("scope", &lift.scope), ("cmd", &lift.shown())]),
            None => format!("  - {}{}", lift.shown(), say(lang, "skill.step_not_called")),
        });
    }
    for step in &retiring.steps {
        out.push(match step.ok {
            Some(true) => fill(say(lang, "skill.retired"), &[("id", step.id), ("scope", &step.scope)]),
            Some(false) => fill(say(lang, "skill.retire_failed"), &[("id", step.id), ("cmd", &step.shown())]),
            // 등록이 안 됐거나 그 범위를 못 올려 안 불렀다 — `uninstall` 이 같은 판에 내는 줄과 같다.
            None => format!("  - {}{}", step.shown(), say(lang, "skill.step_not_called")),
        });
    }
    out.extend(retiring.undeclare.iter().map(|u| u.line(lang)));
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
    Ok((serde_json::Value::Null, out))
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
    let shared = Shared::read(&root);
    // Claude 는 위의 `claude` 줄이 이미 댄다 — 여기는 `.agents/skills` 를 읽는 둘이다.
    let others: Vec<(&str, bool)> =
        ON_PATH.iter().filter(|(a, _)| *a != Agent::Claude).map(|(_, bin)| (*bin, which(bin).is_some())).collect();

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
            "agents": shared.json(&others),
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
        // **자리로 견준다**(moai-gu5m). `exe` 는 [`invoked`] 가 고른 부른 철자라, 같은 파일을 링크 너머의
        // 다른 철자로 부르면 글자만 갈린다 — 그때 "다른 moai" 라고 하면 거짓이다. 글자가 같으면 자리도
        // 같고, 어느 한쪽이 안 도는 자리면 글자로만 가른다.
        let same = *hook == exe
            || matches!((&hook_path, runs(&exe, on_path.as_deref())),
                (Some(there), Some(here)) if crate::path::real(there) == crate::path::real(&here));
        if !same {
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
    // **Claude 의 줄 밑에 `.agents` 한 줄과 그것을 읽는 둘의 PATH 줄**(사용자 결정 2026-10-04).
    let shared_row = say(lang, "skill.row_agents");
    out.push(match (shared.planted, shared.stale.is_empty()) {
        (false, _) => row(false, shared_row, say(lang, "skill.agents_missing")),
        (true, false) => {
            row(false, shared_row, &fill(say(lang, "skill.agents_stale"), &[("n", &shared.stale.len().to_string())]))
        }
        (true, true) => row(true, shared_row, say(lang, "skill.agents_current")),
    });
    for (bin, on) in &others {
        let said = match on {
            true => say(lang, "skill.on_path"),
            false => say(lang, "skill.not_on_path"),
        };
        out.push(row(*on, bin, said));
    }
    Ok(out)
}

/// [`skill::AGENTS_DIR`] 가 지금 판의 글과 같은가(moai-xs2h.v2n) — `status` 가 Codex·Antigravity 의 자리를 대는 값.
struct Shared {
    dir: PathBuf,
    /// 지금 판과 다르거나 없는 파일 — 그 자리부터의 상대 경로.
    stale: Vec<String>,
    /// 심을 파일 가운데 하나라도 서 있는가 — 없으면 "안 심겼다" 고, 있는데 다르면 "낡았다" 다.
    planted: bool,
}

impl Shared {
    /// **보통 파일만, 체크아웃 안에서만 읽는다**(`held::read_inside`). 그 자리는 이 판 전의 `status` 가 한 번도 안 열던
    /// 곳이라, 거기 선 FIFO 하나가 읽기를 멈춰 세우면 아무것도 안 막는다던 명령이 멈춘다. 못 읽은 파일은 낡은 것으로
    /// 센다 — 보통 파일이면 다시 심어 갈아끼우고, 보통 파일이 아닌 자리는 다시 심는 길이 그 자리를 대며 멈춘다
    /// ([`write_shared`]).
    fn read(root: &Path) -> Shared {
        let dir = root.join(skill::AGENTS_DIR);
        let home = crate::held::Home::of(root);
        let mut stale = Vec::new();
        let mut planted = false;
        for (path, body) in skill::agents_tree(&skill::skills()) {
            let at = dir.join(&path);
            planted |= std::fs::symlink_metadata(&at).is_ok();
            if crate::held::read_inside(&at, &home).ok().as_deref() != Some(body.as_str()) {
                stale.push(path.display().to_string());
            }
        }
        Shared { dir, stale, planted }
    }

    /// `--json` 의 `agents` — 자리·상태(`current`·`stale`·`missing`)·낡은 파일·PATH 에 선 둘.
    fn json(&self, others: &[(&str, bool)]) -> serde_json::Value {
        let state = match (self.planted, self.stale.is_empty()) {
            (false, _) => "missing",
            (true, false) => "stale",
            (true, true) => "current",
        };
        let mut v = serde_json::json!({
            "dir": self.dir.display().to_string(),
            "state": state,
            "stale": self.stale,
        });
        for (bin, on) in others {
            v[*bin] = serde_json::json!(on);
        }
        v
    }
}

/// 이름 칸을 **표시 폭**으로 맞춘다(moai-uzgp). 한글은 한 글자가 두 칸이라 `{:<14}` 로 맞추면
/// 말마다 이 표가 어긋난다 — `text::width` 가 CLI 표와 탐색기가 함께 쓰는 자다.
fn label(what: &str) -> String {
    const WIDE: usize = 14;
    format!("{what}{}", " ".repeat(WIDE.saturating_sub(crate::text::width(what))))
}

/// 고른 에이전트에서 심은 것을 걷는다. **파일은 남긴다** — Claude 는 `claude` 의 등록만 걷고([`claude_uninstall`]),
/// Codex·Antigravity 는 걷을 등록이 없어 손으로 지울 자리를 찍기만 한다(moai-xs2h.v2n, 이 모듈 머리의 "지우지 않는다").
///
/// **고르지 않은 자리에 moai 의 스킬이 남았으면 한 줄로 댄다.** `--agent codex` 로 심은 사람이 맨 `uninstall` 을 부르면
/// Claude 쪽은 "걷을 것이 없다" 고 끝나는데, 그 줄만으로는 `.agents/skills` 가 그대로라는 것이 안 보인다.
pub fn uninstall(ctx: &Ctx, agents: &[Agent], dry_run: bool) -> R<Vec<String>> {
    let chosen = Chosen::of(agents);
    let place = place(ctx)?;
    let shared = place.root.join(skill::AGENTS_DIR);
    // moai 가 심는 이름만 댄다 — 그 자리의 다른 스킬은 남의 것이다.
    let left: Vec<PathBuf> =
        skill::NAMES.iter().map(|n| shared.join(n)).filter(|p| std::fs::symlink_metadata(p).is_ok()).collect();
    let (mut json, claude) = match chosen.claude {
        true => claude_uninstall(ctx, place, dry_run)?,
        false => (serde_json::json!({ "dry_run": dry_run }), Vec::new()),
    };

    if ctx.json {
        if let Some(o) = json.as_object_mut() {
            o.insert("agents".into(), serde_json::json!(chosen.names()));
            o.insert("found".into(), serde_json::json!(chosen.found));
            let left: Vec<String> = left.iter().map(|p| p.display().to_string()).collect();
            o.insert("agents_left".into(), serde_json::json!(left));
        }
        return super::json_line(&json);
    }

    let lang = ctx.lang();
    let at = shared.display().to_string();
    let mut out: Vec<String> = chosen.found_line(lang).into_iter().collect();
    if !chosen.shared.is_empty() {
        if left.is_empty() {
            out.push(fill(say(lang, "skill.agents_nothing"), &[("dir", &at)]));
        } else {
            out.push(say(lang, "skill.agents_by_hand").to_string());
            out.extend(left.iter().map(|p| format!("  rm -r {}", crate::text::shell_word(&p.display().to_string()))));
        }
        if chosen.claude {
            out.push(String::new());
        }
    }
    out.extend(claude);
    if chosen.shared.is_empty() && !left.is_empty() {
        out.push(fill(say(lang, "skill.agents_left_hint"), &[("dir", &at)]));
    }
    Ok(out)
}

/// `claude` 에서 이 저장소의 등록을 걷어낸다. **파일은 남긴다.** `--json` 이면 값만, 아니면 사람의 줄만 낸다.
fn claude_uninstall(ctx: &Ctx, place: Place, dry_run: bool) -> R<(serde_json::Value, Vec<String>)> {
    let Place { root, dir, market, .. } = place;
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
        // **moai 를 걷는 그 설치로 잰다** — 장부를 다시 읽어 재던 판은 두 읽기 사이에 `claude` 가 장부를
        // 고쳐 쓰면(옆 세션의 `skill install --scope user`) moai 를 안 걷는 범위의 것까지 걷을 수 있었다.
        retiring = retire(&root, &target, &installs, None);
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
        return Ok((
            serde_json::json!({
                "dry_run": dry_run,
                "dir": dir.display().to_string(),
                "market": market,
                "planned": plan.iter().map(|a| shown(a)).collect::<Vec<_>>(),
                "steps": steps.iter().map(|(c, ok)| serde_json::json!({"command": c, "ok": ok})).collect::<Vec<_>>(),
                "removed": !dry_run && !failed && !plan.is_empty(),
                "blocked_by": clash.as_ref().map(|p| p.display().to_string()),
                "claude": claude,
                "retired": retiring.json(),
                "undeclared": retiring.undeclared_json(),
                "kept": retiring.kept,
            }),
            Vec::new(),
        ));
    }

    let lang = ctx.lang();
    if let Some(other) = &clash {
        let out = vec![
            fill(say(lang, "skill.remove_elsewhere"), &[("market", &market), ("at", &other.display().to_string())]),
            say(lang, "skill.remove_elsewhere_there").to_string(),
        ];
        return Ok((serde_json::Value::Null, out));
    }
    if plan.is_empty() {
        return Ok((serde_json::Value::Null, vec![fill(say(lang, "skill.nothing_to_remove"), &[("market", &market)])]));
    }
    if dry_run || !claude {
        let mut out = vec![match dry_run {
            true => say(lang, "skill.plan_calls").to_string(),
            false => say(lang, "skill.no_claude_by_hand").to_string(),
        }];
        out.extend(plan.iter().map(|a| format!("  {}", shown(a))));
        out.extend(retiring.steps.iter().map(|step| format!("  {}", step.shown())));
        out.extend(retiring.undeclare.iter().map(|u| format!("  {}", u.what(lang))));
        out.extend(retiring.kept_lines(lang));
        return Ok((serde_json::Value::Null, out));
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
    out.extend(retiring.undeclare.iter().map(|u| u.line(lang)));
    out.extend(retiring.kept_lines(lang));
    out.push(String::new());
    out.push(say(lang, "skill.reopen_to_finish").to_string());
    out.push(fill(say(lang, "skill.files_left"), &[("dir", skill::DIR)]));
    Ok((serde_json::Value::Null, out))
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

/// 옛 판이 moai 곁에 함께 깔던 한국어 글쓰기 플러그인 둘 — `(설치 id, 옛 판이 더하던 마켓플레이스 저장소)`
/// (moai-lr1s 가 깔았다).
///
/// **이제는 깔지 않고 걷는다**(사용자 결정 moai-vtfu, 2026-10-03). 글은 기본으로 쓰고, moai 가 깐 것은 moai
/// 가 거둔다 — `install` 과 `uninstall` 이 [`retire`] 로 같은 범위를 걷는다. 마켓플레이스(`@` 뒤)는 기계에
/// 두고 간다: `marketplace remove` 는 기계 하나 전체에 걸려, 다른 저장소나 사람이 그것으로 깐 것까지 끊는다.
/// project 범위의 커밋된 설정에 옛 판이 적은 **선언**만 걷는다([`Undeclare`], moai-6ugu).
const RETIRED: [(&str, &str); 2] =
    [("korean-skills@korean-skills", "DaleSeo/korean-skills"), ("humanize-korean@im-not-ai", "epoko77-ai/im-not-ai")];

/// 옛 판의 moai 스킬이 가르치던 글 — 설치본의 `SKILL.md` 에 이것이 있으면 그 설치는 [`RETIRED`] 를 곁에 깐
/// 판이다. v0.1.0 부터 v0.3.0 까지 모든 판의 SKILL.md 가 `korean-skills:humanizer` 를 부르라고 했고, 이 에픽이
/// 그 절을 걷었다(`guide::tests::no_surface_asks_for_the_korean_writing_plugins` 가 새 판에 이 글이 없음을 잰다).
const RETIRED_MARK: &str = "korean-skills:";

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

/// 이번 `--scope` 밖의 범위에 옛 판으로 선 이 저장소의 moai 를 새 판으로 올리는 걸음(사용자 결정 moai-tl3k.jvz) —
/// `install` 만 세우고, 그 범위를 걷기 **전에** 부른다.
///
/// 걷는 문은 "그 범위의 moai 설치본이 옛 판" 이다([`teaches_retired`]). `claude plugin update --scope <범위>` 는
/// 장부의 그 범위 줄만 새 판으로 올려서(claude 2.1.287 로 쟀다), 올리지 않은 범위는 다음 `install` 에서도 옛 판으로
/// 읽혀 그 사이 사람이 손으로 다시 깐 것을 매번 걷었다(리뷰 moai-6ugu.3kw 9번). 올리면 장부가 곧 "옮긴 범위" 의
/// 표식이라 따로 적는 상태가 없다. 이 걸음은 장부만 고치고 커밋된 설정은 안 건드린다(같은 판으로 쟀다).
struct Lift {
    target: String,
    scope: String,
    /// [`Retire::ok`] 와 같다 — 부르지 않았으면 `None`.
    ok: Option<bool>,
}

impl Lift {
    fn argv(&self) -> Vec<String> {
        update_argv(&self.target, &self.scope)
    }

    fn shown(&self) -> String {
        shown(&self.argv())
    }
}

/// 커밋된 설정에서 [`RETIRED`] 의 마켓플레이스 선언 하나를 지우는 걸음(사용자 결정 moai-6ugu.aae) — `claude` 를
/// 안 부르고 moai 가 그 파일을 고친다([`skill::drop_marketplace`] 가 까닭을 든다).
struct Undeclare {
    market: &'static str,
    file: PathBuf,
    /// [`Retire::ok`] 와 같다 — 부르지 않았으면 `None`.
    ok: Option<bool>,
}

impl Undeclare {
    /// 그 파일에서 선언을 지운다. **이미 없으면 이룬 것이다.** 그 파일이 이 마켓의 플러그인을 아직 켜 두었으면
    /// 안 지운다 — 같은 `call` 에서 앞선 `plugin uninstall` 이 실패한 판이고, 선언을 걷으면 켠 플러그인이 출처를
    /// 잃는다. 그래서 계획한 때가 아니라 **지우기 직전에** 다시 읽는다.
    fn call(&self, root: &Path) -> bool {
        let Ok(text) = std::fs::read_to_string(&self.file) else { return false };
        let Ok(settings) = serde_json::from_str::<serde_json::Value>(&text) else { return false };
        if settings.get("extraKnownMarketplaces").and_then(|m| m.get(self.market)).is_none() {
            return true;
        }
        if !skill::plugins_from(&settings, self.market).is_empty() {
            return false;
        }
        skill::drop_marketplace(&text, self.market)
            .is_some_and(|out| crate::store::write_atomic_inside(&self.file, out.as_bytes(), root).is_ok())
    }

    /// 할 일 — 연습이 [`Retire::shown`] 자리에 댄다.
    fn what(&self, lang: crate::i18n::Lang) -> String {
        fill(say(lang, "skill.undeclare_what"), &[("market", self.market), ("file", &self.file.display().to_string())])
    }

    /// 부른 뒤의 줄 — 지웠다·못 지웠다(손으로 지울 자리)·안 불렀다.
    fn line(&self, lang: crate::i18n::Lang) -> String {
        let file = self.file.display().to_string();
        let args = [("market", self.market), ("file", file.as_str())];
        match self.ok {
            Some(true) => fill(say(lang, "skill.undeclared"), &args),
            Some(false) => fill(say(lang, "skill.undeclare_failed"), &args),
            None => format!("  - {}{}", self.what(lang), say(lang, "skill.step_not_called")),
        }
    }
}

/// 걷을 것과, 다른 저장소도 쓰는 줄이라 두고 가는 것.
#[derive(Default)]
struct Retired {
    /// 걷기 전에 올릴 범위 — `install` 만 세운다. `uninstall` 은 moai 를 범위째 걷어 올릴 것이 없다.
    lifts: Vec<Lift>,
    steps: Vec<Retire>,
    /// 커밋된 설정의 선언을 지우는 걸음 — 플러그인 걸음 **뒤에** 부른다.
    undeclare: Vec<Undeclare>,
    /// 사용자 범위라 두고 가는 설치 id — 사람 출력은 손으로 걷는 줄을 함께 댄다.
    kept: Vec<&'static str>,
}

impl Retired {
    /// 걸음마다 `claude` 를 부르고 결과를 그 걸음에 적는다 — **하나가 실패해도 다음 것을 부른다.** 올리기가 맨
    /// 앞이고 선언 지우기는 맨 뒤다 — `plugin uninstall --scope project` 가 그 파일의 `enabledPlugins` 줄을 걷어야
    /// 선언이 빈다.
    ///
    /// **못 올린 범위는 안 걷는다**(사용자 결정 moai-tl3k.jvz). 그 범위의 moai 는 옛 판 그대로라 다음 `install` 이 같은
    /// 문으로 다시 재어 올리기부터 다시 한다. 걷고 나면 옛 판이 남아, 올리기가 될 때까지 그 사이 다시 깐 것을 매번 걷는다.
    fn call(&mut self, root: &Path) {
        for lift in &mut self.lifts {
            lift.ok = Some(run(root, &lift.argv()));
        }
        let lifts = &self.lifts;
        let stuck = |scope: &str| lifts.iter().any(|l| l.scope == scope && l.ok == Some(false));
        for step in self.steps.iter_mut().filter(|s| !stuck(s.scope.as_str())) {
            step.ok = Some(run(root, &step.argv()));
        }
        // 선언은 늘 project 범위의 것이다([`retire`]) — project 를 못 올렸으면 그것도 안 지운다.
        for step in self.undeclare.iter_mut().filter(|_| !stuck("project")) {
            step.ok = Some(step.call(root));
        }
    }

    /// `--json` 의 `lifted` — 올린 범위마다 명령과 결과. `ok` 는 `retired` 와 같은 셈이다.
    fn lifted_json(&self) -> Vec<serde_json::Value> {
        self.lifts.iter().map(|l| serde_json::json!({"scope": l.scope, "command": l.shown(), "ok": l.ok})).collect()
    }

    /// `--json` 의 `retired` — 걸음마다 명령과 결과. 부르지 않은 걸음은 `ok` 가 `null` 이다.
    fn json(&self) -> Vec<serde_json::Value> {
        self.steps
            .iter()
            .map(|step| serde_json::json!({"id": step.id, "scope": step.scope, "command": step.shown(), "ok": step.ok}))
            .collect()
    }

    /// `--json` 의 `undeclared` — 선언마다 이름·파일·결과. `ok` 는 `retired` 와 같은 셈이다.
    fn undeclared_json(&self) -> Vec<serde_json::Value> {
        self.undeclare
            .iter()
            .map(|u| serde_json::json!({"marketplace": u.market, "file": u.file.display().to_string(), "ok": u.ok}))
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

/// 이 설치본이 [`RETIRED`] 를 곁에 깐 옛 판인가 — 설치본(`installPath`)의 moai 스킬이 [`RETIRED_MARK`] 를
/// 가르친다. **설치본을 못 읽으면 아니다** — 걷기는 되돌리기 어려워, 모르는 것은 남기는 쪽으로 읽는다.
fn teaches_retired(install: &skill::Install) -> bool {
    std::fs::read_to_string(Path::new(&install.install_path).join("skills/moai/SKILL.md"))
        .is_ok_and(|text| text.contains(RETIRED_MARK))
}

/// 옛 판이 곁에 깐 것을 걷는 걸음(사용자 결정 moai-vtfu.dvk). `installs` 는 **이 저장소의 moai(`target`)의
/// 설치**고, 부르는 쪽이 저 쓸 값을 그대로 건넨다 — `uninstall` 은 moai 를 걷는 그 설치를, `install` 은
/// 등록하기 전의 설치를. `registering` 은 `install` 이 등록하는 범위다 — 그 밖의 옛 판 범위는 걷기 전에 새 판으로
/// 올린다([`Lift`]). 등록하는 범위는 등록이 올린다. `uninstall` 은 `None` 이다.
///
/// 장부는 누가 깔았는지 안 적으니 셋이 겹칠 때만 "moai 가 깐 것" 으로 읽는다(사용자 결정 둘째 판).
/// - **그 범위에 선 moai 설치본이 옛 판이다**([`teaches_retired`]). 옛 `install` 은 moai 와 같은 범위·자리에
///   깔았다. 새 판으로 한 번 옮겨 간 범위는 다시 안 걷는다 — 그 뒤 사람이 손으로 깐 것을 매 `install` 마다
///   걷던 판을 막는다. `install` 이 걷는 범위는 새 판으로 올라가므로(이번 `--scope` 는 등록이, 그 밖은 [`Lift`]
///   가 올린다) 범위당 한 번이 선다. 빈 데가 하나 있다 — 이번 `--scope` 는 등록이 된 것으로 세는데, `plugin
///   install` 은 이미 선 설치에 0 을 내고 판을 안 올린다(claude 2.1.287 로 쟀다). 거기서 `update` 만 실패하면 그
///   범위는 옛 판인 채 걷히고 다음 `install` 이 또 걷는다. 대가로 걷기가 실패한 채 그 범위가 올라가면 다음
///   `install` 은 다시 안 걷는다 — 손으로 칠 줄은 그때 낸다
/// - **그 범위·자리에 그 플러그인이 서 있다**
/// - **마켓플레이스가 옛 판이 더하던 저장소를 가리킨다**([`skill::market_repo`] — 대소문자와 주소 꼴은 가리지
///   않는다). 다른 출처(포크)를 가리키거나 그 이름을 모르면 걷지 않는다 — 옛 `install` 도 그때는 안 깔았다
///
/// **사용자 범위의 설치는 다른 저장소의 moai 가 사용자 범위에 서 있으면 둔다** — 그 줄은 기계에 하나라 그
/// 저장소의 옛 판도 그것을 함께 깔았다. 가리지 않던 판은 한 저장소의 걷기로 다른 저장소의 두 플러그인까지
/// 지웠다(리뷰 moai-5wk4.76z).
///
/// **옛 판이 선 범위가 project 면 커밋된 `.claude/settings.json` 의 마켓플레이스 선언도 걷는다**(사용자 결정
/// moai-6ugu.aae) — 같은 문 뒤에서, 그 파일에 적힌 출처가 옛 판의 것이고 그 파일이 이 마켓의 다른 플러그인을
/// 안 켜 두었을 때만([`Undeclare`]).
fn retire(root: &Path, target: &str, installs: &[skill::Install], registering: Option<&str>) -> Retired {
    let old: Vec<skill::Install> = installs.iter().filter(|i| teaches_retired(i)).cloned().collect();
    let mut out = Retired::default();
    if old.is_empty() {
        return out;
    }
    let scopes = scopes_of(&old);
    let known = ledger("known_marketplaces.json");
    let shared = other_user_moai(target);
    for (id, repo) in RETIRED {
        let market = market_of(id);
        if !known.as_ref().and_then(|k| skill::market_repo(k, market)).is_some_and(|at| at.eq_ignore_ascii_case(repo)) {
            continue;
        }
        let theirs = installs_here(id, root);
        for scope in &scopes {
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
    // **걷을 것이 없는 옛 판 범위도 올린다** — 안 올리면 문이 열린 채라, 사람이 그 범위에 다시 깐 것을 다음
    // `install` 이 걷는다. 출처가 옛 판의 것이 아니어서 아무것도 안 걷는 판도 같은 셈으로 올린다. **두고 간 것이
    // 있는 사용자 범위는 안 올린다**(리뷰 moai-tl3k.wx7 2번) — 올리면 문이 닫혀, 다른 저장소의 moai 가 빠진 뒤에도
    // 이 저장소가 그 범위를 다시는 안 걷는다. 옛 판으로 두면 그때 한 번 걷는다.
    if let Some(here) = registering {
        out.lifts = scopes
            .iter()
            .filter(|s| **s != here && !(**s == "user" && !out.kept.is_empty()))
            .map(|s| Lift { target: target.to_string(), scope: s.to_string(), ok: None })
            .collect();
    }
    // **project 범위면 커밋된 설정의 선언도 걷는다**(사용자 결정 moai-6ugu.aae). 옛 `install --scope project` 는
    // `marketplace add <저장소> --scope project` 로 그 파일에 선언을 적었고, `plugin uninstall` 은 그것을 남긴다.
    // 출처는 그 파일에 적힌 것으로 잰다. 그 파일이 이 마켓의 플러그인을 켜 두었으면 그것이 이번에 project 에서
    // 걷는 바로 그 플러그인일 때만 걷는다 — 다른 것을 켜 두었으면 그 선언은 이제 그것의 것이다. 못 읽는 파일에는
    // 걸음을 안 세운다 — 어느 출처를 선언했는지 모른다.
    if !scopes.contains(&"project") {
        return out;
    }
    let file = root.join(".claude/settings.json");
    let Some(settings) =
        std::fs::read_to_string(&file).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
    else {
        return out;
    };
    for (id, repo) in RETIRED {
        let market = market_of(id);
        if !skill::declared_repo(&settings, market).is_some_and(|at| at.eq_ignore_ascii_case(repo)) {
            continue;
        }
        let going = out.steps.iter().any(|s| s.id == id && s.scope == "project");
        if skill::plugins_from(&settings, market).iter().any(|p| !(going && p == id)) {
            continue;
        }
        out.undeclare.push(Undeclare { market, file: file.clone(), ok: None });
    }
    out
}

/// 설치 id(`<플러그인>@<마켓플레이스>`)의 마켓플레이스 이름 — [`RETIRED`] 의 장부 줄과 설정 선언을 이 이름으로 찾는다.
fn market_of(id: &str) -> &str {
    id.split_once('@').map_or(id, |(_, m)| m)
}

/// `claude` 에 등록한다. **사람의 `settings.json` 은 우리가 안 건드린다** —
/// `claude` 가 제 손으로 두 키만 넣는다. 하나뿐인 예외는 옛 판이 적은 선언을 걷는 [`Undeclare`] 다.
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
    let updated = run(root, &update_argv(&target, scope));
    steps.push((say(lang, "skill.step_plugin_registered").to_string(), installed || updated));
    steps
}

/// 한 범위의 moai 를 새 판으로 올리는 줄 — 등록([`register`])과 [`Lift`] 가 이 하나를 부른다. 둘이 따로 적으면
/// 등록하는 범위와 함께 올리는 범위가 다른 명령으로 오른다. `-y` 의 까닭은 [`register`] 에 있다.
fn update_argv(target: &str, scope: &str) -> Vec<String> {
    argv(&["plugin", "update", target, "--scope", scope, "-y"])
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

    /// **글마다 제 자리에 선다.** `skill::tree` 는 같은 `&str` 넷을 자리로 받아, 여기서 둘을
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
            ("skills/moai-wiki/SKILL.md", "---\nname: moai-wiki\n"),
        ] {
            assert!(files[path].starts_with(head), "{path} 에 엉뚱한 글이 섰다");
        }
    }
}
