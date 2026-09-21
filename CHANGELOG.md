# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

`scripts/bump-version.sh <version>` moves whatever has accumulated under
`[Unreleased]` into a new release section and leaves `[Unreleased]` empty for the
next one. It does not commit and it does not tag — see `CONTRIBUTING.md`.

## [Unreleased]

## [0.1.0] - 2026-09-21

### Added

- Issues, epics, milestones, ideas and deferral, stored as one JSONL line per
  issue with a separate journal that is never read to compute state.
- `moai status`, `moai ready`, `moai show`, and a terminal explorer (`moai tui`).
- `moai prime`, a short markdown page of what you hold and what comes next —
  small enough for an agent to re-read at the start of a session and again
  after a compaction.
- `--json` on every command, so an agent loop can run with no human in it.
- `moai mv <id> in_progress --from todo` moves only while the column you saw
  still holds, so sessions sharing one repository cannot overwrite each other's
  pick-up. The loser gets `stale` and a non-zero exit code.
- `moai mv <id> done` says in one line what that close unblocked.
- `moai link` to record that one issue blocks another, and `moai read` to mark
  an issue read, kept in your own config only.
- `moai add --from -` creates an epic and its issues in one go from a plan
  written in markdown; `moai idea promote` unfolds a parked thought the same way.
- While a milestone is running, `moai ready` hands out what is inside it first
  and leaves only `p0` outside it.
- Commits are found on the spot by the issue id written in their subject —
  `moai show <id>` and the explorer draw them, and nothing is stored on the issue.
- `moai show <id> --json` reads the `model:` notes back as `work`, and carries
  `started_at` and `done_at` for how long a piece of work took.
- English is the language on screen by default. `MOAI_LANG=ko moai status`, or
  `lang` under `[i18n]` in your user config, switches it; en, ko, zh, ja and es
  exist, en and ko are complete, and the system locale is never read.
- `moai init`, which writes `.moai/` and a managed block in `AGENTS.md`.
- `moai merge-driver --install`, resolving `.moai/issues.jsonl` per issue
  instead of per neighbouring line. `moai status` measures whether the driver
  actually runs and shines one line on a clone where it was never installed.
- `moai skill install`, which installs the Claude skills and hooks.
- A `prepare-commit-msg` hook that writes `MOAI_ACTOR` into an `Executed-By:`
  trailer, so `git log` alone shows which commits an agent made.
- Multiple projects registered in one place, and worktree-aware `ready`.
- CI: clippy, `cargo fmt --check` against a committed `rustfmt.toml`, and tests
  on every pull request, plus a 15 MB release binary budget.
- A release workflow that publishes verified archives on a `v*` tag, and
  `install.sh`, which refuses to install anything it cannot check against
  `SHA256SUMS`.
