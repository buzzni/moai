# Kill a process group only with kill -s KILL -- -PGID

## What happened

A test killed its child's process group with `Command::new("kill").args(["-9", "-<pgid>"])`. Every process the user owned — all agent sessions, tmux, the editor server — died at the moment that test binary started, twice in one day. It looked like an out-of-memory kill or a server restart. (moai-3d6j.idr)

## Why

procps-ng `/usr/bin/kill` (4.0.4) reads only the **first digit** of a negative pid that follows a signal option: `-9 -155416` becomes pid `-1`, `-KILL -155416` too. Pid `-1` means every process the user may signal.

## Do this instead

- Kill a process group only as `kill -s KILL -- -<pgid>` — in tests, scripts and briefs alike
- Better still, signal the group from code (`libc::kill(-pgid, SIGKILL)` or the standard library) instead of shelling out
- When every session dies at once, first look at which test or command started at that second, before blaming memory
