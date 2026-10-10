# CLI reference

**Generated from `moai --help`. Do not edit by hand** — run
`scripts/gen-cli-docs.sh` after `cargo build --release`, and commit the result.
A test compares this file against the binary's own help, so a reference that
drifts fails the build rather than misleading a reader.

**A large diff here is normal.** Every command is re-rendered from the binary,
so one changed word in one help text rewrites this whole file. The file is the
binary's own text, not a copy anybody keeps by hand - read the code change, not
this diff.

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
  stats         Count them - spread, flow, lead and cycle time, AI work
  archive       Move eligible closed rows into yearly archive files
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
  backlog       Jot a passing thought down where you are (`--type backlog`)
  wiki          Read the project wiki - the markdown pages under `docs/`
  tui           Open the explorer (the one write to an issue is `SPC n`, jot)
  hook          Called by an agent's hook. Reads an event on stdin
  merge-driver  Called by git. Merges issues.jsonl per issue, three-way
  skill         Plant skills for Claude, Codex, Antigravity (safe to run again)
  project       Register a directory to watch several projects from one moai
  update        Upgrade the moai you are running with install.sh
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

  moai backlog add 'a passing thought'  jot it. A title is enough - not work yet
  moai backlog promote <id> --from -    unfold it into an epic and issues later

When you are not doing an existing piece of work right now:

  moai defer <id> -m 'next quarter'  out of the plan for a while
  moai defer <id> --undo             pick it back up

To read the project manual:

  moai wiki ls                  the markdown pages under docs/ - the wiki
  moai wiki show <slug>         one page

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
    status_backlog_pile  = 5      when this many thoughts have piled up
    status_due_days      = 3      days before a milestone deadline to say so
```

## `moai ready`

```
What you can pick up now

Usage: moai ready [OPTIONS]

Options:
      --worktree             Also overlay other worktrees (no file changes)
  -n, --limit <count>        Give at most this many rows (held stays whole)
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

  Epics themselves, deferred rows and what is under them, and parents with
  unfinished children are left out.
  Urgent first, then epics near the end, then the oldest.

  Only your own rows are offered. A row that is someone else's or nobody's
  stands apart below - ask before you pick it up (`moai mv <id> <column>
  --take` on a yes). --json carries it under others, always an array, with
  owner theirs or unowned. Who you are comes from --user, MOAI_ACTOR or git
  config; when it is unknown nothing is set apart.

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

  With no tracker here it says how to start one (--json: no_tracker). With a
  tracker it cannot read - a link out of the checkout, a broken config, a
  `.moai` that is not a directory - it says why instead and does not send you
  to `moai init` (--json: tracker_error, whose code is the one the other
  commands stop with). When that tracker is one it climbed to from a checkout
  with no `.moai`, it says so and names it (--json: climbed_to), and gives both
  ways out: fix that tracker, or start a separate one for this checkout with
  `moai init` - named with `-C <checkout top>` when you stand below the top
  (--json: init_at). When it climbed past a `.moai` it could not look at, it
  says that instead and does not send you to `moai init` (--json: unseen_at).

  Wire it where your editor injects context at session start. For Claude Code
  that is a SessionStart hook, which fires again after a compact:

    moai prime

  What you hold and what comes next are your own rows - the same split
  `moai ready` makes, with the rest under others.

  With --worktree, work picked up in a sibling worktree shows with its branch
  and drops out of what is next - the same overlay `moai ready` uses.
```

## `moai add`

```
Create an issue

Usage: moai add [OPTIONS] [title]

Arguments:
  [title]
          One line. Wrap it in quotes. A lone `-x` needs `--` in front

Options:
  -e, --epic <id>
          Put it in this epic

      --milestone <id>
          Put it in this milestone (with `--from`, on the epic it creates)

  -t, --tag <tag>
          Join with commas or give it several times

  -p, --priority <0-3>
          0 is the highest

  -s, --status <status>
          The column it first stands in. The first column when absent

  -b, --body <text>
          Text, not a path. `-` reads stdin (with `--from`, on the first epic)
          
          **A file goes in as `--body - < <file>`.** `--body <path>` takes
          the path as the body, it does not open the file.

  -a, --assignee <who|none>
          Assignee. The creator when absent; `none` clears it, `me` is you

      --start <date>
          Start of a milestone, `YYYY-MM-DD` (milestone rows only)

      --due <date>
          Deadline of a milestone, `YYYY-MM-DD` (milestone rows only)

      --type <issue|epic|milestone|backlog>
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

A title of several words may start with `-`. One word that opens with `-`
reads as a flag — put it after `--` (`moai add -- -x`).
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
      --removed              Removed issues, from the journal (see below)
      --worktree             Also overlay other worktrees (no file changes)
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

Filters  (comma = or,  repeated = and):
  -s, --status <status>                      In that column
  -t, --tag <tag>                            Carrying that tag
      --no-tag <tag>                         Not carrying that tag
  -e, --epic <id|none>                       In that epic (`none` = no epic)
      --milestone <id|none>                  In that milestone (`none` = none)
      --parent <id|none>                     Child of this issue (`none` = top)
  -p, --priority <0-3>                       
  -a, --assignee <who|none|me>               That assignee (repeated = or)
      --type <issue|epic|milestone|backlog>  
  -g, --grep <text>                          In id, title, tag, body or notes
      --stale <days>                         Sitting in its column that long
      --since <when>                         Changed since that time (see below)
      --created <from..to>                   Created in that range (see below)
      --done <from..to>                      Closed in that range (see below)
      --deferred                             Only what is deferred
      --all                                  Include done/deferred, no archive
      --archived                             Include the archive too (old done)
      --filter <item=value>                  One filter string (`status=todo`)

Order and paging:
      --sort <key>     Order the list by that key (priority when absent)
      --reverse        Turn the order around, ties included
  -n, --limit <count>  Give at most this many rows
      --after <id>     Start after this row: the last id of the page before

  Order: --sort priority (the default: urgent first, then id), created and
  updated (newest first), status (the column order of .moai/config.toml),
  assignee (the name the screen shows, unowned last), title (ignoring case),
  id. Ties under created and updated fall to id alone, and under every
  other order to priority, then id. --reverse turns the whole order around.

  Paging: -n cuts the list, and --after <id> starts the next page after the
  last id of the page before. The cursor is that row's value in the order,
  not a position, so rows created or removed meanwhile never shift a page.
  A row whose place in the order changes between pages - the cursor row or
  any other, a priority edit included - can repeat or be skipped; --sort id
  and --sort created are the orders no edit moves. Lines sharing one id
  (twins a merge left behind) stand together and a page never splits them,
  so such a page can run past -n. --json stays an array - fewer rows than
  -n means the list has ended.

    moai show --sort id -n 100 --json
    moai show --sort id -n 100 --after <last id> --json

  Archive: a row that has stood in done for archive_days (14 unless
  .moai/config.toml says otherwise, 0 turns it off) is the archive - an
  epic or milestone counted from when it got to done, the clock --done
  reads. --all and -s done leave it out and the tail says how many;
  --archived brings it back, done and deferred with it. Asking by time
  (--since, --created, --done, --stale) finds it anyway, and so does -g
  once done is let in (-g --all). Nothing is stored: it is read from the
  column and the clock each time.

    moai show --archived -g parser

  Time: --since <when> keeps the rows whose own updated_at is at or after
  it. --created and --done take a range from..to or from~to with either
  side left open, or a single time, which is all it spans. <when> is
  YYYY-MM-DD, a day on your own clock - the time zone the screen and
  milestone deadlines use; YYYY-MM-DD HH:MM, one minute on that clock;
  or YYYY-MM-DDTHH:MM:SSZ, an instant in UTC that no time zone moves. The
  end of a range takes all it spans too, so ~2026-10-05 23:59 runs to
  23:59:59. Quote a range that opens with ~ ('~2026-10-05'): zsh takes a
  bare ~2026-10-05 for a named directory. --done looks at rows standing in
  done now, at the time they last got there - an epic or milestone when its
  last member got to done, or when the rest were deferred if that came
  later. Moving rows in or out and removing them leave no trace on the
  group: closed that way it counts from its last finished member, and a
  deferred row moved into a closed group dates it from that row's deferral.
  Asking by time opens what the list hides by default - done, deferred and
  backlog items - because a row closed meanwhile changed too. Narrow with
  -s (name the columns you want) or --type; --deferred keeps only what is
  deferred, and no flag leaves deferred rows out. A lone instant given to
  --created or --done is that one second, not a day.

  --filter takes each filter as item=value, one per flag, and adds four
  time items named after the --json fields: created_at, updated_at,
  started_at and done_at. Each reads the row's own field - a group's too,
  never its members' - in the forms above, and a row without that field
  is unknown and falls in no range. done_at stays when a row is reopened,
  so add status=done for what is closed now; done= keeps the meaning of
  --done. updated_at= with a single day is that day, where since= runs on
  from it. In the explorer (SPC f) a value right after = may be quoted.

    moai show --filter 'done_at=2026-10-03 00:00~2026-10-05 23:59'
    moai show -a raven -a joshep --filter started_at=2026-10-01~

  --since keys on each row's own stamp. It misses a removed row (`moai rm`
  leaves no row - --removed below gives those), a note (`moai note` writes
  the journal, not the row), a row whose derived value changed without a
  write of its own (a group's column, an inherited epic) and a row merged
  in with an older stamp. A stamp moai cannot read (fractions, an offset)
  falls in no time range. For a complete copy, pull the whole list
  (--archived, and `moai backlog show --archived` for backlog items)
  and compare row by row.

    moai show --since 2026-09-29T00:00:00Z --json
    moai show --done 2026-09-01..2026-09-30 --type issue

  Removed: `moai show --removed` lists the issues `moai rm` took out,
  oldest first, read from the journal beside the tracker - other
  worktrees are not overlaid, `rm --line` removed an unreadable line, not
  an issue, and `moai <kind> show` refuses it. With --since, those whose
  rm line is stamped at or after it - like --since on rows, that misses a
  removal merged in with an older stamp or never written to the journal,
  so keep the full compare. --json gives each line in the shape of
  `journal` in `moai show <id> --json`, without fields this build does not
  know. It lays the history out and holds it against nothing: an id there
  may live again - created anew, or brought back with an older stamp the
  row list misses - and only the snapshot says whether it lives now.
  A journal line it cannot read that may have held a removal - one cut
  short by a full disk or a crash - is named on stderr by file and line,
  and the exit code is not 0; with --since, only a line stamped in the
  range or with no stamp it can read. --since is the one flag it takes.

    moai show --removed --since 2026-09-29T00:00:00Z --json

  There is no query language. The filters read derived values the file does
  not hold - a group's column, an inherited epic - so run SQL on the --json
  output, where derived_status and derived_epic are worked out already:

    moai show --type issue --json |
      jq -r '.[] | .derived_epic // "none"' | sort | uniq -c
    moai show --archived --json | duckdb -c "SELECT kind, count(*)
      FROM read_json('/dev/stdin', columns = {kind: 'VARCHAR'}) GROUP BY 1"
```

## `moai stats`

```
Count them - spread, flow, lead and cycle time, AI work

Usage: moai stats [OPTIONS]

Options:
      --by <axis>            Print these axes in full (see below)
      --bucket <day|week>    What one row of the flow covers (week when absent)
      --last <n>             How many rows of flow, the current one included
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

Filters  (comma = or,  repeated = and):
  -s, --status <status>                      In that column
  -t, --tag <tag>                            Carrying that tag
      --no-tag <tag>                         Not carrying that tag
  -e, --epic <id|none>                       In that epic (`none` = no epic)
      --milestone <id|none>                  In that milestone (`none` = none)
      --parent <id|none>                     Child of this issue (`none` = top)
  -p, --priority <0-3>                       
  -a, --assignee <who|none|me>               That assignee (repeated = or)
      --type <issue|epic|milestone|backlog>  
  -g, --grep <text>                          In id, title, tag, body or notes
      --stale <days>                         Sitting in its column that long
      --since <when>                         Changed since that time (see below)
      --created <from..to>                   Created in that range (see below)
      --done <from..to>                      Closed in that range (see below)
      --deferred                             Only what is deferred
      --all                                  Include done/deferred, no archive
      --archived                             Include the archive too (old done)
      --filter <item=value>                  One filter string (`status=todo`)

  Counts the rows the filters pick - the same filters as `moai show` -
  with done, deferred, backlog items and the archive in: it counts what
  happened, so nothing finished is hidden. Every number counts one kind, issue
  unless --type names another; a group is measured through its members
  (-e, --milestone). The kind axis alone counts every row picked, to show
  what was left out. --all and --archived are taken and change nothing.
  The time filters read as in `moai show --help`.

  Nothing is stored, and the journal's column moves are never folded in.
  The numbers come from the rows and from the `model:` lines in the notes -
  the same `work` that `moai show --json` gives.

  Axes - --by status,tag prints those in full (with --bucket or --last
  the flow follows them), and without --by the overview shows the first
  three:
    status      the column, a group's read from its members as on the board
    kind        issue, epic, milestone, backlog - every row picked
    priority    0 to 3
    tag         a row counts once per tag it carries
    assignee    name and email
    epic        the epic a row stands in (derived_epic)
    milestone   the milestone it stands in, through its epic too

  Flow: per day or per week (Monday first) on your clock - the day
  `--created <day>` reads. Created counts rows made then, done counts rows
  standing in done that got there then (the time --done reads). --last is
  how many buckets, the current one included (14 days or 8 weeks).

  Lead time runs from created to done, cycle time from started_at - the
  first move out of the first column - to done, over rows standing in done,
  in minutes. A row with no start is unknown, not zero: measured and
  unknown are counted apart, and the median and p90 cover the measured
  ones only. It is wall clock, not effort.

  AI work sums the `model:` lines, and recorded counts the rows that carry
  one. Tokens add up over the lines that carry them (tokened), and tokens
  is null when none does - unknown, not 0. Reviews are the rows tagged
  review; the grade on their lines is the review grade.

  moai stats                           the overview
  moai stats -e moai-1hka              one epic's members
  moai stats --by tag,assignee         two axes in full
  moai stats --bucket day --last 30 --json

  --json gives one object. Every key is always there except three: `by`
  holds the axes asked (all seven without --by), an assignee carries email
  only when one is written, and journal_error stands only when a journal
  could not be read - then work and reviews are short. A null key means
  none - no tag, no epic, no assignee. An axis need not add up to rows: a
  row counts once per tag, and a row placed in no group (a twin's eclipsed
  line, as -e none leaves it out) stands under no epic or milestone at
  all. Durations are minutes.

    {"kind":"issue","rows":42,
     "by":{"status":[{"key":"todo","rows":9},...],
           "priority":[{"key":0,"rows":1},...],
           "assignee":[{"key":"Kim","email":"kim@example.com","rows":7},
                       {"key":null,"rows":2}],...},
     "flow":{"bucket":"week","zone":"UTC",
             "buckets":[{"start":"2026-09-28","created":3,"done":2},...]},
     "lead_time":{"done":30,"measured":30,"unknown":0,
                  "median":95,"p90":4100},
     "cycle_time":{...the same keys...},
     "work":{"recorded":25,"lines":31,"tokened":28,"tokens":5100000,
             "by_model":[{"provider":"anthropic","model":"opus-5",
                          "lines":31,"tokened":28,"tokens":5100000}],
             "by_grade":[{"grade":"high","lines":12,...},...]},
     "reviews":{"rows":6,"work":{...the same keys as work...}}}
```

## `moai archive`

```
Move eligible closed rows into yearly archive files

Usage: moai archive [OPTIONS]

Options:
      --dry-run              Show what would move without changing files
      --drop <ID>            Remove archive copies; keep the live row
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help
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
          A note on this move (journal only). `-` reads stdin
          
          Text, not a path - a file goes in as `-m - < <file>`. A one-line
          text that names a file still goes in as those words, and one
          stderr line says so.

      --from <column>
          Only while still in this column (racing pickups)
          
          Without it nothing is blocked, as before. With it, the column is
          looked at again inside the lock, and a row whose column changed in
          the meantime is left untouched and stands as a partial failure.

      --take
          Become the assignee and note whose it was

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

  Work that is someone else's, or nobody's, is asked about before it is picked
  up. Moving it into a started column still goes through - one stderr line says
  whose it is. On a yes, `--take` makes you the assignee in the same write and
  leaves a note `Taken-over: <who it was|none>`; `-m` says who said yes. It also
  takes a row that already stands in that column. `--json` carries taken
  (rows whose assignee changed) and theirs (moved without --take), always as
  arrays.

  moai mv moai-4aex in_progress --from todo --take -m 'the owner said yes'

  Closing says what that write opened - work that just became ready, a parent
  whose last unfinished child is now done, and the next pick in the same epic.
  Nothing of that is stored: it is read from the rows each time, and `--json`
  carries unblocked, closable and next, always as arrays.
```

## `moai edit`

```
Edit title, body, tags, epic or priority

Usage: moai edit [OPTIONS] <id>

Arguments:
  <id>  

Options:
      --title <text>         One line. A lone `-x` needs `--title=-x`
  -b, --body <text>          Text, not a path. `-` reads stdin: `-b - < <file>`
  -t, --tag <tag>            Add tags
      --untag <tag>          Remove tags
  -e, --epic <id|none>       Move the epic (`none` clears only its own field)
      --milestone <id|none>  Move the milestone (`none` clears its own field)
  -p, --priority <0-3>       
  -a, --assignee <who|none>  Give `name (email)`. `none` clears it, `me` is you
      --start <date|none>    Start of a milestone. `none` clears it
      --due <date|none>      Deadline of a milestone. `none` clears it
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

Usage: moai rm [OPTIONS] [id]...

Arguments:
  [id]...  

Options:
      --line <n>             Remove the unreadable line at number <n>
      --yes                  With --line and --match: remove it. Else only shown
      --match <hash>         With --yes: the hash the preview printed
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

  An unreadable line - a row that cannot be read as an issue, which
  `moai status` counts and `moai show` names by line number - is not
  reachable by id. When it is not worth keeping (a broken twin, junk),
  remove it by its line number; a row a newer moai wrote reads again after
  upgrading, so keep that one:

  moai rm --line 812                          shows the line and its hash
  moai rm --line 812 --yes --match 1a2b3c4d   removes it

  Only an unreadable line is removed; any other line is refused, and the
  refusal lists the unreadable lines as they stand now. Numbers move when a
  row comes or goes - an earlier removal included - so --yes needs the hash
  the preview printed, and a line at that number that no longer matches it
  is refused with the number the line shown stands at now. Removing cannot
  be undone: the raw line is printed, goes into the journal's `rm` entry (up
  to 64KB) and `--json` hands it back whole, to be put back by hand.
```

## `moai note`

```
Leave a note on an issue (journal only)

Usage: moai note [OPTIONS] <id> [text]

Arguments:
  <id>
          

  [text]
          What the next person (or agent) should read. A lone `-x` needs `--`

Options:
  -b, --body <text>
          Text, not a path. `-` reads stdin: `-b - < <file>`
          
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
          Why it is deferred (journal only). `-` reads stdin
          
          Text, not a path - a file goes in as `-m - < <file>`. A one-line
          text that names a file still goes in as those words, and one
          stderr line says so.

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
          One line. Wrap it in quotes. A lone `-x` needs `--` in front

Options:
  -e, --epic <id>
          Put it in this epic

      --milestone <id>
          Put it in this milestone (with `--from`, on the epic it creates)

  -t, --tag <tag>
          Join with commas or give it several times

  -p, --priority <0-3>
          0 is the highest

  -s, --status <status>
          The column it first stands in. The first column when absent

  -b, --body <text>
          Text, not a path. `-` reads stdin (with `--from`, on the first epic)
          
          **A file goes in as `--body - < <file>`.** `--body <path>` takes
          the path as the body, it does not open the file.

  -a, --assignee <who|none>
          Assignee. The creator when absent; `none` clears it, `me` is you

      --start <date>
          Start of a milestone, `YYYY-MM-DD` (milestone rows only)

      --due <date>
          Deadline of a milestone, `YYYY-MM-DD` (milestone rows only)

      --type <issue|epic|milestone|backlog>
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
      --removed              Removed issues, from the journal (see below)
      --worktree             Also overlay other worktrees (no file changes)
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

Filters  (comma = or,  repeated = and):
  -s, --status <status>                      In that column
  -t, --tag <tag>                            Carrying that tag
      --no-tag <tag>                         Not carrying that tag
  -e, --epic <id|none>                       In that epic (`none` = no epic)
      --milestone <id|none>                  In that milestone (`none` = none)
      --parent <id|none>                     Child of this issue (`none` = top)
  -p, --priority <0-3>                       
  -a, --assignee <who|none|me>               That assignee (repeated = or)
      --type <issue|epic|milestone|backlog>  
  -g, --grep <text>                          In id, title, tag, body or notes
      --stale <days>                         Sitting in its column that long
      --since <when>                         Changed since that time (see below)
      --created <from..to>                   Created in that range (see below)
      --done <from..to>                      Closed in that range (see below)
      --deferred                             Only what is deferred
      --all                                  Include done/deferred, no archive
      --archived                             Include the archive too (old done)
      --filter <item=value>                  One filter string (`status=todo`)

Order and paging:
      --sort <key>     Order the list by that key (priority when absent)
      --reverse        Turn the order around, ties included
  -n, --limit <count>  Give at most this many rows
      --after <id>     Start after this row: the last id of the page before

  Order: --sort priority (the default: urgent first, then id), created and
  updated (newest first), status (the column order of .moai/config.toml),
  assignee (the name the screen shows, unowned last), title (ignoring case),
  id. Ties under created and updated fall to id alone, and under every
  other order to priority, then id. --reverse turns the whole order around.

  Paging: -n cuts the list, and --after <id> starts the next page after the
  last id of the page before. The cursor is that row's value in the order,
  not a position, so rows created or removed meanwhile never shift a page.
  A row whose place in the order changes between pages - the cursor row or
  any other, a priority edit included - can repeat or be skipped; --sort id
  and --sort created are the orders no edit moves. Lines sharing one id
  (twins a merge left behind) stand together and a page never splits them,
  so such a page can run past -n. --json stays an array - fewer rows than
  -n means the list has ended.

    moai show --sort id -n 100 --json
    moai show --sort id -n 100 --after <last id> --json

  Archive: a row that has stood in done for archive_days (14 unless
  .moai/config.toml says otherwise, 0 turns it off) is the archive - an
  epic or milestone counted from when it got to done, the clock --done
  reads. --all and -s done leave it out and the tail says how many;
  --archived brings it back, done and deferred with it. Asking by time
  (--since, --created, --done, --stale) finds it anyway, and so does -g
  once done is let in (-g --all). Nothing is stored: it is read from the
  column and the clock each time.

    moai show --archived -g parser

  Time: --since <when> keeps the rows whose own updated_at is at or after
  it. --created and --done take a range from..to or from~to with either
  side left open, or a single time, which is all it spans. <when> is
  YYYY-MM-DD, a day on your own clock - the time zone the screen and
  milestone deadlines use; YYYY-MM-DD HH:MM, one minute on that clock;
  or YYYY-MM-DDTHH:MM:SSZ, an instant in UTC that no time zone moves. The
  end of a range takes all it spans too, so ~2026-10-05 23:59 runs to
  23:59:59. Quote a range that opens with ~ ('~2026-10-05'): zsh takes a
  bare ~2026-10-05 for a named directory. --done looks at rows standing in
  done now, at the time they last got there - an epic or milestone when its
  last member got to done, or when the rest were deferred if that came
  later. Moving rows in or out and removing them leave no trace on the
  group: closed that way it counts from its last finished member, and a
  deferred row moved into a closed group dates it from that row's deferral.
  Asking by time opens what the list hides by default - done, deferred and
  backlog items - because a row closed meanwhile changed too. Narrow with
  -s (name the columns you want) or --type; --deferred keeps only what is
  deferred, and no flag leaves deferred rows out. A lone instant given to
  --created or --done is that one second, not a day.

  --filter takes each filter as item=value, one per flag, and adds four
  time items named after the --json fields: created_at, updated_at,
  started_at and done_at. Each reads the row's own field - a group's too,
  never its members' - in the forms above, and a row without that field
  is unknown and falls in no range. done_at stays when a row is reopened,
  so add status=done for what is closed now; done= keeps the meaning of
  --done. updated_at= with a single day is that day, where since= runs on
  from it. In the explorer (SPC f) a value right after = may be quoted.

    moai show --filter 'done_at=2026-10-03 00:00~2026-10-05 23:59'
    moai show -a raven -a joshep --filter started_at=2026-10-01~

  --since keys on each row's own stamp. It misses a removed row (`moai rm`
  leaves no row - --removed below gives those), a note (`moai note` writes
  the journal, not the row), a row whose derived value changed without a
  write of its own (a group's column, an inherited epic) and a row merged
  in with an older stamp. A stamp moai cannot read (fractions, an offset)
  falls in no time range. For a complete copy, pull the whole list
  (--archived, and `moai backlog show --archived` for backlog items)
  and compare row by row.

    moai show --since 2026-09-29T00:00:00Z --json
    moai show --done 2026-09-01..2026-09-30 --type issue

  Removed: `moai show --removed` lists the issues `moai rm` took out,
  oldest first, read from the journal beside the tracker - other
  worktrees are not overlaid, `rm --line` removed an unreadable line, not
  an issue, and `moai <kind> show` refuses it. With --since, those whose
  rm line is stamped at or after it - like --since on rows, that misses a
  removal merged in with an older stamp or never written to the journal,
  so keep the full compare. --json gives each line in the shape of
  `journal` in `moai show <id> --json`, without fields this build does not
  know. It lays the history out and holds it against nothing: an id there
  may live again - created anew, or brought back with an older stamp the
  row list misses - and only the snapshot says whether it lives now.
  A journal line it cannot read that may have held a removal - one cut
  short by a full disk or a crash - is named on stderr by file and line,
  and the exit code is not 0; with --since, only a line stamped in the
  range or with no stamp it can read. --since is the one flag it takes.

    moai show --removed --since 2026-09-29T00:00:00Z --json

  There is no query language. The filters read derived values the file does
  not hold - a group's column, an inherited epic - so run SQL on the --json
  output, where derived_status and derived_epic are worked out already:

    moai show --type issue --json |
      jq -r '.[] | .derived_epic // "none"' | sort | uniq -c
    moai show --archived --json | duckdb -c "SELECT kind, count(*)
      FROM read_json('/dev/stdin', columns = {kind: 'VARCHAR'}) GROUP BY 1"
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
          One line. Wrap it in quotes. A lone `-x` needs `--` in front

Options:
  -e, --epic <id>
          Put it in this epic

      --milestone <id>
          Put it in this milestone (with `--from`, on the epic it creates)

  -t, --tag <tag>
          Join with commas or give it several times

  -p, --priority <0-3>
          0 is the highest

  -s, --status <status>
          The column it first stands in. The first column when absent

  -b, --body <text>
          Text, not a path. `-` reads stdin (with `--from`, on the first epic)
          
          **A file goes in as `--body - < <file>`.** `--body <path>` takes
          the path as the body, it does not open the file.

  -a, --assignee <who|none>
          Assignee. The creator when absent; `none` clears it, `me` is you

      --start <date>
          Start of a milestone, `YYYY-MM-DD` (milestone rows only)

      --due <date>
          Deadline of a milestone, `YYYY-MM-DD` (milestone rows only)

      --type <issue|epic|milestone|backlog>
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
      --removed              Removed issues, from the journal (see below)
      --worktree             Also overlay other worktrees (no file changes)
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

Filters  (comma = or,  repeated = and):
  -s, --status <status>                      In that column
  -t, --tag <tag>                            Carrying that tag
      --no-tag <tag>                         Not carrying that tag
  -e, --epic <id|none>                       In that epic (`none` = no epic)
      --milestone <id|none>                  In that milestone (`none` = none)
      --parent <id|none>                     Child of this issue (`none` = top)
  -p, --priority <0-3>                       
  -a, --assignee <who|none|me>               That assignee (repeated = or)
      --type <issue|epic|milestone|backlog>  
  -g, --grep <text>                          In id, title, tag, body or notes
      --stale <days>                         Sitting in its column that long
      --since <when>                         Changed since that time (see below)
      --created <from..to>                   Created in that range (see below)
      --done <from..to>                      Closed in that range (see below)
      --deferred                             Only what is deferred
      --all                                  Include done/deferred, no archive
      --archived                             Include the archive too (old done)
      --filter <item=value>                  One filter string (`status=todo`)

Order and paging:
      --sort <key>     Order the list by that key (priority when absent)
      --reverse        Turn the order around, ties included
  -n, --limit <count>  Give at most this many rows
      --after <id>     Start after this row: the last id of the page before

  Order: --sort priority (the default: urgent first, then id), created and
  updated (newest first), status (the column order of .moai/config.toml),
  assignee (the name the screen shows, unowned last), title (ignoring case),
  id. Ties under created and updated fall to id alone, and under every
  other order to priority, then id. --reverse turns the whole order around.

  Paging: -n cuts the list, and --after <id> starts the next page after the
  last id of the page before. The cursor is that row's value in the order,
  not a position, so rows created or removed meanwhile never shift a page.
  A row whose place in the order changes between pages - the cursor row or
  any other, a priority edit included - can repeat or be skipped; --sort id
  and --sort created are the orders no edit moves. Lines sharing one id
  (twins a merge left behind) stand together and a page never splits them,
  so such a page can run past -n. --json stays an array - fewer rows than
  -n means the list has ended.

    moai show --sort id -n 100 --json
    moai show --sort id -n 100 --after <last id> --json

  Archive: a row that has stood in done for archive_days (14 unless
  .moai/config.toml says otherwise, 0 turns it off) is the archive - an
  epic or milestone counted from when it got to done, the clock --done
  reads. --all and -s done leave it out and the tail says how many;
  --archived brings it back, done and deferred with it. Asking by time
  (--since, --created, --done, --stale) finds it anyway, and so does -g
  once done is let in (-g --all). Nothing is stored: it is read from the
  column and the clock each time.

    moai show --archived -g parser

  Time: --since <when> keeps the rows whose own updated_at is at or after
  it. --created and --done take a range from..to or from~to with either
  side left open, or a single time, which is all it spans. <when> is
  YYYY-MM-DD, a day on your own clock - the time zone the screen and
  milestone deadlines use; YYYY-MM-DD HH:MM, one minute on that clock;
  or YYYY-MM-DDTHH:MM:SSZ, an instant in UTC that no time zone moves. The
  end of a range takes all it spans too, so ~2026-10-05 23:59 runs to
  23:59:59. Quote a range that opens with ~ ('~2026-10-05'): zsh takes a
  bare ~2026-10-05 for a named directory. --done looks at rows standing in
  done now, at the time they last got there - an epic or milestone when its
  last member got to done, or when the rest were deferred if that came
  later. Moving rows in or out and removing them leave no trace on the
  group: closed that way it counts from its last finished member, and a
  deferred row moved into a closed group dates it from that row's deferral.
  Asking by time opens what the list hides by default - done, deferred and
  backlog items - because a row closed meanwhile changed too. Narrow with
  -s (name the columns you want) or --type; --deferred keeps only what is
  deferred, and no flag leaves deferred rows out. A lone instant given to
  --created or --done is that one second, not a day.

  --filter takes each filter as item=value, one per flag, and adds four
  time items named after the --json fields: created_at, updated_at,
  started_at and done_at. Each reads the row's own field - a group's too,
  never its members' - in the forms above, and a row without that field
  is unknown and falls in no range. done_at stays when a row is reopened,
  so add status=done for what is closed now; done= keeps the meaning of
  --done. updated_at= with a single day is that day, where since= runs on
  from it. In the explorer (SPC f) a value right after = may be quoted.

    moai show --filter 'done_at=2026-10-03 00:00~2026-10-05 23:59'
    moai show -a raven -a joshep --filter started_at=2026-10-01~

  --since keys on each row's own stamp. It misses a removed row (`moai rm`
  leaves no row - --removed below gives those), a note (`moai note` writes
  the journal, not the row), a row whose derived value changed without a
  write of its own (a group's column, an inherited epic) and a row merged
  in with an older stamp. A stamp moai cannot read (fractions, an offset)
  falls in no time range. For a complete copy, pull the whole list
  (--archived, and `moai backlog show --archived` for backlog items)
  and compare row by row.

    moai show --since 2026-09-29T00:00:00Z --json
    moai show --done 2026-09-01..2026-09-30 --type issue

  Removed: `moai show --removed` lists the issues `moai rm` took out,
  oldest first, read from the journal beside the tracker - other
  worktrees are not overlaid, `rm --line` removed an unreadable line, not
  an issue, and `moai <kind> show` refuses it. With --since, those whose
  rm line is stamped at or after it - like --since on rows, that misses a
  removal merged in with an older stamp or never written to the journal,
  so keep the full compare. --json gives each line in the shape of
  `journal` in `moai show <id> --json`, without fields this build does not
  know. It lays the history out and holds it against nothing: an id there
  may live again - created anew, or brought back with an older stamp the
  row list misses - and only the snapshot says whether it lives now.
  A journal line it cannot read that may have held a removal - one cut
  short by a full disk or a crash - is named on stderr by file and line,
  and the exit code is not 0; with --since, only a line stamped in the
  range or with no stamp it can read. --since is the one flag it takes.

    moai show --removed --since 2026-09-29T00:00:00Z --json

  There is no query language. The filters read derived values the file does
  not hold - a group's column, an inherited epic - so run SQL on the --json
  output, where derived_status and derived_epic are worked out already:

    moai show --type issue --json |
      jq -r '.[] | .derived_epic // "none"' | sort | uniq -c
    moai show --archived --json | duckdb -c "SELECT kind, count(*)
      FROM read_json('/dev/stdin', columns = {kind: 'VARCHAR'}) GROUP BY 1"
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
          One line. Wrap it in quotes. A lone `-x` needs `--` in front

Options:
  -e, --epic <id>
          Put it in this epic

      --milestone <id>
          Put it in this milestone (with `--from`, on the epic it creates)

  -t, --tag <tag>
          Join with commas or give it several times

  -p, --priority <0-3>
          0 is the highest

  -s, --status <status>
          The column it first stands in. The first column when absent

  -b, --body <text>
          Text, not a path. `-` reads stdin (with `--from`, on the first epic)
          
          **A file goes in as `--body - < <file>`.** `--body <path>` takes
          the path as the body, it does not open the file.

  -a, --assignee <who|none>
          Assignee. The creator when absent; `none` clears it, `me` is you

      --start <date>
          Start of a milestone, `YYYY-MM-DD` (milestone rows only)

      --due <date>
          Deadline of a milestone, `YYYY-MM-DD` (milestone rows only)

      --type <issue|epic|milestone|backlog>
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
      --removed              Removed issues, from the journal (see below)
      --worktree             Also overlay other worktrees (no file changes)
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

Filters  (comma = or,  repeated = and):
  -s, --status <status>                      In that column
  -t, --tag <tag>                            Carrying that tag
      --no-tag <tag>                         Not carrying that tag
  -e, --epic <id|none>                       In that epic (`none` = no epic)
      --milestone <id|none>                  In that milestone (`none` = none)
      --parent <id|none>                     Child of this issue (`none` = top)
  -p, --priority <0-3>                       
  -a, --assignee <who|none|me>               That assignee (repeated = or)
      --type <issue|epic|milestone|backlog>  
  -g, --grep <text>                          In id, title, tag, body or notes
      --stale <days>                         Sitting in its column that long
      --since <when>                         Changed since that time (see below)
      --created <from..to>                   Created in that range (see below)
      --done <from..to>                      Closed in that range (see below)
      --deferred                             Only what is deferred
      --all                                  Include done/deferred, no archive
      --archived                             Include the archive too (old done)
      --filter <item=value>                  One filter string (`status=todo`)

Order and paging:
      --sort <key>     Order the list by that key (priority when absent)
      --reverse        Turn the order around, ties included
  -n, --limit <count>  Give at most this many rows
      --after <id>     Start after this row: the last id of the page before

  Order: --sort priority (the default: urgent first, then id), created and
  updated (newest first), status (the column order of .moai/config.toml),
  assignee (the name the screen shows, unowned last), title (ignoring case),
  id. Ties under created and updated fall to id alone, and under every
  other order to priority, then id. --reverse turns the whole order around.

  Paging: -n cuts the list, and --after <id> starts the next page after the
  last id of the page before. The cursor is that row's value in the order,
  not a position, so rows created or removed meanwhile never shift a page.
  A row whose place in the order changes between pages - the cursor row or
  any other, a priority edit included - can repeat or be skipped; --sort id
  and --sort created are the orders no edit moves. Lines sharing one id
  (twins a merge left behind) stand together and a page never splits them,
  so such a page can run past -n. --json stays an array - fewer rows than
  -n means the list has ended.

    moai show --sort id -n 100 --json
    moai show --sort id -n 100 --after <last id> --json

  Archive: a row that has stood in done for archive_days (14 unless
  .moai/config.toml says otherwise, 0 turns it off) is the archive - an
  epic or milestone counted from when it got to done, the clock --done
  reads. --all and -s done leave it out and the tail says how many;
  --archived brings it back, done and deferred with it. Asking by time
  (--since, --created, --done, --stale) finds it anyway, and so does -g
  once done is let in (-g --all). Nothing is stored: it is read from the
  column and the clock each time.

    moai show --archived -g parser

  Time: --since <when> keeps the rows whose own updated_at is at or after
  it. --created and --done take a range from..to or from~to with either
  side left open, or a single time, which is all it spans. <when> is
  YYYY-MM-DD, a day on your own clock - the time zone the screen and
  milestone deadlines use; YYYY-MM-DD HH:MM, one minute on that clock;
  or YYYY-MM-DDTHH:MM:SSZ, an instant in UTC that no time zone moves. The
  end of a range takes all it spans too, so ~2026-10-05 23:59 runs to
  23:59:59. Quote a range that opens with ~ ('~2026-10-05'): zsh takes a
  bare ~2026-10-05 for a named directory. --done looks at rows standing in
  done now, at the time they last got there - an epic or milestone when its
  last member got to done, or when the rest were deferred if that came
  later. Moving rows in or out and removing them leave no trace on the
  group: closed that way it counts from its last finished member, and a
  deferred row moved into a closed group dates it from that row's deferral.
  Asking by time opens what the list hides by default - done, deferred and
  backlog items - because a row closed meanwhile changed too. Narrow with
  -s (name the columns you want) or --type; --deferred keeps only what is
  deferred, and no flag leaves deferred rows out. A lone instant given to
  --created or --done is that one second, not a day.

  --filter takes each filter as item=value, one per flag, and adds four
  time items named after the --json fields: created_at, updated_at,
  started_at and done_at. Each reads the row's own field - a group's too,
  never its members' - in the forms above, and a row without that field
  is unknown and falls in no range. done_at stays when a row is reopened,
  so add status=done for what is closed now; done= keeps the meaning of
  --done. updated_at= with a single day is that day, where since= runs on
  from it. In the explorer (SPC f) a value right after = may be quoted.

    moai show --filter 'done_at=2026-10-03 00:00~2026-10-05 23:59'
    moai show -a raven -a joshep --filter started_at=2026-10-01~

  --since keys on each row's own stamp. It misses a removed row (`moai rm`
  leaves no row - --removed below gives those), a note (`moai note` writes
  the journal, not the row), a row whose derived value changed without a
  write of its own (a group's column, an inherited epic) and a row merged
  in with an older stamp. A stamp moai cannot read (fractions, an offset)
  falls in no time range. For a complete copy, pull the whole list
  (--archived, and `moai backlog show --archived` for backlog items)
  and compare row by row.

    moai show --since 2026-09-29T00:00:00Z --json
    moai show --done 2026-09-01..2026-09-30 --type issue

  Removed: `moai show --removed` lists the issues `moai rm` took out,
  oldest first, read from the journal beside the tracker - other
  worktrees are not overlaid, `rm --line` removed an unreadable line, not
  an issue, and `moai <kind> show` refuses it. With --since, those whose
  rm line is stamped at or after it - like --since on rows, that misses a
  removal merged in with an older stamp or never written to the journal,
  so keep the full compare. --json gives each line in the shape of
  `journal` in `moai show <id> --json`, without fields this build does not
  know. It lays the history out and holds it against nothing: an id there
  may live again - created anew, or brought back with an older stamp the
  row list misses - and only the snapshot says whether it lives now.
  A journal line it cannot read that may have held a removal - one cut
  short by a full disk or a crash - is named on stderr by file and line,
  and the exit code is not 0; with --since, only a line stamped in the
  range or with no stamp it can read. --since is the one flag it takes.

    moai show --removed --since 2026-09-29T00:00:00Z --json

  There is no query language. The filters read derived values the file does
  not hold - a group's column, an inherited epic - so run SQL on the --json
  output, where derived_status and derived_epic are worked out already:

    moai show --type issue --json |
      jq -r '.[] | .derived_epic // "none"' | sort | uniq -c
    moai show --archived --json | duckdb -c "SELECT kind, count(*)
      FROM read_json('/dev/stdin', columns = {kind: 'VARCHAR'}) GROUP BY 1"
```

## `moai backlog`

```
Jot a passing thought down where you are (`--type backlog`)

Usage: moai backlog [OPTIONS] <COMMAND>

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

  moai backlog add 'a passing thought'   jot it
  moai backlog ls                   what has piled up (`backlog show`)

moai backlog add 'install the merge driver in every clone' -b - <<'BACKLOG'
Today `moai merge-driver --install` has to be typed once per clone.
BACKLOG

  Backlog items stand outside `moai ready` and the board's counts,
  and living without an epic is normal for it, so it never trips the
  "issues with no epic" warning.

  Editing and dropping are the verbs you already have: `moai edit <id>`,
  `moai rm <id>`.
```

## `moai backlog add`

```
Create

Usage: moai backlog add [OPTIONS] [title]

Arguments:
  [title]
          One line. Wrap it in quotes. A lone `-x` needs `--` in front

Options:
  -e, --epic <id>
          Put it in this epic

      --milestone <id>
          Put it in this milestone (with `--from`, on the epic it creates)

  -t, --tag <tag>
          Join with commas or give it several times

  -p, --priority <0-3>
          0 is the highest

  -s, --status <status>
          The column it first stands in. The first column when absent

  -b, --body <text>
          Text, not a path. `-` reads stdin (with `--from`, on the first epic)
          
          **A file goes in as `--body - < <file>`.** `--body <path>` takes
          the path as the body, it does not open the file.

  -a, --assignee <who|none>
          Assignee. The creator when absent; `none` clears it, `me` is you

      --start <date>
          Start of a milestone, `YYYY-MM-DD` (milestone rows only)

      --due <date>
          Deadline of a milestone, `YYYY-MM-DD` (milestone rows only)

      --type <issue|epic|milestone|backlog>
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

## `moai backlog show`

```
Open one, or list them (`ls` is the same)

Usage: moai backlog show [OPTIONS] [target]

Arguments:
  [target]  An issue id, or a kind (issue, epic). The whole list when absent

Options:
      --raw                  Print the body as it is in the file, not rendered
      --tree                 Fold it as epic, issue, child
      --as-plan              Print an epic back as `add --from` markdown
      --removed              Removed issues, from the journal (see below)
      --worktree             Also overlay other worktrees (no file changes)
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

Filters  (comma = or,  repeated = and):
  -s, --status <status>                      In that column
  -t, --tag <tag>                            Carrying that tag
      --no-tag <tag>                         Not carrying that tag
  -e, --epic <id|none>                       In that epic (`none` = no epic)
      --milestone <id|none>                  In that milestone (`none` = none)
      --parent <id|none>                     Child of this issue (`none` = top)
  -p, --priority <0-3>                       
  -a, --assignee <who|none|me>               That assignee (repeated = or)
      --type <issue|epic|milestone|backlog>  
  -g, --grep <text>                          In id, title, tag, body or notes
      --stale <days>                         Sitting in its column that long
      --since <when>                         Changed since that time (see below)
      --created <from..to>                   Created in that range (see below)
      --done <from..to>                      Closed in that range (see below)
      --deferred                             Only what is deferred
      --all                                  Include done/deferred, no archive
      --archived                             Include the archive too (old done)
      --filter <item=value>                  One filter string (`status=todo`)

Order and paging:
      --sort <key>     Order the list by that key (priority when absent)
      --reverse        Turn the order around, ties included
  -n, --limit <count>  Give at most this many rows
      --after <id>     Start after this row: the last id of the page before

  Order: --sort priority (the default: urgent first, then id), created and
  updated (newest first), status (the column order of .moai/config.toml),
  assignee (the name the screen shows, unowned last), title (ignoring case),
  id. Ties under created and updated fall to id alone, and under every
  other order to priority, then id. --reverse turns the whole order around.

  Paging: -n cuts the list, and --after <id> starts the next page after the
  last id of the page before. The cursor is that row's value in the order,
  not a position, so rows created or removed meanwhile never shift a page.
  A row whose place in the order changes between pages - the cursor row or
  any other, a priority edit included - can repeat or be skipped; --sort id
  and --sort created are the orders no edit moves. Lines sharing one id
  (twins a merge left behind) stand together and a page never splits them,
  so such a page can run past -n. --json stays an array - fewer rows than
  -n means the list has ended.

    moai show --sort id -n 100 --json
    moai show --sort id -n 100 --after <last id> --json

  Archive: a row that has stood in done for archive_days (14 unless
  .moai/config.toml says otherwise, 0 turns it off) is the archive - an
  epic or milestone counted from when it got to done, the clock --done
  reads. --all and -s done leave it out and the tail says how many;
  --archived brings it back, done and deferred with it. Asking by time
  (--since, --created, --done, --stale) finds it anyway, and so does -g
  once done is let in (-g --all). Nothing is stored: it is read from the
  column and the clock each time.

    moai show --archived -g parser

  Time: --since <when> keeps the rows whose own updated_at is at or after
  it. --created and --done take a range from..to or from~to with either
  side left open, or a single time, which is all it spans. <when> is
  YYYY-MM-DD, a day on your own clock - the time zone the screen and
  milestone deadlines use; YYYY-MM-DD HH:MM, one minute on that clock;
  or YYYY-MM-DDTHH:MM:SSZ, an instant in UTC that no time zone moves. The
  end of a range takes all it spans too, so ~2026-10-05 23:59 runs to
  23:59:59. Quote a range that opens with ~ ('~2026-10-05'): zsh takes a
  bare ~2026-10-05 for a named directory. --done looks at rows standing in
  done now, at the time they last got there - an epic or milestone when its
  last member got to done, or when the rest were deferred if that came
  later. Moving rows in or out and removing them leave no trace on the
  group: closed that way it counts from its last finished member, and a
  deferred row moved into a closed group dates it from that row's deferral.
  Asking by time opens what the list hides by default - done, deferred and
  backlog items - because a row closed meanwhile changed too. Narrow with
  -s (name the columns you want) or --type; --deferred keeps only what is
  deferred, and no flag leaves deferred rows out. A lone instant given to
  --created or --done is that one second, not a day.

  --filter takes each filter as item=value, one per flag, and adds four
  time items named after the --json fields: created_at, updated_at,
  started_at and done_at. Each reads the row's own field - a group's too,
  never its members' - in the forms above, and a row without that field
  is unknown and falls in no range. done_at stays when a row is reopened,
  so add status=done for what is closed now; done= keeps the meaning of
  --done. updated_at= with a single day is that day, where since= runs on
  from it. In the explorer (SPC f) a value right after = may be quoted.

    moai show --filter 'done_at=2026-10-03 00:00~2026-10-05 23:59'
    moai show -a raven -a joshep --filter started_at=2026-10-01~

  --since keys on each row's own stamp. It misses a removed row (`moai rm`
  leaves no row - --removed below gives those), a note (`moai note` writes
  the journal, not the row), a row whose derived value changed without a
  write of its own (a group's column, an inherited epic) and a row merged
  in with an older stamp. A stamp moai cannot read (fractions, an offset)
  falls in no time range. For a complete copy, pull the whole list
  (--archived, and `moai backlog show --archived` for backlog items)
  and compare row by row.

    moai show --since 2026-09-29T00:00:00Z --json
    moai show --done 2026-09-01..2026-09-30 --type issue

  Removed: `moai show --removed` lists the issues `moai rm` took out,
  oldest first, read from the journal beside the tracker - other
  worktrees are not overlaid, `rm --line` removed an unreadable line, not
  an issue, and `moai <kind> show` refuses it. With --since, those whose
  rm line is stamped at or after it - like --since on rows, that misses a
  removal merged in with an older stamp or never written to the journal,
  so keep the full compare. --json gives each line in the shape of
  `journal` in `moai show <id> --json`, without fields this build does not
  know. It lays the history out and holds it against nothing: an id there
  may live again - created anew, or brought back with an older stamp the
  row list misses - and only the snapshot says whether it lives now.
  A journal line it cannot read that may have held a removal - one cut
  short by a full disk or a crash - is named on stderr by file and line,
  and the exit code is not 0; with --since, only a line stamped in the
  range or with no stamp it can read. --since is the one flag it takes.

    moai show --removed --since 2026-09-29T00:00:00Z --json

  There is no query language. The filters read derived values the file does
  not hold - a group's column, an inherited epic - so run SQL on the --json
  output, where derived_status and derived_epic are worked out already:

    moai show --type issue --json |
      jq -r '.[] | .derived_epic // "none"' | sort | uniq -c
    moai show --archived --json | duckdb -c "SELECT kind, count(*)
      FROM read_json('/dev/stdin', columns = {kind: 'VARCHAR'}) GROUP BY 1"
```

## `moai backlog promote`

```
Unfold into one epic and several issues, and close that thought

Usage: moai backlog promote [OPTIONS] --from <file|-> <id>

Arguments:
  <id>  The backlog to unfold

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

moai backlog promote <id> --from - <<'PLAN'
# Epic title
- [p1] first issue #enhancement
- [p2] second issue
PLAN

  **A line in the plan becomes the issue title as it is.** A long backlog title
  carries its length over to the issue, so write a short title again when
  unfolding. The original text stays on that backlog and the history leads back.

  Unfolding closes it - that backlog goes to `done`. What came from what is kept
  in the journal (the history in `moai show <id>`).

  With the epic already standing, `-e <epic>` unfolds into it as members.
  That is where you take back something the epic needs that had gone out as
  a backlog item - the plan then holds `- issue` lines only.

moai backlog promote <id> -e <epic> --from - <<'PLAN'
- [p1] what the epic set out to do
PLAN

  `--dry-run` is where a person looks at the unfolded plan once and says yes.
```

## `moai wiki`

```
Read the project wiki - the markdown pages under `docs/`

Usage: moai wiki [OPTIONS] <COMMAND>

Commands:
  ls    Every page - slug, title, and what does not resolve
  show  One page, drawn (`--json` gives the raw body)
  help  Print this message or the help of the given subcommand(s)

Options:
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

  The wiki is the markdown files under one directory of this checkout -
  `docs/` unless `.moai/config.toml` says otherwise (`wiki_dir = "manual"`,
  a relative path inside the checkout). A page is one `.md` file below it.
  Its slug is that path without `.md` (`guide/explorer`) and its title is the
  first `# ` heading, else the file name. README.md and index.md at the top
  come first, the rest by title.

  moai wiki ls                  every page, and what does not resolve
  moai wiki show <slug>         one page, drawn, and the pages linking to it.
                                `--json` gives the raw body

  **Nothing is stored.** The tracker holds no page and no index - the list,
  the links, the pages linking to each page (`linked_from`) and the ids are
  read from the files every time. Pages are documents, so they ride the
  branch: inside a linked worktree they are read from that worktree, while
  the tracker still goes to the main checkout. Git merges them like any
  other file.

  There is no command that writes a page - edit the file and commit it. A
  page names an issue by its bare id, as a commit subject does, and another
  page by a relative link: `[the explorer](explorer.md)`. A link can land on
  a heading - `glossary.md#epic`, or `#epic` on the same page. The anchor is
  the heading as GitHub makes it (lowercase, punctuation dropped, each space
  a `-`, a repeated heading `-1`, `-2`), so the same link works there too.
```

## `moai wiki ls`

```
Every page - slug, title, and what does not resolve

Usage: moai wiki ls [OPTIONS]

Options:
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

  A page that cannot be read stays in the list with its file name as the
  title and says why - over 1 MB, a link out of the checkout, not a regular
  file, not UTF-8. Over 1 MB is left unread on purpose and the exit code
  stays 0; any other page left unread, or a name or directory the walk had
  to leave out (a line on stderr names it), makes it non-zero once the whole
  list is out. No `wiki_dir` directory yet is not an error: there is no
  wiki to list and the exit code is 0. A `wiki_dir` that is absolute, has
  `..`, leads out of the checkout or into `.git/`, or is not a directory is.

  Under the list, notices count pages with conflict markers, links leading to
  no page, links to a heading the page does not have, and ids naming no
  issue. They block nothing and leave the exit code alone.

  --json gives {"dir","pages":[{"slug","title","path","bytes","issues",
  "links","linked_from","conflict"}]}. `issues` is [{"id","exists"}] - the
  ids with this tracker's prefix the page names outside code blocks; the name
  of a skill moai plants (`moai-wiki`) is never one. `links` is
  [{"text","to","anchor","resolved","anchor_resolved"}] - relative links to a
  `.md` page of this wiki, `to` being the target slug (the page itself for a
  bare `#anchor`). `anchor` is the part after `#` and `anchor_resolved` says
  whether that page has the heading; both are absent on a link with no `#`,
  and `anchor_resolved` is absent when the page could not be read.
  `linked_from` is the slugs of the pages linking to this one.
  A page that could not be read carries `error` ({"kind","said"}, kind
  too_large, refused or failed); absent, it was read whole. What the walk
  left out stands in `skipped` ([{"path","kind","said"}], kind dir_link,
  not_utf8 or unreadable); absent, nothing was left out. When the directory
  itself cannot be used, `pages` gives way to `error` with kind no_dir,
  outside, not_a_dir or failed.
```

## `moai wiki show`

```
One page, drawn (`--json` gives the raw body)

Usage: moai wiki show [OPTIONS] <slug>

Arguments:
  <slug>  The page - its path under the wiki directory without `.md`

Options:
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

  The slug is what `moai wiki ls` prints - matched letter for letter, so
  `Guide` is not `guide`.

  --json gives the page as `ls` does plus `body`, the file as written.
  A page that could not be read has no `body`, carries `error`, and the exit
  code is non-zero. When the wiki directory itself cannot be used - not there
  yet included - it gives {"dir","error"} as `ls` does, non-zero.

  `linked_from` counts only the pages that could be read. When another page
  could not be read, or the walk left a name or directory out, the count may
  be short: `--json` adds `linked_from_partial` (true) and a line under the
  page says how many places were not read. With every page read, neither
  stands. It leaves the exit code alone - `moai wiki ls` names those places.
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
  G and End to the bottom. Ctrl-d and Ctrl-u scroll half a page, Ctrl-f and
  Ctrl-b (PageDown and PageUp) a whole page and the wheel three rows — the
  list, not the cursor: the chosen row and the detail stay put even once the
  row is off screen, and the next cursor key moves from that row and scrolls
  back to it.
  Ctrl-w w moves the focus between the list and the detail (Ctrl-w W goes the
  other way; Ctrl-w h and Ctrl-w l pick the left and right pane, Ctrl-w k and
  Ctrl-w j the top and bottom one when the detail is split that way), and every
  movement key moves the focused pane — to scroll the detail, go there with
  Ctrl-w w.
  / searches, Esc clears the filter you set. r marks the row under the cursor
  as read (see [NEW] below).
  The search and filter fields take Enter to apply and Esc to give up, the
  search filters the list as you type, and Tab and Shift-Tab pick where it
  looks: everything, id, title, tag, body or note (everything reads the
  notes too). In the filter field, Tab completes key names and values,
  Shift-Tab cycles backwards, and another key keeps the current candidate;
  a unique key gains an equals sign so its value can be typed straight away.
  The filter field lists the keys it takes and a few examples
  above itself, and with the cursor in the value of assignee=, tag=, no-tag=
  or milestone= it lists the values there instead: typing narrows them, Up
  and Down pick one, and Enter puts it in; with no list standing it applies
  the filter. Fixed fields also complete status names, priority p0 to p3 and
  type names with Tab; these fields keep Enter to apply. The header at the
  top numbers every registered project, and
  pressing that number without SPC jumps straight there — 0 is everything,
  one list of all projects.

  The rest lives in the menu that opens the moment you press SPC. The menu
  stands up only what works where you are, ignores keys it does not know,
  closes on Esc or SPC and goes one level up on Backspace. Keys that move
  the focused pane close the menu and make that move in the same press,
  unless a key on that level holds the letter (SPC g, SPC g l, SPC c h,
  SPC v l, SPC m g).
  Toggles and sorts (SPC v, SPC c, SPC s) do not close it — try them, watch
  the state, and leave with Esc. That level says so at the bottom right with a
  close hint.
    SPC /    search              SPC f    filter             SPC n    jot
    SPC q    quit
    SPC p a  register            SPC p d  drop from the list
  Screen — which one stands; the root menu names it, as in [board]:
    SPC g l  list                SPC g b  board — described below
    SPC g s  statistics — the numbers `moai stats` gives, drawn (see below)
    SPC g w  wiki — the project's manual pages, read only (see below)
  View — every toggle except the list columns (SPC c) is here:
    SPC v l  deferred         SPC v b  backlog        SPC v a  show all
    SPC v o  the archive — done that has sat a while [shown/hidden]; SPC v a
             leaves it as it is
    SPC v 1  first column of the config [shown/hidden] — the next ones count up
             done has no letter of its own: the column that holds it does
    SPC v d  detail pane [shown/hidden]
    SPC v w  overlay worktrees [on/off]
    SPC v r  raw or rendered
  Sorts and columns call priority, created, updated and assignee by the same
  letter — only title (SPC s t) and tag (SPC c t) split one letter:
    SPC s p  priority            SPC s c  created            SPC s u  updated
    SPC s s  column              SPC s a  assignee           SPC s t  title
    SPC c i  id                  SPC c p  priority           SPC c a  assignee
    SPC c c  created             SPC c u  updated            SPC c n  counts
    SPC c t  tag                 SPC c h  column names [shown/hidden]
    SPC c e  epic — the name of the epic the row is in; off to begin with
    SPC c w  branch mark [shown/hidden] — needs SPC v w to overlay first
  Read:
    SPC m a  everything unread   SPC m g  every member of this group
  Options — how this screen draws what stands, not which rows stand:
    SPC o d  detail pane goes right, bottom, left, top — press again to turn
             it. Whether it stands at all is SPC v d
    SPC o t  the timezone times are written in. It opens a window with the
             names this machine knows. Type to narrow it down and pick one.
             Stored times stay UTC, and so does --json; a bare day or a
             minute in an SPC f filter (created=2026-10-01,
             done_at="2026-10-03 00:00~2026-10-05 23:59") is on this clock too
    SPC o m  mouse [on/off] — on to begin with, and the choice is kept.
             Clicking puts the focus on the pane and the cursor on the row
             under it. The wheel scrolls the pane under the pointer, and
             neither the focus nor a cursor follows: the list, the detail, the
             statistics window, the wiki window's two panes (a click there
             picks the pane and the page, and a click on a link follows it).
             Dragging the line between the list and the detail resizes them,
             and the share the list takes is kept under [tui] as list_width
             and list_height. The wiki window's line drags the same way and
             keeps a share of its own as wiki_width — until it is dragged it
             splits by list_width. With the SPC menu open, a
             click on an item is its key — a group goes down a level, a
             toggle keeps the menu open, and Esc and Bksp on the bottom line
             close it or go up — and a click or a roll outside the menu closes
             it and does what it does there. While a form, a picker or
             a prompt is up the mouse is the terminal's again — selecting and
             middle-button paste work there as they always did. Over the list
             and the detail, the terminal's own selection and paste need Shift
             held in most terminals, Option in iTerm2
  The one key that quits outright is Ctrl-C — anywhere, even mid-typing.
  The screen rereads itself — issues written next door, and `moai read` or
  `moai project add` in another terminal, land without a keypress.

  The version line in the header says whether a newer release is out. When
  one is, the banner says so and names `moai update`, which upgrades the moai
  you are running in place; quitting prints it again. Where `moai update`
  cannot run but install.sh can (a directory under your home you cannot
  write), quitting prints the install.sh line instead, with --dir when it is
  not ~/.local/bin. No line is shown for a build from source, for a binary not
  named moai, or on a machine the releases do not cover.

  Of what came to me (assigned to me or under it), rows changed since the
  last look carry a [NEW] mark in front of the title. Read marks live in my own
  config and the tracker does not change — on the CLI that is `moai read`.

  The list shows done and hides deferred work to begin with — the mark on
  the path line says so, [deferred hidden] or, with the archive below,
  [deferred·archive 312 hidden]. The view is separate from the filter, so
  Esc does not clear it and the two apply together.
  Done that has sat in done for a while is the archive (archive_days in
  .moai/config.toml, 14 unless written; 0 turns it off). It stays hidden on
  the list and the board even with done shown, and the path line counts it
  — [archive 312 hidden]. SPC v o shows it and is kept under [tui] as
  show_archived; SPC v a (all) leaves it hidden. / search finds it anyway.
  Sorting puts urgent, new, earlier column and alphabetical on top, and
  pressing the chosen one again turns it around. When it is not the default
  (priority) the path line says which order it is.
  Columns (SPC c) turn on and off with [shown/hidden]. Tag, epic, assignee,
  created and updated dates stand on the right of the row, and when it gets
  narrow they are dropped in that order — dates, then epic, then assignee,
  then tag — to leave room for the title.
  View, sort and columns are written into the [tui] table of the user config
  on every press and carry over to the next run and to other projects (the
  same file `moai project add` writes).

  SPC g b lays the same rows out as a kanban board instead of a list, and
  SPC g l brings the list back. It is the list's layout, not another window:
  the cursor, the filter, the view, search, [NEW] and the detail are the
  list's, and the choice is kept under [tui] as layout. The menu's root names
  the screen that stands, as in +screen [board]. The columns are backlog,
  deferred and the config's columns in order — backlog is a kind and deferred
  an axis, so nothing is stored for them.
  SPC v b hides backlog items in both the board and the list, and is kept
  under [tui] as hide_backlog.
  At the project root every milestone is a lane, with (no milestone) last;
  inside a milestone or an epic there is one lane. Epics and milestones are
  not cards. Each card is two lines, its id, column and priority over its title,
  and the cursor's card is marked and drawn reversed. Cards Enter can go
  into end their title with /, as in the list. The foot, a third line,
  stands while SPC c has tag, epic, assignee or a date on — an epic shows up on
  the board only as the name in that foot (SPC c e). Cards that are not yours
  grow that foot anyway and say whose they are, dimmed — → name, or unowned —
  in the assignee's place; your own cards stay two lines. Whose is decided the
  way `moai ready` decides it, and when who you are is unknown no card is set
  apart. On a narrow card the tags and the epic name shrink, longest first, so
  whose it is and the dates still show. j and k move within the
  column, h and l go to the next column at the nearest height, gg and G go to
  the column's ends, Enter and Backspace go in and out as in the list, and Tab
  does nothing. The whole board scrolls as one; the column names stay on top.
  When the columns do not fit, the column the cursor stands in always stands
  and the frame title names the rest with their counts. Clicking a card puts
  the cursor on it. The wheel, Ctrl-d and Ctrl-u, Ctrl-f and Ctrl-b, and
  PageDown and PageUp scroll the board, not the cursor: the chosen card and
  the detail stay put even once the card is off screen, and the board scrolls
  to its end. The next cursor key moves from that card and scrolls back to
  it. In the overview (0) every project stands as a header
  across the columns, the same line the list gives it, and an unfolded project
  has its milestone lanes under it; the columns are those of every project
  shown, together. The cursor stands on a header too — k from the project's
  top card in a column goes up to it — and there l unfolds, h folds, Tab
  unfolds everything and Enter goes in, as in the list.

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

  SPC g s opens the statistics window in place of the list and the detail —
  the numbers `moai stats` gives, drawn as bars: the flow per week, the
  columns and priorities, lead and cycle time, and AI work by model. It
  counts the project you are in, or on the one list (0) the project of the
  row under the cursor. With a filter hung (SPC f or /) it counts only what
  passes, and the title says so. b switches the flow between weeks and
  days; j and k, Ctrl-d and Ctrl-u, Ctrl-f and Ctrl-b, gg and G scroll it;
  Esc goes back to where you were, with the cursor, the filter and the
  detail as they were. SPC opens the menu over it with the screens alone:
  SPC g l and SPC g b close it onto the list or the board, and SPC g s
  counts again from scratch — the window is opened fresh every time and
  nothing of it is kept. On a narrow screen it shows the same figures as
  text.

  SPC g w opens the wiki window in place of the list and the detail — the
  project's manual, the markdown pages `moai wiki ls` lists (under docs/
  unless wiki_dir in .moai/config.toml says otherwise). It reads the project
  you are in, or on the one list (0) the project of the row under the
  cursor, and it only reads: pages are files you edit and commit. The pages
  stand on the left, the home page (README or index) first, and the right
  pane shows the page under the cursor. j and k, Ctrl-d and Ctrl-u, Ctrl-f
  and Ctrl-b, gg and G move the focused pane — on the list, Ctrl-d, Ctrl-u,
  Ctrl-f, Ctrl-b and the wheel scroll it and leave the cursor and the page
  where they are, as in the explorer's list. Enter on the list goes to the
  page and Ctrl-w w goes back and forth. Tab picks the links drawn on the
  page in turn, from the first one on screen and round from the last to the
  first, and Shift-Tab goes the other way from the last one on screen: the
  picked link shows reversed, the key bar names where it goes, Enter follows
  it, and once it leaves the pane it is dropped. Clicking a link follows it
  at once. On the page with no link picked, Enter opens
  a list of its links and the issue ids it names, then the pages linking to
  it (marked ←), which moves with j, k, gg, G, Ctrl-d, Ctrl-u, Ctrl-f and
  Ctrl-b, takes one with Enter and closes on Esc. Taking a page link or a
  page linking here goes there and Bksp comes back the way you came. Links
  to a heading (page.md#anchor) open at that heading, its line marked ▸
  until you scroll; one to a heading the page does not have is marked and
  opens the page at its top. Taking an id closes
  the window onto that row (from the one list, inside that project). Links
  that lead nowhere, addresses outside the wiki and ids the tracker does not
  hold are marked and only say so. The page as written (SPC v r) marks no
  link — Tab says so. / searches the titles and bodies of the pages and
  narrows the list as you type, and Esc drops a picked link, then clears
  that search, and then closes the window, with the cursor, the filter and
  the detail as they were. SPC opens the menu over it with the screens and
  SPC v r (raw or rendered). The window is read fresh every time and nothing
  of it is kept.

  SPC n opens the jot form anywhere inside a project. It keeps a backlog item
  with no epic. If an editor is there ($VISUAL, $EDITOR, or vi or nano on
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
Called by an agent's hook. Reads an event on stdin

Usage: moai hook [OPTIONS] <event>

Arguments:
  <event>  Which place it was called from (see the list below)

Options:
      --dialect <agent>      Whose shapes: claude (default), codex, antigravity
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

  Nobody calls this by hand. The hooks `moai skill install` plants call it -
  Claude's plugin, Codex's .codex/hooks.json, Antigravity's .agents/hooks.json.

  **It blocks nothing, and whatever goes wrong the exit code is 0.** A hook
  that spits errors makes every session start noisy, and then people turn the
  hook off - a rule that is off is no rule.

  The directory (cwd) and the session id come from stdin. They are not in the
  environment.

  Events:
    session-start       Writes the baseline. Loads what is held after a compact
    user-prompt-submit  A person asked. Loads the board once
    pre-tool-use        Just before a tool call. The rules stand here
    stop                The turn ends. Checks the state

  --dialect says which agent's shapes come in and go out: claude (the
  default), codex or antigravity. The rules are the same for all three.
  Antigravity has no user-prompt-submit; its first model call of a turn
  (PreInvocation) stands in for it.

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
  trace. A line this binary cannot read travels through byte for byte - paired
  by its id when it has one, so only a row both sides changed is handed over,
  and counted line by line when it has none. What still hands the whole file
  over inside conflict markers is a duplicated id among readable rows, and a
  file that is not text - keeping only what parsed would lose the rest.

  Installing is once per clone. Git reads the driver command from the config
  only, and the config is not committed. In a clone without it, merge=moai in
  `.gitattributes` is simply ignored and git's own merge runs - merging is
  exactly as it was without it. When that repository does set merge=moai,
  `moai status` says in one line that it is not installed here.

  **To say this repository does not want the driver, write that decision in
  `.gitattributes`.** A line for the snapshot that settles merge itself -
  `.moai/issues.jsonl   text eol=lf -merge` - is read as the decision: `init`
  leaves that line alone and every merge-driver line goes quiet. Deleting the
  line instead is read as a gap, and the next `moai init` writes it back.
```

## `moai skill`

```
Plant skills for Claude, Codex, Antigravity (safe to run again)

Usage: moai skill [OPTIONS] <COMMAND>

Commands:
  install    Plant the skills for each agent; register Claude's with `claude`
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
Plant the skills for each agent; register Claude's with `claude`

Usage: moai skill install [OPTIONS]

Options:
      --scope <scope>        Where to register: local (default), project, user
      --agent <agent>        Agent: claude (default), codex, antigravity, auto
      --with <skill>         Also plant this optional skill (repeatable)
      --without <skill>      Remove this optional skill (repeatable)
      --dry-run              Change nothing; only say what would be done
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

  Which agents: --agent claude (the default), codex, antigravity or auto -
  repeat it for several. auto takes whichever of claude, codex and agy is on
  PATH, and claude when none is.

  Codex and Antigravity read the same `.agents/skills/` in this repository, so
  naming either plants it for both. There is nothing to register: commit the
  directory and the team has it. --scope is Claude's registration only. The
  text is the one Claude gets; the steps that differ per agent are in the
  `moai` skill's "Words per agent" table. The supervisor (moai-supervise),
  its hands on tmux panes (moai-tmux) and the recovery of sessions that died
  (moai-recover) are planted for Claude Code only — Codex and Antigravity get
  no supervisor.

  Optional skills are planted only where you choose them: today moai-tmux,
  the supervisor's hands on tmux panes. --with <skill> plants one and
  --without <skill> removes it; both repeat, and a comma separates names.
  What you chose is not written down anywhere - a skill planted in the tree
  is the answer. So a plain install refreshes the optional skills already
  planted and plants no new one, and a first install plants none. Naming a
  skill that is always planted, or one name in both, is refused.

  The hooks are each agent's own: --agent codex plants `.codex/hooks.json`,
  --agent antigravity plants `.agents/hooks.json` - commit them too. Codex
  runs project hooks only once you trust them: open /hooks in a codex session
  in this repository, and again whenever they change. A hooks file moai did
  not write is left as it is - merge moai's in by hand, or move it aside. Codex
  sends hooks only for its shell, apply_patch and MCP calls.

  For Claude, skills and hooks are installed into `.claude/moai-plugin/` and
  registered with `claude`. Your settings.json is not touched - putting the
  two keys in is `claude`'s job. The one exception is the old declarations
  below.

  **Running again only overwrites.** Deleting a hook file a running session
  holds would block every tool call of that session. The one thing deleted
  is a skill directory an earlier moai planted in the same tree and this one
  no longer plants (skills/moai-work/, and skills/moai-supervise/ in
  .agents/skills/), or an optional skill left out with --without, and only
  while it holds nothing but the files moai planted there; one with a file
  of yours in it is left, and one line names it.

  The version is a hash of what is installed. Same content, same version, so
  there are no empty updates.

  The two Korean text plugins an earlier moai installed alongside
  (korean-skills and humanize-korean) are **removed**, once per scope: only
  where this repository's moai stands as an earlier version (its installed
  skill still teaches them), and only when their marketplace is the one an
  earlier moai added. Once that scope holds this version, nothing there is
  removed again. Where that earlier version stands at a scope other than
  --scope, install first brings it up to this version there (claude plugin
  update --scope <that scope>), so the next install does not remove from it
  again; if that fails, nothing is removed there and the next install tries
  again. Nothing is removed or brought up when moai's own registration
  fails. A user-scope install is left behind when another repository's moai
  stands there, and then the command to remove it is printed in one line;
  that scope is not brought up either, so it is removed once that moai is
  gone. The marketplaces are left behind. If one cannot be removed, moai's own
  registration still stands and the command to remove it by hand is printed.

  At project scope an earlier moai also declared their marketplaces in the
  committed .claude/settings.json, which kept offering them to the team. moai
  deletes those entries itself - only theirs, only when they point where an
  earlier moai pointed them, and only when nothing in that file still enables
  a plugin from them. The rest of the file stays as it was; commit the
  change. `claude plugin marketplace remove --scope project` is not used: with
  no other declaration in sight it removes the marketplace from the whole
  machine. If the entry cannot be deleted, where to delete it is printed.

  moai skill install                  just me (the default. settings.local.json)
  moai skill install --scope user     every repository on this machine
  moai skill install --scope project  with the team (committed settings.json)
  moai skill install --dry-run        only show what would be done
  moai skill install --agent codex    .agents/skills/ for Codex and Antigravity
  moai skill install --agent auto     whichever agent is on PATH
  moai skill install --with moai-tmux     plant the tmux skill too
  moai skill install --without moai-tmux  take it out again

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

  It **only reads** - `claude`'s registry (~/.claude/plugins/) and the planted
  files. Whatever is out of line the exit code is 0 - this is a command that
  shows, not one that blocks.

  What it looks at:
    marketplace   registered under this repository's name, not pointing
                  somewhere else
    install       which version at which scope, and whether it matches the
                  version that would be installed now
    hook          whether the executable the install calls is still there
    claude        whether it is on PATH (without it nothing can be installed
                  or removed)
    .agents       whether .agents/skills/ holds this version's skills
                  (codex and antigravity read it)
    codex hooks   whether .codex/hooks.json is this version's, and moai's
    agy hooks     whether .agents/hooks.json is this version's, and moai's
    codex, agy    whether they are on PATH
```

## `moai skill uninstall`

```
Remove the registration from `claude`. Installed files stay

Usage: moai skill uninstall [OPTIONS]

Options:
      --agent <agent>        Agent: claude (default), codex, antigravity, auto
      --only <skill>         Remove only this optional skill (repeatable)
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
  settings is `claude`'s job - we do not touch someone else's JSON, except
  the old declarations below.

  **`.claude/moai-plugin/` is not deleted.** Deleting a file a running
  session holds can block that session's tool calls. Delete it after closing
  the session.

  The two Korean plugins an earlier moai installed alongside are removed **at
  the scope moai was removed from**, under the same conditions as install: an
  earlier version of moai stands there and the marketplace is the one it
  added. The marketplace is left behind - the name is global to one machine
  and another repository's install uses it. A user-scope install is also left
  behind when another repository's moai stands there, and then the command to
  remove it is printed in one line. At project scope the marketplace's
  declaration in the committed .claude/settings.json goes, as with install.

  A Claude session already open keeps calling the old hook - reopen it for
  the removal to land.

  For codex and antigravity (--agent) there is no registration to remove,
  and no file is deleted either: the `rm -r` lines for moai's skills in
  `.agents/skills/`, and the `rm` line for that agent's hooks file when moai
  wrote it, are printed, to run once no session holds them. Without that
  --agent, one line says when moai's skills are still there.

  --only <skill> removes one optional skill and nothing else: it is
  `moai skill install --without <skill>` under another name. That skill's
  directory goes (only while it holds nothing but moai's files) and the
  plugin is planted again; the registration and the other skills stay.
  Where this repository's moai is registered, that scope is brought up to
  the new version; where it is registered nowhere, only the files change -
  no registration is made. A skill that is always planted is refused.

  moai skill uninstall --dry-run      only show what would be called
  moai skill uninstall --agent codex  name what to delete in .agents/skills/
  moai skill uninstall --only moai-tmux   take the tmux skill out
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

## `moai update`

```
Upgrade the moai you are running with install.sh

Usage: moai update [OPTIONS]

Options:
      --version <tag>        Release tag to install instead of the latest
      --dry-run              Print what would be fetched and run; touch nothing
      --json                 Machine-readable output. Every human line goes away
      --no-color             Turn colour off (same as `--color never`)
      --color <how>          auto|always|never (auto by default, off when piped)
  -C, --dir <path>           Run in this directory (same as `git -C`)
      --user <name (email)>  Who is doing this (from `git config` when absent)
  -h, --help                 Print help

  moai update                    the latest release, over this binary
  moai update --version v0.9.0   that release instead
  moai update --dry-run          print what would be fetched and run

  It fetches install.sh from the main branch of the repository releases come
  from and runs it with sh, installing into the directory of the binary that is
  running, with --force:

    MOAI_REPO=<owner/name> sh -s -- --dir <that directory> --force

  install.sh's output streams through, and the exit code is sh's. The script
  is fetched by moai itself, not by curl; install.sh then downloads the
  release with curl or wget and checks it against the release checksums.

  The repository is `buzzni/moai` unless your user config says otherwise -
  `repo = "owner/name"` under [update] - and MOAI_REPO wins over both. The
  repository's own .moai/config.toml is never read for it: what it names is
  piped into a shell, and a repository you cloned must not choose that. A
  value that is not owner/name is refused, not replaced with the default.

  Nothing is fetched where it cannot work, and it stops with one line:
  a binary cargo built (target/ or cargo install - rebuild from source
  instead), a binary not named moai, a directory you cannot write (run it as a
  user who can, or reinstall with install.sh --dir), and a machine the releases
  do not cover. A directory outside your home is fine when you can write it.

  --dry-run fetches nothing and runs nothing. --json prints {repo, script,
  dir, command, dry_run} and, after a run, `code` - sh's exit code; install.sh's
  own output then goes to stderr so stdout stays one JSON value.
```

## `moai init`

```
Put a .moai/ into this repository (safe to run again)

Usage: moai init [OPTIONS] [PREFIX]

Arguments:
  [PREFIX]  id prefix (up to 8). Made from the directory name when absent

Options:
      --no-agents            Leave AGENTS.md alone (same as --guide none)
      --guide <how>          block, file (.moai/guide.md + link), hook or none
      --driver               Plant the merge driver in .git/config (the default)
      --no-driver            Leave .git/config alone (plant no merge driver)
      --tracking <how>       Git tracks it (commit) or not (exclude, gitignore)
      --skill                Then run moai skill install --scope local
      --no-skill             Do not install the hooks and skills
      --register             Then add this repository to your project list
      --no-register          Do not add it to your project list
  -y, --yes                  Ask nothing; unset rows plant as init always did
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

  **In a terminal the first init asks.** It shows the prefix and each choice
  with its default picked, and Enter plants. A flag picks its row and locks
  it; give every row a flag and nothing is asked. --yes asks nothing and
  uses the old defaults - committed, with the guide block - for choices that
  neither a flag nor existing git rules and guide files settle. Nothing is
  asked where a script or an agent calls it - stdin or stdout is not a
  terminal, TERM=dumb, or --json - and there init plants the same as --yes.
  Running it again where .moai already stands never asks. Esc stops with nothing
  written. Even a first run reads existing git ignore rules and the moai
  guide block. A later run also recognizes installed moai hooks when no block
  stands. If git fails, init refuses rather than guessing commit mode.

  A new prefix is up to 8 characters - you type it with every id. A longer
  one is refused with shorter candidates. Without one it is made from the
  directory name: dropping hyphens if that fits (moa-issue becomes moaissue),
  else the initials of the hyphenated words (my-company-backend becomes mcb),
  and with a single word the first 8 characters. A repository already
  installed with a longer prefix is read and written as it is.

  **It installs the merge driver too.** The repository declares merge=moai in
  `.gitattributes`, and the command that word names lives in .git/config,
  which is not committed - so init writes both. It picks the `moai` on PATH
  when that is the same build, else the binary running now, and says which.
  Run it again and a path that has gone dead is replaced. A clone of a
  repository that already has a .moai never runs init: there the one line
  from `moai status` is what asks for `moai merge-driver --install`.

  --no-driver leaves .git/config alone. A repository that wants no driver at
  all says so in `.gitattributes` - a line for the snapshot that settles
  merge itself (`.moai/issues.jsonl   text eol=lf -merge`) is read as the
  decision and init leaves it alone. An explicit --driver with --tracking
  exclude or gitignore is refused.

  --json reports gitignore=true only when .gitignore was written; exclude=true
  means the ignore lines were written to .git/info/exclude. Its driver is
  planted, current (already the same line), off (the repository declares no
  driver, or is not a git repository), failed (driver_trouble says why),
  skipped (--no-driver, this run only) or untracked (a tracker kept out of
  git - git never merges it, so there is nothing to drive).

  --check writes nothing and only answers whether the AGENTS.md block is
  current, stale or missing, and where the merge driver stands. It is
  non-zero when a file cannot be read or git cannot determine tracking.

  --print only prints that block. That is where to copy it from when the file
  the agent reads is not AGENTS.md - --print and init write the same text.
```
