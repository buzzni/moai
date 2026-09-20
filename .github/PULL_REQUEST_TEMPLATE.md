<!--
Three headings, because a reviewer needs three things: what moved, why it had to
move, and what you actually ran. Delete the parts that do not apply — an empty
heading is worse than no heading.
-->

## What

<!-- One or two sentences. The diff already lists the files. -->

## Why

<!--
The part the diff cannot show. If a decision here should not be reverted later,
say so and say why — that sentence is the reason this section exists.
-->

## Verification

<!--
What you ran, and what it said. "Tests pass" is not verification; `cargo test`
with the result is.

  cargo test
  cargo clippy --all-targets -- -D warnings
-->

---

- [ ] The commit subject names the issue id, if there is one — `fix(view): … (moai-xxxx)`
- [ ] `docs/cli.md` regenerated, if any `--help` text changed (`scripts/gen-cli-docs.sh`)
- [ ] No changes under `.moai/` — that is this repository's own tracker, and CI
      rejects it from a fork. Describe the issue here instead.
