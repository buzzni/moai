# Control an open window

`moai ctl` connects to an explorer or web tab you already opened. It never opens a window or controls a terminal multiplexer. Start `moai tui` in one terminal, then run `moai ctl ls` in another to see its window id. `moai ctl state --json` reads the most recently used window in the current repository; `moai ctl state -t <id> --rows --json` reads a specific window and includes visible row ids.

`moai ctl ls --all` includes other repositories. Without `--all`, `ls` lists only windows in the current repository. A missing window gives `no_window` for `state`; it never silently chooses a window in another repository. From outside a repository, the most recently used window across repositories is the default.

The transport is a Unix socket in `MOAI_CTL_DIR`, otherwise `$XDG_RUNTIME_DIR/moai`, otherwise the system temporary directory's `moai-<uid>` folder. The folder must belong to your user and have no group or other access. The socket is `<pid>.sock`; no metadata file is written. The listener removes its socket on exit, and discovery cleans up stale sockets only after verifying their type and owner. Requests and replies are one JSON object per line, limited to 1 MiB. Requests carry `proto: 1`; unknown versions are refused.

On Windows, `ctl ls` returns an empty list and commands that need a window report `unsupported`. Named pipes remain to be designed.

## Press keys and run actions

`moai ctl keys 'SPC g b'` presses the explorer's current key bindings. Quote each sequence; the spelling is the same as `keys.toml` and `moai keys`. Several sequences run in order. `--text 'parser'` pastes literal text into a field opened by an earlier control request, for example `moai ctl keys /` followed by `moai ctl keys --text 'parser'` and `moai ctl keys Enter`.

`moai ctl do browse.go_list` runs a named action even if the person changed its key binding. `moai ctl actions --json` lists the current `browse`, `gantt`, and `moves` actions with their bindings and whether each is enabled. Plain `actions` output marks disabled actions with `·` and `(off: …)`. The name `browse.read_all` and its default keys `SPC m a` both mark all visible NEW rows read. Column moves and notes use `moai mv` and `moai note`.

Control requests wait until their keys have run and return the resulting screen, mode, menu, cursor, notice, and whether they changed anything. They never count as human input when selecting the most recently used window. A request cannot take over a field or unfinished key sequence the person started, open an editor, or close the person's window. A field opened by a control request remains available to later control requests until the person types in it.

Tracker writes made through the explorer carry the calling agent's identity. Pass `--user 'Name (email)'` or set `MOAI_ACTOR` when Git identity is not configured; a write with no caller identity returns `no_actor` and never opens the explorer's identity prompt. The window owner's read marks remain theirs. A future action that moves columns must honor the same ownership rule as `moai mv`: work assigned to someone else requires an explicit takeover.

## Change the view by name

These commands act on the selected open window. Use `-t <id>` to choose one explicitly. They return the small state from `keys` and any verb-specific result. Values are checked by the CLI and by the window, since a window may receive a request from an older CLI.

| Command | Example | Effect |
|---|---|---|
| `sort` | `moai ctl sort 'title:desc,priority:desc'` | Set the current project's order; `--default` also changes the default. |
| `filter` | `moai ctl filter 'status=todo tag=parser'` | Apply a filter; `--clear` removes it. |
| `find` | `moai ctl find 'parser' --in title` | Search `all`, `id`, `title`, `tag`, `body`, or `note`. |
| `screen` | `moai ctl screen board` | Open `list`, `board`, `gantt`, `stats`, or `wiki`. |
| `view` | `moai ctl view deferred on` | Set `deferred`, `backlog`, `archived`, `worktree`, `raw`, or `column:<name>` on or off. |
| `cell` | `moai ctl cell assignee off` | Show or hide a list field, including `id`, `priority`, `tally`, `tags`, `epic`, `names`, and `branch`. |
| `detail` | `moai ctl detail right` | Show or hide the detail, or place it `right`, `bottom`, `left`, or `top`. |
| `goto` | `moai ctl goto moai-1234 --enter` | Select a row without clearing a filter; `--project <number or path>` selects a project. |
| `read` | `moai ctl read --all` | Mark the window owner's NEW rows read; also accepts ids or `--group <id>`. Use when the person asks. |
| `wiki` | `moai ctl wiki 'guide#start'` | Open a page and heading; `--search 'words'` opens the top ranked hit and returns all hits. |
| `pane` | `moai ctl pane --list-width 60` | Set `--list-width`, `--list-height`, `--wiki-width` percentages or `--gantt-label` columns; `none` restores a default. |
| `zone` | `moai ctl zone Europe/Paris` | Set the explorer timezone; `none` restores UTC. |
| `theme` | `moai ctl theme dark` | Supported by web tabs; the explorer returns `unsupported` because terminal colours belong to the terminal. |

`pane` changes proportions inside the explorer. It does not resize the terminal or browser; use the terminal's window manager or `tmux resize-pane` for that. View, cell, detail, pane, sort, and zone choices are saved in the person's config and may appear again when they reopen the explorer; `raw` is a session-only view toggle. Every change leaves a `⇄ ctl` notice in the window.
