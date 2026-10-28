# Open the tracker in a browser

`moai web` serves the tracker as a page on your own machine: a board, a list and
the repository's wiki, with the issue under the cursor described below or beside
it. It reads the tracker and writes it in one place only — a Gantt bar
dragged to new days — besides your own read marks. The page is the [explorer](explorer.md) for a
browser tab — same words, same filter syntax, same keys where a browser allows —
so that a person who would rather not live in a terminal can still see how the
work stands.

## Start it

Run `moai web` inside a tracker. Its first line names the address and the
tracker it serves; open that address yourself — moai never launches a browser.
`Ctrl-C` stops it; there is nothing to clean up. `moai web --help` lists the
flags.

- **When the port is taken**, it stops with the code `port_in_use`. Give it
  another one with `--port`, or `--port 0` to let the system pick a free port —
  the first line says which it got
- **To serve an address other than the loopback one**, give `--host`. It warns
  that the address is not a loopback one, because anyone who can reach that port
  can read the whole tracker — read "See it from another machine" below first
- **A script that starts the page** passes `--json` and reads the address from
  the first line, which is then one JSON value
- **Inside a linked git worktree**, it serves the main checkout's tracker, the
  same one every other command reads

## What you see

Across the top stands the explorer's header: the moai logo, then `User` (who
the page reads as, named the way the project's `naming` says) and `Version`
(this moai, and what the explorer last heard of the latest release — the page
never asks GitHub itself). A short window leaves the header out and a narrow one
the logo, as the explorer does.

Beside them stand your [registered projects](explorer.md), numbered `<1>` to
`<9>` as in the explorer's header, the one on screen marked. Click one, press
its number, or pick it under `SPC p`, and the page reads that project instead:
the address becomes `/?project=<its path>`, so a reload or a bookmark stays on
it. The project the server was started in comes first when it is not
registered itself. Only registered projects can be read this way.

The page has four tabs.

| Tab | Shows | Switch |
|---|---|---|
| **Board** | A kanban: one column per status, with backlog and deferred columns and one lane per [milestone](glossary.md#milestone) | `SPC g b` |
| **List** | The tree of milestones, [epics](glossary.md#epic) and issues, which you step into and fold | `SPC g l` |
| **Gantt** | The tree beside a date axis: when each row was planned and when it actually ran | `SPC g g` |
| **Wiki** | The pages of this wiki: the page list on one side, the page on the other | `SPC g w` |

Click a tab, or use the keys. Switching tabs keeps the list's cursor, filter and
sort.

Choosing an issue shows its detail: column, priority, assignee, tags, what blocks
it, the body, its children, its commits, the wiki about it (the lessons its notes
point at, the pages that name it, the related sections — as `moai show <id>`
lists them) and its history. History and work log start expanded and keep the
line breaks of the text as written. In the list, in-progress rows have a pale gold
background and review rows a pale purple one. The detail starts on the right;
`SPC o d`, or the position button, moves it right → bottom → left → top. Below
640px it stays below the list. `SPC v d` hides it, and `SPC v r` switches the body
between rendered markdown and the raw text.
`Enter` on an issue moves the focus to the detail, and `SPC w w` moves it back.

## Gantt

The Gantt tab draws the same chart as the explorer's `SPC g g`, with the same
meaning. The tree stays on the left with every folder open, the filter, search and
view toggles of the list applied; the dates scroll on the right.

| Mark | Means |
|---|---|
| light bar | The planned span: the row's start to its deadline |
| hatched bar | The planned span of an epic or milestone that wrote no day of its own, counted from its members |
| dashed outline | A deferred row |
| solid band over the bar | When the work actually ran, in its column's colour; a row whose start nobody recorded has none |
| red hatch and `!` | Past the row's own deadline, up to today |
| `◆` | A row with a deadline and no start |
| red dashed line | Now |
| `‹` `›` | The row goes on off screen |
| `n/m` and the fill | Finished work under an epic or milestone |
| arrows | The rows that block the one under the cursor |

The page only places these: which span a group covers and whether a row is late is
decided by moai (`moai tui --deep --json` carries it as `gantt`). Lateness is counted
in the time zone the page draws in ([Time zone](#time-zone)): the page asks for the
chart values in it. A zone the machine `moai web` runs on cannot read is counted in the zone
moai runs in instead — an offset such as `+09:00`, a name its time zone data leaves
out (browsers spell `Asia/Kolkata` as the older `Asia/Calcutta`, which newer data
packages no longer carry), or any name on a machine with no time zone data.

Chart values are requested only while this tab is active. Opening Gantt after a
tree-only read requests them once; rows may appear briefly before their bars.
Other tabs read the same tree without calculating the chart values.

| Key / mouse | Does |
|---|---|
| `j` `k`, click | Move the cursor; a click on a row or bar opens its detail |
| `H` `L`, Shift+wheel, drag the date header | Move the dates |
| `-` `=`, the zoom buttons, Ctrl+wheel over the dates | Hour / day / week / month |
| `t` / `z` | Jump to today / fit every drawn row |
| drag the line between grid and dates | Grid width, kept in this browser |
| drag a bar's middle | Move its start and deadline by the same number of days |
| drag a bar's left or right end, or a `◆` | Move the start alone, or the deadline alone |
| double-click a day on a row with no days | Plan that one day as its start and deadline |

A bar moves by whole days and is written once, when you let go — the same write as
`moai edit <id> --shift <N>` (`--shift-start` or `--shift-due` for an end). What is
sent is the move, not the days: moai counts them inside its write from the row as it
stands then, so a row someone moved in the two seconds before the page read it again
moves on from where they put it. A double-click plans its day as `moai edit <id>
--start <day> --due <day> --if-unplanned`, refused if the row gained days meanwhile.
A move moai refuses (a start after the deadline, a day past the deadline of the
epic, milestone or parent above) is told in moai's own words under the banners and
the bar goes back. While the write and refresh run, the bar keeps the dropped days
through scrolling and redraws. A move that went through says the `moai edit` line
that shifts the same edge back by the same number of days. Changes to the other
edge stay in place. If that edge had no day when the page read it, undo clears it;
a day another session added to that edge before the write would be cleared too.
A newly planned day is undone by clearing both days. The TUI's notice instead
names the dates it read inside the write. A bar an epic or milestone counts from its members
(hatched) does not move — move the members. Nor does a row the write cannot reach:
one the worktree overlay shows from another worktree, and an archived row.

## Theme

The button left of the hide-detail button, in the tab bar's right corner, cycles
three states: **system** (a half-filled circle) follows the browser's light or
dark scheme and changes with it, **light** (a sun) and **dark** (a moon) stay as
chosen whatever the system says. Its name says the state it is in and what
pressing it does next. The choice is kept in this browser only (nothing goes to
the server or your user config), so another browser starts at system; choosing
system again forgets it.

## Search, filter, sort, view, column

- **Search** (`/`) searches as you type, with the words of `moai show -g`: the
  id, title, tags, body and notes of every row. `Enter` keeps the search and
  returns to the list; `Esc` closes it
- **Filter** (`SPC f`) takes the same `key=value` words as `--filter`. `Tab`
  completes, `Up`/`Down` pick a value and `Enter` applies. `Esc` in the list
  clears an applied search or filter
- **Sort** (`SPC s`) works as in the explorer: `p c u s a t w` sorts by priority,
  created, updated, status, assignee, title or deadline, and the same key again
  turns it around. The capital letter adds the field to the end of the order. `SPC s e`
  opens the sort editor
- **View** (`SPC v`) shows or hides deferred work (`l`), backlog items (`b`) and
  the archive (`o`; the page asks for the archive's rows, and for its lines in
  the tree, only while it is shown); `w` overlays what the other worktrees changed, as
  `moai show --worktree` does (on when the page opens); the overlay covers the
  list, the tree that places each row under its epic, the stamp the page follows,
  the milestone and epic counts (`moai status --worktree`), the read marks and the detail pane, so a row only a sibling worktree holds
  stands under its epic like any other and opens like one; `a` shows everything but the archive, `1`~`9` toggle the
  nth status column
- **Column** (`SPC c`) picks what a list row carries: `i` id, `p` priority,
  `a` assignee, `c` created, `u` updated, `n` counts, `t` tags, `e` epic,
  `h` names

The page remembers its tab, where the detail sits, the theme and the time zone, and keeps the
filter, the sort, the view and the columns apart for each project it reads, in the
browser's own local storage — one project's filter never lands on another's page.
Tabs save only the settings they change, preserving choices saved by another tab.
Open tabs follow each other's theme and time zone; each keeps its own tab, layout
and query while open. The search is not kept. A browser that keeps no site data (a private window)
remembers them until the tab is closed.

## Time zone

Dates are drawn in the zone the explorer uses (`[tui] timezone` in your user
config), or the browser's own when you have not picked one. `SPC o t` opens a
list of zones to pick from — type to narrow it, `Enter` to take one. The choice
only changes what this page draws — the Gantt tab's lateness included, read again in
the new zone — and is kept in the browser; it never touches your user config.
The filter's days (`created=<day>` and other timestamp keys given a day or local
time) are judged in that zone too. Plan days (`due_on` and `starts_on`) are calendar
days and do not change with the zone.

## What it writes

Each list, issue and wiki page the page shows is the answer of `moai --json
<command>`, run in a child process for that request and passed on as it is, so
what you see is exactly what the CLI says. Writing stays where it was: the CLI for
moving, editing and noting, and the explorer's `SPC n` for a thought you want to
park and its Gantt for planned days.

The one write to the tracker is a Gantt bar's days ([above](#gantt)): the server
runs `moai edit --shift` for it (`--start --due --if-unplanned` for a double-click),
so it goes through the same lock, checks and refusals as the CLI. The server takes
it only from the page itself: a request another site sends is refused.

The other write is read marks. `r` marks the issue under the cursor read,
`SPC m a` everything new to you and `SPC m g` the group under the cursor — the
same as `moai read`, which keeps them in a file of your own beside your user
config and never in the tracker. The server takes that write only from the page
itself: a request another site sends is refused.
With the worktree overlay on (`SPC v w`), the marks are made on the version the
overlay shows, as `moai read --worktree` does: a row only another worktree holds
can be marked, and a row another worktree changed later is marked at that later
version, so its NEW goes away. Nothing is written to the other worktree.

Nothing is stored for the page either: each list is read again when you ask.
Search, filters and sort are moai's own — the server hands them to `moai show`
and `moai tui --deep` as `--grep`, `--filter` and `--sort` — so a filter means
on the page what it means on the command line, and one moai does not know is
refused in the same words.

## It follows the tracker

When someone writes the tracker — you in another terminal, a worker in another
window — the page notices and reads again. It asks the server for a short
stamp, folded from the files its answers come from (the tracker's snapshot,
journal, archive and config, plus your own user config and the read marks kept
beside it, which say what is new to you), and reloads the list only when the
stamp has changed. With the worktree overlay on, the stamp also folds the other
worktrees' snapshots, so a write in a sibling worktree moves the page within
those two seconds; with it off, a sibling's writes leave the page alone. It asks every two seconds while the tab is visible, and not at
all while it is hidden; shown again, it asks at once. The issue open in the
detail is read again with it, and the cursor, the folded groups and what you are
typing stay where they were. A tracker nobody touches costs one tiny question and
no reread. What changes with the clock alone — a deadline passing, done work
ageing into the archive — shows on the next reload, or when you click the
refresh time.

When the server stops, one line says it cannot be reached, and the page asks
again every ten seconds; the line goes once the server answers. A read that
comes back refused or cut short (`busy`, `timeout`, a Cancel) is asked again on
the next look, so the list never stays on the rows from before a change.

## See it from another machine

Leave the server on `127.0.0.1` and bring the port to you:

    ssh -L 6624:127.0.0.1:6624 you@the-machine

Then open `http://localhost:6624/` on your own machine. This is the way to go
rather than `--host 0.0.0.0`, which serves the tracker to anyone on the network —
and lets them move its planned days, since a Gantt bar dragged on their page
writes the tracker as yours does.

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
| `forbidden_origin` | A read mark or a Gantt bar's days sent from anywhere but this page |
| `not_registered` | `?project=` names a directory that is not a registered project |
| `not_a_project` | `?project=` names a registered directory that has no `.moai` of its own (`moai init` it); the server does not read a parent's tracker in its place |
| `method_not_allowed` | Anything but `GET`, except the `POST` that marks rows read or moves a Gantt bar's days; the `Allow` header names the method the path takes (`POST` on those write paths, `GET` elsewhere) |
| `bad_input` | A request the server could not read, one carrying a body, or a query this path does not take or whose value is the wrong shape (a `sort` moai does not know, a flag other than `1`, a Gantt move that is not an edge and a whole number of days, a Gantt day that is not `YYYY-MM-DD`, a time zone that is not a zone name or an offset) |
| `not_found` | No such path |
| `busy` | Too many requests at once; try again |
| `timeout` | The child `moai` did not answer in time |
| `too_large` | The child's answer was larger than the server will pass on |
| `error` | The child `moai` could not be run, or died before it answered |

When the child `moai` itself refuses — an issue that is not there, a filter it
does not know — that is still an answer: it comes back as moai's own JSON
failure, with moai's own `code`, the same as on the command line.

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

A browser keeps some `Ctrl` keys for itself, and `Ctrl-w` closes the tab, so the
page binds no `Ctrl-w`: `SPC w` moves between panes. Where a browser takes
`Ctrl-f` or `Ctrl-b` first, `PageDown` and `PageUp` do the same. Click the page
once and the keys reach it.

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
| `SPC g l` `b` `g` `w` | List / board / Gantt / wiki tab |
| `SPC o d` | Detail position: right → bottom → left → top |
| `SPC o t` | Pick the time zone dates are drawn in |

**Projects and read marks**

| Key | Does |
|---|---|
| `1`~`9` or `SPC p 1`~`9` | Read the project with that number |
| `r` | Mark the issue under the cursor read |
| `SPC m a` | Mark everything new to you read |
| `SPC m g` | Mark the group under the cursor read |

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
| `Tab` / `Shift-Tab` | Complete a filter |
| `Up` / `Down` | Pick a filter value |
| `Ctrl-u` | Clear the input |

**Sort editor** (`SPC s e`)

| Key | Does |
|---|---|
| `j` `k` | Move in the list |
| `J` `K` | Move the field later / earlier in the order |
| `SPC` | Add / remove the field |
| `d` | Flip direction |
| `D` | Save the order as the default: what an emptied order returns to, kept in this browser |
| `Enter` / `Esc` | Apply / cancel |

**Wiki tab**

| Key | Does |
|---|---|
| move keys | Move in the focused side (page list or page) |
| `l` `Right` | From the page list to the page |
| `Enter` | Open the page (list); follow the chosen link (page) |
| `Tab` / `Shift-Tab` | Next / previous link in the page |
| `Backspace` `h` `Left` | Back along the path you came by |
| `/` | Search titles and bodies, in `moai wiki search`'s order |
| `SPC v r` | Raw / rendered page |

The page does not show the stats screen, and what it writes is a Gantt bar's days
and your read marks.

Decided in: moai-8r2l, moai-0aa7, moai-jshy, moai-d86e.3ud, moai-ijjd, moai-lj8g
