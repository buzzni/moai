---
name: moai-supervise
description: Use in Claude Code when handing the backlog piled up on one repository, one at a time, to the idle Claude Code sessions of that repository as workers, and taking their reports. Triggers on "supervise", "hand out the backlog", "put the idle sessions to work", "감독해 줘", "backlog 나눠 줘", "놀고 있는 세션에 일 시켜".
---

# moai-supervise — hand backlog out to the idle sessions of this repository

The supervisor **picks, sends and checks.** It does not fix code, it does not merge,
and it does not settle design in a worker's place. The workers merge. Overlapping
merges are prevented by splitting the files when the supervisor sends (1), and where
they still collide the worker goes back into its worktree and resolves them.

**This skill is for Claude Code, and moai carries no messaging.** The supervisor and its
workers talk with Claude Code's own tools:

- `ListAgents` lists the live sessions, and subagents too — each row's name, its kind
  (`interactive`, `bg`), whether it is busy or idle, and its tmux pane if it has one
- `SendMessage(to: <name>, message: …)` sends to one session. With `notify_when_idle: true`
  you also get one notice when that session goes idle; leave `message` out and it only
  subscribes
- A reply comes in as a cross-session message. Answer it by copying its `from` as `to`

**Every session here is one a person opened.** moai never launches an agent or runs one
headless, and neither does the supervisor. **A worker is every idle session of this
repository in `ListAgents`, except you** — a row whose name starts with the root
directory's name and a `-`. Nobody registers and nobody is asked which windows count.
The message you send is the whole assignment, and the worker's steps travel inside it (3).

Five things about the messaging, one line each:

- A session in a different permission mode holds an incoming message for its person's
  approval — a worker that stays idle after you sent may be waiting on that
- `notify_when_idle` answers only for a session on this machine — from one elsewhere no
  idle notice comes, only its report
- A subagent sends under its parent session's address — a message can come from a session
  that did not write it itself
- `@path` in a message attaches nothing — what the worker has to read goes into the message
- Never poll `ListAgents` in a loop — the report comes to you (4)

**Work you send out is always done in a worktree** — even if the repository has no
worktree convention. Several workers share one root checkout, so fixing things in the
root mixes their edits and commits together. Worktrees stand in
`<root>/.worktrees/`, and `moai init` writes that path into the gitignore.

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

Fill the name you read into `<base branch>` in the commands below and in the message you
send the worker. **The worker does not read it again** — read inside a worktree, it
gives that worktree's own branch.

## One round

**0. Reclaim first — work that lost its place.** When a session dies the row it picked
up stays `in_progress` and nobody carries it on. Look at this before picking new backlog.

    moai status --json                     the ids of warnings whose kind is "stranded"
                                           (inside a worktree it stands only with `--worktree`)
    moai show <id>                         the `Place` line — one of the four words below
                                           (inside a worktree this too needs `--worktree`)

`stranded` is a row that was picked up while no live worktree holds that work — either
the worktree is gone, or **the work was being done in the root with no worktree**. This
row alone does not tell the two apart: if `ListAgents` shows a `busy` session of this
repository, it may be that one, so ask what it is holding before handing the work on —
a worker with a message, any other window through the person.

The `Place` line (`place` under `--json`) has four values. **Only `none` is handed on.**

    <path> (<branch>)  at        it runs there. Go in and carry on
    not showing yet    fresh     just picked up — the gap while the worker raises its worktree. Leave it
    unknown            unknown   **a sibling worktree could not be read.** It may be there, so do not hand it on
    none               lost      it lost its place — only this one is reclaimed

**`stranded` being quiet does not mean there is nothing to reclaim.** If a sibling
snapshot that names no picked-up row cannot be read, the place verdict folds into
`unknown` for everything and this warning is locked for the whole repository. When the
`unreadable_worktrees` key stands under `status --json`, **fix that worktree and look
again** — an empty list read before that is not "none", it is "not counted".

**The `Trouble in sibling worktrees <n>` on the person's screen is a different number.**
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
died does not show under `stranded`** — it is a worktree in `git worktree list` whose
worker — the session you sent that work to — no longer stands in `ListAgents`. A session
that still stands there, idle, has not ended: its person may be answering it, or it may
be holding your message for approval. Hand its work on
only once the person says that window has ended; until then it is that worker's.

- When there is such work, hand carrying it on to one idle worker **before any new
  backlog**. Send the message in 3 with its first two lines changed to the two below, and
  the rest filled as 3 says (`<other work>` too — 4-3 points at that line). The worker
  steps you paste after the lines have the section the first line names

      You are handed the stalled work in <epic> — the previous session did not finish it. Do it by the worker steps below, from "Carrying on stalled work".
      Read first: moai show <epic> (history and notes) · moai show <member> (the place too — a place stands on work only)

- **Whether it is carried on or put down is not the supervisor's call.** If it looks like
  work to put down (`moai mv <id> todo`, `moai defer <id> -m '<why>'`), ask the person
- Work handed on to be carried is, like a backlog item, not sent again until its report is checked

**1. Pick.** Out of the backlog that have piled up, keep only the ones that do not collide
with what is open right now.

    moai backlog ls                           what has piled up
    moai show -s in_progress,review        what is picked up
    moai show <id>                         what that backlog touches

Look at `git worktree list` too. A backlog item that touches the **same files, the same area**
as a worktree already standing or an epic already picked up comes out of this round —
when two of them change the same place, one waits for the other at the merge. **Compare
the backlog you send in this same round against each other too** — a worker only raises
its worktree after it receives the work, so what you just sent is not in the lists above
yet. Do this count again for every further backlog. An epic left open with only
first-column members (what the worker's 7-1 left behind) shows as `in_progress` but is not
picked up — there is no worktree and no picked-up member, so do not drop backlog over it.

**If a milestone is running, what is inside it comes first.** The line `moai ready` prints
under its list says what is running and how many it held back outside it, and the
`moai status` notice shines on the same thing. Then what you send this round is work
attached to that milestone — a backlog item from outside waits for the next round unless it
should stand as `p0`.
**The tool does not block this** (a pick-up goes straight through), which is why the
place to decide is here. If two milestones are running, both are inside.

**Work is never pulled into a running milestone — the supervisor does not bring an
outside backlog in.** `moai backlog promote` carries over the body and the release the backlog
stands in — the one `moai show --milestone` lists it under, not its own field — so an
backlog parked outside the release unfolds into an epic that stands outside it, and there it
stays. What you send while a release runs is work that already stands in it; a backlog item from
outside waits for the next round, unless it should stand as `p0` or the person attaches
the release themselves. **So `<milestone>` in 3 is the release that backlog already stands
under, never one you picked for it**: the line the worker runs in its step 1 —
`moai edit <epic> --milestone <milestone>` — re-affirms what `promote` carried and is not a door you open. With
nothing running, and for a backlog item that stands under no release, it is `none`.
The 2026-09-21 round is why both halves are written down: a worker picked up a row outside
the running release, and the person, not the tool, is what caught it. The answer is to
stop sending outside work while a release runs, not to hang the release on it — hanging it
on would make the release grow after it started, and that is the person's call alone.
**The tool refuses none of this**, so this paragraph is the only thing holding it.

**A backlog item you sent comes out of the candidates until its report is checked.** Until the
worker unfolds it, it stays in `moai backlog ls`, and the same backlog goes to a second worker.

**Send only what is yours.** A backlog item or member whose assignee is someone else — or
nobody — is asked about first: ask the person, and send it only on a yes, writing in the
message who said yes so the worker takes it over (`--take`, hook rule 5). `moai ready` sets
such rows apart under `others`.

**2. Find a worker.** Call `ListAgents` once. A worker is a row that

- belongs to this repository. `ListAgents` shows no directory; a session takes its name from
  the directory it was opened in — `<root dir name>-` and a short suffix, as in `moa-issue-bc`
  for a session opened in `moa-issue`. A row whose name does not start that way — renamed, or
  opened somewhere else — is not one
- is a session a person opened — under "Peer sessions" and `interactive`. Not a subagent,
  yours or another session's (they stand under "Subagents", and a message to one resumes that
  subagent instead), and not a `bg` session
- reads `idle`
- is not you

Its name is what you send to. Nobody registers: any idle session a person opened here is a
worker, and the message is the whole assignment.

- **Leave out a worker whose sent work has not had its report checked.** It goes idle
  whenever its turn ends — while it asks its person something, say — and it is still
  holding your work
- **If no row is left, nobody is free here.** Tell the person, and stop — do not send to a
  session of another repository
- **A worker that refused the work comes out of the candidates and is not sent to
  again.** Some sessions take work only from their own person
- **A test agent is no worker.** One raised for a test is opened outside the repository (a
  scratchpad), so its name is not this repository's

The root checkout and, for a subdirectory project in a monorepo, the path down to it come
from the lines below. The root is where `.moai` stands, so for a monorepo it is the
subdirectory that has `.moai`, and a worktree stands for the whole repository — the worker
has to go into the same subdirectory inside it (its step 3). The first line is `root dir`;
a `subdir` line stands only when the root is not the top of the repository.

```sh
python3 - "$(git worktree list --porcelain | sed -n 's/^worktree //p' | head -1)" "$(git rev-parse --show-toplevel)" <<'PY'
import os, sys
if not sys.argv[1]:
    sys.exit("call this inside a git repository")
top, here = os.path.realpath(sys.argv[2]), os.path.realpath(os.getcwd())
while here not in (top, os.path.dirname(here)) and not os.path.isdir(os.path.join(here, ".moai")):
    here = os.path.dirname(here)
root = os.path.realpath(os.path.join(sys.argv[1], os.path.relpath(here, top)))
print("root dir", root)
if os.path.relpath(here, top) != ".":
    print("subdir", os.path.relpath(here, top))
PY
```

**2-1. Pick a model — by one word of difficulty.** The grade of the epic-end review
(the worker's 7) is measured on this same rubric, member by member. Keep two axes and the
message carries two sets of judgement, and on the day they differ the cheap model takes the
write path.

| Level | How it is measured | Model |
|---|---|---|
| `low` | text, comments, a one-line fix; behaviour unchanged | `haiku` |
| `medium` | a behaviour change inside one file, ringed by tests | `sonnet` |
| `high` | several files, the write path, concurrency, the storage format, hooks; hard to undo | `opus` |

The epic-end review is that epic's only review, because the members are not reviewed
separately (the worker's 5 and 7).

**The grade is one step above the heaviest member's difficulty** — `medium` if the members
are all `low`, `high` if one is `medium`, `xhigh` if one is `high`.
**If any member touched the write path, concurrency, the storage format or hooks**, it is `max`.
Raise it one more step if the epic crosses surfaces or carries several design decisions. If you hesitate, raise it.
**A worktree that carries members of two epics is measured as one epic and then raised one more step** — the review has to read both epics' contracts at once.
`max` is the top of the ladder: a step above it is still `max`.
The model follows that grade — `medium` means `sonnet`, `high` and up means `opus`.

The supervisor picks before reading any code, so this is a suggestion; the last word
belongs to the worker who read the issue. A running session's model cannot be changed by
a message and cannot be changed by config — the person in that window changes it with
`/model`.

**3. Send.** Send **one** backlog to one idle worker. The message carries the assignment —
the lines below, every slot filled — and after them, **the whole text of this skill's
`references/worker.md`**: the worker's steps. The worker knows nothing of this conversation
and has no skill of its own for this, so what is not in the message does not reach it —
read that file and paste it whole; a path or `@path` brings nothing.

    SendMessage(to: <worker>, message: <the lines below, a blank line, references/worker.md>, notify_when_idle: true)

Fill in `<id>`, `<title>`, `<base branch>`, `<milestone>`, `<model>`, `<difficulty>`, `<why>`, `<other work>`, `<root>`, `<person>` and — only for a subdirectory project — `<subdir>`.
`<root>` is the `root dir` from 2. **Leave it unfilled** and the worker, inside its worktree,
reads its own place as the root. With no `subdir` line in 2, leave the `Subdir:` line out
of the message.
`<milestone>` is the release that backlog already stands under **and that is still alive**,
read in 1 — `none` when it stands under none, `none` when the one it stands under has
shipped or been deferred (the worker would otherwise re-open a release that is already
out, which is what `promote` itself declines to carry), and `none` when nothing is
running. It is never a release you picked for it: work is not pulled into a running
milestone (1). **Leave it unfilled** and the worker hangs the placeholder itself on the
epic, which the tool refuses because it is not an id at all. **A wrong id it does not
refuse** — the check is the shape, not whether that milestone stands, so a stale one goes
in with exit 0: one line on stderr says there is no such milestone, and `moai status`
counts the epic as `dangling_milestone`. Copy it off
the release `moai show --milestone` stands that backlog under; do not write it from memory.
`<model>`, `<difficulty>` and `<why>` are the pair you picked in 2-1 and your reason.
**Leave them unfilled** and those placeholders travel as they are, so the note the worker
leaves when it closes says `<model>` instead of what actually did the work.
Do not use a single quote inside `<why>` — it closes the single quote in the worker's 9-1
and the rest of the text leaks into the shell.
`<other work>` is the sibling worktrees you measured in 1, the work you send in this same
round, and the files that work holds — `none` if there is none. What the supervisor
measured before sending cannot cover a file that turns out to be needed mid-epic, so when
the worker meets such a file it does not fix it: it leaves it as a member and reports it
(its 4-3).
`<person>` is `here`, or `away` when the person told you they are stepping away — the
worker then settles a design question by its own recommendation instead of waiting on an
answer, writes down what it decided, and stops at what cannot be undone.
Do not fill `<grade>` — that is the review grade the worker picks in 7, after developing.
Do not fill `<vendor>` or `<count>` either — those are the vendor and the token count the
worker reads in its own window in 9-1.

    You are handed backlog <id> — <title>. Do it by the worker steps below, from step 1.
    Read first: moai show <id>
    Model: <model> (<difficulty> — <why>)
    Work running alongside: <other work> — do not touch those files (4-3)
    Base branch: <base branch>
    Milestone: <milestone>
    Root: <root>
    Subdir: <subdir>
    Person: <person>

**4. Wait.** End your turn. The report comes back as a cross-session message from the
worker, `report: <epic>` at its head, and it wakes you. The idle notice that
`notify_when_idle` sends is not a report: a worker goes idle whenever its turn ends — while
it waits on its person's answer, say — and one that asked its person something sends nothing
until it is answered. **Do not poll `ListAgents`** — the report comes to you.

If the supervisor is in the root, then in the gap after the worker picks the member up
and before it raises its worktree, the hook holds that member as "still picked up" when
the supervisor's turn ends. **That member is the worker's** — do not move it, do not
defer it, do not put a note on it; just finish the turn.

**5. Check the report and send the next.** Look at three things before you believe a report.

    git merge-base --is-ancestor <merge hash> <base branch> && echo yes   is the merge on the base branch
    moai show <epic>                       are the unfolded epic and its members done
    git worktree list                      is that worktree gone

**A member left because the work beside it holds the file** (the worker's 4-3) goes to an
idle worker after that other work's report is checked. Send the message of 3 with `<id>`
filled with the epic and its first line changed to the one below. Until then, count it in 1
as work holding that file.

    You are handed epic <id> — <title>. It is already unfolded; do not promote. Do it by the worker steps below, from step 2.

Like the ones from 7-1, that member is right even when it is not done — it keeps the epic
open, so the epic is not done either.

A member the report says was left in the first column by the worker's 7-1 is right even
when it is not done — that member keeps the epic open, so the epic is not done either. It
is not a backlog item and it does not show in 1's list, so pass it to the person along with the
reason it was left (a person's decision, a file held beside it).

`<epic>` is the epic id carried in the report. A backlog item is already done once it is
unfolded and it does not show its members, so `moai show <id>` cannot tell you whether
the work finished — when the report does not carry it, read it from that backlog's history
line about being unfolded.

If the three hold, send the next backlog to an idle worker. **Clearing a window is the
person's** — the supervisor never types into a window. The report ends with the worker
telling its person when its window can be cleared, so a message you send to that same window
right away can be erased by a clear that comes after it, and that backlog then waits for a
report that never comes. Send to that window once the person has cleared it or said they will
not — a clear does not show from here, so ask the person — or send to another idle worker.

If they do not hold, ask that worker with a message what is left, and do not finish it in
its place.

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

- If there is no backlog that does not collide, or no idle worker, say so to the person and
  stop — do not force a colliding backlog out
- If a worker is waiting on a person's decision, the supervisor does not answer in their
  place. The decision is the person's
