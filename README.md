# moai

An issue tracker that lives in your repository, for people and coding agents
working in the same tree.

No approval gate. Anything can be created and anything can be moved. Discipline
is something `moai status` reflects back at you, never something it enforces.

```
$ moai
Issues 941  · Epics 148       .moai/issues.jsonl

  · todo 36    ▸ in_progress 5    ? review 0    ✓ done 900

! Started at once 5 — finishing one at a time goes better

+ Backlog 36 (oldest 4 days)
    → `moai backlog ls`

Last 7 days   created 41  ·  done 38   piling up +3

Next:  `moai ready` picks what to take
```

## Why it exists

Issue trackers that agents can use have to answer two questions at once: what
should I work on next, and what did we already decide about it. A web tracker
answers neither from inside the repository, and a Markdown TODO list answers
neither after the third session.

moai keeps the whole thing in append-friendly files under `.moai/`:

- `issues.jsonl` — one line per issue, sorted by id. **The snapshot is the
  truth.** Nothing is folded, replayed or recomputed to answer a question.
- `journal/<email>.jsonl` — who created, moved, noted or removed what, one file
  per writer so two people never collide on a merge. It is history for humans,
  and it is never read to compute state. The older single `journal.jsonl` is
  still read where it exists; nothing is written there any more.

Everything else follows from those two sentences. A question that can only be
answered by folding the journal is a question whose answer should have been a
field on the snapshot.

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/buzzni/moai/main/install.sh | sh
```

The installer downloads the release archive **and** `SHA256SUMS`, and refuses to
install anything it cannot verify. There is no flag to skip the check.

It installs to `~/.local/bin`. Pick another directory with `--dir`, or a
specific release with `--version v0.1.0`. Through the pipe those flags belong to
the script, not to your shell, so they need `-s --`:

```sh
curl -fsSL https://raw.githubusercontent.com/buzzni/moai/main/install.sh \
  | sh -s -- --dir ~/bin --version v0.1.0
```

### Upgrade

The same line upgrades. When the `moai` already in that directory is this moai,
the installer replaces it without `--force` and says which version it went from
and to — to tell, it runs that `moai` (`--version`, then `merge-driver --help`)
before downloading anything. When it is already the version it would install,
it downloads nothing and exits 0. When it is newer than the latest release (a
build from source, or a pre-release you picked), the plain line leaves it alone;
asking for an older release with `--version` replaces it, and says it is going
down.

`--force` is for what the installer will not replace on its own — a binary it
does not recognise as this moai (another tool that happens to be called
`moai`) or a symlink — and for downloading the version you already have again.
Over either of the first two it refuses before downloading anything, and prints
the line to run again with `--force` added, in both the pipe form and the file
form:

```sh
curl -fsSL https://raw.githubusercontent.com/buzzni/moai/main/install.sh | sh -s -- --force
```

When a newer release is out, `moai tui` says so in its header and banner, and
when you quit it prints the line that upgrades the `moai` you are running — with
`--dir` when it lives under your home but not in `~/.local/bin`. A build from source, a `moai`
outside your home and a machine the releases do not cover get no line.

Prebuilt binaries are published for `x86_64-unknown-linux-musl` and
`aarch64-apple-darwin`. On anything else, build from source:

```sh
cargo build --release      # target/release/moai
```

## First five minutes

```sh
moai init                            # asks first in a terminal; --yes plants as before
moai status                          # board, warnings, flow — start sessions here
moai add 'the board wraps at 80 columns' -t bug -p 1
moai ready                           # what you can pick up right now
moai mv <id> in_progress             # pick it up
moai note <id> 'the width comes from view.rs, not the terminal'
moai mv <id> done
```

Two groupings sit above issues, and both read their own column from their
members — you never move an epic by hand:

```sh
moai epic add 'storage layer'
moai milestone add 'v0.1.0'
moai add 'write atomically' -e <epic> --milestone <milestone>
moai show --tree
```

Anything that is not work yet goes in as a backlog item. Backlog items stay out of the board
and out of `moai ready`, so they never blur the plan:

```sh
moai backlog add 'what if the board could fold by epic'
moai backlog ls
moai backlog promote <id> --from -      # opens it into an epic and issues
```

`moai idea` remains a hidden alias for one release (removed in v0.9.0).
Issue JSON views use `kind: "backlog"`; the shared file keeps `kind: "idea"`
for older binaries. `rm --json` returns the removed rows with that stored
spelling. The warning key is `backlog_pile`, configured by
`status_backlog_pile`; the old `status_idea_pile` setting remains readable.

Work you have decided not to do *right now* is deferred, not closed. `moai defer
<id> --undo` brings the same line back, in the same column, with the same kind.

`moai tui` walks the same tracker on one screen — the list on one side, the
issue under the cursor on the other. It answers keys and the mouse alike: click
a pane or a row, roll the wheel over the pane you want to move, drag the line
between the two to resize them, click an item in the `SPC` menu to press its
key — a click outside the menu closes it and lands where you clicked. `moai tui
--help` lists the keys, and `SPC o m` lets the mouse go when you would rather
select or paste with it in the terminal.
`SPC g` picks the screen: `SPC g b` lays the same list out as a kanban board —
backlog, deferred and your columns side by side, one lane per milestone, and in the
overview one header per project — `SPC g l` brings the list back, and the choice
is kept for the next run. `SPC v b` hides the backlog items, and a card that is not
yours says whose it is.
`SPC f` filters with the same `key=value` words as `--filter`. While you type,
the keys it takes and a few examples stand above the field, and in the value of
`assignee=`, `tag=`, `no-tag=` or `milestone=` the values this tracker holds
do — typing narrows them, Up and Down pick one and Enter puts it in.

Done work that has sat in done for two weeks is the archive. It is not removed
and nothing is stored for it — moai reads it off the column and the clock each
time. The list, the board and `moai status` leave it out even when done is
shown, and say how many they left; `SPC v o` in the explorer and `moai show
--archived` bring it back. `/` search finds it anyway, and so does `moai show
-g` once done is let in with `--all`.
`archive_days` in `.moai/config.toml` sets the two weeks, and `0` turns it off.

## The project wiki

The manual lives next to the code: markdown pages under `docs/` — or wherever
`wiki_dir` in `.moai/config.toml` points, a path inside the checkout — one page
per `.md` file.

```sh
moai wiki ls                 # every page, and what does not resolve
moai wiki show <slug>        # one page, drawn; --json gives the raw body
```

Nothing about it is stored in the tracker. The list, the links between pages and
the issue ids a page names are read from the files every time. Pages are
documents, so they ride the branch: inside a worktree you read that worktree's
pages, and they merge like any other file. No command writes a page — edit the
file and commit it. A page names an issue by its bare id and another page by a
relative link (`[the explorer](explorer.md)`), and `moai wiki ls` counts the
links that lead to no page, the ids the tracker does not have and the pages left
with conflict markers. It blocks nothing.

In the explorer `SPC g w` opens the same pages in a window, read only — the
pages on the left, the one under the cursor on the right. `Enter` on a page
lists its links, the issue ids it names and the pages that link to it: a page goes
there and `Bksp` comes back, an id closes the window onto that row. `/` searches the titles and
bodies of the pages.

## Working in parallel

Several sessions in one repository is the normal case, not the exception.

```sh
moai mv <id> in_progress --from todo   # only moves if the column is still todo
moai ready --worktree                  # overlay what neighbouring worktrees hold
```

`--from` is how two sessions stop fighting over the same line: the loser gets a
non-zero exit and moves on instead of silently taking over work someone else
already picked up.

When several *people* share one tracker, an agent asks before it picks up work
that is not its person's. `moai ready` hands out only your own rows and sets the
rest apart — someone else's, or nobody's. On a yes, take it over in one write:

```sh
moai mv <id> in_progress --from todo --take -m 'Kim said yes'   # you become the assignee
```

The assignee changes and a note `Taken-over: <who it was|none>` keeps whose it
was. A person in a terminal is never stopped — `mv` moves and says whose it was
on stderr; the planted hook is what refuses an agent's pick-up without `--take`.

`.moai/issues.jsonl` is one line per issue, so two branches that touched
different issues collide only because their lines are neighbours. Install the
merge driver once per clone and git resolves those per issue:

```sh
moai merge-driver --install
```

## For agents

`moai init` writes a managed block into `AGENTS.md` describing every command and
the three decisions an agent has to make — create or backlog, defer or done, and
whether a request is big enough to split into an epic. Re-run `moai init` when
the tool grows; it rewrites that block and tops up the `.gitattributes` and
`.gitignore` rules it manages. It never touches your issues or your journal.

The first `init` in a terminal asks where the guide goes — the whole block,
`.moai/guide.md` with a link in the block, or the Claude hooks for a tracker kept
out of git — and `--guide` says it without asking. Scripts and agents use the old
block default when no existing ignore rules or guide files settle the choice.
Initialization also preserves a clone's existing guide link and recognizes installed
moai hooks; a git failure is reported before anything is planted.

If your agent reads some other file, take the same block and paste it there:

```sh
moai init --print          # writes nothing, prints the block
moai init --check          # writes nothing, says current / stale / missing
```

`moai skill install` plants the skills for the agents you name — the tracker's
rules and the wiki for each of them, and the supervisor for Claude Code only.
Claude Code gets a plugin in
`.claude/moai-plugin/`, with the hooks, registered with `claude`. Codex and
Antigravity both read `.agents/skills/`, so naming either plants it for both,
and committing it hands it to the team. One text serves all three: the steps
that differ per agent sit in the `moai` skill's "Words per agent" table. The hooks are
each agent's own: Codex gets `.codex/hooks.json` (trust it once in `/hooks`)
and Antigravity `.agents/hooks.json` — commit them too.

```sh
moai skill install                         # Claude Code (the default)
moai skill install --agent codex           # .agents/skills/ and .codex/hooks.json
moai skill install --agent antigravity     # .agents/skills/ and .agents/hooks.json
moai skill install --agent auto            # whichever of claude, codex, agy is on PATH
moai skill status                          # what is planted where, and what is stale
```

moai never launches an agent or runs one headless. A person opens each session
(`claude`, `codex` or `agy`) in the repository root, the ordinary way — it asks
that person before it acts, as it always does — and the session reads
`moai ready --json` to choose its next row.

moai carries no messaging between agents. In Claude Code, one session that calls
`/moai-supervise` hands piled-up backlog items to the other idle sessions of the
repository it sees in `ListAgents`, through Claude Code's own `SendMessage`, and
takes their reports back the same way. Codex and Antigravity have no supervisor.
What each agent needs first — Codex's trust in `/hooks`, and its sandbox on a
machine where that cannot stand — is in
[working with agents](docs/agents.md#open-a-session-for-each-agent).

### The `--json` contract

**Every command takes `--json`.** That is the contract this tool offers to
editors, web UIs, orchestrators and other agents: one flag, on everything, with
no human-shaped output mixed in.

- `--json` prints **exactly one JSON value on one line**, and nothing else goes
  to stdout. Human output disappears entirely; it is not interleaved.
- Failures are JSON too, with a stable `code` you can branch on — `not_found`,
  `bad_status`, `bad_filter`, `bad_target`, `bad_input`, `no_actor`,
  `already_exists`, `locked`, `broken`, and `error` as the catch-all. The human
  sentence beside it is not something to match on.
- Keys that are always present stay present. `moai ready --json` is
  `{"ready":[…],"others":[…],"held":[…]}`, never a bare array — `others` is ready
  work that is someone else's or nobody's (`owner` says which), and `held` is work
  that exists but cannot be picked up, with where to pick it up from. `moai mv`
  carries `taken` and `theirs` the same way, always as arrays.
- A partial result says so in the payload rather than only in the exit code.
  `moai mv <id> <col> --from <col>` carries `moved`, `already`, `missing` and
  `stale` side by side, so a loser in a race reads `stale` and moves on. The
  one exception is a command whose `--json` is a bare array — the `moai show`
  list and `moai show --removed`: a row or journal line it could not read is
  named on stderr and the exit code is not 0, while stdout still carries the
  whole array of what it could read. Valid JSON on stdout with a non-zero exit
  is that partial answer; a failure prints no array.
- **A value that cannot be absent is never absent.** `kind` and `priority` have
  defaults, and the snapshot leaves a default out so that one file-wide diff does
  not follow every release — but that silence is legible only to the writer, so
  `--json` fills it back in (`"kind":"issue"`, `"priority":2`). What the file
  leaves out and what the contract leaves out are two different things. Keys that
  genuinely can be absent — `epic`, `milestone`, `deferred_at` — stay absent, and
  the absence is the answer.
- `moai show <id> --json` always carries `commits` and `work` as arrays. An empty
  `commits` means no commit named this issue; a `commits_error` object means git
  could not be read at all. The two are deliberately different answers.

Nothing here is derived at read time from folding the journal, and `report` and
`query` are pure functions over `&[Issue]` that print nothing. That is what makes
a second surface — a TUI, a web view, your own tool — cheap to attach.

### Taking the list out

`moai show --json` is the list. The file is always read whole; what the flags
below shrink is the output, and with it the tokens.

- `--sort <key>` and `--reverse` pick the order — `priority` (the default),
  `created`, `updated`, `status`, `assignee`, `title` or `id`. Ties under
  `created` and `updated` fall to id alone, and under every other order to
  priority, then id.
- `-n <count>` cuts the list, and `--after <id>` starts the next page after the
  last id of the page before. The cursor is that row's value in the order, not
  an offset, so rows other sessions create or remove meanwhile never shift a
  page. A row whose place in the order changes between pages — the cursor row
  or any other, a priority edit included — can repeat or be skipped; `--sort id`
  and `--sort created` are the orders no edit moves. The output stays a bare
  array — fewer rows than `-n` means the list has ended.
- `--since <when>` keeps the rows whose own `updated_at` is at or after a time,
  and `--created` and `--done` take a range `from~to` (or `from..to`) with
  either side left open. A bare `YYYY-MM-DD` is a day on your own clock, the
  time zone the screen uses; `YYYY-MM-DD HH:MM` is one minute on that clock;
  `YYYY-MM-DDTHH:MM:SSZ` is an instant in UTC. `--filter` adds `created_at=`,
  `updated_at=`, `started_at=` and `done_at=`, each reading the row's own field,
  and in the explorer's `SPC f` a value right after `=` may be quoted —
  `done_at="2026-10-03 00:00~2026-10-05 23:59"`. Asking by time opens what the
  list hides by default — done, deferred and backlog items — since a row closed
  meanwhile changed too.
- `-g` looks through the notes and move messages as well as the id, title, tags
  and body.

`--since` keys on each row's own stamp, so it is a list of rows written since a
time, not a full change feed. It misses a removed row (`moai rm` leaves no row
behind — `--removed` below lists those), a note (`moai note` writes the journal,
not the row), a row whose derived value changed without a write of its own (a
group's column, an inherited epic, milestone or deferral) and a row merged in
from another branch with an older stamp. A stamp moai cannot read — fractional
seconds or an offset left by a hand edit — falls in no time range, for
`--created` and `--done` too.

```sh
# rows written since the last pass, a page at a time - keep the time the last
# pass started, not the time it ended, less a few seconds (a write takes its
# stamp before it waits up to 5s for the lock), and drop repeats by id
moai show --since 2026-09-29T00:00:00Z --sort id -n 200 --json
moai show --since 2026-09-29T00:00:00Z --sort id -n 200 --after <last id> --json
# and the issues removed since then, from the journal
moai show --removed --since 2026-09-29T00:00:00Z --json
```

`moai show --removed` lists what `moai rm` took out, oldest first, as journal
lines in the shape of `journal` in `moai show <id> --json` — each carries `ts`,
`id` and `title`, and a field this build does not know is left out. Without
`--since` it is the whole history. It lays that history out and holds it against
nothing, so an id listed there may live again: created anew, or brought back
with an older stamp that the list above misses — a restored snapshot, or the
twin left when `moai rm` took one of two lines sharing an id. Before dropping an
id from a copy, ask the snapshot: `moai show <id> --json` answers `not_found`
for one that is gone. `--since` keys on the `rm` line's own stamp, so like the
list above it misses a removal merged in from another branch with an older
stamp, and nothing lists a removal whose journal line was never written — keep
the full compare below. It reads the journal beside the tracker only (other
worktrees are not overlaid), takes `--since` and no other filter, and leaves out
`rm --line`, which removed an unreadable line rather than an issue. A journal
file moai cannot read is skipped, named on stderr, and the exit code is not 0.
The same goes for a journal line it cannot read that may have held a removal —
one cut short by a full disk or a crash: the list still comes out, the line is
named on stderr by file and line, and the exit code is not 0. With `--since`
only a line whose own stamp falls in the range, or cannot be read, counts, so
one old cut line does not fail every later pass. moai never rewrites the
journal, so look at what the line held and then delete it by hand. A whole line
that an older moai wrote straight onto a cut one is read back and listed.

When a copy has to be complete, pull the whole list and compare it row by row:
`moai show --archived --json`, plus `moai backlog show --archived --json` since
`moai show` leaves backlog items out. `--all` is not enough for either: it leaves the
archive out, and a closed backlog goes to the archive like any other done row. An
id missing from the new pull was removed.

### Counting

`moai stats` adds the tracker up — rows per column and priority, how many were
created and closed per week or day, lead time (created → done) and cycle time
(started → done), and the AI work written in `model:` note lines, by model and by
grade. It takes the filters `moai show` takes, and `--json` gives the same numbers
to a machine. In the explorer `SPC g s` draws them as bars.

```sh
moai stats                               # overview
moai stats -e <epic> --by tag,assignee   # one epic, two axes in full
moai stats --bucket day --last 14 --json # daily flow for a script
```

What it cannot tell, it says rather than guessing: a closed row with no recorded
start counts as `unknown` in cycle time, not as zero minutes, and a model line
without a token count is counted apart from the ones that have it.

### SQL over the output

There is no query language inside moai. The filters look at derived values — the
column a group reads from its members, the epic a row inherits, a row eclipsed by
a twin, a deferral handed down from a group — so SQL run on
`.moai/issues.jsonl` itself gets those answers wrong. Run it on the `--json`
output, where they are already worked out (`derived_status`, `derived_epic`).
A key that can be absent stays absent — `derived_status` stands only on group
rows — so name the columns you read:

```sh
# open work per epic - an epic or a milestone row is a group, not work
moai show --type issue --json | jq -r 'group_by(.derived_epic) | .[] | "\(.[0].derived_epic // "none")\t\(length)"'

# rows per column - a group stands in the column its members give it
moai show --archived --json | duckdb -c "
  SELECT coalesce(derived_status, status) AS col, kind, count(*) AS n
  FROM read_json('/dev/stdin', columns = {status: 'VARCHAR', derived_status: 'VARCHAR', kind: 'VARCHAR'})
  GROUP BY ALL ORDER BY n DESC"

# tokens per model, read from the `model:` note lines on each issue
moai show --archived --json | duckdb -c "
  SELECT w.model, sum(w.tokens) AS tokens
  FROM (SELECT unnest(work) AS w
        FROM read_json('/dev/stdin', columns = {work: 'STRUCT(model VARCHAR, tokens BIGINT)[]'}))
  GROUP BY ALL ORDER BY tokens DESC NULLS LAST"
```

## Screen language

The interface defaults to English. Pick another with `MOAI_LANG`:

```sh
MOAI_LANG=ko moai status
```

To keep one, put `lang = "ko"` under `[i18n]` in your user config; `MOAI_LANG` wins
over it. `en`, `ko`, `zh`, `ja` and `es` are recognised; anything not yet translated
falls back to English, and the system locale (`LANG`, `LC_ALL`) is not read.
`i18n/README.md` describes how to add a language.

## Budgets

| What | Limit |
| --- | --- |
| release binary | 15 MB |
| clean build, unloaded machine | 5 minutes |

CI measures the binary on every pull request. Build time is not measured there —
a shared runner is by definition a loaded machine, and the number above is about
an unloaded one. There is no line-count limit on the implementation —
that limit existed once and was removed, because what kills a tool like this is
a design that was wrong from the start, not the number of lines that design
eventually needs.

## Documentation

- `docs/` — the project wiki; `moai wiki ls` lists its pages
- `docs/cli.md` — every command's `--help`, generated from the binary
- `CONTRIBUTING.md` — building, testing, and what a commit here looks like
- `SECURITY.md` — reporting a vulnerability
- `docs/recovery.md` — what to do when the tracker files get into a bad state
- `CHANGELOG.md` — what changed, per release

## License

MIT. See `LICENSE`.
