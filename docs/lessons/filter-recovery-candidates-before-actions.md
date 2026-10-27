# Filter recovery candidates before offering any action

## What happened

- In moai-1m21, the recovery skill offered manual recovery for an unusual path before dropping sessions that were already back. That fallback could open a second window on the same conversation.
- The skipped-session summary also included already recovered sessions, making them eligible for the same fallback.

## Why

- A suggested manual command is still an action. A path check alone cannot determine whether recovery is appropriate.

## Do this instead

- Apply every exclusion before offering automatic or manual recovery. Never offer a session that is already back.
- Keep intentionally skipped conversations separate from live sessions when listing what the person can ask back.
- Apply path guards to shell reads as well as launches; a diagnostic command can expand shell syntax too.

Learned in: moai-1m21.8c9
