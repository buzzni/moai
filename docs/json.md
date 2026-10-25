# Draw the tracker from another program

A web panel, an editor plugin or a dashboard can draw what the explorer draws
without parsing `.moai/` itself. moai already works out every value that is not
written on a row — a group's column, the epic and milestone a row inherits, what
is deferred and what is archived — and hands it over in `--json`. Read those
values here instead of rebuilding the rules: a second copy of them drifts from
the binary the day a rule changes.

## Read the settings first

`moai config --json` gives the repository's settings with the defaults filled
in, under the names `.moai/config.toml` uses: `statuses` in board order,
`archive_days`, `naming`, `wiki_dir`, `prefix`, the `status_*` thresholds and the
`due_*` rule (`due_priority`, `due_p0_days`, `due_p1_days`, `due_p2_days`).
These are the values the binary acts on, so a board drawn from them lines its
columns up with the explorer's.

`moai status --json` carries the same column order as `statuses` beside
`counts`. `counts` is a map in name order — draw the columns from `statuses`
and look each count up in `counts`.

## Walk the explorer's directories

`moai tui --json` prints one directory of the explorer — the project root, or
the milestone, epic or basket given with `--path`. Each row says whether it is
a folder (`dir`) and the handle to go into it (`path`). Folder rows with an id —
a milestone, an epic, an issue with children — carry `progress`, the done and
total the explorer's counts column shows, counted over everything under them.
The two baskets (`none`, `lost`) carry none.

To show the same filtered list a person would see, pass the filters of
`moai show` — `-s`, `-g`, `--filter`, `--all`, `--archived` and the rest — and
`--sort` in its words. A row stands when `moai show` with those filters would
list it, and a folder stands when something under it does. Filters bring
`moai show`'s defaults with them, so done rows are hidden until `--all` or
`-s done` asks for them; without filters the listing is every row.

`moai tui --deep --json` prints everything under the place at once instead of
one directory: the rows in the order of the explorer's fully unfolded tree, a
folder row followed by what is inside it, with the filters and `--sort` of the
one-level listing applied at every place. Each row also carries `home`, the
`--path` handles from the root down to the directory it stands in (`[]` for a
row at the root, the basket rows included; `["none"]` or `["lost"]` for a row
inside a basket), always counted from the root even when `--path` starts the
walk lower. `--deep` needs `--json` and a
`.moai`, and one-level listings carry no `home`.

## Read rows and what they inherit

`moai show --json` gives the rows themselves. Beside the fields written on a
row it carries what the row inherits:

- `derived_status` — the column a milestone or epic stands in, read from its members
- `derived_epic` — the epic a row is in, whether written or carried by its id
- `derived_milestone` — the milestone the row stands under, inherited from its epic included
- `shelved_by` — the nearest row that took it out of the plan; its own id when it
  was deferred itself, another id when it inherits the deferral
- `archived` — `true` on a row the list hides as the archive

The last three stand only when they hold. `epic` and `milestone` are what the
file says, so read membership from the `derived_` keys. A closed row is in no
plan, so it carries no `shelved_by` even when it was deferred — `--deferred`
picks it by its own `deferred_at`. A row is deferred when it carries either.

`due_on` is the row's own deadline and nothing else — no key carries the deadline
inherited from its epic or milestone, because a deadline is not stored on the rows
below: the one above bounds them, and `moai due apply` raises them by the earliest that
applies. Read the ancestors' `due_on` to find it. `moai due apply --json` names it in
each raised row's `due_on`, with `due_from` naming the ancestor when it is not the row's
own; on `moai status --json` the `due_priority_behind` notice carries it per id in
`deadlines`, beside the days left in `ages`.

## Read the wiki about one row

`moai show <id> --json` also carries `wiki`, the wiki pages about that row, read
from the checkout you call it in:

- `named_in` — the pages whose text names the row's id or the id of the epic it
  stands in, in `moai wiki ls` order: `slug`, `path`, `title` and `via`, the id
  the page names (the row's own when a page names both)
- `related` — up to three sections found from the row's title, best first and
  one per page, leaving out the pages already in `named_in`: `slug`, `path`,
  `title`, `line` (counted from 1, the section's heading line), `score`, and
  `heading` and `anchor` unless the section comes before the page's first
  heading. A section stands only when it matches two words of the title that
  are not common across the wiki (one, when the title has only one such word);
  a word in more than a quarter of the sections does not count

`wiki` is always there; with no wiki directory, or nothing about the row, both
arrays are empty. `wiki_error` stands only when the wiki directory cannot be
used, beside two empty arrays: `kind` takes the words of `moai wiki ls --json`'s
`error.kind` (`not_a_dir`, `outside`, `failed`) and `said` is one line for a
person. A missing directory is a wiki not started yet, not an error.

Only `moai show <id>` carries these two keys. The list form,
`moai show [filters] --json`, leaves them out — it would walk the whole wiki
for every row. Ask per row when you need them.

## Keep the payload small

A whole tracker with bodies runs to megabytes — this repository's
`moai show --archived --json` is over 2 MB. Two things cut it down:

- `--fields id,title,status` keeps only those keys of each row, in that order
  and each once. The costly keys it leaves out are not worked out: without
  `work` a list skips the journals, and `moai show <id>` reads the wiki only
  for `wiki` and asks git only for
  `commits`. `moai show -g <word> --all --fields id --json` is a small list of
  the ids a search found, notes included — `-g` still reads the notes it
  searches
- Paging: `--sort id -n <count> --after <last id>` reads the list a page at a
  time. Archived rows and live rows share one order, and an edit between pages
  does not move a row in id order, so the pages join into the whole list

## When it goes wrong

- **`--fields` or a filter is refused** — `--fields` needs `--json`, and the
  filters of `moai tui` shape its `--json` only, inside a `.moai`. Run
  `moai -C <dir> tui … --json` for another project
- **A key is missing** — keys that can be absent are absent when they do not
  hold, never `null`. A row without `derived_milestone` stands in no milestone
- **Pages repeat or skip a row** — the order moved under you. Page in `id` or
  `created` order; priority and title move when someone edits a row

Decided in: moai-wk7i, moai-g8pt.5aw
