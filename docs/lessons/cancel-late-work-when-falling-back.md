# Cancel late work when falling back

## What happened
- Browser identity initialization gained a timeout and a local-storage fallback, but an IndexedDB transaction could still resume after the fallback had chosen an identity.
## Why
- Settling a promise does not cancel work already in progress. A late write can conflict with the value the fallback has returned to its caller.
## Do this instead
- When a deadline selects a fallback, cancel pending writes where possible and guard callbacks against running after abandonment.
- Test a delayed operation after the fallback returns, including its side effects, and consider operations that have already committed before cancellation.

Learned in: moai-xvw3.vnd
