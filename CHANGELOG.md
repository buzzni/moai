# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

`scripts/bump-version.sh auto` moves whatever has accumulated under
`[Unreleased]` into a new release section and leaves `[Unreleased]` empty for the
next one. The headings pick the version: before 1.0, `Fixed` and `Security` alone
make a patch release and anything else a minor one. It does not commit and it
does not tag — see `CONTRIBUTING.md`.

## [Unreleased]

### Changed

- **The planted hooks no longer listen for `StopFailure` and `SessionEnd`
  (Claude Code) or `Interrupt` and `SessionEnd` (Codex)** — only presence used
  them. `moai skill install` rewrites the planted files without them. For one
  release `moai hook stop-failure`, `interrupt` and `session-end` stay as
  commands that do nothing, so hooks planted before 0.9 do not print an error.
  (moai-5uwh)
- **`moai-supervise` is planted for Claude Code only, and talks through Claude
  Code's own `ListAgents` and `SendMessage`.** It finds the idle sessions of the
  repository there, sends each one its assignment with a line telling it to
  read the worker's steps (`references/worker.md`, by absolute path), and takes
  the report back the same way. It
  no longer says hello, sends letters or waits on `moai inbox`, and it no longer
  clears a worker's tmux pane — clearing a window is the person's, and the
  worker's report says when it is safe. `moai skill install --agent codex` or
  `antigravity` no longer plants it in `.agents/skills/`. (moai-obxm)
- **`moai init` no longer writes the `.moai/mail/` and `.moai/agents/` lines
  into `.gitignore`**, and no longer looks for them. Lines an earlier `init`
  wrote stay where they are, and so do the directories — each holds its own
  `.gitignore`, so git does not see them either way. (moai-six5.p9p)
- **`moai skill install` removes the skill directories an earlier moai planted
  and this one no longer does** — `skills/moai-work/` in the Claude plugin and
  in `.agents/skills/`, and `skills/moai-supervise/` in `.agents/skills/`
  (Codex and Antigravity). Only a directory that holds nothing but the files
  moai planted there goes — their text is not compared, so a `SKILL.md` you
  edited goes with it, as an install overwrites one; a directory with a file
  of yours in it is left and named in one line.
  `moai skill uninstall --agent codex` still names such a leftover for you to
  delete, and the wiki still does not read `moai-work` as an issue id.
  (moai-six5.1xz)

### Added

- **`moai status` says when the planted skills or hooks differ from this
  binary's, and names the line that plants them again** — `moai skill install`,
  with `--agent codex` or `antigravity` where `.codex/` or `.agents/` holds
  them. It is a notice like the AGENTS.md one: it blocks nothing and never
  changes the exit code. `moai init` says the same line when it does not run the
  install itself. **After upgrading from 0.8, run `moai skill install` again**
  (with `--agent codex` / `--agent antigravity` if you planted for them) — it
  removes `moai-work`, the `.agents/` copy of `moai-supervise` and the hook
  events 0.9 no longer listens for. (moai-ybns.451.rpd)

### Removed

- **The mailbox and presence are gone: `moai send`, `moai inbox`, `moai hello`
  and `moai agents`.** moai carries no messaging between agents any more —
  sessions of one vendor talk with that vendor's own means (in Claude Code,
  `ListAgents` and `SendMessage`). The hooks no longer load letters into a
  prompt or the end of a turn, no longer hold a turn for an unread letter, and
  no longer record whether a session is busy or idle. Directories 0.7 and 0.8
  left behind (`.moai/mail/`, `.moai/agents/`) are ignored and left as they
  are. A `mail_read_days` key in `.moai/config.toml` is now ignored without a
  word. (moai-5uwh)
- **The worker skill `moai-work` is gone.** A window no longer becomes a worker
  by calling a skill and waiting for letters: every idle Claude Code session of
  the repository is a worker, and the supervisor's message carries the
  assignment and names the file of the worker's steps for it to read. (moai-obxm)

## [0.8.0] - 2026-10-07

### Added

- **Explicit yearly issue archives.** `moai archive --dry-run` previews eligible
  closed rows and `moai archive` moves closed issue and epic bundles into
  `.moai/archive/<year>.jsonl`; `show` and `stats` can read archived rows, while
  board counts stay on the active snapshot and archived rows supply reference
  and milestone context. `moai archive --drop <id>` removes the stale archive
  copies of a live row; it refuses an ID without a live row and an archived row
  that is another issue under the same ID. (moai-fx9t, moai-bth3)
- **`moai init --yes` (`-y`) plants without asking.** Choices not settled by
  flags or existing git rules and guide files take the old defaults.
- **`moai init --driver` pins the merge-driver row**, the pair of `--no-driver`,
  so a terminal run that gives every row a flag asks nothing.
- **`moai init --tracking exclude|gitignore` keeps the tracker out of git.**
  The ignore rules, `/.moai/` and the hook plugin's directory go into
  `.git/info/exclude` (no committed file changes) or `.gitignore`, and
  `.gitattributes` and the merge driver are left alone — there is nothing for
  git to merge. A tracker planted in a subdirectory gets lines anchored at that
  subdirectory, and when the lines cannot be written nothing is planted. A
  linked worktree refuses both — the lines would hide the main checkout's
  tracker too.
  `--tracking commit` is what `init` has always done. Run again,
  `init` asks git which one stands rather than storing it, and refuses to switch.
  `moai status` and `init --check` follow the same answer: a tracker kept out
  of git is never told it lacks `.gitattributes` rules or an AGENTS.md block,
  and a line missing from `.git/info/exclude` is named as that file's.
- **`moai init --guide file` writes the agent guide to `.moai/guide.md`** and
  leaves only a few lines in the AGENTS.md block that point at it — with
  `moai init --print` as the way to the same text where the file is missing.
  `--guide block` is the full block as before, and `--guide none` is
  `--no-agents`. `init --check` and `moai status` measure both the link and the
  file; run again, `init` reads which one stands from the block itself.
- **`moai init --guide hook` leaves AGENTS.md alone and lets the hooks say it.**
  It is what the screen picks for a tracker kept out of git, so a file nobody
  else has is never named in a committed one.
- **`moai init --skill` and `--register` run `moai skill install --scope local`
  and `moai project add` once the tracker is planted**, and print what they
  said under their names. The screen picks both; `--no-skill` and
  `--no-register` turn them off, and a hooks guide keeps the install on —
  `--guide hook --no-skill` is refused. A run that fails leaves what was
  planted and still exits 0.

- **Tab completes keys and values in the explorer's filter field (`SPC f`).**
  A unique key gains `=` (`mil` becomes `milestone=`), and Tab or Shift-Tab
  cycles through matching keys in query-table order or the available values
  for tags, assignees and milestones. Status columns, priorities `p0` to `p3`
  and kind names also complete; another key keeps the inserted candidate.
  Enter and Esc keep their existing meanings. (moai-fc97)

### Changed

- **Parked work is called backlog.** The primary command is `moai backlog`
  (`add`, `ls`, `show`, `promote`), the kind is `backlog` in JSON views, and the
  board, explorer, guides and translations use the same name. `--type idea`
  and stored `kind: "idea"` remain readable; the shared file still writes
  `kind: "idea"` for older binaries. The JSON warning key is `backlog_pile`.
  `SPC v b` hides backlog items, saved as `[tui] hide_backlog`; legacy
  `hide_ideas` and `status_idea_pile` settings remain readable, with the new
  keys taking precedence. The threshold's new name is `status_backlog_pile`.
  (moai-jtvp)
- **The first `moai init` in a terminal asks before it plants.** A short screen
  shows the id prefix, whether git tracks the tracker, where the agent guide
  goes, whether to install the hooks and skills, the merge driver and whether to
  add the repository to your project list, each with its default picked; Enter
  plants and Esc stops with nothing written. **The screen's defaults are not the
  old `init`:** in a git repository Enter keeps the tracker out of git
  (`.git/info/exclude`), leaves AGENTS.md alone and lets the hooks say it,
  installs the hooks and skills for this clone (`claude plugin install`) and adds
  the repository to your project list. A flag picks its row and locks it, and
  `--yes` asks nothing and plants the old way. It asks only where a person is
  watching — when stdin or stdout is not a terminal, or under `--json`, `init`
  uses the old defaults for choices not settled by flags or existing git rules
  and guide files; the `--json` line gains `tracking`, `guide`, `guide_file`, `skill`
  and `project`. Running `init` again where `.moai` already stands never asks.
- **The board the hook loads on a session's first prompt says where moai's
  usage lives when AGENTS.md carries no moai block** — one line naming the
  `moai` skill and `moai prime`. Checkouts with the block see no change.
- **An archive file that cannot be read is broken data.** `archive_unreadable`
  is now a warning rather than a notice, so `moai status` exits non-zero on it
  the way it does on an unreadable line in the active snapshot, and points at
  `moai show --archived`, which names each file and line. The hook's `Stop`
  check counts it too, reached by the live rows or not. `moai stats` and
  `moai show --archived` still exit 0. (moai-5y2a)

### Fixed

- **The first init screen installs the hooks and skills when Enter keeps the
  default hooks guide.** Only a guide inferred from already installed hooks
  skips installation; an explicit `--guide hook` or `--skill` still installs.
  (moai-95do)
- **Tracker writes compare changed rows through an ID map.** Adding or updating
  an issue no longer scans the original tracker once for every row. The file
  format and handling of unchanged rows remain the same. (moai-fx9t.opm)
- **Initialization preserves the tracking and guide choices already present in a
  clone**, including global git excludes and installed moai hooks. Git failures
  or probe timeouts refuse initialization before it writes; status reports the
  unknown tracking state and retains dotfile symlink notices. A linked worktree
  reads the guide from the main tracker, and a clone without the local tracker
  uses the guide link's `init --print` fallback without a false hand-edit notice.
- **The init chooser skips `TERM=dumb`, checks refusals before opening, and puts
  the report at the chooser's origin.** All paths validate prefix, git, conflicting
  flags, and AGENTS.md readability in the same order.
- **Local tracking ignores both a `.moai` symlink and its target directory.**
  An explicit `--driver` with excluded tracking is refused; JSON `gitignore`
  reports writes to `.gitignore`, and `exclude` reports writes to `.git/info/exclude`.
- **Archive regression repairs.** Shipped milestones and archived blockers keep
  their meaning after storage moves in `status` inside and outside a repository,
  `ready` and the hook board, and a restored member retains its parent context
  without restoring its former bundle. Rows already moved to the archive stay
  out of `moai show` lists and the status board whatever `archive_days` says
  now. Unreadable archive files leave writes and board reads available,
  diagnostics name the archive source, and nothing about them is reported as
  lost history. Conflicting bundles stay live while other bundles move, and
  `--dry-run` and the board count only what will move. Restore cleanup failure
  preserves the successful move and any journal warning; cleanup skips archive
  files it cannot read. Hook ownership checks include archived pickups, sibling
  overlays do not create archive collision or eligibility warnings, and archive
  temporary files stay under the existing ignore rule. (moai-bth3)
- **Writes can point at archived rows.** `add -e`, `edit -e`, `add --parent`,
  `link` and `backlog promote -e` accept an epic, parent or blocker that
  `moai archive` moved, instead of calling it missing; the new or edited row
  stays live and the archived row stays archived. (moai-tzzt)
- **A deferred epic in the archive can be taken back.** `moai defer <id> --undo`
  on an archived row brings that row live, out of the deferral, instead of
  calling it missing, and a member restored under an archived deferred epic is
  told — by `mv` and by `defer <member> --undo` — which deferral keeps it out
  of the plan. (moai-b6w3)
- **The overview names the archived epic of a restored member.** `status --json`
  and `ready --json` called outside any `.moai` now carry `derived_epic` on a
  member restored under an epic that `moai archive` moved, as the in-repo
  `ready --json` does. (moai-kfjy)
- **The end of an agent turn no longer parses the whole archive.** The hook's
  `Stop` check and its session baseline read only the archived rows that reach
  the live ones — parents, epics, milestones, blockers and their members — and
  skip the `archive_pending` count, a notice they never counted. The warnings
  they count are the ones `moai status` counts. (moai-i9ji)
- **The explorer counts warnings and notices the way `moai status` does when an
  archive exists.** The project layer's `+N` (and `tui --json` outside a
  repository) and the banner inside a project read archived rows as context
  only: a milestone whose members were all archived no longer shows as an
  overdue empty todo, an archived blocker is no longer a dangling reference,
  archived rows are not counted as work, and an unreadable archive file is
  `archive_unreadable` rather than an unreadable-line warning. Archive
  collisions and bundles waiting for `moai archive` now show in both places,
  and the layer re-reads a project when its archive changes. The banner's
  urgent "N rows could not be read" counts lines in the active snapshot only; a
  bad line in an archive file shows as that warning instead. (moai-nkwg, moai-e18s)
- **An archived deferred epic over a restored member counts as deferred again.**
  When a member of a deferred epic that `moai archive` moved is restored, the
  epic is back on the board as that member's group, and the board now marks it
  `deferred` and counts it in the Deferred notice, as it does for a live epic
  and as the `moai show --deferred` the notice points at lists it. An archived
  deferred epic with nothing live under it stays out of the count.
  (moai-sai2)

### Deprecated

- **`moai idea` is a hidden alias for `moai backlog` for one release.** It is
  removed in v0.9.0. (moai-jtvp)

## [0.7.0] - 2026-10-06

### Added

- **Agents leave each other letters: `moai send`, `moai inbox`, `moai agents`
  and `moai hello`.** A letter is one file in its recipient's box,
  `.moai/mail/<name>/` or `.moai/mail/any-idle-worker/` —
  `{"v":1,"to","from","subject","body","sent_at","reply_to"}`, keys this build
  does not know passed through — and its id is the file name. An agent opens
  its own box and the `any-idle-worker` one, nothing else, so a hook never reads
  other agents' letters and a broken letter only stands in its own box. `moai send <to>
  <subject> -b <text|->` leaves one for an agent name or for `any-idle-worker`,
  which the first agent that is neither the sender nor a supervisor keeps.
  `moai inbox` shows the letters for you, `--ack` moves them to `read/` inside
  the box (`--all` shows them again until `moai agents` sweeps them,
  `mail_read_days` after they were read — 7 unless `.moai/config.toml` says
  otherwise, `0` keeps them; an unread letter is never swept) — one
  `any-idle-worker` letter per call, as the hooks take them, so several spread
  over the agents that wait — and `--wait <seconds>` waits for one to come.
  `moai inbox <id>` shows one letter, read or not, a page of about 24 KB at a
  time: the last line names `--from <n>` — characters of the body, counted
  from 0 — for the next part, so each page stays under an agent's own output
  cap (30,000 characters in Claude Code, 10,000 tokens in Codex), past which
  the agent cuts or sets aside what a command printed.
  `--json` gives the letter whole. Who you are is `--as`, else
  `MOAI_AGENT`, else the registered agent the command runs under. Codex runs
  every session's shell under one shared app-server, so there it is the row of
  the session id Codex sets in the shell (`CODEX_THREAD_ID`) — the row its hooks
  wrote — and never `MOAI_AGENT`, which that shell inherits from the shared
  app-server; a Codex that does not set it passes `--as` with the name the
  hooks give the session in its first context.
  **Nothing goes into `issues.jsonl` or the journal** — a letter is delivery,
  not record, and the mailbox follows the tracker into the main checkout, so
  every session of a repository sees one mailbox. Like every other file the
  repository holds, the mailbox and the presence rows follow a link only
  inside the checkout: a committed `.moai/mail`, `.moai/agents`, box or
  `read/` that points elsewhere is not opened — writing through it stops with
  `broken`, and reading names it. There is no lock: sending
  links a finished temporary file into place and never overwrites a letter,
  and marking read is one rename, so two readers never both take a letter.
- **A letter left for an agent that went away goes back to its sender.** When
  `moai agents` sweeps a row whose process is gone, when a new session takes
  over the name of a row that has gone, and when a Codex session ends, the
  unread letters in that box move to their senders' boxes, marked as
  returned in the file name (`<id>.returned.json`; `moai inbox --json`:
  `"returned":true`). A returned letter still waiting when a new agent takes
  that name was the previous holder's, and is put away as read. A letter sent to a name
  nobody holds yet still waits for the first agent that takes it, and
  `moai hello --name` carries an agent's letters to its new name.
- **`moai hello` and the hooks keep a presence row** in
  `.moai/agents/<name>.json` — name, vendor, model, role, busy or idle, since
  when, the agent's process and when it started, the session and the tmux
  pane. The `SessionStart` hook registers a Claude session under the name
  Claude Code shows for it, `UserPromptSubmit` marks it busy and `Stop` idle.
  A turn that ends without a `Stop` is marked idle too — Claude's
  `StopFailure` (an API error) and `SessionEnd`, Codex's `Interrupt`, and an
  Antigravity run that stopped on an error. A Codex `SessionEnd` takes the
  session's row away instead. A turn broken off with Esc in Claude or
  Antigravity sends no hook at all, so that row stays busy until the next
  prompt. `StopFailure` needs Claude Code 2.1.78 or later — an older `claude`
  refuses the plugin's hooks as a whole. The hooks keep the row and read the
  letters in the main checkout's tracker even when its `config.toml` cannot be
  read and the rules fall back to a worktree's own tracker, so a turn's end
  still marks the row idle and a Codex `SessionEnd` still takes it away. A turn's
  end that holds the turn with letters marks the row busy before it takes them,
  so a hook that stalls on that write leaves the letters unread for the next
  one rather than marked read and never shown. A window that sets `MOAI_AGENT` is
  registered under that name, by the hooks and `moai hello` alike (not a row
  `moai hello --pid` or `--as` names, and not a Codex window). In Codex,
  `moai hello` takes up the row the session's hooks wrote, found by
  `CODEX_THREAD_ID` (or named with `--as`), or writes the one they would; it
  never ties a row to the app-server's process or the tmux pane it was started
  from.
  `moai agents` lists who is here and sweeps a row whose process is gone; on
  Linux a reused pid is told apart by the time the process started, and a
  session resumed in a new process moves its row there — found by its session
  id, which `moai hello` in a Claude window without hooks reads too — while a
  row whose process still runs here stays with it. On Linux a row also
  names the machine its pid belongs to (`machine` — the boot id and pid
  namespace — and `host` for the screen), so when several containers share one
  repository a row written in another one is never swept for a pid this
  machine does not have: the table and the name-taken refusal show that pid as
  `<pid>@<host>`, `--wake` never knocks on it and prefers an idle worker on
  this machine, and a wait here never marks it alive. Such a row, like a Codex
  row that has no process to look at, is told by `seen`, which every row's own
  hooks (tool calls included) and `moai inbox --wait` write. A row told that
  way that nothing wrote for 20 minutes reads `gone` — `--status idle` and
  `--wake` pass it over — but keeps its role and name for the session to come
  back to; after a day it is swept. A row written on a clock running ahead
  counts as far ahead as it is, from the nearer of the times it was last marked
  and last changed status: more than 20 minutes ahead reads `gone` and more than
  a day ahead is swept at once, and it reads alive again within 20 minutes of
  those times as this clock passes them. A Codex row's letters stay for that session;
  another machine's go back to their senders, since a later session can be
  given the same name. A container started again reads its earlier rows as
  another machine's, so those wait out the day too. Until that day is out,
  another machine's row also keeps its name from any name the tools make up
  here — a Claude session name, which each container counts on its own,
  `<vendor>-` and the start of a session id, or `moai hello`'s
  `<vendor>-<pid>` — even while it reads `gone`: a Claude session resting at
  its prompt writes nothing, and a new session here that took over its name
  would send that live session's letters back. Such a new session gets the
  name with a piece of its session id appended instead — then the whole id,
  then a number, so it always gets a row — and a `moai hello`
  that has to make the name up refuses it (in Codex it appends the piece too).
  A name a window asks for (`MOAI_AGENT`, `moai hello --name`) still comes
  back to it once the old row reads `gone`. `moai agents --json` says on each
  row whether it is this machine's (`here` — true also for a row that names no
  machine); it is measured as the list is read and never written to the row.
- **The hooks deliver letters.** Each prompt (`UserPromptSubmit`) loads the
  letters for the session into the conversation, the end of a turn (`Stop`)
  holds the turn with them, and a session opened after a compaction gets them
  with what it was holding. A delivered letter is marked read; one load stays
  inside the 10,000 characters Claude Code carries per hook, cuts a letter too
  long for it (naming `moai inbox <id> --from <n>`, which goes on from where
  the cut fell),
  takes one `any-idle-worker` letter at a time and says how many are still
  waiting, naming the next one. A
  turn held by letters still gets the closing check. `PreToolUse`, `moai
  status` and the other read commands never open the mailbox. A Claude Code
  subagent shares its parent's process, so in a subagent's tool call the hook
  refuses `moai hello` and `moai inbox --ack`/`--wait` without `--as` — they
  would rename the parent and take its letters.
- **A worker waits for its letters, and `moai send --wake` is a bonus.**
  `moai inbox --ack --wait` is how a worker session — one a person opened —
  gets its next task. While it waits, its presence row reads idle; once it
  takes a letter it reads busy, and a wait that runs out leaves it idle — a
  worker waits inside its turn, where no `Stop` hook marks it, and an agent
  without hooks would otherwise stay at the busy `moai hello` wrote. A name
  with no row gets none from the wait, and `--wake` passes over an agent whose
  `moai inbox --wait` is running (Linux reads it from the processes): that wait
  takes the letter itself. `--wake` knocks once on an idle recipient: `moai inbox` is typed into
  its tmux pane when its presence row carries one, and for a Claude session the
  line printed tells the sender to use SendMessage — naming the session the way
  Claude Code knows it when that is not its row's name, read from Claude's own
  session file as it wakes (`--json`: `send_message_to`). With neither it does
  nothing and says nothing; an agent at work is left alone, and the line says
  since when it has been at work (`--json`: `since`) rather than promising the
  end of its turn. moai never runs an agent's own program to wake it.
- **`moai agents --role <word> --status <word>`** keeps the rows whose role and
  state are exactly those words — `moai agents --role worker --status idle` is
  how a supervisor finds the workers waiting for work. The sweep of rows whose
  process is gone still runs over every row.
- **A fourth skill, `moai-work`, makes a window a worker.** A person calls it
  once in a window they opened — Claude Code, Codex or Antigravity — and the
  window says `moai hello --role worker`, waits with `moai inbox --ack --wait`,
  does the work a supervisor's letter hands over in a worktree by the steps the
  skill carries (they used to travel whole in every brief), reports with
  `moai send` and waits again. Each wait that runs out costs one short turn of
  tokens; nobody needs tmux. When the letter says the person is away, the
  worker settles a design question by its recommendation and leaves a
  `Decided alone:` note instead of waiting. A wait that comes back at once with
  no letter because the mailbox links out of the checkout or into `.git` is not
  run again — the worker tells the person watching, since its report would be
  refused the same way — and a letter moai could not mark read is done once
  before the worker tells the person, since the next wait hands it back at
  once. The steps are laid out as markdown lists, so they read as steps on
  GitHub too. `moai skill install` plants it in every tree.
- **`moai init` adds `.moai/mail/` and `.moai/agents/` to `.gitignore`.** The
  two directories also carry their own `.gitignore`, so a repository that has
  not run `moai init` again does not commit them either.
- **`moai skill install --agent` plants the skills for Codex and Antigravity
  too.** `--agent` takes `claude`, `codex`, `antigravity` or `auto` and can be
  repeated; without it the install is Claude's alone, exactly as before. Codex
  and Antigravity read the same `.agents/skills/` in the repository, so naming
  either writes it for both — there is nothing to register, and committing the
  directory hands it to the team. `auto` takes whichever of `claude`, `codex`
  and `agy` is on PATH (`claude` when none is) and says what it found.
  `--scope` stays Claude's registration; given without Claude, one line says
  so. `--json` adds `agents`, `found`, `agents_dir`, `agents_files` and
  `hooks`, and Claude's keys stand only when Claude is among the agents.
- **The hooks stand in Codex and Antigravity too.** `moai skill install --agent
  codex` plants `.codex/hooks.json` and `--agent antigravity` plants
  `.agents/hooks.json` — commit them with the skills. They call `moai hook
  <event> --dialect codex|antigravity`: the five rules, the board and the
  letters are the same, only the shapes in and out are each agent's. Codex
  runs project hooks once a person trusts them in `/hooks`, and sends them only
  for its shell, `apply_patch` and MCP calls; a patch — through the
  `apply_patch` tool or typed as `apply_patch <<'EOF'` in its shell — is
  judged file by file. Each Codex hook carries only what its event takes, so
  `/hooks` has no configuration warning for the file: `additionalContextLimit`
  stands on `SessionStart`, `UserPromptSubmit` and `PreToolUse`, the events
  that can add context, and `Interrupt` and `SessionEnd` get 3 seconds, the
  most Codex gives them. The letters a Codex turn's end holds the turn with
  stay within 8,000 bytes, because Codex keeps that text to its default of
  about 2,500 tokens and no setting raises it — a longer letter is cut there,
  naming `moai inbox <id> --from <n>` for the rest, where Codex would have
  moved its middle into a file. The question a turn's end asks while work is
  still held fits the same room: open reviews tied to the held work, and held
  rows that would close their epic if deferred, get the room first; the other
  held rows fill what is left from the top, those that do not fit are left out
  whole, and one more line counts them and names `moai prime`. Antigravity has no prompt
  event, so its first model call of a turn loads the board and the letters; a
  turn is held with `decision: continue`. Both carry the same 10,000
  characters as Claude Code —
  measured whole on agy 1.2.16 and 1.2.17, up to a letter written entirely in
  Korean (28 KB). A hooks file moai did not write is left as it is, with
  one line saying so — for Antigravity that includes moai's group with a
  handler of your own in it, or turned off. `moai skill status` judges each
  file by the moai it calls. Codex runs its hooks from a daemon its sessions
  share, so a Codex session's presence row follows its session id, records no
  pid or tmux pane, and goes when the session ends; `moai agents` cannot sweep
  it by its process.
- **The AGENTS block says where no hook stands** — an agent without them, Codex
  before the trust, a tool call that sends none — the rules are words only,
  rules 4 and 5 most of all. `moai init` writes the new block.
- **One text for every agent.** The three trees get the same skills; the steps
  only one agent has — entering a worktree, asking the person, calling the
  review, changing the model, clearing the window (`/clear`, `/new`), calling a
  skill, waking a session, stopping what a review left running — sit in a
  "Words per agent" table in the `moai`, `moai-supervise` and `moai-work`
  skills, one column per agent, and the steps name them in italics. A step an
  agent does not have reads `—`: tell the person and go on.
- **`moai skill status` shows `.agents/skills/`** — not planted, current, or
  how many of its files differ from this version — the two hooks files
  (current, stale, missing, or not moai's), and whether `codex` and `agy` are
  on PATH. `--json` adds an `agents` object (`dir`, `state`, `stale`, `codex`,
  `agy`) and a `hooks` list. The exit code is still 0 whatever it finds.
- **`moai skill uninstall --agent codex|antigravity` prints the `rm -r` lines**
  for moai's skills in `.agents/skills/`, and the `rm` line for that agent's
  hooks file when moai wrote it, and deletes nothing; a plain `uninstall` says
  in one line when the skills are still there. `--json` adds `agents`, `found`,
  `agents_left` and `hooks_left`.

### Changed

- **Rule 3 in the AGENTS block names the review as a step.** It used to say
  `/code-review` alone, a command only Claude Code has, though Codex and
  Antigravity read the same block; it now names the review for all three —
  `/code-review` in Claude Code, and in Codex and Antigravity the review that
  session has, or the agent reading the diff itself — says the review runs
  inside the agent's own session and never starts another agent program, and points at
  the "Words per agent" table in the `moai` skill for the other steps.
  `moai init` writes the new block.
- **`moai-supervise` hands work to waiting workers of any vendor.** The
  supervisor registers with `moai hello --role supervisor`, finds workers with
  `moai agents --role worker --status idle`, sends one letter per idea with
  `moai send` — the assignment only: the idea, the model and difficulty, the
  work beside it, the base branch, the milestone, the root, whether to wait
  again or end the turn after the report, and whether the person is away — and
  waits for the report with `moai inbox --wait`. It no longer reads Claude
  Code's undocumented session files or sends and waits with SendMessage. A
  worker's report is now its last step, after its `Next:` note. On tmux a
  supervisor of any vendor still clears a Claude Code worker's window once the
  report checks out, finding the pane from the worker's presence row; the script
  then sends the next letter and types `moai inbox` into the box it just emptied,
  so the cleared window takes the next task. The clear stops when the worker
  turns out to be waiting for a letter inside its turn, and when the worker's
  row is not this machine's (`here` in `moai agents --json`) — containers can
  share a tmux socket path, so the pane number would name someone else's pane
  here; such a worker is told to wait again instead. Another vendor's input
  box cannot be read yet, so that window is left to the person. A worker on
  another machine or in Codex that reads `gone` has only been quiet for 20
  minutes — a window resting while its person answers reads the same — so the
  supervisor hands on its worktree only once the person says that window ended.
  The supervisor's report wait stops the way the worker's does: on a mailbox
  that links out of the checkout or into `.git` it tells the person instead of
  waiting again, since its letters would be refused too, and a report moai
  could not mark read is checked once.
- **New worktrees stand in `<root>/.worktrees/`**, a place Claude Code, Codex
  and Antigravity share, instead of Claude's `.claude/worktrees/`. The worker
  and supervisor skills create them there, `moai init` adds `/.worktrees/` to
  `.gitignore` and keeps the old line for worktrees still standing in the old
  place, and hook rule 2 does not count an edit under `.worktrees/` as the
  root's work.
- **Hook rule 2 does not count an edit under `.agents/` or `.codex/`**, as it
  already did not for `.claude/` — the skills Codex and Antigravity read
  (`.agents/skills/`), Antigravity's hooks file and Codex's own hooks file and
  config stand there. Editing `.claude/moai-plugin/skills/moai/SKILL.md` passed
  while the same text under `.agents/skills/` was refused until an issue was
  picked up, and `.codex/hooks.json` was refused where `.agents/hooks.json`
  passed.
- **The AGENTS block no longer offers `ready --json` as a loop that runs
  without a person.** It now says a session a person opened reads that shape to
  choose its next row, and that moai never launches or drives a session itself.
  `moai init` writes the new block.
- **`moai skill install` follows a link in Claude's plugin tree only inside the
  checkout it plants in**, as it already did for `.agents/skills`. In a
  subdirectory project that checkout is the subdirectory, so a `.claude` there
  that links up to the repository's own `.claude` is now refused by name
  instead of written through; plant from the top, or make it a directory.
- **In Claude Code, the question a turn's end asks while work is still held
  fits the 10,000 characters a letter gets there.** It used to list every row
  the session held, however long that ran. Now open reviews tied to the held
  work, and held rows that would close their epic if deferred, get the room
  first; the other held rows fill what is left from the top, those that do not
  fit are left out whole, and one more line counts them and names `moai prime`.
  The order on the page is unchanged, and the first line and the "warnings
  grew" line always stand.

### Removed

- **`examples/bash-agent` and `examples/python-agents`.** Both ran a command per
  issue with no person watching — usually `claude -p` — and moai does not launch
  agents or run them headless: a person opens each session. The AGENTS block,
  the skills, the README and the agents page no longer point at them. In their
  place the agents page says how to open a Claude Code, Codex or Antigravity
  session in the repository and make it a worker — including Codex's trust in
  `/hooks` and what to do on a machine where its sandbox cannot stand.

### Fixed

- **`moai skill install` no longer writes Claude's plugin through a committed
  link that points out of the checkout.** A repository that committed
  `.claude/moai-plugin/.claude-plugin/plugin.json -> ~/.bashrc` had that file
  overwritten with the plugin's JSON, and the link stayed a link, so nothing
  showed. The plugin tree is now written the way `.agents/skills` already was:
  a place that is not a regular file is refused by name, and nothing is built
  through a directory link that leaves the checkout. A refused tree stops before
  `claude` is asked to register it. `skill status --json` says `written: false`
  for such a manifest, and `skill uninstall` no longer tells you to delete
  `.claude/moai-plugin/` when that tree lands outside the checkout — deleting
  it would delete what lies behind the link.
- **`moai skill install` checks every place it will write before it writes the
  first file.** `.agents/skills`, the Codex and Antigravity hook files and
  Claude's plugin tree are all checked first, so a place that would be refused —
  a link out of the checkout or into `.git`, something that is not a regular
  file, or a directory that cannot be made because a file or a dangling link
  stands where it goes — now stops the run with nothing written, where it used
  to write the skills and hooks and then fail. `--dry-run` checks the same way
  and exits non-zero with the same message, instead of listing the plan and
  promising a `claude plugin install` the real run would refuse. A hook file is
  judged again just before it is overwritten, so hooks a person added to it in
  the meantime are kept.
- **`moai skill install` builds its temporary files in `.moai/`.** A run killed
  between writing a file and moving it into place left `<file>.tmp.<pid>.<n>`
  in `.claude/moai-plugin/`, `.agents/skills/` or `.codex/`, where nothing
  ignores it and `git add -A` picks it up. They are now built in the checkout's
  `.moai/`, which `moai init` already ignores, as `AGENTS.md`'s temporary file
  already was; where `.moai/` cannot be used — another filesystem, read-only, or
  a link out of the checkout — the file is built beside its target as before.
  The same goes for the `.claude/settings.json` an install edits to remove an
  earlier moai's marketplace declarations.
- **`moai skill status`, `install` and `uninstall` no longer hang on a FIFO, or
  use up memory on a link to `/dev/zero`, in the committed plugin manifest or
  `.claude/settings.json`.** Both are read only as regular files inside the
  checkout, like the snapshot and `AGENTS.md`. A committed `.claude/settings.json`
  that cannot be read — a link out of the checkout or into `.git`, a FIFO, no
  permission — is no longer read to plan removing an earlier moai's marketplace
  declarations; one line names it and says why, and `--json` carries it as
  `settings_unread` (`file`, `kind`, `said`, the shape of `journal_error`). A
  missing file stays quiet.

## [0.6.0] - 2026-10-04

### Added

- **`moai wiki ls` and `moai wiki show <slug>` read the project wiki.** The
  wiki is the markdown pages under `docs/` — or the directory `wiki_dir` in
  `.moai/config.toml` names, a relative path inside the checkout — one page per
  `.md` file. A page's slug is its path without `.md` and its title is its first
  `# ` heading. Nothing is stored in the tracker: the list, the links between
  pages and the issue ids a page names are read from the files every time —
  the name of a skill moai plants (`moai-wiki` has the shape of an id) is never
  read as one.
  Pages ride the branch, so inside a linked worktree that worktree's pages are
  read while the ids are still looked up in the main checkout's tracker. `ls`
  counts the links that lead to no page, the ids the tracker does not have and
  the pages left with conflict markers, and blocks nothing; a page over 1 MB
  stays in the list unread. `show` ends with the pages that link to the one
  shown, and with one line counting the places it could not read when that
  list may be short. `--json` gives each page's `slug`, `title`, `path`, `bytes`, `issues`,
  `links`, `linked_from` (the slugs of the pages linking to it, in list order,
  `[]` when none does — counted from the other pages' links every time, so
  `show` reads every page) and `conflict`, an `error` on a page that could not
  be read, `skipped` for what the walk left out (a link to a directory, a name
  that is not UTF-8, a directory it could not open or list to the end, a name
  whose kind it could not read), and `body` under `show`,
  beside `linked_from_partial` (true) when another page could not be read or
  the walk left something out, so `linked_from` may be short — absent when every
  page was counted. A `wiki_dir` that does not exist yet exits
  0; one that is absolute, climbs out with `..`, leads out of the checkout or
  into `.git/`, or is not a directory fails `moai wiki` alone — every other
  command reads the config as before.

- **`moai skill install` plants a third skill, `moai-wiki`.** It keeps the
  repository's manual — the markdown pages `moai wiki ls` lists — in step with
  the work. A worker sent by `moai-supervise` now asks at the end of each epic
  (brief step 7-4, beside the CHANGELOG's 7-3) whether the epic changed what a
  person does — a key, a command, a flag, a file, a format, a procedure — and if
  it did, fixes the page on its branch before the merge, so the page rides the
  same merge. Called by a person, the skill sweeps the work closed since the last
  release tag and shows the pages it would change once before writing. Nothing
  checks any of it: an epic that changed nothing a person does writes nothing,
  and no warning stands for a stale page. The AGENTS block carries a short
  "The wiki" section — run `moai init` to bring it in.

- **The explorer reads the wiki under `SPC g w`.** The window covers the list
  and the detail the way the statistics window does: the pages `moai wiki ls`
  lists stand on the left, home page first, and the right pane shows the page
  under the cursor, drawn like an issue body. It reads the project you are in,
  or on the overview (`0`) the project of the row under the cursor, and it only
  reads — pages are files you edit and commit. On the page, `Tab` picks the
  links drawn there in turn, from the first one on screen and round from the
  last to the first, and `Shift-Tab` goes the other way from the last one on
  screen: the picked link shows reversed, the key bar names where it goes,
  `Enter` follows it and `Esc` drops it. A click on a link follows it at once.
  With no link picked, `Enter` on a page lists its links, the issue ids it names
  and then the pages that link to it, marked `←`: a page goes there and `Bksp`
  comes back, an id closes the window onto that row (from the overview, inside
  that project), and a link that leads nowhere, an address outside the wiki or
  an id the tracker does not hold is marked and only says so — picking or
  clicking one does the same. `/` searches the titles and bodies of the pages
  and narrows the list; `Esc` drops a picked link first, then clears that
  search, and then closes the window. The `SPC` menu over it holds the screens and `SPC v r`, the wheel
  scrolls the pane under the pointer — on the list, as do `Ctrl-d`, `Ctrl-u`,
  `Ctrl-f` and `Ctrl-b`, without moving the cursor or the page — and a click
  picks the pane and the page. Dragging the line between the two panes resizes
  them; that share is the window's own, kept under `[tui]` as `wiki_width`, and
  until it is dragged the window splits by `list_width`. The
  window is read fresh every time it opens and nothing of it is kept; a wiki that
  does not exist or holds no page opens nothing and says why.

- **A wiki link can land on a heading.** `[epic](glossary.md#epic)`, or
  `[above](#epic)` on the same page, names the heading by its anchor as GitHub
  makes it — lowercase, punctuation dropped, each space a `-`, a repeated heading
  `-1`, `-2` — so the same link lands there on GitHub too. `moai wiki ls` and
  `show` count a link to a heading the page does not have beside the links that
  lead to no page, and block nothing. In `--json` such a link carries `anchor`,
  the part after `#`, and `anchor_resolved`, false when that page has no such
  heading and absent when the page could not be read; a same-page `#anchor`
  stands with `to` naming its own page. The explorer's wiki window opens a heading
  link at that heading, its line marked `▸` until you scroll or leave, and marks
  a link whose heading the page lacks `(no heading)` and opens that page at its
  top. The moai-wiki skill teaches linking to a heading and the two keys — run
  `moai skill install` to bring it in.

### Changed

- **The explorer's list scrolls under the wheel instead of moving the cursor.**
  The wheel over the list, and `Ctrl-d`, `Ctrl-u`, `Ctrl-f`, `Ctrl-b`,
  `PageDown` and `PageUp` with the list focused, now scroll it the way they
  already scroll the board: the chosen row and the detail stay put even once the
  row is off screen, and the list scrolls to its end. The next cursor key (`j`,
  `k`, `gg`, `G`, and `l`, `h` or `Tab` on a group row) moves from that row and
  scrolls back to it, and a search, a filter, a view toggle or going into a
  folder shows the cursor's row again; a reread from a write next door leaves
  the scrolled list where it is. Before, the wheel moved the cursor three rows
  at a time and those keys moved it half a page or a page, so the detail changed
  under every roll. The wheel over the detail still scrolls the detail.

- **The explorer's `SPC` menu takes the mouse.** A click on a menu item is
  that item's key: a group (`+screen`) goes one level down, a toggle keeps the
  menu open as its key does, and `Esc` and `Bksp` on the bottom line close it
  or go up. A click or a wheel roll outside the menu closes it and does what it
  does there — a click on a row puts the cursor on it — the way a movement key
  already closes the menu and moves. Before, a click or the wheel over the open
  menu was ignored and the menu waited for a key. Empty space inside the menu,
  the wheel over it and the right or middle button still do nothing, the menu
  over the statistics window follows the same rules, and a search or filter
  prompt still leaves the mouse to the terminal.

### Fixed

- **The explorer's version line no longer calls a release build "ahead" of an
  answer it heard before that release existed.** The answer from GitHub is kept
  for a day, so after an upgrade a 0.5.0 binary went on reading the v0.3.0 heard
  the day before and said `ahead of the latest release (v0.3.0)` while v0.5.0 was
  out. Now, when the tag it holds is older than the moai you are running and this
  version has not asked yet, it asks again straight away; until the answer comes
  the line reads `latest not checked · v0.3.0 seen today`. A build from source
  that really is ahead of every release asks once and hears the same tag; that
  "ahead" answer is then kept for an hour rather than a day, so a build made from
  the release commit before the release was published stops calling itself ahead
  within the hour after it is. A failed ask counts as that once and is kept for
  the day. `latest.toml` gains an `asked_by` key, the version that last asked.

- **A new issue no longer leaves one yellow cell behind in the explorer.** When
  a new card or row pushed the others down so that the `]` of its `[NEW]` mark
  landed on the first half of a wide character (Hangul, say) that stood there a
  frame before, the terminal blanked the other half in the mark's yellow, and
  the explorer never drew that cell again — it was blank in both frames as far
  as the explorer knew. The cell stayed yellow until something else covered it.
  Now the half a wide character leaves behind is drawn again whenever something
  covers the character. Seen in Ghostty; inside tmux the cell stood only on the
  outer terminal, never on the pane tmux keeps, so `capture-pane` showed nothing.

## [0.5.0] - 2026-10-04

### Added

- **`mv -m` and `defer -m` say so when the text names a file.** `-m <text>`
  takes the text itself, so `moai mv <review> done -m closing.md` closed the
  review with the words `closing.md`, and nothing said so. When the value is one
  line and a file by that name exists, one line on stderr now says the journal
  got those words and how to stream the file instead (`-m - < closing.md`) — the
  line `--body` has carried since 0.3.0, in words of its own. It stands only
  once the words went into the journal; the text is kept as given and the exit
  code does not change. The `-m` help of both now says it takes text, not a path.

- **Under `--json`, a project that cannot be opened carries its `code`.** The
  overview outside a tracker (`status`, `ready`, `tui`), `project ls` and
  `project add` gave such a project only its `error` line, so telling a link to
  fix by hand from anything else meant reading the words. `code` now stands
  beside it with the value the project's own commands stop with: `broken` for a
  snapshot or config moai will not read (a link out of the checkout or into
  `.git/`, not a regular file), `error` otherwise.

### Changed

- **`moai prime` no longer reads a tracker it cannot open as no tracker.** A
  snapshot link out of the checkout, a snapshot it cannot read or a broken config
  printed "No `.moai` here. `moai init` starts one" and `no_tracker:true`, with
  the reason only on stderr — an agent opening a session ran `moai init`, which
  answered that everything was in line. The page now names the reason on its
  first line and says `moai init` will not fix it, and the reason is no longer
  repeated on stderr. Under `--json`, `no_tracker` stands only where there is no
  tracker; a tracker that cannot be opened carries `tracker_error` with `code` —
  the one the repository's other commands stop with — and `said`. The exit code
  is still always 0.

- **An `AGENTS.md` that links out of the checkout or into `.git/`, or is not a
  regular file, is no longer read.** `moai init` leaves it alone, plants
  everything else and names the file and why — put a regular file there to get
  the block, or pass `--no-agents` — and under `--json` it stands in `untouched`
  as `unreadable`. `moai init --check` exits non-zero naming it, and the
  stale-block notice on `moai status` stays quiet. Such a link used to be read:
  `init` said it could not write the block (`unwritable`), while `status` and
  `--check` reported the block of a file outside the repository. A directory
  there used to stop `init` with the system's `Is a directory`. An `AGENTS.md`
  that cannot be read for permissions or encoding still stops `init` before
  anything is planted.

- **The explorer picks its screen under `SPC g`.** `SPC g l` shows the list,
  `SPC g b` the board and `SPC g s` the statistics window. The board was
  `SPC v b` and the statistics `SPC p s`; both keys are gone with no alias, and
  pressed in the menu they now do nothing, like any key it does not know.
  Choosing is not a toggle: picking the screen that already stands leaves it as
  it is, and the menu closes once you pick. The root of the menu names the
  screen that stands, as in `+screen [board]`, and the board is still kept
  under `[tui]` as `layout`. Over the statistics window `SPC` now opens the
  menu with the screens alone — `SPC g l` and `SPC g b` close the window onto
  the list or the board, and `SPC g s` counts again from scratch.

### Fixed

- **The explorer's `Journal` header row clears once the journal can be read
  again.** Since 0.3.0 the row named a journal it could not open until you quit,
  so after a `chmod 644` it went on calling a readable file unreadable. The
  explorer now checks those files again once a minute and drops the ones that
  open; if a `/` note search is in effect, it re-reads the notes too, so the
  search finds what the file holds. It does not check while the stats window is
  open — that window's token totals were counted without the file, and the row is
  what says so. What `moai tui` prints when it quits, and its exit code, still
  name every journal it failed to read along the way.

- **The board no longer hangs or runs out of memory on the files `moai init`
  looks after.** `AGENTS.md`, `.gitignore`, `.gitattributes` and `CLAUDE.md` are
  now read the way the snapshot is: only a regular file inside the checkout and
  outside `.git/`, and no further than the size its open handle gives. A FIFO in
  place of `AGENTS.md`, `.gitignore` or `.gitattributes` hung `moai status`, the
  hook's first board, the overview outside a tracker and the explorer, `moai
  init` waited on any of the four, and a committed `AGENTS.md -> /dev/zero` ran
  them out of memory. In a git repository git itself still waits on a FIFO
  `.gitattributes` under `moai init` and `init --check`. A dotfile that is not
  read is passed over the way one that cannot be read always was, and `init` no
  longer asks for an `@AGENTS.md` pointer in a `CLAUDE.md` it cannot read.

- **`moai init` no longer swaps the repository lock through a link.** A
  committed `AGENTS.md -> .moai/lock` made `init` replace the lock file once,
  and writes running at that moment stopped keeping each other out. No file the
  repository holds is written through a link that lands on a repository lock —
  the one under `.moai`, the one beside a linked `.moai/issues.jsonl`, or those
  of a tracker nested inside the checkout. The lock is matched as a file, so a
  name in another case on a case-insensitive volume is caught too. Such a write
  names the lock, says to put a regular file in place of the link, and changes
  nothing.

- **`moai init` no longer plants ignore or merge rules for a place inside
  `.git/`.** When `.moai` or `.moai/issues.jsonl` linked into `.git/`, `init`
  wrote rules for that place into `.gitignore` and `.gitattributes`, where git
  never looks.

- **`note -b <file name>` no longer calls the note a body.** The line saying
  the text went in as those words now says the note is those words. The same
  line, on every command that gives it, is said only once the command stands —
  a failed call no longer puts it ahead of the `--json` refusal object on stderr
  — and it looks for the file where you typed the command first, so
  `moai -C <dir> …` from elsewhere names a file your shell can stream.

- **A command run from a directory that has been removed says where it failed.**
  It printed a bare `No such file or directory (os error 2)`; it now says it
  cannot tell where you are.

- **`moai edit -a me` and `moai add -a me` make you the assignee.** They wrote
  a person named `me` into the row, and `moai ready`, `moai prime` and the hook
  then read it as someone else's row — picking up a row you had just taken
  needed `--take`. `me` now means you, as it already did in `moai show -a me`.
  Who you are is asked only when `me` is given, and when that cannot be told
  nothing is written. A row that already holds `me` is left as it is; run
  `moai edit <id> -a me` on it again (`moai show --all --archived --json | jq
  -r '.[] | select(.assignee == "me").id'` lists them). `-a ' none '` with
  spaces around it now clears the assignee instead of naming a person `none`.

- **An email with brackets in it stays whole in `Name (email)`.** `--user`,
  `MOAI_ACTOR` and `-a` cut at the last `(`, so `Kim (k(work)@x.io)` stood as
  the name `Kim (k` and the email `work)@x.io`. With such an email in `git
  config`, `moai ready`, `moai prime`, `moai show -a me` and the hook read your
  own rows as someone else's. The split now takes the `(` that the closing `)`
  pairs with, and everything the old split accepted is still accepted. Rows
  written before this release with the cut email keep it and now read as
  someone else's for the same person — `moai edit <id> -a me` puts them right.
  An email holding a `(` with no partner is still misread.

## [0.4.0] - 2026-10-03

### Added

- **The explorer lays the list out as a kanban board.** `SPC v b` turns the list
  into a board and back, and the choice is kept under `[tui]` as `layout`. It is
  the list's layout, not another window: the cursor, the filter, the view,
  search, `[NEW]` and the detail stay as they were. The columns are idea,
  deferred and the config's columns in order — idea is a kind and deferred an
  axis, so nothing new is stored. Only open ideas are cards: a promoted idea is
  stood for by the cards it unfolded into, and the list still shows closed ones.
  At the project root every milestone is a lane and `(no milestone)` comes last;
  inside a milestone or an epic there is one lane. A card is its id, column and
  priority over its title — ending in `/`, as in the list, when `Enter` can go
  into it — with a foot for the `SPC c` columns that are on. `j`/`k` move within
  a column, `h`/`l` to the next one, and a click takes a card. The wheel,
  `Ctrl-d`/`Ctrl-u`, `Ctrl-f`/`Ctrl-b` and `PageDown`/`PageUp` scroll the board,
  not the cursor: the chosen card and the detail stay as they are even once the
  card is off screen, and the board scrolls all the way to its end. The next
  cursor key moves from that card and brings the board back to it, and so does
  anything that lays the board out again — going in or out of a folder, a
  search, a filter, or a `SPC v`, `SPC s` or `SPC c` toggle; a reload leaves the
  scrolled board where it is. When the cursor's card goes away — a
  reload, a view toggle, a filter or a search — the cursor stays in that column,
  on the nearest card left in it, or else moves to the nearest column beside it.
  While a search is being typed it measures from the card the search opened on,
  so fixing a typo brings the cursor back to that card. In the
  overview (`0`) every project stands as a header across the columns, with an
  unfolded project's milestone lanes under it, and the columns are those of
  every project shown; on a header `l`, `h`, `Tab` and `Enter` unfold, fold and
  go in as in the list.

- **`SPC c e` shows the epic a row is in** — a column on the right of the list
  and, on the board, the card's foot, where epics appear instead of as cards.

- **`SPC v i` hides ideas** — the idea column on the board and the idea rows in
  the list alike. Search still finds them, `SPC v a` brings them back, and the
  choice is kept under `[tui]` as `hide_ideas`.

- **A board card that is not yours says whose it is.** A card assigned to
  someone else, or to nobody, grows a dim foot with `→ name` or `unowned` even
  when no `SPC c` column is on; your own cards stay two lines. Whose is decided
  the way `moai ready` decides it, and when who you are is unknown no card is
  set apart. On a narrow card the tags and the epic name shrink first, so the
  word stays in sight.

- **Done work folds into an archive.** A row that has stood in done for
  `archive_days` (14 unless `.moai/config.toml` says otherwise; `0` turns it
  off) is the archive — an epic or a milestone counted from when it got to
  done: when its last member got there, or when the rest were deferred if that
  came later. A group with one recent member stays, and an epic closed today by
  deferring what was left does not drop out the same day. Nothing is
  stored: no field, no column, no command; it is read off the column and the
  clock each time. The explorer's list and board leave the archive out even
  with done shown and the path line counts it (`[archive 312 hidden]`);
  `SPC v o` shows it and is kept under `[tui]` as `show_archived`, and `/`
  search finds it anyway. `moai show --archived` brings it back, done and
  deferred with it; the time filters and `--stale` find it too, and so does
  `-g` once done is let in with `--all`. `moai status` lists only the milestones and epics
  outside the archive and counts the rest in one line, `archived` in `--json`.
  `moai stats` counts it as before.
- **The explorer's filter field says what it takes.** While `SPC f` is open, a
  panel above the field lists the keys and a few examples. With the cursor in
  the value of `assignee=`, `tag=`, `no-tag=` or `milestone=`, the panel lists
  the values this tracker holds instead: people as `name (email)` (picking one
  puts in the email), tags, and milestones by id and title, with `me` and
  `none` where they apply. Typing narrows the list, `Up` and `Down` pick a
  value and `Enter` puts it in. When no list is showing, `Enter` applies the
  filter as before. The panel pushes the list up rather than covering it, the
  same way the `SPC` menu does.

- **The filter text takes four more time items, minutes and `~` ranges.**
  `created_at=`, `updated_at=`, `started_at=` and `done_at=` — in `--filter`
  and the explorer's `SPC f` — are named after the `--json` fields and read
  the row's own field: a group's too, never its members', and a row without
  that field falls in no range. `done_at=` stays when a row is reopened, so add
  `status=done` for what is closed now; `since=`, `created=` and `done=` keep
  their meaning. Every time filter, flags included, now also takes
  `YYYY-MM-DD HH:MM` — that one minute on your own clock, the way a lone day is
  that whole day — and every one that takes a range takes `from~to` as well as
  `from..to`, either side open; `--since` and `since=` still take one end.
  `--filter 'done_at=2026-10-03 00:00~2026-10-05 23:59'` runs to 23:59:59.
  Quote a range that opens with `~` on the command line (`'~2026-10-05'`) —
  zsh reads a bare `~2026-10-05` as a named directory. In `SPC f` a value
  right after `=` may be quoted (`"…"` or `'…'`) and is then one value with
  the spaces inside it (its two ends are trimmed, as `--filter` trims them);
  a quote anywhere else is a character, so `grep=don't` still finds what it
  did.

### Changed

- **`moai show --all` and `-s done` leave the archive out.** They used to give
  every closed row; now done rows past `archive_days` are left out and the tail
  says how many, with `--archived` to bring them in. `moai idea show --all`
  leaves closed ideas past `archive_days` out the same way. A script that pulls
  a complete copy with `--all --json` should use `--archived --json`. The
  `milestones` and `epics` of `moai status --json` leave archived groups out
  the same way, counted in `archived`, and so do the lists on the `moai status`
  board. In the explorer, showing done (`SPC v <n>`) or everything (`SPC v a`)
  no longer brings back done rows past `archive_days`; `SPC v o` does.

- **`moai show --done` dates a group closed by deferring from the deferral.** An
  epic or milestone whose last open members were deferred used to count from
  when its last member finished, so an epic closed today by `moai defer` could
  stand weeks back; it now counts from the day the rest were deferred, if that
  came later. The same clock is what the archive, `--stale` on a done group and
  `moai stats` read. Moving rows in or out and removing them leave nothing on
  the group: one closed by removing the rest or moving them to another epic
  still counts from its last finished member, and a deferred row moved into a
  closed group dates it from that row's deferral.

- **The version line in the `moai tui` header names the latest release.** It
  reads `latest (v0.3.0)` and `ahead of the latest release (v0.3.0)` where it
  used to say only `latest` and `ahead of the latest release`. When the daily
  check fails, the line no longer draws the last answer as if it were fresh: it
  says the check did not happen, then the release it last heard and when —
  `latest not checked (no network) · v0.3.0 seen 2 days ago`. With no earlier
  answer it stays `latest not checked`. If the release last heard is newer than
  yours, the banner and the upgrade line printed on quit still stand.
  `latest.toml` gains a `heard_at` key and keeps `trouble` next to the tag; a
  file written before reads as heard when it was asked. Two reasons were
  shortened so the line fits 80 columns: `(bad answer)` and `(call failed)`.
  When the project numbers need the room, the last answer moves to the row
  under the version line instead of pushing them out.

- **`moai skill install` removes the two Korean writing plugins an earlier
  moai installed alongside it** (`korean-skills@korean-skills` and
  `humanize-korean@im-not-ai`) instead of installing them. It removes them once
  per scope, only where this repository's moai still stands as an earlier
  version (its installed skill teaches them) and only when their marketplace is
  the one an earlier moai added — `claude` does not record who installed a
  plugin, so this is how one an earlier moai put there is told from one a
  person installed. Once a scope holds this version, nothing there is removed
  again. A scope other than `--scope` where the earlier version still stands
  is first brought up to this version (`claude plugin update --scope <that
  scope>`), so it too is removed from only once; if that fails, nothing is
  removed there and the next install tries again. Nothing is removed or
  brought up when moai's own registration fails. A user-scope install another
  repository's moai also stands beside is left, with the command to remove it
  printed, and that scope is not brought up, so it is removed once that moai
  is gone. The marketplaces stay on the machine —
  `claude plugin marketplace remove` reaches the whole machine. At project
  scope an earlier moai also declared them in the committed
  `.claude/settings.json`, so the team kept being offered them: moai deletes
  just those two entries itself, only when they point where an earlier moai
  pointed them and nothing in that file still enables a plugin from them, and
  leaves the rest of the file as it was — commit the change. One line
  says what was removed; a removal that fails prints the command to run by
  hand (for a declaration, where to delete it) and does not change the exit
  code. `--dry-run` lists the removals and the scopes it would bring up. In
  `--json`, `skill install` reports the scopes it brought up under `lifted`
  (`scope`, `command`, `ok`), the removals under `retired` (`id`, `scope`,
  `command`, `ok`), the declarations under `undeclared` (`marketplace`,
  `file`, `ok`), and `kept` in place of `companions`; in `skill uninstall`,
  which already had `kept`, `companions` becomes `retired` in the same shape,
  beside the same `undeclared`.

- **The supervisor's worker brief stands an epic's members in `review` while
  its epic-end review runs.** Where the worker creates the review issue it now
  moves each finished member with `moai mv <member> review --from in_progress`,
  so the board no longer shows work under review as still in progress. The
  `--from` passes over members left in the first column, which that review
  does not see. In a repository whose columns have no `review`, the refusal
  says so and the worker skips the step; the default columns and the config
  `moai init` writes have one. Closing after the merge is unchanged — `moai mv
  <member> done` closes from either column. Run `moai skill install` to plant
  the new text.

- **Giving the assignee filter twice picks either one.** `-a 철수 -a 영희`,
  `assignee=` repeated in `--filter` or `SPC f`, and the two mixed are one
  list read as or, the same as a comma. They used to be refused as a filter
  used twice — a row has one assignee, so "and" never picked anything. It is
  the one filter where repeating means or. The `AGENTS.md` block and the skill
  say so; run `moai init` and `moai skill install` to bring them in.

- **`-m -` on `moai mv` and `moai defer` reads stdin instead of being the
  text.** This breaks a caller that passed a lone `-` as the note: with
  nothing on stdin, or only blank space, it used to write `-` and exit 0, and
  it now exits 1 (`bad_input`) with nothing moved or deferred. With other
  input waiting on stdin, such as a `while read` loop's, that input becomes
  the note, and in a terminal the command waits for the text. The empty case
  is refused because the hook reads only the command line and cannot see that
  text, so this is where an empty closing line is caught.

- **`moai defer` refuses an empty reason with the code `bad_input`.** `-m ''`
  and a reason of blank space were refused with the code `error` under
  `--json`, while an empty `-m -` and an empty `moai note` answer
  `bad_input`. A loop that branches on `code` now reads all three as an
  argument to fix.

### Removed

- **Polishing Korean text is no longer part of moai.** The "Korean text" section
  is gone from the `AGENTS.md` block, the planted moai skill, its
  `references/commands.md` and the supervisor's worker brief, and the hook no
  longer adds a polishing notice to a `moai` write that carries Hangul. Korean
  text goes in as it is written. `moai skill status` no longer shows the
  `alongside` row, and its `--json` no longer carries the `companions` key.
  Rule 2 counts a `_workspace/` folder like any other — the exception existed
  only for the plugin that created it. Run `moai init` to bring an existing
  `AGENTS.md` block up to date.

### Fixed

- **`Esc` in the search box goes back to the row it was opened on, even after a
  reload.** It went back to that row's number, and the file is re-read while
  the box is open — every minute and on every write from another session — so a
  different row could be standing there by then.

- **In the overview, re-reading a project no longer moves the cursor to another
  row.** The overview strings the rows of every unfolded project together, and
  when a project above the cursor was re-read with a different number of rows,
  the cursor kept its row number and stood on whatever row had moved into it.

- **After `SPC p a` registers a project in the overview, the cursor stands on
  that project's header.** With a project above it unfolded, the cursor landed
  on one of that project's rows instead: the new project's place among the
  projects was read as a row number.

- **`moai skill install` writes the path moai was called by, not the one its
  symlinks resolve to.** Called through a symlinked directory — a `target/`
  that links elsewhere, say — it wrote the resolved path into the hooks, so a
  committed `.claude/moai-plugin` changed on every install and `moai skill
  status` said the hooks call another moai when they call the same file. The
  path comes from how moai was called (with `-C`, from where it was typed) and
  is trusted only when it is the same file as the running binary; a path the
  hook line cannot carry, or one under `/proc` or `/dev`, falls back to the
  resolved path. Once the hooks are planted, installing again with any path to
  the same file keeps the planted one, so the tree does not change. `skill
  status` compares the hooks and the running moai by file, not by spelling.

- **`moai mv -m - < <file>` and `moai defer -m - < <file>` write the file's
  text.** They wrote a lone `-` as the note, so a review closed that way had
  `-` for its closing line, and the hook let it through as a line that was not
  empty. `-m -` now reads stdin, the way `note -b -` and `add -b -` already
  did; what that changes for a call that passed `-m -` is under **Changed**.
  Text given on the command line is taken as before.

## [0.3.0] - 2026-10-02

### Added

- **`moai mv --take` takes someone else's work over in one write.** The
  assignee becomes you and a note `Taken-over: <who it was|none>` keeps whose it
  was, in the same write as the move; `-m` says who said yes. It also takes a row
  that already stands in that column, which is how stalled work is reclaimed.
  `--json` carries `taken` and `theirs`, always as arrays.

- **Hook rule 5 — ask before you pick up someone else's work.** The planted hook
  refuses an agent's `moai mv` into a started column on a row whose assignee is
  someone else, or nobody, unless it carries `--take`, and hands back the line to
  run on a yes. It is the one place that refuses: when who you are is unknown it
  lets the move through.

- **A `--body` that names a file says so.** `--body <text>` takes the text
  itself, so `moai add --from - --body plan.md` put the words `plan.md` on the
  epic, not the file's contents. When the value is one line and a file by that
  name exists, one line on stderr says the body is those words and how to stream
  the file instead (`--body - < plan.md`, or with the plan on stdin, the plan
  moved into a file). It covers `add`, `add --from`, `idea add`, `edit -b` and
  `note -b`; the file is not opened, the text is kept as given and the exit code
  does not change. The `--body` help of all of them now says it takes text, not
  a path.

### Changed

- **`moai ready` and `moai prime` hand out only your own rows.** A ready row
  assigned to someone else, or to nobody, stands apart below the list, and under
  `others` in `--json` with `owner` set to `theirs` or `unowned` — the key is
  always there, and `prime --json` carries `others` and `others_rest` the same
  way, each row naming its `assignee`. `p0` is no exception. `prime`'s `picked`
  and the "already picked up" line under `moai ready` drop what someone else
  picked up or nobody owns, and the planted hook no longer counts those rows as
  your focus. Who you are is matched by name or email, the same as `-a me`; when it
  is unknown (no `--user`, `MOAI_ACTOR` or git identity) nothing is set apart.
  A repository run under one name reads as before as long as its rows carry an
  assignee — a row left with `-a none`, or written before rows carried one, now
  stands apart as `unowned`. The next pick in the same epic that `moai mv … done`
  names is your own row too.

- **A `moai mv` of someone else's row into a started column says so.** It still
  moves — one line on stderr names whose it is and the `--take` line to run, and
  the exit code does not change.

- **A journal link that points out of the checkout stands as `outside`.** In
  `journal_error` (`show --json`, `stats --json`) such a link — one that leaves
  the checkout or goes into `.git/` — stood as `kind: failed`, so a machine could
  not tell "fix that link" from a passing I/O failure, and `said` carried an
  English sentence moai wrote itself, so under `MOAI_LANG=ko` a Korean line ended
  in English. `kind` is now `outside` and the reason in `said` comes in the
  chosen language. The same holds when the link's target is missing, cannot be
  reached or is not a regular file: such a link stood as `failed` or
  `permission`, or was passed over without a word. A `kind` you do not know still
  reads as `failed`.

- **A snapshot or config linked out of the checkout, or not a regular file,
  stops the commands in that repository as `broken`.** A `.moai/config.toml`
  linked to a file outside the checkout or inside `.git/` used to be read by
  every command, and a `.moai/issues.jsonl` linked there was read under a
  `tracker_linked` notice, only writes refusing it, as `error` under `--json`.
  Both now stop reads too, with one line naming the file and why. The checkout
  is the directory that holds `.moai`, so a monorepo sub-tracker whose
  `config.toml` links to a shared file elsewhere in the same git work tree stops
  as well — copy the file in, or keep the link inside that directory. A snapshot
  or config that is a directory already stopped every command, but with the
  system's `Is a directory` and the code `error`; it now stops with moai's own
  line in the chosen language and the code `broken`. A loop branching on `code`
  sees `broken` in all of these — the fix is the link or the file
  (`docs/recovery.md`).

- **A `.moai/lock` that is a link or not a regular file stops every write as
  `broken`.** A link there used to be followed wherever it pointed, even inside
  the checkout, and a directory there stopped writes with the system's `Is a
  directory` and the code `error`. moai no longer takes its lock through a link
  at all — one pointing at `issues.jsonl` lands on a file each write replaces —
  so every write now stops with moai's own line naming the lock and why, in the
  chosen language, and the code `broken`. The lock beside a linked
  `.moai/issues.jsonl` follows the same rule, and reads are not affected. moai
  makes that file itself, so the fix is to remove what stands there
  (`docs/recovery.md`).

- **Ties under `--sort created` and `--sort updated` fall to id alone.** A
  plan creates its rows within one second, so rows sharing a `created_at` are
  common. Those ties used to fall to priority, then id, so a priority edit
  between pages moved a row across the `--after` cursor and it was repeated or
  skipped. They now fall to id alone, and
  `--sort created` joins `--sort id` as an order no edit moves; `updated` still
  moves, since an edit changes the row's own `updated_at`. Every other order
  still breaks ties by priority, then id. The explorer's `SPC s` goes through
  the same order, so its created and updated lists change the same way.

### Fixed

- **A `.moai/lock` that is a link no longer loses writes.** A cloned repository
  could commit `.moai/lock` as a link, and every write took its lock through it:
  with `-> /proc/self/fd/2` each process locked its own stderr, so 24 concurrent
  `moai add` all exited 0 and only a handful of issues remained, and
  `-> ../.git/index.lock` broke every later `git commit`; a FIFO there hung every
  write. The lock is now taken only on a regular file, never through a link, and
  only inside the checkout and outside `.git/`. The lock beside a linked
  `.moai/issues.jsonl` is taken the same way, and a link there is no longer
  passed over as a lock moai already holds — `-> /proc/self/fd/3` resolved to
  each process's own `.moai/lock`, so two trackers sharing one snapshot both
  skipped it and lost writes. A snapshot linked onto a lock (`.moai/issues.jsonl -> lock`),
  which replaced the lock on every write and lost writes the same way, is
  refused too. Each of these stops the write with one line naming the file and
  why, nothing is changed, and under `--json` the code is `broken`; links that
  used to be followed, and a directory there, are under **Changed**. A `.moai`
  directory linked out of the checkout no longer leaves a lock file out there.
  The locks beside your user config and read marks still follow links, so
  dotfiles linked by stow or rcm keep working.

- **A snapshot or config that links out of the checkout no longer runs every
  command out of memory.** A cloned repository that committed
  `.moai/issues.jsonl -> /proc/self/pagemap` or `.moai/config.toml -> /dev/zero`
  made every command read without end, and a FIFO there hung every command.
  Both files are now read the way 0.1.6 reads the journal: only when the link
  lands inside the checkout and outside `.git/`, only when it is a regular file,
  and no further than the size its open handle gives. A link whose target is
  missing is measured where it would land, so one pointing out of the checkout
  no longer reads as an empty board. Otherwise the commands in that repository
  stop with one line naming the file and why, and under `--json` the code is
  `broken`; links that used to read, and a directory in either place, are under
  **Changed**. A sibling worktree's snapshot in that state is skipped and named
  in the chosen language, and a FIFO there no longer hangs `moai status`.

- **A journal's file name can no longer repaint the terminal.** A cloned
  repository decides the names under `.moai/journal/`, and a journal moai could
  not read was named as it stood, so an escape sequence in that name reached the
  terminal from `moai show`, `-g`, `--removed` and `stats`, and a newline in it
  drew a `moai:` line that was never said. The name is now folded onto one line
  with its control characters taken out, on stderr and in `journal_error`'s
  `said`.

- **`Tab` on a project the explorer could not open no longer opens it whole
  later.** When a project in the explorer's list could not be opened — a broken
  `config.toml`, or a snapshot that is not a file moai reads — the `Tab` meant
  for it stayed behind, and once the file was fixed, `l` opened the whole
  project, every epic's members included, instead of one level.

- **Journal names hard-linked to one file are read once.** Each line stood twice
  in the history, `-g` and `--removed`, and `work` added its tokens twice. Names
  now fold by the file itself, as symlinked names already did. A clone still gets
  two separate files — git does not carry hard links.

- **`moai add --milestone` with an id that is not a milestone says so on a
  single issue too.** It wrote the field and exited 0 without a word, and the
  mistake surfaced only later as a `dangling_milestone` warning; `add --from`
  and `idea promote` already said it. The row is still written and the exit code
  does not change.

- **`moai edit --milestone` with an id that is not a milestone says so too.**
  It wrote the field and exited 0 without a word, while `moai add` already said
  it. One line on stderr now, under `--json` too and when the field already held
  that id; the row is still written and the exit code does not change. When an
  epic or ancestor decides the milestone instead, the line that says so no
  longer offers that missing id as the way to move it — on `add` either.

- **`moai add -e` and `moai edit -e` say a wrong epic the way `--milestone`
  does.** The line came before the write was accepted, so a write refused
  afterwards had already said "no epic"; it only asked whether some row had that
  id, so `-e <an issue id>`, and `-e <an epic>` on an epic or milestone row, went
  by without a word while `moai status` counted the row as `dangling_epic`; and
  `edit` said it only when the row changed. It now comes after the write,
  measured the way `moai status` measures it, under `--json` too, and `edit`
  says it on every call that writes `-e <id>` and not on one that leaves `-e`
  alone. The row is still written and the exit code does not change.

- **`moai rm` names the rows whose milestone or epic it removed.** Removing a
  milestone ended with `dangling: []`, and the next `moai status` warned
  `dangling_milestone` for the rows on it; removing an epic named only rows
  whose own `epic` field held it, not the ones that took it from a parent (a
  review row made with `--parent`). Those rows are now named on stderr and in
  `--json`'s `dangling`, measured the way `moai status` measures them; a row
  that was already dangling on that axis before the removal is not named.

- **`moai rm` no longer calls rows cut off while a twin of the removed id
  stands.** Where one id stood on two lines, removing one of them named its
  children, blocked rows and epic members as dangling, though the other line
  still answered for them. An epic member is still named when the line left
  behind is not an epic, as `moai status` counts it.

- **The docs no longer point down three wrong paths.** The README said the
  screen defaults to Korean (it is English); `docs/recovery.md` gave
  `git show <commit>:.moai/issues.jsonl` to recover a removed line, which on a
  linked tracker prints only the link — it now uses
  `git cat-file --batch --follow-symlinks`; and the AGENTS block and the refusal
  for `--from - --body -` read as if `--body` took a file.

- **`SPC o t` no longer picks a zone when yours is a rule.** When the zone in use
  is a POSIX rule such as `TZ=JST-9`, it is not a name on the list, so the window
  now opens with no row highlighted and says so at the bottom. Enter alone keeps
  the zone; moving or typing picks one. Before, Enter switched to the first name,
  `Africa/Abidjan`, and wrote it to your config. Typing a filter that matches no
  name now says so, instead of claiming the machine has no timezone data.

- **The explorer says when it could not read a journal.** A journal it could not
  open — someone else's file left at `0600`, say — now stands as a third header
  row, `Journal : <n> unreadable — <first file> (<kind>)`, until you quit; in a
  window too low for the header it is one banner line. Before, the stats window
  and the `/` note search counted without that history and said nothing until the
  explorer quit.

- **Wheel reports no longer reach the shell under mosh.** mosh answers the
  explorer's "are the reports all in?" question itself, before the wheel reports
  still crossing the network arrive. When you quit or open the editor within a
  second of scrolling or dragging, the explorer now keeps dropping mouse reports
  that follow the answer until 300ms pass quietly or a key arrives — that first
  key is lost. Quitting without having scrolled waits no longer and loses nothing.

## [0.2.0] - 2026-10-02

### Added

- **`moai stats` adds the tracker up, and `SPC p s` draws it.** Rows per column,
  priority, tag, assignee, epic and milestone; how many were created and closed
  per week or day (`--bucket`, `--last`); lead time (created → done) and cycle
  time (started → done) as median and p90; and the AI work written in `model:`
  note lines, summed by model and by grade, with reviews apart. It takes the
  filters `moai show` takes, `--by` picks the axes printed in full, and `--json`
  gives the same numbers to a machine — every key is always there but `by`,
  `email` and `journal_error`. What cannot be told is not guessed: a closed row
  with no recorded start counts as `unknown`, not zero minutes, and lines with no
  token count are counted apart. In the explorer `SPC p s` opens the same
  numbers as bars in place of the list and the detail, narrowed by the filter
  that is hung; on the all-projects view it counts the project under the cursor,
  and `b` switches day and week. The numbers come from the snapshot and the note
  lines only — how long a row stood in review, say, is not among them.

- **The explorer takes the mouse.** A click puts the focus on the pane and the
  cursor on the row under it. The wheel moves whichever pane is under the
  pointer — the list's cursor, the detail, the `SPC p s` window — and leaves the
  focus where it was. Dragging the line between the list and the detail resizes
  them; the share the list takes is kept under `[tui]` as `list_width` (detail
  left or right) and `list_height` (detail above or below), in percent, and is
  not written until you drag. Over the `SPC` menu the mouse does nothing, and
  while a form, a picker or a prompt is up the explorer lets it go, so selecting
  and middle-button paste work there as before. Wheel reports still on their way
  when you quit, or when `SPC n` hands the terminal to `$EDITOR`, are thrown
  away rather than landing at the shell prompt as `65;40;12M` or in the editor
  as keystrokes: the explorer asks the terminal (DA1) and drops what comes before
  the answer, so keys typed in that one round trip go with them. A terminal that
  never answers is waited on for one second at most.

- **`TZ` takes a POSIX rule as well as a zone name.** `TZ=JST-9`, `TZ=<+09>-9`
  and `TZ=EST5EDT,M3.2.0,M11.1.0` now set the clock times are written in, on a
  machine with no zoneinfo directory too — the static musl build. A name the
  zoneinfo directory holds still comes first, as in glibc, and the zone is called
  by the text you gave, so `[tui] timezone` takes the same text. Before, a rule
  with angle brackets was ignored without a word in favour of `/etc/localtime`,
  and `JST-9` fell back to UTC with a line saying the zone was unknown. A `TZ`
  that is neither a name nor a rule now falls back to UTC with that line, the way
  glibc reads it. The `SPC o t` list still holds the zoneinfo names alone.

### Changed

- **The explorer holds the mouse from the start.** Over the list and the
  detail a plain drag no longer selects text in the terminal and a middle click
  no longer pastes — most terminals do both with Shift held, iTerm2 with
  Option. `SPC o m` lets the mouse go everywhere, and the choice is kept as
  `[tui] mouse`.

### Fixed

- **Daylight saving time that runs past the next year's start is read as
  permanent.** A zone file whose footer rule ends daylight saving after the next
  year's begins — `AAA3BBB,J1/0,J365/26` — was drawn on standard time for most of
  every year, because that end was taken for a change. moai now reads such a rule
  the way tzcode does, as daylight saving time all year. Footers written by zic
  (`J365/25`) were already read right.

- **A zone file whose header counts more data than the file holds is no longer
  read.** It is reported as not zone data, as tzcode and glibc report it. Before,
  a cut file could still be read with its tail missing, and on a 32-bit machine
  the size those counts add up to could overflow.

## [0.1.6] - 2026-10-01

### Fixed

- **An editor's lock file beside a journal no longer fails every read of the
  history.** Opening `.moai/journal/<email>.jsonl` in Emacs without saving leaves
  a dangling link `.#<email>.jsonl` beside it, and moai counted that link as a
  journal it could not read: while the file stayed open, `moai show`, `-g`,
  `--json` and `--removed` in every session said the history was unreadable and
  exited 1, though no line was missing. Names under `.moai/journal/` that begin
  with `.` are no longer read as journals — moai never writes one, and the ones
  that land there (Emacs locks, macOS `._` AppleDouble files) belong to other
  tools.

- **`assignee=me` in the explorer's filter now means you.** `SPC f` with
  `assignee=me` looked for someone literally named `me` and showed nothing, while
  `moai show -a me` and `--filter assignee=me` already meant the current person.
  The explorer now resolves `me` the same way, to the person the header names in
  the project you are in. When it cannot tell who you are, the prompt refuses the
  filter with the first line of the command line's refusal, in the language you
  picked; it never asks who you are.

- **A journal link that points outside the checkout is no longer read.** If a
  repository committed `.moai/journal/<name>.jsonl`, the old
  `.moai/journal.jsonl` or `.moai/journal` itself as a link to a file outside its
  checkout — `/proc/self/pagemap`, say — `moai show`, `-g`, `--removed` and the
  explorer's note search kept reading until memory ran out. Reads now follow the
  rule writes already did: a file the repository holds follows a link only inside
  its own checkout, and never into `.git/`. A journal is also read no further
  than the size its open file reports, so a name swapped for such a link between
  the check and the read cannot run away either. The skipped link is named on
  stderr with where it points and the run ends non-zero, as with any journal that
  cannot be read; links inside the checkout, such as an old address's file
  linked to the new one, are read as before.

## [0.1.5] - 2026-10-01

### Added

- **`moai show` takes the list out in pieces: `--sort`, `--reverse`, `-n` and
  `--after <id>`.** An agent or a third-party UI reading `show --json` used to get
  the whole list at once — 1.48MB on this repository — in the one fixed order.
  `--sort` takes `priority` (the default), `created`, `updated`, `status`,
  `assignee`, `title` or `id`, the same words the explorer writes to its config,
  and `--reverse` turns the whole order around. `-n` cuts the list and `--after
  <id>` starts the next page after the last id of the page before. The cursor is
  that row's value in the order, not an offset, so rows other sessions create or
  remove meanwhile never shift a page, and a cursor row that has since closed and
  dropped out of the list still works. A row whose place in the order changes
  between pages can repeat or be skipped — ties in every order fall to priority,
  so a priority edit moves rows under `created` too — and `--sort id` is the one
  order no edit moves. Lines that share one id, the twins a merge can leave,
  stand together and a page never splits them. A cursor id that no row carries
  any more is refused with `not_found` rather than starting over silently.
  `--json` stays a bare array — fewer rows than `-n` means the list has ended —
  and the human list names how many were cut and the command for the next page.
  On one id and on `--tree` these flags are refused, not swallowed.

- **`moai ready -n <count>`** cuts what is ready to pick. `held` and `outside`
  stay whole — they say why the list is short — and the count at the top is the
  number before the cut.

- **`moai show --since <when>`, `--created <from>..<to>` and `--done
  <from>..<to>` filter by time.** `--since` reads the row's own `updated_at`, so
  the rows written since a time come back with one flag. A time is `YYYY-MM-DD`,
  a day on your own clock — the time zone the screen and milestone deadlines use;
  the end of a range takes the whole day — or `YYYY-MM-DDTHH:MM:SSZ`, an instant
  in UTC. Either side of a range may be left open, and a single day stands for
  that day. An empty value, a reversed range, a day or time that does not exist,
  and `--done` with a `-s` that leaves out done are refused rather than answered
  with nothing. Asking by time opens what the list hides by default — done,
  deferred and ideas — because a row closed meanwhile changed too; `-s` and
  `--type` narrow it again. `--done` looks at rows
  standing in done now, at the time they last got there, so a reopened row is not
  counted as closed and a group counts from the moment its last member got to
  done. `--since` keys on each row's own stamp, so it misses a removed row, a
  note (`moai note` writes the journal, not the row), a row whose derived value
  changed without a write of its own and a row merged in with an older stamp —
  for a complete copy, pull the whole list and compare.

- **`moai show --removed [--since <when>]` lists the issues `moai rm` took out.**
  A removed row leaves nothing for `--since` to find; its one trace is the
  journal's `rm` line, and this lays those lines out oldest first — `--json` in
  the shape of `journal` in `moai show <id> --json`, so each carries `ts`, `id`
  and `title`. It is history, not state: nothing is held against the snapshot,
  so an id listed there may live again, and `moai show <id>` says whether it
  does. `--since` keys on the `rm` line's own stamp, so a removal merged in with
  an older stamp is missed the way such a row is; the full compare stays the
  complete answer. It reads the journal beside the tracker only, leaves out
  `rm --line` (an unreadable line, not an issue), takes `--since` and no other
  filter, and is refused under `moai <kind> show`. A journal line it cannot read
  that may still hold a removal — one cut short by a full disk or a crash — is
  named on stderr by file and line, and the run ends non-zero; the list still
  comes out. Whether a cut line may hold a removal is read from the kind it
  names itself, and with `--since` only a line stamped in the range, or with no
  stamp left to read, counts, so one old cut line does not fail every later
  pass. moai never rewrites the journal: once you have seen what the line held,
  delete it by hand.

### Changed

- **`-g` looks through notes and move messages too.** A decision written only in
  a `moai note`, or in the `-m` of a move, was invisible to `moai show -g`; on
  this repository `show -g '사용자 결정' --all` goes from 85 rows to 254. The
  journal is read only when `-g` is given, and `query` stays a pure function —
  the command reads the notes and hands them in. The text a `moai rm --line`
  kept is not a note. `moai tui`'s `/` looks through them too, in its whole
  scope and in a notes scope at the end of the Tab cycle, and the detail shows
  the note lines that matched. The explorer opens the journal only while such a
  search, or a `grep=` filter, is applied: then every time it reads the tracker
  again, and a note added on its own is also a reason to read again.

- **The README says where SQL goes.** There is no query language inside moai: the
  filters read values the file does not hold — a group's column, an inherited
  epic — so SQL on `.moai/issues.jsonl` gets them wrong. A new section shows jq
  and DuckDB run on the `--json` output instead, next to one on paging and
  incremental sync.

### Fixed

- **A line written after a cut journal tail stands on its own line.** When the
  journal ended without its newline — a full disk, or a write cut short — the
  next line was glued onto the cut one and neither could be read, so a
  `moai rm` made then vanished from `moai show --removed`. Appending now puts
  the newline back first; the cut line stays as it was — the history skips it,
  and `--removed` names it if it may have held a removal. A file moai may write
  to but not read is appended to as before, without that check. The lines
  `moai init` adds to `.gitignore` and `.gitattributes` go through the same
  append. A whole line that an older moai already glued onto a cut one is read
  back: the history, `-g`, `work` and `--removed` all see it again.

- **A journal reached through a symbolic link is read once.** When a file under
  `.moai/journal/`, or the old `.moai/journal.jsonl`, was a link to another
  journal file — someone who changed their email linking the old
  `<email>.jsonl` to the new one — both names were read, so every line in that
  file stood twice in the history, `-g` and `--removed`, and `work` added its
  tokens twice. Names that resolve to the same file are now read once, and a
  line moai cannot read there is named by that file, not by the link. A hard
  link still reads twice: it cannot be told apart from two files, and git
  commits it as two.

- **A hand-edited stamp no longer reaches the terminal.** A stamp moai cannot
  read was printed as it stood, so an escape sequence or a newline in a journal
  line's `ts`, or in a row's `created_at`, `updated_at`, `started_at` or
  `done_at`, could repaint the screen from `moai show <id>`. Every stamp is now
  folded onto one line with its control characters taken out. The history's
  column names, kinds and names are folded the same way, so a newline in them
  no longer draws a history line that was never written; notes and move
  messages still keep their lines.

- **A refused filter now speaks the language you picked.** `-s todo -s review`,
  an unknown or malformed `--filter` item, a priority out of range and a
  `stale=` that is not a number of days were refused in Korean even on the
  default English screen, and the same Korean came out in the `error` of
  `--json`. The list and the explorer's filter prompt now word these refusals in
  the chosen language, and so do the new time filters and `show --removed`. On
  the command line the command to type instead, such as `-s todo,review`, stands
  as it was, except that a value the shell would split — an assignee's
  `Name (email)` — now comes back quoted, and a third repeat (`-s todo -s review
  -s done`) is no longer dropped from it. `type=` keeps the English sentence
  `--type` gives.

- **The explorer's filter prompt shows what to type instead, in its own form.**
  The prompt was one row and drew only the first line of a refusal, so the
  command to type instead — and the list of keys after an unknown one — never
  showed; had it shown, it was the command line's `-s todo,review`, which the
  prompt refuses again. While you write a filter (`SPC f`) the bottom now holds
  two rows: the refusal stays beside what you typed, and the row above it offers
  the prompt's own `status=todo,review`, `done=` and `status=review,done`. The
  row stands even with nothing refused, so the list does not jump as you type.

- **Times after a zone's last listed change follow the zone's own rule.** zic has
  built zone files `-b slim` by default since 2020b, and a slim file stops listing
  changes once the rule at its end can work them out — New York's last listed one
  is March 2007. moai read the listed ones only, so on a machine with such files
  every later time kept that last offset: New York drew January 2026 an hour off,
  on daylight time, and a day typed into `--created`, `--since`, `--done` or the
  explorer's `created=` picked its rows by the same clock. moai now reads that
  rule, the POSIX TZ string at the end of the file, and follows it from its first
  change after the last listed one, as tzcode's own reader does; a rule it cannot
  read leaves the last listed offset in force, as before. A zone name that points
  at a file which is not zone data, such as `TZ=leapseconds`, is now reported in
  the language you picked rather than in Korean.

- **The explorer's search scopes speak the language you picked, and a `grep=`
  filter marks what it found.** On the English screen a narrowed search (`/`, then
  Tab) was labelled `search·노트` and its badge read `/노트:…`; it now reads
  `search·note` and `/note:…`. The names could not change before because the
  explorer read the scope and the query back out of the badge text; it now holds
  them apart and only draws the badge. A `grep=` typed into `SPC f` found its rows
  but marked nothing, so a row caught by its body or only by a note showed no
  reason in the detail; it now marks the matched text and draws the matching note
  lines, as `/` does. It still leaves the view alone — only `/` brings back what
  the view hides.

## [0.1.4] - 2026-09-29

### Added

- **`moai tui` prints the line that upgrades it when you quit, if a newer release is
  out.** The version line in the header only had room to say that a release is out,
  so whoever saw it went to the README to find out how. The banner now says the line
  will be printed, and quitting leaves
  `curl -fsSL https://raw.githubusercontent.com/buzzni/moai/main/install.sh | sh`
  in the shell — whole, where the banner cut it at 80 columns — with
  `-s -- --dir <dir>` added when the running `moai` lives under your home but not
  in `~/.local/bin`. A build from source (a cargo `target/` or `cargo install`), a
  `moai` outside your home and a machine the releases do not cover get no line — a
  line that does not upgrade the binary you are running is worse than none. It is not
  marked urgent: there is something to receive, not something to fix.

- **`moai rm --line <n>` removes an unreadable line from inside the tool.** A line
  that does not read as an issue — most often the broken twin of an issue that also
  stands whole, left by a hand-resolved merge — could only be removed by opening
  `.moai/issues.jsonl` in an editor. The number is the one `moai show` names. Without
  `--yes` the line is only shown, together with a short hash of it and the whole
  command to type; `--yes` needs that hash in `--match`, so when rows came or went
  after the preview and another line now stands at that number, nothing is removed
  and the refusal names the number the line you saw stands at now. A number that
  points at a readable line is refused. The removed text is printed, handed back
  under `--json` (`dry_run` tells the preview from the removal, `match` carries the
  hash) and kept in the journal's `rm` entry.

### Changed

- **A move key in the `moai tui` SPC menu closes the menu and makes the move.** The
  menu used to ignore every key it did not list, so moving the cursor meant Esc first
  and then the move — two presses for one. Now j·k·h·l, the arrows, gg·G,
  Ctrl-d/u/f/b, Home·End·PgUp·PgDn close the menu and act on the focused pane in the
  same press, toggle levels (`SPC v`, `SPC c`, `SPC s`) included. A letter the open
  level holds itself stays that level's item — `SPC c h`, `SPC v l`, `SPC m g` — even
  where the item is switched off, and a move that would do nothing where the cursor
  stands leaves the menu open rather than just making it vanish.

- **The install one-liner upgrades.** When the `moai` already in the install
  directory is this moai — `install.sh` runs it, and it answers `--version` with
  `moai <version>` and `merge-driver --help` with this tool's first line, as every
  release from 0.1.0 on does — it is replaced without `--force`, and the installer
  says which version it went from and to. The same version downloads nothing and
  exits 0. A `moai` newer than the latest release is left alone unless you ask for an
  older release with `--version`, which replaces it and says it is going down.
  `--force` is for what the installer will not replace on its own — another tool
  called `moai`, or a symlink — and for fetching the same version again. The README
  used to promise that an existing `moai` is never overwritten without `--force`;
  over this moai it now is.
- **What the installer will not replace is refused before anything is downloaded**,
  and the refusal prints the line to run again with `--force` added — in both the
  pipe form and the file form, carrying the directory and version you gave, and
  `MOAI_REPO` when it is not the default. A directory at `<dir>/moai` is refused even
  with `--force`, and an install directory that cannot be made or written stops
  before the download too.

- **The merge driver settles an id that stands twice as one group.** A readable row
  and an unreadable twin under one id went to a person unless all three sides held
  the same rows, so a branch that removed the twin with `moai rm --line` conflicted
  with every branch that had not touched it, and when both had removed it the two
  panels held the same line. The rows under that id are now compared as a group,
  three-way: a group only one side changed is taken as that side holds it, a group
  both sides changed alike is taken once, and only a group the two sides changed
  differently goes to a person — one side removing the live row while the other
  removes its twin still does, and so does removing the twin on one side while the
  issue is edited on the other. This takes back part of 0.1.2's "everything either
  side touched still goes to a person": a branch that brought in a twin is now
  merged as it holds it, at exit 0, and `moai status` is what names the id
  (`Ids standing twice`).

### Fixed

- **The merge driver writes the same bytes whichever way you merge.** Two sides
  holding the same unreadable line with different bytes — key order, a trailing
  blank, a trailing `\r` — used to keep this side's bytes, so merging A into B and B
  into A left different files. The side that changed the bytes now wins, and when
  both did, the smaller bytes win; the line is still carried as written, not rebuilt.
  Unreadable lines both sides added no longer follow the merge direction either.

- **The hook's record of which session picked what no longer lives in the shared temp
  directory.** It sat at a guessable `/tmp/moai-picks-<key>`, where a directory or record
  planted by someone else silently switched rules 1 and 2 off. It now lives beside your
  user config, in `<config dir>/picks/`, created closed to others, and a planted symlink is
  never written through. A worktree kept on its own tracker with `MOAI_HERE=1` no longer
  shares that record with the main checkout — the two read different snapshots, and a pick
  made in the root kept rule 2 off in the worktree. With no config location (or a relative
  `MOAI_CONFIG`) nothing is recorded and the hook judges as it did without a record.
- **One unresponsive project no longer stalls `moai status` outside a `.moai`.** The
  merge-driver notice asks git three questions per project (`check-attr`, then `config`),
  and none of them had a time limit — a project whose git hung held back the board of every
  other project. Each question now gets the same 2 seconds the driver probe already had, and
  a question left unanswered keeps the notice quiet. `moai init` and `moai init --check`
  still wait for the answer, since there "unknown" is not "off". A project directory that
  is itself on a dead mount can still stall the overview before git is asked.
- **Commands that draw no time stay quiet on a machine without zoneinfo.** `moai ready`
  (in a `.moai` and its overview outside one), `moai prime`, `moai show` (the list and the
  tree), `moai idea ls` and `moai show <id> --json` used to resolve the system timezone while
  building the screen, so on a machine with no tzdb — the static musl build dropped into
  Alpine or scratch — each of them printed the "no timezone data" line on stderr without
  drawing a single timestamp. The timezone is now resolved the first time a screen actually
  draws a time, so the line stands where a time is drawn (`moai show <id>`, `moai edit`) and
  wherever deadlines are judged — `moai status` and its overview (a bare `moai` included) and
  the hook, which still resolve it whether or not any milestone carries a deadline. Nothing
  was ever blocked and the exit code is unchanged.
- **A write no longer truncates a temporary file that someone else is still writing.**
  A write that replaces a file goes through a temporary file and a `rename`; that file
  was opened with a plain create, which silently empties whatever already sits at that
  name. Two processes with the same pid in different pid namespaces — two containers
  sharing one mount — could pick the same name. The tracker, the user config and read marks
  are written under a lock that already kept such processes apart on one machine, but a file
  written without it (`latest.toml`, the `AGENTS.md` that `moai init` plants) could end up
  with text that was neither side's. The temporary file is now created exclusively
  (`O_EXCL`): a name that is taken is left alone and the next one is tried, and when every
  name is taken the write fails with the target untouched. The names after the first belong
  to this process alone, so the files a killed write leaves behind — no longer reused by the
  next write with the same pid — can never use them all up.
- **Closing work holds the tracker lock for less time.** `moai mv <id> done` works out
  what the close just freed while it still holds the exclusive lock, and it used to build
  the whole ready list up to three times over to do it, with four more copies of the
  membership maps on top — on this repository that took the lock-held time of a close from
  21.6ms to 35ms, in a place where half a dozen sessions share one `.moai`. It now builds
  the ground once per snapshot and asks it twice: in a debug build on this repository,
  that part of a close went from 194ms to 101ms.
- **A write no longer replaces a symlink with a plain file.** Every file moai rewrites goes
  through a temporary file and a `rename`, and `rename` swaps the link itself, not the file
  it points to. So in a repository with `AGENTS.md -> CLAUDE.md`, `moai init` turned
  `AGENTS.md` into a separate file and never put the block into `CLAUDE.md`, and a
  `.moai/issues.jsonl` linked to another place became a plain file on the next write, leaving
  the file it pointed to behind. Writes now follow the link and keep it. The temporary file is
  still made next to the name that was given — inside `.moai/` for the tracker, where it is
  ignored — and next to the file the link points to only when the two are on different
  filesystems. A file the repository holds (`.moai/issues.jsonl`, `AGENTS.md`) follows a link only
  when it points inside the checkout — a cloned repository could otherwise commit a link to
  `~/.bashrc` and have `moai init` or `moai add` rewrite it — while your own config, read marks
  and update check follow links anywhere, as dotfiles need. A link loop, a link into a directory that is not there, and a link to something
  that is not a regular file (a device, a socket, a FIFO) are refused with the link named and
  nothing written. Two trackers whose `issues.jsonl` point at one file also take the lock of
  the directory that file lives in, so their writes no longer silently undo each other, and
  `moai init` no longer asks for an `@AGENTS.md` line in a `CLAUDE.md` that is the same file
  as `AGENTS.md`.
- **A linked tracker is merged, overlaid and appended to where its rows live.** With
  `.moai/issues.jsonl` (or `.moai` itself) linked to another file in the checkout, git
  merged that file with its default text merge, so two branches editing neighbouring issues
  left conflict markers in the tracker while `moai init --check` said nothing was missing.
  `moai init` now adds a `merge=moai` line for the file the link points at, and `moai status`
  and `init --check` name that line when it is missing. `--worktree` views lost the base
  that tells a row removed here from one created beside, so a row `moai rm`'d on the main
  checkout came back from a sibling worktree; the base is now read where the rows live.
  Journal appends follow a link only inside the checkout, the same rule rewrites follow, a
  directory link on the way included, and nothing the repository holds follows a link into
  `.git/`. When the tracker lives behind a link, `moai init` also names its lock, its
  temporary files and its journal where they really sit — `tracker/lock` for `.moai -> tracker`,
  `shared/lock` and `shared/issues.jsonl.tmp.*` for a linked `issues.jsonl` — since git never
  follows the link to match the `.moai/` lines. `moai status` shows one notice naming the file
  a linked tracker points at; it blocks nothing.
- **`moai init` no longer writes into a `.gitignore` or `.gitattributes` that is a symbolic
  link.** git 2.32 and later never read one inside the checkout, so the lines `init` appended
  through it applied nowhere while `init` said it wrote them. It now leaves the link alone and
  says so — `moai status` shows a `dotfile_linked` notice, `moai init --check --json` gains a
  `linked` key — and asks for a regular file instead: one holding what the link pointed at when
  that file is in the checkout, a fresh one when the link leads outside it.

## [0.1.3] - 2026-09-29

### Changed

- **The planted worker brief no longer teaches a git command the harness refuses.** In a
  session isolated to a git worktree, Claude Code reads each Bash call and refuses what it
  cannot prove stays inside that worktree; the brief's own pre-merge check
  (`git -C <root> symbolic-ref -q HEAD`) and its monorepo step (`cd "$(git -C <root>
  rev-parse --show-prefix)"`) were both such shapes, so a worker doing as told was blocked
  with no way through. The check now happens in the root, the monorepo step takes a
  `<subdir>` the supervisor fills in, and a new block in both the brief and the
  reclaimed-work text names the shapes that pass: one plain command per call, a heredoc
  message in a file (`-m "…"` is fine), several steps in a `bash /abs/script.sh`, no path
  built with `$(…)`, no git aimed at the root from inside the worktree, and a look at
  `git status -- .moai/` before a tracker commit in the shared root. Measured over one
  repository's transcripts: 714 refusals, 608 of them compound commands.
- **Work is never pulled into a running milestone.** The three teachings — the
  `AGENTS.md` block, the supervisor skill and the worker brief — now say who may put
  work into a release that has started: the person attaches it, or it came out of a
  member being worked on and is created inside that member's epic, where the release
  is inherited. An agent no longer writes `--milestone <the running one>` on a row
  that stood outside, and the supervisor answers an outside idea by not sending it
  rather than by hanging the release on it (the 2026-09-21 round, where a worker
  picked up a row outside the running release, is still written down; only the answer
  changed). **Nothing is blocked** — `moai edit --milestone` and `moai mv` behave
  exactly as before, because a refusal here would be a gate.
- **A one-word title or note that opens with a single `-` is refused as a flag.**
  This breaks a caller: `moai add -x`, `moai add -bWHY`, `moai note <id> -x` and
  `moai edit <id> --title -x` used to write that token as the title or note and
  exit 0; they now exit 1 (`bad_input`) and name the way through —
  `moai <verb> -- -x` for `add` and `note`, `moai edit <id> --title=-x` for `edit`,
  whose `--title` has no positional after it for `--` to guard. A title of several
  words, a lone `-`, and a note given with `-b` go in as before.
- **`moai add --json` carries `inherited_milestone`** when the `--milestone` it wrote
  lost to the row's epic or ancestor, the same key `moai edit --json` carries; the
  text output says it on stderr in `edit`'s words.
- **`duplicate_id` names what to type** — `moai show <id>` as its hint, and the
  warning says which row `moai rm <id>` removes.
- **New warning `twin_parent`.** A row below an id that stands twice, where the
  rows hand down different epics (or, when neither has an epic, different
  milestones), now stands in `(lost)` and in no group, and `twin_parent` names the
  row's id. Before, it silently wore the last row's epic or milestone, so swapping
  the two rows moved it, and only the ancestor's `duplicate_id` stood. Rows are
  compared by what they finally hand down, so twins that hand down the same answer —
  an epic row duplicated by a merge, or a member that wrote the epic it already
  inherited — still hand it down. An idea is judged under each of its parent's
  rows: when it folds under one and not the other and that changes what it hands
  down, the rows below it stand in `(lost)` too. `derived_epic` is absent on such a
  row, `-e none` and `--milestone none` do not pick it, and `no_epic`·`no_milestone`
  do not count it. It inherits no deferral and no worktree claim through that id, and
  it stays in `moai ready` labelled `(lost)`. An idea that wrote its own epic is not
  folded under such a parent; it stands at the root. The warning is not fatal;
  resolving the `duplicate_id` brings the row back.

### Fixed

- **The user config keeps the quotes you wrote.** Changing a value such as
  `sort = 'title'` or `color = 'red'` from the explorer, or a read mark, used to
  write it back double-quoted; it now keeps `'…'`, `'''…'''` or
  `"""…"""` as it stood, with the comment after it. A word added to a double-quoted
  array stays double-quoted even when it holds a `"` — one such column name used to
  flip the whole array to single quotes on the next write. A word holding a tab, a
  zero-width space or another invisible character is written double-quoted with an
  escape (`"a\tb"`, `"a​b"`) instead of standing raw inside single quotes.
- `moai add --from … --milestone <id>` and `moai idea promote` no longer say the
  epics stand on a milestone that is not there; they say on stderr that there is no
  such milestone (with `--json` too) and still write it, as `--epic <missing>` does.
  With several `#` roots the line counts them.
- `--body -` that reads nothing from stdin says so on stderr (`add`, `add --from`,
  `edit`) instead of ending in 0 with no body. Nothing is blocked.
- **The hook sees a `find -execdir` write under an absolute start inside the
  repository.** `find /repo/src -execdir tee x \;` used to read every start that
  opens with `/` as somewhere else and drop its relative writes, so rule 2 let it
  through. A relative path in such a line is now read as sitting under each start
  (`/repo/src/x`) — and under the start's parent, where `-execdir` runs for the
  start itself at depth 0 — and counted like any absolute path; `find /tmp -execdir …`
  still goes through. A `-fprint` file on such a line is now seen too. `moai` run through
  `-execdir` is still not counted against this tracker.

### Fixed

- **The planted hook line no longer loses the board, forwards a torn answer, or
  lets a stranger silence its notice.** The board and `Stop` marks are now set by
  the shell line after it has handed the answer over — `moai hook` writes the mark's
  name to a slip the line passes in `MOAI_HOOK_HANDOFF` — so a hook killed between
  the two no longer leaves a session without its board for good. The mark is taken
  only when `moai` finished on its own (exit 0 or 1), and only for absolute paths;
  anything else falls back to `moai` setting the mark itself or to the board riding
  once more. Slips older than ten minutes are swept whenever a new one is written.
  Outside exits 0 and 1, only output shaped like a JSON object counts as an answer: a
  verdict torn by SIGKILL, another binary's usage text or a lone space is dropped and
  the "could not run" notice speaks instead. That notice's once-a-session mark is
  created with `set -C` (one winner among hooks running side by side), and a mark
  that is not the user's own regular file — a stranger's file, a symlink, a named
  pipe — makes the notice speak every time rather than never. The line never opens a
  mark that already exists, so a named pipe planted there cannot stall the hook until
  its timeout. Takes effect after `moai skill install` re-plants the hook; an old
  planted line keeps working as before.

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

- **The detail reads a row's defer from the row, not from the id map.** Which
  defer took a row out of the plan is folded by id and the later line wins there,
  which is right where only an id is in hand — but `moai show` holds the row. So
  where the same id stood twice under one parent, both child lines were given the
  later line's answer: an earlier line deferred under an epic lost its `deferred`
  mark whenever the later line stood in the plan, and an earlier line that stood
  *in* the plan wore the later line's deferred epic — and running the
  `moai defer <that epic> --undo` that mark points at brings that row back not at
  all, because it was never deferred. `moai show --deferred` had been saying the
  opposite about those same rows all along. Two lines that were **both** out of
  the plan but held there by different rows read the same way: each now names the
  row that actually holds it, and a line deferred on its own account beside a twin
  under an epic keeps its own mark instead of losing it. `shelved_by` in `--json`
  and the mark on the opened row itself answer the same as before: `moai show <id>`
  opens the later line, so that line's answer is the one they already gave. **Epic and
  milestone rows are read per line too**, and `moai show --deferred`, the board and
  `moai ready` read them the same way: whether such a row is out of the plan used to
  be folded by id on those surfaces, so an epic held out by a deferred release was
  counted as in the plan whenever a later line sharing its id was.
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
- **The hook reads which group a row is in from the row, not from the id map.**
  Where the same id stands twice, the row you picked up was judged by its twin's
  epic and release, and the hook is the one surface that refuses: `moai add -e
  <the epic that row wrote>` was blocked as work created outside the focus, and
  the refusal named the twin's epic instead — run the command it hands you and it
  is refused again, so the way out was outside the tool. Every place that asked by
  id now asks the row, on both axes: rule 1's unit and the epic its refusal names,
  rule 3's "is this review tied to what you hold", the close-of-session notice,
  the check for a defer that would close an epic, the distance that decides which
  worktree holds a row (so rule 2 no longer hands your work to the worktree named
  after your twin's epic), and the `[NEW]` mark on unread rows. A row eclipsed by a
  twin of another kind stands in no group here either, as it already did on every
  read surface — the hook no longer pins a review to an epic that `moai show -e`
  will not list.
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
