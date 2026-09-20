---
name: moai-supervise
description: Use when handing the ideas piled up on one repository, one at a time, to the Claude sessions idling on it and taking their reports. Triggers on "supervise", "hand out the ideas", "put the idle sessions to work".
---

# moai-supervise — hand ideas out to the sessions that are idling

The supervisor **picks, sends and checks.** It does not fix code, it does not merge,
and it does not settle design in a worker's place. The workers merge. Overlapping
merges are prevented by splitting the files when the supervisor sends (1), and where
they still collide the worker goes back into its worktree and resolves them.

**Work you send out is always done in a worktree** — even if the repository has no
worktree convention. Several workers share one root checkout, so fixing things in the
root mixes their edits and commits together. Worktrees stand in
`<root>/.claude/worktrees/`, and `moai init` writes that path into the gitignore.

**Read the base branch once, at the start of the round.** The place a worker branches
its worktree from and merges back into is the root checkout, so that checkout's current
branch is the base branch — the remote's default branch may differ from the root and may
be stale. The root checkout is the first entry of `git worktree list`, so the line below
gives the root's branch no matter where in the repository you call it, inside a worktree
included. If nothing comes out, report the error git gave and stop.

```sh
if w=$(git worktree list --porcelain); then b=$(printf '%s\n' "$w" | sed -n '1,/^$/s|^branch refs/heads/||p'); if [ -n "$b" ]; then echo "$b"; else echo "the root is detached" >&2; fi; fi
```

**If the root is detached, do not send.** The worker's pick-up commit and its merge
land on a HEAD with no branch, the check (`merge-base <base branch>`) and `worktree add`
fail, and `branch -d` deletes that work's only reference. Do not read the remote's
default branch instead — the root does not stand on that branch, so it is the same
accident. Ask the person to put the root on a branch, and stop.

Fill the name you read into `<base branch>` in the commands below and in the text you
send the worker. **The worker does not read it again** — read inside a worktree, it
gives that worktree's own branch.

## One round

**0. Reclaim first — work that lost its place.** When a session dies the row it picked
up stays `in_progress` and nobody carries it on. Look at this before picking new ideas.

    moai status --json                     the ids of warnings whose kind is "stranded"
                                           (inside a worktree it stands only with `--worktree`)
    moai show <id>                         the `Place` line — one of the four words below
                                           (inside a worktree this too needs `--worktree`)

`stranded` is a row that was picked up while no live worktree holds that work — either
the worktree is gone, or **the work was being done in the root with no worktree**. This
row alone does not tell the two apart: if the session list in 2 shows a session living
in the root it may be that one, so ask that session what it is holding before handing
the work on.

The `Place` line (`place` under `--json`) has four values. **Only `none` is handed on.**

    <path> (<branch>)  at        it runs there. Go in and carry on
    not visible yet    fresh     just picked up — the gap while the worker raises its worktree. Leave it
    unknown            unknown   **a sibling worktree could not be read.** It may be there, so do not hand it on
    none               lost      it lost its place — only this one is reclaimed

**`stranded` being quiet does not mean there is nothing to reclaim.** If a sibling
snapshot that names no picked-up row cannot be read, the place verdict folds into
`unknown` for everything and this warning is locked for the whole repository. When the
`unreadable_worktrees` key stands under `status --json`, **fix that worktree and look
again** — an empty list read before that is not "none", it is "not counted".

**The `N problems in sibling worktrees` on the person's screen is a different number.**
That one counts **every** worktree it could not read among the snapshots it opened, and
counts the other problems met while overlaying too (a snapshot with unparseable rows,
a worktree list that could not be read) — a worktree that could not be read but whose
name points at a picked-up row does not hide the verdict, because that row already
stands in its own place, so while only such rows stand you can trust `stranded` as it
is. It says there is something to fix, not that it could not count — whether it could
count is answered by `unreadable_worktrees` alone. A worktree with a broken snapshot is
named to machines too, by `broken_worktrees` under `status --json` — look at that key
when you are hunting for the worktree to fix. But it is **among the snapshots opened**:
if the names point at every picked-up row, not one sibling snapshot is opened and the
key does not stand even though one is broken. No key does not mean "nothing is broken".

A row picked up less than an hour ago does not show (that is the gap while a worker
raises its worktree). **A worktree that is still there while the session working in it
died does not show under `stranded`** — it is the worktree in `git worktree list` that
the script in 2 does not print as a `worktree` row.

- When there is such work, hand carrying it on to one idle session **before any new
  idea**. Send the text below in place of the text in 3, and after it append the text
  of 3 whole, **from 4-1 to the end** — leave 4-1 out and the worker taking over edits
  the tracker inside the worktree; cut the end off and the step after closing (clearing
  the window) is missing. Fill the placeholders as 3 says to (`<other work>` too — 4-3
  points at that line)

      Supervisor session (<my name>) is handing you the stalled work in <epic> — the previous session did not finish it.
      Read first: moai show <epic> (history and notes) · moai show <member> (the place too — a place stands on work only)
      Model: <model> (<difficulty> — <why>) — a suggestion picked by difficulty. The model is
         changed with `/model`, and only a person types that — if this window is not on that model,
         ask the person watching the window to match it, and if it reads harder than it looked,
         raise it the same way to the model that pairs with the difficulty you just measured — not
         one step at a time (haiku → sonnet → opus). Handed `low` but it is `high`, the model is `opus`.
         The grade of the epic-end review (7) is measured on this same rubric, member by member
      Work running alongside: <other work> — do not touch those files (4-3)
      Base branch: <base branch> — the supervisor read it in the root and filled it in; do not read it again.
      Before you commit or merge in the root, **only check** that the root still stands on that
      branch — if `git -C <root> symbolic-ref -q HEAD` is not `refs/heads/<base branch>` (detached, or
      someone switched the branch), do not run it: tell the supervisor and stop. A merge that lands on
      the wrong HEAD leaves no reference at all once `branch -d` runs
      Root: <root> — the `root` from 2. The tracker you edit is always the one there (4-1 of the text in 3)
      - If the worktree is there, go in with EnterWorktree(path), read how far it got with
        `git log <base branch>..HEAD` and `git status`, and carry on
      - If it is not, raise it again from the root. If the branch survives, on that branch
        (`git worktree add .claude/worktrees/<epic> worktree-<epic>`); if it does not,
        `git worktree add -b worktree-<epic> .claude/worktrees/<epic> <base branch>`
      - **If the root is not the top of the repository** (a subdirectory project in a
        monorepo), go into the worktree and then move to the same subdirectory inside it and
        work there — standing at the top, `moai` finds and writes the root's `.moai`, and the
        hook does not count edits under `.claude/`
          cd "$(git -C <root> rev-parse --show-prefix)"
      - The member's column is already picked up — do not pick it up again
      - The note in 9-1 records this window's share only. Append `reclaimed work, the previous
        session's share is unknown` to the end of the reason — the previous session's model and
        tokens are written nowhere, and without it the whole member reads as this window's work
      - The `2` the steps below point at is **the tracker commit with a path** — the root is
        shared by every session, so run `git commit -m "…" -- .moai/`. If a merge is open
        (MERGE_HEAD) git refuses it, so wait for that merge to finish and run it again

- **Whether it is carried on or put down is not the supervisor's call.** If it looks like
  work to put down (`moai mv <id> todo`, `moai defer <id> -m '<why>'`), ask the person
- Work handed on to be carried is, like an idea, not sent again until its report is checked

**1. Pick.** Out of the ideas that have piled up, keep only the ones that do not collide
with what is open right now.

    moai idea ls                           what has piled up
    moai show -s in_progress,review        what is picked up
    moai show <id>                         what that idea touches

Look at `git worktree list` too. An idea that touches the **same files, the same area**
as a worktree already standing or an epic already picked up comes out of this round —
when two of them change the same place, one waits for the other at the merge. **Compare
the ideas you send in this same round against each other too** — a worker only raises
its worktree after it receives the work, so what you just sent is not in the lists above
yet. Do this count again for every further idea. An epic left open with only
first-column members (what brief 7-1 left behind) shows as `in_progress` but is not
picked up — there is no worktree and no picked-up member, so do not drop ideas over it.

**If a milestone is running, what is inside it comes first.** The header of `moai ready`
says what is running and how many it held back outside it, and the `moai status` notice
shines on the same thing. Then what you send this round is work attached to that
milestone — an idea from outside waits for the next round unless it should stand as `p0`.
**The tool does not block this** (a pick-up goes straight through), which is why the
place to decide is here. If two milestones are running, both are inside.

**An idea you sent comes out of the candidates until its report is checked.** Until the
worker unfolds it, it stays in `moai idea ls`, and the same idea goes to a second worker.

**2. Find a worker.** `ListAgents` does not show a session's place (cwd). Read
`~/.claude/sessions/*.json`, which Claude Code writes per session (under
`CLAUDE_CONFIG_DIR` if you moved it). For a subdirectory project in a monorepo, the
subdirectory that has `.moai` is the root. The script prints that `<root>` on the first
line, `root`, and if the root is not the top of the repository it prints the path from
the top down to the root on a `subdir` line — a worktree stands for the whole
repository, so the worker has to go into the same subdirectory inside it (brief 3).

```sh
python3 - "$(git worktree list --porcelain | sed -n 's/^worktree //p' | head -1)" "$(git rev-parse --show-toplevel)" <<'PY'
import glob, json, os, subprocess, sys
if not sys.argv[1]:
    sys.exit("call this inside a git repository")
top, here = os.path.realpath(sys.argv[2]), os.path.realpath(os.getcwd())
while here not in (top, os.path.dirname(here)) and not os.path.isdir(os.path.join(here, ".moai")):
    here = os.path.dirname(here)
root = os.path.realpath(os.path.join(sys.argv[1], os.path.relpath(here, top)))
trees = os.path.join(root, ".claude", "worktrees") + os.sep
print("root  ", root)
if os.path.relpath(here, top) != ".":
    print("subdir", os.path.relpath(here, top))
home = os.environ.get("CLAUDE_CONFIG_DIR") or os.path.expanduser("~/.claude")
def parents(pid):
    while pid > 1:
        yield pid
        try:
            out = subprocess.run(["ps", "-o", "ppid=", "-p", str(pid)], capture_output=True, text=True).stdout.strip()
        except OSError:
            return
        pid = int(out) if out.isdigit() else 0
def reachable():
    """Can this supervisor reach the tmux server. If not, it cannot ask about a single pane."""
    try:
        r = subprocess.run(["tmux", "display-message", "-p", '#{pid}'], capture_output=True, text=True)
    except OSError:
        return False
    return r.returncode == 0 and r.stdout.strip().isdigit()
tmux_up = reachable()
def detached(s):
    """Was the tmux pane the session file names not spawned by that session on this server —
    that is, a pane on a separate test server.

    **None when it cannot be asked** — that is unknown, not detached. When the supervisor runs
    outside tmux or a sandbox blocks the socket, every question comes back empty, and reading
    that as detached filtered out every session in the root so nobody got any work (with no
    line saying why).
    """
    pane = str(s.get("tmux") or "").rpartition(".")[2]
    if not pane.startswith("%"):
        return False
    if not tmux_up:
        return None
    try:
        r = subprocess.run(["tmux", "display-message", "-p", "-t", pane, '#{pane_pid}'], capture_output=True, text=True)
    except OSError:
        return None
    owner = r.stdout.strip()
    # Reaching the server but not finding the pane means it is not this server's pane — that is detached.
    if r.returncode != 0 or not owner.isdigit():
        return True
    return int(owner) not in parents(int(s["pid"]))
if not tmux_up:
    print("cannot reach tmux — detached panes cannot be told apart. A test claude may be mixed into `root` below")
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
    elif cwd == root and detached(s):
        print("detached", s.get("status"), s.get("name"))
    elif cwd == root:
        print("root    ", s.get("status"), s.get("name"))
    elif cwd.startswith(trees):
        print("worktree", s.get("status"), s.get("name"), cwd)
if unread:
    print("unreadable files", unread)
PY
```

- **Hand work only to a session whose place is the root and whose status is `idle` or
  `waiting`.** `busy` is working, and the other values (`shell` and such) mean something
  you do not know, so do not hand work to them
- **Leave out a session whose sent idea has not had its report checked.** A worker is in
  the root while it unfolds, picks up and merges — it shows `waiting` when it is waiting
  on a person's answer or a permission, and `idle` when it finishes a turn
- **Do not hand work to a `detached` pane.** The tmux pane the session file names was not
  spawned by that session on this server — it is a test `claude` raised on a separate
  tmux server (`-L`). Even with the root as its place, it is not a worker
- **If tmux cannot be reached at all, do not filter.** When the supervisor runs outside
  tmux or a sandbox blocks the socket, not one pane can be asked about — read that as
  detached and every session in the root drops out and nobody gets any work. The script
  prints one line, `cannot reach tmux`, and leaves them in, so take it that a test
  `claude` may be mixed into `root` and look at the names once more with `ListAgents`
  before you send
- A session whose place is `<root>/.claude/worktrees/*` is **working** in this
  repository. Watch it, but do not hand it work
- **Do not touch sessions in other directories**
- **A session that refused the work comes out of the candidates and is not sent to
  again.** Some sessions take work only from their own person — once one has refused,
  do not even leave a notification on it after that
- The file survives the session, and if another process reuses that pid it reads as
  alive. Before you send, check once that the name also shows in `ListAgents`
- This file is an internal one, not in the documentation, so its fields may differ from
  release to release. If the script prints `unreadable files` or does not run, look at
  the names with `ListAgents` and tell them apart by asking — **only sessions on this
  machine** — for `pwd` and what they are doing; a remote or cloud session is a
  different checkout even when it names the same path

**2-1. Pick a model — by one word of difficulty.** The grade of the epic-end review
(brief 7) is measured on this same rubric, member by member. Keep two axes and the brief
carries two sets of judgement, and on the day they differ the cheap model takes the
write path.

| Level | How it is measured | Model |
|---|---|---|
| `low` | text, comments, a one-line fix; behaviour unchanged | `haiku` |
| `medium` | a behaviour change inside one file, ringed by tests | `sonnet` |
| `high` | several files, the write path, concurrency, the storage format, hooks; hard to undo | `opus` |

The epic-end review is that epic's only review, because the members are not reviewed
separately (brief 5 and 7 carry that to the worker).

**The grade is one step above the heaviest member's difficulty** — `medium` if the members
are all `low`, `high` if one is `medium`, `xhigh` if one is `high`.
**If any member touched the write path, concurrency, the storage format or hooks**, it is `max`.
Raise it one more step if the epic crosses surfaces or carries several design decisions. If you hesitate, raise it.
The model follows that grade — `medium` means `sonnet`, `high` and up means `opus`.

The supervisor picks before reading any code, so this is a suggestion; the last word
belongs to the worker who read the issue. A running session's model cannot be changed by
`SendMessage` and cannot be changed by config — the person in that window changes it
with `/model`.

**3. Send.** Send **one** idea to one idle session with `SendMessage`. The worker knows
nothing of this conversation, so send the text below **whole** — it is all the worker
receives, so everything the worker has to keep is inside it.
Fill in `<my name>`, `<id>`, `<title>`, `<base branch>`, `<model>`, `<difficulty>`, `<why>`, `<other work>` and `<root>`.
`<root>` is the `root` from 2. **Leave it unfilled** and the worker, inside its worktree,
reads its own place as the root.
`<model>`, `<difficulty>` and `<why>` are the pair you picked in 2-1 and your reason.
**Leave them unfilled** and those placeholders travel as they are, so the note the worker
leaves when it closes says `<model>` instead of what actually did the work.
Do not use a single quote inside `<why>` — it closes the single quote in 9-1 and the rest
of the text leaks into the shell.
`<other work>` is the sibling worktrees you measured in 1, the work you send in this same
round, and the files that work holds — `none` if there is none. What the supervisor
measured before sending cannot cover a file that turns out to be needed mid-epic, so when
the worker meets such a file it does not fix it: it leaves it as a member and reports it
(brief 4-3).
Do not fill `<grade>` — that is the review grade the worker picks in 7, after developing.
Do not fill `<vendor>` or `<count>` either — those are the vendor and the token count the
worker reads in its own window in 9-1.

    Supervisor session (<my name>) is handing you idea <id> — <title>.
    Read first: moai show <id>
    Model: <model> (<difficulty> — <why>) — a suggestion picked by difficulty. The model is
       changed with `/model`, and only a person types that — if this window is not on that model,
       ask the person watching the window to match it, and if it reads harder than it looked,
       raise it the same way to the model that pairs with the difficulty you just measured — not
       one step at a time (haiku → sonnet → opus). Handed `low` but it is `high`, the model is `opus`.
       The grade of the epic-end review (7) is measured on this same rubric, member by member
    Work running alongside: <other work> — do not touch those files (4-3)
    Base branch: <base branch> — the branch name below. The supervisor read it in the root and filled it in; do not read it again.
    Before you commit or merge in the root, **only check** that the root still stands on that
    branch — if `git -C <root> symbolic-ref -q HEAD` is not `refs/heads/<base branch>` (detached, or
    someone switched the branch), do not run it: tell the supervisor and stop. A merge that lands on
    the wrong HEAD leaves no reference at all once `branch -d` runs
    1. Unfold it in the root — the one way to turn an idea into work is
       `moai idea promote <id> --from -`. Unfold into an epic plus issues even for a single
       issue. Look at `--dry-run` first — that is for this window to see, not to show a person
       and ask. Showing a split plan to a person once is a step of work a person asked for
       directly; what a supervisor hands you is work a person already passed on. Design
       decisions are asked in 4.
       If that idea is already done (someone unfolded it), do not unfold: tell the supervisor —
       unfolding again puts up two epics. **Write a short new title** — a line in the plan
       becomes the issue title verbatim, so copying over an idea title that grew long while it
       was parked spreads that length into the issues. The original text stays on that idea and
       the history leads back to it
    2. Pick the members up with `moai mv <member> in_progress --from todo` and commit in the root.
       **Pass the column you saw** — this is a place where several sessions share one `.moai`,
       and overwriting a row picked up beside you means two of you do the same work. A non-zero
       code means it is taken, so leave that member and tell the supervisor. The root is shared
       by every session — a commit made while someone has a merge open (MERGE_HEAD) seals that
       merge with its own subject. So give the tracker commit a path. With a merge open git
       refuses it, so wait for that merge to finish and run it again
         git commit -m "chore(tracker): pick <epic> up in a worktree" -- .moai/
    3. Right after the commit in 2, branch from the local <base branch> with
       `git worktree add -b worktree-<epic> .claude/worktrees/<epic> <base branch>` and go in with
       EnterWorktree(path). The name is the unfolded epic's id, not the idea's. Until the worktree
       stands, the other sessions in the root read this member as their own focus.
       **If the root is not the top of the repository** (a subdirectory project in a monorepo) the
       worktree stands for the whole repository, so once inside, move to the same subdirectory in
       it and work there — standing at the worktree top, `moai` walks up and finds the root's
       `.moai` to write, and the hook does not count edits under `.claude/`
         cd "$(git -C <root> rev-parse --show-prefix)"
    4. Do not guess a design decision that is not in the notes — ask with AskUserQuestion; a
       person is watching the worker's window
    4-1. **The tracker you edit is always the root's.** `<root>` is the root checkout's place,
       filled in by the supervisor — do not guess it from inside the worktree. **The tool moves
       that by itself** — even a bare `moai` typed inside the worktree reads and writes the
       root's tracker, and when it writes, one line says where. `moai -C <root> <command>` lands
       in the same place, so writing it that way is fine too. Short of a branch with no tracker
       in the root, the only thing that stops the move is `MOAI_HERE`, so **do not turn it on** —
       turn it on and that worktree's `.moai` changes, and the snapshots conflict on the merge
       (and merging them overwrites someone else's rows).
       Put `-e <epic>` on an idea you park mid-epic — it does not keep the epic open, and 7-1
       reclaims it through that even if the window is cleared or the work is taken over.
       **Give a review subagent the same words.** If that worktree's `.moai` changed anyway,
       undo it with `git checkout -- .moai`, and if the row was already committed, undo that
       commit too and park it again from the root
    4-2. **If you test tmux, do it on a separate server only** — `env -u TMUX tmux -L <unique name>`
       on every call. Put the epic id in the name so it cannot collide with the test servers of
       the workers beside you or of a review subagent. `-S <socket>` works too, but a socket path
       does not stand past the unix limit (about 100 bytes), and a scratchpad path is usually too
       long. Never use `kill-server` or `kill-session` without `-L`/`-S`: inside tmux a bare
       `tmux` goes to the person's default server and kills every session, and `TMUX_TMPDIR` does
       not fence it in. A script that calls `tmux` inside itself cannot be given `-L` by hand, so
       run it with a wrapper at the front of `PATH` that calls the real `tmux` by absolute path
       and inserts `-L`. Do not send keys into a pane someone else raised. If you raise a test
       `claude` on that server, keep its cwd outside the root (the scratchpad) — a session raised
       in the root slips into the supervisor's session list as an idle worker.
       **Give a review subagent these words too** — it was a review subagent that killed a whole server
    4-3. **If you would have to touch a file that work running alongside holds, do not fix it** —
       the files named by `Work running alongside` in the header, or files a sibling branch in
       `git worktree list` already changed
       (`git diff --name-only <base branch>...<sibling branch>`). When two of them change the
       same place, one waits for the other at the merge. If this epic cannot deliver what it
       promised without that, it is not an idea but a member — create it with
       `moai -C <root> add '<what>' -e <epic>`, leave it in the first column, and name it in 11
       as **a member left because the work beside it holds the file**, together with that other
       work. The supervisor sends it once that work is done. Do not defer it
    4-4. **Polish Korean text going into moai with the Korean writing plugins** — every title,
       body, note, `-m` and review-text note that carries Hangul. Polish with
       `korean-skills:humanizer`, and when it runs past 20 lines put it through
       `humanize-korean:humanize-korean` outside the repository (the scratchpad), delete that
       `_workspace/`, come back into the worktree (standing outside it the hook cannot find the
       tracker and no rule stands), and finish with `korean-skills:grammar-checker` for spelling.
       Leave ids, commands, paths and numbers as they are, and do not polish the model line in
       9-1, the `Next:` line in 12, a `Regression-of:` line, or the `Summary:` first line of a
       shortened review text. **Give a review subagent these words too**
    5. **Do not review member by member.** When one member is finished, run the tests, commit and
       move to the next — the review looks at the whole epic once, in 7, after every member is
       finished. One review is expensive; do not call it as many times as there are members. The
       cost of member 2 piling onto a bug in member 1 is paid in that one review.
       `low`·`medium`·`high` is the rubric the model in the header was picked on, and the same rubric measures
       the members when you pick the grade in 7.
       `low` — text, comments, a one-line fix; behaviour unchanged
       `medium` — a behaviour change inside one file, ringed by tests
       `high` — several files, the write path, concurrency, the storage format, hooks; hard to undo
    6. When the members' work is all done, pull <base branch> into the worktree, resolve the
       conflicts and run the tests. Fix things here — while the worktree stands, rule 2 blocks
       edits in the root
    7. Before merging, review the whole epic with `/code-review <grade> --fix` — the members were
       not reviewed separately, so this once is the only review.
       **The grade is one step above the heaviest member's difficulty** — `medium` if the members
       are all `low`, `high` if one is `medium`, `xhigh` if one is `high`.
       **If any member touched the write path, concurrency, the storage format or hooks**, it is `max`.
       Raise it one more step if the epic crosses surfaces or carries several design decisions. If you hesitate, raise it.
       The model follows that grade — `medium` means `sonnet`, `high` and up means `opus`.
       If the window is not on that model, ask the person watching it for `/model <that model>`
       before you call — as in `/model opus` (a review agent inherits the window's model).
       Write the grade you picked and why in one line in the angle (`-b`). The diff runs from
       where the branch left <base branch> (`git merge-base <base branch> HEAD`). You pulled
       <base branch> in 6, so the conflict resolution is inside it too. Create the review issue
       (rule 3)
         moai add 'review — <what you are looking at>' -t review --parent <epic> -b '<what you are looking for and why>'
       This line is called from the worktree too, so run it as `moai -C <root>`, per 4-1. What
       you take in goes in a separate fix: commit; what you hand on goes in a note with the issue id.
       Only when the worktree's hook cannot see a review issue created or picked up in the root
       and blocks you — a binary from before the hook moved the tracker to the root reads that
       worktree's snapshot only — run a review subagent with the same angle, grade and `--fix`
       scope. A subagent inherits the window's model, so pass the model for the grade above in
       `Agent`'s `model`. Keep the review issue, the angle (`-b`), the text note and the closing
       `-m` as they are. Any other refusal, such as a missing angle, is not worked around: fix it
       the way the refusal's own command says
    7-1. Before merging, go back over the ideas parked mid-epic
       (`moai -C <root> show --type idea -e <epic>` and what this window remembers) and what the
       review handed on — **can the epic deliver what it promised without them.** If not, it is
       not an idea but an unfinished member. What you sorted as "not for now" while parking has
       these mixed in — the one waiting on a person's decision, the one pushed out because a
       worker beside you held that file. This step sits after 7 so that it sees what 7's review
       handed on too. Unfold such an idea as a member of the epic already standing — write only
       `- issue` lines in the plan; the idea closes by itself and its source stays. You type this
       from the worktree, so pin the root into the line (4-1). **If that idea is already done, do
       not unfold it** — someone unfolded it, or you came back from 8 and are going round again.
       promote unfolds a closed idea too, and the same member stands twice
         moai -C <root> idea promote <idea id> -e <epic> --from -
       Do not do a reclaimed member here: merge with it left in the first column — work that has
       not been through 7's review does not get mixed into the merge, and a member still standing
       keeps the epic open. Do not `defer` that member. Deferring it closes the epic without its
       promise delivered. 7's review did not see that member, so write it in the `Next:` note in
       12 — the window that closes the epic with that member calls the epic-end review again
    7-2. **If the epic ran inside a milestone, sort what you handed on once more, by the release
       bar.** A review makes its findings without regard to the release bar, so fixing all of them
       inside pushes the release out by as many findings as there are, and sending all of them
       outside ships with bugs in. This window is what sorts them — you already decided in 7 what
       to take in and what to hand on, so this is the extension of that. **Bug-level stays
       inside**: create it as a member of that epic (`-e <epic>`) or under an epic in the same
       milestone. **What this release can do without goes outside** — not "not doing it", but
       "not in this release"
         moai -C <root> edit <that row> -e none --milestone none
       **Pass `-e none` with it.** A milestone is inherited from the epic, so on an epic member
       `--milestone none` alone changes nothing and comes back with one line saying the place
       comes from the epic and cannot be cut — a row 7-1 reclaimed stands as that epic's member,
       so take it out of the epic here as well. That the row stops keeping the epic open is the
       point: it is a row decided out of this release.
       **Bug-level is measured with the words that already exist** — does a `#bug` tag fit, and
       can a `Regression-of:` line be written (did something already merged break). Those two are
       inside; the rest is outside. A new axis is not made because it would become a fourth
       vocabulary beside the column, the kind and the defer
    8. Come back to the root with ExitWorktree(keep) — remove it from inside the worktree and the
       session's place stays in a directory that is gone, and the supervisor never sees this
       session in the root again.
       Before merging, check that the root stands on <base branch> — if it does not, do not merge:
       tell the supervisor
         git symbolic-ref -q HEAD                  it has to be refs/heads/<base branch>
       Merge in the root, **in one call**. Overlap with the workers beside you was split when the
       supervisor sent the work, and where it still collides, undo as below and resolve in the
       worktree — do not go looking for the other session to tell it. Do not use `--no-commit`.
       Without `--no-ff` it ends as a fast-forward and no merge commit stands
         git merge --no-ff worktree-<epic> -m "merge: …"
       If the root's `.moai` holds uncommitted rows from another session the merge is refused —
       take them in first with a commit with a path, as in 2. If it stops on a conflict, do not
       resolve it in the root — undo with `git merge --abort`, go back into the worktree with
       EnterWorktree(path) and run again from 6
    9. Once the merge has really landed, remove the worktree and the branch from the root with
       `git worktree remove .claude/worktrees/<epic>` and `git branch -d worktree-<epic>`
    9-1. Before closing, leave one line per member **on what did this work** in this window —
       leaving out the members left in the first column by 7-1 and 4-3, which nobody did. Not the
       suggestion in the header but the model that **actually ran** in this window. The line below
       was filled in by the supervisor as a suggestion, so if you raised it, or the window was on
       a different model from the start, correct the model and the difficulty to the real ones and
       write why in the reason — the next person reads "what was put on work of this size" there.
       It is a note, not a field: the journal is not read to compute state and derived values are
       not stored. The supervisor does not fill `<vendor>` or `<count>` — the vendor is
       `anthropic`, `openai` or `google`, and the model is its real name (`opus-5`), not the
       `/model` alias — the `<model>` the supervisor filled in is an alias (`opus`), so correct it
       to the real name even if you did not change models.
       `<count>` is the tokens this window used. **If you do not know the token count, drop
       `tokens=<count>` whole** — do not write 0 and do not estimate. **One line per id** —
       write the same line on several ids and the tokens multiply by the number of ids. A window's
       tokens cannot be split per member, so write them on **one member only** and leave
       `tokens=<count>` out of the other members' lines.
       Quote free text with single quotes — inside double quotes the shell expands backticks and
       `$(…)` as commands. If the text itself contains a single quote, stream it from stdin with `-b -`
         moai note <member> 'model: <vendor>/<model> tokens=<count> (<difficulty> — <why>)'
    10. Close them after that. **Run `moai mv <member> done` only once that merge has really
       landed** — a worker moved them before the merge and had to undo it. Do not close the
       members left in the first column by 7-1 and 4-3 — those members keep the epic open. While
       the worktree still stands, the hook reads this work as a sibling worktree's and cannot
       refuse a review closed without `-m`. Close the review issue leaving what came out of it
         moai note <review id> -b - < <review text>   the reviewer's own words (summarize past 64KB)
         moai mv <review id> done -m '<what you took in, what you handed on>'
       If the text runs past 64KB, summarize it — put `Summary: original <size>KB agent-<task-id>` on
       the first line, keep every finding's number and place, and shorten only the sentences. Leave fences and indentation alone
       Leave the tests passing in the root with a commit with a path, as in 2
    11. Report with SendMessage to "<my name>" — the merge hash, the unfolded epic's id, a line or
       two of summary, what you handed on and any new ideas, the members reclaimed in 7-1 and left
       in the first column, and the members left in 4-3 because the work beside you held the file,
       with that other work named
    12. Finally, **say when the window can be cleared.** Leave the line to take over from
       (`moai note <epic> 'Next: …'`), take it into the root with a commit with a path as in 2 —
       it is written after the commit in 10, so leaving it out leaves it in the shared root where
       someone else's commit sweeps it up — and tell the person watching that window, in one line,
       that a `/clear` is fine now. The context lives in the tracker, not in the conversation:
       issue bodies, notes, review texts, commit messages. If you can see your own context usage,
       put that number in the line too.
       **Say the opposite in the same line** — not to clear while a review is running in the
       background, while a merge conflict is being resolved, while waiting on a person's answer,
       or after the supervisor's next message has arrived in this window. Clearing then loses what
       is not yet moved into the tracker, or the message that arrived.
       On tmux the supervisor may check the report and type `/clear` into this window itself — the
       supervisor types it once it sees the `Next:` note stand. That is why the note is the last
       step: if anything is left (a background review, say), finish it before the note, and if you
       cannot, send the supervisor one more line saying so before the note (the report already went
       in 11) — the supervisor does not clear such a window

**4. Wait.** On a working session, leave `notify_when_idle: true` with no message.
**Do not sweep `ListAgents` over and over** — the notification comes. It comes once only,
so if it arrives without a report (the worker asked the person something and finished its
turn), leave it again.

If the supervisor is in the root, then in the gap after the worker picks the member up
and before it raises its worktree, the hook holds that member as "still picked up" when
the supervisor's turn ends. **That member is the worker's** — do not move it, do not
defer it, do not put a note on it; just finish the turn.

**5. Check the report and send the next.** Look at three things before you believe a report.

    git merge-base --is-ancestor <merge hash> <base branch> && echo yes   is the merge on the base branch
    moai show <epic>                       are the unfolded epic and its members done
    git worktree list                      is that worktree gone

**A member left because the work beside it holds the file** (brief 4-3) goes to an idle
session after that other work's report is checked. Send the text of 3 but fill `<id>`
with the epic, and in place of 1 write "this epic is already unfolded — do not promote;
pick up the first-column members from 2 on". Until then, count it in 1 as work holding
that file. Like the ones from 7-1, that member is right even when it is not done — it
keeps the epic open, so the epic is not done either.

A member the report says was left in the first column by brief 7-1 is right even when it
is not done — that member keeps the epic open, so the epic is not done either. It is not
an idea and it does not show in 1's list, so pass it to the person along with the reason
it was left (a person's decision, a file held beside it).

If the report holds and **that session has finished its turn** (the report is 11 and the
worker still has 12 to do), you may point out that the window is at a good place to be
cleared — the worker says the same in its own window (brief 12). The three checks look
only at the merge, the closing and the worktree; they do not read the notes. **Once you
have pointed it out, send the next idea only after the person has cleared that window or
said they will not** — a message sent before that disappears with a late `/clear`, and
that idea and that session sit out of the candidates waiting for a report that will never
come.

`<epic>` is the epic id carried in the report. An idea is already done once it is
unfolded and it does not show its members, so `moai show <id>` cannot tell you whether
the work finished — when the report does not carry it, read it from that idea's history
line about being unfolded.

If the three hold — on tmux, clear the window first with 5-1 — send the next idea to that
session. If they do not, ask that session what is left, and do not finish it in its place.

**5-1. On tmux, the supervisor clears the window.** Instead of pointing it out to the
person and waiting, the supervisor types `/clear` into that pane. Call it only after the
three hold, and only after the `Next:` note the worker leaves in 12 stands in the history
of `moai show <epic>` — that note is the last step of 12, so without it the worker is
still moving things into the tracker. Only a note that stands **after the report** counts
— `Next:` is also the hand-over line a session leaves when it could not finish, so an
epic reclaimed in 0 already has the previous session's one. The report (11) comes before
12, so when a report arrives the note is usually not there yet and the worker is `busy` —
do not point it out to the person: leave the notification from 4 and look after it
arrives. Clearing erases the whole conversation that worker holds, so never call it
before the check. `<session>` is the name of the session that sent the report, and
`<root>` is the `root` from 2.

```sh
python3 - '<session>' '<epic>' '<my name>' '<root>' <<'PY'
import glob, json, os, re, subprocess, sys, time
name, epic, me, root = sys.argv[1:5]
home = os.environ.get("CLAUDE_CONFIG_DIR") or os.path.expanduser("~/.claude")
erased = False
def gone(kept, left):
    """**Only the lines erased** from the draft. What is left stands in order as lines of the
    earlier screen (`shrunk`), so those are subtracted and the rest returned. If what is left is
    not a line of the draft (the person typed meanwhile), None — what was erased cannot be told."""
    rest = iter(kept.split("\n"))
    out = []
    for line in left.split("\n"):
        if not line:
            continue
        for k in rest:
            if k == line:
                break
            out.append(k)
        else:
            return None
    out.extend(rest)
    return "\n".join(out)
def skip(why, then="only point out to the person that it can be cleared"):
    print("not clearing —", why, "—", then)
    if erased:
        # **Give back only what was erased.** Stop after erasing one line and the rest is still
        # in the box — give the whole draft back and the person pastes the lines still in their
        # box on top of it, so the same lines stand twice.
        left = draft(pane)
        if left and dim_only(pane):
            left = ""
        back = None if left is None else gone(kept, left)
        if back is None:
            print("erased part of the draft — look at what is left in that box and give the person of that window the `draft` copied above, minus the lines that overlap")
        elif not back.strip("\n"):
            # Only blank lines left means nothing was erased — blank lines in the box are skipped
            # above, so the draft's blank lines flow in here unpaired. Calling that "erased" would
            # tell the supervisor to hand back an empty text.
            print("the draft is still in that box — there is nothing to hand back")
        elif back == kept:
            print("the draft is already erased — give the person of that window the `draft` copied above")
        else:
            print("erased from the draft — hand back only this to the person of that window. The rest is still in that box")
            print(back)
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
            # The first line is the prompt and one space, the following lines two — strip only
            # that much and the indentation survives. Lines without that prefix are not cut: the
            # day the screen draws differently two characters would vanish silently, and the copy
            # is the only one the person has left, so nobody would see it shrink.
            head = (PROMPT + " ", "  ")
            return "\n".join((l[2:] if l[:2] in head else l[1:] if l[:1] == PROMPT else l).rstrip() for l in box).strip("\n")
        box.append(line)
def grey(code):
    """Is this foreground colour a dim grey. Look at both the 256-colour grey ramp and true colour
    (r=g=b) — Claude Code's colours are the theme's hex values, so a pane that takes true colour
    gets `38;2;136;136;136` rather than `38;5;244`. The black end is not grey — a light theme draws
    text the person typed as `rgb(0,0,0)`."""
    n = code.split(";")
    if code == "90":
        return True
    if n[:2] == ["38", "5"] and len(n) == 3 and n[2].isdigit():
        return int(n[2]) == 8 or 238 <= int(n[2]) <= 247
    if n[:2] == ["38", "2"] and len(n) == 5 and all(p.isdigit() for p in n[2:]):
        return len(set(n[2:])) == 1 and 64 <= int(n[2]) < 160
    return False
def sgr(code, was):
    """Fold one SGR piece into (dim attribute, dim foreground, reverse). tmux emits the foreground
    separately (`\x1b[2m\x1b[37m`) and gathers attributes into one piece — drop one attribute and it
    prefixes a reset, `0;2`; set two at once and it is `2;3`. So attribute pieces are read one by one."""
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
    """Is every visible character in the input box dim — that is, Claude Code's suggestion text
    rather than text the person typed."""
    lines = screen(pane, True)
    bare = lambda l: re.sub(SGR, "", l)
    # Open the input box on the **same line** as `draft`. Searching with `in` lets a prompt glyph
    # inside text the person typed drag it further down, so the person's text above is missed. The
    # end of the box is read with `startswith` too — return true on one dash inside a pasted line
    # and `/clear` lands after the person's text.
    at = [i for i, l in enumerate(lines) if bare(l).startswith(PROMPT)]
    if not at:
        return False
    # Fold the colours from the top of the screen — tmux does not re-emit the same colour across a
    # line break, so the second line of a wrapped suggestion arrives with no colour piece. If no dim
    # character was seen at all, it is not true.
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
            # Claude Code draws the cursor of an empty box reversed over the first character of the
            # suggestion (with no dim). If the first character of the prompt line is reversed, take
            # that one cell out as the cursor and count the rest as it is.
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
def shrunk(now, was):
    """Is this text the erasing pared down. One erase only empties a line and pulls the next one up,
    so every line left stands in order as **the same line** of the earlier screen (a blank line is
    the one just emptied). A line that does not match is text the person typed meanwhile — testing
    only whether a line is contained in the earlier screen cannot tell them apart, because one newly
    typed character or a retyped prefix is contained in some earlier line."""
    rest = iter(was.split("\n"))
    return all(not line or line in rest for line in now.split("\n"))
QUIET = '#{pane_in_mode}#{pane_synchronized}'
if not os.environ.get("TMUX"):
    skip("outside tmux")
try:
    tmux("-V")
except OSError:
    skip("no tmux")
f, s = session()
if not s:
    skip("could not find exactly one live session named " + name)
pane = str(s.get("tmux") or "").rpartition(".")[2]
if not pane.startswith("%"):
    skip("no pane in the session file")
if pane == os.environ.get("TMUX_PANE"):
    skip("the supervisor's own window")
if s.get("status") != "idle":
    skip("not idle — " + str(s.get("status")), "leave the notification from 4 and call again after it arrives")
cwd = str(s.get("cwd") or "")
if not cwd or os.path.realpath(cwd) != os.path.realpath(root):
    skip("not in the root — still inside a worktree")
owner = looks('#{pane_pid}')
if not owner.isdigit() or int(owner) not in parents(int(s["pid"])):
    skip("pane " + pane + " does not belong to that session")
if looks(QUIET) != "00":
    skip("the pane is in copy mode (the person is scrolling to read) or synchronized with others")
kept = draft(pane)
if kept is None:
    skip("could not read the input box")
if kept:
    print("draft —", name, pane)
    print(kept)
seen, ghost = kept, None
for _ in range(20):
    left = draft(pane)
    if left == "" and ghost is None:
        break
    if left is None or looks(QUIET) != "00":
        skip("lost the input box while erasing")
    # If text taken for dim changed when erased, it was not suggestion text — suggestions do not
    # erase. It is already copied above.
    if ghost is not None and left != ghost:
        skip("the person is typing", "stopped erasing. Also hand back the `dim text that appeared meanwhile` copied above to the person of that window")
    # Text the person typed while erasing was never copied out — erase more and no copy is left for
    # them (user decision).
    if not shrunk(left, seen):
        if not dim_only(pane):
            print("typed meanwhile —", name, pane)
            print(left)
            skip("the person is typing", "stopped erasing. Only point out to the person that it can be cleared")
        # A box emptied of the person's text gets dim suggestion text again — it is not typed text,
        # so do not stop. Do not tell them apart by colour alone either: copy it out and try
        # erasing, and if it erases it was the person's text drawn dim (`ghost` above).
        print("dim text that appeared meanwhile —", name, pane)
        print(left)
        ghost = left
    seen = left
    erased = True
    tmux("send-keys", "-t", pane, "C-e", "C-u", "DC")
    time.sleep(0.2)
else:
    # Tell them apart by erasing (user decision): if text that survived even the last stroke is all
    # dim, it is not what the person typed but Claude Code's suggestion — that does not land in front
    # of `/clear`, so type it. Do not require it to equal the draft — when a suggestion reappears in
    # a box emptied of the person's text, the draft is already copied out and only the suggestion is
    # left.
    rest = draft(pane)
    if ghost is not None and rest != ghost:
        skip("the person is typing", "stopped erasing. Also hand back the `dim text that appeared meanwhile` copied above to the person of that window")
    if rest and rest == left and dim_only(pane):
        if rest == kept:
            print("the `draft` above was dim suggestion text — the person did not type it")
            erased = False
            # It is not the person's text, so do not say "copied into the supervisor's window" on the
            # status line — a person who reads that goes hunting in the supervisor's window for text
            # they never wrote.
            kept = ""
        elif rest == ghost:
            print("the `dim text that appeared meanwhile` above was suggestion text — the person did not type it")
    elif rest != "":
        # If nothing was erased, that text is still in the box — say "already erased" and the
        # supervisor hands the person a second copy of text that is sitting right there.
        erased = rest != kept
        skip("could not empty the input box")
if (read(f) or {}).get("status") != "idle":
    skip("no longer idle")
if looks(QUIET) != "00":
    skip("the pane went into copy mode meanwhile")
# If the person typed after the last read, `/clear` lands after that text — read once more right
# before typing. Type it when the box is empty, unchanged, or holds only new dim suggestion text.
last = draft(pane)
if last is None or (last not in ("", left) and not dim_only(pane)):
    skip("text appeared in the input box meanwhile", "stopped erasing. Only point out to the person that it can be cleared")
say ="supervisor " + me + ": checked the report for " + epic + " — clearing this window" + (". The draft is copied into the supervisor's window" if kept else "")
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
        print("cleared —", name, pane, epic)
        sys.exit(0)
print("cannot tell whether it cleared —", name, pane, "— look at that window before sending the next text")
PY
```

- **Call it only on a session you handed work to.** The session that sent the report is the
  worker, so the supervisor's own window and another supervisor's window are already out by
  name. The script filters its own pane (`$TMUX_PANE`) once more as well
- **With no tmux it skips quietly.** If `$TMUX` is unset or `tmux` is missing it prints one
  `not clearing` line and exits 0 — then point it out to the person and wait, as above. When
  the script prints `not clearing`, do what the end of that line says — only `not idle` means
  wait for the notification and call again; for the rest, point it out to the person
- **Type only into an `idle` pane.** Typing into `busy`, `waiting` or `shell` slips characters
  into a running turn or between a person's answers. Read once more right before sending. The
  times the worker said "do not clear" in 12 — a review running in the background, a merge
  conflict being resolved, waiting on a person's answer — hold here too. If the report or a
  message after it says any of that is left, do not call it
- **Do not type into a pane in copy mode, or a pane tied together by `synchronize-panes`
  either.** Copy mode means the person is scrolling to read, and the characters typed go to
  copy-mode keys where `/` opens a search. In a tied pane the characters go to every pane of
  that window and erase the conversation of the worker next door too
- **Read the pane from the session file's `tmux` field (`session:@window.%pane`) and check
  that the pane's process spawned that session.** The session file survives the session and
  pane numbers are reused, so someone else's session lives in the pane a stale file names
- **Erase the draft before typing** (user decision). Type over it and `<draft>/clear` goes to
  the worker as a prompt. Before erasing, read that text off the screen and copy it into the
  supervisor's window as `draft`, and one line on the status line of every client watching
  that pane says so — the person looks for that text in the supervisor's window. Claude
  Code's `Ctrl+Y` restores only the last line of a multi-line text, so it cannot be relied
  on. The input box is read as the last line of the Claude Code screen where the prompt glyph
  (U+276F) stands. That screen, like the session file, is undocumented, so where it cannot be
  read it falls towards not typing. If erasing stops midway, **only the lines erased so far**
  come out with it — `the draft is already erased` when it all went, and `erased from the
  draft` gives the lines when it stopped after one. The rest is still in the box, so handing
  the whole draft back would make the same lines stand twice. If what is left is not a line
  of the draft (the person typed meanwhile) what was erased cannot be told, and then it says
  to look at that box and leave the overlapping lines out
- **Strip only the two leading columns when copying** (user decision). The first line is the
  prompt and one space, the following lines two spaces, and the rest is the screen as it is —
  trim every line and indented code comes back flattened. Lines that do not carry that prefix
  are **not cut** — the day the screen draws differently two characters would be shaved off
  silently, and the copy is the only one the person has left, so nobody would see it shrink.
  A line the screen wrapped cannot be told from a newline the person typed, so the copy may
  show one newline more than there was
- **Tell text that will not erase apart by erasing it** (user decision). The dim suggestion
  Claude Code floats in an empty box is not what the person typed, and it does not erase.
  When it survives twenty strokes and all of it is dim (`capture-pane -e`), take it as a
  suggestion and type `/clear` — a suggestion does not land in front of `/clear`. Colour
  alone does not decide it, because on a build that draws the person's text dim `/clear`
  would land after that text. Text that erases is always taken as the person's. Claude Code
  draws the cursor of an empty box reversed over the suggestion's first character, so that one
  cell does not count as text. A suggestion reappearing in a box emptied of the person's text
  is the same — the draft is already copied out, so type it
- **Stop if the person types while you erase** (user decision). Read the input box again after
  every stroke, and the moment text appears that was not pared down from the earlier screen,
  stop and do not type `/clear` — that text was never copied out, so erasing more leaves the
  person no copy. Copy the new text into the supervisor's window as `typed meanwhile`, and
  `the draft is already erased` speaks for what went before. Merging them and erasing on is
  not the road taken — while the person types, `/clear` lands after their text. A line left
  must be **equal** to a line of the earlier screen to count as pared down — test only
  whether it is contained and one newly typed character is contained in some earlier line.
  If the new text is all dim it may be a suggestion that reappeared in a box emptied of the
  person's text, so do not stop: copy it out as `dim text that appeared meanwhile` and try
  erasing — if it erases, take it as the person's text and stop. Read the input box once more
  right before typing `/clear` too
- **Do not clear and assign in one breath.** `/clear` erases what is queued along with it.
  Send the next idea after the script prints `cleared` — after the session id changed — and
  on `cannot tell whether it cleared`, do not send before you have looked at that window
- **Leave one line in your own window when you clear** — `cleared <session> pane %N (<epic>)`.
  A person who was watching that window finds in the supervisor's window why the screen went away
- **Do not type into a live worker's window while testing.** Raise the pane you are testing
  on a **separate tmux server** and give the same name to **every** call that reaches that
  server — `new-session`, `send-keys`, `capture-pane`, `display-message`, `list-clients`,
  `kill-session`. Put the epic id in the name so it cannot collide with the test servers of
  the workers beside you or of a review subagent. To run the script in that pane, put a
  `tmux` wrapper that inserts `-L` at the front of `PATH` — the wrapper has to call the real
  `tmux` **by absolute path** so it does not call itself again. Call the script itself without
  `env -u TMUX` — with no `$TMUX` it skips as `outside tmux`

      env -u TMUX tmux -L <unique name> new-session -d -s <pane> …
      env -u TMUX tmux -L <unique name> capture-pane -p -t <pane>
      mkdir -p <scratchpad>/bin; printf '#!/bin/sh\nexec env -u TMUX %s -L <unique name> "$@"\n' "$(command -v tmux)" > <scratchpad>/bin/tmux
      chmod +x <scratchpad>/bin/tmux; PATH=<scratchpad>/bin:$PATH python3 - …      the script into that pane
      env -u TMUX tmux -L <unique name> kill-server          to clean up — only the server of that name dies

  **Never use `tmux kill-server` or `kill-session` without `-L`/`-S`.** Inside tmux a bare
  `tmux` follows `$TMUX` to the person's default server, and every pane and session on that
  machine dies at once. `TMUX_TMPDIR` does not fence it in — `$TMUX` wins.
  A bare `tmux new-session -d` is not isolation either, because it raises the pane on the
  default server — cleaning up then means running `kill-*` against the default server, and
  that road has killed a whole server before

## The shared root

The root checkout is **shared by every session.** While one session has a merge open
(`MERGE_HEAD`), another session committing a tracker note seals that merge with its own
subject — that has actually happened. So in the root, supervisor and worker alike:

- Give the tracker commit a path — `git commit -m "…" -- .moai/`. With a merge open git
  refuses a commit with a path, so wait until the session that opened it finishes and run it
  again. A `git commit` without a path seals that merge even when you ran `git status` first
- Finish your own merge in one call, `git merge --no-ff <branch> -m "…"`. Do not use
  `--no-commit`. If it stops on a conflict, do not resolve it in the root: `git merge --abort`

## When to stop

- If there is no idea that does not collide, or no session idling, say so to the person and
  stop — do not force a colliding idea out
- If a worker is waiting on a person's decision, the supervisor does not answer in their
  place. The decision is the person's
