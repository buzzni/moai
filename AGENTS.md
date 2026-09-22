<!-- moai:begin v:0.1.1 hash:b9c4f6c2 -->
## Issue tracker — moai

This repository's work lives in `.moai/issues.jsonl`.
Do not use TodoWrite or a markdown TODO list. There is no approval gate — create anything, move anything. Do not ask a human.

Start a session by running `moai status`. The board and the warnings come up on one screen.

    moai status                            board · warnings · flow (start a session here)
    moai prime                             what you hold and what is next, nothing else
    moai ready                             what you can pick up right now
    moai show <id>                         body, children, history. Why it was decided is here
    moai show -g <keyword>                 find out whether it is written down already
    moai show -s todo -t bug               filters (comma = or, repeated flag = and)
    moai show --tree                       epic → issue → child
    moai ready --worktree                  overlay what the other worktrees picked up
    moai tui                               walk the explorer. SPC n parks a thought
                                           on a group row: l one step · Tab expand all · h fold
    moai add '<title>' -p 1 -t bug -e <epic>   create
    moai mv <id> in_progress               pick up  →  review  →  done
    moai edit <id> --tag parser            change
    moai note <id> '<what you found>'      a memo for whoever comes next
    moai defer <id> -m '<why>'             take work out of the plan for now

Every command takes `--json`. `ready --json` gives `{"ready":[…],"held":[…]}` —
`held` is what is deferred or blocked behind an empty group, and where to pick it up
again. That is enough to build a loop that runs without a person — one such loop, in
bash and jq alone, is the moai repository's `examples/bash-agent/agent.sh`.

**A key that cannot be absent is never absent.** `kind` and `priority` hold a default,
and the file leaves a default out, but `--json` fills it back in — `jq -r .priority`
on a row gives `2`, never `null`. Keys that genuinely can be absent (`epic`,
`milestone`, `deferred_at`) stay absent, and that absence is the answer.

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

### Korean text

**Polish Korean text before it goes into moai** — any title, body, note or `-m`
that carries even one Hangul character, review text included. Text written only in
English goes in as it is.

- Run `korean-skills:humanizer` to take the AI tell out, add `humanize-korean:humanize-korean`
  when it runs past 20 lines, and finish with `korean-skills:grammar-checker` for spelling and spacing
- Leave ids, commands, paths, numbers, code fragments and the fixed-form lines
  (`model: …`, `Next: …`, `Regression-of: …`, `Summary: original …`) exactly as they are
- **Keep the technical term, and never drop the original.** An everyday word carries several
  meanings, so once `layer`, `network` or `wrapper` is traded for one and the English behind it
  deleted, nobody can read the sentence back to the code — and
  a name that came from the code goes in exactly as it is
- `moai skill install` installs both plugins together. The detail is under "Korean text"
  in the moai skill's `references/commands.md`

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
    moai milestone add 'v0.1'                      a milestone
    moai add '<title>' -e <epic> --milestone <milestone>
    moai show <epic|milestone id>                  what stands under it
    moai show --milestone <id>                     everything attached to that milestone

**Belonging is inherited.** A child inherits its parent's epic, an issue inherits
its epic's milestone. A child created with `--parent <epic>` belongs to that epic.
Do not write it again on every issue — move the epic and the members come along.

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
The answer, and when it was asked, are held next to your user config in `latest.toml`.

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
  are adding several issues up, call the list once instead of calling per id

### The four things the hook actually watches

They stand once `moai skill install` has planted the hooks into Claude.

**1. New issues stay inside what you picked up.** The issue in focus is the one you picked up — it has left the
first column and is not closed yet (`in_progress`·`review`).
Anything that comes out of that work belongs in the same epic (`-e <epic>`) or
under that issue (`--parent <id>`). If the epic cannot deliver what it promised without this, it is one of those two
even when you cannot do it now (fork 1). If it is not for now, park it with
`moai idea add` — an idea is always free of this rule, and so is
`moai add --from` (what it creates is an epic and its children, one unit on its own).

**2. Pick something up before you change the repository.** `moai mv <id> in_progress`.
What counts is work inside the repository — `.moai/`, `.claude/`, `target/` and
anything outside the repository (scratchpad, temporary files) do not. Shell
writes (`>`, `>>`, `sed -i`, `tee`) count as much as `Edit` and `Write`. If it
was not in the plan, create it with `moai add 'a title'` and pick that up.

**3. A review is an issue too.** Before you call `/code-review`, create a review issue tied to
what you are reviewing.

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

### The supervisor

`moai skill install` also plants a second skill, `moai-supervise`. Call it to hand
the ideas that have piled up, one at a time, to the sessions idling on the same
repository, and to take their reports — the supervisor picks, sends and checks;
it does not fix and it does not merge.

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
