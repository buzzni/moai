# moai

An issue tracker that lives in your repository, for people and coding agents
working in the same tree.

No approval gate. Anything can be created and anything can be moved. Discipline
is something `moai status` reflects back at you, never something it enforces.

```
$ moai
이슈 941  · 에픽 148       .moai/issues.jsonl

  · todo 36    ▸ in_progress 5    ? review 0    ✓ done 900

! 한 번에 벌여 놓은 것 5건 — 하나씩 끝내는 편이 낫다
+ 쌓인 idea 36건

다음:  `moai ready` 로 집을 것을 고른다
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
moai init                            # writes .moai/ and the AGENTS.md block
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

Anything that is not work yet goes in as an idea. Ideas stay out of the board
and out of `moai ready`, so they never blur the plan:

```sh
moai idea add 'what if the board could fold by epic'
moai idea ls
moai idea promote <id> --from -      # opens it into an epic and issues
```

Work you have decided not to do *right now* is deferred, not closed. `moai defer
<id> --undo` brings the same line back, in the same column, with the same kind.

## Working in parallel

Several sessions in one repository is the normal case, not the exception.

```sh
moai mv <id> in_progress --from todo   # only moves if the column is still todo
moai ready --worktree                  # overlay what neighbouring worktrees hold
```

`--from` is how two sessions stop fighting over the same line: the loser gets a
non-zero exit and moves on instead of silently taking over work someone else
already picked up.

`.moai/issues.jsonl` is one line per issue, so two branches that touched
different issues collide only because their lines are neighbours. Install the
merge driver once per clone and git resolves those per issue:

```sh
moai merge-driver --install
```

## For agents

`moai init` writes a managed block into `AGENTS.md` describing every command and
the three decisions an agent has to make — create or idea, defer or done, and
whether a request is big enough to split into an epic. Re-run `moai init` when
the tool grows; it rewrites that block and tops up the `.gitattributes` and
`.gitignore` rules it manages. It never touches your issues or your journal.

If your agent reads some other file, take the same block and paste it there:

```sh
moai init --print          # writes nothing, prints the block
moai init --check          # writes nothing, says current / stale / missing
```

`examples/bash-agent/agent.sh` is a complete pick-work-close loop in bash and jq,
and `examples/python-agents/agents.py` is the multi-agent version. Both are run
by the test suite, so neither can rot quietly.

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
  `{"ready":[…],"held":[…]}`, never a bare array — `held` is work that exists but
  cannot be picked up, with where to pick it up from.
- A partial result says so in the payload rather than only in the exit code.
  `moai mv <id> <col> --from <col>` carries `moved`, `already`, `missing` and
  `stale` side by side, so a loser in a race reads `stale` and moves on.
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
  `created`, `updated`, `status`, `assignee`, `title` or `id`. Ties in every
  order fall to priority, then id.
- `-n <count>` cuts the list, and `--after <id>` starts the next page after the
  last id of the page before. The cursor is that row's value in the order, not
  an offset, so rows other sessions create or remove meanwhile never shift a
  page. A row whose place in the order changes between pages — the cursor row
  or any other, a priority edit included — can repeat or be skipped; `--sort id`
  is the one order no edit moves. The output stays a bare array — fewer rows
  than `-n` means the list has ended.
- `--since <when>` keeps the rows whose own `updated_at` is at or after a time,
  and `--created` and `--done` take a range `from..to`. A bare `YYYY-MM-DD` is
  a day on your own clock, the time zone the screen uses; `YYYY-MM-DDTHH:MM:SSZ`
  is an instant in UTC. Asking by time opens what the list hides by default —
  done, deferred and ideas — since a row closed meanwhile changed too.
- `-g` looks through the notes and move messages as well as the id, title, tags
  and body.

`--since` keys on each row's own stamp, so it is a list of rows written since a
time, not a full change feed. It misses a removed row (`moai rm` leaves no row
behind), a note (`moai note` writes the journal, not the row), a row whose
derived value changed without a write of its own (a group's column, an
inherited epic, milestone or deferral) and a row merged in from another branch
with an older stamp. A stamp moai cannot read — fractional seconds or an offset
left by a hand edit — falls in no time range, for `--created` and `--done` too.

```sh
# rows written since the last pass, a page at a time - keep the time the last
# pass started, not the time it ended, and drop repeats by id
moai show --since 2026-09-29T00:00:00Z --sort id -n 200 --json
moai show --since 2026-09-29T00:00:00Z --sort id -n 200 --after <last id> --json
```

When a copy has to be complete, pull the whole list and compare it row by row:
`moai show --all --json`, plus `moai idea show --all --json` since `--all` still
leaves ideas out. An id missing from the new pull was removed.

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
moai show --all --json | duckdb -c "
  SELECT coalesce(derived_status, status) AS col, kind, count(*) AS n
  FROM read_json('/dev/stdin', columns = {status: 'VARCHAR', derived_status: 'VARCHAR', kind: 'VARCHAR'})
  GROUP BY ALL ORDER BY n DESC"

# tokens per model, read from the `model:` note lines on each issue
moai show --all --json | duckdb -c "
  SELECT w.model, sum(w.tokens) AS tokens
  FROM (SELECT unnest(work) AS w
        FROM read_json('/dev/stdin', columns = {work: 'STRUCT(model VARCHAR, tokens BIGINT)[]'}))
  GROUP BY ALL ORDER BY tokens DESC NULLS LAST"
```

## Screen language

The interface currently defaults to Korean. Pick another with `MOAI_LANG`:

```sh
MOAI_LANG=en moai status
```

`en`, `ko`, `zh`, `ja` and `es` are recognised; anything not yet translated falls
back to English. `i18n/README.md` describes how to add a language.

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

- `docs/cli.md` — every command's `--help`, generated from the binary
- `CONTRIBUTING.md` — building, testing, and what a commit here looks like
- `SECURITY.md` — reporting a vulnerability
- `docs/recovery.md` — what to do when the tracker files get into a bad state
- `docs/korean-terms.md` — the Korean terms this repository's issues, notes, commit messages
  and `CLAUDE.md` use
- `CHANGELOG.md` — what changed, per release

## License

MIT. See `LICENSE`.
