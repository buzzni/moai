---
name: moai-supervise
description: 같은 저장소에서 놀고 있는 Claude 세션들에 쌓인 idea 를 하나씩 나눠 주고 보고를 받을 때 쓴다. "감독해 줘", "idea 나눠 줘", "놀고 있는 세션에 일 시켜" 가 나오면.
---

# moai-supervise — 놀고 있는 세션에 idea 를 나눠 준다

감독은 **고르고, 보내고, 확인한다.** 코드를 고치지 않고, 병합하지 않고, 일꾼
대신 설계를 정하지 않는다. 병합은 일꾼이 하고, 겹치는 병합은 일꾼끼리 먼저
알린다.

## 한 바퀴

**1. 고른다.** 쌓인 idea 에서 지금 벌여 놓은 일과 부딪히지 않는 것만 남긴다.

    moai idea ls                           쌓인 것
    moai show -s in_progress,review        집혀 있는 것
    moai show <id>                         그 idea 가 어디를 건드리는가

`git worktree list` 도 본다. 이미 선 워크트리나 집힌 에픽과 **같은 파일·같은
영역**을 건드리는 idea 는 이번 바퀴에서 뺀다 — 둘이 같은 곳을 고치면 병합에서
한쪽이 다른 쪽을 기다린다.

**2. 일꾼을 찾는다.** `ListAgents` 는 세션의 자리(cwd)를 안 보여 준다.
Claude Code 가 세션마다 적어 두는 `~/.claude/sessions/*.json` 을 읽는다.

    python3 - "$(git worktree list --porcelain | sed -n 's/^worktree //p' | head -1)" <<'PY'
    import glob, json, os, sys
    root = os.path.realpath(sys.argv[1])
    trees = os.path.join(root, ".claude", "worktrees") + os.sep
    for f in glob.glob(os.path.expanduser("~/.claude/sessions/*.json")):
        try:
            s = json.load(open(f))
            os.kill(s["pid"], 0)
            cwd = os.path.realpath(s["cwd"]) if s.get("cwd") else None
        except (OSError, ValueError, KeyError, TypeError):
            continue
        if cwd is None:
            continue
        if cwd == root:
            print("루트    ", s.get("status"), s.get("name"))
        elif cwd.startswith(trees):
            print("워크트리", s.get("status"), s.get("name"), cwd)
    PY

- **맡기는 것은 자리가 루트이고 `idle`·`waiting` 인 세션뿐이다.** `busy` 는
  일하는 중이고, 그 밖의 값(`shell` 따위)은 뜻을 모르니 맡기지 않는다
- 자리가 `<루트>/.claude/worktrees/*` 인 세션은 이 저장소에서 **일하는 중**이다.
  지켜보되 맡기지 않는다
- **다른 디렉터리의 세션은 건드리지 않는다**
- **맡기기를 거절한 세션은 후보에서 빼고 다시 보내지 않는다.** 제 사람이 준 일만
  받는 세션이 있다 — 한 번 거절했으면 그 뒤로는 알림도 걸지 않는다
- 이 파일은 문서에 없는 속 파일이라 판이 바뀌면 필드가 달라질 수 있다. 못 읽으면
  `ListAgents` 로 이름을 보고, 그 세션에 `pwd` 와 지금 하는 일을 물어 가린다

**3. 보낸다.** 놀고 있는 세션 하나에 idea **하나**를 `SendMessage` 로 보낸다.
일꾼은 이 대화를 모르니 절차를 **통째로** 싣는다.

    감독 세션(<내 이름>)이 idea <id> 를 맡긴다 — <제목>.
    먼저 읽을 것: moai show <id>
    1. main 에서 펼친다 — idea 를 일감으로 바꾸는 길은 `moai idea promote <id> --from -`
       하나다. 이슈 하나짜리여도 에픽 + 이슈로 펼친다. `--dry-run` 을 먼저 본다
    2. 멤버를 `moai mv <멤버> in_progress` 로 집고 main 에
       "chore(tracker): <에픽> 를 워크트리에서 집는다" 로 커밋한다.
       main 에서 커밋하기 전에는 언제나 `git status` 를 본다 — 아래 "공유 main" 을 따른다
    3. `git worktree add -b worktree-<에픽> .claude/worktrees/<에픽> main` 으로
       로컬 main 에서 뜨고 EnterWorktree(path) 로 들어간다. 이름은 idea id 가
       아니라 펼친 에픽 id 다
    4. 노트에 없는 설계 결정은 추측하지 말고 AskUserQuestion 으로 묻는다 —
       사람이 일꾼 창을 보고 있다
    5. 리뷰 이슈를 세워(규칙 3) `/code-review high --fix`. 반영은 별도 fix: 커밋,
       넘긴 것은 이슈 번호와 함께 노트.
       워크트리에서 `/code-review` 가 규칙 3 에 막히면 — 지금 훅은 그 워크트리의
       스냅샷만 읽어 main 에서 집은 리뷰 이슈를 못 본다(moai-iz38, 에픽 moai-wofj
       에서 고치는 중) — 같은 관점·단계·`--fix` 범위로 리뷰 서브에이전트를 돌린다.
       리뷰 이슈·원문 노트·닫는 `-m` 은 그대로 남긴다
    6. 멤버의 일이 다 끝나면 병합 전에 에픽 전체를 `/code-review max --fix` 로 본다 —
       가지가 main 을 떠난 자리(`git merge-base main HEAD`)부터의 diff 다. 리뷰
       이슈를 따로 세운다(`-t review --parent <에픽>`). 막히면 5 의 길로 간다
    7. 워크트리에서 main 을 받아 충돌을 푼다
    8. ExitWorktree(keep) 로 루트로 돌아온다 — 워크트리 안에서 그것을 지우면 세션의
       자리가 사라진 디렉터리에 남아 감독이 다시는 이 세션을 루트로 못 본다.
       옆 세션과 병합이 겹치면 먼저 알린 뒤 루트에서 `git merge worktree-<에픽> -m "merge: …"`
       로 **한 번에** 병합한다 — `--no-commit` 으로 열어 두지 않는다.
       **`moai mv <멤버> done` 은 그 병합이 실제로 끝난 뒤에만 친다** — 병합 전에
       옮겼다가 되돌린 일꾼이 있었다. 시험 통과를 보고 멤버·리뷰 이슈를 done 으로 커밋
    9. 루트에서 `git worktree remove .claude/worktrees/<에픽>` 과
       `git branch -d worktree-<에픽>` 으로 워크트리와 가지를 지운다
    10. SendMessage to "<내 이름>" 로 보고 — 머지 해시, 한두 줄 요약, 넘긴 것·새 idea

### 공유 main

루트 체크아웃은 **모든 세션이 같이 쓴다.** 한 세션이 `git merge --no-commit` 으로
병합을 열어 둔 사이에 다른 세션이 트래커 노트를 커밋하면, 그 커밋이 남의 병합을
제 제목으로 봉인한다 — 실제로 그렇게 됐다. 그래서 감독이든 일꾼이든, 트래커 노트
하나라도 main 에서 `git commit` 하기 전에:

- `git status` 를 본다. `.git/MERGE_HEAD` 가 있거나 "still merging" 이면 **커밋하지
  않는다.** 그 병합을 연 세션이 끝낼 때까지 기다린다
- 제 병합은 `git merge <가지> -m "…"` 한 번으로 끝낸다. `--no-commit` 을 쓰지 않는다

**4. 기다린다.** 일하는 세션에는 메시지 없이 `notify_when_idle: true` 로
걸어 둔다. **`ListAgents` 를 되풀이해 훑지 않는다** — 알림이 온다.

**5. 보고를 확인하고 다음을 보낸다.** 보고를 믿기 전에 셋을 본다.

    git log --oneline main                 머지 해시가 main 에 있는가
    moai show <id>                         펼친 에픽과 멤버가 done 인가
    git worktree list                      그 워크트리가 사라졌는가

셋이 맞으면 그 세션에 다음 idea 를 보낸다. 어긋나면 그 세션에 무엇이 남았는지
묻고, 대신 끝내지 않는다.

## 멈출 때

- 부딪히지 않는 idea 가 없거나 놀고 있는 세션이 없으면 사람에게 그렇게 말하고
  멈춘다 — 부딪히는 idea 를 억지로 보내지 않는다
- 일꾼이 사람의 결정을 기다리면 감독이 대신 답하지 않는다. 결정은 사람의 것이다
