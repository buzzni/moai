# CLI reference

**Generated from `moai --help`. Do not edit by hand** — run
`scripts/gen-cli-docs.sh` after `cargo build --release`, and commit the result.
A test compares this file against the binary's own help, so a reference that
drifts fails the build rather than misleading a reader.

The help text itself is English, one copy for everyone: clap builds it before
the arguments are parsed, so it cannot follow `MOAI_LANG` (moai-l5uf). The rest
of the screen does follow it - see `moai --help` for how to pick a language.

## `moai`

```
Issue tracker. No approval gate. `moai status` shows the discipline.

Usage: moai [OPTIONS] [COMMAND]

Commands:
  status        Board, warnings, flow. A session starts here
  ready         What you can pick up now
  prime         A short markdown page - what you hold and what comes next
  add           Create an issue
  show          Open one, or list them
  mv            Move the status
  edit          Edit title, body, tags, epic or priority
  rm            Remove
  note          Leave a note on an issue (journal only)
  defer         Take work out of the plan for now (or pick it back up)
  read          Mark as read (kept in your own config only)
  link          One blocks another (or clear that block)
  issue         The verbs above, pinned to `--type issue`
  epic          The verbs above, pinned to `--type epic`
  milestone     The verbs above, pinned to `--type milestone`
  idea          Jot a passing thought down where you are (`--type idea`)
  tui           Open the explorer (the one write to an issue is `SPC n`, jot)
  hook          Called by Claude's hook. Reads an event on stdin
  merge-driver  Called by git. Merges issues.jsonl per issue, three-way
  skill         Install the skills and hooks into Claude (safe to run again)
  project       Register a directory to watch several projects from one moai
  init          Put a .moai/ into this repository (safe to run again)
  help          Print this message or the help of the given subcommand(s)

Options:
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help
  -V, --version              Print version

Start a session like this:

  moai status                   board, warnings, flow. Also the bare call
  moai ready                    what you can pick up now
  moai show <id>                body and history - why it was decided so
  moai mv <id> in_progress      pick it up.  done when finished
  moai note <id> 'what I found' a note for whoever comes next
  moai tui                      explorer - epics open like directories.
                                SPC n jots a thought down

When something not for now comes to mind:

  moai idea add 'a passing thought'  jot it. A title is enough - not work yet
  moai idea promote <id> --from -    unfold it into an epic and issues later

When you are not doing an existing piece of work right now:

  moai defer <id> -m 'next quarter'  out of the plan for a while
  moai defer <id> --undo             pick it back up

To watch several projects from one place:

  moai project add <dir>        registered, moai/status/ready outside a
                                `.moai` show every project at a glance
  moai -C <dir> <command>       other commands need to be told which one

To change the language of the screen:

  MOAI_LANG=ko moai status      English by default. en, ko, zh, ja, es
                                To keep it, put lang = "ko" under [i18n]
                                in your user config

To lay out a whole plan at once:

moai add --from - <<'PLAN'
# Epic title
- [p1] first issue #bug
PLAN

There is no approval gate. Create anything, move anything. In exchange
`moai status` shows issues with no epic, reviews stalled for days, and how
much you have opened at once.

`moai <command> --help` tells you all of that command. If the repository has
an AGENTS.md, how to work in that repository is written there.
```

## `moai status`

```
Board, warnings, flow. A session starts here

Usage: moai status [OPTIONS]

Options:
      --worktree             Also overlay other worktrees (no file changes)
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

  It blocks nothing. No approval, no gate.
  Instead it surfaces issues with no epic, reviews stalled for days, and how
  much you have opened at once.
  The exit code is non-zero only when the data itself is broken.

  --worktree also overlays the issues of other git worktrees. For one id the
  row that moved column, or was deferred and picked back up, later wins (on a
  tie, the later updated_at), and a row from another branch gets a leading
  branch mark in front of its title.
  A row removed here does not come back unless the other side touched it
  after the fork.
  When a sibling worktree cannot be read the board says so and the exit code
  stays the same.
  Overlaying is for showing only - no file changes.

  What to nag about first comes from .moai/config.toml. Unwritten, these are
  the values, and numbers go without quotes. No value ever blocks - a lower
  one only shows more.
    status_review_days   = 3      days in review before it counts as rot
    status_wip_days      = 2      days untouched after pickup before forgotten
    status_blocked_days  = 3      days blocked before it counts as stuck
    status_wip_limit     = 3      more than this opened at once
    status_no_epic_ratio = 0.15   issues with no epic from this ratio up
    status_no_epic_min   = 5      from this count up, even at a low ratio
    status_flow_days     = 7      the window the flow is measured over
    status_idea_pile     = 5      when this many thoughts have piled up
```

## `moai ready`

```
What you can pick up now

Usage: moai ready [OPTIONS]

Options:
      --worktree             Also overlay other worktrees (no file changes)
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

  Epics themselves, deferred rows and what is under them, and parents with
  unfinished children are left out.
  Urgent first, then epics near the end, then the oldest.

  With --worktree, work already picked up in another worktree drops out here
  and what you hold shows with its branch. For one id the row that moved
  column, or was deferred and picked back up, later wins - so editing only
  the title or priority here does not free what the other side picked up, and
  work the other side deferred later is not offered here.
```

## `moai prime`

```
A short markdown page - what you hold and what comes next

Usage: moai prime [OPTIONS]

Options:
      --worktree             Also overlay other worktrees (no file changes)
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

  Made for a session's first read and for the context injected again
  after a compact. The board is for a person: it draws warnings, the flow and
  the group bars, and that is far more than the two questions asked there -
  what was I holding, and what comes next.

  Markdown, never coloured, and the exit code is always 0. If it ever spoke
  with a non-zero code, a session that wires it into a start-up hook would
  open on a failure - and then this is a lint, and a lint is a gate.

  Wire it where your editor injects context at session start. For Claude Code
  that is a SessionStart hook, which fires again after a compact:

    moai prime

  With --worktree, work picked up in a sibling worktree shows with its branch
  and drops out of what is next - the same overlay `moai ready` uses.
```

## `moai add`

```
Create an issue

Usage: moai add [OPTIONS] [title]

Arguments:
  [title]
          One line. Wrap it in quotes. It may start with `--`

Options:
  -e, --epic <id>
          Put it in this epic

      --milestone <id>
          Put it in this milestone

  -t, --tag <tag>
          Join with commas or give it several times

  -p, --priority <0-3>
          0 is the highest

  -s, --status <status>
          The column it first stands in. The first column when absent

  -b, --body <text>
          Body. `-` reads it from stdin

  -a, --assignee <who|none>
          Assignee. The creator when absent; `none` clears it

      --type <issue|epic|milestone|idea>
          What kind to create (issue when absent, epic under `epic add`)

      --parent <id>
          Create it as a child of this issue (the id gets a `.xxx`)

      --from <file|->
          Epic and issues from markdown at once. `-` is stdin

      --dry-run
          Create nothing; only say what would be created (with `--from`)
          
          **It means nothing without `--from`.** It once took it alone, and
          then `moai add 'title' --dry-run` printed a line saying it was a
          rehearsal and then actually created it - a command called to hold
          back that writes instead is the worst kind.

      --var <name=value>
          Fill `{{name}}` in a plan template (with `--from`, several times)
          
          **Every variable is required** - an unfilled name, an empty value, a
          value with a line break, a name not in the plan, or the same name
          twice is refused and nothing is created. Names are letters, digits,
          `_` and `-`, and values are always title text, so variables belong
          in the title only. Like `--dry-run`, giving it without `--from` is
          refused.

  -q, --quiet
          Print the id only (for scripts)

      --json
          Machine-readable output. Every human line goes away

      --no-color
          Turn colour off (same as `--color never`)

      --color <how>
          auto|always|never (auto by default, off when piped)

  -C, --dir <path>
          Run in this directory (same as `git -C`)

      --user <name (email)>
          Who is doing this (from `git config` when absent)

  -h, --help
          Print help (see a summary with '-h')

Examples:
  moai add 'the parser dies on a BOM' -t bug -p 1
  moai add 'storage layer' --type epic
  moai add 'work under a parent' --parent moai-4aex
  moai add 'body from stdin' -b -
  moai add 'to someone else' -a 'Kim (kim@example.com)'   else the creator
  moai add 'with no owner' -a none

Title and body go separately. The title is one line saying what is wrong -
the board, `ready` and the explorer list show the title only. Do not push a
long text into the title; split it into the body. The body is markdown and
`-b -` reads it from stdin:

moai add 'the parser dies on a BOM' -t bug -b - <<'BODY'
- what is wrong: it reads the three leading bytes as the title
- where to fix: the read in src/store.rs
BODY

Several at once (`--from`):

moai add --from - <<'PLAN'
# Storage layer
- [p1] write atomically #bug
- recover a truncated line
- \[WIP] issue \#12
PLAN

  A `#` line is an epic, a `-` line is an issue of the epic above it.
  [pN] and #tag are optional.
  Put \ in front of a leading [ or a trailing #word in a title.
  --dry-run keeps a heredoc typo from creating six wrong issues.

Plan templates (`{{name}}` filled by --var, every variable required):
  moai add --from .moai/templates/release.md --var version=1.2

A title may start with `--`. Anything that is not a known flag is a title.
```

## `moai show`

```
Open one, or list them

Usage: moai show [OPTIONS] [target]

Arguments:
  [target]  An issue id, or a kind (issue, epic). The whole list when absent

Options:
      --raw                  Print the body as it is in the file, not rendered
      --tree                 Fold it as epic, issue, child
      --as-plan              Print an epic back as `add --from` markdown
      --worktree             Also overlay other worktrees (no file changes)
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

Filters  (comma = or,  repeated = and):
  -s, --status <status>                   In that column
  -t, --tag <tag>                         Carrying that tag
      --no-tag <tag>                      Not carrying that tag
  -e, --epic <id|none>                    In that epic (`none` = no epic)
      --milestone <id|none>               In that milestone (`none` = none)
      --parent <id|none>                  A child of that issue (`none` = top)
  -p, --priority <0-3>                    
  -a, --assignee <who|none|me>            That assignee (`none` and `me` too)
      --type <issue|epic|milestone|idea>  
  -g, --grep <text>                       In id, title, tag or body
      --stale <days>                      Sitting in its column that long
      --deferred                          Only what is deferred
      --all                               Include done and what is deferred
      --filter <item=value>               Filters as one string (`status=todo`)
```

## `moai mv`

```
Move the status

Usage: moai mv [OPTIONS] <id>...

Arguments:
  <id>...
          The issues to move, and the column to go to at the end

Options:
  -m, --msg <text>
          One line of note on this move (journal only)

      --from <column>
          Only while still in this column (racing pickups)
          
          Without it nothing is blocked, as before. With it, the column is
          looked at again inside the lock, and a row whose column changed in
          the meantime is left untouched and stands as a partial failure.

      --json
          Machine-readable output. Every human line goes away

      --no-color
          Turn colour off (same as `--color never`)

      --color <how>
          auto|always|never (auto by default, off when piped)

  -C, --dir <path>
          Run in this directory (same as `git -C`)

      --user <name (email)>
          Who is doing this (from `git config` when absent)

  -h, --help
          Print help (see a summary with '-h')

  The last argument is the column to go to, everything before it the issues.
  Column names and their order come from statuses in .moai/config.toml
  (todo, in_progress, review, done by default).

  Skipping ahead and going back are both allowed. This tool has no approval.
  `moai status` surfaces what was rewound and what has stalled.

  moai mv moai-4aex in_progress
  moai mv moai-4aex moai-9k2p done
  moai mv moai-4aex review -m 'tests moved to the next issue'

  When several people share one .moai, say which column you saw. `--from`
  looks again inside the lock and moves only if it is still that column - the
  loser gets one stderr line and a non-zero code. **Give one id** then:
  with several, the won and the lost rows share one exit code.

  moai mv moai-4aex in_progress --from todo
```

## `moai edit`

```
Edit title, body, tags, epic or priority

Usage: moai edit [OPTIONS] <id>

Arguments:
  <id>  

Options:
      --title <text>         One line. It may start with `--`
  -b, --body <text>          Body. `-` reads it from stdin
  -t, --tag <tag>            Add tags
      --untag <tag>          Remove tags
  -e, --epic <id|none>       Move the epic (`none` clears only its own field)
      --milestone <id|none>  Move the milestone (`none` clears its own field)
  -p, --priority <0-3>       
  -a, --assignee <who|none>  Give `name (email)`. `none` clears it
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

  `--epic none` and `--milestone none` clear only the field on that row.
  Membership inherited from a parent or an epic stays.

  Title and body go separately - `--title` is one line, `-b` is a markdown
  body and `-b -` reads it from stdin. **`-b` replaces the whole body.** To
  append one line, read the body first and join it:

{ moai show <id> --json | jq -r '.body // empty'
printf '\none more line\n'; } | moai edit <id> -b -
```

## `moai rm`

```
Remove

Usage: moai rm [OPTIONS] <id>...

Arguments:
  <id>...  

Options:
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help
```

## `moai note`

```
Leave a note on an issue (journal only)

Usage: moai note [OPTIONS] <id> [text]

Arguments:
  <id>
          

  [text]
          What the next person (or agent) should read. May start with `--`

Options:
  -b, --body <text>
          A long text. `-` reads it from stdin
          
          **It pushes the positional out.** Given both, nobody can remember
          which one wins, and a rule nobody remembers erases someone's text
          sooner or later.

      --json
          Machine-readable output. Every human line goes away

      --no-color
          Turn colour off (same as `--color never`)

      --color <how>
          auto|always|never (auto by default, off when piped)

  -C, --dir <path>
          Run in this directory (same as `git -C`)

      --user <name (email)>
          Who is doing this (from `git config` when absent)

  -h, --help
          Print help (see a summary with '-h')

  moai note moai-4aex 'the parser dies on a BOM'
  moai note moai-4aex -b - < review.txt        a long text from stdin

moai note moai-4aex -b - <<'NOTE'
a long text over several lines
NOTE

  A short finding goes as the positional, a long one - a whole review - with
  `-b -`. The two push each other out: given both, nobody can remember which
  one wins.

  Notes pile up in the journal only and never touch the snapshot.
  `moai show <id>` prints them as history.
```

## `moai defer`

```
Take work out of the plan for now (or pick it back up)

Usage: moai defer [OPTIONS] <id>...

Arguments:
  <id>...
          

Options:
      --undo
          Pick it back up

  -m, --msg <text>
          Why it is deferred (journal only)

      --from <column>
          Only while it is in this column (racing pickups)
          
          The same measure as `mv --from`. A row the other side picked up and
          started on is not taken out of the plan late. Without it nothing is
          blocked, as before.

      --json
          Machine-readable output. Every human line goes away

      --no-color
          Turn colour off (same as `--color never`)

      --color <how>
          auto|always|never (auto by default, off when piped)

  -C, --dir <path>
          Run in this directory (same as `git -C`)

      --user <name (email)>
          Who is doing this (from `git config` when absent)

  -h, --help
          Print help (see a summary with '-h')

  moai defer moai-4aex                          defer it
  moai defer moai-4aex moai-9k2p -m 'next quarter'   several, with a reason
  moai defer moai-4aex --undo                   pick it back up

  **Neither the column nor the kind changes.** Which column it was in is
  exactly what you need when picking it back up, and the same row has to come
  back. Deferred rows drop out of `moai ready`, the board and the warnings,
  and once they pile up `moai status` says so in one line. Defer an epic, a
  milestone or a parent and the work under it drops out too.

  `moai show --deferred` shows only what is deferred.

  `--from <column>` is the same measure as `mv --from` - a late defer does
  not take a row **whose column moved** out of the plan. Deferring does not
  change the column, so two racing defers are not told apart by it.

  moai defer moai-4aex -m 'next quarter' --from todo
```

## `moai read`

```
Mark as read (kept in your own config only)

Usage: moai read [OPTIONS] [id]...

Arguments:
  [id]...
          The issues to mark as read
          
          What was read is always named - called with no argument it would
          mark nothing while exiting as a success, and a person would move on
          believing it was all marked. `--all` and `-e` fill that place.

Options:
      --all
          Everything unread that came to me

  -e, --epic <epic>
          That group's members and what is under them

      --json
          Machine-readable output. Every human line goes away

      --no-color
          Turn colour off (same as `--color never`)

      --color <how>
          auto|always|never (auto by default, off when piped)

  -C, --dir <path>
          Run in this directory (same as `git -C`)

      --user <name (email)>
          Who is doing this (from `git config` when absent)

  -h, --help
          Print help (see a summary with '-h')

  moai read moai-4aex              this row as read
  moai read --all                  everything unread that came to me
  moai read -e moai-9k2p           that epic's members and what is under them

  **Nothing is written to the tracker.** Read marks differ per person, so
  writing them on the issue row would make even reading collide with others.
  Next to the config, <config dir>/read/ holds **one file per project** with
  the issue id and the updated_at of the row you saw - change that row after
  that and it becomes unread again. The file name is a hash of the repository
  root, and the path inside it says which root. The old [read] table in the
  config file is still read but never written again.

  Unread rows carry a [NEW] mark in front of the title in the explorer list -
  only what is assigned to me and what is under it (children, reviews, epic
  members) counts.
```

## `moai link`

```
One blocks another (or clear that block)

Usage: moai link [OPTIONS] <id>

Arguments:
  <id>  

Options:
      --blocks <id>          Block this issue (or issues)
      --unblocks <id>        Clear the block on this issue (or issues)
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

  moai link moai-4aex --blocks moai-9k2p     4aex blocks 9k2p
  moai link moai-4aex --unblocks moai-9k2p   clear that block

  `blocked_by` is written on the blocked side, not on the blocking one.
  A cycle (A blocks B while B already blocks A) is refused before any write.
```

## `moai issue`

```
The verbs above, pinned to `--type issue`

Usage: moai issue [OPTIONS] <COMMAND>

Commands:
  add   Create
  show  Open one, or list them (`ls` is the same)
  help  Print this message or the help of the given subcommand(s)

Options:
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help
```

## `moai issue add`

```
Create

Usage: moai issue add [OPTIONS] [title]

Arguments:
  [title]
          One line. Wrap it in quotes. It may start with `--`

Options:
  -e, --epic <id>
          Put it in this epic

      --milestone <id>
          Put it in this milestone

  -t, --tag <tag>
          Join with commas or give it several times

  -p, --priority <0-3>
          0 is the highest

  -s, --status <status>
          The column it first stands in. The first column when absent

  -b, --body <text>
          Body. `-` reads it from stdin

  -a, --assignee <who|none>
          Assignee. The creator when absent; `none` clears it

      --type <issue|epic|milestone|idea>
          What kind to create (issue when absent, epic under `epic add`)

      --parent <id>
          Create it as a child of this issue (the id gets a `.xxx`)

      --from <file|->
          Epic and issues from markdown at once. `-` is stdin

      --dry-run
          Create nothing; only say what would be created (with `--from`)
          
          **It means nothing without `--from`.** It once took it alone, and
          then `moai add 'title' --dry-run` printed a line saying it was a
          rehearsal and then actually created it - a command called to hold
          back that writes instead is the worst kind.

      --var <name=value>
          Fill `{{name}}` in a plan template (with `--from`, several times)
          
          **Every variable is required** - an unfilled name, an empty value, a
          value with a line break, a name not in the plan, or the same name
          twice is refused and nothing is created. Names are letters, digits,
          `_` and `-`, and values are always title text, so variables belong
          in the title only. Like `--dry-run`, giving it without `--from` is
          refused.

  -q, --quiet
          Print the id only (for scripts)

      --json
          Machine-readable output. Every human line goes away

      --no-color
          Turn colour off (same as `--color never`)

      --color <how>
          auto|always|never (auto by default, off when piped)

  -C, --dir <path>
          Run in this directory (same as `git -C`)

      --user <name (email)>
          Who is doing this (from `git config` when absent)

  -h, --help
          Print help (see a summary with '-h')

  Title and body go separately - the title is one line saying what is wrong,
  and a long text is a markdown body poured in from stdin with `-b -`.
  For examples see `moai add --help`.
```

## `moai issue show`

```
Open one, or list them (`ls` is the same)

Usage: moai issue show [OPTIONS] [target]

Arguments:
  [target]  An issue id, or a kind (issue, epic). The whole list when absent

Options:
      --raw                  Print the body as it is in the file, not rendered
      --tree                 Fold it as epic, issue, child
      --as-plan              Print an epic back as `add --from` markdown
      --worktree             Also overlay other worktrees (no file changes)
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

Filters  (comma = or,  repeated = and):
  -s, --status <status>                   In that column
  -t, --tag <tag>                         Carrying that tag
      --no-tag <tag>                      Not carrying that tag
  -e, --epic <id|none>                    In that epic (`none` = no epic)
      --milestone <id|none>               In that milestone (`none` = none)
      --parent <id|none>                  A child of that issue (`none` = top)
  -p, --priority <0-3>                    
  -a, --assignee <who|none|me>            That assignee (`none` and `me` too)
      --type <issue|epic|milestone|idea>  
  -g, --grep <text>                       In id, title, tag or body
      --stale <days>                      Sitting in its column that long
      --deferred                          Only what is deferred
      --all                               Include done and what is deferred
      --filter <item=value>               Filters as one string (`status=todo`)
```

## `moai epic`

```
The verbs above, pinned to `--type epic`

Usage: moai epic [OPTIONS] <COMMAND>

Commands:
  add   Create
  show  Open one, or list them (`ls` is the same)
  help  Print this message or the help of the given subcommand(s)

Options:
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help
```

## `moai epic add`

```
Create

Usage: moai epic add [OPTIONS] [title]

Arguments:
  [title]
          One line. Wrap it in quotes. It may start with `--`

Options:
  -e, --epic <id>
          Put it in this epic

      --milestone <id>
          Put it in this milestone

  -t, --tag <tag>
          Join with commas or give it several times

  -p, --priority <0-3>
          0 is the highest

  -s, --status <status>
          The column it first stands in. The first column when absent

  -b, --body <text>
          Body. `-` reads it from stdin

  -a, --assignee <who|none>
          Assignee. The creator when absent; `none` clears it

      --type <issue|epic|milestone|idea>
          What kind to create (issue when absent, epic under `epic add`)

      --parent <id>
          Create it as a child of this issue (the id gets a `.xxx`)

      --from <file|->
          Epic and issues from markdown at once. `-` is stdin

      --dry-run
          Create nothing; only say what would be created (with `--from`)
          
          **It means nothing without `--from`.** It once took it alone, and
          then `moai add 'title' --dry-run` printed a line saying it was a
          rehearsal and then actually created it - a command called to hold
          back that writes instead is the worst kind.

      --var <name=value>
          Fill `{{name}}` in a plan template (with `--from`, several times)
          
          **Every variable is required** - an unfilled name, an empty value, a
          value with a line break, a name not in the plan, or the same name
          twice is refused and nothing is created. Names are letters, digits,
          `_` and `-`, and values are always title text, so variables belong
          in the title only. Like `--dry-run`, giving it without `--from` is
          refused.

  -q, --quiet
          Print the id only (for scripts)

      --json
          Machine-readable output. Every human line goes away

      --no-color
          Turn colour off (same as `--color never`)

      --color <how>
          auto|always|never (auto by default, off when piped)

  -C, --dir <path>
          Run in this directory (same as `git -C`)

      --user <name (email)>
          Who is doing this (from `git config` when absent)

  -h, --help
          Print help (see a summary with '-h')

  Title and body go separately - the title is one line saying what is wrong,
  and a long text is a markdown body poured in from stdin with `-b -`.
  For examples see `moai add --help`.
```

## `moai epic show`

```
Open one, or list them (`ls` is the same)

Usage: moai epic show [OPTIONS] [target]

Arguments:
  [target]  An issue id, or a kind (issue, epic). The whole list when absent

Options:
      --raw                  Print the body as it is in the file, not rendered
      --tree                 Fold it as epic, issue, child
      --as-plan              Print an epic back as `add --from` markdown
      --worktree             Also overlay other worktrees (no file changes)
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

Filters  (comma = or,  repeated = and):
  -s, --status <status>                   In that column
  -t, --tag <tag>                         Carrying that tag
      --no-tag <tag>                      Not carrying that tag
  -e, --epic <id|none>                    In that epic (`none` = no epic)
      --milestone <id|none>               In that milestone (`none` = none)
      --parent <id|none>                  A child of that issue (`none` = top)
  -p, --priority <0-3>                    
  -a, --assignee <who|none|me>            That assignee (`none` and `me` too)
      --type <issue|epic|milestone|idea>  
  -g, --grep <text>                       In id, title, tag or body
      --stale <days>                      Sitting in its column that long
      --deferred                          Only what is deferred
      --all                               Include done and what is deferred
      --filter <item=value>               Filters as one string (`status=todo`)
```

## `moai milestone`

```
The verbs above, pinned to `--type milestone`

Usage: moai milestone [OPTIONS] <COMMAND>

Commands:
  add   Create
  show  Open one, or list them (`ls` is the same)
  help  Print this message or the help of the given subcommand(s)

Options:
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help
```

## `moai milestone add`

```
Create

Usage: moai milestone add [OPTIONS] [title]

Arguments:
  [title]
          One line. Wrap it in quotes. It may start with `--`

Options:
  -e, --epic <id>
          Put it in this epic

      --milestone <id>
          Put it in this milestone

  -t, --tag <tag>
          Join with commas or give it several times

  -p, --priority <0-3>
          0 is the highest

  -s, --status <status>
          The column it first stands in. The first column when absent

  -b, --body <text>
          Body. `-` reads it from stdin

  -a, --assignee <who|none>
          Assignee. The creator when absent; `none` clears it

      --type <issue|epic|milestone|idea>
          What kind to create (issue when absent, epic under `epic add`)

      --parent <id>
          Create it as a child of this issue (the id gets a `.xxx`)

      --from <file|->
          Epic and issues from markdown at once. `-` is stdin

      --dry-run
          Create nothing; only say what would be created (with `--from`)
          
          **It means nothing without `--from`.** It once took it alone, and
          then `moai add 'title' --dry-run` printed a line saying it was a
          rehearsal and then actually created it - a command called to hold
          back that writes instead is the worst kind.

      --var <name=value>
          Fill `{{name}}` in a plan template (with `--from`, several times)
          
          **Every variable is required** - an unfilled name, an empty value, a
          value with a line break, a name not in the plan, or the same name
          twice is refused and nothing is created. Names are letters, digits,
          `_` and `-`, and values are always title text, so variables belong
          in the title only. Like `--dry-run`, giving it without `--from` is
          refused.

  -q, --quiet
          Print the id only (for scripts)

      --json
          Machine-readable output. Every human line goes away

      --no-color
          Turn colour off (same as `--color never`)

      --color <how>
          auto|always|never (auto by default, off when piped)

  -C, --dir <path>
          Run in this directory (same as `git -C`)

      --user <name (email)>
          Who is doing this (from `git config` when absent)

  -h, --help
          Print help (see a summary with '-h')

  Title and body go separately - the title is one line saying what is wrong,
  and a long text is a markdown body poured in from stdin with `-b -`.
  For examples see `moai add --help`.
```

## `moai milestone show`

```
Open one, or list them (`ls` is the same)

Usage: moai milestone show [OPTIONS] [target]

Arguments:
  [target]  An issue id, or a kind (issue, epic). The whole list when absent

Options:
      --raw                  Print the body as it is in the file, not rendered
      --tree                 Fold it as epic, issue, child
      --as-plan              Print an epic back as `add --from` markdown
      --worktree             Also overlay other worktrees (no file changes)
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

Filters  (comma = or,  repeated = and):
  -s, --status <status>                   In that column
  -t, --tag <tag>                         Carrying that tag
      --no-tag <tag>                      Not carrying that tag
  -e, --epic <id|none>                    In that epic (`none` = no epic)
      --milestone <id|none>               In that milestone (`none` = none)
      --parent <id|none>                  A child of that issue (`none` = top)
  -p, --priority <0-3>                    
  -a, --assignee <who|none|me>            That assignee (`none` and `me` too)
      --type <issue|epic|milestone|idea>  
  -g, --grep <text>                       In id, title, tag or body
      --stale <days>                      Sitting in its column that long
      --deferred                          Only what is deferred
      --all                               Include done and what is deferred
      --filter <item=value>               Filters as one string (`status=todo`)
```

## `moai idea`

```
Jot a passing thought down where you are (`--type idea`)

Usage: moai idea [OPTIONS] <COMMAND>

Commands:
  add      Create
  show     Open one, or list them (`ls` is the same)
  promote  Unfold into one epic and several issues, and close that thought
  help     Print this message or the help of the given subcommand(s)

Options:
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

  One step below todo. **Jotting has to cost nearly nothing** - neither a
  priority nor an epic is asked for. Title and body still go separately: keep
  the title to one short line and pour a long thought into the body with
  `-b -`. That title becomes the issue title when it is unfolded, so a long
  one here carries straight over.

  moai idea add 'a passing thought'   jot it
  moai idea ls                        what has piled up (same as `idea show`)

moai idea add 'install the merge driver by hand in every clone' -b - <<'IDEA'
Today `moai merge-driver --install` has to be typed once per clone.
IDEA

  An idea is not work - it is in neither `moai ready` nor the board's counts,
  and living without an epic is normal for it, so it never trips the
  "issues with no epic" warning.

  Editing and dropping are the verbs you already have: `moai edit <id>`,
  `moai rm <id>`.
```

## `moai idea add`

```
Create

Usage: moai idea add [OPTIONS] [title]

Arguments:
  [title]
          One line. Wrap it in quotes. It may start with `--`

Options:
  -e, --epic <id>
          Put it in this epic

      --milestone <id>
          Put it in this milestone

  -t, --tag <tag>
          Join with commas or give it several times

  -p, --priority <0-3>
          0 is the highest

  -s, --status <status>
          The column it first stands in. The first column when absent

  -b, --body <text>
          Body. `-` reads it from stdin

  -a, --assignee <who|none>
          Assignee. The creator when absent; `none` clears it

      --type <issue|epic|milestone|idea>
          What kind to create (issue when absent, epic under `epic add`)

      --parent <id>
          Create it as a child of this issue (the id gets a `.xxx`)

      --from <file|->
          Epic and issues from markdown at once. `-` is stdin

      --dry-run
          Create nothing; only say what would be created (with `--from`)
          
          **It means nothing without `--from`.** It once took it alone, and
          then `moai add 'title' --dry-run` printed a line saying it was a
          rehearsal and then actually created it - a command called to hold
          back that writes instead is the worst kind.

      --var <name=value>
          Fill `{{name}}` in a plan template (with `--from`, several times)
          
          **Every variable is required** - an unfilled name, an empty value, a
          value with a line break, a name not in the plan, or the same name
          twice is refused and nothing is created. Names are letters, digits,
          `_` and `-`, and values are always title text, so variables belong
          in the title only. Like `--dry-run`, giving it without `--from` is
          refused.

  -q, --quiet
          Print the id only (for scripts)

      --json
          Machine-readable output. Every human line goes away

      --no-color
          Turn colour off (same as `--color never`)

      --color <how>
          auto|always|never (auto by default, off when piped)

  -C, --dir <path>
          Run in this directory (same as `git -C`)

      --user <name (email)>
          Who is doing this (from `git config` when absent)

  -h, --help
          Print help (see a summary with '-h')

  Title and body go separately - the title is one line saying what is wrong,
  and a long text is a markdown body poured in from stdin with `-b -`.
  For examples see `moai add --help`.
```

## `moai idea show`

```
Open one, or list them (`ls` is the same)

Usage: moai idea show [OPTIONS] [target]

Arguments:
  [target]  An issue id, or a kind (issue, epic). The whole list when absent

Options:
      --raw                  Print the body as it is in the file, not rendered
      --tree                 Fold it as epic, issue, child
      --as-plan              Print an epic back as `add --from` markdown
      --worktree             Also overlay other worktrees (no file changes)
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

Filters  (comma = or,  repeated = and):
  -s, --status <status>                   In that column
  -t, --tag <tag>                         Carrying that tag
      --no-tag <tag>                      Not carrying that tag
  -e, --epic <id|none>                    In that epic (`none` = no epic)
      --milestone <id|none>               In that milestone (`none` = none)
      --parent <id|none>                  A child of that issue (`none` = top)
  -p, --priority <0-3>                    
  -a, --assignee <who|none|me>            That assignee (`none` and `me` too)
      --type <issue|epic|milestone|idea>  
  -g, --grep <text>                       In id, title, tag or body
      --stale <days>                      Sitting in its column that long
      --deferred                          Only what is deferred
      --all                               Include done and what is deferred
      --filter <item=value>               Filters as one string (`status=todo`)
```

## `moai idea promote`

```
Unfold into one epic and several issues, and close that thought

Usage: moai idea promote [OPTIONS] --from <file|-> <id>

Arguments:
  <id>  The idea to unfold

Options:
      --from <file|->        Epic and issues from markdown. `-` is stdin
  -e, --epic <epic>          Unfold as members of this standing epic
      --var <name=value>     Fill `{{name}}` in the template (repeatable)
      --dry-run              Create nothing; only say what would be created
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

  The markdown it takes is the same shape as `add --from`. With two shapes,
  you get the grammar wrong every single time.

moai idea promote <id> --from - <<'PLAN'
# Epic title
- [p1] first issue #enhancement
- [p2] second issue
PLAN

  **A line in the plan becomes the issue title as it is.** A long idea title
  carries its length over to the issue, so write a short title again when
  unfolding. The original text stays on that idea and the history leads back.

  Unfolding closes it - that idea goes to `done`. What came from what is kept
  in the journal (the history in `moai show <id>`).

  With the epic already standing, `-e <epic>` unfolds into it as members.
  That is where you take back something the epic needs that had gone out as
  an idea - the plan then holds `- issue` lines only.

moai idea promote <id> -e <epic> --from - <<'PLAN'
- [p1] what the epic set out to do
PLAN

  `--dry-run` is where a person looks at the unfolded plan once and says yes.
```

## `moai tui`

```
Open the explorer (the one write to an issue is `SPC n`, jot)

Usage: moai tui [OPTIONS]

Options:
      --path <id|basket>     Open here - a directory opens inside it
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

  Milestones and epics behave like directories. Move around on the left and
  what the cursor rests on is described on the right.

  j and k or the arrow keys move, Enter goes in, Backspace comes back out.
  On a milestone or epic row, l and the right arrow unfold one step there and
  h and the left arrow fold it. On an unfolded member row, h folds its parent
  and lands on the parent; with nothing to fold it leaves one level. On a
  project header row, l and the right arrow unfold and h and the left arrow
  fold. Tab unfolds everything under it recursively and folds it again on a
  second press. Unfolded members stand with branch marks in the title column,
  and that unfolding is not kept in the config. gg and Home go to the top,
  G and End to the bottom, Ctrl-d and Ctrl-u half a page, Ctrl-f and Ctrl-b
  (PageDown and PageUp) a whole page.
  Ctrl-w w moves the focus between the list and the detail (Ctrl-w W goes the
  other way, Ctrl-w h and Ctrl-w l pick the left and right pane), and every
  movement key moves the focused pane — to scroll the detail, go there with
  Ctrl-w w.
  / searches, Esc clears the filter you set. r marks the row under the cursor
  as read (see [NEW] below).
  The search and filter fields take Enter to apply and Esc to give up, the
  search filters the list as you type, and Tab and Shift-Tab pick where it
  looks: everything, id, title, tag or body. The header at the top
  numbers every registered project, and pressing that number without SPC
  jumps straight there — 0 is everything, one list of all projects.

  The rest lives in the menu that opens the moment you press SPC. The menu
  stands up only what works where you are, ignores keys it does not know,
  closes on Esc or SPC and goes one level up on Backspace. Toggles and sorts
  (SPC v, SPC c, SPC s) do not close it — try them, watch the state, and
  leave with Esc. That level says so at the bottom right with a close hint.
    SPC /    search              SPC f    filter             SPC n    jot
    SPC q    quit
    SPC p a  register            SPC p d  drop from the list
  View — every toggle except the list columns (SPC c) is here:
    SPC v d  done [shown/hidden] SPC v l  deferred           SPC v a  show all
    SPC v 1  first column of the config [shown/hidden] — the next ones count up
    SPC v p  detail pane [shown/hidden]
    SPC v w  overlay worktrees [on/off]
    SPC v r  raw or rendered
  Sorts and columns call priority, created, updated and assignee by the same
  letter — only title (SPC s t) and tag (SPC c t) split one letter:
    SPC s p  priority            SPC s c  created            SPC s u  updated
    SPC s s  column              SPC s a  assignee           SPC s t  title
    SPC c i  id                  SPC c p  priority           SPC c a  assignee
    SPC c c  created             SPC c u  updated            SPC c n  counts
    SPC c t  tag                 SPC c h  column names [shown/hidden]
    SPC c w  branch mark [shown/hidden] — needs SPC v w to overlay first
  Read:
    SPC m a  everything unread   SPC m g  every member of this group
  The one key that quits outright is Ctrl-C — anywhere, even mid-typing.
  The screen rereads itself — issues written next door, and `moai read` or
  `moai project add` in another terminal, land without a keypress.

  Of what came to me (assigned to me or under it), rows changed since the
  last look carry a [NEW] mark in front of the title. Read marks live in my own
  config and the tracker does not change — on the CLI that is `moai read`.

  The list hides done to begin with — the [done hidden] mark on the path line
  says so. The view is separate from the filter, so Esc does not clear it and
  the two apply together.
  Sorting puts urgent, new, earlier column and alphabetical on top, and
  pressing the chosen one again turns it around. When it is not the default
  (priority) the path line says which order it is.
  Columns (SPC c) turn on and off with [shown/hidden]. Assignee, tag, created
  and updated dates stand on the right of the row, and when it gets narrow
  they are dropped in that order — dates, then assignee, then tag — to leave
  room for the title.
  View, sort and columns are written into the [tui] table of the user config
  on every press and carry over to the next run and to other projects (the
  same file `moai project add` writes).

  With registered projects (`moai project add`), 0 lists them all — a header
  row per project with that project's rows under it. Started outside a
  `.moai` it begins there; started inside one it begins in that project.
  Coming out through 0 leaves the project you were in unfolded and the rest
  as header rows only — a project is read the moment it unfolds (its header
  spins while it reads).
  Enter on a header row goes into that project, Backspace only goes up a
  directory. Enter on a group row under it goes into that project and lands
  there — you never drill down in the one list; drilling down always happens
  inside a project.
  View, sort and columns apply to every unfolded project while search and
  filter apply inside one project only — jotting (SPC n) and read (r) go to
  the project of the row under the cursor. With nothing registered and
  started outside, an empty list stands and says to add the first project
  with SPC p a (`--json` gives an empty `projects` array and exits 0).

  SPC p a opens a window to pick a directory and register it as a project —
  it walks in and out one level at a time (Enter and Backspace, moving with
  the same j, k, gg and G as the list), and directories with a `.moai` and those
  already registered are marked. Inside the window, a registers the directory
  under the cursor, `.` shows or hides dotted directories, and g p opens a
  field to type a path (Enter goes, Esc gives up). The window closes on Esc.
  Any subdirectory of a monorepo stands on its own exactly as picked. It is
  taken even without a `.moai`. The window opens inside a project too, so the
  first project can be added with nothing registered.
  On a header row, SPC p d asks once and then only drops it from the list —
  y is yes and any other key gives up. The directory and its `.moai` stay.
  It writes where `moai project add|rm` writes.

  SPC n opens the jot form anywhere inside a project — it is kept as an idea
  (with no epic). If an editor is there ($VISUAL, $EDITOR, or vi or nano on
  PATH) it opens like a git commit message: the first line is the title, then
  a blank line, then the body, and comment lines are guidance to be deleted.
  Leave the title empty, or end the editor with an error, and nothing is
  kept. With no editor the built-in form opens — one title line and a body,
  Tab moves between them (Enter in the title goes to the body) and Ctrl-S
  keeps it. Esc closes it, asking once if you had typed something (y throws
  it away). This is the one write to an issue — editing is done on the CLI.
  `--json` prints only that directory's listing, with no screen (outside a
  `.moai`, the rows of the layer).

  `--path` takes an issue id, or one of the two baskets by the word this
  tool uses for them: none (no milestone) and lost.
```

## `moai hook`

```
Called by Claude's hook. Reads an event on stdin

Usage: moai hook [OPTIONS] <event>

Arguments:
  <event>  Which place it was called from (see the list below)

Options:
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

  Nobody calls this by hand. The plugin installed into Claude calls it.

  **It blocks nothing, and whatever goes wrong the exit code is 0.** A hook
  that spits errors makes every session start noisy, and then people turn the
  hook off - a rule that is off is no rule.

  The directory (cwd) and the session id come from stdin. They are not in the
  environment.

  Events:
    session-start       Writes the baseline. Loads what is held after a compact
    user-prompt-submit  A person asked. Loads the board once per session
    pre-tool-use        Just before a tool call. The rules stand here
    stop                The turn ends. Checks the state against reality

  echo '{"session_id":"x","cwd":"/repo"}' | moai hook user-prompt-submit
```

## `moai merge-driver`

```
Called by git. Merges issues.jsonl per issue, three-way

Usage: moai merge-driver [OPTIONS] [base] [ours] [theirs] [marker] [path]

Arguments:
  [base]    `%O` - the file at the fork
  [ours]    `%A` - this side's file. **The answer is written here too**
  [theirs]  `%B` - the other side's file
  [marker]  `%L` - the length of the conflict markers (7 by default)
  [path]    `%P` - the name of the file being merged. Only used when speaking

Options:
      --install              Install the driver into this repo's `.git/config`
      --as <command>         The command to install with (default: this binary)
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

  The only thing anyone types by hand is `--install`. Git gives the rest.

  moai merge-driver --install        install into this repository's .git/config
  moai merge-driver --install --as <path>  install with that command

  **When the installed path disappears, git falls back to its own merge.**
  Git reads a driver that did not run as a conflict while leaving this side's
  file as it is, and with no markers in it whoever runs `git add` throws the
  other side away wholesale. So the installed line makes the driver write
  neither an answer nor markers and merges the failed round again with
  `git merge-file` - markers stand, and the worst case equals an uninstalled
  clone. Even so the path is worth keeping: a fallen-back round loses the
  value of resolving per issue. The default is the absolute path of the
  binary running now, and pointing that at a worktree `target/` kills it with
  that worktree - `--local` is shared by the clone, so every checkout lands in
  that state at once. `--as` also takes a word on PATH, but in this
  repository do not pass a bare `moai`: that is the old moai binary and does
  not know this command.

  One line is one issue and they are sorted by id, so two branches that fixed
  different issues collide just because those lines are neighbours. Here they
  are paired by id and merged three-way per issue. Different fields of the
  same issue are merged too - tags and blocks add what was added and remove
  what was removed.

  **When the same field was changed differently, a person resolves it.**
  Picking one side silently makes the other side's edit vanish without a
  trace. With an unreadable line or a duplicated id the whole file is handed
  over inside conflict markers - keeping only what parsed would lose the rest.

  Installing is once per clone. Git reads the driver command from the config
  only, and the config is not committed. In a clone without it, merge=moai in
  `.gitattributes` is simply ignored and git's own merge runs - merging is
  exactly as it was without it. When that repository does set merge=moai,
  `moai status` says in one line that it is not installed here.
```

## `moai skill`

```
Install the skills and hooks into Claude (safe to run again)

Usage: moai skill [OPTIONS] <COMMAND>

Commands:
  install    Install the plugin tree and register it with `claude`
  status     What is installed at which scope, and where it differs
  uninstall  Remove the registration from `claude`. Installed files stay
  help       Print this message or the help of the given subcommand(s)

Options:
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help
```

## `moai skill install`

```
Install the plugin tree and register it with `claude`

Usage: moai skill install [OPTIONS]

Options:
      --scope <scope>        Where to register: local (default), project, user
      --dry-run              Install nothing; only say what would be installed
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

  Skills and hooks are installed into `.claude/moai-plugin/` and registered
  with `claude`. Your settings.json is not touched - putting the two keys in
  is `claude`'s job.

  **Nothing is deleted.** Running again only overwrites. Deleting a hook file
  a running session holds would block every tool call of that session.

  The version is a hash of what is installed. Same content, same version, so
  there are no empty updates.

  The two plugins that polish Korean text are installed **at the same scope**
  (korean-skills and humanize-korean). If they cannot be installed, moai's
  own registration still stands, and they are removed along with it.

  moai skill install                  just me (the default. settings.local.json)
  moai skill install --scope user     every repository on this machine
  moai skill install --scope project  with the team (committed settings.json)
  moai skill install --dry-run        only show what would be installed

  A Claude session already open keeps the old version - reopen it to pick
  this one up.
```

## `moai skill status`

```
What is installed at which scope, and where it differs

Usage: moai skill status [OPTIONS]

Options:
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

  It **only reads** `claude`'s registry (~/.claude/plugins/). Whatever is out
  of line the exit code is 0 - this is a command that shows, not one that
  blocks.

  What it looks at:
    marketplace   registered under this repository's name, not pointing
                  somewhere else
    install       which version at which scope, and whether it matches the
                  version that would be installed now
    companions    whether the two Korean text plugins are in this repository
    hook          whether the executable the install calls is still there
    claude        whether it is on PATH (without it nothing can be installed
                  or removed)
```

## `moai skill uninstall`

```
Remove the registration from `claude`. Installed files stay

Usage: moai skill uninstall [OPTIONS]

Options:
      --dry-run              Call nothing; only say what would be called
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

  This repository's install is removed per scope with
  `claude plugin uninstall`, and the marketplace with
  `claude plugin marketplace remove`. Deleting the two keys from your
  settings is `claude`'s job - we do not touch someone else's JSON.

  **`.claude/moai-plugin/` is not deleted.** Deleting a file a running
  session holds can block that session's tool calls. Delete it after closing
  the session.

  The two Korean plugins installed alongside are removed **at the scope moai
  was removed from**. The marketplace is left behind - the name is global to
  one machine and another repository's install uses it.
  A user-scope install is also left behind when another repository's moai
  stands there, and then the command to remove it is printed in one line.

  A Claude session already open keeps calling the old hook - reopen it for
  the removal to land.

  moai skill uninstall --dry-run      only show what would be called
```

## `moai project`

```
Register a directory to watch several projects from one moai

Usage: moai project [OPTIONS] <COMMAND>

Commands:
  add    Register a directory (already there, nothing changes)
  ls     List what is registered - name, path, whether it has a `.moai`
  rm     Drop from the list. The directory and its `.moai` stay
  color  Pick the colour that project wears at a glance and in the explorer
  help   Print this message or the help of the given subcommand(s)

Options:
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

Examples:
  moai project add ~/work/argos         register it. Taken without a .moai
  moai project add repo/apps/a          a monorepo registers subdirs one by one
  moai project ls                       what is registered and its state
  moai project rm ~/work/argos          drop from the list only. Directory stays
  moai project color ~/work/argos green pick a colour (auto picks by path)

  Once registered, `moai`, `moai status` and `moai ready` called outside a
  `.moai` show every registered project at a glance (`--json` gives a
  `projects` array). With `--worktree` each project overlays its sibling
  worktrees too. Other commands do not know which project, so call them as
  `moai -C <dir> <command>`.

  This is **your** config, not the repository's - call it anywhere outside a
  `.moai`. The place is MOAI_CONFIG, then $XDG_CONFIG_HOME/moai/config.toml,
  then ~/.config/moai/config.toml. A relative path is joined to where you are
  (the `-C` directory when you gave one) and symlinks are resolved.

  It does not ask who did it. This is not a file that keeps history.
```

## `moai project add`

```
Register a directory (already there, nothing changes)

Usage: moai project add [OPTIONS] <dir>

Arguments:
  <dir>  The directory to register. It must exist; a `.moai` need not

Options:
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

Examples:
  moai project add .                    the current directory
  moai project add ~/work/argos         taken without a .moai ("before init")

  Safe to run again - already registered, it says so and exits 0.
```

## `moai project ls`

```
List what is registered - name, path, whether it has a `.moai`

Usage: moai project ls [OPTIONS]

Options:
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

  The name is the directory name, and when two collide the segment above is
  joined to tell them apart (`apps/a`, `libs/a`).
  It always exits 0 - a broken config file is shown in one stderr line and it
  carries on (with `--json`, in a `problems` array instead of stderr).
```

## `moai project rm`

```
Drop from the list. The directory and its `.moai` stay

Usage: moai project rm [OPTIONS] <dir>

Arguments:
  <dir>  The directory to drop. Found by the written path even if it is gone

Options:
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help
```

## `moai project color`

```
Pick the colour that project wears at a glance and in the explorer

Usage: moai project color [OPTIONS] <dir> <colour>

Arguments:
  <dir>     A registered directory. Found by the written path even if it is gone
  <colour>  cyan, green, blue or auto

Options:
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

Examples:
  moai project color ~/work/argos green   green instead of the colour by path
  moai project color ~/work/argos auto    clear it and pick by path again

  The colours to pick from are cyan, green and blue only. Red, yellow and
  magenta already mean error, held work and review, so next to an id they
  would read as something they are not, and bright colours and grey vanish on
  one background or the other. The colour is a companion - the name always
  stands next to it. Use it when two projects land on the same colour.

  It is written as `color = "green"` under `[[project]]` in the user config.
  Writing it by hand is fine - a wrong value is shown in one line by
  `moai project ls`, which then uses the colour picked by path.
```

## `moai init`

```
Put a .moai/ into this repository (safe to run again)

Usage: moai init [OPTIONS] [PREFIX]

Arguments:
  [PREFIX]  id prefix (up to 8). Made from the directory name when absent

Options:
      --no-agents            Leave AGENTS.md alone
      --check                Write nothing; say if the AGENTS.md block is stale
      --print                Write nothing; print that block (to paste it)
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

  Run again where it is already installed and only the attached files
  (.gitattributes, .gitignore, AGENTS.md) are brought back in line. Issues
  and the journal are not touched.

  The prefix is decided once - every id already issued carries it.

  A new prefix is up to 8 characters - you type it with every id. A longer
  one is refused with shorter candidates. Without one it is made from the
  directory name: dropping hyphens if that fits (moa-issue becomes moaissue),
  else the initials of the hyphenated words (my-company-backend becomes mcb),
  and with a single word the first 8 characters. A repository already
  installed with a longer prefix is read and written as it is.

  --check writes nothing and only answers whether the AGENTS.md block is
  current, stale or missing. It is non-zero only when a file cannot be read.

  --print only prints that block. That is where to copy it from when the file
  the agent reads is not AGENTS.md - --print and init write the same text.
```
