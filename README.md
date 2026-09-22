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

It installs to `~/.local/bin` and will not overwrite an existing `moai` unless
you pass `--force`. Pick another directory with `--dir`, or a specific release
with `--version v0.1.0`. Through the pipe those flags belong to the script, not
to your shell, so they need `-s --`:

```sh
curl -fsSL https://raw.githubusercontent.com/buzzni/moai/main/install.sh \
  | sh -s -- --dir ~/bin --version v0.1.0
```

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
- `docs/korean-terms.md` — the Korean terms this repository's issues and notes use
- `CHANGELOG.md` — what changed, per release

## License

MIT. See `LICENSE`.
