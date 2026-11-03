# Keep optional wakeup failures local

## What happened
- An optional self-pipe made terminal resizes wake an idle explorer sooner. A socketpair or signal-registration error propagated out of the input wait and ended the explorer.

## Why
- The optimization shared a Result with essential event parsing. Resource exhaustion in the optional setup was treated as failure of the whole input loop, despite an existing periodic fallback.

## Do this instead
- Separate optional wakeup setup from essential input errors. If an existing periodic path preserves correctness, keep that fallback when the optimization cannot initialize.
- Remember the failed optional initialization so an idle loop does not repeatedly allocate or register on every turn.
- Keep real parser and terminal failures distinct; an optional fallback is not a reason to discard unrelated errors.

Learned in: moai-lunf.nw0
