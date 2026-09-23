# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

`scripts/bump-version.sh <version>` moves whatever has accumulated under
`[Unreleased]` into a new release section and leaves `[Unreleased]` empty for the
next one. It does not commit and it does not tag — see `CONTRIBUTING.md`.

## [Unreleased]

## [0.1.2] - 2026-09-23

**Two things in this release break a caller.** Both are written up where they
belong below; they are gathered here so that reading the release does not depend
on finding them.

- **`prime --json`'s `epic` changed meaning.** It is now what the file says, like
  `epic` everywhere else, and the resolved answer moved to `derived_epic`. A loop
  reading `.picked[].epic` or `.ready[].epic` changes that one word to
  `.derived_epic` and gets what it used to get.
- **`moai add --from` refuses the flags a plan cannot honour.** `--status`,
  `--quiet` and a typed `--type` used to be accepted and thrown away by a call
  that ended in 0; they now exit 1. The five the argument parser already refused
  — a positional title, `--epic`, `--tag`, `--priority` and `--parent` — are
  still refused, but the exit code changes from 2 to 1 and the message becomes the
  command's own rather than the parser's. `--start` and `--due` are new in this
  release, so no call ever passed them to a plan. `--body` goes the other way: a
  plan takes it now.

### Added

- A milestone carries a **start and a deadline**: `moai milestone add 'v0.1'
  --start 2026-09-05 --due 2026-09-20`, and `moai edit <milestone> --due none`
  clears one. They are calendar days (`YYYY-MM-DD`), not timestamps, so the
  stored day is the day you typed, and they stand on a milestone row only —
  anywhere else the write is refused rather than quietly kept where nothing reads
  it. A day that does not exist (`2026-02-30`) and a start that falls after the
  deadline are refused too.
- The board says when a deadline has **passed**, and when one falls due within
  three days (`status_due_days`). Each line carries the date and the days, the
  most urgent first, and a milestone that is finished or deferred says nothing.
  **Nothing is blocked and the exit code never changes** — it is a warning, as
  every other one is.
- The board shines one line when **two or more milestones are running at once**.
  It is a notice, not a warning: `moai ready` counts them all as inside, so
  nothing is held back, and the line says that is the case rather than asking for
  a fix.
- `moai show <milestone>` says **how long it took** — the wall clock over its
  closed members, with the median, in `--json` as `spent`. Nothing is stored: it
  is read from each member's start and finish right then, so closing a member
  never writes another row. It always comes with the number it could measure
  ("2 of 3 closed"), because a member with no `started_at` is *unknown*, neither
  zero nor work not done. Wall clock is not effort — sessions running beside each
  other overlap, and time waiting on a person is in there. A child of a member is
  not counted twice: its span sits inside its parent's.
- **`derived_epic`** — the epic a row stands in, on every row `add`, `show`,
  `ready`, `prime`, `mv`, `edit`, `defer`, `link` and `idea promote` print under
  `--json`. A plan member carries its epic in its id and writes no `epic` field of
  its own, so asking `epic` alone read those rows as belonging to no epic. `epic`
  stays what the file says; the resolved answer comes beside it under its own key,
  the way `derived_status` already does. It never stands on a group row — an epic's
  own `epic` field is not a belonging — and its absence means the row is in no
  epic at all. Where the same id stands twice and the other line is a different
  `kind`, this line is counted into no group anywhere, so `derived_epic` is absent
  there even when `epic` is written: the one row where the two keys read against
  each other, and `duplicate_id` on the board names it.

### Changed

- Where moai asks "is this runnable" — the hook binary `moai skill` reports on,
  the editor it picks off `PATH`, the merge driver candidate — it now asks whether
  **you** can run it, not whether anyone can. The old check read the execute bits,
  so a file owned by someone else at `0o700`, or one on a `noexec` mount, passed
  here and then gave the shell a 126 that the hook line swallowed in silence: the
  screen said installed while none of the four rules stood. (The hook line says
  it out loud now too — see the entry under **Fixed**.) The answers that flip
  are exactly those two cases; a file you can run, and a file with no execute bit
  at all, read as before. On a `noexec` `TMPDIR` this now also means
  `moai skill status` will say the hook is not runnable rather than claiming it is
  installed.
- The refusal for rule 2 hands back the path **you typed**, not the one moai
  resolved. Judging still follows symlinks — the two spellings are one place, as
  they have to be — but a machine whose `TMPDIR`, `/tmp` or project directory is a
  link no longer asks you to retype a path that is not in your file list. This
  holds for a write caught inside a shell command too, where the word you wrote is
  what comes back.
- The release check says *why* it could not ask. The version line still reads as
  one of four, but the fourth now carries the reason in parentheses — no network,
  timed out, rate limited, a server error, TLS failed, unreadable answer, odd
  tag, the call failed — because what a person can do about it differs per reason:
  waiting fixes a rate limit, a proxy that swaps certificates never will. Nothing
  is blocked and the exit code never changes.
- `latest.toml` also records **why** the last call could not answer, so the reason
  stands for the whole day rather than only on the run that asked it. A run that
  heard a tag clears it: the file holds one answer, and a tag is that answer.
- `latest.toml` also records **where** the answer came from, and an answer is only
  reused for the place it came from. Pointing `MOAI_API_URL` at a mirror once no
  longer makes that mirror's tag stand as the real release for a day, and the
  reverse. A file written before this key is read as before, and asked again once.
- `check` under `[update]` is read in the same pass as the rest of your user
  config, so a value that is not `true` or `false` now says so in the explorer's
  notices instead of being ignored in silence. It still does not block anything,
  and anything that is not `false` leaves the check on.
- A stamp a little ahead of this machine's clock no longer forces a fresh ask.
  Two machines sharing one config directory with clocks seconds apart made one of
  them knock on GitHub every single run.
- Unfolding a parked thought no longer drops what the thought was standing on.
  `moai idea promote` puts the idea's milestone and its body onto the epic it
  unfolds, so `moai ready` hands the members out with the release they belong to
  and `moai show <epic>` can say why those issues are one bundle. The body goes
  onto the first epic of the plan only — it is not a value members inherit, and
  copying it onto every epic would leave one thought with several copies that
  drift. Unfolding into a standing epic (`-e <epic>`) carries neither: that epic
  is already the owner. What comes over is the release `moai show --milestone`
  stands that thought under, not whatever its own field says — a thought parked
  inside an epic comes over in that epic's release with an empty field of its own,
  and a field that loses to that epic never reaches the new one. If what came over
  is not the milestone that is running, hang the running one on the epic yourself —
  the line is the same one the supervisor's brief uses, so the two texts cannot
  drift apart. A release that is **deferred or already closed** is named on one
  stderr line as it comes over, in the rehearsal as well as the real run: nothing
  is blocked, but a plan that is out of the plan the moment it is created no
  longer says only that it succeeded.
- The worker brief holds a running review's worktree. While `/code-review --fix` is
  going, its branch and working tree are left alone — the fixes sit there
  uncommitted, and `reset --hard`, `rebase` or `commit --amend` throw them away —
  and when it returns, whatever subagent it left running is stopped before anything
  else touches the tree. Both are lines to read: nothing is blocked.
- The brief carries the five places a review keeps finding — a doc block taken over
  by an item inserted above it, a test that cannot go red on a revert, a value set
  up in only one of the several paths that build it, comments that no longer say
  what the code does, and a read path that newly opens something it never opened.
  They go on top of the angle rather than in place of it, and they have to reach the
  review itself — the angle on the issue is what the next person reads.
- An epic checks, in its worktree and before the merge, whether its line stands in
  the CHANGELOG section for the release being prepared. That window is the only one
  that knows what was taken out as well as what went in, and writing it there lets
  the line ride the merge instead of landing in the shared root checkout. Nothing
  checks it.
- The review grades gained the case they were missing: a worktree that carries
  members of two epics is measured as one epic and then raised one more step.
- `moai add --from <plan> --milestone <id>` hangs the milestone on the epics the
  plan creates instead of dropping it in silence. A milestone is inherited, so the
  epic alone carries it to every member, including the ones added later. The
  rehearsal (`--dry-run`) says which milestone it will be, on one line and as a
  `milestone` key under `--json`, and it refuses an id of the wrong shape there
  rather than after you have said yes.
- `--from` refuses the flags a plan cannot honour — `--status`, `--quiet` and a
  typed `--type` join a positional title, `--epic`, `--tag`, `--priority`,
  `--parent`, `--start` and `--due`. They used to be accepted and thrown away, so
  a call that ended in 0 silently swallowed the column or the id a script was
  capturing. **This is a break**: a call that passed one of them now exits
  non-zero. The refusal is the command's own, not the argument parser's: it names
  only what you actually passed (the positional stands as `[title]`), exits 1, and
  under `--json` it is the `{"code":"bad_input", …}` object every other refusal
  gives, so a loop that branches on `code` sees this one too. **The title, the
  epic, the tag, the priority and the parent were refused before too, but by the
  argument parser** — so for those five the exit code changes from 2 to 1 and the
  message stops being plain text. The two dates are new in this release, so a plan
  has refused them from the start.
- **A plan takes `--body`**, and puts it on the first epic it creates — the one
  place `moai show <epic>` reads why these issues are one bundle, and the same
  place `moai idea promote` has been putting the thought's body. What is refused
  is only the call where there is nothing to read it from: `--body -` together
  with a plan that also reads stdin (`-`, and the other spellings of it such as
  `/dev/stdin`), because there is one stdin. `moai add --from plan.md --body -`,
  `moai add --from - --body '<text>'` and `moai add --from plan.md --body
  '<text>'` all go through, and `--dry-run` measures that body against the same
  64KB limit the write does — in the same order the write measures it, so a plan
  where both a title and the body are too long gets one answer, not two.
  The rehearsal also **says where the body will land**: one line naming the first
  epic, and `body_on` under `--json` holding that draft's index, the same way
  `--milestone` has its own line and key. The text itself is not echoed — a 64KB
  body would bury the plan it is supposed to be shown beside.
- `--from` no longer swallows a typed `--type`. `moai add --from - --type issue`
  used to build the whole epic tree and exit 0, because the namespace default
  (`moai issue add --from -` routes through the same place) and a `--type` the
  caller typed were folded into one value, and letting the default through let
  the typed flag through with it. Undoing that meant deleting rows by hand. The
  markdown decides what gets created — `#` is an epic, `-` is an issue — so a
  typed `--type` is refused whatever its value, while the verb's own default
  still stands. `--type idea` and `--type milestone` keep pointing at where that
  work does go (`moai idea promote`, `moai milestone add`) under every verb, not
  only under a bare `moai add`.
- When the body `moai idea promote` carries over is past the 64KB a single write
  takes, the refusal names **the idea** and the way out of it — `moai edit <idea>
  -b -` — instead of naming the epic the write was about to create. Such a body
  can only get there through a merge resolved by hand, and the old message sent
  you off to shorten the first line of the plan you had just typed, which changed
  nothing. The rehearsal (`--dry-run`) measures it too, at the same point in the
  run the write measures it, so the two say the same words even when the plan's
  own title is over the limit as well or the milestone it carries has been
  deferred. It is still a refusal, not a truncation: text a person wrote is not
  shortened on their behalf. The way out is written in English like the rest of
  that refusal, which carries the planted review guidance and has always been one
  language.
- **A plan called through the wrong verb is told that first.** `moai idea add
  --from - --body …` and `moai milestone add --from - --body …` used to answer
  with the flag conflict, so the caller dropped `--body`, ran it again, and only
  then learned that markdown does not go in through `idea add` at all. The
  namespace refusal now comes before the flag refusal, and both of them are the
  command's own.
- `prime --json`'s `epic` key is now what the file says, like `epic` everywhere
  else; the resolved answer moved to `derived_epic`. **This is a break**: it was
  the only surface that put the inherited epic under `epic`, so one binary gave
  two answers to "which epic is this row in" depending on which command you asked.
  A loop reading `.picked[].epic` or `.ready[].epic` changes that one word to
  `.derived_epic` and gets what it used to get.

### Fixed

- A row that **wrote no `epic` of its own no longer wears the one its twin
  wrote**. Which epic a row is in is read from the row, but what it fell back on
  when the row wrote nothing was the id-keyed map — and that map holds the later
  line's answer, the written `epic` included. So where the same id stood twice,
  the earlier row, whose epic is the epic its id sits under, was drawn, counted,
  filtered and reported under the *other* row's epic: `derived_epic` on every
  machine surface (`show`, `ready`, `status`, `prime`, `add`, `mv`, `edit`),
  `moai show <that epic>`'s member list, `moai show -e <it>` and the tree all
  named it, while the epic its id actually sits under said `Members 0/0` and drew
  the row as a child. The fall-back now asks the id's parent, which both rows
  share, so each gets its own answer and the row's own `epic` still wins.
- `moai ready` no longer offers to undo a defer that **took the other row out of
  the plan**. Where an id stands twice, `moai show <id>` draws one line and says
  which defer took it out, but the undo command beside it was the union of every
  line's walk, so it also named a release that pinned the line you are not
  looking at — releasing that one changed nothing on screen.
- Where the same id stands twice, a row now **inherits the defer of the epic or
  the release it wrote on itself**. Which group a row is in is read from the row
  (`epic`, then the map), but the walk that carries a defer down still climbed by
  id alone, so a row counted as a member of a deferred epic was handed out by
  `moai ready` and never listed by `moai show --deferred` — and a row that wrote
  nothing was pulled out of the plan by its twin's deferred release. AGENTS.md
  says deferring a group takes the work under it out of the plan; on those rows it
  did not.
- A row with no epic now **stands in the release it wrote on itself**. The
  milestone axis fell back to the id-keyed map for such a row, so two rows sharing
  an id and carrying different releases both counted under the later one:
  `moai show <the first release>` said `Members 0/0` while `moai show <the other>`
  listed both, and the first row's `milestone` was readable on no surface at all.
  The filter (`--milestone`), the tree and the `(lost)` verdict answer from the row
  too, so all four read the same line.
- `moai ready` no longer prints `moai defer  --undo` with no id in it. Where an id
  stands twice, the row that left the plan and the row the undo target was read
  back from could be different lines.
- **A defer no longer travels from one row to its twin.** The map of what is out
  of the plan was keyed by id and written with `filter_map`, so it had no way to
  say "this row is not deferred" — the defer a row inherited from a deferred epic
  or release stayed on that id and reached the other row, which had written a live
  epic of its own. `moai ready` then handed out neither row and listed neither
  under `held`, `moai show --deferred` listed both, and `moai show <the live
  release>` still counted the second row as a live member: one repository saying
  four different things, on an id whose writes `duplicate_id` had already stopped.
  The surfaces that offer, hide, count, draw and gate a row now read that row's own
  answer: `ready`, the board and the lists, the `deferred` notice, the hook's "what
  you hold", the `deferred` word on `moai show --tree`, the explorer's hide toggle,
  its spinner and its detail badge, the member count a group reads its column from,
  and the "you can close this now" line a closing write prints. Where only an id is
  in hand — the blocker a row names in `blocked_by`, `shelved_by` on `moai show
  <id>`, and the `moai defer <id> --undo` these lines print — the later row answers,
  as it already does for the kind, the column and the title shown under that id.
  A row whose id a group also stands on keeps its own answer too; only the group
  row itself falls back to the id, because whether a group is out of the plan
  depends on the column it reads from members counted by id.

- A row one side broke by hand now stands **inside** the conflict markers on that
  side. Pairing a row needed its JSON to parse, so a row edited until it stopped
  being JSON left that side of the marker empty — which reads as "that side
  deleted it", and it had not — while the broken bytes were written below the
  markers with nothing tying them to the conflict just resolved. A row is now
  paired by the `id` at the head of the line even when the JSON no longer parses,
  and it travels through the merge byte for byte. Only the shape this tool writes
  is read: `{"id":"…"` first, and the id has to be a well-formed one. A line whose
  head is gone too, and a line that is two issues glued together by a missing
  newline, stay outside the markers as before — pairing the glued line would bury
  the second issue inside the first one's marker, where taking the other side
  deletes an issue that exists nowhere else. When both sides broke the **same**
  row and only one of the two heads survived, pairing that one side alone would
  leave the other panel empty again — the very lie this entry removes — so there
  the pairing is given back: the id resolves as "neither side has a readable row
  left", and both sides' broken bytes ride out below, where `moai status` counts
  them. Giving it back is held to three things — this side really did break that
  row (a row nobody here touched keeps its key, or the other side's deletion of it
  came back at exit 0 with no marker), no readable row is left under that id (a
  live row and its broken twin go to a person whole, so resolving the marker cannot
  leave the twin behind), and all three sides are split the same way (they were not,
  and the count that decides which lines are new read the base as holding none).
  **What it cannot yet tell apart is one id from the next.** The evidence left at a
  single id — base had the row, we hold it broken, they hold nothing — is the same
  whether they broke it too or deleted it, so what stands in for the missing bit is
  a question about the whole file: did that side bring in any unreadable line of its
  own. One such line anywhere therefore opens the door for every row we broke, and a
  side that really deleted one of them gets no marker. Nothing is lost — those bytes
  ride out below, and `moai status` counts them — but a delete standing against an
  edit is settled without a word. Telling them apart means reading the other side's
  broken bytes to see whose row they were, which is the guessing this entry declines
  to do; which way that goes is a decision, and it is written on `unpair`. A merge
  that gave a key back is also not quite its own fixed point: the line it let go
  rides out below the base's other unreadable lines this time and above them the
  next time, once its head is scraped again — so a merge that changes nothing
  rewrites the file once, moving one line. Every line still stands, and reading
  that file and writing it back is byte for byte the same.
- Two rows carrying the same id now hand that id to a person instead of being
  quietly mixed. A snapshot that holds a readable row and an unreadable one under
  one id — what a hand-resolved conflict leaves behind — used to have one of them
  pushed aside, and which one depended on what else that file held, so the three
  sides of a merge disagreed about it. Two ways out of that: a branch that deleted
  the stale copy got the deletion reverted with no marker, and a branch carrying
  the extra copy had it merged straight in, leaving a snapshot with a duplicated
  id. Both at exit 0. Such a snapshot is already fatal to `moai status`
  (`Ids standing twice`, `Unreadable rows`); only the merge was quiet about it.
  Where all three sides carry **the same rows**, though, the merge chose nothing,
  and handing the id over would put the same two lines in both panels on every
  merge from then on, with no `moai` command to clear it — so that one shape goes
  through untouched. The order they stand in is not part of "the same": a snapshot
  writes unreadable lines at the end, so one `moai` write moves a broken twin that
  sat above its live row, and comparing position for position handed that file over
  on every merge although the two sides were byte for byte alike. Everything either
  side touched still goes to a person.
  `moai status` now names the id as well: the id at the head of a broken line is
  read by the same one reader the snapshot uses, so a broken twin of a live row
  raises `Ids standing twice` instead of `Unreadable rows` alone, and a new id is
  never minted onto it.
- A single row this binary cannot read no longer turns **every** merge into a
  whole-file conflict. The merge driver paired rows by id only when both sides
  carried the very same unreadable lines, in the very same order, so one row
  written by a newer binary — a `kind` this one does not know, say — put conflict
  markers around the entire file from then on, and the per-issue resolution the
  driver exists for was gone. There was no way out of it either: `moai status`
  counts an unreadable row as fatal, so repairing that row on one branch is
  precisely what makes the two sides differ. A row is now paired by id whenever
  its JSON and its `id` can be read, whether or not the rest of it can, and it
  travels through the merge byte for byte — one side's change comes through, and
  only a row **both** sides changed is handed to a person, with the markers around
  that row alone. Lines with no id to pair on are merged by counting them: what
  one side added is added, what one side removed is removed, a removal both sides
  made is made once, and a line standing several times keeps its count. **Counting
  cannot tell an edit from a delete plus an add**, so a line with no id that both
  branches rewrote comes through as both lines, on a merge that exits 0 — better
  than picking one and losing the other, and `moai status` counts the pair. What
  still hands the whole file over is a duplicated id among readable rows, and a
  snapshot that is not text at all.
- A deferral is no longer dropped without a word when a timestamp cannot be read.
  When both branches had changed `planned_at`, the merge driver picked the later
  of the two, and a timestamp it could not parse — a `+09:00` offset left by a
  hand-resolved conflict, say — counted as "no time at all", so the other side
  won and took its `deferred_at` with it. The branch that had actually deferred
  the row last lost that decision with no marker, no warning and exit code 0.
  Now the later side is picked only when both timestamps are canonical;
  otherwise the row goes to a person. **A `planned_at` one side does not carry
  at all now goes to a person too**, where before the side that had one won:
  every other timestamp reads a missing value as "unknown" and takes the side
  that has one, but a missing `planned_at` is not unknown — it is the decision
  not to defer, and handing it to the other side takes that decision away along
  with its `deferred_at`.
- The release check no longer follows a redirect down to plaintext `http`. A call
  that starts on `https` is refused rather than downgraded, on every hop. A call
  you pointed at a plaintext mirror yourself still works — that one is your choice.
- One stale row no longer makes `.moai/issues.jsonl` conflict on **every** merge.
  The merge driver judged rows neither side had touched, so a single row carrying
  a value today's rules reject — left by a hand-resolved conflict, or written by a
  binary of another version — put conflict markers around itself on every merge
  from then on, with the two sides inside them byte-identical and nothing for a
  person to choose. A row whose value already stands on one branch now comes
  through unjudged; what the merge itself composes field by field is still judged,
  so a line the tool would refuse is still handed to a person. This is the same
  rule the rest of the tool keeps: strictness is about the row being written now,
  not about the whole file. What is skipped is the check alone — every row the
  merge writes is still put in the shape the tool itself writes, so a merge never
  leaves behind an unfolded tag, an empty timestamp or a duplicated JSON key for
  the next command to trip over.
- A hook binary that is **there but cannot be run** now says one line instead of
  passing in silence. The planted hook line ended in `|| exit 0`, which swallowed
  the 126 the shell gives for a file without its execute bit or on a `noexec`
  mount — the four rules did not stand and the session looked exactly as it does
  when they pass. The line now separates the two: nothing there stays silent, as
  it must (a `cargo clean` should not make every session noisy), and a binary that
  cannot be run prints a notice naming the hook and the exit code, and the command
  that says which file it was (`moai skill status`). It reads the same on either
  shell a machine may put at `/bin/sh`: `command -v` answers "is it there" for a
  path differently in dash and in bash — bash checks the execute bit as well — so
  the line asks once more with `[ -e ]` before it gives up, or the one case named
  first here would still be silent wherever `/bin/sh` is bash. **Nothing is
  blocked and the exit code is still 0**, even under `set -e` — a hook that blocks
  on its own missing permission stops every tool call in the session. A hook that
  ran and then failed keeps its silence: it has already written its own answer to
  stdout, and a second line appended there would throw that answer — a refusal
  included — away. The notice now covers **every exit that is not 0 or 1 and left
  stdout empty**, not just the two the shell gives: the 2 an older binary answers
  an event name it does not know with, the 101 of a panic, the 128+N of a signal.
  Two of those can arrive *after* the answer is already on its way, which is what
  the second half of that rule is for — the line holds what the binary wrote,
  speaks only when that was empty, and hands the answer back otherwise. It follows
  that a binary which prints something and then dies is never named: there is no
  way to say so without throwing away what it printed. Measured against
  the real client: two JSON objects on one hook's stdout are **both** discarded, so
  a refusal printed just before a panic used to let through the very write it
  refused. moai helps from its own side by putting its output last, after the
  lines it writes to stderr, which leaves a panic no room to land between the two —
  and because those lines go to stderr, a terminal now shows them above the
  command's own output rather than under it. Handing the answer back is not quite
  byte for byte: holding it in the shell collapses a run of trailing newlines into
  one and drops a NUL, neither of which moai's own JSON can carry. The shell that
  holds it is also the one that writes it, so it now takes the `SIGPIPE` moai used
  to swallow — a reader that closes early would end the hook on 141, which is why
  the line arms `trap 'exit 0' PIPE` first. The notice stands **once per session**
  for each binary, event and exit code, so a hook that cannot run says so once
  rather than on all of a session's tool calls (140 on average in this repository,
  1,257 at the most).
- `moai project ls` no longer reads the timezone database. It draws no time at all
  — it shows each project's column counts — but it asked for the reader's timezone
  anyway, so on a machine without zoneinfo (a static musl build on Alpine or
  scratch) it added a line saying so to a command that had never said one. The
  count is the same either way: the only sum a timezone reaches is the milestone
  deadline judgement.

## [0.1.1] - 2026-09-22

### Added

- `moai tui` says whether a newer release is out. It asks GitHub once a day, and
  the version line in the header reads as one of four: a new release, the latest,
  ahead of the latest (a build from source), or not asked. It only asks where a
  person is watching — `--json`, a pipe and anything that is not a terminal never
  ask, so a machine running agents does not knock on the outside every run. The
  answer and when it was asked are held in `latest.toml` beside your user config,
  and `check = false` under `[update]` there, or `MOAI_NO_UPDATE_CHECK=1`, turns
  it off. Nothing is blocked and the exit code never changes. The HTTPS client
  it needs is most of why the release binary grew this release — it now measures
  7.6 MB of the 15 MB budget, where both numbers count as `scripts/check-size.sh`
  does (8,001,296 bytes of 15,728,640).
- Times on screen are drawn in the reader's timezone. They stood in UTC alone,
  so a reader in Seoul saw every stamp nine hours out. What is stored and what
  `--json` carries are still UTC — only the letters a person reads move. The CLI
  follows the system (`TZ`, then `/etc/localtime`); the explorer takes a name
  picked with `SPC o t` and writes it as `timezone` under `[tui]`. The release is
  a static musl build, so a machine with no `/usr/share/zoneinfo` (Alpine,
  scratch) falls back to UTC and says so in one line, blocking nothing.
- The journal is one file per person, named from the email
  (`.moai/journal/raven_buzzni_com.jsonl`). Several files is the normal shape —
  the reader merges all of them, so an email that changes only adds one. The old
  single `.moai/journal.jsonl` is read as one of those files and nothing is
  migrated. Where the email is not known, nothing is written at all: a file whose
  purpose is history is worse with unowned lines in it than with one question
  asked.
- `moai init` plants the merge driver itself. Until now it wrote the
  `merge=moai` name into `.gitattributes` and left the command that name points
  at to be installed by hand — half of something that does not work in halves.
  `--no-driver` leaves `.git/config` alone, and `--check` answers in one word and
  writes nothing. A repository that decides against per-issue merging records
  that in git's own vocabulary (`-merge` on the snapshot path), and both `init`
  and the `moai status` notice read it as a decision instead of putting the line
  back.
- The explorer's detail pane has a side: `SPC o d` cycles it through right,
  bottom, left and top, `Ctrl-w j` and `Ctrl-w k` move between the panes while it
  is split top and bottom, and the epic, milestone and blocked lines inside it
  carry the same overlap marks the list rows do.
- `show --json`, for one issue and for a list, carries `journal_error` beside a
  row whose history came up short — `kind` (`permission`, `failed`) to branch on
  and `said` naming the file to `chmod`, the same shape as `commits_error`. Until
  now the exit code was the only signal, and it is shared with every other partial
  answer, so a machine could tell "this run was not whole" but never which row.
  A row carries only its own checkout's failures.
- The epic and milestone bars name how many of their members are deferred
  (`1/2  deferred 1`), on the board, in `moai show <group>` and in
  `status --json`. The denominator still counts them — the bar is "of what was
  promised, how much is done" — so without the count beside it the reader sees a
  number that never drops and reads the tally as broken.
- `--json` always carries `kind` and `priority`, including on the rows whose
  snapshot line leaves them out because they hold the default. What the file
  omits and what the contract omits are two different things — keys that can
  genuinely be absent (`epic`, `milestone`, `deferred_at`) stay absent.
- Refusals carry data, so they translate. `MOAI_LANG=ko` reached the screens in
  0.1.0 but not the messages that say no; the write path and its validation, the
  config reader, the plan parser, git, resolving a person, the issue commands,
  entering the explorer, and the `skill` and `merge-driver` notices now all speak
  through `i18n`.
- The supervisor skill treats a running milestone as the gate before it hands
  work out. Work outside one is not assigned, and to assign it you attach the
  milestone to its epic — which is the same ordering `moai ready` already used.
- `rust-toolchain.toml` pins the toolchain that builds, formats and lints this
  repository, so `cargo fmt` on a contributor's machine gives what CI sees.
  `rust-version` in `Cargo.toml` is proven by the `msrv` job below rather than by
  the toolchain CI happens to run.
- CI restores the pinned toolchain from a cache keyed on `rust-toolchain.toml`,
  before the first rustup call in each job. Pinning made every job fetch and
  unpack the toolchain again, and that cost is single-threaded xz rather than
  bandwidth, so a faster runner does not pay it back.
- An `msrv` job builds the crate with the `rust-version` read out of
  `Cargo.toml`, so bumping that one line moves what CI actually walks.
- CI refuses a pull request into `main` that does not come from `develop`, so a
  release is always cut from the branch it was integrated on.

### Changed

- The explorer's detail pane opens and closes with `SPC v d`, not `SPC v p`, so
  that the key toggling it and the key placing it (`SPC o d`) read as one pair.
  **`SPC v d` used to show and hide the `done` column**, so that keystroke now
  does something else.
- `done` has no letter of its own under `SPC v` any more — `SPC v d` and the
  column's number toggled the same setting, and the menu said it twice. The
  number alone counts, and it reads the column's name out of the config instead
  of carrying `done` in the code.
- `moai` exits non-zero and names the file when a journal cannot be read. A skip
  drew a history that had quietly lost one person's lines and still looked whole.
- `moai init` no longer creates an empty `.moai/journal.jsonl`. History lives in
  `.moai/journal/` now, and the old path is only ever read.

### Fixed

- The release workflow could not have built its musl target. `ureq` pulls in
  `rustls`, which pulls in `ring`, which builds C and assembly, and that job had
  no C toolchain — `cc` resolves `x86_64-unknown-linux-musl` to
  `x86_64-linux-musl-gcc` or `musl-gcc` and `ubuntu-latest` carries neither. The
  job would have died *after* the tag was pushed, and `release` needs `build`, so
  nothing at all would have shipped. No pull request could have caught it:
  `ci.yml` and `smoke.yml` build the host target only.
- The pre-push tag check read the working tree's `Cargo.toml` instead of the
  `Cargo.toml` of the commit being pushed, so on a branch already moved on to the
  next version it refused to re-push an older tag whose version was correct.
  Shallow clones and machines without git fall back to the working tree, and the
  line it prints says which of the two it read.
- `release.yml` restored its cache after `rustup target add` — the first rustup
  call in that job — so the toolchain was already downloaded by the time the
  cache arrived.
- The install one-liner in `README.md` and `install.sh` points at `main`, the
  branch a release is cut from. It pointed at `develop`, so a receiver installed
  a release with a script that release does not carry.
- A journal file that cannot be read no longer blocks the rest of the history.
  The run steps over it, counts what it skipped, and names the file.
- Read marks survive their own edges: a line that cannot be read no longer stops
  the whole write, a stamp for a place that is gone is kept as pending instead of
  dropped, stamps already seated are taken back out of the pending list, `path`
  no longer swallows the comment above it, and a run that stops on a pending
  place says which file it stopped on.
- The explorer no longer lets a sweep that arrives late overwrite a row just
  opened, and the read marks of an expanded project follow a table that changed
  under them.
- A checkout with no tracker in it is told to run `moai init` first, and `init`
  no longer tells you to ignore the tracker file it is reading.
- The hook that `moai skill install` plants reads more shells correctly: heredoc
  bodies, `su -c` and `runuser -c` text, `runuser -u <user> -- <command>`,
  commands behind `xargs`, the `errexit` spellings zsh and ksh use, nested `!`
  where the outer one wins, a `-C` that points where you already are, and a
  pick-up that wins inside text that ends in a background `&`. Its re-reading of
  quoted text is bounded by a budget rather than by depth alone, and the same
  fragment is no longer scanned twice in one run.

## [0.1.0] - 2026-09-21

### Added

- Issues, epics, milestones, ideas and deferral, stored as one JSONL line per
  issue with a separate journal that is never read to compute state.
- `moai status`, `moai ready`, `moai show`, and a terminal explorer (`moai tui`).
- `moai prime`, a short markdown page of what you hold and what comes next —
  small enough for an agent to re-read at the start of a session and again
  after a compaction.
- `--json` on every command, so an agent loop can run with no human in it.
- `moai mv <id> in_progress --from todo` moves only while the column you saw
  still holds, so sessions sharing one repository cannot overwrite each other's
  pick-up. The loser gets `stale` and a non-zero exit code.
- `moai mv <id> done` says in one line what that close unblocked.
- `moai link` to record that one issue blocks another, and `moai read` to mark
  an issue read, kept in your own config only.
- `moai add --from -` creates an epic and its issues in one go from a plan
  written in markdown; `moai idea promote` unfolds a parked thought the same way.
- While a milestone is running, `moai ready` hands out what is inside it first
  and leaves only `p0` outside it.
- Commits are found on the spot by the issue id written in their subject —
  `moai show <id>` and the explorer draw them, and nothing is stored on the issue.
- `moai show <id> --json` reads the `model:` notes back as `work`, and carries
  `started_at` and `done_at` for how long a piece of work took.
- English is the language on screen by default. `MOAI_LANG=ko moai status`, or
  `lang` under `[i18n]` in your user config, switches it; en, ko, zh, ja and es
  exist, en and ko are complete, and the system locale is never read.
- `moai init`, which writes `.moai/` and a managed block in `AGENTS.md`.
- `moai merge-driver --install`, resolving `.moai/issues.jsonl` per issue
  instead of per neighbouring line. `moai status` measures whether the driver
  actually runs and shines one line on a clone where it was never installed.
- `moai skill install`, which installs the Claude skills and hooks.
- A `prepare-commit-msg` hook that writes `MOAI_ACTOR` into an `Executed-By:`
  trailer, so `git log` alone shows which commits an agent made.
- Multiple projects registered in one place, and worktree-aware `ready`.
- CI: clippy, `cargo fmt --check` against a committed `rustfmt.toml`, and tests
  on every pull request, plus a 15 MB release binary budget.
- A release workflow that publishes verified archives on a `v*` tag, and
  `install.sh`, which refuses to install anything it cannot check against
  `SHA256SUMS`.
