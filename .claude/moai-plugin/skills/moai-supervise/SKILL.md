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
        except (OSError, ValueError, KeyError):
            continue
        cwd = os.path.realpath(s.get("cwd", ""))
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
- 이 파일은 문서에 없는 속 파일이라 판이 바뀌면 필드가 달라질 수 있다. 못 읽으면
  `ListAgents` 로 이름을 보고, 그 세션에 `pwd` 와 지금 하는 일을 물어 가린다

**3. 보낸다.** 놀고 있는 세션 하나에 idea **하나**를 `SendMessage` 로 보낸다.
일꾼은 이 대화를 모르니 절차를 **통째로** 싣는다.

    감독 세션(<내 이름>)이 idea <id> 를 맡긴다 — <제목>.
    먼저 읽을 것: moai show <id>
    1. main 에서 펼친다 — idea 를 일감으로 바꾸는 길은 `moai idea promote <id> --from -`
       하나다. 이슈 하나짜리여도 에픽 + 이슈로 펼친다. `--dry-run` 을 먼저 본다
    2. 멤버를 `moai mv <멤버> in_progress` 로 집고 main 에
       "chore(tracker): <에픽> 를 워크트리에서 집는다" 로 커밋한다
    3. `git worktree add -b worktree-<에픽> .claude/worktrees/<에픽> main` 으로
       로컬 main 에서 뜨고 EnterWorktree(path) 로 들어간다. 이름은 idea id 가
       아니라 펼친 에픽 id 다
    4. 노트에 없는 설계 결정은 추측하지 말고 AskUserQuestion 으로 묻는다 —
       사람이 일꾼 창을 보고 있다
    5. 리뷰 이슈를 세워(규칙 3) `/code-review high --fix`. 반영은 별도 fix: 커밋,
       넘긴 것은 이슈 번호와 함께 노트
    6. main 을 받아 충돌을 풀고, 옆 세션과 병합이 겹치면 먼저 알린 뒤 main 에서
       `git merge worktree-<에픽>`. 시험 통과를 보고 멤버·리뷰 이슈를 done 으로 커밋
    7. 워크트리와 가지를 지운다
    8. SendMessage to "<내 이름>" 로 보고 — 머지 해시, 한두 줄 요약, 넘긴 것·새 idea

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
