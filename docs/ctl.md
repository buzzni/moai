# Control an open window

`moai ctl` connects to an explorer or visible web tab you already opened. It never opens a window or controls a terminal multiplexer. Start `moai tui` or `moai web`, then run `moai ctl ls` in another terminal to see its window id. A web server lists each visible tab separately. `moai ctl state --json` reads the most recently used window in the current repository; `moai ctl state -t <id> --rows --json` reads a specific window and includes visible row ids.

`moai ctl ls --all` includes other repositories. Without `--all`, `ls` lists only windows in the current repository. A missing window gives `no_window` for `state`; it never silently chooses a window in another repository. From outside a repository, the most recently used window across repositories is the default.

The transport is a Unix socket in `MOAI_CTL_DIR`, otherwise `$XDG_RUNTIME_DIR/moai`, otherwise the system temporary directory's `moai-<uid>` folder. The folder must belong to your user and have no group or other access. The socket is `<pid>.sock`; no metadata file is written. The listener removes its socket on exit, and discovery cleans up stale sockets only after verifying their type and owner. Requests and replies are one JSON object per line, limited to 1 MiB. Requests carry `proto: 1`; unknown versions are refused.

A web tab holds one event stream while it is visible. Hiding or closing the tab ends that stream, removes its id from `ctl ls`, and promptly returns `gone` to waiting ctl calls. A tab that shows again gets a new id and token. The server sends ctl requests through that stream and accepts a JSON reply only from the same-origin page with the tab token; it limits the reply body to 1 MiB. If the local ctl socket cannot open, the web page still serves normally and the server says why on stderr.

On Windows, `ctl ls` returns an empty list and commands that need a window report `unsupported`. Named pipes remain to be designed.

## Press keys and run actions

`moai ctl keys 'SPC g b'` presses the explorer's current key bindings. Quote each sequence; the spelling is the same as `keys.toml` and `moai keys`. Several sequences run in order. `--text 'parser'` pastes literal text into a field opened by an earlier control request, for example `moai ctl keys /` followed by `moai ctl keys --text 'parser'` and `moai ctl keys Enter`.

`moai ctl do browse.go_list` runs a named action even if the person changed its key binding. `moai ctl actions --json` lists the current `browse`, `gantt`, and `moves` actions with their bindings and whether each is enabled. Plain `actions` output marks disabled actions with `·` and `(off: …)`. The name `browse.read_all` and its default keys `SPC m a` both mark all visible NEW rows read. Column moves and notes use `moai mv` and `moai note`.

Web tabs use the names from their own key table, such as `tab.gantt` and `read.all`. `keys 'SPC g b'` presses the web page's key router. `moai ctl theme dark` changes that tab's theme and remembers it in the browser; the browser's other open tabs follow the stored theme. The key help, a human-focused input or menu, a half-typed key sequence such as the `g` of `g g`, or an open sort editor gives `person_busy`; what a ctl request leaves open stays ctl's until the person presses a key or clicks. A search or filter opened by ctl accepts a later `keys --text`. Keys use the explorer's spelling (`Ctrl-d`, `Shift-Tab`) on both surfaces. On a web tab, `do` refuses an action that `actions` marks off, including one whose key carries an argument; press its keys instead.

Control requests wait until their keys have run and return the resulting screen, mode, menu, cursor, notice, and whether they changed anything. They never count as human input when selecting the most recently used window. A request cannot take over a field or unfinished key sequence the person started, open an editor, or close the person's window. While the person's editor holds the terminal, the explorer stays in `ctl ls` and actions answer `person_busy`. A web tab discards a request whose deadline passed before it arrived. A field opened by a control request remains available to later control requests until the person types in it.

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
| `pane` | `moai ctl pane --list-width 60` | Set `--list-width`, `--list-height`, `--wiki-width` percentages or `--gantt-label` character columns in the explorer. On the web, `--list-width` is converted from list share to detail share; `--list-height` applies with detail above or below, and `--gantt-grid <px>` sets the Gantt name grid in pixels. The other surface's Gantt unit returns `unsupported`; `none` restores a default. |
| `zone` | `moai ctl zone Europe/Paris` | Set the explorer timezone; `none` returns to this machine's zone, as when no timezone is saved. |
| `theme` | `moai ctl theme dark` | Supported by web tabs; the explorer returns `unsupported` because terminal colours belong to the terminal. |

`pane` changes proportions inside the explorer. It does not resize the terminal or browser; use the terminal's window manager or `tmux resize-pane` for that. View, cell, detail, pane, sort, and zone choices are saved in the person's config and may appear again when they reopen the explorer; `raw` is a session-only view toggle. Every change leaves a `⇄ ctl` notice in the explorer.

On the web, view choices are saved in that browser's local storage, not in the user's moai config. The web has no `stats` screen, no `--wiki-width`, and no `--gantt-label`; those return `unsupported`. Web search covers all fields, so `find --in title` and other restricted scopes return `unsupported`. `zone none` follows the browser or TUI default.

## Suggest something to the person

`moai ctl propose 'Show the remaining parser work?' --then 'filter tag=parser status=todo'` puts a suggestion in one visible explorer or web tab. The person accepts it with `SPC a y`, declines with `SPC a n`, or opens the list with `SPC a l`. The explorer shows a line below its banner; a web tab shows a card at the lower right. A suggestion never takes focus from an input or changes the person's current screen before they accept. An accepted `--then` runs at that moment through the window's ordinary ctl action path, using the window owner's identity for a write. What it opens is the person's: ctl cannot type into a field an acceptance opened, and a person who writes in their editor gets their editor. The answer contains `state` and, when there was a follow-up action, its result under `ran`.

`--then` accepts a named view verb or `do <action>`; it does not accept raw keys, another proposal, or a tracker column move or note. For a column move, suggest it without `--then` and wait for the person's choice. After acceptance, use `moai mv <id> <status> --from <seen status>` yourself. If the row belongs to someone else, use `--take -m 'accepted proposal <proposal id> in the explorer'`; accepting the suggestion is the person's permission for that action. Notes likewise use `moai note`.

By default, `propose` returns a pending id immediately. `--wait <seconds>` keeps the command open until the person answers or the wait ends. A wait ending first returns `pending` and leaves the suggestion visible. `moai ctl answer <id> [--wait <seconds>]` reads it later, and `moai ctl withdraw <id>` takes down a pending one. Accepted, declined, expired, withdrawn, and pending answers all exit successfully; a missing or closed window and a proposal refused before display do not. The id includes the window id, so `answer` and `withdraw` can find the same window. A closed window returns `gone` to its waiters.

If a waiting command disconnects, its waiter slot is released promptly; the proposal itself stays pending until answered, withdrawn, or expired. A proposed time-zone change is checked by the target explorer or browser before the suggestion appears, so an unsupported zone is refused at once.

A window holds up to five pending suggestions. Text is limited to 500 characters and three lines, with control characters removed. `--ttl` defaults to 900 seconds and is limited to 3600; an unanswered suggestion expires and disappears. Answered suggestions remain readable for ten minutes. Waiting callers have four slots separate from ordinary ctl requests, so a set of unanswered suggestions does not stop `state` from answering.

Decided in: moai-zgc1 moai-v5of
