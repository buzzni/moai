<!-- moai:begin v:1.1.0 hash:b5aaec02 -->
## Issue tracker — moai

This repository's work lives in `.moai/issues.jsonl`.
Do not use TodoWrite or a markdown TODO list. There is no approval gate — create anything, move anything. Do not ask a human, except before you pick up work that is someone else's or nobody's (hook rule 5).
Start a session with `moai status`. `moai ready --json` separates your ready work,
other people's or unowned work (`others`), and deferred or blocked work (`held`).

    moai prime · moai ready                 what you hold and can pick up
    moai tui                                open the explorer
    moai show <id> · moai show -g <keyword> details and search
    moai add '<title>' -p 1 -t bug -e <epic> create
    moai mv <id> in_progress                pick up → review → done
    moai note <id> '<what you found>'        leave a memo
    moai backlog add '<thought>'            park work for later
    moai wiki search '<words>'              search the manual

Every command accepts `--json`; use `moai <command> --help` for details.
`references/commands.md` below is the installed `moai` skill's reference —
`.claude/moai-plugin/skills/moai/` for Claude, `.agents/skills/moai/` for Codex and Antigravity.
`moai skill install` plants it; without it, command help is the reference.
In shared repositories pick up one id at a time with `moai mv <id> in_progress --from todo`;
a stale move fails rather than overwriting another session. `defer` also accepts `--from`.
A linked worktree's `moai`
reads and writes the main checkout's tracker; do not edit its `.moai` snapshot.

### The three forks

1. Use `moai add` for work you would pick up now; use `moai backlog add` for later.
   If an epic cannot deliver its promise without it, create a member in that epic even
   when it cannot be done now; do not park or defer that promise.
2. Use `moai defer <id> -m '<why>'` for work not done now; never mark it done.
3. If a request spans files, show one plan with an epic and three to seven issues and ask
   before coding. Use `/moai-plan` in Claude Code, or plan in this session in Codex and
   Antigravity. Ask about the deadline and any running milestone in the same breath;
   attach the milestone only on a yes. Ask when a requested issue or backlog item is due too;
   no answer means no deadline, and a start date is the person's to choose. On a yes use
   `moai add --from -`. See `references/commands.md` for plan syntax and exceptions.

### What to write in an issue

Pass a short title and a separate markdown body (`-b -`); use lists, not narrative or emoji.
The person's `[writing] detail` wins; default to `detailed`. Read `moai writing --json`
if the board did not say it. Detailed bodies cover goal, context, decisions, steps, tests,
out of scope, order, shared files and role; brief ones give what, where and next. Notes
record what was done and what remains. Keep machine-readable lines `model:`, `Lesson:`,
`Report-checked:`, `Review-ready:`, `Regression-of:`, `Summary:`, `Sent:`, `Next:`,
`report:`, `Model ran:`, `Subagent ran:`, `Unfolded:`, `Decided alone:` and `Plan stopped:`
in their original format. Do not shorten a reviewer's own words.
Bodies and wiki pages support footnotes, math, Mermaid, snippets and pictures (`moai image add`), notes
all but snippets; use them when lists fall short. See `references/commands.md` for syntax and surface limits.

### The five things the hook watches

1. **New issues stay inside what you picked up.** Create them in the same epic or under
   the issue; park later work in backlog. An epic's unfinished promise remains a member.
2. **Pick something up before you change the repository.** `moai mv <id> in_progress`.
   Shell writes count too; tracker, agent settings, worktrees, target and scratch do not.
3. **A review is an issue too.** Create it before calling a review:
   `moai add 'review — <what>' -t review --parent <id> -b '<angle>'`. Pick it up, record
   the reviewer's words separately from your decision, and close with `-m '<decision>'`.
   In Claude Code review with `/code-review`; in Codex and Antigravity use the review this
   session has, else read the diff yourself. Never start another agent program for review.
4. **Never kill the person's tmux server.** Test on a separate server with
   `env -u TMUX tmux -L <unique name> …`; under cmux the shim drops `-L` and `-S`, so
   use a real tmux by absolute path: `env -u TMUX <real tmux> -L <unique name> …`.
5. **Ask before you pick up someone else's work**, including an unowned row; on yes use
   `moai mv <id> in_progress --from todo --take -m '<who said yes>'`.

Hooks do not cover every agent or tool call; keep these rules yourself. Details and the
commands for refusals are in `references/commands.md`.

### Commits and closing

Put the issue id in the commit subject; squash commits need a `Refs: <id>` trailer.
A commit containing only a pickup or close starts `chore(tracker):`.
Before closing, record the AI that did the work without token counts:

    moai note <id> 'model: <vendor>/<model> (<grade> — <why>)'

Run `moai status`, leave `moai note <id> 'Next: <what comes next>'` for unfinished work,
and record repeatable mistakes with `moai wiki search` and `moai wiki learn`.

### Further details

- Explorer control: `references/commands.md` and `moai ctl --help`.
- Backlog promotion, deferral, groups and deadlines: `references/commands.md` and command help.
  While a milestone is running, work inside it comes first; `p0` is the outside hotfix slot.
  The tool does not block outside work; never pull work into a running milestone yourself.
- Projects, language, update checks, merge driver and archive: `references/commands.md`.
- Supervisor, workers and wiki: `references/commands.md` and the relevant installed skill.
- Refresh this block with `moai init`; `moai init --check` reports whether it is stale.
<!-- moai:end -->
