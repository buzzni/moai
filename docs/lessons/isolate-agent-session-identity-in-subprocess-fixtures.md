# Isolate agent session identity in subprocess fixtures

## What happened

- A heartbeat fixture passed in one agent session but failed in another: a child command inherited a session ID and wrote a newer pick than the fixture.

## Why

- Session identifiers are active inputs to subprocesses, not just metadata; inherited values can alter which session holds a row.

## Do this instead

- Clear agent session identity variables in subprocess fixtures unless the test explicitly exercises them.
- When testing a session-dependent path, set the identity deliberately and verify the fixture is the one selected.

Learned in: moai-wbdh.7fs
