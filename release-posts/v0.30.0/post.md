## moai v0.30.0
### What's new
- **`moai tui --json --deep --no-gantt` reads the tree without chart values**
- Deferred ancestors, including ancestors under a deferred group, no longer cap a descendant's planned dates, and `moai due apply` no longer raises a row by…
- **Open web tabs follow theme and time zone choices made in another tab**
### Fixes
- **TUI Gantt headers leave a space between date labels**
- **TUI Gantt plans shown from a sibling worktree stay unchanged here**
- `moai web` briefly drains a delayed request before closing a busy connection, so a client can receive the 503 response without a broken pipe while sending…
- **Skill shell commands keep repository paths together**
- **`moai-recover` leaves conversations moved off with `/resume` or `/continue` out of automatic recovery**
- The explorer header marks the current project with `▸`, so its position stays visible without colour or shimmer
- …and 13 more in the release notes

Release notes: <https://github.com/buzzni/moai/releases/tag/v0.30.0>
Update with `moai update` · install: <https://moai.buzzni.com/guide/#install>
