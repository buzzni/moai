# Keep test barriers reachable after a setup panic

## What happened

- A HOME isolation test had two workers meet at barriers after creating and checking their directories. A setup assertion or file write could panic before the first barrier, leaving the other worker blocked forever.

## Why

- A barrier counts arrivals rather than successful workers. Joining a panicked worker does not release its peer, so an intended assertion failure becomes a hung test suite.

## Do this instead

- When a test uses barriers, keep every participant able to reach every rendezvous after a recoverable setup panic. Capture setup with catch_unwind, pass the barriers, then resume the original panic.
- Keep both workers alive until they have observed each other's files; otherwise teardown can hide a shared-directory leak before the assertion reads it.

Learned in: moai-fcj3.8wg
