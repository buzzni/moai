#!/usr/bin/env python3
"""사람 없이 도는 에이전트 여럿. moai 가 다음 일을 고르고, 일꾼 N 이 겨뤄 집는다.

    AGENT_WORK=./do-one.sh python3 examples/python-agents/agents.py --agents 3

저장소 뿌리(`.moai` 가 있는 git 저장소)에서 돌린다. 일꾼마다 이렇게 돈다.

  1. `moai ready --json --worktree` 로 집을 일을 본다 — 옆 워크트리에서 집은 것도 겹쳐 본다
  2. `moai mv <id> in_progress --from <본 칸>` 으로 집는다. 옆 일꾼이 먼저 집었으면 진다
  3. 뿌리의 `$AGENT_BASE`(기본 `main`)에서 그 일의 워크트리를 띄운다 — 가지는 `agent/<id>`
  4. 그 워크트리 안에서 `$AGENT_WORK <id> <제목>` 을 돌리고, 출력을 노트로 남긴다
  5. 일이 스스로 칸을 옮기거나 미루지 않았으면 done 으로 닫는다

**워크트리는 뿌리 밑 `.worktrees/` 에 선다**(`AGENT_WORKTREES` 로 바꾼다). 뿌리의 `git status` 에
안 보이게 하려면 `.gitignore` 에 `/.worktrees/` 를 적는다.

**병합은 안 한다.** 워크트리와 가지는 그대로 남는다 — 무엇을 어디에 합칠지는 일의 판단이라,
예제가 대신 정하면 가장 위험한 걸음을 가장 모르는 자리가 진다.

**트래커는 늘 뿌리에 쓴다**(`moai -C <뿌리>`). 워크트리 안의 `.moai` 를 고치면 합칠 때 스냅샷이
부딪치고, 옆 일꾼은 그 집기를 못 본다 — 겨루기는 같은 `.moai` 를 쓸 때만 성립한다. **일도 그렇게
쓴다.** 일은 워크트리를 cwd 로 돌아 맨 `moai` 는 워크트리의 `.moai` 를 고치고, 그러면 5 가 그
판단을 못 보고 done 으로 덮는다. 그래서 일에게 뿌리를 `AGENT_ROOT` 로, 이 예제가 쓰는 moai 를
`MOAI`(절대 경로)로 넘긴다 — `"$MOAI" -C "$AGENT_ROOT" mv "$1" review`.

옆 워크트리에만 있는 줄(뿌리에 없다)은 뿌리에서 못 집으니 건너뛴다.

AGENT_WORK 는 이슈 id 와 제목을 인자로 받는 실행 파일이다 (예: `claude -p` 를 감싼 스크립트).
일꾼 여럿이 한꺼번에 돌므로 stdin 은 주지 않는다. 실패하면 그 일을 in_progress 로 둔 채 그
일꾼만 멈추고, 끝에 1 로 끝난다 — 사람이 볼 차례다. 필요한 것: python 3.9+, git, 기본 칸
(todo → in_progress → done), 누가 하는지(git config 또는 MOAI_ACTOR="이름 (메일)"). PATH 의
moai 가 아닌 것을 쓰려면 MOAI=./target/release/moai. 표준 라이브러리만 쓴다.
"""

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
import threading

# 빈 값은 안 준 것이다 — bash 판의 `${MOAI:-moai}` 와 같다. `AGENT_WORKTREES=` 가 빈 채로 새면
# 워크트리가 뿌리 바로 밑(`<뿌리>/<id>`)에 흩어진다.
MOAI = os.environ.get("MOAI") or "moai"
WORK = os.environ.get("AGENT_WORK") or ""
BASE = os.environ.get("AGENT_BASE") or "main"
ROOT = os.getcwd()
TREES = os.environ.get("AGENT_WORKTREES") or os.path.join(ROOT, ".worktrees")

# `git worktree add` 는 저장소의 관리 디렉터리(`.git/worktrees`)를 고친다 — 일꾼 둘이 한꺼번에
# 띄우면 한쪽이 잠금에 걸려 실패한다. 띄우는 동안만 한 줄로 선다.
TREE_LOCK = threading.Lock()
# 여러 일꾼의 한 줄이 서로 끼어들지 않게 한다.
SAY_LOCK = threading.Lock()
# 이 판에 **일꾼 누구든 집은** 줄. 일꾼마다의 `seen` 만으로는 일이 첫 칸으로 되돌린 줄을 그것을
# 못 본 옆 일꾼이 다시 집어 같은 일을 또 한다.
WORKED = set()
WORKED_LOCK = threading.Lock()


def say(who, text, err=False):
    with SAY_LOCK:
        print(f"[{who}] {text}", file=sys.stderr if err else sys.stdout, flush=True)


def moai(*args, stdin=None):
    """뿌리의 트래커로 moai 를 부른다. 종료 코드와 stdout·stderr 를 돌려준다.

    **UTF-8 로 주고받는다.** moai 의 입출력은 늘 UTF-8 인데 `text=True` 는 로캘을 따른다 — UTF-8 이
    아닌 로캘에서는 한글 노트를 넘기다 인코딩 오류로 스레드가 죽는다.
    """
    p = subprocess.run(
        [MOAI, "-C", ROOT, *args], input=stdin, capture_output=True, encoding="utf-8", errors="replace"
    )
    return p.returncode, p.stdout, p.stderr


def git(*args):
    return subprocess.run(["git", "-C", ROOT, *args], capture_output=True, encoding="utf-8", errors="replace")


def ready():
    """집을 일. **덜 나온 목록으로 일하지 않는다** — 0 아닌 코드는 못 읽는 줄이 있다는 뜻이다."""
    code, out, err = moai("ready", "--json", "--worktree")
    if code != 0:
        raise RuntimeError(err.strip() or "moai ready 가 실패했다")
    queue = json.loads(out)
    if "ready" not in queue:
        # `.moai` 밖이면 등록한 프로젝트마다의 한눈 보기(`{"projects":…}`)가 온다 — 그것을 빈 큐로
        # 읽으면 할 일이 쌓여 있는데 "집을 일이 없다" 로 조용히 끝난다.
        raise RuntimeError(f"moai 저장소(.moai 가 있는 디렉터리) 안에서 돌린다: {out.strip()}")
    return queue


def claim(item):
    """본 칸을 함께 주고 집는다. 집었으면 `moved`, 못 집었으면 그 까닭을 내고, 실패면 던진다.

    못 집은 까닭은 셋이다. `stale` — 옆에서 먼저 집었다. `already` — 첫 칸이 곧 집는 칸인
    설정이라 `--from` 이 맞고도 아무것도 안 옮겼다(0 으로 끝난다 — 안 보면 둘이 같은 일을 한다).
    `missing` — 뿌리에 없는 줄이다(`--worktree` 가 옆 워크트리에서만 가져왔다).

    **0 아닌 코드를 전부 "졌다" 로 읽지 않는다.** 신원 없음·락 걸림·칸 오타도 같은 코드로 온다.
    못 집은 것은 stdout 에 그 까닭을 담은 줄을 내고, 실패는 stdout 이 빈다.
    """
    code, out, err = moai("mv", item["id"], "in_progress", "--json", "--from", item["status"])
    if code != 0 and not out.strip():
        raise RuntimeError(err.strip() or f"{item['id']} 를 못 집었다")
    said = json.loads(out)
    # id 를 하나만 주므로 넷 가운데 하나만 선다.
    for why in ("moved", "stale", "missing", "already"):
        if said.get(why):
            return why
    raise RuntimeError(f"{item['id']} 를 집은 결과를 못 읽었다: {out.strip()}")


def worktree(item_id):
    """그 일의 워크트리를 `BASE` 에서 띄운다. **앞서 다룬 일이면 그 자리를 잇는다** — 워크트리가
    남아 있으면 그것을 쓰고, 워크트리는 치웠는데 가지(`agent/<id>`)가 남아 있으면 그 가지로 다시
    띄운다. 그 가지를 `BASE` 에서 새로 뜨려 하면 git 이 "이미 있다" 로 거절해, 도로 집은 일이
    매번 여기서 in_progress 에 박힌다."""
    path = os.path.join(TREES, item_id)
    if os.path.isdir(path):
        return path
    branch = f"agent/{item_id}"
    with TREE_LOCK:
        kept = git("rev-parse", "--verify", "-q", f"refs/heads/{branch}").returncode == 0
        p = git("worktree", "add", "-q", *([path, branch] if kept else ["-b", branch, path, BASE]))
    if p.returncode != 0:
        raise RuntimeError(f"{item_id} 의 워크트리를 못 띄웠다: {p.stderr.strip()}")
    return path


def work(item, path):
    """일을 돌리고 (종료 코드, 출력) 을 낸다.

    출력은 파이프가 아니라 **파일로** 받는다. 파이프로 받으면 일이 띄워 두고 간 자식(dev 서버·워처)이
    그것을 쥐고 있는 동안 끝낸 일이 노트도 없이 멈춘다. **stdin 은 주지 않는다** — 일꾼 여럿의 일이
    한 터미널을 나눠 읽거나, 읽을 것을 기다리며 멈춘다.
    """
    # cwd 가 워크트리라 맨 `moai` 는 워크트리의 트래커를 고친다 — 뿌리와 moai 를 넘긴다.
    env = dict(os.environ, AGENT_ROOT=ROOT, MOAI=MOAI)
    with tempfile.TemporaryFile() as log:
        try:
            code = subprocess.run(
                [WORK, item["id"], item["title"]],
                cwd=path,
                env=env,
                stdin=subprocess.DEVNULL,
                stdout=log,
                stderr=subprocess.STDOUT,
            ).returncode
        except OSError as e:
            # 못 띄운 것도 일의 실패로 돌려준다 — 던지면 `worker` 가 멈추기는 해도 노트에 까닭이 안 선다.
            return 126, f"{WORK} 를 못 띄웠다: {e}"
        log.seek(0)
        # `note` 는 UTF-8 이 아닌 입력을 통째로 거절한다 — 깨진 바이트는 U+FFFD 로 바꾼다.
        out = log.read().decode("utf-8", errors="replace")
    # 빈 것은 유니코드 공백으로 가른다(`note` 가 빈 메모로 거절하는 것을 다 덮는다) — 그대로 넘기면
    # 빈 메모로 거절당해 끝낸 일이 in_progress 로 남는다.
    return code, out if out.strip() else "(출력 없음)"


def close(name, item_id):
    """일이 스스로 옮겼거나(review·done) 미뤘으면 덮지 않는다. **묶음을 미룬 것도 센다** —
    `deferred_at` 은 제 줄에 적힌 것뿐이라, 계획 밖인지는 `shelved_by` 가 말한다.

    못 닫아도 멈추지 않는다(bash 판의 `|| true`). 다만 **까닭은 말한다** — moai 의 stderr 를
    받아 두는 자리라, 말하지 않으면 in_progress 로 남은 까닭이 아무 데도 없다."""
    code, out, err = moai("show", item_id, "--json")
    if code != 0:
        say(name, f"{item_id}  칸을 못 읽어 닫지 않았다: {err.strip()}", err=True)
        return
    shown = json.loads(out)
    if shown.get("status") == "in_progress" and shown.get("shelved_by") is None:
        # 닫을 때도 본 칸을 준다 — `show` 와 이 줄 사이는 집을 때와 같은 틈이다. 진 것은
        # 실패가 아니라 남의 일이다.
        code, _, err = moai("mv", item_id, "done", "--from", "in_progress")
        if code != 0:
            say(name, f"{item_id}  닫지 않았다: {err.strip()}", err=True)


def worker(name, failed):
    """일꾼 하나. **어디서 멈추든 실패로 센다** — 스레드에서 새어 나간 예외는 그 스레드만 죽이고
    `failed` 를 안 세워, 집은 일이 in_progress 에 박힌 채 0 으로 끝난다."""
    try:
        shift(name)
    except Exception as e:
        say(name, str(e) if isinstance(e, (RuntimeError, ValueError)) else f"{type(e).__name__}: {e}", err=True)
        failed.set()


def shift(name):
    """큐가 빌 때까지 집고·일하고·닫는다. 멈출 일이면 던진다 — 받는 것은 `worker` 하나다."""
    # 한 일꾼은 한 줄을 한 번만 다룬다. 같은 줄이 또 오면 집기가 안 먹거나(첫 칸이 in_progress 인
    # 설정) 일이 그 줄을 첫 칸으로 되돌린 것이다 — 그대로 두면 같은 줄을 끝없이 돈다.
    seen = set()
    # 뿌리에 없는 줄. 다시 나와도 "또 나왔다" 가 아니다 — 여기서는 원래 못 집는다.
    away = set()
    while True:
        # 옆 일꾼이 집은 줄은 **읽기 전에** 뜬다. 읽은 뒤에 뜨면 읽기와 옆의 집기가 엇갈린 줄(여기
        # 읽기에는 아직 todo 인데 그새 옆이 집었다)이 "또 나왔다" 로 잘못 잡힌다.
        with WORKED_LOCK:
            taken = set(WORKED)
        queue = ready()
        fresh = [i for i in queue["ready"] if i["id"] not in seen and i["id"] not in taken]
        if not fresh:
            back = [i for i in queue["ready"] if i["id"] not in away]
            if back:
                raise RuntimeError(f"{back[0]['id']} 가 또 집을 일로 나왔다 — 사람이 볼 차례다")
            # 남은 것이 옆 워크트리에만 있는 줄뿐이면 그 수도 댄다 — 안 대면 `ready` 에 줄이 서 있는데
            # "없다" 고 한다.
            away_n = f" · 옆 워크트리에만 있는 줄 {len(queue['ready'])}건" if queue["ready"] else ""
            say(name, f"집을 일이 없다 (미뤄 둔 것·빈 묶음에 막힌 일 {len(queue['held'])}건{away_n})")
            return
        # **줄 차례대로 겨룬다.** 맨 위 하나만 노리면 일꾼 셋이 같은 줄에 몰려 둘이 매 판 진다 —
        # 진 일꾼은 곧장 다음 줄로 간다.
        mine = None
        for item in fresh:
            # **겨루기 전에 적는다.** 이긴 것만 적으면 `already` 처럼 매번 "진" 줄이 다음 판에 또
            # fresh 로 나와 끝없이 돈다. 옆 일꾼에게 진 줄은 이미 in_progress 라 `ready` 에 다시
            # 안 나온다.
            seen.add(item["id"])
            with WORKED_LOCK:
                if item["id"] in WORKED:
                    # 읽은 뒤에 옆이 집어 이미 다룬 줄이다 — 되돌려졌어도 다시 하지 않는다.
                    continue
            why = claim(item)
            if why == "moved":
                with WORKED_LOCK:
                    WORKED.add(item["id"])
                mine = item
                break
            if why == "missing":
                away.add(item["id"])
                where = item.get("branch") or "옆 워크트리"
                say(name, f"{item['id']}  건너뛴다 — 뿌리에 없고 {where} 에만 있는 줄이다")
            elif why == "already":
                say(name, f"{item['id']}  이미 in_progress 다 — 집기가 안 먹는 칸이다")
            else:
                say(name, f"{item['id']}  졌다 — 옆에서 먼저 집었다")
        if mine is None:
            continue
        try:
            path = worktree(mine["id"])
        except RuntimeError as e:
            # 집은 채로 둔다 — 워크트리가 왜 안 섰는지는 사람이 볼 일이다.
            note(name, mine["id"], f"실패: {e}\n")
            raise
        say(name, f"{mine['id']}  집었다  {mine['title']}  ({os.path.relpath(path, ROOT)})")
        code, out = work(mine, path)
        if code != 0:
            note(name, mine["id"], f"실패({code}): {out}")
            raise RuntimeError(f"{mine['id']}  실패({code}) — in_progress 로 두고 이 일꾼은 멈춘다")
        if not note(name, mine["id"], out):
            # 노트를 못 남겼으면 닫지 않는다 — 닫으면 일의 출력이 사라진 채 done 이 된다.
            raise RuntimeError(f"{mine['id']}  노트를 못 남겼다 — in_progress 로 두고 멈춘다")
        close(name, mine["id"])


def note(name, item_id, text):
    """노트를 남긴다. 못 남기면 moai 가 댄 까닭을 말하고 False — 멈출지는 부르는 쪽이 정한다."""
    code, _, err = moai("note", item_id, "-b", "-", stdin=text)
    if code != 0:
        say(name, f"{item_id}  노트를 못 남겼다: {err.strip()}", err=True)
    return code == 0


def main():
    global WORK, MOAI
    parser = argparse.ArgumentParser(description="moai 큐를 일꾼 여럿이 겨뤄 푼다")
    parser.add_argument("--agents", type=int, default=2, help="일꾼 수 (기본 2)")
    args = parser.parse_args()
    # **채비는 저장소를 건드리기 전에 다 잰다.** 여기서 걸리면 아무것도 안 집는다(종료 코드 2).
    # 집은 뒤에 드러나면 맨 위 일이 in_progress 에 박히고, 다시 부를 때마다 그다음 일이 또 박힌다.
    if args.agents < 1:
        print("--agents 는 1 이상이다", file=sys.stderr)
        return 2
    if not WORK:
        print("AGENT_WORK 에 일할 명령을 준다 (id 와 제목을 받는 실행 파일)", file=sys.stderr)
        return 2
    for need in ("git", WORK, MOAI):
        if shutil.which(need) is None:
            print(f"실행할 수 없다: {need}", file=sys.stderr)
            return 2
    # 일은 워크트리를 cwd 로 돈다 — `./do-one.sh` 같은 상대 경로를 그대로 넘기면 워크트리 안에서
    # 찾아, 뿌리에만 있는 스크립트는 못 찾고 커밋된 것이면 BASE 의 옛 판을 돌린다. 뿌리 기준으로 박는다.
    # 일에게 넘기는 `MOAI` 도 같은 까닭으로 박는다.
    WORK = os.path.abspath(shutil.which(WORK))
    MOAI = os.path.abspath(shutil.which(MOAI))
    if git("rev-parse", "--verify", "-q", f"{BASE}^{{commit}}").returncode != 0:
        print(f"워크트리를 띄울 가지가 없다: {BASE} (AGENT_BASE 로 준다)", file=sys.stderr)
        return 2
    try:
        ready()
    except (RuntimeError, ValueError) as e:
        print(str(e), file=sys.stderr)
        return 2

    failed = threading.Event()
    crew = [threading.Thread(target=worker, args=(f"a{n + 1}", failed)) for n in range(args.agents)]
    for t in crew:
        t.start()
    for t in crew:
        t.join()
    # 실패는 언제나 1 이다 — 뒤에서 종료 코드로 가를 수 있어야 한다.
    return 1 if failed.is_set() else 0


if __name__ == "__main__":
    sys.exit(main())
