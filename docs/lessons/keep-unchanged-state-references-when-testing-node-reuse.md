# Keep unchanged state references when testing node reuse

## What happened
- A Gantt test expected a row node to survive a cursor change, but it supplied a fresh initial state and triggered a model rebuild instead.

## Why
- Rendering uses reference identity for state inputs such as matched results and boot data. Recreating those inputs changes more than the cursor.

## Do this instead
- Reuse the same state and index references when checking a cursor-only render; change only the cursor. Test scrolling and refreshed indexes as separate paths.

Learned in: moai-skjr.4i6
