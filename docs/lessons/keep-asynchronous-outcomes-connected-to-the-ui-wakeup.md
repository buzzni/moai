# Keep asynchronous outcomes connected to the UI wakeup

## What happened
- Preview encoding moved to a bounded worker, but rejected requests and pending cache evictions carried no image cells that could invalidate the held layout.
- Completed pictures also waited for the slow file-watch cadence, even though the worker was already done.
## Why
- Returning a placeholder is not enough to preserve the request: a cache miss has no cell identity, and the event loop cannot infer background work from rendered cells.
## Do this instead
- Give every asynchronous outcome a path back to the owner loop: success, failure, queue refusal and eviction.
- Keep pending work visible to the loop's polling policy, and request a new layout when a request must be retried even if no result can name it.
- Test through the app's follow and polling entry points, with held layouts and real asynchronous results; helper-only tests do not pin the wakeup path.

Learned in: moai-zidx.ine
