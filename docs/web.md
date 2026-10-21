# Open the tracker in a browser

`moai web` serves the tracker as a page on your own machine: a board, a list and
the repository's wiki, with the issue under the cursor described below or beside
it. It reads, and only reads. The page is the [explorer](explorer.md) for a
browser tab — same words, same filter syntax, same keys where a browser allows —
so that a person who would rather not live in a terminal can still see how the
work stands.

## Start it

    moai web                  http://127.0.0.1:6624/ - open it in a browser
    moai web --port 0         let the system pick a free port
    moai web --dry-run        say where it would serve; bind nothing

The first line names the address and the tracker it serves. Open that address
yourself — moai never launches a browser. `Ctrl-C` stops it; there is nothing to
clean up.

- **`--port <n>`** is 6624 unless given. `--port 0` asks the system for a free
  port and prints the one it got. A port that is taken stops it with the code
  `port_in_use`
- **`--host <addr>`** is `127.0.0.1` unless given. Any address that is not a
  loopback one is served with a warning, because anyone who can reach that port
  can read the whole tracker
- **`--json`** prints the first line as one value, `{url, host, port, root,
  here}`, and then serves. A script that starts the page reads its address there
- **Inside a linked git worktree**, it serves the main checkout's tracker, the
  same one every other command reads

## What you see

The page has three tabs.

| Tab | Shows | Switch |
|---|---|---|
| **Board** | A kanban: one column per status, with backlog and deferred columns and one lane per [milestone](glossary.md#milestone) | `SPC g b` |
| **List** | The tree of milestones, [epics](glossary.md#epic) and issues, which you step into and fold | `SPC g l` |
| **Wiki** | The pages of this wiki: the page list on one side, the page on the other | `SPC g w` |

Click a tab, or use the keys. Switching tabs keeps the list's cursor, filter and
sort.

Choosing an issue shows its detail: column, priority, assignee, tags, what blocks
it, the body, its children and its history. The detail sits below the list, or on
its left when the window is wide enough; `SPC o d` moves it and `SPC v d` hides
it. `SPC v r` switches the body between rendered markdown and the raw text.
`Enter` on an issue moves the focus to the detail, and `SPC w w` moves it back.

## Search, filter, sort, view, column

- **Search** (`/`) searches as you type, in the scope the select beside it shows:
  all, id, title, tag or body. `Tab` cycles the scope. Notes are not searched.
  `Enter` keeps the search and returns to the list; `Esc` closes it
- **Filter** (`SPC f`) takes the same `key=value` words as `--filter`. `Tab`
  completes, `Up`/`Down` pick a value and `Enter` applies. `Esc` in the list
  clears an applied search or filter
- **Sort** (`SPC s`) works as in the explorer: `p c u s a t` sorts by priority,
  created, updated, status, assignee or title, and the same key again turns it
  around. The capital letter adds the field to the end of the order. `SPC s e`
  opens the sort editor
- **View** (`SPC v`) shows or hides deferred work (`l`), backlog items (`b`) and
  the archive (`o`); `a` shows everything but the archive, `1`~`9` toggle the nth
  status column
- **Column** (`SPC c`) picks what a list row carries: `i` id, `p` priority,
  `a` assignee, `c` created, `u` updated, `n` counts, `t` tags, `e` epic,
  `h` names

The page remembers its tab and where the detail sits in the browser's own local
storage. Search, filter, sort, view and column last until you close the tab.

## It only reads

Nothing on the page changes the tracker. Every request the page makes is answered
by running `moai --json <command>` in a child process and passing its answer on,
so what you see is exactly what the CLI says. Writing stays where it was: the CLI
for moving, editing and noting, and the explorer's `SPC n` for a thought you want
to park.

Nothing is stored for the page either. The list is read again when you ask, and
the page filters, sorts and groups it itself, so keys answer at once.

## It follows the tracker

When someone writes the tracker — you in another terminal, a worker in another
window — the page notices and reads again. It asks the server for a short
stamp, folded from the files the explorer watches (the snapshot, the journal, the
archive and the config), and reloads the list only when the stamp has changed. A
tracker nobody touches costs one tiny question and no reread.

## See it from another machine

Leave the server on `127.0.0.1` and bring the port to you:

    ssh -L 6624:127.0.0.1:6624 you@the-machine

Then open `http://localhost:6624/` on your own machine. This is the way to go
rather than `--host 0.0.0.0`, which serves the tracker to anyone on the network.

Whatever it binds, the server answers only requests whose `Host` header is
`localhost` or an IP address. A request with another name — what a page on some
other site would send after pointing its own name at your machine — is refused
with `forbidden_host`. Putting the page behind a proxy that rewrites the name
needs the proxy to send `Host: localhost`.

## When it refuses

A refusal is JSON with a stable `code`, like every other failure. `port_in_use`
comes from the start. The rest come from the server while it runs:

| Code | Means |
|---|---|
| `forbidden_host` | The `Host` header was not `localhost` or an IP address |
| `method_not_allowed` | Anything but `GET`; the page never writes |
| `busy` | Too many requests at once; try again |
| `timeout` | The child `moai` did not answer in time |
| `too_large` | The child's answer was larger than the server will pass on |

## Keys

Typing in an input (search, filter) turns the single-key shortcuts off. Outside an
input the letter keys follow the key's position, so a Korean input source works:
`ㅓ` is `j`. `SPC` opens a menu showing what each next key does; `Esc` or `SPC`
closes it, `Backspace` goes up one level, and clicking an item is the same as
pressing its key. `?` shows the overlay of every key.

**Move**

| Key | Does |
|---|---|
| `j` `Down` / `k` `Up` | One line down / up (board: within the column) |
| `gg` `Home` / `G` `End` | Top / bottom |
| `Ctrl-d` / `Ctrl-u` | Half page down / up; the cursor stays |
| `Ctrl-f` `PageDown` / `Ctrl-b` `PageUp` | Page down / up |

**Step in and fold**

| Key | Does |
|---|---|
| `Enter` | Step into a group; on an issue, focus the detail |
| `Backspace` | Step out one level |
| `l` `Right` / `h` `Left` | Unfold / fold (board: column to the right / left) |
| `Tab` | Unfold / fold everything under the group (list) |

**Panes and tabs**

| Key | Does |
|---|---|
| `SPC w w` / `SPC w W` | Next / previous pane |
| `SPC w h` `j` `k` `l` | Pane to the left / below / above / right |
| `SPC g l` `b` `w` | List / board / wiki tab |
| `SPC o d` | Detail position: left or below |

**Search, filter**

| Key | Does |
|---|---|
| `/` or `SPC /` | Search |
| `SPC f` | Filter |
| `Esc` | Clear the applied search or filter |
| `?` or `SPC ?` | Shortcut overlay |

**View, sort, column** (`SPC v`, `SPC s`, `SPC c`) work as described above.

**Inside the search or filter input**

| Key | Does |
|---|---|
| `Enter` / `Esc` | Apply / cancel and return to the previous cursor |
| `Tab` / `Shift-Tab` | Cycle search scope, complete a filter |
| `Up` / `Down` | Pick a filter value |
| `Ctrl-u` | Clear the input |

**Sort editor** (`SPC s e`)

| Key | Does |
|---|---|
| `j` `k` | Move in the list |
| `J` `K` | Move the field later / earlier in the order |
| `SPC` | Add / remove the field |
| `d` | Flip direction |
| `Enter` / `Esc` | Apply / cancel |

**Wiki tab**

| Key | Does |
|---|---|
| move keys | Move in the focused side (page list or page) |
| `l` `Right` | From the page list to the page |
| `Enter` | Open the page (list); follow the chosen link (page) |
| `Tab` / `Shift-Tab` | Next / previous link in the page |
| `Backspace` `h` `Left` | Back along the path you came by |
| `/` | Search titles and bodies |
| `SPC v r` | Raw / rendered page |

The page is tied to the one tracker the server was started in: it does not switch
between projects, show the stats screen, or write read marks.

Decided in: moai-0aa7, moai-jshy
