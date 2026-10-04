# The explorer

`moai tui` walks the tracker on one screen: the list on one side, the row under
the cursor described on the other. It is for reading — finding work, seeing how a
milestone stands, catching up on what changed — and the one thing it writes to
the tracker is a thought you park (`SPC n`). Moving, editing and noting are done
with the CLI. Every key is in `moai tui --help`; this page says which ones to
reach for.

## Open it

- **Inside a repository**, `moai tui` opens that project
- **Outside any `.moai`**, it opens the overview of the projects you registered
  with `moai project add` — or an empty list that points at `SPC p a` when there
  are none
- **`--path <id>`** opens at that issue; `--path none` and `--path lost` open the
  two baskets — rows with no milestone, and rows that cannot be placed because
  the epic or milestone they name is not there (or their parent's id stands twice)

It needs a terminal. A script that wants the same rows calls `moai tui --json`,
which prints the listing and opens no screen.

## Find your way around

Milestones and epics behave like directories.

- `j`/`k` move, `Enter` goes into a milestone or an epic, `Backspace` comes back
  out. Rows you can go into end their title with `/`
- `l` unfolds a group one step where it stands and `h` folds it; `Tab` unfolds
  everything under it
- `gg` and `G` go to the top and the bottom, `Ctrl-d`/`Ctrl-u` half a page
- The detail describes the row under the cursor. `Ctrl-w w` moves the focus there
  so the same keys scroll it, and back again

**What the list shows is the view.** Done is hidden to begin with, and the path
line above the list says what is hidden and how it is sorted. The `SPC v` keys
show or hide deferred work, ideas, each column, the detail and the archive;
`SPC s` sorts and `SPC c` picks the columns on the right of a row. These choices
are kept in your user config and carry over to the next run and to every project.

## Pick a screen

`SPC g` picks which screen stands; the menu's root names the one you are on.

- **`SPC g l` — the list**, described above
- **`SPC g b` — the board.** The same rows as a kanban board: idea, deferred, then
  your columns, with one lane per milestone at the project root. A card that is
  not yours says whose it is. `h` and `l` go across the columns, `j` and `k` along
  one, and the wheel and `Ctrl-d`/`Ctrl-u` scroll the board without moving the
  cursor. The cursor, filter, view and detail are the list's, so switching keeps
  your place. The board does not move cards — `moai mv` does
- **`SPC g s` — statistics.** The numbers `moai stats` gives, drawn: the flow per
  week, the columns and priorities, lead and cycle time, and AI work by model. It
  counts the project you are in, and only what passes the filter when one is set.
  `b` switches between weeks and days and `Esc` goes back to where you were.
  Nothing of it is kept; it counts afresh each time it opens

The list or the board you leave on is the one the next run opens with.

## The menu

Everything that is not moving or searching lives in the menu that opens the
moment you press `SPC`. It stands up only what works where you are — screens,
view, sort, columns, read marks, options and projects — and ignores keys it does
not know.

- `Esc` or `SPC` closes it, and `Backspace` goes one level up
- Toggles and sorts leave it open so you can try one and watch the screen change;
  that level says so with an `Esc` hint
- A movement key closes it and makes the move in the same press

Over the open menu the mouse does nothing: a click or the wheel is ignored and the
menu stays open. Close it with `Esc` or `SPC` first.

## The mouse

The mouse is on to begin with; `SPC o m` turns it off and on, and the choice is
kept.

- **A click** puts the focus on the pane under it and the cursor on that row or
  card
- **The wheel** moves the pane under the pointer — the list's cursor, the detail,
  the board, the statistics — without taking the focus there
- **Dragging the line** between the list and the detail resizes them, and the
  share is kept
- **To select or paste with the terminal**, hold `Shift` over the list and the
  detail (`Option` in iTerm2), or turn the mouse off. While a form, a picker or a
  prompt is open, the mouse is the terminal's again

## Narrow it down

- **`/` searches** the list as you type. `Tab` and `Shift-Tab` pick where it looks
  — everything, id, title, tag, body or note — and it finds rows the view hides,
  the archive included. `Enter` keeps it and `Esc` gives up
- **`SPC f` filters** with the same `key=value` words as `moai show --filter`.
  While you type, the keys it takes and a few examples stand above the field, and
  in the value of `assignee=`, `tag=`, `no-tag=` or `milestone=` the values this
  tracker holds do — `Up` and `Down` pick one and `Enter` puts it in
- **`Esc` clears** the search or filter you set. It does not touch the view: what
  `SPC v` hides stays hidden, and the two apply together

Search and filter work inside one project. In the overview, go into the project
with `Enter` first.

## The archive

Work that has sat in done for two weeks is the archive. Nothing is stored for it
— it is read off the column and the clock each time — and it stays hidden on the
list and the board even when done is shown; the path line counts what it left
out. `SPC v o` shows it, and `/` finds it either way. On the CLI it is
`moai show --archived`. `archive_days` in `.moai/config.toml` sets the two weeks,
and `0` turns the archive off (moai-47mz).

## Catch up on what changed

Rows that came to you — assigned to you, or under something that is — and changed
since you last looked carry a `[NEW]` mark. `r` marks the row under the cursor
read, `SPC m g` every member of the group it is in, and `SPC m a` everything.
Read marks live in your own config, not in the tracker, so marking changes
nothing anyone else sees; the CLI side is `moai read`.

## Park a thought

`SPC n` opens the jot form anywhere inside a project, and what you write is kept
as an idea with no epic — off the board and out of `moai ready` until someone
unfolds it. With an editor on hand (`$VISUAL`, `$EDITOR`, `vi` or `nano`) it opens
like a git commit message: the first line is the title. Without one, a built-in
form opens and `Ctrl-S` keeps it. Leave the title empty and nothing is kept.

## Several projects

Register a project once with `moai project add <dir>`, or from inside the explorer
with `SPC p a`, which opens a window to walk to the directory. The header numbers
every registered project; press that number to jump there, and `0` for one list
of them all, a header row per project. `Enter` on a header goes into that project.
`SPC p d` on a header drops the project from the list and leaves its files alone.

## The version line

The header says whether a newer release is out — a new release, the latest, ahead
of the latest (a build from source), or not asked. It asks GitHub at most once a
day and only when a person is watching; `MOAI_NO_UPDATE_CHECK=1`, or
`check = false` under `[update]` in your user config, stops it asking. When a new
release is out, quitting prints the line that upgrades the moai you are running.

## When it goes wrong

- **"Not a terminal"** — the output is piped or redirected. A script wants
  `moai tui --json`
- **A row you expected is not there.** Read the path line: done or the archive
  may be hidden, or a filter or search may be set. `Esc` clears the filter,
  `SPC v a` shows everything but the archive, and `SPC v o` adds the archive
- **The terminal will not select text** — hold `Shift` (`Option` in iTerm2), or
  turn the mouse off with `SPC o m`
- **Parking a thought says another moai is writing** — the form stays open; wait
  a moment and keep it again
- **It asks who you are when you park a thought** — the tracker records a name and
  an email on every write and `git config` had none. The answer holds for this
  run; set `user.name` and `user.email` to stop it asking

Decided in: moai-z46r moai-9nfw moai-47mz moai-irrj moai-ucx8 moai-1hka moai-h2rh moai-gelm
