---
name: moai-supervise
description: 같은 저장소에서 놀고 있는 Claude 세션들에 쌓인 idea 를 하나씩 나눠 주고 보고를 받을 때 쓴다. "감독해 줘", "idea 나눠 줘", "놀고 있는 세션에 일 시켜" 가 나오면.
---

# moai-supervise — 놀고 있는 세션에 idea 를 나눠 준다

감독은 **고르고, 보내고, 확인한다.** 코드를 고치지 않고, 병합하지 않고, 일꾼
대신 설계를 정하지 않는다. 병합은 일꾼이 하고, 겹치는 병합은 일꾼끼리 먼저
알린다.

**본 가지는 바퀴를 시작할 때 한 번 읽는다.** 일꾼이 워크트리를 뜨고 병합하는
곳이 루트 체크아웃이라 그 체크아웃의 지금 가지가 본 가지다 — 원격의 기본 가지는 루트와
다를 수 있고 낡았을 수 있다. 루트가 detached 면 `origin/HEAD`, 그것도 없으면 `main` 이다.
루트 체크아웃은 `git worktree list` 의 첫 자리라, 아래 한 줄은 저장소 어디서 불러도 —
워크트리 안에서도 — 루트의 가지를 낸다. 아무것도 안 나오면 git 이 낸 오류를 보고 멈춘다.

```sh
if w=$(git worktree list --porcelain); then b=$(printf '%s\n' "$w" | sed -n '1,/^$/s|^branch refs/heads/||p'); [ -n "$b" ] || b=$(git symbolic-ref -q refs/remotes/origin/HEAD | sed 's|^refs/remotes/origin/||'); echo "${b:-main}"; fi
```

읽은 이름을 아래 명령의 `<본 가지>` 와 일꾼에게 싣는 글의 `<본 가지>` 에 채운다.
**일꾼은 다시 읽지 않는다** — 워크트리 안에서 읽으면 제 가지가 나온다.

## 한 바퀴

**0. 먼저 거둔다 — 자리를 잃은 일.** 세션이 죽으면 집은 줄은 `in_progress` 로 남고
아무도 이어 하지 않는다. 새 idea 를 고르기 전에 본다.

    moai status --json                     warnings 에서 kind 가 "stranded" 인 것의 ids
                                           (워크트리 안이면 `--worktree` 를 붙여야 선다)
    moai show <id>                         `자리` 줄 — 아래 네 낱말 중 하나
                                           (워크트리 안이면 여기도 `--worktree` 가 있어야 선다)

`stranded` 는 집었는데 살아 있는 워크트리가 그 일을 안 쥔 줄이다 — 워크트리가 사라졌거나,
**워크트리 없이 루트에서 하던 일**이다. 둘은 이 줄만으로 안 갈린다: 2 의 세션 목록에서 루트에
산 세션이 있으면 그쪽일 수 있으니, 맡기기 전에 그 세션에 무엇을 쥐고 있는지 묻는다.

`자리` 줄(`--json` 의 `place`)은 넷이다. **맡기는 것은 `없다` 하나뿐이다.**

    <경로> (<가지>)   at        거기서 돈다. 들어가 이어 한다
    아직 안 보인다     fresh     방금 집었다 — 일꾼이 워크트리를 띄우는 틈이다. 그냥 둔다
    모른다             unknown   **못 읽은 옆 워크트리가 있다.** 거기일 수 있으니 안 맡긴다
    없다               lost      자리를 잃었다 — 이것만 거둔다

**`stranded` 가 조용한 것이 곧 거둘 것이 없다는 뜻은 아니다.** 못 읽는 옆 스냅샷이 하나라도
있으면 자리 판정이 통째로 `모른다` 로 접혀 이 경고가 저장소째로 잠긴다. `status --json` 의
`unreadable_worktrees` 키(사람 화면은 `옆 워크트리 문제 N건`)가 서 있으면 **그 워크트리를 먼저
고치고 다시 본다** — 그 전에 읽은 빈 목록은 "없다" 가 아니라 "못 셌다" 다.

집은 지 한 시간이 안 된 줄은 안 뜬다(일꾼이 워크트리를 띄우는 틈이다). **워크트리는
남았는데 거기서 일하던 세션이 죽은 것은 `stranded` 에 안 뜬다** — `git worktree list` 의
워크트리 중 2 의 스크립트에 `워크트리` 줄로 안 나오는 것이 그것이다.

- 그런 일이 있으면 **새 idea 보다 먼저** 놀고 있는 세션 하나에 이어 하기를 맡긴다. 3 의
  글 대신 아래를 싣고, 그 뒤에 3 의 글의 **4-1 부터 끝까지**를 통째로 잇는다 — 4-1 을 빼면
  이어받은 일꾼이 워크트리 안에서 트래커를 고치고, 끝을 자르면 닫은 뒤의 걸음(창 비우기)이
  빠진다

      감독 세션(<내 이름>)이 <에픽> 의 멈춘 일을 맡긴다 — 앞 세션이 끝을 못 냈다.
      먼저 읽을 것: moai show <에픽> (이력·노트) · moai show <멤버> (자리도 — 자리는 일에만 선다)
      모델: <모델> (<난이도> — <까닭>) — 난이도로 고른 제안이다. 모델은 `/model` 로 바꾸는데 그것은
         사람만 친다 — 이 창이 그 모델이 아니면 창을 보는 사람에게 청해 맞추고, 읽어 보니 더
         어려우면 같은 길로 한 단계 올린다(haiku → sonnet → opus). 리뷰 등급도 같은 축이다
      본 가지: <본 가지> — 감독이 루트에서 읽어 채웠으니 다시 읽지 않는다
      루트: <루트> — 2 의 `루트 자리`. 트래커를 고치는 것은 언제나 이 자리다(3 의 글의 4-1)
      - 워크트리가 있으면 EnterWorktree(path) 로 들어가 `git log <본 가지>..HEAD` 와
        `git status` 로 어디까지 했는지 읽고 이어 한다
      - 없으면 루트에서 다시 뜬다. 가지가 남아 있으면 그 가지로
        (`git worktree add .claude/worktrees/<에픽> worktree-<에픽>`), 없으면
        `git worktree add -b worktree-<에픽> .claude/worktrees/<에픽> <본 가지>`
      - 멤버의 칸은 이미 집혀 있다 — 다시 집지 않는다
      - 아래 걸음들이 가리키는 `2` 는 **경로를 준 트래커 커밋**이다 — 루트는 모든 세션이
        같이 쓰니 `git commit -m "…" -- .moai/` 로 친다. 병합이 열려 있으면(MERGE_HEAD)
        git 이 거절하니 그 병합이 끝나기를 기다렸다 다시 친다

- **이어 할지 놓을지는 감독이 정하지 않는다.** 놓을 일로 보이면(`moai mv <id> todo`·
  `moai defer <id> -m "왜"`) 사람에게 묻는다
- 이어 하기를 맡긴 일은 idea 와 같이 그 보고를 확인할 때까지 다시 안 보낸다

**1. 고른다.** 쌓인 idea 에서 지금 벌여 놓은 일과 부딪히지 않는 것만 남긴다.

    moai idea ls                           쌓인 것
    moai show -s in_progress,review        집혀 있는 것
    moai show <id>                         그 idea 가 어디를 건드리는가

`git worktree list` 도 본다. 이미 선 워크트리나 집힌 에픽과 **같은 파일·같은
영역**을 건드리는 idea 는 이번 바퀴에서 뺀다 — 둘이 같은 곳을 고치면 병합에서
한쪽이 다른 쪽을 기다린다. **이번 바퀴에 함께 보내는 idea 끼리도 견준다** — 일꾼은
받은 뒤에야 워크트리를 세우니, 방금 보낸 것은 아직 위 목록에 안 뜬다. 다음 idea 를
보낼 때도 이 셈을 다시 한다.

**보낸 idea 는 그 보고를 확인할 때까지 후보에서 뺀다.** 일꾼이 펼치기 전까지는
`moai idea ls` 에 그대로 남아, 둘째 일꾼에게 같은 idea 가 또 간다.

**2. 일꾼을 찾는다.** `ListAgents` 는 세션의 자리(cwd)를 안 보여 준다.
Claude Code 가 세션마다 적어 두는 `~/.claude/sessions/*.json` 을 읽는다
(`CLAUDE_CONFIG_DIR` 를 옮겼으면 그 아래다). 모노레포의 하위 프로젝트면 `.moai` 가
있는 그 하위가 루트다. 스크립트는 첫 줄 `루트 자리` 에 그 `<루트>` 를 낸다.

```sh
python3 - "$(git worktree list --porcelain | sed -n 's/^worktree //p' | head -1)" "$(git rev-parse --show-toplevel)" <<'PY'
import glob, json, os, sys
if not sys.argv[1]:
    sys.exit("git 저장소 안에서 부른다")
top, here = os.path.realpath(sys.argv[2]), os.path.realpath(os.getcwd())
while here not in (top, os.path.dirname(here)) and not os.path.isdir(os.path.join(here, ".moai")):
    here = os.path.dirname(here)
root = os.path.realpath(os.path.join(sys.argv[1], os.path.relpath(here, top)))
trees = os.path.join(root, ".claude", "worktrees") + os.sep
print("루트 자리", root)
home = os.environ.get("CLAUDE_CONFIG_DIR") or os.path.expanduser("~/.claude")
unread = 0
for f in glob.glob(os.path.join(home, "sessions", "*.json")):
    try:
        s = json.load(open(f))
        os.kill(s["pid"], 0)
        cwd = os.path.realpath(s["cwd"]) if s["cwd"] else None
    except OSError:
        continue
    except (ValueError, KeyError, TypeError, OverflowError):
        unread += 1
        continue
    if cwd is None:
        unread += 1
    elif cwd == root:
        print("루트    ", s.get("status"), s.get("name"))
    elif cwd.startswith(trees):
        print("워크트리", s.get("status"), s.get("name"), cwd)
if unread:
    print("못 읽은 파일", unread)
PY
```

- **맡기는 것은 자리가 루트이고 `idle`·`waiting` 인 세션뿐이다.** `busy` 는
  일하는 중이고, 그 밖의 값(`shell` 따위)은 뜻을 모르니 맡기지 않는다
- **보낸 idea 의 보고를 아직 확인하지 않은 세션은 뺀다.** 일꾼은 펼치고 집고 병합하는
  동안 루트에 있다 — 사람의 답이나 권한을 기다리면 `waiting`, 턴을 마치면 `idle` 로 뜬다
- 자리가 `<루트>/.claude/worktrees/*` 인 세션은 이 저장소에서 **일하는 중**이다.
  지켜보되 맡기지 않는다
- **다른 디렉터리의 세션은 건드리지 않는다**
- **맡기기를 거절한 세션은 후보에서 빼고 다시 보내지 않는다.** 제 사람이 준 일만
  받는 세션이 있다 — 한 번 거절했으면 그 뒤로는 알림도 걸지 않는다
- 파일은 세션이 끝나도 남고, 그 pid 를 다른 프로세스가 다시 쓰면 산 것처럼 읽힌다.
  보내기 전에 그 이름이 `ListAgents` 에도 뜨는지 한 번 본다
- 이 파일은 문서에 없는 속 파일이라 판이 바뀌면 필드가 달라질 수 있다. 스크립트가
  `못 읽은 파일` 을 내거나 못 돌면 `ListAgents` 로 이름을 보고, **이 기계의 세션에만**
  `pwd` 와 지금 하는 일을 물어 가린다 — 원격·클라우드 세션은 같은 경로를 대도 다른
  체크아웃이다

**2-1. 모델을 고른다 — 난이도 한 낱말로.** 리뷰 등급을 고르는 그 축이다. 축을 둘로 두면
브리프가 판단을 두 벌 들고, 어긋나는 날 싼 모델이 쓰기 경로를 맡는다.

| 난이도 | 무엇으로 재나 | 모델 | 리뷰 |
|---|---|---|---|
| `low` | 글·주석·한 줄 고침, 동작이 안 바뀐다 | `haiku` | `low` |
| `medium` | 한 파일 안의 동작 변경, 시험으로 둘러싸인 것 | `sonnet` | `medium` |
| `high` | 여러 파일·쓰기 경로·동시성·저장 형식·훅, 되돌리기 어려운 것 | `opus` | `high` |

에픽 끝의 리뷰(`xhigh`·`max`)는 언제나 `opus` 다 — 멤버마다 싸게 지나갔어도 한 번은 비싼
눈으로 전체를 본다(브리프 7 이 일꾼에게 싣는다). **망설여지면 한 단계 올린다.** 감독은 코드를
읽기 전에 고르므로 이것은 제안이고, 마지막 자는 이슈를 읽은 일꾼이다. 도는 세션의 모델은
`SendMessage` 로도 설정으로도 못 바꾼다 — 그 창의 사람이 `/model` 로 바꾼다.

**3. 보낸다.** 놀고 있는 세션 하나에 idea **하나**를 `SendMessage` 로 보낸다.
일꾼은 이 대화를 모르니 아래 글을
`<내 이름>`·`<id>`·`<제목>`·`<본 가지>`·`<모델>`·`<난이도>`·`<까닭>`·`<루트>` 를 채워
**통째로** 싣는다 — 일꾼이 받는 것은 이 글뿐이라, 일꾼이 지킬 것은 모두 이 안에 있다.
`<루트>` 는 2 의 `루트 자리` 다. **안 채우면** 일꾼이 워크트리 안에서 제 자리를 루트로 읽는다.
`<모델>`·`<난이도>`·`<까닭>` 은 2-1 에서 고른 짝과 그 까닭이다. **안 채우면** 그 자리표시자가
그대로 실려, 일꾼이 닫을 때 남기는 노트가 무엇이 일했는지 대신 `<모델>` 이라고 적는다.
`<까닭>` 은 백틱·`$` 없이 적는다 — 9-1 의 큰따옴표 안에 들어가 셸이 그것을 명령으로 푼다.
`<등급>` 은 채우지 않는다 — 5 에서 개발해 본 일꾼이 고르는 리뷰 등급의 자리다.

    감독 세션(<내 이름>)이 idea <id> 를 맡긴다 — <제목>.
    먼저 읽을 것: moai show <id>
    모델: <모델> (<난이도> — <까닭>) — 난이도로 고른 제안이다. 모델은 `/model` 로 바꾸는데 그것은
       사람만 친다 — 이 창이 그 모델이 아니면 창을 보는 사람에게 청해 맞추고, 읽어 보니 더
       어려우면 같은 길로 한 단계 올린다(haiku → sonnet → opus). 리뷰 등급도 같은 축이다
    본 가지: <본 가지> — 아래의 가지 이름이다. 감독이 루트에서 읽어 채웠으니 다시 읽지 않는다
    1. 루트에서 펼친다 — idea 를 일감으로 바꾸는 길은 `moai idea promote <id> --from -`
       하나다. 이슈 하나짜리여도 에픽 + 이슈로 펼친다. `--dry-run` 을 먼저 본다.
       그 idea 가 이미 done 이면(누가 펼쳤다) 펼치지 말고 감독에게 알린다 — 다시 펼치면
       에픽이 둘 선다
    2. 멤버를 `moai mv <멤버> in_progress --from todo` 로 집고 루트에서 커밋한다.
       **본 칸을 함께 준다** — 여기는 세션 여럿이 한 `.moai` 를 쓰는 자리라, 옆에서
       먼저 집은 줄을 뒤늦게 덮으면 둘이 같은 일을 한다. 0 아닌 코드가 오면 집힌
       것이니 그 멤버는 두고 감독에게 알린다. 루트는 모든 세션이
       같이 쓴다 — 남이 병합을 열어 둔 사이(MERGE_HEAD)에 친 커밋은 그 병합을 제 제목으로
       봉인한다. 그래서 트래커 커밋에는 경로를 준다. 병합이 열려 있으면 git 이 거절하니,
       그 병합이 끝나기를 기다렸다 다시 친다
         git commit -m "chore(tracker): <에픽> 를 워크트리에서 집는다" -- .moai/
    3. 2 의 커밋 뒤 곧바로 `git worktree add -b worktree-<에픽> .claude/worktrees/<에픽> <본 가지>`
       로 로컬 <본 가지> 에서 뜨고 EnterWorktree(path) 로 들어간다. 이름은 idea id 가 아니라
       펼친 에픽 id 다. 워크트리가 서기 전에는 루트의 다른 세션들이 이 멤버를 제 초점으로 읽는다
    4. 노트에 없는 설계 결정은 추측하지 말고 AskUserQuestion 으로 묻는다 —
       사람이 일꾼 창을 보고 있다
    4-1. **트래커는 언제나 루트의 것을 고친다.** `<루트>` 는 감독이 채운 루트 체크아웃의
       자리다 — 워크트리 안에서 짐작하지 않는다. 스냅샷을 고치는 명령 전부(`add`·`idea add`·
       `note`·`mv`·`edit`·`defer`·`rm`·`idea promote` …)를 워크트리 안에서 맨 `moai` 로
       부르면 그 워크트리의 `.moai` 가 바뀌어, 병합할 때 스냅샷이 충돌한다
       (합쳐도 남의 줄을 덮는다). 워크트리에서는 `moai -C <루트> <명령>` 으로 부르고,
       **리뷰 서브에이전트에게도** 같은 말을 준다 — 넘긴 것을 담다가 그 줄을 워크트리에
       적은 적이 있다. 이미 적었으면 `git checkout -- .moai` 로 되돌리고, 그 줄이 이미
       커밋됐으면 그 커밋까지 되돌린 뒤 루트에서 다시 담는다
    4-2. **tmux 를 시험하면 떼어 낸 서버에서만 한다** — 모든 호출에 `env -u TMUX tmux -L <고유 이름>`
       (또는 `tmux -S <스크래치패드 안의 소켓>`). `-L`/`-S` 없는 `kill-server` 는 쓰지 않는다: tmux
       안에서 맨 `tmux` 는 사람의 기본 서버로 가 모든 세션을 죽이고, `TMUX_TMPDIR` 로는 안 갇힌다.
       남이 띄운 판에는 키를 보내지 않는다. **리뷰 서브에이전트에게도** 이 말을 준다 — 서버 전체를
       죽인 것이 리뷰 서브에이전트였다
    5. 리뷰 이슈를 세워(규칙 3) `/code-review <등급> --fix`. 등급은 개발한 난이도로
       `low`·`medium`·`high` 에서 고른다 — 머리의 모델을 고른 그 잣대다.
       `low` — 글·주석·한 줄 고침, 동작이 안 바뀐다
       `medium` — 한 파일 안의 동작 변경, 시험으로 둘러싸인 것
       `high` — 여러 파일·쓰기 경로·동시성·저장 형식·훅, 되돌리기 어려운 것
       망설여지면 한 단계 올리고, 고른 등급과 까닭은 관점(`-b`)에 한 줄로 적는다.
       반영은 별도 fix: 커밋, 넘긴 것은 이슈 번호와 함께 노트.
       루트에서 세우거나 집은 리뷰 이슈를 워크트리의 훅이 못 봐서 막힐 때만 — 훅은 그
       워크트리의 스냅샷만 읽는다 — 같은 관점·단계·`--fix` 범위로 리뷰 서브에이전트를
       돌린다. 리뷰 이슈·관점(`-b`)·원문 노트·닫는 `-m` 은 그대로 남긴다. 관점이 없다
       같은 다른 거절은 돌아가지 않고 거절문이 내는 명령대로 고친다
    6. 멤버의 일이 다 끝나면 워크트리에서 <본 가지> 를 받아 충돌을 풀고 시험을 돌린다. 고칠
       것은 여기서 고친다 — 워크트리가 남아 있는 동안 루트에서는 규칙 2 가 편집을 막는다
    7. 병합 전에 에픽 전체를 `/code-review <xhigh|max> --fix` 로 본다 — 멤버가 서로 거의
       안 닿고 각자 high 를 지났으면 xhigh, 표면을 가로지르거나 설계 결정이 여럿이거나
       쓰기·저장·훅을 건드렸으면 max. 이 리뷰는 머리의 모델과 상관없이 `opus` 로 본다 — 창이
       `opus` 가 아니면 부르기 전에 창을 보는 사람에게 `/model opus` 를 청한다(리뷰 에이전트는
       창의 모델을 물려받는다). 고른 등급과 까닭은 관점(`-b`)에 적는다. 가지가 <본 가지> 를 떠난
       자리(`git merge-base <본 가지> HEAD`)부터의 diff 다. 6 에서 <본 가지> 를 받았으니 충돌을 푼
       자리도 든다. 리뷰 이슈를 따로 세운다. 막히면 5 의 길로 간다
         moai add "리뷰 — <무엇을 보는가>" -t review --parent <에픽> -b "<무엇을 왜 보는가>"
       이 줄도 워크트리에서 부르니 4-1 대로 `moai -C <루트>` 로 친다 — 5 의 리뷰 이슈도 같다
    8. ExitWorktree(keep) 로 루트로 돌아온다 — 워크트리 안에서 그것을 지우면 세션의
       자리가 사라진 디렉터리에 남아 감독이 다시는 이 세션을 루트로 못 본다.
       옆 세션과 병합이 겹치면 먼저 알린 뒤 루트에서 **한 번에** 병합한다.
       `--no-commit` 을 쓰지 않는다. `--no-ff` 가 없으면 fast-forward 로 끝나 병합 커밋이 안 선다
         git merge --no-ff worktree-<에픽> -m "merge: …"
       루트의 `.moai` 에 커밋 안 된 옆 세션의 줄이 있으면 병합이 거절된다 — 2 처럼 경로를
       준 커밋으로 먼저 담는다. 충돌로 멈추면 루트에서 풀지 않는다 — `git merge --abort`
       로 되돌리고 EnterWorktree(path) 로 워크트리에 돌아가 6 부터 다시 한다
    9. 병합이 실제로 끝났으면 루트에서 `git worktree remove .claude/worktrees/<에픽>` 과
       `git branch -d worktree-<에픽>` 으로 워크트리와 가지를 지운다
    9-1. 닫기 전에 **무엇이 이 일을 했는지** 멤버마다 한 줄로 남긴다 — 머리의 제안이 아니라
       이 창에서 **실제로 돈 모델**이다. 아래 줄은 감독이 제안으로 채워 보냈으니, 올렸거나 창이
       처음부터 다른 모델이었으면 모델·난이도를 실제 것으로 고치고 까닭에 그 까닭을 적는다 —
       다음 사람이 "이만한 일에 무엇이 붙었나" 를 거기서 읽는다. 필드가 아니라 노트다:
       저널은 상태 계산에 안 읽히고 파생값은 저장하지 않는다
         moai note <멤버> "model: <모델> (<난이도> — <까닭>)"
    10. 그 뒤에 닫는다. **`moai mv <멤버> done` 은 그 병합이 실제로 끝난 뒤에만 친다** —
       병합 전에 옮겼다가 되돌린 일꾼이 있었다. 워크트리가 남아 있으면 훅이 이 일을 옆
       워크트리의 것으로 읽어 `-m` 없는 리뷰 닫기를 못 막는다. 리뷰 이슈는 무엇이
       나왔는지를 남기며 닫는다
         moai note <리뷰 id> -b - < <리뷰 원문>   리뷰가 낸 글을 그대로
         moai mv <리뷰 id> done -m "<무엇을 반영하고 무엇을 넘겼나>"
       시험 통과를 보고 2 처럼 경로를 준 커밋으로 루트에 남긴다
    11. SendMessage to "<내 이름>" 로 보고 — 머지 해시, 펼친 에픽 id, 한두 줄 요약,
       넘긴 것·새 idea
    12. 마지막으로 **창을 비워도 되는 때를 알린다.** 이어받을 한 줄을 남겨
       (`moai note <에픽> "다음: …"`) 2 처럼 경로를 준 커밋으로 루트에 담고 — 10 의 커밋 뒤에
       적은 줄이라 안 담으면 공유 루트에 남아 남의 커밋에 쓸려 들어간다 — 그 창을 보는 사람에게
       한 줄로, 지금 `/clear` 해도 된다고. 맥락은 대화가 아니라 트래커에 산다: 이슈 본문·노트·
       리뷰 원문·커밋 메시지. 제 맥락 사용량을 볼 수 있으면 그 수도 그 줄에 담는다.
       **반대도 같은 줄에서 말한다** — 리뷰가 백그라운드에서 도는 중, 머지 충돌을 푸는 중,
       사람의 답을 기다리는 중, 감독의 다음 글이 이 창에 온 뒤에는 지우지 말라고. 그때 지우면
       아직 트래커에 안 옮긴 것이나 받은 글이 사라진다.
       tmux 면 감독이 보고를 확인하고 이 창에 `/clear` 를 직접 칠 수 있다 — 감독은 `다음:` 노트가
       선 것을 보고 친다. 그래서 그 노트가 마지막 걸음이다: 남은 일(백그라운드 리뷰 따위)이 있으면
       노트 전에 끝내고, 못 끝내면 노트 전에 감독에게 그렇다고 한 줄 더 보낸다(보고는 11 에서
       이미 갔다) — 감독은 그런 창을 비우지 않는다

**4. 기다린다.** 일하는 세션에는 메시지 없이 `notify_when_idle: true` 로
걸어 둔다. **`ListAgents` 를 되풀이해 훑지 않는다** — 알림이 온다. 알림은 한 번뿐이라,
보고 없이 온 알림(일꾼이 사람에게 묻고 턴을 마쳤다)이면 다시 걸어 둔다.

감독이 루트에 있으면, 일꾼이 멤버를 집고 워크트리를 세우기 전의 틈에 감독의 턴이
끝날 때 훅이 그 멤버를 "아직 집고 있는 것" 으로 붙든다. **그 멤버는 일꾼의 것이다** —
옮기거나 미루거나 노트를 달지 않고 그대로 턴을 마친다.

**5. 보고를 확인하고 다음을 보낸다.** 보고를 믿기 전에 셋을 본다.

    git merge-base --is-ancestor <머지 해시> <본 가지> && echo 있다   머지가 본 가지에 있는가
    moai show <에픽>                       펼친 에픽과 멤버가 done 인가
    git worktree list                      그 워크트리가 사라졌는가

보고가 맞고 **그 세션이 턴을 마쳤으면**(보고는 11 이고 일꾼은 12 를 마저 한다) 그 창이
비우기 좋은 자리라고 짚어 줄 수 있다 — 일꾼도 제 창에서 그렇게 말한다(브리프 12). 셋이
보는 것은 머지·닫기·워크트리뿐이라 노트까지 읽지는 않는다. **짚었으면 다음 idea 는 사람이
그 창을 비웠거나 안 비운다고 한 뒤에 보낸다** — 먼저 보낸 글은 뒤늦은 `/clear` 에 같이
사라지고, 그 idea 와 세션은 오지 않을 보고를 기다리며 후보에서 빠져 있다.

`<에픽>` 은 보고에 실린 에픽 id 다. idea 는 펼칠 때 이미 done 이 되고 멤버를 안 보여 줘,
`moai show <id>` 로는 일이 끝났는지 모른다 — 보고에 없으면 그 idea 의 이력 "… 로
펼쳤다" 에서 읽는다.

셋이 맞으면 — tmux 면 5-1 로 그 창을 먼저 비우고 — 그 세션에 다음 idea 를 보낸다.
어긋나면 그 세션에 무엇이 남았는지 묻고, 대신 끝내지 않는다.

**5-1. tmux 면 감독이 그 창을 비운다.** 사람에게 짚고 기다리는 대신 감독이 그 판에
`/clear` 를 친다. 셋이 맞은 뒤에만, 그리고 `moai show <에픽>` 의 이력에 일꾼이 12 에서
남긴 `다음:` 노트가 선 뒤에만 부른다 — 그 노트가 12 의 마지막 걸음이라, 없으면 일꾼이
아직 트래커에 옮기는 중이다. 노트는 **보고 뒤에** 선 것만 센다 — `다음:` 은 끝을 못 낸
세션이 남기는 이어받기 줄이기도 해서, 0 에서 거둔 에픽에는 앞 세션의 것이 이미 있다.
보고(11)는 12 보다 먼저 오니 보고를 받은 때는 대개 노트가 아직 없고 일꾼은 `busy` 다 —
사람에게 짚지 말고 4 의 알림을 걸어 두고, 그 알림이 온 뒤에 본다. 비우면 그 일꾼이 쥔
대화가 통째로 사라지니 확인보다 먼저 부르지 않는다. `<세션>` 은 보고를 보낸 세션의 이름,
`<루트>` 는 2 의 `루트 자리` 다.

```sh
python3 - '<세션>' '<에픽>' '<내 이름>' '<루트>' <<'PY'
import glob, json, os, re, subprocess, sys, time
name, epic, me, root = sys.argv[1:5]
home = os.environ.get("CLAUDE_CONFIG_DIR") or os.path.expanduser("~/.claude")
erased = False
def skip(why, then="사람에게 비워도 된다고만 짚는다"):
    print("안 비운다 —", why, "—", then)
    if erased:
        print("치던 글은 이미 지웠다 — 위에 옮긴 `치던 글` 을 그 창의 사람에게 돌려준다")
    sys.exit(0)
def tmux(*args):
    return subprocess.run(["tmux", *args], capture_output=True, text=True)
def read(f):
    try:
        with open(f) as fh:
            s = json.load(fh)
        os.kill(s["pid"], 0)
        return s
    except (OSError, ValueError, KeyError, TypeError, OverflowError):
        return None
def session():
    found = [(f, s) for f in glob.glob(os.path.join(home, "sessions", "*.json")) for s in [read(f)] if s and s.get("name") == name]
    return found[0] if len(found) == 1 else (None, None)
def parents(pid):
    while pid > 1:
        yield pid
        try:
            out = subprocess.run(["ps", "-o", "ppid=", "-p", str(pid)], capture_output=True, text=True).stdout.strip()
        except OSError:
            return
        pid = int(out) if out.isdigit() else 0
PROMPT = "\u276f"
SGR = "\x1b\\[([0-9;:]*)m"
def screen(pane, colour=False):
    args = ["capture-pane", "-p"] + (["-e"] if colour else []) + ["-t", pane]
    return tmux(*args).stdout.split("\n")
def draft(pane):
    lines = screen(pane)
    at = [i for i, l in enumerate(lines) if l.startswith(PROMPT)]
    box = []
    for line in lines[at[-1] :] if at else []:
        if line.startswith("─"):
            # 첫 줄은 프롬프트와 빈칸 하나, 이어지는 줄은 두 칸 — 그만큼만 벗겨 들여쓰기를 지킨다.
            # 그 앞머리가 아닌 줄은 안 자른다 — 화면이 달리 그리는 날 두 글자가 말없이 깎이고,
            # 옮긴 글이 사람에게 남은 단 하나의 복사라 줄어든 것을 아무도 못 본다.
            head = (PROMPT + " ", "  ")
            return "\n".join((l[2:] if l[:2] in head else l[1:] if l[:1] == PROMPT else l).rstrip() for l in box).strip("\n")
        box.append(line)
def grey(code):
    """이 글자색이 흐린 회색인가. 256색 회색 계단과 참색(r=g=b) 을 함께 본다 — Claude Code 의
    색은 테마의 16진값이라, 판이 참색을 받으면 `38;5;244` 가 아니라 `38;2;136;136;136` 으로 온다.
    검정 쪽은 회색이 아니다 — 밝은 테마는 사람이 친 글을 `rgb(0,0,0)` 으로 그린다."""
    n = code.split(";")
    if code == "90":
        return True
    if n[:2] == ["38", "5"] and len(n) == 3 and n[2].isdigit():
        return int(n[2]) == 8 or 238 <= int(n[2]) <= 247
    if n[:2] == ["38", "2"] and len(n) == 5 and all(p.isdigit() for p in n[2:]):
        return len(set(n[2:])) == 1 and 64 <= int(n[2]) < 160
    return False
def sgr(code, was):
    """SGR 한 조각을 (흐림 속성, 흐린 글자색, 뒤집힘) 으로 접는다. tmux 는 글자색을 따로 내보내고
    (`\x1b[2m\x1b[37m`) 속성은 한 조각에 모은다 — 속성이 하나 빠지면 리셋을 앞에 붙여 `0;2`,
    둘이 한꺼번에 서면 `2;3` 이다. 그래서 속성 조각은 낱낱이 읽는다."""
    attr, fg, rev = was
    n = code.split(";")
    if n[0] in ("38", "39") or (len(n) == 1 and n[0].isdigit() and (30 <= int(n[0]) <= 37 or 90 <= int(n[0]) <= 97)):
        return attr, grey(code), rev
    if n[0] in ("48", "58"):
        return was
    for p in n:
        if p in ("", "0"):
            attr, fg, rev = False, False, False
        elif p in ("2", "22"):
            attr = p == "2"
        elif p in ("7", "27"):
            rev = p == "7"
    return attr, fg, rev
def dim_only(pane):
    """입력 칸에 보이는 글이 모두 흐린 색인가 — 사람이 친 글이 아니라 Claude Code 의 제안 글이다."""
    lines = screen(pane, True)
    bare = lambda l: re.sub(SGR, "", l)
    # 입력 칸은 `draft` 와 **같은 줄**에서 연다. `in` 으로 찾으면 사람이 친 글에 든 프롬프트
    # 표시가 그 아래로 끌고 가 위의 사람 글을 못 본다. 상자 끝도 `startswith` 로 본다 — 사람이
    # 붙여 넣은 줄 속의 붙임표 하나에 그 자리에서 참을 내면 사람의 글 뒤에 `/clear` 가 붙는다.
    at = [i for i, l in enumerate(lines) if bare(l).startswith(PROMPT)]
    if not at:
        return False
    # 색은 화면 맨 위부터 접는다 — tmux 는 줄이 바뀌어도 같은 색을 다시 내보내지 않아, 접힌
    # 제안 글의 둘째 줄은 색 조각 없이 온다. 흐린 글자를 하나도 못 봤으면 참이 아니다.
    was, prompt, cursor, seen = (False, False, False), True, True, False
    for n, line in enumerate(lines):
        if n > at[-1] and bare(line).startswith("─"):
            return seen
        for i, piece in enumerate(re.split(SGR, line)):
            if i % 2:
                was = sgr(piece, was)
                continue
            if n < at[-1]:
                continue
            if n == at[-1] and prompt and piece:
                piece, prompt = piece[1:], False
            # Claude Code 는 빈 칸의 커서를 제안 글 첫 글자에 뒤집어 그린다(흐림 없이). 프롬프트
            # 줄의 첫 글자가 뒤집혀 있으면 그 한 칸만 커서로 빼고, 나머지는 그대로 센다.
            if n == at[-1] and cursor and piece.strip():
                cursor = False
                if was[2] and not (was[0] or was[1]):
                    piece = piece.lstrip()[1:]
            if piece.strip():
                if not (was[0] or was[1]):
                    return False
                seen = True
    return False
def looks(fmt):
    return tmux("display-message", "-p", "-t", pane, fmt).stdout.strip()
QUIET = '#{pane_in_mode}#{pane_synchronized}'
if not os.environ.get("TMUX"):
    skip("tmux 밖이다")
try:
    tmux("-V")
except OSError:
    skip("tmux 가 없다")
f, s = session()
if not s:
    skip("산 세션 " + name + " 을 하나로 못 찾았다")
pane = str(s.get("tmux") or "").rpartition(".")[2]
if not pane.startswith("%"):
    skip("세션 파일에 판이 없다")
if pane == os.environ.get("TMUX_PANE"):
    skip("감독 제 창이다")
if s.get("status") != "idle":
    skip("idle 이 아니다 — " + str(s.get("status")), "4 의 알림을 걸어 두고 그 알림이 온 뒤에 다시 부른다")
cwd = str(s.get("cwd") or "")
if not cwd or os.path.realpath(cwd) != os.path.realpath(root):
    skip("루트에 없다 — 아직 워크트리 안이다")
owner = looks('#{pane_pid}')
if not owner.isdigit() or int(owner) not in parents(int(s["pid"])):
    skip("판 " + pane + " 이 그 세션의 것이 아니다")
if looks(QUIET) != "00":
    skip("판이 복사 모드이거나(사람이 스크롤해 읽는 중) 다른 판과 묶여 있다")
kept = draft(pane)
if kept is None:
    skip("입력 칸을 못 읽었다")
if kept:
    print("치던 글 —", name, pane)
    print(kept)
for _ in range(20):
    left = draft(pane)
    if left == "":
        break
    if left is None or looks(QUIET) != "00":
        skip("지우던 입력 칸을 놓쳤다")
    erased = True
    tmux("send-keys", "-t", pane, "C-e", "C-u", "DC")
    time.sleep(0.2)
else:
    # 지워 보고 가른다(사용자 결정): 마지막 한 번에도 안 지워진 글이 모두 흐린 색이면 사람이 친
    # 것이 아니라 Claude Code 의 제안 글이다 — 그것은 `/clear` 앞에 붙지 않으니 그대로 친다.
    # 치던 글과 같기를 바라지 않는다 — 사람의 글을 지운 빈 칸에 제안 글이 다시 서면, 치던 글은
    # 이미 옮겼고 남은 것은 제안 글뿐이다.
    rest = draft(pane)
    if rest and rest == left and dim_only(pane):
        if rest == kept:
            print("위의 `치던 글` 은 흐린 제안 글이었다 — 사람이 친 것이 아니다")
            erased = False
            # 사람의 글이 아니니 상태줄에 "감독 창에 옮겼다" 고 말하지 않는다 — 그 말을 읽은 사람이
            # 감독 창에서 제가 쓴 적 없는 글을 찾는다.
            kept = ""
    elif rest != "":
        # 하나도 안 지워졌으면 그 글은 아직 그 칸에 있다 — "이미 지웠다" 고 하면 감독이 그 창에
        # 그대로 있는 글을 사람에게 한 벌 더 돌려준다.
        erased = rest != kept
        skip("입력 칸을 못 비웠다")
if (read(f) or {}).get("status") != "idle":
    skip("그새 idle 이 아니다")
if looks(QUIET) != "00":
    skip("그새 판이 복사 모드로 갔다")
say = "감독 " + me + ": " + epic + " 보고를 확인했다 — 이 창을 /clear 한다" + (". 치던 글은 감독 창에 옮겼다" if kept else "")
for client in tmux("list-clients", "-t", pane, "-F", '#{client_name}').stdout.split("\n"):
    if client:
        tmux("display-message", "-c", client, "-d", "8000", "-t", pane, say)
tmux("send-keys", "-t", pane, "-l", "/clear")
time.sleep(0.5)
tmux("send-keys", "-t", pane, "Enter")
for _ in range(30):
    time.sleep(0.5)
    now = read(f)
    if now and now.get("sessionId") != s.get("sessionId"):
        print("비웠다 —", name, pane, epic)
        sys.exit(0)
print("비웠는지 모른다 —", name, pane, "— 다음 글을 보내기 전에 그 창을 본다")
PY
```

- **맡긴 세션에만 부른다.** 보고를 보낸 세션이 곧 일꾼이라 감독 제 창과 다른 감독의 창은
  이름에서 이미 빠진다. 스크립트도 제 판(`$TMUX_PANE`)은 한 번 더 거른다
- **tmux 가 없으면 조용히 건너뛴다.** `$TMUX` 가 없거나 `tmux` 가 없으면 `안 비운다` 한 줄을
  내고 0 으로 끝난다 — 그때는 위처럼 사람에게 짚고 기다린다. 스크립트가 `안 비운다` 를 내면
  그 줄의 끝이 말하는 대로 한다 — `idle 이 아니다` 만 알림을 기다려 다시 부르고, 나머지는
  사람에게 짚는다
- **`idle` 인 판에만 친다.** `busy`·`waiting`·`shell` 에 치면 도는 턴이나 사람의 답 사이에
  글자가 끼어든다. 보내기 직전에 한 번 더 읽는다. 일꾼이 12 에서 "지우지 말라" 고 한
  때 — 리뷰가 백그라운드에서 도는 중, 머지 충돌을 푸는 중, 사람의 답을 기다리는 중 — 도
  그대로 산다. 보고나 그 뒤에 온 글에 그런 것이 남았다고 적혀 있으면 부르지 않는다
- **복사 모드인 판, `synchronize-panes` 로 묶인 판에도 치지 않는다.** 복사 모드면 사람이
  스크롤해 읽는 중이고, 친 글자가 복사 모드의 키로 가 `/` 가 검색을 연다. 묶인 판이면
  친 글자가 그 창의 판 모두로 가 옆 일꾼의 대화까지 지운다
- **판은 세션 파일의 `tmux` 필드(`세션:@창.%판`)에서 읽고, 그 판의 프로세스가 그 세션을
  낳았는지 본다.** 세션 파일은 세션이 끝나도 남고 판 번호는 다시 쓰여, 낡은 파일이 가리키는
  판에는 남의 세션이 산다
- **치던 글은 지우고 친다**(사용자 결정). 그대로 치면 `치던 글/clear` 가 일꾼에게 프롬프트로
  간다. 지우기 전에 화면에서 그 글을 읽어 `치던 글` 로 감독 창에 옮기고, 그 판을 보는
  클라이언트의 상태줄에 한 줄이 그렇다고 말한다 — 사람은 그 글을 감독 창에서 찾는다. Claude
  Code 의 `Ctrl+Y` 는 여러 줄 글의 마지막 줄만 되살려 기댈 수 없다. 입력 칸은 Claude Code
  화면에서 프롬프트 표시(U+276F)가 선 마지막 줄로 읽는다. 그 화면도 세션 파일처럼 문서에 없는
  것이라, 못 읽으면 치지 않는 쪽으로 넘어진다. 지우다가 멈추면 `치던 글은 이미 지웠다` 가
  따라 나온다 — 그때는 옮긴 글을 그 창의 사람에게 돌려준다
- **옮길 때 앞머리 두 칸만 벗긴다**(사용자 결정). 첫 줄은 프롬프트와 빈칸 하나, 이어지는 줄은
  두 칸이고 나머지는 화면 그대로다 — 줄마다 다듬으면 들여쓴 코드가 납작해져 돌아간다.
  그 앞머리가 아닌 줄은 **안 자른다** — 화면이 달리 그리는 날 두 글자가 말없이 깎이는데, 옮긴
  글은 사람에게 남은 단 하나의 복사라 줄어든 것을 아무도 못 본다.
  화면이 접은 줄과 사람이 친 줄바꿈은 가를 수 없으니, 옮긴 글에 줄바꿈이 하나 더 보일 수 있다
- **안 지워지는 글은 지워 보고 가른다**(사용자 결정). Claude Code 가 빈 칸에 띄우는 흐린 제안
  글은 사람이 친 것이 아니라 지워지지도 않는다. 스무 번 쳐도 그대로이고 그 글이 모두 흐린
  색이면(`capture-pane -e`) 제안 글로 보고 `/clear` 를 친다 — 제안 글은 `/clear` 앞에 안 붙는다.
  색으로만 가르지 않는 까닭은, 사람이 친 글을 흐리게 그리는 판이 있으면 그 글 뒤에 `/clear` 가
  붙기 때문이다. 지워지는 글은 언제나 사람의 것으로 본다. Claude Code 는 빈 칸의 커서를 제안 글
  첫 글자에 뒤집어 그리니 그 한 칸은 글로 안 센다. 사람의 글을 지운 빈 칸에 제안 글이 다시
  서도 같다 — 치던 글은 이미 옮겼으니 그대로 친다
- **비우기와 다음 배정을 한 호흡에 하지 않는다.** `/clear` 는 큐에 쌓인 글을 함께 지운다.
  스크립트가 `비웠다` 를 낸 — 세션 id 가 바뀐 — 뒤에 다음 idea 를 보내고, `비웠는지 모른다`
  면 그 창이 어떤지 보기 전에는 보내지 않는다
- **비웠으면 제 창에 한 줄 남긴다** — `<세션> 판 %N 을 비웠다 (<에픽>)`. 사람이 그 창을
  보다가 화면이 사라진 까닭을 감독 창에서 찾는다
- **시험으로 살아 있는 일꾼의 창에 치지 않는다.** 시험할 판은 **떼어 낸 tmux 서버**에 띄우고,
  그 서버에 닿는 호출 **모두** — `new-session`·`send-keys`·`capture-pane`·`display-message`·
  `kill-session` — 에 같은 이름을 준다. 스크립트를 그 판에 돌릴 때는 `-L` 을 끼워 넣는
  `tmux` 감싸개를 `PATH` 앞에 둔다

      env -u TMUX tmux -L <고유 이름> new-session -d -s <판> …
      env -u TMUX tmux -L <고유 이름> capture-pane -p -t <판>
      env -u TMUX tmux -L <고유 이름> kill-server          치울 때 — 그 이름의 서버만 죽는다

  **`-L`/`-S` 없는 `tmux kill-server` 는 쓰지 않는다.** tmux 안에서 맨 `tmux` 는 `$TMUX` 를 따라
  사람의 기본 서버로 가, 그 기계의 판과 세션이 모두 한꺼번에 죽는다. `TMUX_TMPDIR` 로는 안
  갇힌다 — `$TMUX` 가 이긴다. 맨 `tmux new-session -d` 도 기본 서버에 판을 세우는 것이라 격리가
  아니다 — 치우려면 기본 서버에 `kill-*` 를 쳐야 하고, 그 길로 서버 전체가 죽은 적이 있다

## 공유 루트

루트 체크아웃은 **모든 세션이 같이 쓴다.** 한 세션이 병합을 열어 둔 사이(`MERGE_HEAD`)에
다른 세션이 트래커 노트를 커밋하면, 그 커밋이 남의 병합을 제 제목으로 봉인한다 — 실제로
그렇게 됐다. 그래서 감독이든 일꾼이든 루트에서는:

- 트래커 커밋에 경로를 준다 — `git commit -m "…" -- .moai/`. 병합이 열려 있으면 git 이
  경로 준 커밋을 거절하니, 그 병합을 연 세션이 끝낼 때까지 기다렸다 다시 친다. 경로 없는
  `git commit` 은 `git status` 를 보고 쳐도 그 병합을 그대로 봉인한다
- 제 병합은 `git merge --no-ff <가지> -m "…"` 한 번으로 끝낸다. `--no-commit` 을 쓰지 않는다.
  충돌로 멈추면 루트에서 풀지 않고 `git merge --abort` 한다

## 멈출 때

- 부딪히지 않는 idea 가 없거나 놀고 있는 세션이 없으면 사람에게 그렇게 말하고
  멈춘다 — 부딪히는 idea 를 억지로 보내지 않는다
- 일꾼이 사람의 결정을 기다리면 감독이 대신 답하지 않는다. 결정은 사람의 것이다
