# Codex worker steps

Read `worker.md` first — it stands beside this file, whose absolute path the assignment's
first line names. Follow its assignment and step numbers; this file changes only the rows below.
You are Codex, a session in a tab or pane. The person watching this window comes first.
The supervisor skill remains Claude Code only; reading this file does not install it here.

| Instead of (worker.md) | Do this here |
|---|---|
| Never `EnterWorktree` ("Stay in the root") | The same: aim at the worktree's absolute path on every command call (`workdir` or `cd`). |
| `ExitWorktree(keep)` | Run every root command at the assignment's `Root:` absolute path. |
| `AskUserQuestion` | Ask this window's person with the available question tool; otherwise ask in conversation and wait. |
| `/code-review` (7) | Send the review readiness note below and wait for the supervisor's review. |
| `TaskStop` | No equivalent step here; the supervisor manages its review subagent. |
| `SendMessage(to: <supervisor>, …)` | Write an epic note in the root's tracker, as below. |
| `/clear` | The person or supervisor sends `/clear`. |

`/model` exists in Codex too. The person changes the model; use the assignment's `Model:`
as the suggestion, and record the model that actually ran. Do not change it yourself.
`Model:` names the Codex model (`[models] codex`); worker.md's Claude models and their ladder
do not apply here — when the work reads harder than its difficulty, say so in 9-1's reason.
`Subagents:` names Claude models raised with Claude Code's `Agent`; it does not apply here —
do the work yourself.
A question tool can depend on the mode: use `request_user_input_async` when available,
or `request_user_input` in Plan mode; without either, ask in conversation and wait.
One call's `cd` may not persist into the next: always supply the absolute command directory.

## Step 1: name the epic you unfolded

A backlog has no epic for the supervisor to watch until you unfold it. Right after the
promote in 1, before 2, name the epic on the backlog `<id>` you were handed:

    moai -C "<root>" note <id> 'Unfolded: <epic>'

Until it is written, a note the steps below put on the epic goes on that backlog instead.

## Steps 2–6

Keep worker.md's pick-up, worktree, design questions, root tracker, test server and file
ownership rules (2, 3, 4, 4-1, 4-2, 4-3, 5, 6). Never set `MOAI_HERE`.
The root branch check and path-only tracker commits apply here too. Return to `Root:`
before `git symbolic-ref -q HEAD`, `git status -- .moai/` and each root tracker commit (8, 11).
The assignment's base branch is where the worktree splits and eventually merges.
Hooks may be words only here: keep the five yourself. The person trusts moai in `/hooks`;
check that screen rather than assuming a wrapper or another installation enabled them.

## Step 7: hand the review to the supervisor

When the members are finished, pull the base branch as in 6, test and commit in the worktree.
Create the review issue using the shared command, called with `moai -C "<root>"` (4-1):

    moai add 'review — <what you are looking at>' -t review --parent <epic> -b '<what you are looking for and why>'

Fill the angle with the grade, why, and "review by the supervisor's review subagent", then
the epic body's `Review focus / 리뷰에서 볼 것` section (either heading) and the focus development
added, as worker.md's 7 says.
Do not run the review yourself. Move each finished member to review, one id per call:

    moai -C "<root>" mv <member> review --from in_progress

Keep worker.md's handling of missing review columns and members left in todo.
Record the worktree absolute path, branch, `git merge-base <base branch> HEAD` hash and
review issue id in a note whose first line is `Review-ready: <epic>`:

```sh
moai -C "<root>" note <epic> -b - <<'CODEX_REVIEW'
Review-ready: <epic>
- Worktree: <absolute path>; branch: <branch>; merge-base: <hash>
- Review issue: <review id>; tested: <checks and results>
CODEX_REVIEW
```

The note stands in the root's file, where the supervisor reads it — it needs no commit.
Stop until the supervisor pastes "merge and close — review <id> done"; its reviewer used
"The review brief" in `references/worker-branches.md` and its review model in this worktree. Then finish 7 as worker.md says — the
review's fixes stand uncommitted in the worktree: take them in as a separate `fix:` commit
and note what you hand on with the issue id — and go on with 7-1 to 11, and 12 below.

## Step 9-1: name the model that did the work

Use the actual OpenAI model name from the assignment's `Model:`; correct it if different.
The person's decision here is to omit token counts. Leave one line per finished member:

    moai note <member> 'model: openai/<model> (<difficulty> — <why>)'

Do not write the review issue's model line: the supervisor's reviewer writes its own.

## Step 12: report by epic note

Put worker.md's final report details in `moai note <epic> -b -`, called with
`moai -C "<root>"` from the worktree. Its first line is `report: <epic>`.
It stands in the root's file, where the supervisor reads it — it needs no commit.
For a decision needed during work, use the same note, starting with
`Decision needed:` or `진행 중 결정이 필요합니다`, and ask this window's person too.
The supervisor watches these notes; a note does not grant permission for a decision.
When the report is complete, tell the person here they may clear this window with `/clear`.
Do not clear it yourself; unexpected menus and permission prompts belong to the person.
