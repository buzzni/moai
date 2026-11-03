# Preserve every consumer when narrowing shared context

## What happened
- A partial archive reader kept placement and group-closure dependencies for a deadline edit but dropped the edited row's blocker context.
- The same context also rendered the edit result. A revived blocking epic whose finished members remained archived appeared empty and held instead of freed.
- The full reader had supplied both consumers. A helper-only gate test and broad randomized comparisons missed this particular combination; an end-to-end CLI regression exposed it.

## Why
- The condition that opens a data source and the dependencies that must come along once it is open answer different questions.
- An optimization followed the primary calculation's dependencies without enumerating the other readers of its shared result.

## Do this instead
- Before narrowing shared context, enumerate every consumer, including rendered output and error diagnostics, and preserve the union of their dependencies.
- Separate dependencies that open a source from those that ride along once it is open. A blocker can need archived group members for display without being allowed to open the source on its own.
- Compare the actual command's output with the full-context baseline. Exercise a dependency used only by a secondary consumer, and verify source-access budgets at the real call site as well as in the helper.

Learned in: moai-4slm.exm
