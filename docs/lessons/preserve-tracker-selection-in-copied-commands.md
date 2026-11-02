# Preserve tracker selection in copied commands

## What happened
- Review finding 1 on moai-yigx.2hv found that an undo named a linked worktree with `-C` but omitted the selection override used by the original write.
- Pasting the command selected the main tracker's row with the same ID, so a relative undo could change the wrong tracker.
## Why
- A directory names where resolution begins; flags and environment can change which resource it resolves to.
- Quoting the path correctly does not preserve that selection contract by itself.
## Do this instead
- Build copied commands from the resolved resource and every option needed to select it again, including a meaningful environment override.
- Execute the generated command from another directory with duplicate IDs in both resources and verify only the intended resource changes.
- Resolve selection outside the write lock if determining it can read files or start subprocesses.

Learned in: moai-yigx.2hv
