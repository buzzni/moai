# Check exit status separately from partial output

## What happened
- A regression fixture deliberately included an unreadable live row, but its helper required `show` to exit successfully before checking the displayed blocker.
## Why
- A command can emit useful partial output while reporting unreadable input with a nonzero exit status.
## Do this instead
- Check the expected exit status and diagnostic for the fixture separately from assertions on its output.

Learned in: moai-8rf1.hb3
