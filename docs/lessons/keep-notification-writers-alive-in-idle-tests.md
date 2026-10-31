# Keep notification writers alive while testing idle waits

## What happened
- The queued-request wake test returned immediately on its second idle wait even after draining the notification byte.
- The synthetic client thread owned the only write end of the wake socket and dropped it when it finished. The read end therefore reported hang-up, unlike the production listener that keeps a writer alive.

## Why
- Readability can mean either data or EOF. A fixture that drops its last writer changes the state being measured from idle to disconnected.

## Do this instead
- Keep a writer alive for the complete idle-wait assertion, matching the production owner lifetime.
- Exercise the actual enqueue-and-notify path, consume the queued request, drain the wake byte, and then assert a subsequent idle wait consumes its timeout.
- Test disconnect behavior separately when it is part of the contract.

Learned in: moai-4km9.9ra
