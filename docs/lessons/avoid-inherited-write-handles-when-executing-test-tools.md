# Avoid inherited write handles when executing test tools

## What happened

- A test wrote a fake executable in the test process and immediately ran it. Parallel tests could fork while its write handle was open, making execution fail with ETXTBSY.

## Why

- A different child may inherit the write handle and keep the executable open after the writer closes it.

## Do this instead

- Reuse a test helper that has a short-lived child copy the source to the executable path and set its permissions before execution.
- Keep the executable writer shared across tests so the workaround is not duplicated.

Learned in: moai-rkre.6g7
