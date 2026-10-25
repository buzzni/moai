# Restore swapped sources with git checkout HEAD --, not git checkout --

## What happened

A benchmark swapped sources with `git checkout <base> -- src/` to measure two versions, then restored with `git checkout -- src/`. The restore brought back the base, not the work, and `git diff` showed nothing. It happened twice in one session — once when the script timed out mid-run, once even though the trap ran. A commit made beforehand saved the work both times. (moai-jk2u)

## Why

`git checkout <ref> -- <path>` writes the index as well as the working tree. `git checkout -- <path>` restores from the index, which now holds `<ref>`. With index and tree equal, `git diff` is empty and the loss is silent.

## Do this instead

- Commit before swapping versions; then `git checkout HEAD -- <path>` always gets back
- A restore or trap uses `git checkout HEAD -- <path>`, never `git checkout -- <path>`
- Do not run a swap that can die midway in the background; run it in front with a generous timeout
- After restoring, read `git status --short` — the index column too; `git diff` alone misses `M `
- Better: build both binaries first, copy them out of `target/`, and measure those without swapping sources
