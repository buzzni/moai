## moai v0.34.0
### What's new
- `moai web`: list rows tint completed work pale green, and gantt labels take matching tints for in-progress, review and completed work
- Plans accept the fullwidth `＠` as a date marker, with the same escaping rules as `@`
- `moai ready` orders equal-priority work by the earliest deadline it follows, including open ancestors
- Your own `moai edit`, and a plan you move on the explorer's or `moai web`'s gantt, marks the row you wrote read if its previous version was already read
- When the editor for a parked thought ends on a signal (killed or crashed), the explorer keeps its file and names the path instead of deleting what you saved
### Fixes
- `moai init` says when it could not measure the planted merge driver and left it as it was; `init --json` reports `"driver":"unmeasured"`
- `moai ready`, `moai prime` and the next row `moai mv … done` names follow only live rows' deadlines, so an archived epic's deadline no longer reorders its revived members
- Hook rule 2 lets a session write into a worktree that carries the pick-up mark of a row it picked up there, even when that worktree is named for another epic
- `moai edit` and explorer gantt moves take their `updated_at` once they hold the tracker lock, so a write that waited behind another session no longer stamps an older time
- Nested task lists in `moai web` retain their indentation
- TUI gantt undo reverses only the dates moved, and archive-only rows and archive copies cannot be dragged or planned
- …and 8 more in the release notes

Release notes: <https://github.com/buzzni/moai/releases/tag/v0.34.0>
Update with `moai update` · install: <https://moai.buzzni.com/guide/#install>
