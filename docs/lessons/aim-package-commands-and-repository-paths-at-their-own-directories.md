# Aim package commands and repository paths at their own directories

## What happened
- Commands targeting a nested web package and file edits using repository-relative paths repeatedly executed from incorrect worktree directories.

## Why
- The package manager requires execution within the specific package directory, whereas repository-relative file paths resolve from the worktree root.

## Do this instead
- Specify an explicit absolute working directory for every command invocation.
- Execute package-level commands inside the package directory and resolve repository-relative paths against the worktree root.

Learned in: moai-cij1
