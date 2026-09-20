# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

`scripts/bump-version.sh <version>` moves whatever has accumulated under
`[Unreleased]` into a new release section and leaves `[Unreleased]` empty for the
next one. It does not commit and it does not tag — see `CONTRIBUTING.md`.

## [Unreleased]

### Added

- Issues, epics, milestones, ideas and deferral, stored as one JSONL line per
  issue with a separate journal that is never read to compute state.
- `moai status`, `moai ready`, `moai show`, and a terminal explorer (`moai tui`).
- `--json` on every command, so an agent loop can run with no human in it.
- `moai init`, which writes `.moai/` and a managed block in `AGENTS.md`.
- `moai merge-driver --install`, resolving `.moai/issues.jsonl` per issue
  instead of per neighbouring line.
- `moai skill install`, which installs the Claude skills and hooks.
- Multiple projects registered in one place, and worktree-aware `ready`.
- CI: clippy and tests on every pull request, plus a 15 MB release binary
  budget.
- A release workflow that publishes verified archives on a `v*` tag, and
  `install.sh`, which refuses to install anything it cannot check against
  `SHA256SUMS`.
