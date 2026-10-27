# Run the web suite without forcing the test runner to exit

## What happened

- The web suite was run with `--test-force-exit` to avoid waiting for request timers. It reported 434 passes while omitting about 45 tests registered after later top-level awaits.
- The review ran plain `npm test` and measured 481 passes, including the previously skipped concurrency cases.

## Do this instead

- Run plain `npm test` and wait for natural completion. A forced exit can end discovery as well as clean up open handles.
- If a suite is slow, diagnose the timers separately; do not replace the required suite with a run that ends discovery early.
- When new tests are added, confirm their names and the expected test count appear in the results before reporting them as passed.

Learned in: moai-ijjd.56c
