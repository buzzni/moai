# Every command

`moai --help` and `moai <command> --help` are the truth. This file is a summary of
them, so where they differ the help wins.

## There are two kinds of group

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
- **Nothing is blocked.** A `moai mv` that picks up work from outside goes straight
  through. What is already picked up is simply finished — the same ground as never taking work back late
- If two milestones are running, both are inside, and the ordering within them is as it always was (`p` · age)
- **A setup with only two columns has no running milestone** — there is no column
  that says "started but not finished", so the rule itself does not stand. In that
  setup `ready` and the board are as they were

## Filters

A comma means "or"; the same flag twice means "and".

    moai show -s todo -t bug          todo and bug
    moai show -s todo,review          todo or review
    moai show -e none                 the ones with no epic
    moai show --deferred              only what is deferred
    moai show --stale 7               stuck in the current column for more than seven days
    moai show --tree                  epic → issue → child

## Seeing the worktrees together

When agents each take a git worktree, this worktree's board knows nothing of what
was picked up and moved beside it. Add `--worktree` to `status`, `ready` or `show`
and the sibling worktrees' issues are overlaid. The explorer (`moai tui`) opens with
them overlaid and `SPC v w` turns it off and on.

    moai ready --worktree             work picked up beside you drops out and stands under "held"
    moai status --worktree            the board header says "⎇ <worktrees> overlaid"
    moai show --worktree --json       only rows from beside you carry a "branch" key

For the same id, the row that changed its place in the plan last wins — the later
column move (`status_since`) or defer / undefer (`planned_at`); on a tie the later
`updated_at`; on a tie again the row on the current branch. So a field-only edit does
not undo a pick-up made beside you, and a later defer made beside you is not hidden
behind a column you picked up here first.
A row `rm`-ed here does not come back as the sibling's row — if it existed at the
point they split (`git merge-base`), it is read as deleted here. A row picked up or
changed beside you after that does stand.
A memo (`moai note`) does not change the snapshot, so it does not count as a change —
a row that only got a memo beside you stays hidden, and that memo surfaces from the
journal when the branches are merged.
A row that is not on the current branch carries `⎇ <branch>` before its title.
**They are overlaid for showing only** — no file changes.

**Writes go to the root's tracker.** A `moai` called inside a linked worktree reads
and writes the main checkout's `.moai` — editing the worktree's snapshot makes that
file conflict on the merge, and then the only way out is outside the tool. One line
says where it wrote. To edit that worktree's tracker on purpose, pass `MOAI_HERE=1` —
that is the place that builds a state where the sibling snapshots have diverged.

## Contested pick-up — `--from <column>` on `mv` and `defer`

When several sessions share one `.moai`, a session beside you picks up the same row
between your `ready` and your `mv`. `--from <column>` moves it **only while the
column you saw still holds** — it is re-read inside the lock, so a row that changed
in between is not touched. Without it nothing is blocked, as before.

The shape of the call is shown end to end in the moai repository's `examples/bash-agent/agent.sh`.
Cut short it is this — `col` is the column of that row as `ready` gave it, and `moved`
says whether you got it.

```sh
row=$(moai ready --json | jq -c '.ready[0]')
id=$(jq -r '.id' <<<"$row")
col=$(jq -r '.status' <<<"$row")
claim=$(moai mv "$id" in_progress --json --from "$col") || {
  [ -n "$claim" ] || exit 1          # empty stdout is a failure, not a lost contest
  continue                           # lost — on to the next
}
[ "$(jq '.moved | length' <<<"$claim")" -gt 0 ] || continue
```

- **Pass one id at a time.** Several at once mix the rows you won and the rows you
  lost into **one exit code** — the won rows have already moved while the caller
  believes it picked up nothing
- A lost row stands as one line on stderr (`moai: <id> already stands <column> — not moved`)
  and as `stale: [{"id":…,"status":<the column it stands in>}]` under `--json`. The
  `already <column>` on stdout is **a different thing** — that row was already in the column
  you asked for, and the exit code is 0. Both lines come out of the language bundle, so they
  read in whatever language the screen is set to — tell the two apart by the exit code and
  by `--json`, never by the words
- **Not every non-zero code means "lost".** No identity, the lock being held, a
  mistyped column and a broken row all come back with the same code. Losing a contest
  puts the row on stdout; a failure puts `{"code":…}` on stderr and leaves stdout
  empty — read a failure as "someone else took it" and the same row comes round again
  and stops with the wrong reason
- **`--from` matching is not the same as picking up.** In a setup where the first
  column is also the working column, the column matches, nothing moves, and it exits 0
  with `already` — look at `moved` as well
- **`--from` cannot be used on a group (epic, milestone)** — it is refused
  (`bad_status`). A group's column is read from its members while the write goes to the
  column on the row itself (`deferred_at` for a defer), so the axis you measure and the
  axis you write diverge and **both** contestants win. A guard that pretends to bite is
  worse than no guard — pick up a member, or give up on the group with `moai defer` and
  no `--from`
- `defer`'s `--from` also looks at the **column**. A defer does not change the column,
  so it filters out a row whose column moved because it was picked up beside you, but a
  contest where **both sides defer** is not decided by it
- `--from` takes **a column it knows, or a column some row actually stands in**. A name
  nobody stands in is refused (`bad_status`) — read a typo as "it did not match" and it
  does nothing while returning a code, and the caller cannot read why. What the refusal
  is aimed at is the typo, not staleness, so after you rename a column in `config` a row
  still standing in the old name can be picked up by that old name

## Creating in one go

A `#` line is an epic; a `-` line is an issue of the epic just above it. `[pN]` and
`#tag` are optional. If a title starts with `[` or ends with `#word`, write it as
`\[` / `\#` (`- \[WIP] issue \#12`).

```sh
moai add --from - <<'PLAN'
# Storage layer
- [p1] write atomically #enhancement
- recover a truncated line #bug
PLAN
```

`--dry-run` keeps a heredoc typo from creating six of the wrong things.

Keep a plan you repeat in a file and fill `{{name}}` with `--var name=value` (the
name takes letters, digits, `_` and `-`, no spaces). By convention it lives in the
repository at `.moai/templates/<name>.md` and is called with `--from <path>`.
Every variable is required — a name you did not fill, an empty value, a value with a
newline in it, a name not in the plan, and the same name twice are all refused and
create nothing. The value becomes the title text exactly as written, so put variables
in title positions only (in a tag or priority position it is refused). To put a literal
`{{` in a template title, write `\{{`. A backslash immediately before `{{` is counted
in pairs — for a literal backslash followed by a variable, write `\\{{name}}`
(`C:\\{{dir}}`). `idea promote --from` takes the same `--var`.

    moai add --from .moai/templates/release.md --var version=1.2 --dry-run

## Several projects

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

## The language on screen

English is the default. For another language, pass it as in `MOAI_LANG=ko moai status`,
or write `lang = "ko"` under `[i18n]` in your user config (the environment variable
wins over the config). The languages are en, ko, zh, ja and es, and text a language
does not carry yet comes out in English — the two that are full right now are en and ko.
**The system locale (`LANG`, `LC_ALL`) is not read**: it changes only through one of
those two ways. How to add a translation is in the moai repository's `i18n/README.md`.

## Checking for a new release

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

## Unfolding a parked thought

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

**If what came over is not the milestone that is running, hang the running one on
the epic yourself** — or clear it with `--milestone none` when nothing is running.
`promote` carries the release it stands in whatever state that release is in, so
that covers an idea parked with no milestone, one parked under a release that has
since shipped, and one parked under a milestone since deferred. Without this the
epic stands outside the release and every member under it is work picked up from
outside it, of which `moai ready` hands out only what is `p0`; and under a
deferred milestone the whole plan is out of the plan the moment it is created —
not in `ready`, not in `held`, and no warning says so. **A dead release is said
out loud**: unfolding into a deferred or closed milestone prints one line on
stderr naming it, and nothing is blocked.

    moai edit <epic> --milestone <milestone>

Copy that id off the line `moai ready` prints under its list for the running milestone. What is checked is the shape
alone, so `moai-zzzz` goes in with exit 0 and surfaces only much later as a
`dangling_milestone` warning.

## Deferring

    moai defer <id> -m '<next quarter>'    take it out of the plan for a while
    moai defer <id> --undo                 take it back
    moai show --deferred                   see only what is deferred

Neither the column nor the kind changes — the same row comes back as it was.
What is deferred drops out of `moai ready`, the board and the warnings, and
`moai status` shines one line on it. Defer an epic, a milestone or a parent and
the work under it drops out with it.

## People

**The assignee comes for free** — whoever created it is the assignee. To hand it to
someone else, `-a "Name (email)"`; to leave it unowned, `-a none`. The name and
email come from `git config`, and when they are not there you pass them with
`--user "Name (email)"` or `MOAI_ACTOR`.

## What to write in an issue — an example

The rule is under "What to write in an issue" in `SKILL.md`. This is one set that keeps it.
The title goes as an argument, the body through `-b -`.

```sh
moai add 'an empty tag slips past the filter' -t bug -e <epic> -b - <<'BODY'
- What: normalizing tags leaves an empty word in place
- What I saw: a list filtered by that tag filters nothing and returns everything
- Where: where tags are normalized (<file>:<line>). Dropping the empty word ends it
BODY
```

Three common ways it goes wrong.

- **The title carries the whole story** — "yesterday while I was … it … so I think … needs checking".
  The board and `ready` cut that line off, and what went wrong is usually not in the part that survives
- **The body is written in paragraphs** — the next session has to dig "where it gets fixed" back out of them.
  Split the call, the evidence and the next step into lines and one `moai show` is enough
- **Emoji mark what is urgent** — urgency goes in the priority (`-p 1`). That is the one `ready` reads

## Korean text

The always-visible rule is under "Korean text" in `SKILL.md`. This is the procedure.

- For review text, polish the sentences only — do not shorten it and do not mix your own call in.
  That is what rule 3 means by taking the reviewer's own words. Shorten it only past 64KB, and say so with `Summary:`
- Move text longer than 20 lines outside the repository (a scratchpad or a temporary directory),
  make that the cwd, and call `humanize-korean:humanize-korean` there. The skill creates `_workspace/` in the cwd — delete it when you are done
- Come back into the repository afterwards — the hook finds the tracker from where the session stands, and standing outside it no rule stands at all
- On an ordinary technical term's first mention in a body, put the original in parentheses after
  the translation and use the translation alone from there. **That rule is not for code names** —
  a name that came from the code goes in exactly as it is, every time, and a gloss goes in
  parentheses after it. Take it the other way round and "the translation alone from there" drops
  the name, which is the swap this rule exists to stop. A repository that wants a fixed list of
  its own terms keeps that list in its own docs — this rule stands without one
- If polishing changed the meaning, go back to the original text. Polishing fixes sentences, not facts
- If a plugin is missing, do not install it yourself — ask the person to run `moai skill install` again.
  That installs both of the plugins below in the same scope as moai. Without them moai blocks nothing

    korean-skills@korean-skills     https://github.com/DaleSeo/korean-skills
    humanize-korean@im-not-ai       https://github.com/epoko77-ai/im-not-ai

## Name the id in the commit

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

## When the issue file has to be merged

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

## Name the AI that did the work

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
  are adding several issues up, call the list once instead of calling per id

## How to find what a review said

The full review text stays in a file. The notice that it finished carries a
`task-id`, and that is the file name.

    ~/.claude/projects/<project>/<session>/subagents/agent-<task-id>.jsonl

The full text is the **`message` of the last `SubagentHandback` call**. Only when
there is no such call is it the **last `text` block**.

```sh
python3 -c "
import json,sys
b=[c for l in open(sys.argv[1])
   for c in json.loads(l).get('message',{}).get('content') or []
   if isinstance(c, dict)]
h=[(c.get('input') or {}).get('message') or '' for c in b
   if c.get('type') == 'tool_use' and c.get('name') == 'SubagentHandback']
t=[c['text'] for c in b if c.get('type') == 'text']
print(h[-1] if h else t[-1] if t else '')" <that file> | moai note <review id> -b -
```

**Where the report was handed back, the last `text` block is not the review text.**
A subagent that handed its report over through that call writes one more closing line
after it, something like "report sent" — take the last `text` only and a 200-byte
closing line stands in as the review text, and the size check measures that instead.
It goes wrong quietly, so the one-liner above looks at the handed-back report first.

**Do not simply take the last line.** One turn's blocks are written line by line with
thinking and tool calls mixed in, so the last line is sometimes not text at all. Then
an empty text is passed on and `moai note` stops, saying the memo is empty. It stops loudly with a
non-zero exit code, so nothing is lost — but do not match on the words: that line is screen text
and comes out in whatever language the screen speaks. Getting it right the first time is better.

**Do not keep the summary and throw the original away.** A summary is your own call;
the original is what the reviewer said. A call can be made again; a discarded original
cannot be recovered.

**Shorten it only when it overflows.** One write takes up to 64KB, so `moai note`
refuses text larger than that.
If the text runs past 64KB, summarize it — put `Summary: original <size>KB agent-<task-id>` on
the first line, keep every finding's number and place, and shorten only the sentences. Leave fences and indentation alone.
Saying it is a summary keeps the next person from reading it as the reviewer's words,
and the `agent-<task-id>` on the first line is the file name above, so the way back to
the original stays open. **Do not split it across several notes** — the journal only
appends, so a split stays forever, and the way the person chose is the summary (moai-b8aj).
Fences and indentation are left alone because a `model:` line copied over, standing at
the beginning of a line, puts work nobody did into `work`.

## The hook

    moai hook <event>    Claude's hooks call this. A person never runs it by hand

The plugin planted by `moai skill install` is what calls it. Whatever goes wrong the
exit code is 0 — a noisy hook gets turned off, and a rule that is off is no rule.

**It may be called from inside a git hook.** git hooks and a linked worktree's
`rebase -x` export the `GIT_DIR` family, and those beat `git -C <path>`, but moai
strips those variables before it starts git. So which **repository** is read is decided
by `-C` or by the search for `.moai` — a `moai -C B` called from repository A's hook
reads only B's history for the commit column and for `--worktree`.

**The person comes from B too.** A value a hook inherits because the caller ran
`git -c user.name=…` is stripped as well — carry that through a `-C` write into
someone else's project and their name stays in B's journal forever. So a name that
exists only in A's repository config is no longer read: with no name in B and none
globally, the write cannot find a person and stops, and stopping inside a hook fails
that commit. In that spot, pin it with `--user "Name (email)"` or `MOAI_ACTOR`.

**It is the same whichever directory you call it from.** Both the person and the
commit column are read from that tracker's `.moai` root — even with another repository
nested inside the project (a submodule, `vendor`), a `moai` called in there does not
write that repository's name.
