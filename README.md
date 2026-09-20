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

moai keeps the whole thing in two append-friendly files under `.moai/`:

- `issues.jsonl` — one line per issue, sorted by id. **The snapshot is the
  truth.** Nothing is folded, replayed or recomputed to answer a question.
- `journal.jsonl` — who created, moved, noted or removed what. It is history
  for humans, and it is never read to compute state.

Everything else follows from those two sentences. A question that can only be
answered by folding the journal is a question whose answer should have been a
field on the snapshot.

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/buzzni/moai/develop/install.sh | sh
```

The installer downloads the release archive **and** `SHA256SUMS`, and refuses to
install anything it cannot verify. There is no flag to skip the check.

It installs to `~/.local/bin` and will not overwrite an existing `moai` unless
you pass `--force`. Pick another directory with `--dir`, or a specific release
with `--version v0.1.0`.

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
the tool grows; it rewrites that block and touches nothing else.

Every command takes `--json`. `moai ready --json` returns
`{"ready":[…],"held":[…]}`, which is enough to drive a loop with no human in it.
`examples/bash-agent/agent.sh` is such a loop in bash and jq;
`examples/python-agents/agents.py` is the multi-agent version.

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

Both are checked in CI. There is no line-count limit on the implementation —
that limit existed once and was removed, because what kills a tool like this is
a design that was wrong from the start, not the number of lines that design
eventually needs.

## Documentation

- `CONTRIBUTING.md` — building, testing, and what a commit here looks like
- `SECURITY.md` — reporting a vulnerability
- `docs/recovery.md` — what to do when the tracker files get into a bad state
- `CHANGELOG.md` — what changed, per release

## License

MIT. See `LICENSE`.
