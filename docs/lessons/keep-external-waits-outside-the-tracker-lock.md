# Keep external waits outside the tracker lock

## What happened
- A post-write read-mark update waited on a separate file lock while the tracker lock remained held, blocking unrelated writes.

## Why
- A callback after snapshot persistence can still execute inside the tracker critical section; another lock or slow filesystem can delay every writer.

## Do this instead
- Carry the exact committed version out of the tracker critical section before touching user configuration, read marks, subprocesses, or other potentially slow resources.
- Test with the secondary resource blocked and verify another tracker write still succeeds.

Learned in: moai-uwi9.w5o
