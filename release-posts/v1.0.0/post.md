## moai v1.0.0
### What's new
- `moai daemon` and `moai ctl` run on Windows, over a named pipe open to your account alone
- `moai daemon start|stop|status` runs a per-user local daemon in the foreground (systemd, launchd or `&` keep it) and serves a JSON-RPC 2.0 API over a Unix…
- The daemon stops cleanly on SIGTERM and SIGINT, as it does on `moai daemon stop`
- Daemon `changes` reports issue edits only
- `moai update` reports a running daemon with a different version or protocol after a successful update
### Fixes
- `moai ctl` reports `busy`, not that the window is gone, when a window runs out of control client slots just as a request reaches it
- The skill's one-liner that pulls a review's text out of a subagent transcript reads and writes UTF-8 on Windows
- The editor opens the right file when `TEMP` is on a network share on Windows
- Windows builds carry an 8 MiB stack, as Linux does
- `moai status`, `show`, `prime`, `project`, `add`, `mv`, `edit`, `defer`, `rm` and `backlog promote` no longer panic when stderr is closed (a broken pipe)…
- `moai note`, `defer`, `rm`, `link`, `add` (with `--from` too), `wiki learn` and the explorer’s `SPC n` take their timestamp after they hold the tracker…

Release notes: <https://github.com/buzzni/moai/releases/tag/v1.0.0>
Update with `moai update` · install: <https://moai.buzzni.com/guide/#install>
