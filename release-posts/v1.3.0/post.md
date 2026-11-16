## moai v1.3.0
### What's new
- `moai ctl ls` keeps listing the windows that answered and reports any it skipped, on stderr or in a `skipped` array in `--json`
### Fixes
- A backlog item saved with `SPC n` in the explorer no longer shows up as [NEW] to the person who wrote it
- `moai show` counts a live member once when an older archive copy shares its ID
- `moai ctl` reports a full window as busy instead of saying there is no window
- `moai daemon install` keeps a symlinked service file a link and keeps its private permissions
- `moai daemon install` refuses a socket path that would be too long once made absolute, instead of registering a daemon that can never start
- `moai update` no longer restarts the daemon service in the middle of an install or uninstall
- `moai update` restarts the service even when another daemon answers on your shell's socket, and tells you that daemon still runs
- A service-started `moai daemon start` that finds the socket taken now fails, so systemd and launchd notice
- The web Gantt tab keeps scrolling inside the chart; in narrow panes the toolbar and legend stay reachable

Release notes: <https://github.com/buzzni/moai/releases/tag/v1.3.0>
Update with `moai update` · install: <https://moai.buzzni.com/guide/#install>
