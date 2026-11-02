# Update every lifetime reader when adding a reservation

## What happened
- Proposal claims ignored the original TTL, but wait_while still slept until that TTL. Once it passed, a missing result made the waiter busy-spin.
- The stream cap released a disconnected tab's capacity while its ctl identity and suggestions remained resumable. An overflow tab could steal its place during reconnect.
- Claimed rows escaped both the outstanding count and retention cleanup when no result arrived.

## Why
- A reservation changes the lifetime of a resource. Updating the acceptance path alone leaves other readers using the old lifetime.
- Timeout, capacity and cleanup decisions then disagree even though each local branch looks consistent.

## Do this instead
- Enumerate every reader of the resource lifetime: expiry, waiter deadline, admission, reconnect, cancellation, outstanding count and pruning. Change them together when adding a reserved state.
- Serialize admission and reservation ownership under the same lock. Count detached resources until their explicit resume deadline, and let a valid resume reuse its existing reservation.
- Give incomplete claims a bounded result deadline and keep them in the outstanding limit. Distinguish an unknown execution result from permission to execute again.
- Test across the old deadline, a competing admission during the reconnect gap, and a result that never arrives. Count wakeups and retained rows as well as returned values.

Learned in: moai-4xmq.sqe
