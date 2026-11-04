---
name: moai-supervise
description: Use in Claude Code when handing the backlog piled up on one repository, one at a time, to workers — subagents raised on the model each role calls for, or the idle Claude Code sessions of that repository — and taking their reports. Triggers on "supervise", "hand out the backlog", "put the idle sessions to work", "감독해 줘", "backlog 나눠 줘", "놀고 있는 세션에 일 시켜".
---

# moai-supervise — hand backlog out to workers on this repository

The supervisor **picks, sends and checks.** It does not fix code, it does not merge,
and it does not settle design in a worker's place. The workers merge. Overlapping
merges are prevented by splitting the files when the supervisor sends (1), and where
they still collide the worker goes back into its worktree and resolves them.

**A worker is a subagent you raise, by default.** Claude Code's `Agent` raises a subagent
with a model and an effort of its own, so each piece of work runs on the model its role
calls for (2-1) — a session runs on whatever its window was opened with. A worker is a
session of this repository instead in two cases: **the person asked to watch the workers'
windows**, or **the work goes to Codex**. Those are "Workers in sessions", below — that path
stays as it was. The moai binary never launches or drives an agent, and nothing here runs
headless: `Agent` is Claude Code's tool, and this skill is what calls it.

**A Codex worker is never a subagent — it lives in a cmux tab or a tmux pane: `moai-cmux` /
`moai-tmux`, section "Codex workers".**

**The models are the person's, read once a round.** Which model each role runs on is the
person's own choice, kept in their user config (`[models]`), so it is not written into this
skill — a skill planted at project scope is committed and shared. A repository may recommend
values in a `[models]` table of its `.moai/config.toml`; the person's own keys win over it.
Read it at the start of every round; a value changed mid-round counts from the next one.

    moai models --json      one object: a model per role (plan, develop, complex, review,
                            docs, bulk, codex), review_grade, review_raise, effort, effort_complex, effort_plan,
                            written — the keys the person set — and repo — the keys the
                            repository recommended, read from repo_path; the rest are defaults

How much agents write is the person's too. Read `moai writing --json` (`detail`) with the
models at the start of each round; a change mid-round takes effect in the next one.

When a value has to change, the person changes it — in their user config for a key in `written`
or a default, and over a `repo` key by writing the same key there. The repository's file is
committed and shared: changing it changes the recommendation for everyone, so name it as that.

**One supervisor per repository.** A supervisor waiting for reports reads `idle` in
`ListAgents` like any worker, so a second one sends it backlog, and the two keep separate
books of what was sent — one backlog, or one worker, gets two jobs. Before the first round,
ask the person whether another window here runs `moai-supervise`; if one does, stop. A
session that refuses work because it is a supervisor comes out of the candidates. **You
refuse too:** a message that hands you backlog to work on came from another supervisor — do
nothing of it, reply to its `from` that you are a supervisor, and tell the person.

**The hook reads your subagent workers as you.** A subagent's tool calls carry your session,
so its pick-ups are recorded as yours: the rows it holds are your focus (hook rule 1 reads
them for what you create, too), and when your turn ends the hook's `Stop` holds them —
once a session. Those rows are the worker's: do not move them, do not defer them, do not put
a note on them; just finish the turn.

**When sessions died** — a restart or an OOM kill took the workers or a supervisor down —
and the person asks to bring them back, load `moai-recover`, inside tmux or not. A subagent
worker lives inside your session and ends with it. Its rows stay picked up and its worktree
stays, so 0 does not list them as `stranded` — the place still reads that worktree, and no
window is there to ask about. So when you start again, a `Sent: subagent <agentId>` (1) with no
`report:` after it is work whose worker has ended: hand it on from "Carrying on stalled work"
as 0 does, once the person agrees that the session it ran in is gone.

## Workers in sessions

When using sessions, read `references/supervise-sessions.md` before selecting workers.

## Before the round

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

**If the root is detached, do not send.** The worker's tracker commit and its merge
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

**0. Reclaim first — work that lost its place.** Read `references/supervise-reclaim.md` to check for and reclaim stranded work.

**1. Pick.** First raise what a near deadline calls for, once a round — reading never writes, so a
deadline raises nothing until someone runs the verb, and the order `moai ready` gives is
only as fresh as that run. A member follows the earliest deadline above it as well as its
own, so the one an epic carries raises its members

    moai due apply                        raises priority by the deadline each row follows, never lowers

A row it raises to `p0` takes the hotfix slot even outside a running milestone — that is
what the person asked a deadline to do. Then, out of the backlog that have piled up, keep
only the ones that do not collide with what is open right now.

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
under, never one you picked for it**: the line the plan subagent runs (2-2) — or the worker,
in its step 1, when you send it the backlog — `moai edit <epic> --milestone <milestone>` re-affirms what `promote`
carried and is not a door you open. With
nothing running, and for a backlog item that stands under no release, it is `none`.
The 2026-09-21 round is why both halves are written down: a worker picked up a row outside
the running release, and the person, not the tool, is what caught it. The answer is to
stop sending outside work while a release runs, not to hang the release on it — hanging it
on would make the release grow after it started, and that is the person's call alone.
**The tool refuses none of this**, so this paragraph is the only thing holding it.

**A backlog item you sent comes out of the candidates until its report is checked.** Until the
worker unfolds it, it stays in `moai backlog ls`, and the same backlog goes to a second worker.
So the moment you send it, mark the row you sent — the backlog item, or the epic when the work
is already unfolded. Without it the send lives only in this conversation, and a supervisor
that starts again (4) cannot see it. The note needs no commit of its own — it stands in the
root's file the moment it is written, and the next tracker commit carries it ("The shared root").

    moai note <id> 'Sent: <worker>'

`<worker>` is the session's name, or for a subagent `subagent <agentId>` — its address (2).

**Send only what is yours.** A backlog item or member whose assignee is someone else — or
nobody — is asked about first: ask the person, and send it only on a yes, writing in the
message who said yes so the worker takes it over (`--take`, hook rule 5). `moai ready` sets
such rows apart under `others`.

**2. Raise a worker.** A subagent, unless the person asked to watch the workers' windows or
the work goes to Codex — then a session (below).

**A subagent.** One `Agent` call per backlog:

    Agent(subagent_type: "general-purpose", model: <model>, effort: <effort>, description: "<id>", prompt: <the message of 3>)

`<model>` goes in as the person's table has it, and `Agent` takes only the model names its
`model` parameter lists. **If `Agent` refuses the value**, the table holds a name Claude Code
does not raise a subagent on — tell the person which role and value, and do not swap in a model
of your own choosing: the table is theirs.
It runs in the background, and a notification comes when it ends (4). **How many run side by
side is the person's** — ask once a round (two unless they say otherwise); the ceiling is how
many backlog items do not collide (1). Do not pass `isolation: "worktree"`: the worker raises
its own worktree at `.worktrees/<epic>` from the base branch (its step 3), where this
repository's convention and the hook look for it — an isolated agent's worktree stands
somewhere else, on a branch of its own. **`Agent` takes no name**, so the `agentId` in its
result is the only address to wake that worker again (`SendMessage(to: <agentId>)`, 4) —
keep it in the send mark (1). Your own subagents stand in `ListAgents` under "Subagents";
they are not session workers.

**A session.** Read `references/supervise-sessions.md` for session selection and delivery.

**2-1. Pick the role, then read its model.** Each backlog goes to one role, and that role's
model and effort come from the `moai models --json` you read at the start of the round.

| Role | What it is for | Effort key |
|---|---|---|
| `plan` | planning — splitting a request into an epic and its issues | `effort_plan` |
| `develop` | development — most work | `effort` |
| `complex` | the write path, concurrency, the storage format, hooks, or a contract across several files | `effort_complex` |
| `review` | the epic-end review, once per epic | `review_grade` (the grade) |
| `docs` | a backlog that only touches the wiki or the CHANGELOG | `effort` |
| `bulk` | sorting and summing up, a review text past 64KB, the same mechanical fix many times | `effort` |
| `codex` | a Codex worker — never a subagent ("Codex workers") | — |

With nothing written and no repository table, the defaults are `plan` `fable` · `develop` `sonnet` · `complex` `opus` · `review` `opus` · `docs` `sonnet` · `bulk` `haiku` · `codex` `gpt-6.1-sol` · `review_grade` `xhigh` · `review_raise` `never` · `effort` `medium` · `effort_complex` `high` · `effort_plan` `xhigh`.

Development is split by one word of difficulty — `develop` or `complex`. The same rubric
decides, member by member, whether the epic-end review is raised a step when `review_raise`
allows it (the worker's 7). Keep
two axes and the message carries two sets of judgement, and on the day they differ the cheap
model takes the write path.

| Level | How it is measured | Role |
|---|---|---|
| `low` | text, comments, a one-line fix; behaviour unchanged | `develop` |
| `medium` | a behaviour change inside one file, ringed by tests | `develop` |
| `high` | several files, the write path, concurrency, the storage format, hooks; hard to undo | `complex` |

A backlog that only touches the wiki or the CHANGELOG is `docs`; sorting, summing up, or the
same mechanical fix many times is `bulk`. **If the planner wrote `Model:` and `Effort:` lines
in the issue body, read the model there as the role whose default it is** — the person's table
still decides which model that role runs on. `<effort>` is `effort_complex` for `complex` and
`effort` for the other worker roles; `effort_plan` goes only to the `plan` subagent (2-2).
The Codex worker's role is `codex`, and its sections say how it is sent.

The epic-end review is that epic's only review, because the members are not reviewed
separately (the worker's 5 and 7).

**The epic-end review runs on the `review` model at `review_grade`** — read both and `review_raise` from
`moai models --json` (defaults `opus`, `xhigh`, `never`).
With `review_raise = "write-path"`, raise one step if any member touched the write path, concurrency, the storage format or hooks, or if the worktree carries members of two epics. Raise at most once, capped at `max`; `never` leaves the configured grade unchanged.
A worktree that carries members of two epics is measured as one epic. If a worker's proposed grade differs, the supervisor corrects it from the settings and records why in the review issue angle.

The supervisor picks before reading any code, so this is a suggestion; the last word
belongs to the worker who read the issue. A session's model cannot be changed by a message
and cannot be changed by config — the person in that window changes it with `/model`. A
subagent's model is fixed when you raise it: a worker that finds the work heavier hands back
`Model: raise to complex` before it picks anything up (4).

**2-2. Unfold with the `plan` role.** Read `references/supervise-planning.md` for this step.

**3. Send.** Read `references/supervise-send.md` for the assignment and delivery steps.

**4. Wait.** End your turn.

**A subagent's report is the notification that it ended.** Its text comes as a message from
it (`<agent-message from="<agentId>">`) with `report: <epic>` at its head, and the
notification carries only numbers. Two other heads stop it before the work is done:

- `decision needed: <id>` — it met a design decision the notes do not settle and stopped there,
  with the situation, the options and its recommendation (a subagent has no
  `AskUserQuestion`). Ask the person with `AskUserQuestion`, and send the answer with
  `SendMessage(to: <agentId>)` — the same worker wakes and carries on from its own record.
  With `Person: away` it settles that by its recommendation instead and does not stop
- `Model: raise to complex — <why>` — it read the work as heavier than its model and picked nothing
  up. Leave it, and raise a new worker on the `complex` model and `effort_complex` with the same
  message (2) — but with its `Model:` line naming that model and the difficulty the worker measured,
  and its `Effort:` line the `effort_complex` value. Sent unchanged, the new worker reads the work as heavier
  than `Model:` says too and hands the same line back

Every wake ends in another notification; read the last. A subagent worker also leaves its
report on the epic as a note before it hands back — a supervisor that started again has lost
the notification, and reads that note (below).

For a session report, read `references/supervise-sessions.md` before checking it.

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

**A `member refused:` line** is a member the hook kept the worker from creating (the worker's
4-3): it stands as a backlog item under the epic. Unfold it into that epic before you send the
next — write only `- issue` lines in the plan; it then stands in the first column like a
member from 4-3 and keeps the epic open.
**If the line says the worker reclaimed it itself, or that backlog is already done, do not
unfold it** — promote unfolds a closed backlog too, and the same member stands twice. If the
hook refuses you as well, pass the line and the refusal to the person; do not create it under
another epic

    moai -C "<root>" backlog promote <backlog id> -e <epic> --from -

A member the report says was left in the first column by the worker's 7-1 is right even
when it is not done — that member keeps the epic open, so the epic is not done either. It
is not a backlog item and it does not show in 1's list, so pass it to the person along with the
reason it was left (a person's decision, a file held beside it).

`<epic>` is the epic id carried in the report. A backlog item is already done once it is
unfolded and it does not show its members, so `moai show <id>` cannot tell you whether
the work finished — when the report does not carry it, read it from that backlog's history
line about being unfolded.

**Before you mark it checked, settle the `model:` lines** — a supervisor that starts again
reads `Report-checked:` alone and does not go round these again, so a line missed before the mark
is missed for good, and a session worker told its window can be cleared is gone by then.

**For a subagent worker, write the `model:` line yourself**, one per member it did — not the
members it left in the first column — the same way, with no commit of their own. `<model ran>` is
the name after `anthropic/` on the report's `Model ran:` line — the real name the worker read in
its own system prompt, not the alias you passed. Copy the vendor once: `anthropic/anthropic/…`
is a line `work` never reads. Each `Subagent ran: anthropic/<model> on <member>` line below it
is a subagent the worker raised itself (its `Subagents:`) — write that on the member too, the
same form with `subagent` in the reason. Leave `tokens=` out: the count a
subagent's notification carries is not what the work cost (the person's decision), and a
false number is worse than none. A session worker writes its own line (its 9-1), and the
review issue's line is the worker's either way (its 10).

    moai note <member> 'model: anthropic/<model ran> (<difficulty> — <why>)'

**For a session worker, check that its lines stand** — the worker writes them, and a member
closed without one counts as nobody's work. Each id the line below prints, other than a member
left in the first column or one an earlier worker finished before this one was sent, is a line
it missed: ask the worker with a message to write it. Do not write it in its place — the model
that ran in that window is the worker's to name.

    moai -C "<root>" show -e <epic> --all --json | jq -r '.[] | select(.work == []) | .id'

If the three hold and no line is missing — none is owed by a member left in the first column or
one finished before this worker was sent, as above — mark the report checked on the epic — a
supervisor that starts again reads that line, not this conversation (4). It needs no commit of its own ("The shared root").

    moai note <epic> 'Report-checked: <merge hash>'

Then send the next backlog to a worker. **A subagent needs no clearing** — it ended with its
hand-back. Raise a new one for the next backlog rather than waking a finished one with new
work: its record is the old epic's.

A session worker's window is different. **Outside tmux and cmux, clearing a window is
the person's** — the supervisor never types into a window. The report ends with the worker
telling its person when its window can be cleared, so a message you send to that same window
right away can be erased by a clear that comes after it, and that backlog then waits for a
report that never comes. Send to that window once the person has cleared it or said they will
not — a clear does not show from here, so ask the person — or send to another idle worker.
**Inside tmux or cmux you may clear it yourself** — only after the report is checked and the note
above is written, only while `ListAgents` and its session record read `idle`, and only when its
input box is empty (`moai-tmux`, "Clear a worker's window"; `moai-cmux`, "Clear a worker's
window"). If any of these fails, do as the paragraph above says. Remove that pane's label once the report is checked (`moai-tmux`), or
that tab's (`moai-cmux`).
Inside Saycode, a worker Saycode lists is cleared through it, on the same report check
(`moai-saycode`, "Clear a worker").

If they do not hold, ask that worker with a message what is left — a subagent by
`SendMessage(to: <agentId>)` — and do not finish it in its place.

## The shared root

The root checkout is **shared by every session.** While one session has a merge open
(`MERGE_HEAD`), another session committing a tracker note seals that merge with its own
subject — that has actually happened. So in the root, supervisor and worker alike:

- **Commit the tracker rarely.** Every session reads the root's file, not git, so a move or a
  note shows beside you the moment it is written; a tracker commit keeps the file in history,
  it tells nobody anything. The worker commits once, at the close (its 11), and you commit
  once before you stop ("When to stop") — not after each send, check or note. That one commit
  carries every row written since, someone else's included, and that is fine: what it carries
  is already shared
- Give the tracker commit a path — `git commit -m "…" -- .moai/`. With a merge open git
  refuses a commit with a path, so wait until the session that opened it finishes and run it
  again. A `git commit` without a path seals that merge even when you ran `git status` first
- Finish your own merge in one call, `git merge --no-ff <branch> -m "…"`. Do not use
  `--no-commit`. If it stops on a conflict, do not resolve it in the root: `git merge --abort`
- Work inside a milestone does not merge in the root at all — it merges in the milestone's
  worktree (the worker's 8), and the root branch takes the milestone branch in at the
  release. Branches go in with `--no-ff` as they stand, never rebased or squashed
- **A repository can keep the tracker out of git** — `.moai` in `.gitignore` or
  `.git/info/exclude`. Ask once a round, in the root:
  `git -C "<root>" ls-files --error-unmatch -- .moai/issues.jsonl`. On a non-zero code every tracker commit here, yours
  and the worker's, does not exist — the notes stand in the root's file alone. The worker asks the same line itself ("Before the steps"), so the
  message needs no word about it

## When to stop

- Before you stop, take what you wrote into the root with **one** tracker commit, as "The
  shared root" says — `git status -- .moai/` first, and nothing to commit is fine. A round that
  never reached this line (its window died or was cleared) left its notes out of git, so when
  you start or resume (4), make the same one commit if `git status -- .moai/` is not clean
- If there is no backlog that does not collide, or no idle worker, say so to the person and
  stop — do not force a colliding backlog out
- If a worker is waiting on a person's decision, the supervisor does not answer in their
  place. The decision is the person's
