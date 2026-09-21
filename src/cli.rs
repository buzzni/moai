//! argv 의 모양만 정의한다. **로직은 여기 없다.**
//!
//! **도움말은 영어다**(moai-l5uf, 2026-09-20 사용자 결정). 화면의 말은 말묶음이 고르지만
//! (`i18n`) 이 글은 못 고른다 — clap 은 인자를 풀기 **전에** 도움말을 지으므로 `Ctx::lang`
//! 이 정해지기 전이고, `docs/cli.md` 는 이 글에서 지어 커밋되는 한 벌이라 어느 한 말로
//! 서야 한다. 심는 문서를 영어로 통일한 결정(moai-54k2)과 같은 자리다.
//!
//! 아래 `//` 주석은 이 저장소의 글이라 그대로 한국어다 — 화면에 안 나간다.

use crate::model::Kind;
use clap::{Args, Parser, Subcommand, ValueEnum};

// **도움말은 접지 않는다** — clap 의 `wrap_help` 를 뺐다(사용자 결정, moai-opjn). 그것은
// 터미널 폭에 맞춰 낱말 사이에 실제 개행을 넣어, 좁은 창에서 heredoc 여는 줄과 예시
// 명령이 두 줄로 갈려 복사하면 깨졌다. 긴 줄은 터미널이 화면에서만 접고, 복사하면 한
// 줄로 돌아온다. clap 에는 문단 하나만 안 접는 설정이 없어 전체를 끈다 — `help_template`
// 에 적은 글자는 안 접히지만, 그리로 옮기면 명령마다 도움말을 손으로 그려야 한다.
// `narrow_terminals_keep_heredoc_openers_whole` 가 좁은 `COLUMNS` 로 모든 도움말을 통째로 견준다.
#[derive(Parser, Debug)]
#[command(
    name = "moai",
    version,
    about = "Issue tracker. No approval gate. `moai status` shows the discipline.",
    after_help = "\
Start a session like this:

  moai status                   board, warnings, flow. Also the bare call
  moai ready                    what you can pick up now
  moai show <id>                body and history - why it was decided so
  moai mv <id> in_progress      pick it up.  done when finished
  moai note <id> 'what I found' a note for whoever comes next
  moai tui                      explorer - epics open like directories.
                                SPC n jots a thought down

When something not for now comes to mind:

  moai idea add 'a passing thought'  jot it. A title is enough - not work yet
  moai idea promote <id> --from -    unfold it into an epic and issues later

When you are not doing an existing piece of work right now:

  moai defer <id> -m 'next quarter'  out of the plan for a while
  moai defer <id> --undo             pick it back up

To watch several projects from one place:

  moai project add <dir>        registered, moai/status/ready outside a
                                `.moai` show every project at a glance
  moai -C <dir> <command>       other commands need to be told which one

To change the language of the screen:

  MOAI_LANG=ko moai status      English by default. en, ko, zh, ja, es
                                To keep it, put lang = \"ko\" under [i18n]
                                in your user config

To lay out a whole plan at once:

moai add --from - <<'PLAN'
# Epic title
- [p1] first issue #bug
PLAN

There is no approval gate. Create anything, move anything. In exchange
`moai status` shows issues with no epic, reviews stalled for days, and how
much you have opened at once.

`moai <command> --help` tells you all of that command. If the repository has
an AGENTS.md, how to work in that repository is written there."
)]
pub struct Cli {
    /// Without one this is `status` (outside a repository, a glance at the
    /// registered projects; with none registered, this help). **Not an error** -
    /// failing a bare call makes a newcomer think the tool is broken.
    #[command(subcommand)]
    pub cmd: Option<Cmd>,

    /// Machine-readable output. Every human line goes away
    #[arg(long, global = true)]
    pub json: bool,

    /// Turn colour off (same as `--color never`)
    #[arg(long, global = true, conflicts_with = "color")]
    pub no_color: bool,

    // 값과 기본값은 글로 적는다 — clap 이 붙이는 `[default: …] [possible values: …]` 가
    // 옵션 열 옆에서 130칸을 넘었다(moai-c57v). `NO_COLOR` 도 auto 가 읽는다.
    /// auto|always|never (auto by default, off when piped)
    #[arg(
        long,
        global = true,
        value_name = "how",
        default_value = "auto",
        hide_default_value = true,
        hide_possible_values = true
    )]
    pub color: ColorArg,

    /// Run in this directory (same as `git -C`)
    #[arg(short = 'C', long = "dir", global = true, value_name = "path")]
    pub dir: Option<std::path::PathBuf>,

    /// Who is doing this (from `git config` when absent)
    #[arg(long, global = true, value_name = "name (email)")]
    pub user: Option<String>,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq)]
pub enum ColorArg {
    Auto,
    Always,
    Never,
}

// **들여쓴 `after_help` 를 `"\` 줄 잇기로 시작하지 않는다.** 잇기가 개행과 함께 다음
// 줄의 앞 공백까지 먹어 첫 줄만 왼쪽 끝에 붙는다 — 같은 줄에서 글을 시작한다.
// `every_help_keeps_its_indent` 가 모든 명령의 `--help` 를 훑어 잡는다 (moai-p63y).
// **heredoc 예시만은 왼쪽 끝이다** — 여는 줄부터 닫는 줄까지 들여쓰지 않고, 표시는
// `EOF`·`MD` 가 아닌 것(`PLAN`·`NOTE`)을 쓴다. 들여쓴 채 복사하면 셸이 닫는 줄을 못
// 찾고, 겹치는 표시는 커밋 메시지나 노트에 인용할 때 바깥 heredoc 을 닫는다
// (`every_help_heredoc_is_copyable`, moai-foc3).
#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// Board, warnings, flow. A session starts here
    #[command(after_help = "  It blocks nothing. No approval, no gate.
  Instead it surfaces issues with no epic, reviews stalled for days, and how
  much you have opened at once.
  The exit code is non-zero only when the data itself is broken.

  --worktree also overlays the issues of other git worktrees. For one id the
  row that moved column, or was deferred and picked back up, later wins (on a
  tie, the later updated_at), and a row from another branch gets a leading
  branch mark in front of its title.
  A row removed here does not come back unless the other side touched it
  after the fork.
  When a sibling worktree cannot be read the board says so and the exit code
  stays the same.
  Overlaying is for showing only - no file changes.

  What to nag about first comes from .moai/config.toml. Unwritten, these are
  the values, and numbers go without quotes. No value ever blocks - a lower
  one only shows more.
    status_review_days   = 3      days in review before it counts as rot
    status_wip_days      = 2      days untouched after pickup before forgotten
    status_blocked_days  = 3      days blocked before it counts as stuck
    status_wip_limit     = 3      more than this opened at once
    status_no_epic_ratio = 0.15   issues with no epic from this ratio up
    status_no_epic_min   = 5      from this count up, even at a low ratio
    status_flow_days     = 7      the window the flow is measured over
    status_idea_pile     = 5      when this many thoughts have piled up")]
    Status(WorktreeArg),

    /// What you can pick up now
    #[command(after_help = "  Epics themselves, deferred rows and what is under them, and parents with
  unfinished children are left out.
  Urgent first, then epics near the end, then the oldest.

  With --worktree, work already picked up in another worktree drops out here
  and what you hold shows with its branch. For one id the row that moved
  column, or was deferred and picked back up, later wins - so editing only
  the title or priority here does not free what the other side picked up, and
  work the other side deferred later is not offered here.")]
    Ready(WorktreeArg),

    /// A short markdown page - what you hold and what comes next
    #[command(after_help = "  Made for a session's first read and for the context injected again
  after a compact. The board is for a person: it draws warnings, the flow and
  the group bars, and that is far more than the two questions asked there -
  what was I holding, and what comes next.

  Markdown, never coloured, and the exit code is always 0. If it ever spoke
  with a non-zero code, a session that wires it into a start-up hook would
  open on a failure - and then this is a lint, and a lint is a gate.

  Wire it where your editor injects context at session start. For Claude Code
  that is a SessionStart hook, which fires again after a compact:

    moai prime

  With --worktree, work picked up in a sibling worktree shows with its branch
  and drops out of what is next - the same overlay `moai ready` uses.")]
    Prime(WorktreeArg),

    /// Create an issue
    // `-h` 도 설명을 옵션 밑 줄에 둔다(`next_line_help`) — 옆 한 줄 모양이면 옵션 열이
    // `--type <issue|epic|milestone|idea>` 에 맞춰 44칸으로 벌어져 설명이 112칸까지
    // 갔다(moai-x18p). 옵션이 스물이 넘는 명령이라 열을 좁혀도 다음 옵션이 다시 넓힌다.
    // `Typed::Add` 도 같은 까닭으로 같다 — `moai idea add` 는 그것을 접어 넣어 쓴다(moai-g33x).
    #[command(
        next_line_help = true,
        after_help = "\
Examples:
  moai add 'the parser dies on a BOM' -t bug -p 1
  moai add 'storage layer' --type epic
  moai add 'work under a parent' --parent moai-4aex
  moai add 'body from stdin' -b -
  moai add 'to someone else' -a 'Kim (kim@example.com)'   else the creator
  moai add 'with no owner' -a none

Title and body go separately. The title is one line saying what is wrong -
the board, `ready` and the explorer list show the title only. Do not push a
long text into the title; split it into the body. The body is markdown and
`-b -` reads it from stdin:

moai add 'the parser dies on a BOM' -t bug -b - <<'BODY'
- what is wrong: it reads the three leading bytes as the title
- where to fix: the read in src/store.rs
BODY

Several at once (`--from`):

moai add --from - <<'PLAN'
# Storage layer
- [p1] write atomically #bug
- recover a truncated line
- \\[WIP] issue \\#12
PLAN

  A `#` line is an epic, a `-` line is an issue of the epic above it.
  [pN] and #tag are optional.
  Put \\ in front of a leading [ or a trailing #word in a title.
  --dry-run keeps a heredoc typo from creating six wrong issues.

Plan templates (`{{name}}` filled by --var, every variable required):
  moai add --from .moai/templates/release.md --var version=1.2

A title may start with `--`. Anything that is not a known flag is a title."
    )]
    Add(AddArgs),
    /// Open one, or list them
    Show(ShowArgs),
    /// Move the status
    #[command(after_help = "  The last argument is the column to go to, everything before it the issues.
  Column names and their order come from statuses in .moai/config.toml
  (todo, in_progress, review, done by default).

  Skipping ahead and going back are both allowed. This tool has no approval.
  `moai status` surfaces what was rewound and what has stalled.

  moai mv moai-4aex in_progress
  moai mv moai-4aex moai-9k2p done
  moai mv moai-4aex review -m 'tests moved to the next issue'

  When several people share one .moai, say which column you saw. `--from`
  looks again inside the lock and moves only if it is still that column - the
  loser gets one stderr line and a non-zero code. **Give one id** then:
  with several, the won and the lost rows share one exit code.

  moai mv moai-4aex in_progress --from todo

  Closing says what that write opened - work that just became ready, a parent
  whose last unfinished child is now done, and the next pick in the same epic.
  Nothing of that is stored: it is read from the rows each time, and `--json`
  carries unblocked, closable and next, always as arrays.")]
    Mv(MvArgs),
    /// Edit title, body, tags, epic or priority
    #[command(after_help = "  `--epic none` and `--milestone none` clear only the field on that row.
  Membership inherited from a parent or an epic stays.

  Title and body go separately - `--title` is one line, `-b` is a markdown
  body and `-b -` reads it from stdin. **`-b` replaces the whole body.** To
  append one line, read the body first and join it:

{ moai show <id> --json | jq -r '.body // empty'
printf '\\none more line\\n'; } | moai edit <id> -b -")]
    Edit(EditArgs),
    /// Remove
    Rm(RmArgs),
    /// Leave a note on an issue (journal only)
    #[command(after_help = "  moai note moai-4aex 'the parser dies on a BOM'
  moai note moai-4aex -b - < review.txt        a long text from stdin

moai note moai-4aex -b - <<'NOTE'
a long text over several lines
NOTE

  A short finding goes as the positional, a long one - a whole review - with
  `-b -`. The two push each other out: given both, nobody can remember which
  one wins.

  Notes pile up in the journal only and never touch the snapshot.
  `moai show <id>` prints them as history.")]
    Note(NoteArgs),
    /// Take work out of the plan for now (or pick it back up)
    #[command(after_help = "  moai defer moai-4aex                          defer it
  moai defer moai-4aex moai-9k2p -m 'next quarter'   several, with a reason
  moai defer moai-4aex --undo                   pick it back up

  **Neither the column nor the kind changes.** Which column it was in is
  exactly what you need when picking it back up, and the same row has to come
  back. Deferred rows drop out of `moai ready`, the board and the warnings,
  and once they pile up `moai status` says so in one line. Defer an epic, a
  milestone or a parent and the work under it drops out too.

  `moai show --deferred` shows only what is deferred.

  `--from <column>` is the same measure as `mv --from` - a late defer does
  not take a row **whose column moved** out of the plan. Deferring does not
  change the column, so two racing defers are not told apart by it.

  moai defer moai-4aex -m 'next quarter' --from todo")]
    Defer(DeferArgs),

    /// Mark as read (kept in your own config only)
    #[command(after_help = "  moai read moai-4aex              this row as read
  moai read --all                  everything unread that came to me
  moai read -e moai-9k2p           that epic's members and what is under them

  **Nothing is written to the tracker.** Read marks differ per person, so
  writing them on the issue row would make even reading collide with others.
  Next to the config, <config dir>/read/ holds **one file per project** with
  the issue id and the updated_at of the row you saw - change that row after
  that and it becomes unread again. The file name is a hash of the repository
  root, and the path inside it says which root. The old [read] table in the
  config file is still read but never written again.

  Unread rows carry a [NEW] mark in front of the title in the explorer list -
  only what is assigned to me and what is under it (children, reviews, epic
  members) counts.")]
    Read(ReadArgs),

    /// One blocks another (or clear that block)
    #[command(after_help = "  moai link moai-4aex --blocks moai-9k2p     4aex blocks 9k2p
  moai link moai-4aex --unblocks moai-9k2p   clear that block

  `blocked_by` is written on the blocked side, not on the blocking one.
  A cycle (A blocks B while B already blocks A) is refused before any write.")]
    Link(LinkArgs),

    /// The verbs above, pinned to `--type issue`
    #[command(subcommand)]
    Issue(Typed),
    /// The verbs above, pinned to `--type epic`
    #[command(subcommand)]
    Epic(Typed),
    /// The verbs above, pinned to `--type milestone`
    #[command(subcommand)]
    Milestone(Typed),
    /// Jot a passing thought down where you are (`--type idea`)
    #[command(
        subcommand,
        after_help = "  One step below todo. **Jotting has to cost nearly nothing** - neither a
  priority nor an epic is asked for. Title and body still go separately: keep
  the title to one short line and pour a long thought into the body with
  `-b -`. That title becomes the issue title when it is unfolded, so a long
  one here carries straight over.

  moai idea add 'a passing thought'   jot it
  moai idea ls                        what has piled up (same as `idea show`)

moai idea add 'install the merge driver by hand in every clone' -b - <<'IDEA'
Today `moai merge-driver --install` has to be typed once per clone.
IDEA

  An idea is not work - it is in neither `moai ready` nor the board's counts,
  and living without an epic is normal for it, so it never trips the
  \"issues with no epic\" warning.

  Editing and dropping are the verbs you already have: `moai edit <id>`,
  `moai rm <id>`."
    )]
    Idea(IdeaCmd),

    /// Open the explorer (the one write to an issue is `SPC n`, jot)
    #[command(after_help = "  Milestones and epics behave like directories. Move around on the left and
  what the cursor rests on is described on the right.

  j and k or the arrow keys move, Enter goes in, Backspace comes back out.
  On a milestone or epic row, l and the right arrow unfold one step there and
  h and the left arrow fold it. On an unfolded member row, h folds its parent
  and lands on the parent; with nothing to fold it leaves one level. On a
  project header row, l and the right arrow unfold and h and the left arrow
  fold. Tab unfolds everything under it recursively and folds it again on a
  second press. Unfolded members stand with branch marks in the title column,
  and that unfolding is not kept in the config. gg and Home go to the top,
  G and End to the bottom, Ctrl-d and Ctrl-u half a page, Ctrl-f and Ctrl-b
  (PageDown and PageUp) a whole page.
  Ctrl-w w moves the focus between the list and the detail (Ctrl-w W goes the
  other way, Ctrl-w h and Ctrl-w l pick the left and right pane), and every
  movement key moves the focused pane — to scroll the detail, go there with
  Ctrl-w w.
  / searches, Esc clears the filter you set. r marks the row under the cursor
  as read (see [NEW] below).
  The search and filter fields take Enter to apply and Esc to give up, the
  search filters the list as you type, and Tab and Shift-Tab pick where it
  looks: everything, id, title, tag or body. The header at the top
  numbers every registered project, and pressing that number without SPC
  jumps straight there — 0 is everything, one list of all projects.

  The rest lives in the menu that opens the moment you press SPC. The menu
  stands up only what works where you are, ignores keys it does not know,
  closes on Esc or SPC and goes one level up on Backspace. Toggles and sorts
  (SPC v, SPC c, SPC s) do not close it — try them, watch the state, and
  leave with Esc. That level says so at the bottom right with a close hint.
    SPC /    search              SPC f    filter             SPC n    jot
    SPC q    quit
    SPC p a  register            SPC p d  drop from the list
  View — every toggle except the list columns (SPC c) is here:
    SPC v l  deferred            SPC v a  show all
    SPC v 1  first column of the config [shown/hidden] — the next ones count up
             done has no letter of its own: the column that holds it does
    SPC v p  detail pane [shown/hidden]
    SPC v w  overlay worktrees [on/off]
    SPC v r  raw or rendered
  Sorts and columns call priority, created, updated and assignee by the same
  letter — only title (SPC s t) and tag (SPC c t) split one letter:
    SPC s p  priority            SPC s c  created            SPC s u  updated
    SPC s s  column              SPC s a  assignee           SPC s t  title
    SPC c i  id                  SPC c p  priority           SPC c a  assignee
    SPC c c  created             SPC c u  updated            SPC c n  counts
    SPC c t  tag                 SPC c h  column names [shown/hidden]
    SPC c w  branch mark [shown/hidden] — needs SPC v w to overlay first
  Read:
    SPC m a  everything unread   SPC m g  every member of this group
  Options — how this screen draws what stands, not which rows stand:
    SPC o d  detail pane goes right, bottom, left, top — press again to turn
             it. Whether it stands at all is SPC v p
    SPC o t  the timezone times are written in. It opens a window with the
             names this machine knows. Type to narrow it down and pick one.
             Stored times stay UTC, and so does --json
  The one key that quits outright is Ctrl-C — anywhere, even mid-typing.
  The screen rereads itself — issues written next door, and `moai read` or
  `moai project add` in another terminal, land without a keypress.

  Of what came to me (assigned to me or under it), rows changed since the
  last look carry a [NEW] mark in front of the title. Read marks live in my own
  config and the tracker does not change — on the CLI that is `moai read`.

  The list hides done to begin with — the [done hidden] mark on the path line
  says so. The view is separate from the filter, so Esc does not clear it and
  the two apply together.
  Sorting puts urgent, new, earlier column and alphabetical on top, and
  pressing the chosen one again turns it around. When it is not the default
  (priority) the path line says which order it is.
  Columns (SPC c) turn on and off with [shown/hidden]. Assignee, tag, created
  and updated dates stand on the right of the row, and when it gets narrow
  they are dropped in that order — dates, then assignee, then tag — to leave
  room for the title.
  View, sort and columns are written into the [tui] table of the user config
  on every press and carry over to the next run and to other projects (the
  same file `moai project add` writes).

  With registered projects (`moai project add`), 0 lists them all — a header
  row per project with that project's rows under it. Started outside a
  `.moai` it begins there; started inside one it begins in that project.
  Coming out through 0 leaves the project you were in unfolded and the rest
  as header rows only — a project is read the moment it unfolds (its header
  spins while it reads).
  Enter on a header row goes into that project, Backspace only goes up a
  directory. Enter on a group row under it goes into that project and lands
  there — you never drill down in the one list; drilling down always happens
  inside a project.
  View, sort and columns apply to every unfolded project while search and
  filter apply inside one project only — jotting (SPC n) and read (r) go to
  the project of the row under the cursor. With nothing registered and
  started outside, an empty list stands and says to add the first project
  with SPC p a (`--json` gives an empty `projects` array and exits 0).

  SPC p a opens a window to pick a directory and register it as a project —
  it walks in and out one level at a time (Enter and Backspace, moving with
  the same j, k, gg and G as the list), and directories with a `.moai` and those
  already registered are marked. Inside the window, a registers the directory
  under the cursor, `.` shows or hides dotted directories, and g p opens a
  field to type a path (Enter goes, Esc gives up). The window closes on Esc.
  Any subdirectory of a monorepo stands on its own exactly as picked. It is
  taken even without a `.moai`. The window opens inside a project too, so the
  first project can be added with nothing registered.
  On a header row, SPC p d asks once and then only drops it from the list —
  y is yes and any other key gives up. The directory and its `.moai` stay.
  It writes where `moai project add|rm` writes.

  SPC n opens the jot form anywhere inside a project — it is kept as an idea
  (with no epic). If an editor is there ($VISUAL, $EDITOR, or vi or nano on
  PATH) it opens like a git commit message: the first line is the title, then
  a blank line, then the body, and comment lines are guidance to be deleted.
  Leave the title empty, or end the editor with an error, and nothing is
  kept. With no editor the built-in form opens — one title line and a body,
  Tab moves between them (Enter in the title goes to the body) and Ctrl-S
  keeps it. Esc closes it, asking once if you had typed something (y throws
  it away). This is the one write to an issue — editing is done on the CLI.
  `--json` prints only that directory's listing, with no screen (outside a
  `.moai`, the rows of the layer).

  `--path` takes an issue id, or one of the two baskets by the word this
  tool uses for them: none (no milestone) and lost.")]
    Tui(TuiArgs),

    /// Called by Claude's hook. Reads an event on stdin
    #[command(after_help = "  Nobody calls this by hand. The plugin installed into Claude calls it.

  **It blocks nothing, and whatever goes wrong the exit code is 0.** A hook
  that spits errors makes every session start noisy, and then people turn the
  hook off - a rule that is off is no rule.

  The directory (cwd) and the session id come from stdin. They are not in the
  environment.

  Events:
    session-start       Writes the baseline. Loads what is held after a compact
    user-prompt-submit  A person asked. Loads the board once per session
    pre-tool-use        Just before a tool call. The rules stand here
    stop                The turn ends. Checks the state against reality

  echo '{\"session_id\":\"x\",\"cwd\":\"/repo\"}' | moai hook user-prompt-submit")]
    Hook {
        // 값은 글로 적는다 — clap 이 붙이는 `[possible values: …]` 가 `-h` 에서 105칸이 됐다
        // (moai-h0r2). 목록의 글은 `hook::Event` 의 doc 주석과 같다 — 시험이 둘을 견준다.
        /// Which place it was called from (see the list below)
        #[arg(value_name = "event", hide_possible_values = true)]
        event: crate::hook::Event,
    },

    /// Called by git. Merges issues.jsonl per issue, three-way
    #[command(after_help = "  The only thing anyone types by hand is `--install`. Git gives the rest.

  moai merge-driver --install        install into this repository's .git/config
  moai merge-driver --install --as <path>  install with that command

  **When the installed path disappears, git falls back to its own merge.**
  Git reads a driver that did not run as a conflict while leaving this side's
  file as it is, and with no markers in it whoever runs `git add` throws the
  other side away wholesale. So the installed line makes the driver write
  neither an answer nor markers and merges the failed round again with
  `git merge-file` - markers stand, and the worst case equals an uninstalled
  clone. Even so the path is worth keeping: a fallen-back round loses the
  value of resolving per issue. The default is the absolute path of the
  binary running now, and pointing that at a worktree `target/` kills it with
  that worktree - `--local` is shared by the clone, so every checkout lands in
  that state at once. `--as` also takes a word on PATH, but in this
  repository do not pass a bare `moai`: that is the old moai binary and does
  not know this command.

  One line is one issue and they are sorted by id, so two branches that fixed
  different issues collide just because those lines are neighbours. Here they
  are paired by id and merged three-way per issue. Different fields of the
  same issue are merged too - tags and blocks add what was added and remove
  what was removed.

  **When the same field was changed differently, a person resolves it.**
  Picking one side silently makes the other side's edit vanish without a
  trace. With an unreadable line or a duplicated id the whole file is handed
  over inside conflict markers - keeping only what parsed would lose the rest.

  Installing is once per clone. Git reads the driver command from the config
  only, and the config is not committed. In a clone without it, merge=moai in
  `.gitattributes` is simply ignored and git's own merge runs - merging is
  exactly as it was without it. When that repository does set merge=moai,
  `moai status` says in one line that it is not installed here.

  **To say this repository does not want the driver, write that decision in
  `.gitattributes`.** A line for the snapshot that settles merge itself -
  `.moai/issues.jsonl   text eol=lf -merge` - is read as the decision: `init`
  leaves that line alone and every merge-driver line goes quiet. Deleting the
  line instead is read as a gap, and the next `moai init` writes it back.")]
    MergeDriver(MergeDriverArgs),

    /// Install the skills and hooks into Claude (safe to run again)
    #[command(subcommand)]
    Skill(SkillCmd),

    /// Register a directory to watch several projects from one moai
    #[command(
        subcommand,
        after_help = "\
Examples:
  moai project add ~/work/argos         register it. Taken without a .moai
  moai project add repo/apps/a          a monorepo registers subdirs one by one
  moai project ls                       what is registered and its state
  moai project rm ~/work/argos          drop from the list only. Directory stays
  moai project color ~/work/argos green pick a colour (auto picks by path)

  Once registered, `moai`, `moai status` and `moai ready` called outside a
  `.moai` show every registered project at a glance (`--json` gives a
  `projects` array). With `--worktree` each project overlays its sibling
  worktrees too. Other commands do not know which project, so call them as
  `moai -C <dir> <command>`.

  This is **your** config, not the repository's - call it anywhere outside a
  `.moai`. The place is MOAI_CONFIG, then $XDG_CONFIG_HOME/moai/config.toml,
  then ~/.config/moai/config.toml. A relative path is joined to where you are
  (the `-C` directory when you gave one) and symlinks are resolved.

  It does not ask who did it. This is not a file that keeps history."
    )]
    Project(ProjectCmd),

    /// Put a .moai/ into this repository (safe to run again)
    #[command(after_help = "  Run again where it is already installed and only the attached files
  (.gitattributes, .gitignore, AGENTS.md) are brought back in line. Issues
  and the journal are not touched.

  The prefix is decided once - every id already issued carries it.

  A new prefix is up to 8 characters - you type it with every id. A longer
  one is refused with shorter candidates. Without one it is made from the
  directory name: dropping hyphens if that fits (moa-issue becomes moaissue),
  else the initials of the hyphenated words (my-company-backend becomes mcb),
  and with a single word the first 8 characters. A repository already
  installed with a longer prefix is read and written as it is.

  **It installs the merge driver too.** The repository declares merge=moai in
  `.gitattributes`, and the command that word names lives in .git/config,
  which is not committed - so init writes both. It picks the `moai` on PATH
  when that is the same build, else the binary running now, and says which.
  Run it again and a path that has gone dead is replaced. A clone of a
  repository that already has a .moai never runs init: there the one line
  from `moai status` is what asks for `moai merge-driver --install`.

  --no-driver leaves .git/config alone. A repository that wants no driver at
  all says so in `.gitattributes` - a line for the snapshot that settles
  merge itself (`.moai/issues.jsonl   text eol=lf -merge`) is read as the
  decision and init leaves it alone.

  --check writes nothing and only answers whether the AGENTS.md block is
  current, stale or missing, and where the merge driver stands. It is
  non-zero only when a file cannot be read.

  --print only prints that block. That is where to copy it from when the file
  the agent reads is not AGENTS.md - --print and init write the same text.")]
    Init {
        /// id prefix (up to 8). Made from the directory name when absent
        prefix: Option<String>,
        /// Leave AGENTS.md alone
        #[arg(long)]
        no_agents: bool,
        /// Leave .git/config alone (plant no merge driver)
        #[arg(long)]
        no_driver: bool,
        /// Write nothing; say if the AGENTS.md block is stale
        #[arg(long, conflicts_with_all = ["prefix", "no_agents", "no_driver"])]
        check: bool,
        /// Write nothing; print that block (to paste it)
        #[arg(long, conflicts_with_all = ["prefix", "no_agents", "no_driver", "check"])]
        print: bool,
    },
}

/// 등록한 프로젝트 목록을 고치고 본다. 이슈의 동사(`add`·`show`·`rm`)와 이름이
/// 겹치지만 **네임스페이스가 가른다** — 대상이 이슈가 아니라 디렉터리다.
///
/// **자리 인자의 필드 이름을 `dir` 로 짓지 않는다.** clap 은 필드 이름을 id 로
/// 쓰고, 전역 `-C` 의 id 가 `dir` 이다 — 같은 id 면 준 경로가 `Cli::dir` 로 새어
/// `main` 이 먼저 그리로 옮겨 가고, `add argos` 가 `argos/argos` 를 찾는다.
#[derive(Subcommand, Debug)]
pub enum ProjectCmd {
    /// Register a directory (already there, nothing changes)
    #[command(after_help = "\
Examples:
  moai project add .                    the current directory
  moai project add ~/work/argos         taken without a .moai (\"before init\")

  Safe to run again - already registered, it says so and exits 0.")]
    Add {
        /// The directory to register. It must exist; a `.moai` need not
        #[arg(value_name = "dir")]
        path: std::path::PathBuf,
    },
    /// List what is registered - name, path, whether it has a `.moai`
    #[command(after_help = "  The name is the directory name, and when two collide the segment above is
  joined to tell them apart (`apps/a`, `libs/a`).
  It always exits 0 - a broken config file is shown in one stderr line and it
  carries on (with `--json`, in a `problems` array instead of stderr).")]
    Ls,
    /// Drop from the list. The directory and its `.moai` stay
    Rm {
        /// The directory to drop. Found by the written path even if it is gone
        #[arg(value_name = "dir")]
        path: std::path::PathBuf,
    },
    /// Pick the colour that project wears at a glance and in the explorer
    #[command(
        alias = "colour",
        after_help = "\
Examples:
  moai project color ~/work/argos green   green instead of the colour by path
  moai project color ~/work/argos auto    clear it and pick by path again

  The colours to pick from are cyan, green and blue only. Red, yellow and
  magenta already mean error, held work and review, so next to an id they
  would read as something they are not, and bright colours and grey vanish on
  one background or the other. The colour is a companion - the name always
  stands next to it. Use it when two projects land on the same colour.

  It is written as `color = \"green\"` under `[[project]]` in the user config.
  Writing it by hand is fine - a wrong value is shown in one line by
  `moai project ls`, which then uses the colour picked by path."
    )]
    Color {
        /// A registered directory. Found by the written path even if it is gone
        #[arg(value_name = "dir")]
        path: std::path::PathBuf,
        // 필드 이름을 `color` 로 짓지 않는다 — 전역 `--color` 의 clap id 와 겹쳐 준 값이
        // 그리로 샌다(`path` 가 `dir` 을 피한 것과 같다). 값은 여기서 거르지 않고 `cmd` 가
        // `user_config::hue_choice` 로 잰다 — 설정 읽기와 한 자로 재고, `--json` 오류로 선다.
        /// cyan, green, blue or auto
        #[arg(value_name = "colour")]
        hue: String,
    },
}

/// `moai <종류> <동사>` ≡ `moai <동사> --type <종류>`.
///
/// 규칙 하나로 네임스페이스가 생기므로 명사별 코드가 없다. `mv`·`edit`·`rm`
/// 은 여기 없다 — id 가 대상을 정확히 가리켜서 종류를 덧붙일 자리가 없다.
#[derive(Subcommand, Debug)]
pub enum Typed {
    /// Create
    #[command(
        next_line_help = true,
        after_help = "  Title and body go separately - the title is one line saying what is wrong,
  and a long text is a markdown body poured in from stdin with `-b -`.
  For examples see `moai add --help`."
    )]
    Add(AddArgs),
    // `ls` 는 같은 것의 다른 이름이다. **어휘를 둘로 만들지 않으려고 별명으로
    // 둔다** — 목록을 내는 동사가 둘이면 도움말이 둘 다 가르쳐야 한다.
    /// Open one, or list them (`ls` is the same)
    #[command(alias = "ls")]
    Show(ShowArgs),
}

/// idea 만 갖는 동사가 하나 있다 — 펼치기. 그래서 `Typed` 를 그대로 쓰지
/// 못하고, `Typed` 에 넣으면 `moai epic promote` 가 생긴다.
///
/// **공통 동사는 베끼지 않고 [`Typed`] 를 접어 넣는다**(moai-g33x). 손으로 옮겨 적었을
/// 때 `#[command(alias = "ls")]` 가 두 곳에 서고, `cmd/mod.rs` 가 `typed()` 를 안 지나고
/// 같은 두 줄을 다시 적었다 — `Typed` 에 동사를 더하는 날 `moai idea` 만 조용히 안 따라오고
/// 컴파일 오류도 안 났다. 접어 넣으면 `moai idea <동사>` 의 목록이 `Typed` 하나에서 나온다.
#[derive(Subcommand, Debug)]
pub enum IdeaCmd {
    /// `moai idea add` and `moai idea show` (`ls`) - the same verbs, kind pinned
    #[command(flatten)]
    Common(Typed),
    /// Unfold into one epic and several issues, and close that thought
    #[command(after_help = "  The markdown it takes is the same shape as `add --from`. With two shapes,
  you get the grammar wrong every single time.

moai idea promote <id> --from - <<'PLAN'
# Epic title
- [p1] first issue #enhancement
- [p2] second issue
PLAN

  **A line in the plan becomes the issue title as it is.** A long idea title
  carries its length over to the issue, so write a short title again when
  unfolding. The original text stays on that idea and the history leads back.

  Unfolding closes it - that idea goes to `done`. What came from what is kept
  in the journal (the history in `moai show <id>`).

  With the epic already standing, `-e <epic>` unfolds into it as members.
  That is where you take back something the epic needs that had gone out as
  an idea - the plan then holds `- issue` lines only.

moai idea promote <id> -e <epic> --from - <<'PLAN'
- [p1] what the epic set out to do
PLAN

  `--dry-run` is where a person looks at the unfolded plan once and says yes.")]
    Promote(PromoteArgs),
}

#[derive(Args, Debug)]
pub struct PromoteArgs {
    /// The idea to unfold
    #[arg(value_name = "id")]
    pub id: String,

    /// Epic and issues from markdown. `-` is stdin
    #[arg(long, value_name = "file|-")]
    pub from: String,

    /// Unfold as members of this standing epic
    #[arg(short, long, value_name = "epic")]
    pub epic: Option<String>,

    /// Fill `{{name}}` in the template (repeatable)
    #[arg(long = "var", value_name = "name=value")]
    pub var: Vec<String>,

    /// Create nothing; only say what would be created
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args, Debug)]
pub struct AddArgs {
    /// One line. Wrap it in quotes. It may start with `--`
    #[arg(value_name = "title", allow_hyphen_values = true)]
    pub title: Option<String>,

    /// Put it in this epic
    #[arg(short, long, value_name = "id")]
    pub epic: Option<String>,

    /// Put it in this milestone
    #[arg(long, value_name = "id")]
    pub milestone: Option<String>,

    /// Join with commas or give it several times
    #[arg(short, long, value_name = "tag", value_delimiter = ',')]
    pub tag: Vec<String>,

    /// 0 is the highest
    #[arg(short, long, value_name = "0-3")]
    pub priority: Option<u8>,

    /// The column it first stands in. The first column when absent
    #[arg(short, long, value_name = "status")]
    pub status: Option<String>,

    /// Body. `-` reads it from stdin
    #[arg(short, long, value_name = "text")]
    pub body: Option<String>,

    /// Assignee. The creator when absent; `none` clears it
    #[arg(short, long, value_name = "who|none")]
    pub assignee: Option<String>,

    // 설명이 없으면 `next_line_help` 가 공백만 든 줄을 그린다(리뷰 moai-5yq0).
    /// What kind to create (issue when absent, epic under `epic add`)
    #[arg(long = "type", value_name = "issue|epic|milestone|idea")]
    pub kind: Option<Kind>,

    /// Create it as a child of this issue (the id gets a `.xxx`)
    #[arg(long, value_name = "id")]
    pub parent: Option<String>,

    /// Epic and issues from markdown at once. `-` is stdin
    #[arg(long, value_name = "file|-", conflicts_with_all = ["title", "epic", "tag", "priority", "parent"])]
    pub from: Option<String>,

    // 거절은 `clap` 이 아니라 `add::run` 이 한다. `requires = "from"` 은
    // 제목이 없을 때만 걸린다 — `from` 이 제목과 `conflicts` 라서, 제목이
    // 있으면 못 채울 요구로 보고 조용히 건너뛴다. **바로 그 자리가 구멍이다.**
    //
    // 아래 `///` 둘째 문단부터는 `--help` 가 옵션 밑에 펴는 긴 글이다. 줄을 70칸
    // 안에서 손으로 끊고 `verbatim_doc_comment` 로 그 끊음을 지킨다 — clap 은
    // 문단을 한 줄로 이어 붙이고 접지 않아(moai-opjn) 200칸을 넘었다(moai-c57v).
    // 이 파일의 다른 긴 글도 같다.
    /// Create nothing; only say what would be created (with `--from`)
    ///
    /// **It means nothing without `--from`.** It once took it alone, and
    /// then `moai add 'title' --dry-run` printed a line saying it was a
    /// rehearsal and then actually created it - a command called to hold
    /// back that writes instead is the worst kind.
    #[arg(long, verbatim_doc_comment)]
    pub dry_run: bool,

    // 변수가 전부 필수인 까닭은 moai-ahyz.
    /// Fill `{{name}}` in a plan template (with `--from`, several times)
    ///
    /// **Every variable is required** - an unfilled name, an empty value, a
    /// value with a line break, a name not in the plan, or the same name
    /// twice is refused and nothing is created. Names are letters, digits,
    /// `_` and `-`, and values are always title text, so variables belong
    /// in the title only. Like `--dry-run`, giving it without `--from` is
    /// refused.
    #[arg(long = "var", value_name = "name=value", verbatim_doc_comment)]
    pub var: Vec<String>,

    /// Print the id only (for scripts)
    #[arg(short, long)]
    pub quiet: bool,
}

#[derive(Args, Debug)]
pub struct ShowArgs {
    /// An issue id, or a kind (issue, epic). The whole list when absent
    #[arg(value_name = "target")]
    pub target: Option<String>,

    /// Print the body as it is in the file, not rendered
    #[arg(long)]
    pub raw: bool,

    /// Fold it as epic, issue, child
    #[arg(long)]
    pub tree: bool,

    /// Print an epic back as `add --from` markdown
    #[arg(long)]
    pub as_plan: bool,

    #[command(flatten)]
    pub worktree: WorktreeArg,

    #[command(flatten)]
    pub filter: FilterArgs,
}

/// `status`·`ready`·`show` 가 함께 받는다. **전역 플래그로 두지 않는다** — 쓰는
/// 명령(`mv`·`edit`)에 붙으면 겹친 화면을 보고 쓴다고 믿게 되는데, 쓰기는 언제나
/// 제 워크트리 파일에만 간다.
#[derive(Args, Debug, Default, Clone, Copy)]
pub struct WorktreeArg {
    /// Also overlay other worktrees (no file changes)
    #[arg(long)]
    pub worktree: bool,
}

/// 쉼표는 "또는", 반복은 "그리고".
///
/// 쉼표를 clap 에게 맡기지 않는 이유가 있다 — `-s todo,review` 와
/// `-s todo -s review` 가 구별돼야 뒤엣것에 친절한 오류를 낼 수 있다.
#[derive(Args, Debug)]
#[command(next_help_heading = "Filters  (comma = or,  repeated = and)")]
pub struct FilterArgs {
    /// In that column
    #[arg(short, long, value_name = "status")]
    pub status: Vec<String>,

    /// Carrying that tag
    #[arg(short, long, value_name = "tag")]
    pub tag: Vec<String>,

    /// Not carrying that tag
    #[arg(long = "no-tag", value_name = "tag")]
    pub no_tag: Vec<String>,

    /// In that epic (`none` = no epic)
    #[arg(short, long, value_name = "id|none")]
    pub epic: Vec<String>,

    /// In that milestone (`none` = none)
    #[arg(long, value_name = "id|none")]
    pub milestone: Vec<String>,

    /// A child of that issue (`none` = top)
    #[arg(long, value_name = "id|none")]
    pub parent: Vec<String>,

    #[arg(short, long, value_name = "0-3")]
    pub priority: Vec<String>,

    /// That assignee (`none` and `me` too)
    #[arg(short, long, value_name = "who|none|me")]
    pub assignee: Vec<String>,

    #[arg(long = "type", value_name = "issue|epic|milestone|idea")]
    pub kind: Option<Kind>,

    /// In id, title, tag or body
    #[arg(short = 'g', long, value_name = "text")]
    pub grep: Option<String>,

    /// Sitting in its column that long
    #[arg(long, value_name = "days")]
    pub stale: Option<i64>,

    /// Only what is deferred
    #[arg(long)]
    pub deferred: bool,

    /// Include done and what is deferred
    #[arg(long)]
    pub all: bool,

    /// Filters as one string (`status=todo`)
    #[arg(long, value_name = "item=value")]
    pub filter: Vec<String>,
}

#[derive(Args, Debug)]
pub struct MvArgs {
    // 개수는 clap 이 아니라 `mv` 가 본다 — `2 values required by '<id> <id>...'`
    // 는 무엇을 빠뜨렸는지 말해 주지 않는다.
    /// The issues to move, and the column to go to at the end
    #[arg(required = true, num_args = 1.., value_name = "id")]
    pub args: Vec<String>,

    /// One line of note on this move (journal only)
    #[arg(short, long, value_name = "text", allow_hyphen_values = true)]
    pub msg: Option<String>,

    /// Only while still in this column (racing pickups)
    ///
    /// Without it nothing is blocked, as before. With it, the column is
    /// looked at again inside the lock, and a row whose column changed in
    /// the meantime is left untouched and stands as a partial failure.
    #[arg(long, value_name = "column", verbatim_doc_comment)]
    pub from: Option<String>,
}

#[derive(Args, Debug)]
pub struct EditArgs {
    #[arg(value_name = "id")]
    pub id: String,

    /// One line. It may start with `--`
    #[arg(long, value_name = "text", allow_hyphen_values = true)]
    pub title: Option<String>,

    /// Body. `-` reads it from stdin
    #[arg(short, long, value_name = "text", allow_hyphen_values = true)]
    pub body: Option<String>,

    /// Add tags
    #[arg(short, long, value_name = "tag", value_delimiter = ',')]
    pub tag: Vec<String>,

    /// Remove tags
    #[arg(long, value_name = "tag", value_delimiter = ',')]
    pub untag: Vec<String>,

    /// Move the epic (`none` clears only its own field)
    #[arg(short, long, value_name = "id|none")]
    pub epic: Option<String>,

    /// Move the milestone (`none` clears its own field)
    #[arg(long, value_name = "id|none")]
    pub milestone: Option<String>,

    #[arg(short, long, value_name = "0-3")]
    pub priority: Option<u8>,

    /// Give `name (email)`. `none` clears it
    #[arg(short, long, value_name = "who|none")]
    pub assignee: Option<String>,
}

#[derive(Args, Debug)]
pub struct DeferArgs {
    #[arg(required = true, value_name = "id")]
    pub ids: Vec<String>,

    /// Pick it back up
    #[arg(long)]
    pub undo: bool,

    /// Why it is deferred (journal only)
    #[arg(short, long, value_name = "text", allow_hyphen_values = true)]
    pub msg: Option<String>,

    /// Only while it is in this column (racing pickups)
    ///
    /// The same measure as `mv --from`. A row the other side picked up and
    /// started on is not taken out of the plan late. Without it nothing is
    /// blocked, as before.
    #[arg(long, value_name = "column", verbatim_doc_comment)]
    pub from: Option<String>,
}

/// `moai read` — 읽었다고 표시한다.
#[derive(Args, Debug)]
pub struct ReadArgs {
    /// The issues to mark as read
    ///
    /// What was read is always named - called with no argument it would
    /// mark nothing while exiting as a success, and a person would move on
    /// believing it was all marked. `--all` and `-e` fill that place.
    #[arg(value_name = "id", required_unless_present_any = ["all", "epic"], verbatim_doc_comment)]
    pub ids: Vec<String>,

    /// Everything unread that came to me
    #[arg(long)]
    pub all: bool,

    /// That group's members and what is under them
    #[arg(short, long, value_name = "epic")]
    pub epic: Option<String>,
}

#[derive(Args, Debug)]
pub struct RmArgs {
    #[arg(required = true, value_name = "id")]
    pub ids: Vec<String>,
}

/// git 이 주는 자리 셋과, 사람이 치는 `--install`.
///
/// **자리 인자를 `Option` 으로 둔다** — `--install` 은 그것들 없이 부르고, 세 자리를
/// `required` 로 걸면 clap 이 `--install` 만 친 사람을 먼저 거절한다. 빠진 자리는
/// 명령 쪽이 제 말로 거절한다: 거기서는 무엇이 빠졌는지와 심는 길을 한 줄에 댈 수 있다.
#[derive(Args, Debug)]
pub struct MergeDriverArgs {
    /// `%O` - the file at the fork
    #[arg(value_name = "base")]
    pub base: Option<std::path::PathBuf>,
    /// `%A` - this side's file. **The answer is written here too**
    #[arg(value_name = "ours")]
    pub ours: Option<std::path::PathBuf>,
    /// `%B` - the other side's file
    #[arg(value_name = "theirs")]
    pub theirs: Option<std::path::PathBuf>,
    /// `%L` - the length of the conflict markers (7 by default)
    #[arg(value_name = "marker")]
    pub marker_size: Option<usize>,
    /// `%P` - the name of the file being merged. Only used when speaking
    #[arg(value_name = "path")]
    pub path: Option<String>,

    /// Install the driver into this repo's `.git/config`
    #[arg(long)]
    pub install: bool,
    /// The command to install with (default: this binary)
    #[arg(long = "as", value_name = "command", requires = "install")]
    pub as_command: Option<String>,
}

#[derive(Args, Debug)]
pub struct TuiArgs {
    // `none`·`lost` 는 화면 글이 아니라 **이 명령이 받는 낱말**이다(`cmd/tui.rs` 의
    // `NO_MILESTONE`·`LOST`). **`value_name` 에는 안 적는다** — 두 낱말이 옵션 열을 넓혀
    // 이 명령의 전역 옵션 설명까지 80칸을 넘겼다(moai-l5uf). 받는 낱말은 `after_help` 가 댄다.
    /// Open here - a directory opens inside it
    #[arg(long, value_name = "id|basket")]
    pub path: Option<String>,
}

#[derive(Args, Debug)]
pub struct LinkArgs {
    #[arg(value_name = "id")]
    pub id: String,

    /// Block this issue (or issues)
    #[arg(long, value_name = "id", value_delimiter = ',')]
    pub blocks: Vec<String>,

    /// Clear the block on this issue (or issues)
    #[arg(long, value_name = "id", value_delimiter = ',')]
    pub unblocks: Vec<String>,
}

#[derive(Args, Debug)]
pub struct NoteArgs {
    #[arg(value_name = "id")]
    pub id: String,
    /// What the next person (or agent) should read. May start with `--`
    #[arg(value_name = "text", allow_hyphen_values = true)]
    pub text: Option<String>,

    /// A long text. `-` reads it from stdin
    ///
    /// **It pushes the positional out.** Given both, nobody can remember
    /// which one wins, and a rule nobody remembers erases someone's text
    /// sooner or later.
    #[arg(short = 'b', long, value_name = "text", conflicts_with = "text", verbatim_doc_comment)]
    pub body: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum SkillCmd {
    /// Install the plugin tree and register it with `claude`
    #[command(after_help = "  Skills and hooks are installed into `.claude/moai-plugin/` and registered
  with `claude`. Your settings.json is not touched - putting the two keys in
  is `claude`'s job.

  **Nothing is deleted.** Running again only overwrites. Deleting a hook file
  a running session holds would block every tool call of that session.

  The version is a hash of what is installed. Same content, same version, so
  there are no empty updates.

  The two plugins that polish Korean text are installed **at the same scope**
  (korean-skills and humanize-korean). If they cannot be installed, moai's
  own registration still stands, and they are removed along with it.

  moai skill install                  just me (the default. settings.local.json)
  moai skill install --scope user     every repository on this machine
  moai skill install --scope project  with the team (committed settings.json)
  moai skill install --dry-run        only show what would be installed

  A Claude session already open keeps the old version - reopen it to pick
  this one up.")]
    Install {
        // 값과 기본값은 `--color` 처럼 글로 적는다 — clap 이 붙이는 괄호가 `-h` 에서
        // 98칸이 됐다(moai-x18p).
        /// Where to register: local (default), project, user
        #[arg(
            long,
            value_name = "scope",
            default_value = "local",
            hide_default_value = true,
            hide_possible_values = true
        )]
        scope: Scope,

        /// Install nothing; only say what would be installed
        #[arg(long)]
        dry_run: bool,
    },

    /// What is installed at which scope, and where it differs
    #[command(after_help = "  It **only reads** `claude`'s registry (~/.claude/plugins/). Whatever is out
  of line the exit code is 0 - this is a command that shows, not one that
  blocks.

  What it looks at:
    marketplace   registered under this repository's name, not pointing
                  somewhere else
    install       which version at which scope, and whether it matches the
                  version that would be installed now
    companions    whether the two Korean text plugins are in this repository
    hook          whether the executable the install calls is still there
    claude        whether it is on PATH (without it nothing can be installed
                  or removed)")]
    Status,

    /// Remove the registration from `claude`. Installed files stay
    #[command(after_help = "  This repository's install is removed per scope with
  `claude plugin uninstall`, and the marketplace with
  `claude plugin marketplace remove`. Deleting the two keys from your
  settings is `claude`'s job - we do not touch someone else's JSON.

  **`.claude/moai-plugin/` is not deleted.** Deleting a file a running
  session holds can block that session's tool calls. Delete it after closing
  the session.

  The two Korean plugins installed alongside are removed **at the scope moai
  was removed from**. The marketplace is left behind - the name is global to
  one machine and another repository's install uses it.
  A user-scope install is also left behind when another repository's moai
  stands there, and then the command to remove it is printed in one line.

  A Claude session already open keeps calling the old hook - reopen it for
  the removal to land.

  moai skill uninstall --dry-run      only show what would be called")]
    Uninstall {
        /// Call nothing; only say what would be called
        #[arg(long)]
        dry_run: bool,
    },
}

/// 설치 범위. **`--user` 를 못 쓴다** — 그 이름은 이미 "누가 하는가" 다.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq)]
pub enum Scope {
    /// In this repository, where it gets committed (`.claude/settings.json`)
    ///
    /// **Not the default.** What `claude` writes there is an absolute path,
    /// so committing it leaves a line that points nowhere on someone else's
    /// machine, and when that person installs it themselves one more line is
    /// added - they pile up per person.
    Project,
    /// In this repository, just me (`.claude/settings.local.json` - default)
    Local,
    /// In every repository on this machine
    User,
}

impl Scope {
    pub fn as_str(self) -> &'static str {
        match self {
            Scope::Project => "project",
            Scope::Local => "local",
            Scope::User => "user",
        }
    }
}
