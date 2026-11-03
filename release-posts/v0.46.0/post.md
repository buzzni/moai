## moai v0.46.0
### What's new
- `moai backlog promote --body` sets the new epic’s body while preserving the original backlog context
- `moai watch` follows supervisor note heads with resumable cursors, and `moai supervise message` prints Codex assignments without controlling sessions
- Backlog promotion carries the original body to a sole member and adds source references to split members
- Epic review grades follow `[models] review_grade`, with escalation opt-in through `review_raise`
- `moai web` budgets live event streams per browser
### Fixes
- Editing an issue reads archived members of a live blocking group, and hook ownership checks let unreadable live IDs take precedence over stale archived copies
- `moai read --atomic` now accepts `--all` and `--epic`
- The explorer no longer repeats a read-mark notice while it retries a read file it could not read

Release notes: <https://github.com/buzzni/moai/releases/tag/v0.46.0>
Update with `moai update` · install: <https://moai.buzzni.com/guide/#install>
