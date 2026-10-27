# Keep bounded client waits off the accept loop

## What happened

- A busy HTTP connection waited up to 200ms for a late request before closing.
- The wait ran in the single accept loop. N silent overflow clients delayed later connections by N times that bound.

## Why

- A deadline bounds one operation; it does not bound the queue of operations sharing a thread.
- Returning a response is also separate from ending it: delaying the write-side shutdown delays EOF for the client.

## Do this instead

- Send the response and shut its write side before waiting for late request bytes.
- Move client-dependent waits off the accept loop and cap the number of concurrent drainers. When that cap is full, close immediately rather than waiting for room.
- Keep a total deadline on each drainer and release its capacity on completion, panic or thread-spawn failure.
- Test that a queue of silent clients cannot delay the next accepted connection. Use enough queued waits to distinguish serialization from ordinary scheduling noise.

Learned in: moai-n8po.7wt
