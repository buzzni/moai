# Build reusable indexes only on paths that reuse them

## What happened

- A reusable wiki ranking index was added for repeated TUI queries.
- The one-shot CLI wrappers also built the complete index on each call, allocating entries for every distinct token before discarding them.
- Opening a wiki window built the index even when the person only browsed pages.

## Why

- Reusing the same scoring implementation does not require every caller to pay for a reusable cache.
- Result parity alone does not check when preprocessing happens or how much data a one-shot request retains.

## Do this instead

- Keep the scorer shared, and choose preprocessing according to the caller's lifetime.
- For one-shot queries, retain only query-token counts while preserving the full section population and field lengths used by scoring.
- Build a reusable index when its first query arrives, and invalidate it together with the immutable pages it describes.
- Compare both entry points with the previous scorer, and check that browsing without searching leaves the index unbuilt.

Learned in: moai-qr5u.59c
