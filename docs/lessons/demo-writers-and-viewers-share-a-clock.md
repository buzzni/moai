# Keep every demo writer and viewer on the same UTC clock

## What happened

- A fixed demo date reached the seed and TUI, while later plan edits still used the real clock (moai-87vm.3hr).
- Equivalent timestamps with an offset were normalized in the seed but passed unchanged to a viewer that compares stored timestamps as text.
- The shared seed also accepted the fixed date for a browser recorder whose client clock could not follow it.

## Why

- Fixing the first writer does not fix later writes or the code that decides what counts as today.
- Shared setup affects every recorder that consumes it, including paths outside the recording being changed.

## Do this instead

- Trace the demo clock through every writer and viewer before accepting a fixed-time option.
- Normalize it once per process to RFC3339 UTC with a `Z` suffix; reject invalid values before writing or starting a viewer.
- Check an equivalent offset timestamp and verify that later edits and read markers use the normalized instant.
- Clear the option or reject it explicitly on paths whose clocks cannot follow it, including browser clients.

Learned in: moai-87vm.3hr
