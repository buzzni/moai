---
name: moai-supervise
description: Use when handing the ideas piled up on one repository, one at a time, to the agent sessions waiting on it as workers — Claude Code, Codex or Antigravity — and taking their reports. Triggers on "supervise", "hand out the ideas", "put the idle sessions to work", "감독해 줘", "idea 나눠 줘", "놀고 있는 세션에 일 시켜".
---

# moai-supervise — hand ideas out to the workers that are waiting

The supervisor **picks, sends and checks.** It does not fix code, it does not merge,
and it does not settle design in a worker's place. The workers merge. Overlapping
merges are prevented by splitting the files when the supervisor sends (1), and where
they still collide the worker goes back into its worktree and resolves them.

**Every session here is one a person opened** — the supervisor and its workers alike,
Claude Code, Codex or Antigravity in any mix. moai never launches an agent or runs one
headless, and neither does the supervisor. Work goes out as a letter (`moai send`) to a
window a person made a worker with the `moai-work` skill, and the report comes back the
same way. tmux is nobody's requirement: on tmux the supervisor may also clear a worker's
window (5-1), and that is all it adds.

**Work you send out is always done in a worktree** — even if the repository has no
worktree convention. Several workers share one root checkout, so fixing things in the
root mixes their edits and commits together. Worktrees stand in
`<root>/.worktrees/`, and `moai init` writes that path into the gitignore.

**Say hello first** — once per window, so the workers have someone to report to:

    moai hello --role supervisor

The name in its reply is `<my name>` below — the name the workers send their reports to.
The role keeps letters to `any-idle-worker` away from you: a supervisor never takes those.
In Codex, moai finds this window by the session id Codex sets in its shell
(`CODEX_THREAD_ID`) — the row its hooks wrote. If `moai hello` says it cannot tell which
session this is, Codex did not set it: say hello as that row, `moai hello --role
supervisor --as <that name>` (its first context names it, `codex-` and eight characters),
and pass the same `--as` to every `moai send` and `moai inbox` you run as yourself.

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

Fill the name you read into `<base branch>` in the commands below and in the letter you
send the worker. **The worker does not read it again** — read inside a worktree, it
gives that worktree's own branch.

## Words per agent

moai plants the same skills for Claude Code, Codex and Antigravity, so the steps in them
are named by what they do — a step written in *italics* is a row of this table. Each agent
types a step its own way — read your own column.

| Step | Claude Code | Codex | Antigravity |
|---|---|---|---|
| Enter the worktree | `EnterWorktree(path)` from the root | `cd` into it and run every command there | `cd` into it and run every command there |
| Come back to the root | `ExitWorktree(keep)` | `cd` to the root and run every command there | `cd` to the root and run every command there |
| Ask the person watching | `AskUserQuestion` | `request_user_input` | ask in the conversation and wait |
| Review the work | `/code-review` | the review this session has, else read the diff yourself | the review this session has, else read the diff yourself |
| Change the model (the person does it) | `/model` | `/model` | — |
| Clear the window (the person, or a supervisor on tmux) | `/clear` | `/new` | `/clear` |
| Call a skill (the person does it) | `/<skill>` | `$<skill>` | ask for the skill by name |
| Wake a session that sits idle (a bonus) | `moai send --wake`, or `SendMessage` when it says so | `moai send --wake` | `moai send --wake` |
| Stop what a review left running | `TaskStop` | — | — |

A `—` is a step that agent does not have, or one moai does not know yet: tell the
person watching and go on without it.

## One round

**0. Reclaim first — work that lost its place.** When a session dies the row it picked
up stays `in_progress` and nobody carries it on. Look at this before picking new ideas.

    moai status --json                     the ids of warnings whose kind is "stranded"
                                           (inside a worktree it stands only with `--worktree`)
    moai show <id>                         the `Place` line — one of the four words below
                                           (inside a worktree this too needs `--worktree`)

`stranded` is a row that was picked up while no live worktree holds that work — either
the worktree is gone, or **the work was being done in the root with no worktree**. This
row alone does not tell the two apart: if `moai agents` — every row, not only the idle
workers of 2 — shows a live session that is `busy`, it may be that one, so ask what it is
holding before handing the work on: a worker with a letter, any other window through the
person.

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
worker — the one you sent that work to — is gone from `moai agents`. A worker on another
machine (`here` false) or in Codex has no process to look at: its row is swept only a day
after nothing marked it, and it reads `gone` after 20 quiet minutes, **which is not an
ended session** — a window resting at its prompt while its person answers marks nothing,
nor does a long stretch of only reading files, nor a Codex window whose hooks are not
trusted. Hand on the work of a worker that reads `gone` but still stands in `moai agents`
only once the person says that window has ended; until then it is that worker's.

- When there is such work, hand carrying it on to one waiting worker **before any new
  idea**. Send the letter in 3 with its first two lines changed to the two below, and the
  rest filled as 3 says (`<other work>` too — 4-3 points at that line). The worker's
  `moai-work` skill has the section the first line names

      Supervisor <my name> hands you the stalled work in <epic> — the previous session did not finish it. Do it by the `moai-work` skill, from "Carrying on stalled work".
      Read first: moai show <epic> (history and notes) · moai show <member> (the place too — a place stands on work only)

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
first-column members (what the worker's 7-1 left behind) shows as `in_progress` but is not
picked up — there is no worktree and no picked-up member, so do not drop ideas over it.

**If a milestone is running, what is inside it comes first.** The line `moai ready` prints
under its list says what is running and how many it held back outside it, and the
`moai status` notice shines on the same thing. Then what you send this round is work
attached to that milestone — an idea from outside waits for the next round unless it
should stand as `p0`.
**The tool does not block this** (a pick-up goes straight through), which is why the
place to decide is here. If two milestones are running, both are inside.

**Work is never pulled into a running milestone — the supervisor does not bring an
outside idea in.** `moai idea promote` carries over the body and the release the idea
stands in — the one `moai show --milestone` lists it under, not its own field — so an
idea parked outside the release unfolds into an epic that stands outside it, and there it
stays. What you send while a release runs is work that already stands in it; an idea from
outside waits for the next round, unless it should stand as `p0` or the person attaches
the release themselves. **So `<milestone>` in 3 is the release that idea already stands
under, never one you picked for it**: the line the worker runs in its step 1 —
`moai edit <epic> --milestone <milestone>` — re-affirms what `promote` carried and is not a door you open. With
nothing running, and for an idea that stands under no release, it is `none`.
The 2026-09-21 round is why both halves are written down: a worker picked up a row outside
the running release, and the person, not the tool, is what caught it. The answer is to
stop sending outside work while a release runs, not to hang the release on it — hanging it
on would make the release grow after it started, and that is the person's call alone.
**The tool refuses none of this**, so this paragraph is the only thing holding it.

**An idea you sent comes out of the candidates until its report is checked.** Until the
worker unfolds it, it stays in `moai idea ls`, and the same idea goes to a second worker.

**Send only what is yours.** An idea or member whose assignee is someone else — or
nobody — is asked about first: ask the person, and send it only on a yes, writing in the
letter who said yes so the worker takes it over (`--take`, hook rule 5). `moai ready` sets
such rows apart under `others`.

**2. Find a worker.** A worker is a window where a person called the `moai-work` skill:
it said `moai hello --role worker` and waits for a letter, and while it waits its row
reads `idle`.

    moai agents --json --role worker --status idle

Each row names the agent (`name` — what you send to), its `vendor` (which column of the
words table it reads, and whether 5-1 can clear it), whether it runs on this machine
(`here` — its machine is this one, or it names none; 5-1 clears only those) and its tmux
pane when it has one. The rows are this repository's: the list follows the tracker into
the root, so every worktree of it sees the same one — and so do other machines
(containers) sharing it. A row whose process is gone is swept as it is read; a row on
another machine or in Codex has no process to look at, so it reads `gone` once nothing
marked it for 20 minutes, and is swept after a day.

- **Hand work only to a row whose role is `worker` and whose status is `idle`.** `busy` is
  working — on your work or on the person's — and a session with no role is one nobody made
  a worker: it does not wait for letters, so a letter to it sits until someone types there
- **Leave out a worker whose sent idea has not had its report checked.** A worker reads
  `busy` while it unfolds, picks up and merges, and it may read `idle` for a moment between
  two waits
- **`idle` also stands on a window whose turn ended without waiting again** — its person
  stopped the wait to talk to it, or the window was just cleared. A letter sent there sits
  until that window's next prompt: if the row still reads `idle` well after you sent, nobody
  took the letter — ask the person watching that window
- **If no row comes back, nobody waits here.** Ask the person to call `moai-work` in the
  windows that should take work (*Call a skill*), and stop — do not send to a session that
  is not a worker
- **A worker that refused the work comes out of the candidates and is not sent to
  again.** Some sessions take work only from their own person — once one has refused,
  do not even leave a letter on it after that
- **A test agent is no worker.** One raised on a separate tmux server registers here like
  any session, but it reads no role until someone says `moai hello --role worker` in it —
  the role is what keeps it out of this list

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
letter carries two sets of judgement, and on the day they differ the cheap model takes the
write path.

| Level | How it is measured | Model |
|---|---|---|
| `low` | text, comments, a one-line fix; behaviour unchanged | `haiku` |
| `medium` | a behaviour change inside one file, ringed by tests | `sonnet` |
| `high` | several files, the write path, concurrency, the storage format, hooks; hard to undo | `opus` |

The model column is Claude Code's. For a worker on Codex or Antigravity, fill `<model>`
with `—` — the person picked the model when opening that window — and still write the
difficulty: it picks the epic-end review's grade there too.

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
a letter and cannot be changed by config — the person in that window changes it
(*Change the model*).

**3. Send.** Send **one** idea to one waiting worker, as a letter. The worker's steps are
in its `moai-work` skill, so the letter carries only the assignment — but all of it: the
worker knows nothing of this conversation. Write the letter below to a file in your
scratchpad with every slot filled, and send it:

    moai send <worker> '<id> — <title>' -b - < <letter file>

Fill in `<my name>`, `<id>`, `<title>`, `<base branch>`, `<milestone>`, `<model>`, `<difficulty>`, `<why>`, `<other work>`, `<root>`, `<after>`, `<person>` and — only for a subdirectory project — `<subdir>`.
`<my name>` is the name your hello printed — the worker reports to it.
`<root>` is the `root dir` from 2. **Leave it unfilled** and the worker, inside its worktree,
reads its own place as the root. With no `subdir` line in 2, leave the `Subdir:` line out
of the letter.
`<milestone>` is the release that idea already stands under **and that is still alive**,
read in 1 — `none` when it stands under none, `none` when the one it stands under has
shipped or been deferred (the worker would otherwise re-open a release that is already
out, which is what `promote` itself declines to carry), and `none` when nothing is
running. It is never a release you picked for it: work is not pulled into a running
milestone (1). **Leave it unfilled** and the worker hangs the placeholder itself on the
epic, which the tool refuses because it is not an id at all. **A wrong id it does not
refuse** — the check is the shape, not whether that milestone stands, so a stale one goes
in with exit 0: one line on stderr says there is no such milestone, and `moai status`
counts the epic as `dangling_milestone`. Copy it off
the release `moai show --milestone` stands that idea under; do not write it from memory.
`<model>`, `<difficulty>` and `<why>` are the pair you picked in 2-1 and your reason.
**Leave them unfilled** and those placeholders travel as they are, so the note the worker
leaves when it closes says `<model>` instead of what actually did the work.
Do not use a single quote inside `<why>` — it closes the single quote in the worker's 9-1
and the rest of the text leaks into the shell. The same goes for `<title>` in the send line,
and there the subject also has to stay within 200 characters — `moai send` refuses a longer
one — so cut a long title in the send line; the letter's first line carries it whole.
`<other work>` is the sibling worktrees you measured in 1, the work you send in this same
round, and the files that work holds — `none` if there is none. What the supervisor
measured before sending cannot cover a file that turns out to be needed mid-epic, so when
the worker meets such a file it does not fix it: it leaves it as a member and reports it
(its 4-3).
`<after>` is `end the turn` only when you will clear that window yourself in 5-1 — you run
inside tmux, whatever your vendor, the worker's row says `here` (its machine is this one,
or it names none — containers can share a tmux socket path, so the socket alone does not
tell) and carries a `tmux_pane` on your tmux server, and its vendor is `claude` (5-1 reads
Claude Code's screen only). 5-1 then sends the next letter itself and wakes the emptied
window. Otherwise it is `wait again`: the worker reports and goes straight back to waiting.
`<person>` is `here`, or `away` when the person told you they are stepping away — the
worker then settles a design question by its own recommendation instead of waiting on an
answer, writes down what it decided, and stops at what cannot be undone.
Do not fill `<grade>` — that is the review grade the worker picks in 7, after developing.
Do not fill `<vendor>` or `<count>` either — those are the vendor and the token count the
worker reads in its own window in 9-1.

    Supervisor <my name> hands you idea <id> — <title>. Do it by the `moai-work` skill, from step 1.
    Read first: moai show <id>
    Model: <model> (<difficulty> — <why>)
    Work running alongside: <other work> — do not touch those files (4-3)
    Base branch: <base branch>
    Milestone: <milestone>
    Root: <root>
    Subdir: <subdir>
    After the report: <after>
    Person: <person>

**Waking is a bonus.** A worker that waits needs none. The one window to wake is one you
clear in 5-1, and the script does it: after the clear it sends the next letter and types
`moai inbox` into the box it just emptied, and the hooks load the letter as that prompt
arrives. `moai send --wake` never types into a Claude Code pane — for one you did not just
clear, a Claude Code supervisor wakes it with SendMessage (*Wake a session that sits idle*).

**4. Wait.** Wait for the reports the way a worker waits for work:

    moai inbox --ack --wait 540

and again when it runs out — keep one wait inside your own limit for a shell command, and
ask for that limit: Claude Code's Bash tool gives a command two minutes unless you pass it a
longer `timeout`, at most ten. A
report is a letter from the worker (`report: <epic>`); with the hooks installed (Claude
Code), one that comes as a turn ends or a prompt arrives is loaded into the conversation
and marked read, and it is the same letter. **Do not sweep `moai agents` over and over** —
the report comes to you. A worker that asked its person something sends nothing until it
is answered.

**If it comes back at once with a non-zero code, no letter and a line naming the mailbox**
(`…/.moai/mail: it points at …`), the mailbox cannot be opened — a link the repository holds
points out of the checkout or into `.git`. Waiting will not open it — do not run it again.
The letters you send are refused the same way, so no worker hears from you either: tell the
person watching this window (*Ask the person watching*) and stop. A report that comes with a
line on the mailbox is still a report to check — that line is moai failing to mark it read,
so the next wait hands the same letter back at once. Check it once, then tell the person
watching this window and stop rather than wait again.

If the supervisor is in the root, then in the gap after the worker picks the member up
and before it raises its worktree, the hook holds that member as "still picked up" when
the supervisor's turn ends. **That member is the worker's** — do not move it, do not
defer it, do not put a note on it; just finish the turn.

**5. Check the report and send the next.** Look at three things before you believe a report.

    git merge-base --is-ancestor <merge hash> <base branch> && echo yes   is the merge on the base branch
    moai show <epic>                       are the unfolded epic and its members done
    git worktree list                      is that worktree gone

**A member left because the work beside it holds the file** (the worker's 4-3) goes to a
waiting worker after that other work's report is checked. Send the letter of 3 with `<id>`
filled with the epic and its first line changed to the one below. Until then, count it in 1
as work holding that file.

    Supervisor <my name> hands you epic <id> — <title>. It is already unfolded; do not promote. Do it by the `moai-work` skill, from step 2.

Like the ones from 7-1, that member is right even when it is not done — it keeps the epic
open, so the epic is not done either.

A member the report says was left in the first column by the worker's 7-1 is right even
when it is not done — that member keeps the epic open, so the epic is not done either. It
is not an idea and it does not show in 1's list, so pass it to the person along with the
reason it was left (a person's decision, a file held beside it).

`<epic>` is the epic id carried in the report. An idea is already done once it is
unfolded and it does not show its members, so `moai show <id>` cannot tell you whether
the work finished — when the report does not carry it, read it from that idea's history
line about being unfolded.

If the three hold, send the next idea. A worker whose letter said `wait again` is already
waiting — send to it straight away. A worker whose letter said `end the turn` ends its turn
right after the report: write the next letter to a file and hand it to 5-1 — the script
clears the window, sends that letter only once the clear went through, and wakes the
window. With no next idea, hand it `-` and it only clears. If 5-1 prints `not clearing`, do
what the end of its line says. Where that is pointing out to the person that the window is
at a good place to be cleared, send the next only after the person has cleared it or said
they will not — a letter loaded into the window before a late clear disappears with it,
and that idea and that worker sit out of the candidates waiting for a report that will
never come. Its turn has ended, so that letter waits until a prompt comes into the window:
a Claude Code supervisor wakes it with SendMessage, any other asks the person.

If they do not hold, ask that worker with a letter what is left, and do not finish it in
its place.

**5-1. On tmux, the supervisor clears the window.** Instead of pointing it out to the
person and waiting, the supervisor types the clear command (*Clear the window*) into that
worker's pane. Call it only after the three hold, and only after the `Next:` note the
worker leaves in its 11 stands in the history of `moai show <epic>`. Only a note that
stands **after the letter that sent this work** counts — `Next:` is also the hand-over
line a session leaves when it could not finish, so an epic reclaimed in 0 already has the
previous session's one. The report is the worker's last act, so when it arrives the turn
is about to end; the script waits up to a minute for the row to read `idle`, so that all of
it ends inside a shell tool's limit (two minutes by default in Claude Code). `idle` alone
does not say the turn is over — `moai inbox --wait` writes it while its shell command runs —
so the script also stops when it finds that wait running under the worker: such a worker
takes the next letter as it is. Clearing erases the whole conversation that worker holds,
so never call it before the check. `<worker>` is the name of the worker that sent the
report, `<root>` is the `root dir` from 2 — the script asks `moai agents` and sends there —
`<letter file>` is the next letter for that worker, written as 3 says (`-` for none), and
`<subject>` is its subject, `<id> — <title>`.

The script reads Claude Code's screen — the input box under the prompt glyph — so it
clears Claude Code workers only, and only those whose row says `here`. For a worker on
another vendor or another machine it prints `not clearing`, and the letter 3 sent it said
`wait again`.

```sh
python3 - '<worker>' '<epic>' '<my name>' '<root>' '<letter file>' '<subject>' <<'PY'
import json, os, re, subprocess, sys, time
name, epic, me, root, letter, subject = sys.argv[1:7]
# Every line goes out as it is printed. Into a pipe Python holds them back, and the copy of
# the person's draft printed below would die with the script if it were stopped mid-way.
sys.stdout.reconfigure(line_buffering=True)
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
def row():
    """The worker's row in `moai agents --json`, asked in the root (this shell may stand
    elsewhere). The name is the key there, and a clear keeps it — the row's `session` is what
    changes. None when moai cannot be asked or the row is gone."""
    try:
        out = subprocess.run(["moai", "agents", "--json"], cwd=root, capture_output=True, text=True).stdout
        found = [a for a in json.loads(out)["agents"] if a.get("name") == name]
    except (OSError, ValueError, KeyError, TypeError):
        return None
    return found[0] if len(found) == 1 else None
def parents(pid):
    while pid > 1:
        yield pid
        try:
            out = subprocess.run(["ps", "-o", "ppid=", "-p", str(pid)], capture_output=True, text=True).stdout.strip()
        except OSError:
            return
        pid = int(out) if out.isdigit() else 0
def waiting(pid):
    """Does a `moai inbox --wait` run under that agent? Then the row's `idle` is the wait's,
    written while a shell command of a running turn goes on — not a window back at its prompt."""
    if pid <= 1:
        return False
    try:
        out = subprocess.run(["ps", "-eo", "pid=,ppid=,args="], capture_output=True, text=True).stdout
    except OSError:
        return False
    kids = {}
    for line in out.splitlines():
        p = line.split(None, 2)
        if len(p) == 3 and p[0].isdigit() and p[1].isdigit():
            kids.setdefault(int(p[1]), []).append((int(p[0]), p[2].split()))
    todo, seen = [pid], set()
    while todo:
        at = todo.pop()
        if at in seen:
            continue
        seen.add(at)
        for kid, words in kids.get(at, []):
            if "inbox" in words and any(w.startswith("--wait") for w in words):
                return True
            todo.append(kid)
    return False
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
    # and the clear command lands after the person's text.
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
pane = ""
if not os.environ.get("TMUX"):
    skip("outside tmux")
try:
    tmux("-V")
except OSError:
    skip("no tmux")
s = row()
if not s:
    skip("could not find exactly one agent named " + name + " in `moai agents` run in " + root + " — check that the `moai` on PATH answers `agents` there")
if s.get("vendor") != "claude":
    skip("this reads Claude Code's input box only, and " + name + " runs " + str(s.get("vendor")))
# A row from another machine names a pane and a pid there. Containers can share the tmux socket
# path, so the socket check below passes, and the pid here is someone else's process or none.
if s.get("here") is not True:
    skip(name + " runs on another machine" if "here" in s else "the `moai` on PATH does not say whether " + name + " runs on this machine")
pane = str(s.get("tmux_pane") or "")
if not pane.startswith("%"):
    skip("no tmux pane on that agent's row")
if pane == os.environ.get("TMUX_PANE"):
    skip("the supervisor's own window")
# Pane ids are per server — the same `%N` on another server is someone else's pane.
server = os.environ["TMUX"].split(",")[0]
if s.get("tmux_socket") and os.path.realpath(s["tmux_socket"]) != os.path.realpath(server):
    skip("that pane is on another tmux server")
# The report is the worker's last act, so the turn is about to end — wait for it, a minute at
# most. A shell tool gives a command two minutes unless told otherwise (Claude Code's default),
# and the steps below need their own seconds: stopped between erasing and typing, the box is
# left erased with nothing typed.
until = time.monotonic() + 60
while s.get("status") != "idle" and time.monotonic() < until:
    time.sleep(0.5)
    s = row() or s
if s.get("status") != "idle":
    skip("not idle — " + str(s.get("status")), "the worker is still in its turn — call again once it ends")
if waiting(int(s["pid"])):
    skip("it waits for a letter inside its turn", "send it the next letter as it is — it takes it without a clear")
owner = looks('#{pane_pid}')
if not owner.isdigit() or int(owner) not in parents(int(s["pid"])):
    skip("pane " + pane + " does not belong to that agent")
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
    # of the clear command, so type it. Do not require it to equal the draft — when a suggestion
    # reappears in a box emptied of the person's text, the draft is already copied out and only the
    # suggestion is left.
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
if (row() or {}).get("status") != "idle":
    skip("no longer idle")
if waiting(int(s["pid"])):
    skip("it went back to waiting for a letter inside its turn meanwhile", "send it the next letter as it is — it takes it without a clear")
if looks(QUIET) != "00":
    skip("the pane went into copy mode meanwhile")
# If the person typed after the last read, the clear command lands after that text — read once more
# right before typing. Type it when the box is empty, unchanged, or holds only new dim suggestion text.
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
    now = row()
    if now and now.get("session") != s.get("session"):
        break
else:
    print("cannot tell whether it cleared —", name, pane, "— look at that window before sending the next letter")
    sys.exit(0)
print("cleared —", name, pane, epic)
if letter == "-":
    sys.exit(0)
# Send only now. A letter that waited while the old turn was ending would be loaded into the
# conversation the clear just erased — the hooks mark it read as they load it, so it would be lost.
try:
    with open(letter) as fh:
        sent = subprocess.run(["moai", "send", name, subject, "--as", me, "-b", "-"], cwd=root, stdin=fh, capture_output=True, text=True)
except OSError as e:
    print("not sent —", e, "— the window is cleared; send the letter yourself")
    sys.exit(0)
if sent.returncode != 0:
    print("not sent —", sent.stderr.strip(), "— the window is cleared; send the letter yourself")
    sys.exit(0)
print("sent —", sent.stdout.strip())
# Wake it — `moai send --wake` never types into a Claude Code pane, so the script does, into the box
# the clear emptied a moment ago, behind the same fences as the clear: an idle row, no copy mode,
# a box that is empty or holds only dim suggestion text. Otherwise the letter waits for the next prompt.
time.sleep(1)
box = draft(pane)
if box is None or (box != "" and not dim_only(pane)) or (row() or {}).get("status") != "idle" or looks(QUIET) != "00":
    print("not woken —", name, pane, "— the letter waits for the next prompt in that window; point the person at it")
    sys.exit(0)
tmux("send-keys", "-t", pane, "-l", "moai inbox")
time.sleep(0.3)
tmux("send-keys", "-t", pane, "Enter")
print("woken —", name, pane)
PY
```

- **Call it only on a worker you handed work to, and whose letter said `end the turn`.** The
  worker that sent the report is the one, so the supervisor's own window and another
  supervisor's window are already out by name. The script filters its own pane
  (`$TMUX_PANE`) once more as well. A worker that waits again is inside its turn
- **With no tmux it skips quietly.** If `$TMUX` is unset or `tmux` is missing it prints one
  `not clearing` line and exits 0 — then point it out to the person and wait, as above. When
  the script prints `not clearing`, do what the end of that line says — only `not idle`
  means call again, and a worker found waiting inside its turn takes the next letter as it
  is; for the rest, point it out to the person
- **Type only into an `idle` pane.** Typing into a `busy` one slips characters into a
  running turn or between a person's answers — and `idle` is not enough on its own, since
  `moai inbox --wait` writes it while a shell command of the turn runs, so the script also
  looks for that wait under the agent's process. Read once more right before sending. The
  times the worker said "do not clear" in its 12 — a review running in the background, a
  merge conflict being resolved, waiting on a person's answer — hold here too. If the report
  or a letter after it says any of that is left, do not call it
- **Do not type into a pane in copy mode, or a pane tied together by `synchronize-panes`
  either.** Copy mode means the person is scrolling to read, and the characters typed go to
  copy-mode keys where `/` opens a search. In a tied pane the characters go to every pane of
  that window and erase the conversation of the worker next door too
- **Clear only a worker on this machine** — its row says `here`. A row from another machine
  (another container sharing this repository) names a pane and a pid on that machine;
  containers can share the tmux socket path, and that pid here is someone else's process or
  none. A `moai` on PATH too old to say `here` stops it as well. `here` is also true for a
  row that names no machine (an older or non-Linux moai wrote it), so the fences below stand
  for every row
- **Read the pane from the worker's row (`tmux_pane`, `tmux_socket`) and check that the
  pane's process spawned that agent.** A pane id names a pane on one server only, so a row
  from another server names someone else's pane here, and pane numbers are reused
- **Erase the draft before typing** (user decision). Type over it and `<draft>/clear` goes to
  the worker as a prompt. Before erasing, read that text off the screen and copy it into the
  supervisor's window as `draft`, and one line on the status line of every client watching
  that pane says so — the person looks for that text in the supervisor's window. Claude
  Code's `Ctrl+Y` restores only the last line of a multi-line text, so it cannot be relied
  on. The input box is read as the last line of the Claude Code screen where the prompt glyph
  (U+276F) stands. That screen is undocumented, so where it cannot be read it falls towards
  not typing — and that is why another vendor's window is not typed into at all. If erasing
  stops midway, **only the lines erased so far** come out with it — `the draft is already
  erased` when it all went, and `erased from the draft` gives the lines when it stopped after
  one. The rest is still in the box, so handing the whole draft back would make the same
  lines stand twice. If what is left is not a line of the draft (the person typed meanwhile)
  what was erased cannot be told, and then it says to look at that box and leave the
  overlapping lines out
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
  suggestion and type the clear command — a suggestion does not land in front of it. Colour
  alone does not decide it, because on a build that draws the person's text dim the command
  would land after that text. Text that erases is always taken as the person's. Claude Code
  draws the cursor of an empty box reversed over the suggestion's first character, so that one
  cell does not count as text. A suggestion reappearing in a box emptied of the person's text
  is the same — the draft is already copied out, so type it
- **Stop if the person types while you erase** (user decision). Read the input box again after
  every stroke, and the moment text appears that was not pared down from the earlier screen,
  stop and do not type — that text was never copied out, so erasing more leaves the person no
  copy. Copy the new text into the supervisor's window as `typed meanwhile`, and `the draft is
  already erased` speaks for what went before. Merging them and erasing on is not the road
  taken — while the person types, the command lands after their text. A line left must be
  **equal** to a line of the earlier screen to count as pared down — test only whether it is
  contained and one newly typed character is contained in some earlier line. If the new text
  is all dim it may be a suggestion that reappeared in a box emptied of the person's text, so
  do not stop: copy it out as `dim text that appeared meanwhile` and try erasing — if it
  erases, take it as the person's text and stop. Read the input box once more right before
  typing too
- **Do not clear and assign in one breath.** A clear erases what is queued along with it,
  and a letter the old turn loaded as it ended is read and gone. So the script sends the
  letter it was handed only after the row's `session` changed — `cleared`, then `sent` —
  and on `cannot tell whether it cleared` it sends nothing: look at that window before you
  send. On `not sent` the window is cleared and idle — send the letter yourself, and wake it
  as 5 says
- **Wake only the window you just emptied.** After `sent` the script types `moai inbox` into
  that box — the prompt the hooks load the letter on — behind the same fences as the clear.
  On `not woken` the letter waits for the next prompt there; point the person at it
- **Leave one line in your own window when you clear** — `cleared <worker> pane %N (<epic>)`.
  A person who was watching that window finds in the supervisor's window why the screen went
  away
- **Do not type into a live worker's window while testing.** Raise the pane you are testing
  on a **separate tmux server** and give the same name to **every** call that reaches that
  server — `new-session`, `send-keys`, `capture-pane`, `display-message`, `list-clients`,
  `kill-session`. Put the epic id in the name so it cannot collide with the test servers of
  the workers beside you or of a review subagent. To run the script in that pane, put a
  `tmux` wrapper that inserts `-L` at the front of `PATH` — the wrapper has to call the real
  `tmux` **by absolute path** so it does not call itself again. Call the script itself without
  `env -u TMUX` — with no `$TMUX` it skips as `outside tmux` — and with `$TMUX` naming that
  test server's socket: the script stops at a row whose `tmux_socket` is not `$TMUX`'s, and
  the test agent's row carries the test server's. The test agent has to stand in this
  repository's `moai agents` too, or the script finds no row — say `moai -C <root> hello`
  in its pane, with no role: no role keeps it out of 2's list, and its row is swept once
  that agent is gone

      env -u TMUX tmux -L <unique name> new-session -d -s <pane> …
      env -u TMUX tmux -L <unique name> capture-pane -p -t <pane>
      env -u TMUX tmux -L <unique name> display-message -p '#{socket_path}'          its socket
      mkdir -p <scratchpad>/bin; printf '#!/bin/sh\nexec env -u TMUX %s -L <unique name> "$@"\n' "$(command -v tmux)" > <scratchpad>/bin/tmux
      chmod +x <scratchpad>/bin/tmux; TMUX=<its socket>,0,0 PATH=<scratchpad>/bin:$PATH python3 - …      the script into that pane
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

- If there is no idea that does not collide, or no worker waiting, say so to the person and
  stop — do not force a colliding idea out
- If a worker is waiting on a person's decision, the supervisor does not answer in their
  place. The decision is the person's
