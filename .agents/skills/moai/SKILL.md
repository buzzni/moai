---
name: moai
description: Use for this repository's work, issues and plans. "what should I do first", "sort out the to-dos", "create an issue", "how is it going", "let us do this later", "뭐부터 할까", "할 일 정리", "이슈 만들어", "진행 상황", "이거 나중에 하자", when a feature request has to be split into several parts, or when something out of scope comes to mind mid-task. Use this instead of TodoWrite or a markdown TODO list.
---

# moai — this repository's issue tracker

The work lives in `.moai/issues.jsonl`. There is no approval gate — create anything, move anything. Do not ask a human, except before you pick up work that is someone else's or nobody's (hook rule 5).

    moai status                            board · warnings · flow (start a session here)
    moai prime                             what you hold and what is next, nothing else
    moai ready                             what you can pick up right now
    moai show <id>                         body, children, history. Why it was decided is here
    moai show -g <keyword>                 find out whether it is written down already
    moai show -s todo -t bug               filters (comma = or, repeated flag = and, except -a: or)
    moai show --tree                       epic → issue → child
    moai ready --worktree                  overlay what the other worktrees picked up
    moai stats                             counts: columns, flow, lead and cycle time, AI work
    moai tui                               walk the explorer. SPC n parks a thought
                                           on a group row: l one step · Tab expand all · h fold
    moai add '<title>' -p 1 -t bug -e <epic>   create
    moai mv <id> in_progress               pick up  →  review  →  done
    moai mv <id> in_progress --take        take over someone else's row (ask first)
    moai edit <id> --tag parser            change
    moai note <id> '<what you found>'      a memo for whoever comes next
    moai defer <id> -m '<why>'             take work out of the plan for now

Every command takes `--json`. `ready --json` gives `{"ready":[…],"others":[…],"held":[…]}` —
`ready` is yours to pick up, `others` is ready work that is someone else's or nobody's
(ask first), and `held` is what is deferred or blocked behind an empty group, and where
to pick it up again. A session a person opened reads that shape to choose its next row —
moai never launches or drives a session itself.

**A key that cannot be absent is never absent.** `kind` and `priority` hold a default,
and the file leaves a default out, but `--json` fills it back in — `jq -r .priority`
on a row gives `2`, never `null`. Keys that genuinely can be absent (`epic`,
`milestone`, `deferred_at`) stay absent, and that absence is the answer.

**Which epic a row is in, you read from `derived_epic`.** `epic` is what the file
says, and a row whose id sits under an epic (`<epic>.<body>`) reads its epic from
that id and writes no `epic` of its own — so on those rows `epic` is absent and
`derived_epic` names the epic. On a row that carries it, absence means the row is in
no epic at all, and on a group row it never stands. One row reads the two keys against
each other: where the same id stands twice and the other line is a different `kind`,
this line is counted into no group anywhere — the tree draws it under `(lost)`, `-e`
picks it up for no epic, and `derived_epic` is absent even when `epic` is written.
`duplicate_id` on the board names that id. A row below an id that stands twice, where
the lines hand down different answers, is counted into no group either — which line
it hangs from cannot be told — and `twin_parent` names it. Two surfaces carry neither key —
`rm --json` hands the removed lines back exactly as the file held them, and `tui
--json` prints the explorer's own shorter row — and there you read the epic off the id.

**When several sessions share one repository, pick up with
`moai mv <id> in_progress --from todo`.** It moves only while the column you saw
still holds, so it never overwrites work someone else picked up first — the loser
gets one line on stderr (`stale` under `--json`) and a non-zero exit code, and
moves on. `defer` takes the same `--from`. Without it nothing is blocked, as before.
**Pass one id only** — several at once mix winners and losers into a single exit
code, so you pick a row up and then throw it away.

**A `moai` run inside a linked git worktree reads and writes the main checkout's
tracker.** Editing the worktree's `.moai` makes that file conflict on the merge,
and then the only way out is outside the tool — one line on stderr says where it
wrote. So that worktree's `.moai/issues.jsonl` stays as it was when the worktree
split off, and the current rows are in the main checkout's file.

Whoever creates an issue is its assignee, for free.

## The three forks

**1. `add` or `backlog`** — what decides is *whether you would pick it up now.*
If you would, `moai add`; if it is for later, `moai backlog add '<what came to mind>'`.
A backlog item stands on neither the board nor `ready`, so it does not blur the plan.
**Walking past it without writing it down is the worst of all.**

If it came out of an epic, ask one more question first — *can this epic deliver
what it promised without this?* If not, it is not for later: it is this work,
unfinished. Even when you cannot do it now (waiting on a person's decision, the
work beside you holds that file), create it as a member with `-e <epic>` and
leave it in the first column — a member still standing keeps the epic from
closing by itself. Send it out as a backlog item and the epic stands `done` without
having delivered what it promised. To `defer` such a member is to decide to give
that promise up.

**2. `defer` or `done`** — never move to `done` what you decided not to do.
`moai defer <id> -m '<why>'` changes neither the column nor the kind, and
`--undo` brings the same row back as it was. A backlog item is "not work yet"; a defer
is "work, but not now".

**3. Is it worth splitting into an epic** — if the request does not end inside one
file, show the person **once**, before writing code, a plan split into one epic
plus three to seven issues, and ask. On a "yes", create it in one go with
`moai add --from -` (`--dry-run` shows it first).

```sh
moai add --from - <<'PLAN'
# Epic title
- [p1] first issue #enhancement
- [p2] second issue
PLAN
```

**A running milestone is the person's to fill.** When one is running, ask in the same
breath as the plan whether this bundle belongs in it, and attach it only on a yes —
`moai add --from - --milestone <id>`. It goes onto the epics the plan creates and the
members inherit it; without it the whole plan stands outside the release, and of it
`moai ready` then hands out only what is `p0`. Do not hang a running release on a plan
because the plan looks urgent: that is the release changing size while it runs.

**`--body` says why these issues are one bundle.** It goes onto the first epic the
plan creates, which is where `moai show <epic>` reads it from. `--body <text>` takes
the text itself (a file path there becomes the body as written), `--body -` reads
stdin, and only `--from <file>` reads a file. `--body -` and `--from -` cannot both
read stdin, so either put the plan in a file and stream the body —
`moai add --from plan.md --body -` — or keep the plan on stdin and pass the body as
text — `moai add --from - --body '<text>'`.

## What to write in an issue

**Pass the title and the body separately.** The title is an argument; the body is
markdown streamed in from stdin with `-b -` — a heredoc is easiest. Do not repeat
the title inside the body.

- **Keep the title short.** One line on what went wrong. The board, `ready` and the explorer list show the title alone
- **Do not write the body as a narrative.** Instead of laying out what happened in paragraphs, split it into a list —
  what went wrong, what you saw, where it gets fixed
- **Do not use emoji** — not in the title, not in the body. Their width differs per terminal, so the board and the tables come out crooked

The one thing that matters is that the next session reads this with
`moai show <id>`. These three are advice for that reason, not rules anything checks.

## The five things the hook actually watches

**1. New issues stay inside what you picked up.** The issue in focus is the one you picked up — it has left the
first column and is not closed yet (`in_progress`·`review`).
Anything that comes out of that work belongs in the same epic (`-e <epic>`) or
under that issue (`--parent <id>`). If the epic cannot deliver what it promised without this, it is one of those two
even when you cannot do it now (fork 1). If it is not for now, park it with
`moai backlog add` — a backlog item is always free of this rule, and so is
`moai add --from` (what it creates is an epic and its children, one unit on its own).

**2. Pick something up before you change the repository.** `moai mv <id> in_progress`.
What counts is work inside the repository — `.moai/`, `.claude/`, `.agents/`, `.codex/`,
`.worktrees/`, `target/` and anything outside the repository (scratchpad, temporary
files) do not. Shell writes (`>`, `>>`, `sed -i`, `tee`) count as much as `Edit` and
`Write`. If it was not in the plan, create it with `moai add 'a title'` and pick that up.

**3. A review is an issue too.** Before you call a review, create a review issue tied to
what you are reviewing. The review is `/code-review` in Claude Code; in Codex and
Antigravity it is the review this session has, else read the diff yourself.
It runs inside your own session — never start another agent program for it. The other
steps that differ per agent are under "Words per agent" in the `moai` skill.

    moai add 'review — <what you are looking at>' -t review --parent <the issue> -b '<what you are looking for and why>'
    moai mv <id> in_progress      when the review starts
    moai note <id> -b - < <review text>   the reviewer's own words (summarize past 64KB)
    moai mv <id> done -m '<what you took in, what you handed on>'

The angle (`-b`) and the closing line (`-m`) are **actually required.** Calling
without them is refused, and the refusal hands you the command to fix it.
**Do not ask a human** — running that command as given goes through.

**Keep the reviewer's words and your own call in two notes** — what the reviewer
said and what you decided are different texts. Write what you handed on **with
the issue id**. A line that only says "handed on" is never read again. Where the
review text lives is in the skill's `references/commands.md`.
If the text runs past 64KB, summarize it — put `Summary: original <size>KB agent-<task-id>` on
the first line, keep every finding's number and place, and shorten only the sentences. Leave fences and indentation alone.

**4. Never kill the person's tmux server.** `tmux kill-server` and `kill-session` without `-L`/`-S`, and
`pkill`/`killall` aimed at tmux, are refused. When the session runs inside tmux
`$TMUX` is set, so a bare `tmux` ignores `TMUX_TMPDIR` and attaches to that
server — one line kills every session in it. A tmux you are testing gets its
own server.

    env -u TMUX tmux -L <unique name> …

**5. Ask before you pick up someone else's work.** A `moai mv` into a started column on a row whose assignee is
someone else — or nobody — is refused unless it carries `--take`. Ask the person
watching first. On a yes, take it over and say who said yes:

    moai mv <id> in_progress --from todo --take -m '<who said yes>'

You become the assignee in the same write, and a note `Taken-over: <who it was|none>`
keeps whose it was. `moai ready` hands out only your own rows and sets the rest
apart (`others`), and what someone else picked up is not your focus. When who you
are is unknown, nothing is refused.

**Where no hook stands, the rules are words only.** The hooks stand where
`moai skill install` planted them for your agent — Claude's plugin, Codex's
`.codex/hooks.json` once a person has trusted it in `/hooks`, Antigravity's
`.agents/hooks.json`. An agent without them, and a tool call that sends no hook
(Codex sends them only for its shell, `apply_patch` and MCP calls) or that the
hooks do not read (input typed into a command already running), is refused
nothing — keep the five yourself. Rules 4 and 5 most of all: they guard the
person's other sessions and other people's work, and nothing else will.

## Words per agent

moai plants this skill for Claude Code, Codex and Antigravity alike, so a step that differs
per agent is named by what it does — each row of this table is one. Each agent types a step
its own way — read your own column.

| Step | Claude Code | Codex | Antigravity |
|---|---|---|---|
| Enter the worktree | `EnterWorktree(path)` from the root | `cd` into it and run every command there | `cd` into it and run every command there |
| Come back to the root | `ExitWorktree(keep)` | `cd` to the root and run every command there | `cd` to the root and run every command there |
| Ask the person watching | `AskUserQuestion` | `request_user_input` | ask in the conversation and wait |
| Review the work | `/code-review` | the review this session has, else read the diff yourself | the review this session has, else read the diff yourself |
| Change the model (the person does it) | `/model` | `/model` | — |
| Clear the window (the person does it) | `/clear` | `/new` | `/clear` |
| Call a skill (the person does it) | `/<skill>` | `$<skill>` | ask for the skill by name |
| Stop what a review left running | `TaskStop` | — | — |

A `—` is a step that agent does not have, or one moai does not know yet: tell the
person watching and go on without it.

## Before you close the session

Run `moai status` once more and see whether the warnings grew. Warnings block
nothing — they shine a light on issues with no epic, reviews stalled for a long
time, and how much you have open at once. Backlog piling up and what is deferred are
not warnings; they stand apart as notices (`notices`).

If you end the session still holding something, leave one line on that issue for
the next session to take over from. The next session reads it in the history under
`moai show <id>`.

    moai note <id> 'Next: <what comes next>'

Every command and the `--from` syntax are in `references/commands.md`.
