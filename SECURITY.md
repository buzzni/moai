# Security

## Reporting a vulnerability

Report privately through GitHub's private vulnerability reporting: open the
[Security tab](https://github.com/buzzni/moai/security) of this repository and
choose **Report a vulnerability**. That thread is visible only to maintainers.

**Please do not open a public issue or pull request for a vulnerability.** This
repository's issue tracker is a file in the repository, so a public report is
also a commit.

Include what you would want to receive: what an attacker can do, the smallest
input or repository state that shows it, and the version (`moai --version`) and
platform you saw it on.

You can expect an acknowledgement within a few working days and an assessment,
including whether we agree it is a vulnerability, soon after. We will tell you
when a fix ships and credit you in the release notes unless you ask us not to.

## Supported versions

Fixes go onto the latest release. There are no maintained older release lines
yet; when that changes, this section will say so.

## What is in scope

moai reads and writes files in a repository you already control, so the
interesting boundaries are the ones where content that is not yours becomes
behaviour:

- Content of `.moai/issues.jsonl`, `.moai/journal/*.jsonl` and the older
  `.moai/journal.jsonl` — these come in over merges and pulls, so a line written
  by someone else must never become a command, a path outside the repository, or
  a crash that loses other lines. A journal file is named from the writer's
  email, so that folding is a path-safety boundary too.
- `moai merge-driver`, which git invokes with paths during a merge.
- `moai hook`, which reads events on stdin and decides what to answer.
- `install.sh` and the release artifacts: a downloaded archive that does not
  match `SHA256SUMS` must never be installed, and there is deliberately no flag
  to skip that check.

Out of scope: anything that requires an attacker to already be able to run
commands as you, and the contents of the repository you point the tool at.

## What the installer guarantees

`install.sh` downloads the archive and `SHA256SUMS` from the GitHub release,
compares them, and stops if it cannot. It also refuses to overwrite an existing
`moai` unless you pass `--force`. Signed artifacts and an SBOM are planned; they
are not there yet, so today the trust root is the GitHub release itself.
