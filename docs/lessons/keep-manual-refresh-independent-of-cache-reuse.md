# Keep manual refresh independent of cache reuse

## What happened
- An unchanged source stamp allowed a manual refresh to reuse an old tree.

## Why
- The stamp alone does not capture clock-driven changes or a restarted server, and an explicit refresh promises a fresh read.

## Do this instead
- Restrict cache reuse to actions that change only presentation, and store each cached result with its own source stamp and view keys.
- Test a manual refresh at an unchanged stamp and while a cached read is in flight.

Learned in: moai-wrod.ud8
