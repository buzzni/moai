# Day-to-day workflow

How work moves through moai when several sessions — people, agents, or both —
share one repository: picking work up, planning it, doing it in a worktree,
reviewing it and getting it merged. The first part holds in any repository that
uses moai; the last section points at what this repository adds on top. Flags are
in [the CLI reference](cli.md), what to do when something ends up wrong is in
[Recovery](recovery.md), and the words this page leans on are in
[the glossary](glossary.md).

## Plant the tracker

`moai init` in a terminal shows a short screen before it writes anything: the id
prefix (it cannot change later), whether git tracks the tracker, where the agent
guide goes ([the agents page](agents.md#where-the-guide-goes)), whether to install
the hooks and skills, the merge driver, and whether to add the repository to your
project list. Each row comes with its default picked; Enter plants, Esc stops with
nothing written. A flag picks its row and locks it. `--yes` asks nothing and plants
what `init` has always planted — committed, with the guide block — in every row no
flag sets; that is not what the screen picks. A script or an agent is never asked,
and gets the same as `--yes`.

**Whether git tracks the tracker** is the one choice that changes how everything
else behaves:

    moai init --tracking commit      shared through git (what init has always done)
    moai init --tracking exclude     kept in this clone; rules go to .git/info/exclude
    moai init --tracking gitignore   kept in this clone; rules go to .gitignore

Kept out of git, no committed file changes with `exclude` — use it to track your
own work in a repository that is not yours. There is nothing for git to merge, so
`.gitattributes` and the merge driver are left alone, and `moai status` does not
ask for them. `init` asks git which way stands every time instead of storing it,
so it will not switch it for you: moving `.moai` in or out of git is a commit you
make yourself (`git rm --cached`, or dropping the ignore line).

## Pick up work

1. `moai status` — start a session here. The board, the warnings and the flow come
   up on one screen, and nothing on it blocks
2. `moai ready` — what you can pick up right now, most urgent first
3. `moai mv <id> in_progress --from todo` — [pick one up](glossary.md#pick-up).
   `--from` names the [column](glossary.md#column) you saw it in, and the move
   happens only while the row still stands
   there. If another session took it first, you get one line on stderr, `stale`
   under `--json` and a non-zero exit — move on to the next row. Give one id per
   call; with several, the rows you won and the rows you lost share one exit code
4. `moai mv <id> review`, then `done` — the columns are `todo`, `in_progress`,
   `review` and `done` unless `statuses` in `.moai/config.toml` says otherwise

Work that belongs to someone else, or to nobody, stands apart under `others` in
`moai ready`. Ask its person before you [take it over](glossary.md#take-over) —
the [agents page](agents.md#the-five-rules) has the `--take` line, and why it is
the one place moai asks.

**Never move to `done` what you decided not to do.** `moai defer <id> -m '<why>'`
takes it out of the plan without changing its column or its
[kind](glossary.md#kind) — it stands [deferred](glossary.md#deferred) — and
`moai defer <id> --undo` brings the same row back. Leave a `moai note <id>` for
whoever comes next when you stop halfway — a [`Next:` note](glossary.md#next-note).

## Plan something bigger

When a request will not end inside one file, split it into one
[epic](glossary.md#epic) and three to seven issues, show the
[plan](glossary.md#plan) to the person once, and on a yes create it in one go:

    moai add --from - --dry-run      see what the plan would create
    moai add --from -                create it (the plan comes on stdin)

A plan is markdown: `# Epic title` opens an epic and `- [p1] issue title #tag`
puts an issue under it. Each issue gets an id under the epic's own,
`<epic>.<body>`, so a [member](glossary.md#member) cannot leave its epic later.

- **Do not move an epic or a [milestone](glossary.md#milestone).** A
  [group](glossary.md#group)'s column is read from its members: pick one member
  up and the group stands `in_progress`, finish them all and it stands `done`
- **Something for later is an [idea](glossary.md#idea)**, not an issue — `moai idea add`. Ideas stay
  off the board and out of `moai ready`, and `moai idea promote <id> --from -`
  unfolds one into an epic and issues when its time comes
- **A [running milestone](glossary.md#running-milestone) is the person's to fill.** A milestone runs once any of
  its members has started; from then `moai ready` hands out its work plus anything
  `p0`, and nothing else is pulled into it unless the person says so

## Work in a worktree

Each piece of work goes on its own branch in a linked git
[worktree](glossary.md#worktree), so sessions do not pile commits onto one branch
and wait on each other to merge.

- **The [tracker](glossary.md#tracker) stays in the main checkout.** A `moai` run inside a linked
  worktree reads and writes the main checkout's `.moai/`, and one line on stderr
  says where the write went. Commit tracker changes from the main checkout with
  `git commit -- .moai/`; the worktree's own copy stays as it was when it split
  off, so the merge never fights over it (moai-y7go). `MOAI_HERE=1` turns this
  off for one run — almost nobody needs it
- **Pick up first, then make the worktree.** Every session on the clone reads the
  main checkout's tracker, so a pick-up shows to the others the moment it is
  written; commit it there so it also reaches other clones
- **See what the neighbours hold.** `moai ready --worktree` and
  `moai status --worktree` overlay what the sibling worktrees picked up. A
  worktree whose directory is named `<id>`, or on a branch `<id>` or
  `worktree-<id>`, counts as the place that work is being done, and `moai status`
  warns about work that was picked up with no worktree at work on it
- **The wiki rides the branch.** Unlike `.moai/`, the wiki pages (`docs/` unless
  `wiki_dir` in `.moai/config.toml` says otherwise) are read from the worktree you
  are in and merge like any other file — write them on the branch, before the merge

## Merge the tracker file

`.moai/issues.jsonl` holds one line per issue, sorted by id, so two branches that
changed different issues collide only because their lines are neighbours. Install
the merge driver once per clone and git resolves that per issue, three-way:

    moai merge-driver --install

It records which binary git should call — the absolute path of the one that ran
it, unless the `moai` on your `PATH` is that same binary — in config that every
checkout of the clone shares. So install it from a binary that will not disappear
with a worktree, or pass `--as <path>`. If that path is gone, git falls
back to its own merge and conflict markers come back; `moai status` says so. A
clone without the driver merges exactly as plain git does.

## Name the issue in the commit

Put the id in the commit subject — `feat: draw the blocked line (<id>)`.
`moai show <id>` finds that issue's commits by the id in the subject; in the body
only `Refs:`, `Closes:` and `Fixes:` lines count. A commit that only picks up or
closes work starts with `chore(tracker):`, and the explorer's detail leaves it
out. Do not copy commit hashes into notes — a rebase makes them stale.

## Review

A review is an issue of its own, under the work it looks at:

1. `moai add 'review — <what>' -t review --parent <issue> -b '<what you look for and why>'`
2. `moai mv <review> in_progress` when the review starts, and move the work under
   review to `review`
3. `moai note <review> -b -` with the reviewer's own words — keep your own call in
   a second note, not mixed into theirs
4. `moai mv <review> done -m '<what you took in, what you handed on>'`, naming
   the id of anything handed on

The angle (`-b`) and the closing line (`-m`) are what the next reader looks for,
and for an agent the hook refuses a review without them (rule 3 on the
[agents page](agents.md#the-five-rules)). Before closing any issue, name the AI
that did the work on it — the [model line](glossary.md#model-line).

## In this repository

The above is moai. This repository adds its own conventions on top, written down
where its contributors read them rather than repeated here:

- **`CLAUDE.md`** — the worktree recipe (the base branch is `develop`, branches
  are `worktree-moai-<id>`, and `target/` is a link to `/tmp/cargo-target/<name>`
  because `/home` stalls under parallel builds, moai-c5xo), the review grade table
  (which `/code-review` level an epic gets, moai-9793, moai-bx6t), the
  [`Regression-of:` line](glossary.md#regression-of) on bugs a merged epic caused
  (moai-21zt), and how a
  milestone is named before and after a release
- **`CONTRIBUTING.md`** — building and testing, regenerating
  [the CLI reference](cli.md), and Releasing: the sections under `[Unreleased]` in
  `CHANGELOG.md` pick the next version, through `scripts/bump-version.sh`
  (moai-ug3j). moai itself does not number versions

## When it goes wrong

- **You lost a pick-up race.** The non-zero exit and the `stale` line are the
  tool working; take the next row from `moai ready`
- **`issues.jsonl` has conflict markers.** Abort the merge, install the driver,
  and merge again — see "A conflicted `issues.jsonl`" in [Recovery](recovery.md)
- **A worktree's `.moai/` changed.** An old binary or `MOAI_HERE=1` wrote there;
  replay the change against the main checkout — see "A worktree wrote the tracker
  in the wrong place" in [Recovery](recovery.md)
- **A command says `locked`.** Another moai is writing right now; wait and run it
  again. Deleting `.moai/lock` releases nothing

Decided in: moai-0zjo moai-40ht moai-bx6t moai-9793 moai-ug3j moai-gelm moai-tllo moai-zynt
