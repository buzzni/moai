<!-- moai:begin v:0.6.0 hash:d31e15a8 -->
## Issue tracker — moai

This repository's work lives in `.moai/issues.jsonl`.
Do not use TodoWrite or a markdown TODO list. There is no approval gate — create anything, move anything. Do not ask a human, except before you pick up work that is someone else's or nobody's (hook rule 5).

Start a session by running `moai status`. The board and the warnings come up on one screen.

    moai status                            board · warnings · flow (start a session here)
    moai prime                             what you hold and what is next, nothing else
    moai ready                             what you can pick up right now
    moai show <id>                         body, children, history. Why it was decided is here
    moai show -g <keyword>                 find out whether it is written down already
    moai show -s todo -t bug               filters (comma = or, repeated flag = and, except -a: or)
    moai show --tree                       epic → issue → child
    moai ready --worktree                  overlay what the other worktrees picked up
    moai stats                             counts: columns, flow, lead and cycle time, AI work
    moai tui                               walk the explorer. SPC n parks a thought
                                           on a group row: l one step · Tab expand all · h fold
    moai add '<title>' -p 1 -t bug -e <epic>   create
    moai mv <id> in_progress               pick up  →  review  →  done
    moai mv <id> in_progress --take        take over someone else's row (ask first)
    moai edit <id> --tag parser            change
    moai note <id> '<what you found>'      a memo for whoever comes next
    moai defer <id> -m '<why>'             take work out of the plan for now

Every command takes `--json`. `ready --json` gives `{"ready":[…],"others":[…],"held":[…]}` —
`ready` is yours to pick up, `others` is ready work that is someone else's or nobody's
(ask first), and `held` is what is deferred or blocked behind an empty group, and where
to pick it up again. A session a person opened reads that shape to choose its next row —
moai never launches or drives a session itself.

**A key that cannot be absent is never absent.** `kind` and `priority` hold a default,
and the file leaves a default out, but `--json` fills it back in — `jq -r .priority`
on a row gives `2`, never `null`. Keys that genuinely can be absent (`epic`,
`milestone`, `deferred_at`) stay absent, and that absence is the answer.

**Which epic a row is in, you read from `derived_epic`.** `epic` is what the file
says, and a row whose id sits under an epic (`<epic>.<body>`) reads its epic from
that id and writes no `epic` of its own — so on those rows `epic` is absent and
`derived_epic` names the epic. On a row that carries it, absence means the row is in
no epic at all, and on a group row it never stands. One row reads the two keys against
each other: where the same id stands twice and the other line is a different `kind`,
this line is counted into no group anywhere — the tree draws it under `(lost)`, `-e`
picks it up for no epic, and `derived_epic` is absent even when `epic` is written.
`duplicate_id` on the board names that id. A row below an id that stands twice, where
the lines hand down different answers, is counted into no group either — which line
it hangs from cannot be told — and `twin_parent` names it. Two surfaces carry neither key —
`rm --json` hands the removed lines back exactly as the file held them, and `tui
--json` prints the explorer's own shorter row — and there you read the epic off the id.

**When several sessions share one repository, pick up with
`moai mv <id> in_progress --from todo`.** It moves only while the column you saw
still holds, so it never overwrites work someone else picked up first — the loser
gets one line on stderr (`stale` under `--json`) and a non-zero exit code, and
moves on. `defer` takes the same `--from`. Without it nothing is blocked, as before.
**Pass one id only** — several at once mix winners and losers into a single exit
code, so you pick a row up and then throw it away.

**A `moai` run inside a linked git worktree reads and writes the main checkout's
tracker.** Editing the worktree's `.moai` makes that file conflict on the merge,
and then the only way out is outside the tool — one line on stderr says where it
wrote. So that worktree's `.moai/issues.jsonl` stays as it was when the worktree
split off, and the current rows are in the main checkout's file.

**The assignee comes for free** — whoever created it is the assignee. To hand it to
someone else, `-a "Name (email)"`; to leave it unowned, `-a none`. The name and
email come from `git config`, and when they are not there you pass them with
`--user "Name (email)"` or `MOAI_ACTOR`.

**Work that is not yours is asked about.** `moai ready` and `moai prime` hand out
only your own rows; a row assigned to someone else, or to nobody, stands apart under
`others` (`owner` is `theirs` or `unowned`). Ask the person before you pick one up,
and on a yes take it over and say who said yes:

    moai mv <id> in_progress --from todo --take -m '<who said yes>'

You become the assignee in the same write and a note `Taken-over: <who it was|none>`
keeps whose it was — it also takes a row that already stands in that column, so give
`--from` the column you saw: if the owner picked it up meanwhile, nothing is taken. Without
`--take` a person in a terminal still moves it (one line on stderr says whose it is);
the hook refuses it (rule 5). Who you are is matched by name or email, the same as
`-a me`; when it is unknown nothing is set apart.

### The three forks

**1. `add` or `idea`** — what decides is *whether you would pick it up now.*
If you would, `moai add`; if it is for later, `moai idea add '<what came to mind>'`.
An idea stands on neither the board nor `ready`, so it does not blur the plan.
**Walking past it without writing it down is the worst of all.**

If it came out of an epic, ask one more question first — *can this epic deliver
what it promised without this?* If not, it is not for later: it is this work,
unfinished. Even when you cannot do it now (waiting on a person's decision, the
work beside you holds that file), create it as a member with `-e <epic>` and
leave it in the first column — a member still standing keeps the epic from
closing by itself. Send it out as an idea and the epic stands `done` without
having delivered what it promised. To `defer` such a member is to decide to give
that promise up.

**2. `defer` or `done`** — never move to `done` what you decided not to do.
`moai defer <id> -m '<why>'` changes neither the column nor the kind, and
`--undo` brings the same row back as it was. An idea is "not work yet"; a defer
is "work, but not now".

**3. Is it worth splitting into an epic** — if the request does not end inside one
file, show the person **once**, before writing code, a plan split into one epic
plus three to seven issues, and ask. On a "yes", create it in one go with
`moai add --from -` (`--dry-run` shows it first).

```sh
moai add --from - <<'PLAN'
# Epic title
- [p1] first issue #enhancement
- [p2] second issue
PLAN
```

**A running milestone is the person's to fill.** When one is running, ask in the same
breath as the plan whether this bundle belongs in it, and attach it only on a yes —
`moai add --from - --milestone <id>`. It goes onto the epics the plan creates and the
members inherit it; without it the whole plan stands outside the release, and of it
`moai ready` then hands out only what is `p0`. Do not hang a running release on a plan
because the plan looks urgent: that is the release changing size while it runs.

**`--body` says why these issues are one bundle.** It goes onto the first epic the
plan creates, which is where `moai show <epic>` reads it from. `--body <text>` takes
the text itself (a file path there becomes the body as written), `--body -` reads
stdin, and only `--from <file>` reads a file. `--body -` and `--from -` cannot both
read stdin, so either put the plan in a file and stream the body —
`moai add --from plan.md --body -` — or keep the plan on stdin and pass the body as
text — `moai add --from - --body '<text>'`.

### What to write in an issue

**Pass the title and the body separately.** The title is an argument; the body is
markdown streamed in from stdin with `-b -` — a heredoc is easiest. Do not repeat
the title inside the body.

- **Keep the title short.** One line on what went wrong. The board, `ready` and the explorer list show the title alone
- **Do not write the body as a narrative.** Instead of laying out what happened in paragraphs, split it into a list —
  what went wrong, what you saw, where it gets fixed
- **Do not use emoji** — not in the title, not in the body. Their width differs per terminal, so the board and the tables come out crooked

The one thing that matters is that the next session reads this with
`moai show <id>`. These three are advice for that reason, not rules anything checks.

### Park what is out of scope

    moai idea add '<what just came to mind>'       park it
    moai idea add '<a longer thought>' -b -        the body comes from stdin
    moai idea ls                                   see what has piled up
    moai show -g <keyword>                         find out whether it is written down already

When the time comes, unfold one into an epic and issues. Unfolding closes the thought.

```sh
moai idea promote <id> --from - <<'PLAN'
# Epic title
- [p1] first issue #enhancement
PLAN
```

**A line in the plan becomes the issue title verbatim.** Copy over an idea title
that grew long while you parked it and that length spreads into the issues, so
write a short new title when you unfold — the original text stays on that idea,
and the history line about being unfolded from it leads back there.

**The idea's milestone and body go onto the epic by themselves.** `promote` puts
both on the epic it unfolds — a milestone is inherited, so the epic alone carries
it to every member, the ones added later included, and the body is what lets
`moai show <epic>` say why these issues are one bundle. Not onto every issue: the
original stays on the closed idea and the history leads back to it, and the one
place worth filling is the epic, so the window that picks a member up does not
have to press every member to find out what this is.

**The milestone that comes over is the one `moai show --milestone` stands the idea
under**, not whatever its own field says — an idea parked inside an epic comes over
in that epic's release even with an empty field of its own, and a field of its own
that loses to the epic it sits in never reaches the new epic. One reader answers
where a row belongs, on every surface.

**Unfolding into a standing epic (`-e <epic>`) carries neither.** That epic is
already the owner — its members inherit its milestone, and writing the idea's over
theirs would stand one bundle in two places.

**If what came over is not the release that is running, leave it where it stands.**
Work is never pulled into a running release, so the epic stays outside it and what
you do is say so — the person attaches it, with this line, if it belongs in the
release:

    moai edit <epic> --milestone <milestone>

`promote` carries the release the idea stands in whatever state that release is in,
so an idea parked with no milestone, one parked under a release that has since
shipped, and one parked under a milestone since deferred all come over exactly as
they stood. **A dead one you do clear yourself** — nobody chose it here and it hides
the new epic: `moai edit <epic> --milestone none` on a release that has shipped or
been deferred. What standing outside costs meanwhile: every member under that epic
is work picked up from outside the running release, of which `moai ready` hands out
only what is `p0`; and under a deferred milestone the whole plan is out of the plan
the moment it is created — not in `ready`, not in `held`, and no warning says so.
**A dead release is said out loud**: unfolding into a deferred or closed milestone
prints one line on stderr naming it, and nothing is blocked.

The id in that line is the release `moai show --milestone` stands that idea under. Only its shape is checked, so `moai-zzzz`
goes in with exit 0 — but one line on stderr says there is no such milestone,
and `moai status` counts the row as `dangling_milestone`.

### When work already created is not for now

    moai defer <id> -m '<next quarter>'    take it out of the plan for a while
    moai defer <id> --undo                 take it back
    moai show --deferred                   see only what is deferred

Neither the column nor the kind changes — the same row comes back as it was.
What is deferred drops out of `moai ready`, the board and the warnings, and
`moai status` shines one line on it. Defer an epic, a milestone or a parent and
the work under it drops out with it.

### There are two kinds of group

    moai epic add '<storage layer>'                an epic
    moai milestone add 'v0.1' --start 2026-09-05 --due 2026-09-20
    moai add '<title>' -e <epic> --milestone <milestone>
    moai show <epic|milestone id>                  what stands under it
    moai show --milestone <id>                     everything attached to that milestone

**A milestone is the one row that carries dates.** `--start` and `--due` take a
calendar day, `YYYY-MM-DD`, and `moai edit <milestone> --due none` clears one.
They stand on a milestone row only — on anything else the write is refused. A
deadline that has passed, and one falling due within `status_due_days` (3 unless
your config says otherwise), is one warning line on the board; **nothing is
blocked and the exit code never changes.**

**`moai show <milestone>` also says how long it took.** That is read from the
closed members' start and finish right then — no field holds it. It comes with
the number it could measure ("2 of 3 closed"), because a member with no
`started_at` is *unknown*, not zero, and it is wall clock, not effort: sessions
running beside each other overlap, and waiting on a person counts too.

**Belonging is inherited.** A child inherits its parent's epic, an issue inherits
its epic's milestone. A child created with `--parent <epic>` belongs to that epic.
Do not write it again on every issue — move the epic and the members come along.

**A plan gives its members the epic's own id.** `moai add --from` and `moai idea
promote` mint `<epic>.<body>` for every issue in the plan, the way `--parent
<epic>` always did for a review — one subject, one id. The member carries no
`epic` field of its own, so `jq -r .epic` on it is `null` while `jq -r
.derived_epic` names the epic; `moai show <epic>` lists it under `members` and
`moai show -e <epic>` picks it up. Rows created before keep the ids they have:
nothing is ever relabelled.

**An id cannot move, so such a member cannot leave its epic.** `moai edit
<member> -e none` says so and changes nothing; `-e <another epic>` does move it,
and the id it keeps still reads `<first epic>.<body>`. Aim a plan before you
unfold it — for a member that has to stand somewhere else, create it there.

**A group's column is read from its members.** Do not `moai mv` an epic or a
milestone — pick up one member and it stands `in_progress`, finish them all and it
stands `done`, by itself. To give up on a group while members are left, `moai defer`
those members — and if no member ever finished, deferring them leaves it in the
first column, so defer the group itself to take it out of the plan.

**While a milestone is running, what is inside it comes first.** Whether it is
running is read the same way as above — if even one member stands in a started
column, it is running. There is no command that opens it and no new field.

- `moai ready` gives out the work inside it and leaves only `p0` outside. What is
  running, and how many it held back, is one line under the list; on the board it is a `moai status` notice
- **`p0` gets picked up whether or not it is in the milestone** — that is the hotfix
  slot. In the ordering `p0` comes first and the milestone second
- **Work is never pulled into a running milestone.** What ships was decided before it
  started, and three doors put work in afterwards — the person opens two of them. They
  attach it (`moai edit <id> --milestone <milestone>`); or they say yes to a plan you
  showed them and you attach it in that same breath (fork 3); or it came out of a member
  you are working on and is created inside that member's epic (`-e <that epic>`), where
  the release is inherited. Writing `--milestone <the running one>` on a row that stood
  outside, or moving such a row under an epic that is in it, is none of those three: it is
  you deciding what the release contains, so say it to the person and leave the row where
  it is
- **Nothing is blocked.** A `moai mv` that picks up work from outside goes straight
  through, and so does a `--milestone` that carries a row in — the rule above is a rule
  for you, not a refusal. What is already picked up is simply finished — the same ground as never taking work back late
- If two milestones are running, both are inside, and the ordering within them is as it always was (`p` · age)
- **A setup with only two columns has no running milestone** — there is no column
  that says "started but not finished", so the rule itself does not stand. In that
  setup `ready` and the board are as they were

### Several projects

    moai project add <dir>                 register it in my config (accepted even without `.moai`)
    moai project ls                        what is registered and how it stands

Called outside a `.moai`, `moai`, `moai status` and `moai ready` give an overview
of every registered project, one block each. Add `--worktree` and each project
overlays its sibling worktrees too. Other commands cannot tell which project you
mean, so call them as `moai -C <dir> <command>`. In `moai tui`, `0` puts every
registered project into one list — a header row per project with that project's
rows under it. Outside a `.moai` it opens there; inside one it opens within that
project. The top header numbers each project, and pressing that number jumps
straight to it — `0` is the single list.
On a header row, `l`/`→` expands it (that is when the project is read) and `h`/`←`
folds it; `Enter` goes inside. Parking (`SPC n`) and marking read (`r`) go to the
project of the row under the cursor, and search and filters apply within a project only.
From there, `SPC p a` picks a directory to register (a monorepo subdirectory
counts separately) and `SPC p d` takes one off the list. With nothing registered,
opening outside still shows an empty list that points at `SPC p a`.

### The language on screen

English is the default. For another language, pass it as in `MOAI_LANG=ko moai status`,
or write `lang = "ko"` under `[i18n]` in your user config (the environment variable
wins over the config). The languages are en, ko, zh, ja and es, and text a language
does not carry yet comes out in English — the two that are full right now are en and ko.
**The system locale (`LANG`, `LC_ALL`) is not read**: it changes only through one of
those two ways. How to add a translation is in the moai repository's `i18n/README.md`.

### Checking for a new release

The explorer asks GitHub once a day whether a newer release is out, and the version
line in its header says which of four it is — a new release, the latest, ahead of the
latest (a build from source), or not asked. **Nothing is blocked**: it is one line, and
the exit code never changes.

It only asks where a person is watching. `--json`, a pipe and anything that is not a
terminal never ask, so a machine running agents does not knock on the outside every run.
The answer, when it was asked and where it was asked are held next to your user config
in `latest.toml`. Ask somewhere else and the answer from the other place is not reused.

    [update]
    check = false        # in your user config: never ask on this machine

    MOAI_NO_UPDATE_CHECK=1 moai tui     # or just for this run

### When a feature request comes in

1. Look at the epics that already exist with `moai status`. If it may overlap, `moai show -g <keyword>`.
2. Show a plan split the way fork 3 says, once, and on a "yes" create it in one go.
3. Pick it up with `moai mv <id> in_progress`, and move it to `done` when it is finished.
4. Park what you find along the way that is out of scope with `moai idea add` — if the
   epic promised it, it is not an idea even when you cannot do it now (fork 1).
5. Why it was decided goes on the issue with `moai note <id>`. The next session reads
   it with `moai show <id>`.

### Name the id in the commit

Put the id of the issue a commit touched in the commit subject — `feat: draw the blocked line (<id>)`.
moai does not store commits on the issue. `moai show <id>` and the explorer detail
find that issue's commits on the spot, **by the id written in the subject**. Do not
copy hashes into notes — one squash or rebase makes them stale, and the commit that
closes an issue does not know its own hash in advance.

- Put it **in the subject**. In the body, only lines that open with a trailer word count — `Refs:`, `Closes:`, `Fixes:`.
  An id written anywhere else in the body does not (a tracker commit lists a whole run of
  ids, and they must not all stand as that issue's commits)
- **A repository that squashes adds the trailer.** A squash moves the subjects of the
  commits it folded into the body, so by subject alone those commits vanish whole —
  one `Refs: <id>` line keeps them
- Ids match whole-word. A child's commit (`<id>.x1y`) is not the parent's — a commit
  that takes review findings in belongs to the review by the review issue's id, and the
  `merge: … (<id>)` that folds a worktree in belongs to the work
- A commit that carries nothing but a pick-up or a close starts with `chore(tracker):`.
  The detail leaves those out of the drawing (they stay in `--json`'s `commits`, flagged `tracker`)
- `commits` in `moai show <id> --json` is **always there.** An empty array means "no
  commit names that id", and `commits_error` stands only when git could not be read —
  it is there so a machine can tell "nobody has touched it yet" from "it could not be
  asked here". That value is an object with `kind` and `said`. What you branch on is
  `kind` (`no_git`, `not_a_repo`, `stream`, `encoding`, `failed`); `said` is one line
  for a person to read, so do not match on its words

When the tool grows and this block goes stale, call `moai init` again. It touches
neither the issues nor the journal; it rewrites this block only. To see only
whether it is stale, `moai init --check` — it writes nothing and answers
`current`, `stale` or `missing`.

### When the issue file has to be merged

In `.moai/issues.jsonl` one line is one issue and the file is sorted by id, so two
branches that changed different issues collide for no reason other than their lines
being neighbours. Install the merge driver and git resolves that per issue, 3-way.

    moai merge-driver --install

**Run it once per clone.** git reads the driver command from config only, and
config is not committed. In a clone without it, `merge=moai` in `.gitattributes`
is simply ignored and git's default merge runs — merging is exactly as it was
before. `moai status` still says one line about it not being installed: a reminder
not to quietly miss the one thing each clone needs, and it blocks nothing.

**If the path it recorded disappears, merging falls back to git's default.** What
gets recorded is the absolute path of the binary that was running. When that path
is empty git treats it as a conflict but leaves this side's file as it was, and
without markers in it whoever runs `git add` throws the other side away whole —
which is why the install writes neither an answer nor markers and re-merges the
failed run with `git merge-file`. The markers stand, and the worst case is the
same as a clone with nothing installed. Keeping the path alive is still better:
the fallback loses the per-issue resolution. The place it records (`--local`) is
shared by the clone, so running it from a linked worktree's `target/` puts every
checkout in that state the moment the worktree is removed. Pass `--as` with a
path that will not disappear. What is merged and how is in
`moai merge-driver --help`.

### Name the AI that did the work

Before you close it, leave one line on the issue naming the AI that actually did the
work. It is a note, not a field.

    moai note <id> 'model: <vendor>/<model> tokens=<count> (<grade> — <why>)'

- The vendor is `anthropic`, `openai` or `google`; the model is its real name (`opus-5`, `sonnet-5`); the grade uses the same words as the review grades
- **If you do not know the token count, drop `tokens=`.** Do not write 0 and do not estimate — blank, 0 and a false number are three different things
- **One line per id.** Write the same line on several ids and the tokens multiply by the number of ids
- **Quote free text with single quotes** — inside double quotes the shell expands
  backticks and `$(…)` as commands, so the text is cut off and the call ends in 0.
  If the text itself contains a single quote, stream it from stdin with `-b -`
- `work` in `moai show <id> --json` reads those lines out. It is **always an array**,
  and a line that does not fit the form simply does not become a value — it stays a
  note. Only lines that start at the beginning of a line count; indented lines and
  lines inside a fence are read as examples
- A list, `moai show [filters] --json`, gives the same `work` on every row — when you
  are adding several issues up, call the list once instead of calling per id, with
  `--archived` so every closed row is in (`--all` leaves out what has stood in done
  for `archive_days`), or let `moai stats --json` add them up: its `work` sums tokens
  by model and by grade and counts the lines that carry none apart

### The five things the hook actually watches

They stand once `moai skill install` has planted the hooks for your agent.

**1. New issues stay inside what you picked up.** The issue in focus is the one you picked up — it has left the
first column and is not closed yet (`in_progress`·`review`).
Anything that comes out of that work belongs in the same epic (`-e <epic>`) or
under that issue (`--parent <id>`). If the epic cannot deliver what it promised without this, it is one of those two
even when you cannot do it now (fork 1). If it is not for now, park it with
`moai idea add` — an idea is always free of this rule, and so is
`moai add --from` (what it creates is an epic and its children, one unit on its own).

**2. Pick something up before you change the repository.** `moai mv <id> in_progress`.
What counts is work inside the repository — `.moai/`, `.claude/`, `.worktrees/`, `target/`
and anything outside the repository (scratchpad, temporary files) do not. Shell
writes (`>`, `>>`, `sed -i`, `tee`) count as much as `Edit` and `Write`. If it
was not in the plan, create it with `moai add 'a title'` and pick that up.

**3. A review is an issue too.** Before you call a review, create a review issue tied to
what you are reviewing. The review is `/code-review` in Claude Code; in Codex and
Antigravity it is the review this session has, else read the diff yourself.
It runs inside your own session — never start another agent program for it. The other
steps that differ per agent are under "Words per agent" in the `moai` skill.

    moai add 'review — <what you are looking at>' -t review --parent <the issue> -b '<what you are looking for and why>'
    moai mv <id> in_progress      when the review starts
    moai note <id> -b - < <review text>   the reviewer's own words (summarize past 64KB)
    moai mv <id> done -m '<what you took in, what you handed on>'

The angle (`-b`) and the closing line (`-m`) are **actually required.** Calling
without them is refused, and the refusal hands you the command to fix it.
**Do not ask a human** — running that command as given goes through.

**Keep the reviewer's words and your own call in two notes** — what the reviewer
said and what you decided are different texts. Write what you handed on **with
the issue id**. A line that only says "handed on" is never read again. Where the
review text lives is in the skill's `references/commands.md`.
If the text runs past 64KB, summarize it — put `Summary: original <size>KB agent-<task-id>` on
the first line, keep every finding's number and place, and shorten only the sentences. Leave fences and indentation alone.

**4. Never kill the person's tmux server.** `tmux kill-server` and `kill-session` without `-L`/`-S`, and
`pkill`/`killall` aimed at tmux, are refused. When the session runs inside tmux
`$TMUX` is set, so a bare `tmux` ignores `TMUX_TMPDIR` and attaches to that
server — one line kills every session in it. A tmux you are testing gets its
own server.

    env -u TMUX tmux -L <unique name> …

**5. Ask before you pick up someone else's work.** A `moai mv` into a started column on a row whose assignee is
someone else — or nobody — is refused unless it carries `--take`. Ask the person
watching first. On a yes, take it over and say who said yes:

    moai mv <id> in_progress --from todo --take -m '<who said yes>'

You become the assignee in the same write, and a note `Taken-over: <who it was|none>`
keeps whose it was. `moai ready` hands out only your own rows and sets the rest
apart (`others`), and what someone else picked up is not your focus. When who you
are is unknown, nothing is refused.

**Where no hook stands, the rules are words only.** The hooks stand where
`moai skill install` planted them for your agent — Claude's plugin, Codex's
`.codex/hooks.json` once a person has trusted it in `/hooks`, Antigravity's
`.agents/hooks.json`. An agent without them, and a tool call that sends no hook
(Codex sends them only for its shell, `apply_patch` and MCP calls) or that the
hooks do not read (input typed into a command already running), is refused
nothing — keep the five yourself. Rules 4 and 5 most of all: they guard the
person's other sessions and other people's work, and nothing else will.

### The supervisor and its workers

`moai skill install` also plants `moai-work` and `moai-supervise`. A person calls
`moai-work` in a window to make it a worker — it says hello, waits for a letter,
does the work the letter hands over in a worktree, reports and waits again — and
`moai-supervise` in one window to hand the ideas that have piled up, one at a time,
to those workers and take their reports. Claude Code, Codex and Antigravity can
each be either, and every one of them is a session a person opened. The supervisor
picks, sends and checks; it does not fix and it does not merge.

### Letters between agents

    moai send '<agent>' '<subject>' -b -    leave a letter - one file under .moai/mail
    moai send any-idle-worker '<subject>'   one agent takes it - not you, not a supervisor
    moai inbox --ack                        the letters for you, marked read as they are shown
    moai inbox --ack --wait 600             a worker waits here for its next letter
    moai agents                             who is here - `moai hello` registers you

A letter is delivery, not record: nothing goes into the tracker, so a decision still goes
on its issue as a note. With the hooks installed you rarely run `moai inbox` — each prompt
and the end of each turn load the letters for this session and mark them read, and an
`any-idle-worker` letter goes, one per load, to whichever agent loads it first. A supervisor
registers with `moai hello --role supervisor` so it never takes those. Waking is a bonus:
`--wake` types `moai inbox` into an idle recipient's tmux pane when its row has one, a
Claude session is woken by the sender with SendMessage, and otherwise nothing happens —
a worker waiting on `moai inbox --wait` needs no waking, and while it waits `moai agents`
shows it idle. A Codex shell is found by the session id Codex sets in it
(`CODEX_THREAD_ID`); where that is missing, pass `--as <name>` — the hooks name the session
in its first context.

### The wiki

The repository's manual is markdown pages under one directory — `docs` unless `wiki_dir`
in `.moai/config.toml` says otherwise. `moai wiki ls` lists them and `moai wiki show <slug>`
prints one. When an epic changes what a person does, the window that did it fixes the page
on its branch before the merge, and `moai skill install` plants a skill for it, `moai-wiki`,
that says how — and sweeps the wiki when a person calls it. Nothing checks this.

### Before you close the session

Run `moai status` once more and see whether the warnings grew. Warnings block
nothing — they shine a light on issues with no epic, reviews stalled for a long
time, and how much you have open at once. Ideas piling up and what is deferred are
not warnings; they stand apart as notices (`notices`).

If you end the session still holding something, leave one line on that issue for
the next session to take over from. The next session reads it in the history under
`moai show <id>`.

    moai note <id> 'Next: <what comes next>'
<!-- moai:end -->
