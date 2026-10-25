# Resolve a merge conflict in the worktree, never in a shared checkout

## What happened

A merge of an epic branch stopped on a conflict in the root checkout, and it was resolved right there. A review later reproduced the harm: a half-resolved merge in the shared root is sealed by another session's commit that names no path, under that session's subject. (moai-hdrd)

## Why

The root (and a milestone worktree) is shared by every session of the repository. While `MERGE_HEAD` stands there, any commit made beside it completes that merge.

## Do this instead

- When `git merge --no-ff worktree-<epic>` stops on a conflict in a shared checkout, run `git merge --abort` there
- Go back into the epic's worktree, merge the base branch in, resolve, run the tests
- Then merge into the shared checkout again — it goes through clean
- Pulling the base into the worktree before the merge avoids the conflict in the first place
