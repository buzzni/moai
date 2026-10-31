# Require complete context for irreversible derived writes

## What happened
- An archive reader returned readable rows alongside Load.errors. The due priority writer discarded the diagnostics and computed group closure from only those rows.
- A damaged year file could hide finished members, make a shipped group appear open and raise parked backlog to p0. Repairing the file would not lower that priority.

## Why
- A successful Result described a lenient read, not a complete snapshot. The caller used that partial answer for a write that only raises and never reverses itself.

## Do this instead
- When a derived decision causes an irreversible update, inspect the context reader's diagnostics as well as its Result.
- Refuse or explicitly isolate decisions whose required context is incomplete; do not report a complete successful decision from a partial read.
- Keep the policy local to the writer. Readable rows can remain available to lenient readers and unrelated writes that need no such context.
- Test the damaged required source, repair and retry, and assert that both snapshot and journal stay unchanged on refusal. Also cover an unrelated damaged source outside the read gate.

Learned in: moai-w30o.x9a
