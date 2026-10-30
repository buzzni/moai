# Control an open window

`moai ctl` connects to an explorer or web tab you already opened. It never opens a window or controls a terminal multiplexer. Start `moai tui` in one terminal, then run `moai ctl ls` in another to see its window id. `moai ctl state --json` reads the most recently used window in the current repository; `moai ctl state -t <id> --rows --json` reads a specific window and includes visible row ids.

`moai ctl ls --all` includes other repositories. Without `--all`, `ls` lists only windows in the current repository. A missing window gives `no_window` for `state`; it never silently chooses a window in another repository. From outside a repository, the most recently used window across repositories is the default.

The transport is a Unix socket in `MOAI_CTL_DIR`, otherwise `$XDG_RUNTIME_DIR/moai`, otherwise the system temporary directory's `moai-<uid>` folder. The folder must belong to your user and have no group or other access. The socket is `<pid>.sock`; no metadata file is written. The listener removes its socket on exit, and discovery cleans up stale sockets only after verifying their type and owner. Requests and replies are one JSON object per line, limited to 1 MiB. Requests carry `proto: 1`; unknown versions are refused.

On Windows, `ctl ls` returns an empty list and commands that need a window report `unsupported`. Named pipes remain to be designed.
