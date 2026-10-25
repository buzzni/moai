# The explorer

`moai tui` walks the tracker on one screen: the list on one side, the row under
the cursor described on the other. It is for reading — finding work, seeing how a
milestone stands, catching up on what changed — and the one thing it writes to
the tracker is a thought you park (`SPC n`). Moving, editing and noting are done
with the CLI. Every key is in `moai tui --help`; this page says which ones to
reach for, and [the glossary](glossary.md) says what the words mean.

## Open it

- **Inside a repository**, `moai tui` opens that project
- **Outside any `.moai`**, it opens the overview of the projects you registered
  with `moai project add` — or an empty list that points at `SPC p a` when there
  are none
- **`--path <id>`** opens at that issue; `--path none` and `--path lost` open the
  two baskets — rows with no milestone, and rows that cannot be placed because
  the epic or milestone they name is not there (or their parent's id stands twice)

It needs a terminal. A script that wants the same rows calls `moai tui --json`,
which prints the listing and opens no screen — with counts, filters and order as
[Draw the tracker from another program](json.md#walk-the-explorers-directories) says.

To read the same tracker in a browser tab instead, run `moai web` — see
[Open the tracker in a browser](web.md).

## Find your way around

[Milestones](glossary.md#milestone) and [epics](glossary.md#epic) behave like
directories.

- `j`/`k` move, `Enter` goes into a milestone or an epic, `Backspace` comes back
  out. Rows you can go into end their title with `/`
- `l` unfolds a group one step where it stands and `h` folds it; `Tab` unfolds
  everything under it
- `gg` and `G` go to the top and the bottom. `Ctrl-d`/`Ctrl-u` scroll half a
  page and `Ctrl-f`/`Ctrl-b` a whole one without moving the cursor: the row you
  chose and its detail stay put even off screen, and the next `j` or `k` brings
  the list back to it
- The detail describes the row under the cursor. `Ctrl-w w` moves the focus there
  so the same keys scroll it, and back again. After the commits it lists the
  wiki about the row, as `moai show <id>` does: the pages that name the row or
  its epic, then the related sections its title finds, `path:line` first. The
  wiki is read once per checkout, so a page written after the explorer opened
  shows the next time it opens

**What the list shows is the view.** To begin with every column is shown,
done included, and [deferred](glossary.md#deferred) work is hidden; the path
line above the list says what is hidden and how it is sorted. The `SPC v` keys
show or hide [deferred](glossary.md#deferred) work, [backlog items](glossary.md#backlog),
each [column](glossary.md#column), the detail and the archive;
`SPC s` sorts and `SPC c` picks the columns on the right of a row. These choices
are kept in your user config and carry over to the next run and to every project —
except the sort, which each project can keep for itself (below). The same file holds
`[models]`, the model each role of a supervisor's workers runs on — taught on
[working with agents](agents.md#the-models-are-yours); a value there that cannot be
read is named among the explorer's problems.

## Sort by several fields

The sort is a list of fields, like `ORDER BY`: the second field decides only where
the first ties, and each field has its own direction.

| Keys | What it does |
|---|---|
| `SPC s p` `c` `u` `s` `a` `t` `w` | sort by that one field (priority, created, updated, status, assignee, title, due — `w` for when); the same key again turns it around |
| `SPC s P` `C` `U` `S` `A` `T` `W` | add that field at the end of the order; if it is already there, turn it around |
| `SPC s e` | open the sort window |

In the sort window the fields in the order come first, numbered and marked
`↑ asc` or `↓ desc`; the rest are marked `·`. `j`/`k` move the cursor, `J`/`K`
move a field down or up the order, `SPC` puts a field in or takes it out, `d`
turns its direction, `Enter` applies it, `D` also keeps it as the default, and
`Esc` leaves without a change. The path line names the whole order
(`sort priority↑, updated↓`).

**Where the sort is kept.** A sort picked inside a project is written for that
project; one picked in the all-projects list (`0`) is the default for every
project that has none of its own. A worktree counts as the project it belongs to.

```toml
[tui]
order = ["priority", "updated:desc"]        # the default

[tui.project."/home/me/work/app"]
order = ["status", "title"]                 # wins in that project
```

`order` takes the words of `moai show --sort` except `id`. An older config with
`sort = "…"` and `sort_reversed = …` is still read where `order` is absent, and
writing the default also writes that pair from the first field, so an older moai
on the same config shows the nearest order.

`SPC v b` hides backlog items in the list and their column on the board.
The choice is saved as `[tui] hide_backlog`. Existing `hide_ideas` settings
remain readable; if both keys exist, `hide_backlog` wins.

## Pick a screen

`SPC g` picks which screen stands; the menu's root names the one you are on.

- **`SPC g l` — the list**, described above
- **`SPC g b` — the board.** The same rows as a kanban board: backlog, deferred, then
  your columns, with one lane per milestone at the project root. The deferred column
  follows the view — hidden to begin with, back with `SPC v l`. A card that is
  not yours says whose it is. `h` and `l` go across the columns, `j` and `k` along
  one, and the wheel and `Ctrl-d`/`Ctrl-u` scroll the board without moving the
  cursor. The cursor, filter, view and detail are the list's, so switching keeps
  your place. The board does not move cards — `moai mv` does
- **`SPC g g` — the gantt.** The same rows opened out on the left and a date axis
  with bars on the right — see [the gantt](#the-gantt) below
- **`SPC g s` — statistics.** The numbers `moai stats` gives, drawn: the flow per
  week, the columns and priorities, lead and cycle time, and AI work by model. It
  counts the project you are in, and only what passes the filter when one is set.
  `b` switches between weeks and days and `Esc` goes back to where you were.
  Nothing of it is kept; it counts afresh each time it opens
- **`SPC g w` — the wiki.** The pages `moai wiki ls` lists, read only: the pages
  on the left with the home page first, the page under the cursor on the right.
  On the overview it reads the project of the row under the cursor. The wheel,
  `Ctrl-d`/`Ctrl-u` and `Ctrl-f`/`Ctrl-b` scroll the page list without moving
  the cursor, as on the main list. `Enter` on
  the list goes to the page. On the page, `Tab` picks the links drawn there in
  turn — from the first one on screen, round from the last to the first — and
  `Shift-Tab` goes the other way, from the last one on screen. The picked link
  shows reversed and the key bar names where it goes; `Enter` follows it, `Esc`
  drops it, and once it leaves the pane it is dropped too.
  Clicking a link follows it at once. With no link picked, `Enter` on the page
  lists its links and the issue ids it names, then the pages that link to it,
  marked `←`. Taking a page — any of these ways — goes there and `Backspace`
  comes back to the line you were reading. A link that names a heading (`glossary.md#epic`, or `#epic` on the
  same page) opens at that heading, its line marked `▸` until you scroll or
  leave; one whose heading the page does not have stands marked
  `(no heading)` and opens the page at its top. Taking an id closes the
  window onto that row, inside that project when you opened it from the
  overview. A link that leads nowhere, an address outside the wiki and
  an id the tracker does not hold stay in that list marked `(none)` or
  `(outside)`, and taking one only says so — picking or clicking one does the
  same. `/` searches the titles and bodies and narrows the list, in
  `moai wiki search`'s order: the pages its words rank, best first, then the
  pages that only contain the letters typed so far. `Esc` drops a
  picked link first, then clears that search, then goes back to where you were.
  `SPC v r` shows the page as written, where no link is marked for `Tab`. It
  reads the pages afresh each time it opens and writes none of them — a page is
  a file you edit and commit

Over either window, `SPC g s` and `SPC g w` read afresh. When the window you are
in cannot be read again it closes and a notice says why; when the other one
cannot be opened, the one you are in stays as it was and only the notice stands.
In both windows the arrows, `Home`/`End` and the page keys move with `Ctrl` or
`Alt` held too, as on the list; letter keys do not.

The list, the board or the gantt you leave on is the one the next run opens with.

## The gantt

Reach for it when you want to see the plan against the calendar: what starts when,
what runs late, and what is moving right now. It is the list's layout, not a window
of its own, so the cursor, filter, view, search and detail carry over and `SPC g l`
brings you back to the same row.

**Reading it.** Every group opens out when you enter, and the left column is the
list's row. On the right, `░` is the planned start to deadline (`▒` on an epic or a
milestone), `◆` the row's own deadline, `█` over it when the work actually ran,
`▓` the time past a deadline, `┄` a deferred row's plan, and `▼` over a dotted line
today. An epic that wrote no days of its own spans its members' days. Planned days
are calendar days; actual times are shown in your time zone. With no planned days
anywhere, the date side says how to add them: `moai edit <id> --start <day> --due
<day>`, or `@start~due` at the end of a plan line.

**Folding.** `h`, `l` and `Tab` fold and unfold inside the gantt only — the list
keeps the folds it had, and the next time you enter, everything is open again.

**Moving the dates.** `=` and `-` zoom in and out (hour, day, week, month), `H` and
`L` slide one cell, `[` and `]` a page, `t` puts today in the middle and `z` fits
every row on one screen. It opens fitted and stays fitted until you move it.

**Moving a plan.** `<` and `>` move the cursor row's whole plan a day, `{` and `}`
only its start. With the mouse, drag a bar's middle, its left end (the start) or its
right end (the deadline); it snaps to whole days and is written once, when you let
go. Click an empty day on a row with no plan to plan that one day. The write is the
same as `moai edit --start --due`: a start after the deadline, or a day past the
deadline above, is refused and the refusal stands on the banner. A move that went
through names the `moai edit` line that puts it back, and the rows it pushed past
this deadline. An epic whose bar spans its members does not move — move the members.
On the overview, enter the project first.

**The detail** stands below the gantt at first, so the dates keep the width.
`SPC o d` there moves only the gantt's detail, for this session.

**Small windows.** On a narrow window the left column gives up its columns first,
then the dates are hidden and the header says so; a low window folds the `▼` row.

## The menu

Everything that is not moving or searching lives in the menu that opens the
moment you press `SPC`. It stands up only what works where you are — screens,
view, sort, columns, read marks, options and projects — and ignores keys it does
not know.

- `Esc` or `SPC` closes it, and `Backspace` goes one level up
- Toggles and sorts leave it open so you can try one and watch the screen change;
  that level says so with an `Esc` hint
- A movement key closes it and makes the move in the same press

The mouse works on the open menu too. Click an item and it is as if you pressed
its key — a group goes one level down and a toggle keeps the menu open. `Esc` and
`Bksp` on the bottom line close it or go up. A click or the wheel outside the menu
closes it and does what it does there, so clicking a row closes the menu and puts
the cursor on that row. Empty space inside the menu does nothing.

## Change the keys

The keys are yours to change, in a file of their own: `keys.toml` next to your
user config — `~/.config/moai/keys.toml`, or under `$XDG_CONFIG_HOME/moai/`
when that is set, unless `MOAI_CONFIG` or `MOAI_KEYS` points elsewhere. Write only the lines you change; every other key keeps its
default.

1. Get a starter file. `moai keys --template` prints every action under its
   section with its default keys, commented out — save it where the file lives:

       moai keys --template > ~/.config/moai/keys.toml

2. Uncomment the lines you want and change them:

   ```toml
   [browse]
   quit = "SPC x"
   grep = ["/", "SPC /"]   # several keys: the first is the one the bar and the menu show
   jot = "none"            # unbind it
   [moves]
   line_down = "n"         # moves are shared by the list, the picker, the statistics and the wiki
   ```

3. Start `moai tui` again — the file is read once, when the explorer starts. Run
   `moai keys` to see the keys in force: the ones you changed are marked with
   `*` and the old key.

A line replaces **every** key of that action, the hidden ones too — `line_down
= "n"` leaves the down arrow doing nothing, so list it again
(`["n", "Down"]`) to keep it. Key names are spelt the way the explorer shows
them: `SPC`, `Enter`, `Esc`, `Tab`, `Bksp`, `Ctrl-w w`, `gg`, `Alt-x`, and a
capital letter for Shift.

A line that cannot work is dropped on its own and that action keeps its default
key; the explorer still opens and says so in one line. Swapping two keys is
fine — clashes are measured after every line is in. Some keys cannot be changed:
`Ctrl-C`, the digits `0`–`9` that jump to a project, `SPC v 1`–`9`, `SPC`
itself, and the keys inside a text field (search, filter, the jot form). A
window's `close` key can move to another key but cannot be set to `none` — it
is the only way out of that window.

The gantt's own keys live under `[gantt]`, one key per line, and are looked up
first only while the gantt stands. A `[browse]` line that uses one of them
(`expand = "L"`) keeps working in the list and the board, and in the gantt that
key does the gantt's thing. A `[gantt]` line that would hide a list key inside
the gantt — `fit = "j"` — is the one dropped.

`moai tui --help` and the menu name the keys you set. The help's sentences about
the arrows, `Home` and the page keys still describe the defaults; `moai keys`
lists every key exactly.

## The mouse

The mouse is on to begin with; `SPC o m` turns it off and on, and the choice is
kept.

- **A click** puts the focus on the pane under it and the cursor on that row or
  card. On a wiki page, a click on a link follows it. With the menu open, a click
  on an item presses its key and a click outside closes the menu first — see
  [the menu](#the-menu)
- **The wheel** scrolls the pane under the pointer — the list, the board, the
  detail, the statistics, either pane of the wiki — without taking the focus
  there or moving a cursor. The row you chose stays chosen even once it is off
  screen; the next cursor key brings the list back to it
- **Dragging the line** between the list and the detail resizes them, and the
  share is kept. The wiki window's line drags the same way and keeps a share of
  its own (`wiki_width`); until you drag it, the wiki splits like the list
- **In the gantt**, a sideways wheel or `Shift` and the wheel slide the dates, and
  so does dragging the date header. Dragging a bar moves its plan — see
  [the gantt](#the-gantt). Dragging the line between the rows and the dates widens
  the rows, and that width is kept (`gantt_label`)
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
  tracker holds do — `Up` and `Down` pick one and `Enter` puts it in.
  `Tab` completes the key at the cursor: `mil` becomes `milestone=` so you can
  start typing its value. If several keys match, Tab cycles through them in
  the order shown in the hint panel, and `Shift-Tab` cycles backwards.
  Tab also inserts and cycles through matching tag, assignee and milestone
  values, or status columns, priorities `p0` to `p3` and kind names. The
  highlighted value follows the candidate inserted into the field. Another
  key keeps that candidate and ends the cycle; Enter applies the completed
  filter, while Esc gives up
- **`Esc` clears** the search or filter you set. It does not touch the view: what
  `SPC v` hides stays hidden, and the two apply together

Search and filter work inside one project. In the overview, go into the project
with `Enter` first.

## The archive

Work that has sat in done for two weeks is eligible for the archive. The archive
is moved explicitly with `moai archive`; use `moai archive --dry-run` first to
see the rows. Closed epic bundles move together into
`.moai/archive/<year>.jsonl`, while milestone rows stay live. Normal board and
ready counts use the active snapshot. Archived rows still supply parent, blocker
and milestone context, so a shipped release stays closed and a restored member
keeps its original group. A new or edited row can point at an archived epic,
parent or blocker (`add -e`, `edit -e`, `add --parent`, `link`,
`backlog promote -e`) without restoring it. `SPC v o` and `moai show --archived`
read archived rows too, and `moai show <id>` can open one directly.
`archive_days` in `.moai/config.toml` sets the two weeks, and `0` turns archive
eligibility off; `moai show` and `moai status` leave rows already moved to the
archive out either way. Moving an archived row with `moai mv` restores that selected
row; its former bundle remains archived (moai-fx9t).

When a live row also has an archive copy, `moai status` names the conflict as
`archive_duplicate_id`. Keep the live row and run `moai archive --drop <id>` to
remove its archive copies. This refuses an ID without a live row, and an archived
row that is a different issue under the same ID (another kind or creation time).
A conflicting bundle stays live during `moai archive`, while other eligible
bundles can move; `--dry-run` and the board count only what will move.
Unreadable archive files are reported by source and readable rows remain available;
they are broken data, so `moai status` counts them as the `archive_unreadable`
warning and exits non-zero ([Recovery](recovery.md#an-archive-file-cannot-be-read)).
The explorer's banner and each project's `+N` in the overview count all of this
the way `moai status` does: archived rows are context, not work, the collision
and unreadable archive files are counted as the same warnings, and bundles
waiting to move as the same notice (moai-nkwg). The banner's urgent "rows could
not be read" counts lines in the active snapshot only; a bad line in an archive
file is that `archive_unreadable` warning instead (moai-e18s). While the archive
is shown (`SPC v o`), the banner also names how many archive lines could not be
read and that the view is without them — dimmed, so it does not make the banner
urgent (moai-ug6x).

## Catch up on what changed

Rows that came to you — [assigned](glossary.md#assignee) to you, or under
something that is — and changed
since you last looked carry a `[NEW]` mark. `r` marks the row under the cursor
read, `SPC m g` every member of the group it is in, and `SPC m a` everything.
Read marks live in your own config, not in the tracker, so marking changes
nothing anyone else sees; the CLI side is `moai read`. Outside the explorer,
`moai show --json` (and `moai show <id> --json`) puts `"unread": true` on the same
rows the explorer marks `[NEW]`; a row that is not unread has no key at all, and
when moai cannot tell who you are or cannot read your read marks, no row carries it.

## Park a thought

`SPC n` opens the jot form anywhere inside a project, and what you write is kept
as a [backlog item](glossary.md#backlog) with no epic — off the board and out of
`moai ready` until someone
unfolds it. With an editor on hand (`$VISUAL`, `$EDITOR`, `vi` or `nano`) it opens
like a git commit message: the first line is the title. Without one, a built-in
form opens and `Ctrl-S` keeps it. Leave the title empty and nothing is kept.
The editor has to save UTF-8. If it saves in another encoding, moai cannot read
the file, so it keeps it where it is, names its path, and says that path again on
the shell when you quit — open it, save it as UTF-8, and park it again.

## Several projects

Register a project once with `moai project add <dir>`, or from inside the explorer
with `SPC p a`, which opens a window to walk to the directory. The header numbers
every registered project; press that number to jump there, and `0` for one list
of them all, a header row per project. `Enter` on a header goes into that project.
`SPC p d` on a header drops the project from the list and leaves its files alone.

## The version line

The header says whether a newer release is out — a new release, the latest, ahead
of the latest (a build from source), or not asked. It asks GitHub once a day,
and once more straight away when the answer it holds is older than the moai you
are running — as after an upgrade. An answer that says your moai is ahead of the
latest release is checked again after an hour. It asks only when a person is
watching; `MOAI_NO_UPDATE_CHECK=1`, or `check = false` under `[update]` in your
user config, stops it asking. It asks the repository releases come from —
`buzzni/moai` unless `repo = "owner/name"` under `[update]` says otherwise, and
`MOAI_REPO` wins over both.

When a new release is out, the banner names `moai update`, which upgrades the
moai you are running in place, and quitting prints it again. Where `moai update`
refuses only because of the repository setting (a malformed `[update] repo` or
`MOAI_REPO`, or a user config it cannot read), quitting prints the installer's
line instead. A directory you cannot write gets no line — the installer cannot
write there either.

## When it goes wrong

- **"Not a terminal"** — the output is piped or redirected. A script wants
  `moai tui --json`
- **A row you expected is not there.** Read the path line: done or the archive
  may be hidden, deferred work is hidden to begin with, or a filter or search
  may be set. `Esc` clears the filter,
  `SPC v a` shows everything but the archive, and `SPC v o` adds the archive
- **The terminal will not select text** — hold `Shift` (`Option` in iTerm2), or
  turn the mouse off with `SPC o m`
- **Parking a thought says another moai is writing** — the form stays open; wait
  a moment and keep it again
- **A key from `keys.toml` does nothing** — that line was dropped. The notice
  line at start names it, and `moai keys` lists every dropped line with the
  reason. Most often the key is already taken, or it starts a longer key and so
  waits for the rest (`g` waits for `gg`)
- **It asks who you are when you park a thought** — the tracker records a name and
  an email on every write and `git config` had none. The answer holds for this
  run; set `user.name` and `user.email` to stop it asking

Decided in: moai-z46r moai-9nfw moai-47mz moai-irrj moai-ucx8 moai-1hka moai-h2rh moai-gelm moai-m6ni moai-p3r9 moai-o3cb moai-fyul moai-ogaw moai-tllo moai-fc97 moai-jtvp moai-bth3 moai-9tqo moai-wk7i moai-pxiy moai-i4hf
