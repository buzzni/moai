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
directory's slug and a `-` (2). Nobody registers and nobody is asked which windows count.
The message you send is the whole assignment, and it names the file of the worker's steps,
which the worker reads (3).

**One supervisor per repository.** A supervisor waiting for reports reads `idle` in
`ListAgents` like any worker, so a second one sends it backlog, and the two keep separate
books of what was sent — one backlog, or one worker, gets two jobs. Before the first round,
ask the person whether another window here runs `moai-supervise`; if one does, stop. A
session that refuses work because it is a supervisor comes out of the candidates. **You
refuse too:** a message that hands you backlog to work on came from another supervisor — do
nothing of it, reply to its `from` that you are a supervisor, and tell the person.

Five things about the messaging, one line each:

- A session in a different permission mode holds an incoming message for its person's
  approval — a worker that stays idle after you sent may be waiting on that
- `notify_when_idle` answers only for a session on this machine — one reason a worker is a
  session on this machine (2)
- A subagent sends under its parent session's address — a message can come from a session
  that did not write it itself
- `@path` in a message attaches nothing — a file the worker has to read is named by its
  absolute path, and the worker reads it itself (3)
- Never poll `ListAgents` in a loop — the report comes to you (4)

**Work you send out is always done in a worktree** — even if the repository has no
worktree convention. Several workers share one root checkout, so fixing things in the
root mixes their edits and commits together. Worktrees stand in
`<root>/.worktrees/`, and `moai init` writes that path into the gitignore.

**Read the root branch once, at the start of the round.** Workers commit the tracker in
the root checkout and branch their worktrees from the local branches there, so that
checkout's current branch is the root branch — the remote's default branch may differ from
the root and may be stale. The root checkout is the first entry of `git worktree list`, so
the line below gives the root's branch no matter where in the repository you call it,
inside a worktree included. If nothing comes out, report the error git gave and stop.

```sh
if w=$(git worktree list --porcelain); then b=$(printf '%s\n' "$w" | sed -n '1,/^$/s|^branch refs/heads/||p'); if [ -n "$b" ]; then echo "$b"; else echo "the root is detached" >&2; fi; fi
```

**If the root is detached, do not send.** The worker's pick-up commit and its merge
land on a HEAD with no branch, the check (`merge-base`) and `worktree add`
fail, and `branch -d` deletes that work's only reference. Do not read the remote's
default branch instead — the root does not stand on that branch, so it is the same
accident. Ask the person to put the root on a branch, and stop.

Fill the name you read into `<root branch>` in the message you send the worker.

**Each work has a base branch** — the branch its worktree splits from and merges back
into. It is decided per work, by the release the work stands under — the one
`moai show --milestone` lists it under, read in 1:

- **Outside every milestone** — it stands under none, or under one that has shipped or been
  deferred (a `p0` fix, say) — it is the root branch, as it always was
- **Inside a live milestone** it is that milestone's own branch, `milestone/<milestone id>`,
  even while nothing runs yet and `<milestone>` in 3 says `none` — the first epic sent is
  what starts it. Every epic of the milestone merges there, and the root branch takes the
  milestone branch in once, at the release, so what a milestone has not shipped yet does
  not stand on the root branch. The branch is checked out in a long-lived worktree of its
  own, `.worktrees/milestone-<milestone id>`, because the root stays on the root branch. The
  worker handed the milestone's first epic raises it when it is missing (its "The milestone
  branch") — you do not raise it, and you do not remove it; it goes after the release

Fill that into `<base branch>` in the message and in the check of 5. For stalled work (0)
it is the release its epic stands under (`moai show <epic>`). **The worker does not
read either again** — asked inside a worktree, git answers with that worktree's own branch.

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
worker — the session you sent that work to — no longer answers. **A name gone from
`ListAgents` is not an ended session**: the name belongs to the process, so a window resumed
with `claude --resume` comes back under a new name, still in that worktree. A session that
still stands there, idle, has not ended either: its person may be answering it, or it may be
holding your message for approval. Look for the worker by its worktree, not its name — ask
the person which window works in it. Hand its work on
only once the person says that window has ended; until then it is that worker's. When it
comes back under a new name, move what you keep under the old one — the work you sent, a
refusal — to the new name (2).

- When there is such work, hand carrying it on to one idle worker **before any new
  backlog**. Send the message in 3 with its first two lines changed to the two below, and
  the rest filled as 3 says (`<other work>` too — 4-3 points at that line). The steps file
  has the section the first line names

      You are handed the stalled work in <epic> — the previous session did not finish it. Read <steps file> and follow its steps from "Carrying on stalled work".
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
So the moment you send it, mark the row you sent — the backlog item, or the epic when the work
is already unfolded — and take the note into the root with a commit with a path. Without it
the send lives only in this conversation, and a supervisor that starts again (4) cannot see it.

    moai note <id> 'Sent: <worker>'

**Send only what is yours.** A backlog item or member whose assignee is someone else — or
nobody — is asked about first: ask the person, and send it only on a yes, writing in the
message who said yes so the worker takes it over (`--take`, hook rule 5). `moai ready` sets
such rows apart under `others`.

**2. Find a worker.** Call `ListAgents` once. A worker is a row that

- belongs to this repository. `ListAgents` shows no directory; a session takes its name from
  the directory it was opened in, slugged — lowercased, every run of characters other than
  `a-z` and `0-9` turned into one `-`, cut at 4 words or 40 characters — then `-` and a short
  hex suffix: `moa-issue-bc` for `moa-issue`, `tvshop-updater-ca` for `tvshop_updater`. A row
  whose name does not start with the root's slug and a `-` — renamed, or opened somewhere
  else — is not one. **A name that does start so is still only a candidate**: `api-gateway-1c`
  starts with `api-`, and a session opened in a clone named `moai-web` starts with `moai-`.
  The worker confirms it: the message carries `Root:`, and a session standing in
  another repository refuses the work, so it comes out of the candidates (below)
- is a session a person opened — under "Peer sessions" and `interactive`. Not a subagent,
  yours or another session's (they stand under "Subagents", and a message to one resumes that
  subagent instead), and not a `bg` session
- runs on this machine — a Remote Control or cloud session cannot read the steps file at the
  path you name (3), and sends no idle notice
- reads `idle`
- is not you, and not a supervisor (one per repository, above)

Its name is what you send to. Nobody registers: any idle session a person opened here is a
worker, and the message is the whole assignment.

- **Leave out a worker whose sent work has not had its report checked.** It goes idle
  whenever its turn ends — while it asks its person something, say — and it is still
  holding your work. A resumed window comes back under a new name (0): while a worker you
  sent to is gone from `ListAgents` with its report unchecked, a name you have not sent to
  may be that worker — ask the person before sending to it
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

**3. Send.** Send **one** backlog to one idle worker. The message is the lines below, every
slot filled. The worker knows nothing of this conversation, so what is not in the message
does not reach it — except the worker's steps: they stand in this skill's
`references/worker.md`, and the first line tells the worker to `Read` that file and follow
it. The worker is a session of this same repository and loaded the same plugin, so the file
is there for it. **Do not copy the file into the message** — name it.

    SendMessage(to: <worker>, message: <the lines below>, notify_when_idle: true)

`<steps file>` is that file's absolute path: this skill's base directory — Claude Code shows
it as "Base directory for this skill" when the skill loads — followed by
`/references/worker.md`. Write it out whole; `@path` attaches nothing.
**If that base directory lies outside `<root>`** — the plugin installed at user scope, in
Claude Code's plugin cache — the worker's read of it is a read outside its working directory,
and in the default permission mode Claude Code asks its person first; the plugin cache is no
exception. A worker whose person is away waits on that prompt and sends nothing. Tell the
person once, before the first send, and let them choose: a person in the worker's window
answers it, or `permissions.additionalDirectories` in their settings holding that plugin
directory lets it through. The settings are theirs — do not write them.
Fill in `<id>`, `<title>`, `<steps file>`, `<root branch>`, `<base branch>`, `<milestone>`, `<model>`, `<difficulty>`, `<why>`, `<other work>`, `<root>`, `<person>` and — only for a subdirectory project — `<subdir>`.
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

    You are handed backlog <id> — <title>. Read <steps file> and follow its steps from step 1.
    Read first: moai show <id>
    Model: <model> (<difficulty> — <why>)
    Work running alongside: <other work> — do not touch those files (4-3)
    Root branch: <root branch>
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

**A report reaches only the name it was sent to.** A supervisor that started again —
restarted, or resumed with `claude --resume` — stands under a new name, and a worker whose
report to the old one fails leaves it on the epic as a note with `report: <epic>` at its
head. So when you start or resume, before waiting, read what the tracker holds:

    moai show -s in_progress,review        the work sent and picked up, not done
    moai show -g 'Sent:'                   sent and not done — a backlog not unfolded yet, or an
                                           epic not picked up yet; neither is a candidate
    moai show -g 'report:' --all           the epics carrying a report nobody received

**Only a note that opens with the marker counts** — `-g` matches any text, and a body or a
note that discusses this protocol carries the same words. Read each match with
`moai show <id>` and look at its history: a report stands checked once a `Report-checked:`
note follows it on the same epic. Check each one that does not as in 5. A worker holding
sent work you have no report for is still left out in 2 — ask it, or its person, how it
stands.

If the supervisor is in the root, then in the gap after the worker picks the member up
and before it raises its worktree, the hook holds that member as "still picked up" when
the supervisor's turn ends. **That member is the worker's** — do not move it, do not
defer it, do not put a note on it; just finish the turn.

**5. Check the report and send the next.** Look at three things before you believe a report.

    git merge-base --is-ancestor <merge hash> <base branch> && echo yes   is the merge on the base branch
    moai show <epic>                       are the unfolded epic and its members done
    git worktree list                      is that worktree gone

`<base branch>` here is the one you sent with that work. For work inside a milestone it is
`milestone/<milestone id>` — the merge lands there, not on the root branch, which takes it
in only at the release, so checking the root branch reads a good merge as missing. In
`git worktree list` the epic's worktree is gone and `.worktrees/milestone-<milestone id>`
stays — that one is the milestone's, not leftover work.

**A member left because the work beside it holds the file** (the worker's 4-3) goes to an
idle worker after that other work's report is checked. Send the message of 3 with `<id>`
filled with the epic and its first line changed to the one below. Until then, count it in 1
as work holding that file.

    You are handed epic <id> — <title>. It is already unfolded; do not promote. Read <steps file> and follow its steps from step 2.

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

If the three hold, mark the report checked on the epic and take it into the root with a
commit with a path — a supervisor that starts again reads that line, not this conversation (4).

    moai note <epic> 'Report-checked: <merge hash>'

Then send the next backlog to an idle worker. **Clearing a window is the
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
- Work inside a milestone does not merge in the root at all — it merges in the milestone's
  worktree (the worker's 8), and the root branch takes the milestone branch in at the
  release. Branches go in with `--no-ff` as they stand, never rebased or squashed

## When to stop

- If there is no backlog that does not collide, or no idle worker, say so to the person and
  stop — do not force a colliding backlog out
- If a worker is waiting on a person's decision, the supervisor does not answer in their
  place. The decision is the person's
