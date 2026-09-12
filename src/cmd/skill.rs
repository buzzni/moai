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
    let files = skill::tree(&repo.config.prefix, &root, &exe, body::SKILL, body::REFERENCE);

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

/// 심는 글. **`init` 의 AGENTS 블록과 한 출처여야 한다** — 지금은 두 벌이고,
/// 합치는 것은 `moai-zy40` 이다.
mod body {
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
막히고, 거절문이 고칠 명령을 함께 낸다. 리뷰 원문은 리뷰가 끝났다는 알림에
실려 오는 task-id 로 찾는다.

막히면 거절문이 고칠 명령을 함께 낸다. **사람을 부르지 않는다** — 그 명령을
그대로 부르면 지나간다.

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

## 훅

    moai hook <event>    Claude 의 훅이 부른다. 사람이 손으로 부를 일은 없다

`moai skill install` 이 심은 플러그인이 이것을 부른다. 무엇이 어긋나도 종료
코드는 0 이다 — 훅이 시끄러우면 사람이 훅을 꺼 버리고, 꺼진 규칙은 없는
규칙이다.
"#;
}
