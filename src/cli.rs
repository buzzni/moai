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
/// 도움말을 그 키 표로 채운 명령(moai-aqmg). 글에 선 키 틀(`{key:browse.quit}`)을 [`crate::tui::keymap::fill_names`] 가
/// 채운다 — `moai tui --help` 가 사람이 `keys.toml` 에서 바꾼 키를 댄다. **모든 마디를 훑는다**: 어느 명령의 글이 탐색기
/// 키를 대는지 손으로 세면, 새로 댄 글 하나가 틀 그대로 화면에 나간다.
///
/// 틀이 깨졌으면 그 글을 그대로 둔다 — 도움말 하나로 도구가 멈추면 안 된다. 기본 표로 모든 글을 그리는 시험
/// (`every_help_template_names_a_key`)이 깨진 틀을 먼저 잡는다.
pub fn command(keys: &crate::tui::keys::Keymap) -> clap::Command {
    use clap::CommandFactory;
    filled(Cli::command(), keys)
}

fn filled(mut c: clap::Command, keys: &crate::tui::keys::Keymap) -> clap::Command {
    let fill = |h: Option<&clap::builder::StyledStr>| {
        h.map(|h| {
            let h = h.to_string();
            crate::tui::keymap::fill_names(&h, keys).unwrap_or(h)
        })
    };
    if let Some(h) = fill(c.get_about()) {
        c = c.about(h);
    }
    if let Some(h) = fill(c.get_long_about()) {
        c = c.long_about(h);
    }
    if let Some(h) = fill(c.get_before_help()) {
        c = c.before_help(h);
    }
    if let Some(h) = fill(c.get_after_help()) {
        c = c.after_help(h);
    }
    if let Some(h) = fill(c.get_after_long_help()) {
        c = c.after_long_help(h);
    }
    let names: Vec<String> = c.get_subcommands().map(|s| s.get_name().to_string()).collect();
    for n in names {
        c = c.mut_subcommand(n, |s| filled(s, keys));
    }
    c
}

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
                                {key:browse.jot} jots a thought down
  moai web                      the same tracker as a page in a browser

When something not for now comes to mind:

  moai backlog add 'a passing thought'  jot it. A title is enough - not work yet
  moai backlog promote <id> --from -    unfold it into an epic and issues later

When you are not doing an existing piece of work right now:

  moai defer <id> -m 'next quarter'  out of the plan for a while
  moai defer <id> --undo             pick it back up

To read the project manual:

  moai wiki ls                  the markdown pages under docs/ - the wiki
  moai wiki show <slug>         one page
  moai wiki search <words...>   the sections that match, best first
  moai wiki learn '<lesson>'    write a lesson page and note it on the issue

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
    status_wip_limit     = 3      more than this opened at once, an epic as one
    status_no_epic_ratio = 0.15   issues with no epic from this ratio up
    status_no_epic_min   = 5      from this count up, even at a low ratio
    status_flow_days     = 7      the window the flow is measured over
    status_backlog_pile  = 5      when this many thoughts have piled up
    status_due_days      = 3      days before a milestone deadline to say so

  A row whose own deadline has passed is a warning too (overdue). A priority
  behind the deadline its row follows is a notice naming `moai due apply`;
  due_priority = \"off\" silences it, and `moai due --help` has its thresholds.")]
    Status(WorktreeArg),

    /// What you can pick up now
    #[command(after_help = "  Epics themselves, deferred rows and what is under them, and parents with
  unfinished children are left out.
  Urgent first, then what is inside a running milestone, then the earliest
  deadline (none last), then epics near the end, then the oldest.

  Only your own rows are offered. A row that is someone else's or nobody's
  stands apart below - ask before you pick it up (`moai mv <id> <column>
  --take` on a yes). --json carries it under others, always an array, with
  owner theirs or unowned. Who you are comes from --user, MOAI_ACTOR or git
  config; when it is unknown nothing is set apart.

  With --worktree, work already picked up in another worktree drops out here
  and what you hold shows with its branch. For one id the row that moved
  column, or was deferred and picked back up, later wins - so editing only
  the title or priority here does not free what the other side picked up, and
  work the other side deferred later is not offered here.")]
    Ready(ReadyArgs),

    /// A short markdown page - what you hold and what comes next
    #[command(after_help = "  Made for a session's first read and for the context injected again
  after a compact. The board is for a person: it draws warnings, the flow and
  the group bars, and that is far more than the two questions asked there -
  what was I holding, and what comes next.

  Markdown, never coloured, and the exit code is always 0. If it ever spoke
  with a non-zero code, a session that wires it into a start-up hook would
  open on a failure - and then this is a lint, and a lint is a gate.

  With no tracker here it says how to start one (--json: no_tracker). With a
  tracker it cannot read - a link out of the checkout, a broken config, a
  `.moai` that is not a directory - it says why instead and does not send you
  to `moai init` (--json: tracker_error, whose code is the one the other
  commands stop with). When that tracker is one it climbed to from a checkout
  with no `.moai`, it says so and names it (--json: climbed_to), and gives both
  ways out: fix that tracker, or start a separate one for this checkout with
  `moai init` - named with `-C <checkout top>` when you stand below the top
  (--json: init_at). When it climbed past a `.moai` it could not look at, it
  says that instead and does not send you to `moai init` (--json: unseen_at).

  Wire it where your editor injects context at session start. For Claude Code
  that is a SessionStart hook, which fires again after a compact:

    moai prime

  What you hold and what comes next are your own rows - the same split
  `moai ready` makes, with the rest under others.

  With --worktree, work picked up in a sibling worktree shows with its branch
  and drops out of what is next - the same overlay `moai ready` uses.")]
    Prime(WorktreeArg),

    /// Create an issue
    // `-h` 도 설명을 옵션 밑 줄에 둔다(`next_line_help`) — 옆 한 줄 모양이면 옵션 열이
    // `--type <issue|epic|milestone|backlog>` 에 맞춰 44칸으로 벌어져 설명이 112칸까지
    // 갔다(moai-x18p). 옵션이 스물이 넘는 명령이라 열을 좁혀도 다음 옵션이 다시 넓힌다.
    // `Typed::Add` 도 같은 까닭으로 같다 — `moai backlog add` 는 그것을 접어 넣어 쓴다(moai-g33x).
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
# Storage layer @2026-10-25~2026-10-31
- [p1] write atomically #bug @2026-10-26~2026-10-28
- recover a truncated line @2026-10-29
- \\[WIP] issue \\#12
PLAN

  A `#` line is an epic, a `-` line is an issue of the epic above it.
  [pN], #tag and @date are optional. @YYYY-MM-DD is a deadline,
  @YYYY-MM-DD~YYYY-MM-DD a start to a deadline, @YYYY-MM-DD~ a start only.
  --due and --start go on each epic the plan creates that carries no
  @date of its own.
  Put \\ in front of a leading [, a trailing #word or @digit in a title.
  --dry-run keeps a heredoc typo from creating six wrong issues.

Plan templates (`{{name}}` filled by --var, every variable required):
  moai add --from .moai/templates/release.md --var version=1.2

A title of several words may start with `-`. One word that opens with `-`
reads as a flag — put it after `--` (`moai add -- -x`)."
    )]
    Add(AddArgs),
    /// Open one, or list them
    #[command(after_help = SHOW_LIST)]
    Show(ShowArgs),
    /// Count them - spread, flow, lead and cycle time, AI work
    #[command(after_help = STATS_HELP)]
    Stats(StatsArgs),
    /// Move eligible closed rows into yearly archive files
    Archive(ArchiveArgs),
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

  Work that is someone else's, or nobody's, is asked about before it is picked
  up. Moving it into a started column still goes through - one stderr line says
  whose it is. On a yes, `--take` makes you the assignee in the same write and
  leaves a note `Taken-over: <who it was|none>`; `-m` says who said yes. It also
  takes a row that already stands in that column. `--json` carries taken
  (rows whose assignee changed) and theirs (moved without --take), always as
  arrays.

  moai mv moai-4aex in_progress --from todo --take -m 'the owner said yes'

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
    #[command(after_help = "  An unreadable line - a row that cannot be read as an issue, which
  `moai status` counts and `moai show` names by line number - is not
  reachable by id. When it is not worth keeping (a broken twin, junk),
  remove it by its line number; a row a newer moai wrote reads again after
  upgrading, so keep that one:

  moai rm --line 812                          shows the line and its hash
  moai rm --line 812 --yes --match 1a2b3c4d   removes it

  Only an unreadable line is removed; any other line is refused, and the
  refusal lists the unreadable lines as they stand now. Numbers move when a
  row comes or goes - an earlier removal included - so --yes needs the hash
  the preview printed, and a line at that number that no longer matches it
  is refused with the number the line shown stands at now. Removing cannot
  be undone: the raw line is printed, goes into the journal's `rm` entry (up
  to 64KB) and `--json` hands it back whole, to be put back by hand.")]
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
    /// Jot a passing thought down where you are (`--type backlog`)
    #[command(
        subcommand,
        alias = "idea",
        after_help = "  One step below todo. **Jotting has to cost nearly nothing** - neither a
  priority nor an epic is asked for. Title and body still go separately: keep
  the title to one short line and pour a long thought into the body with
  `-b -`. That title becomes the issue title when it is unfolded, so a long
  one here carries straight over.

  moai backlog add 'a passing thought'   jot it
  moai backlog ls                   what has piled up (`backlog show`)

moai backlog add 'install the merge driver in every clone' -b - <<'BACKLOG'
Today `moai merge-driver --install` has to be typed once per clone.
BACKLOG

  Backlog items stand outside `moai ready` and the board's counts,
  and living without an epic is normal for it, so it never trips the
  \"issues with no epic\" warning.

  Editing and dropping are the verbs you already have: `moai edit <id>`,
  `moai rm <id>`."
    )]
    Backlog(BacklogCmd),

    /// The project wiki - the markdown pages under `docs/`, and lessons
    #[command(
        subcommand,
        after_help = "  The wiki is the markdown files under one directory of this checkout -
  `docs/` unless `.moai/config.toml` says otherwise (`wiki_dir = \"manual\"`,
  a relative path inside the checkout). A page is one `.md` file below it.
  Its slug is that path without `.md` (`guide/explorer`) and its title is the
  first `# ` heading, else the file name. README.md and index.md at the top
  come first, the rest by title.

  moai wiki ls                  every page, and what does not resolve
  moai wiki show <slug>         one page, drawn, and the pages linking to it.
                                `--json` gives the raw body
  moai wiki search <words...>   the sections that match, best first, with the
                                line to open
  moai wiki learn '<lesson>'    a new lesson page under lessons/, noted on the
                                issue it came from

  **Nothing is stored.** The tracker holds no page and no index - the list,
  the links, the pages linking to each page (`linked_from`) and the ids are
  read from the files every time. Pages are documents, so they ride the
  branch: inside a linked worktree they are read from that worktree, while
  the tracker still goes to the main checkout. Git merges them like any
  other file.

  No command edits a page - edit the file and commit it. `learn` only
  creates a new lesson page and never touches one that stands. A page names
  an issue by its bare id, as a commit subject does, and another page by a
  relative link: `[the explorer](explorer.md)`. A link can land on a heading
  - `glossary.md#epic`, or `#epic` on the same page. The anchor is the
  heading as GitHub makes it (lowercase, punctuation dropped, each space a
  `-`, a repeated heading `-1`, `-2`), so the same link works there too."
    )]
    Wiki(WikiCmd),

    /// Open the explorer (writes a jot, `{key:browse.jot}`, and gantt plans)
    #[command(after_help = "  Milestones and epics behave like directories. Move around on the left and
  what the cursor rests on is described on the right.

  {key:moves.line_down} and {key:moves.line_up} or the arrow keys move, {key:browse.enter} goes in, {key:browse.leave} comes back out.
  On a milestone or epic row, {key:browse.expand} and the right arrow unfold one step there and
  {key:browse.collapse} and the left arrow fold it. On an unfolded member row, {key:browse.collapse} folds its parent
  and lands on the parent; with nothing to fold it leaves one level. On a
  project header row, {key:browse.expand} and the right arrow unfold and {key:browse.collapse} and the left arrow
  fold. {key:browse.expand_all} unfolds everything under it recursively and folds it again on a
  second press. Unfolded members stand with branch marks in the title column,
  and that unfolding is not kept in the config. {key:moves.top} and Home go to the top,
  {key:moves.bottom} and End to the bottom. {key:moves.half_down} and {key:moves.half_up} scroll half a page, {key:moves.page_down} and
  {key:moves.page_up} (PageDown and PageUp) a whole page and the wheel three rows — the
  list, not the cursor: the chosen row and the detail stay put even once the
  row is off screen, and the next cursor key moves from that row and scrolls
  back to it.
  {key:browse.focus_next} moves the focus between the list and the detail ({key:browse.focus_prev} goes the
  other way; {key:browse.focus_left} and {key:browse.focus_right} pick the left and right pane, {key:browse.focus_up} and
  {key:browse.focus_down} the top and bottom one when the detail is split that way), and every
  movement key moves the focused pane — to scroll the detail, go there with
  {key:browse.focus_next}.
  {key:browse.grep} searches, {key:browse.clear_filter} clears the filter you set. {key:browse.read} marks the row under the cursor
  as read (see [NEW] below). {key:browse.help} opens a window listing every key
  ({spc:browse.help} from the menu), read from the same table as moai keys.
  The movement keys scroll it; {key:browse.help} or {key:browse.quit} closes it
  rather than quitting (Ctrl-C quits from anywhere).
  The search and filter fields take Enter to apply and Esc to give up, the
  search filters the list as you type, and Tab and Shift-Tab pick where it
  looks: everything, id, title, tag, body or note (everything reads the
  notes too). In the filter field, Tab completes key names and values,
  Shift-Tab cycles backwards, and another key keeps the current candidate;
  a unique key gains an equals sign so its value can be typed straight away.
  The filter field lists the keys it takes and a few examples
  above itself, and with the cursor in the value of assignee=, tag=, no-tag=
  or milestone= it lists the values there instead: typing narrows them, Up
  and Down pick one, and Enter puts it in; with no list standing it applies
  the filter. Fixed fields also complete status names, priority p0 to p3 and
  type names with Tab; these fields keep Enter to apply. The header at the
  top numbers every registered project, and
  pressing that number without SPC jumps straight there — 0 is everything,
  one list of all projects.

  The rest lives in the menu that opens the moment you press SPC. The menu
  stands up only what works where you are, ignores keys it does not know,
  closes on {key:menu.close} or SPC and goes one level up on {key:menu.up}. Keys that move
  the focused pane close the menu and make that move in the same press,
  unless a key on that level holds the letter (SPC g, {spc:browse.help},
  {key:browse.go_list}, {key:browse.cell_names}, {key:browse.deferred}, {key:browse.read_group}).
  Toggles and sorts (SPC v, SPC c, SPC s) do not close it — try them, watch
  the state, and leave with {key:menu.close}. That level says so at the bottom right with a
  close hint.
    {spc:browse.grep}    search              {key:browse.filter}    filter             {key:browse.jot}    jot
    {spc:browse.help}    keys                {key:browse.quit}    quit
    {key:browse.pick}  register            {key:browse.unregister}  drop from the list
  Screen — which one stands; the root menu names it, as in [board]:
    {key:browse.go_list}  list                {key:browse.go_board}  board — described below
    {key:browse.go_gantt}  gantt — the list opened out beside a date axis (see below)
    {key:browse.go_stats}  statistics — the numbers `moai stats` gives, drawn (see below)
    {key:browse.go_wiki}  wiki — the project's manual pages, read only (see below)
  View — every toggle except the list columns (SPC c) is here:
    {key:browse.deferred}  deferred         {key:browse.backlog}  backlog        {key:browse.show_all}  show all
    {key:browse.archived}  the archive — done that has sat a while [shown/hidden]; {key:browse.show_all}
             leaves it as it is
    SPC v 1  first column of the config [shown/hidden] — the next ones count up
             done has no letter of its own: the column that holds it does
    {key:browse.detail}  detail pane [shown/hidden]
    {key:browse.worktree}  overlay worktrees [on/off]
    {key:browse.raw}  raw or rendered
  Sorts and columns call priority, created, updated and assignee by the same
  letter — only title ({key:browse.sort_title}) and tag ({key:browse.cell_tags}) split one letter:
    {key:browse.sort_priority}  priority            {key:browse.sort_created}  created            {key:browse.sort_updated}  updated
    {key:browse.sort_column}  column              {key:browse.sort_assignee}  assignee           {key:browse.sort_title}  title
    {key:browse.sort_due}  due (when) — earliest first, no deadline last
    {key:browse.sort_add_priority}, {key:browse.sort_add_created}, {key:browse.sort_add_updated}, {key:browse.sort_add_column}, {key:browse.sort_add_assignee}, {key:browse.sort_add_title}, {key:browse.sort_add_due}  the same
             field added at the end of the order [↑ asc/↓ desc]
    {key:browse.sort_edit}  edit the order in a window — described below
    {key:browse.cell_id}  id                  {key:browse.cell_priority}  priority           {key:browse.cell_assignee}  assignee
    {key:browse.cell_created}  created             {key:browse.cell_updated}  updated            {key:browse.cell_tally}  counts
    {key:browse.cell_tags}  tag                 {key:browse.cell_names}  column names [shown/hidden]
    {key:browse.cell_epic}  epic — the name of the epic the row is in; off to begin with
    {key:browse.cell_branch}  branch mark [shown/hidden] — needs {key:browse.worktree} to overlay first
  Read:
    {key:browse.read_all}  everything unread   {key:browse.read_group}  every member of this group
  Options — how this screen draws what stands, not which rows stand:
    {key:browse.detail_at}  detail pane goes right, bottom, left, top — press again to turn
             it. Whether it stands at all is {key:browse.detail}
    {key:browse.timezone}  the timezone times are written in. It opens a window with the
             names this machine knows. Type to narrow it down and pick one.
             Stored times stay UTC, and so does --json; a bare day or a
             minute in an {key:browse.filter} filter (created=2026-10-01,
             done_at=\"2026-10-03 00:00~2026-10-05 23:59\") is on this clock too
    {key:browse.mouse}  mouse [on/off] — on to begin with, and the choice is kept.
             Clicking puts the focus on the pane and the cursor on the row
             under it. The wheel scrolls the pane under the pointer, and
             neither the focus nor a cursor follows: the list, the detail, the
             statistics window, the wiki window's two panes (a click there
             picks the pane and the page, and a click on a link follows it).
             Dragging the line between the list and the detail resizes them,
             and the share the list takes is kept under [tui] as list_width
             and list_height. The wiki window's line drags the same way and
             keeps a share of its own as wiki_width — until it is dragged it
             splits by list_width. With the SPC menu open, a
             click on an item is its key — a group goes down a level, a
             toggle keeps the menu open, and {key:menu.close} and {key:menu.up} on the bottom line
             close it or go up — and a click or a roll outside the menu closes
             it and does what it does there. While a form, a picker or
             a prompt is up the mouse is the terminal's again — selecting and
             middle-button paste work there as they always did. Over the list
             and the detail, the terminal's own selection and paste need Shift
             held in most terminals, Option in iTerm2
  The one key that quits outright is Ctrl-C — anywhere, even mid-typing.
  The screen rereads itself — issues written next door, and `moai read` or
  `moai project add` in another terminal, land without a keypress.

  The version line in the header says whether a newer release is out. When
  one is, the banner says so and names `moai update`, which upgrades the moai
  you are running in place; quitting prints it again. Where `moai update`
  cannot run but install.sh can (a directory under your home you cannot
  write), quitting prints the install.sh line instead, with --dir when it is
  not ~/.local/bin. No line is shown for a build from source, for a binary not
  named moai, or on a machine the releases do not cover.

  Of what came to me (assigned to me or under it), rows changed since the
  last look carry a [NEW] mark in front of the title. Read marks live in my own
  config and the tracker does not change — on the CLI that is `moai read`.

  The list shows done and hides deferred work to begin with — the mark on
  the path line says so, [deferred hidden] or, with the archive below,
  [deferred·archive 312 hidden]. The view is separate from the filter, so
  Esc does not clear it and the two apply together.
  Done that has sat in done for a while is the archive (archive_days in
  .moai/config.toml, 14 unless written; 0 turns it off). It stays hidden on
  the list and the board even with done shown, and the path line counts it
  — [archive 312 hidden]. {key:browse.archived} shows it and is kept under [tui] as
  show_archived; {key:browse.show_all} (all) leaves it hidden. {key:browse.grep} search finds it anyway.
  Sorting puts urgent, new, earlier column and alphabetical on top. The order
  is a list of fields, like ORDER BY: the next field decides only where the
  ones before it tie. Each lowercase SPC s key makes the order that one field,
  and pressing it again turns it around; an uppercase one adds its field at
  the end, and pressing it again turns that field around. When the order is
  not the default (priority) the path line says which it is, every field with
  ↑ or ↓, as in sort priority↑, updated↓.
  Columns (SPC c) turn on and off with [shown/hidden]. Tag, epic, assignee,
  created and updated dates stand on the right of the row, and when it gets
  narrow they are dropped in that order — dates, then epic, then assignee,
  then tag — to leave room for the title.
  View, sort and columns are written into the [tui] table of the user config
  on every press and carry over to the next run and to other projects (the
  same file `moai project add` writes). The sort is the exception: one picked
  inside a project is kept for that project alone, as order under
  [tui.project.\"<project path>\"], and one picked in 0 is kept as order under
  [tui] — the default for every project without its own. order is a list in
  the words of `moai show --sort` but id, such as
  [\"priority\", \"updated:asc\"], and the older sort and sort_reversed pair
  is still read where order is absent and written beside it from the first
  field, for an older moai.

  {key:browse.sort_edit} opens the sort window over the list. All six fields stand in it:
  the ones in the order first, numbered and marked ↑ asc or ↓ desc, then the
  rest marked with a dot. {key:sorter.down} and {key:sorter.up} move, {key:sorter.lower} and {key:sorter.raise} move the field down or up the
  order, {key:sorter.toggle} puts it in at the end or takes it out (the last one stays), and
  {key:sorter.flip} turns its direction. {key:sorter.apply} applies the order where you stand and keeps it
  there — inside a project for that project, in 0 as the default. {key:sorter.default} applies
  it and keeps it as the default, the order under [tui], even inside a
  project; a project that has an order of its own gets this one too. {key:sorter.close}
  leaves without a change.

  {key:browse.go_board} lays the same rows out as a kanban board instead of a list, and
  {key:browse.go_list} brings the list back. It is the list's layout, not another window:
  the cursor, the filter, the view, search, [NEW] and the detail are the
  list's, and the choice is kept under [tui] as layout. The menu's root names
  the screen that stands, as in +screen [board]. The columns are backlog,
  deferred and the config's columns in order — backlog is a kind and deferred
  an axis, so nothing is stored for them.
  {key:browse.backlog} hides backlog items in both the board and the list, and is kept
  under [tui] as hide_backlog.
  At the project root every milestone is a lane, with (no milestone) last;
  inside a milestone or an epic there is one lane. Epics and milestones are
  not cards. Each card is two lines, its id, column and priority over its title,
  and the cursor's card is marked and drawn reversed. Cards Enter can go
  into end their title with /, as in the list. The foot, a third line,
  stands while SPC c has tag, epic, assignee or a date on — an epic shows up on
  the board only as the name in that foot ({key:browse.cell_epic}). Cards that are not yours
  grow that foot anyway and say whose they are, dimmed — → name, or unowned —
  in the assignee's place; your own cards stay two lines. Whose is decided the
  way `moai ready` decides it, and when who you are is unknown no card is set
  apart. On a narrow card the tags and the epic name shrink, longest first, so
  whose it is and the dates still show. {key:moves.line_down} and {key:moves.line_up} move within the
  column, {key:browse.collapse} and {key:browse.expand} go to the next column at the nearest height, {key:moves.top} and {key:moves.bottom} go to
  the column's ends, {key:browse.enter} and {key:browse.leave} go in and out as in the list, and {key:browse.expand_all}
  does nothing. The whole board scrolls as one; the column names stay on top.
  When the columns do not fit, the column the cursor stands in always stands
  and the frame title names the rest with their counts. Clicking a card puts
  the cursor on it. The wheel, {key:moves.half_down} and {key:moves.half_up}, {key:moves.page_down} and {key:moves.page_up}, and
  PageDown and PageUp scroll the board, not the cursor: the chosen card and
  the detail stay put even once the card is off screen, and the board scrolls
  to its end. The next cursor key moves from that card and scrolls back to
  it. In the overview (0) every project stands as a header
  across the columns, the same line the list gives it, and an unfolded project
  has its milestone lanes under it; the columns are those of every project
  shown, together. The cursor stands on a header too — {key:moves.line_up} from the project's
  top card in a column goes up to it — and there {key:browse.expand} unfolds, {key:browse.collapse} folds, {key:browse.expand_all}
  unfolds everything and {key:browse.enter} goes in, as in the list.

  {key:browse.go_gantt} lays the same rows out as a gantt chart: the list, every group opened
  out, stays fixed on the left and a date axis with bars stands on the right.
  It is the list's layout too — the cursor, the filter, the view, search and
  the detail are the list's — but it holds its own folds: it opens with
  everything unfolded, {key:browse.collapse}, {key:browse.expand} and {key:browse.expand_all} fold and unfold only inside it,
  and the list keeps the folds it had. Coming back to the list opens the
  folder that holds the row you stood on. The detail stands below the gantt at
  first; {key:browse.detail_at} there moves the gantt's detail only, for this session.
  The planned bar (░, ▒ on a group)
  runs from the start to the deadline and ◆ marks the row's own deadline; █
  over it is when the work actually ran, ▓ is the time past a deadline, ┄ is
  a deferred row's plan, and ▼ over a dotted line marks today.
  Groups that wrote no days of their own span their members' days. Planned days
  are calendar days; actual times are shown in your time zone.
  Keys that stand only in the gantt move the date axis:
    {key:gantt.zoom_in} {key:gantt.zoom_out}  zoom in and out — hour, day, week, month
    {key:gantt.left} {key:gantt.right}  one cell earlier and later   {key:gantt.page_left} {key:gantt.page_right}  a page earlier and later
    {key:gantt.today}    today in the middle           {key:gantt.fit}    fit every row on one screen
  It opens fitted, and stays fitted as rows come and go until you move it.
  These keys are under [gantt] in keys.toml and win only inside the gantt: a
  [browse] line that uses one of them still works in the list and the board.
  On a narrow window the rows give up their columns first and then the dates
  are hidden, which the header says; a low window folds the ▼ row away.
  Moving a plan writes the row, the way `moai edit --start --due` does. The
  keys move the cursor row's plan by a day: {key:gantt.nudge_earlier} and {key:gantt.nudge_later} the whole plan,
  {key:gantt.nudge_start_earlier} and {key:gantt.nudge_start_later} only its start.
  With the mouse, drag a bar's middle to move the plan, its left end to move
  the start and its right end to move the deadline; it snaps to whole days and
  is written once, when you let go. Clicking an empty day on a row with no
  planned days plans that one day. Groups whose bar spans their members are
  not moved — move the members. Neither is a row whose copy the worktree
  overlay took from another worktree; the notice names that worktree. When a
  move is refused (a start after the deadline, a day past the deadline above)
  the refusal is on the banner; a move that went through names the
  `moai edit` line that puts it back.
  Clicking a row puts the cursor on it and the wheel scrolls the rows; a
  sideways wheel, or Shift and the wheel, slides the dates, and so does
  dragging the date header. Dragging the line between the rows and the dates
  widens the rows, and that width is kept under [tui] as gantt_label.

  With registered projects (`moai project add`), 0 lists them all — a header
  row per project with that project's rows under it. Started outside a
  `.moai` it begins there; started inside one it begins in that project.
  Coming out through 0 leaves the project you were in unfolded and the rest
  as header rows only — a project is read the moment it unfolds (its header
  spins while it reads).
  {key:browse.enter} on a header row goes into that project, {key:browse.leave} only goes up a
  directory. {key:browse.enter} on a group row under it goes into that project and lands
  there — you never drill down in the one list; drilling down always happens
  inside a project.
  View, sort and columns apply to every unfolded project while search and
  filter apply inside one project only — jotting ({key:browse.jot}) and read ({key:browse.read}) go to
  the project of the row under the cursor. With nothing registered and
  started outside, an empty list stands and says to add the first project
  with {key:browse.pick} (`--json` gives an empty `projects` array and exits 0).

  {key:browse.pick} opens a window to pick a directory and register it as a project —
  it walks in and out one level at a time ({key:picker.enter} and {key:picker.up}, moving with
  the same {key:moves.line_down}, {key:moves.line_up}, {key:moves.top} and {key:moves.bottom} as the list), and directories with a `.moai` and those
  already registered are marked. Inside the window, {key:picker.register} registers the directory
  under the cursor, `{key:picker.hidden}` shows or hides dotted directories, and {key:picker.path} opens a
  field to type a path (Enter goes, Esc gives up). The window closes on {key:picker.close}.
  Any subdirectory of a monorepo stands on its own exactly as picked. It is
  taken even without a `.moai`. The window opens inside a project too, so the
  first project can be added with nothing registered.
  On a header row, {key:browse.unregister} asks once and then only drops it from the list —
  {key:confirm.yes} is yes and any other key gives up. The directory and its `.moai` stay.
  It writes where `moai project add|rm` writes.

  {key:browse.go_stats} opens the statistics window in place of the list and the detail —
  the numbers `moai stats` gives, drawn as bars: the flow per week, the
  columns and priorities, lead and cycle time, and AI work by model. It
  counts the project you are in, or on the one list (0) the project of the
  row under the cursor. With a filter hung ({key:browse.filter} or {key:browse.grep}) it counts only what
  passes, and the title says so. {key:stats.bucket} switches the flow between weeks and
  days; {key:moves.line_down} and {key:moves.line_up}, {key:moves.half_down} and {key:moves.half_up}, {key:moves.page_down} and {key:moves.page_up}, {key:moves.top} and {key:moves.bottom} scroll it;
  {key:stats.close} goes back to where you were, with the cursor, the filter and the
  detail as they were. SPC opens the menu over it with the screens alone:
  {key:browse.go_list} and {key:browse.go_board} close it onto the list or the board, and {key:browse.go_stats}
  counts again from scratch — the window is opened fresh every time and
  nothing of it is kept. On a narrow screen it shows the same figures as
  text.

  {key:browse.go_wiki} opens the wiki window in place of the list and the detail — the
  project's manual, the markdown pages `moai wiki ls` lists (under docs/
  unless wiki_dir in .moai/config.toml says otherwise). It reads the project
  you are in, or on the one list (0) the project of the row under the
  cursor, and it only reads: pages are files you edit and commit. The pages
  stand on the left, the home page (README or index) first, and the right
  pane shows the page under the cursor. {key:moves.line_down} and {key:moves.line_up}, {key:moves.half_down} and {key:moves.half_up}, {key:moves.page_down}
  and {key:moves.page_up}, {key:moves.top} and {key:moves.bottom} move the focused pane — on the list, {key:moves.half_down}, {key:moves.half_up},
  {key:moves.page_down}, {key:moves.page_up} and the wheel scroll it and leave the cursor and the page
  where they are, as in the explorer's list. {key:wiki.enter} on the list goes to the
  page and {key:wiki.focus_next} goes back and forth. {key:wiki.next_link} picks the links drawn on the
  page in turn, from the first one on screen and round from the last to the
  first, and {key:wiki.prev_link} goes the other way from the last one on screen: the
  picked link shows reversed, the key bar names where it goes, {key:wiki.enter} follows
  it, and once it leaves the pane it is dropped. Clicking a link follows it
  at once. On the page with no link picked, {key:wiki.enter} opens
  a list of its links and the issue ids it names, then the pages linking to
  it (marked ←), which moves with {key:moves.line_down}, {key:moves.line_up}, {key:moves.top}, {key:moves.bottom}, {key:moves.half_down}, {key:moves.half_up}, {key:moves.page_down} and
  {key:moves.page_up}, takes one with {key:links.enter} and closes on {key:links.close}. Taking a page link or a
  page linking here goes there and {key:wiki.back} comes back the way you came. Links
  to a heading (page.md#anchor) open at that heading, its line marked ▸
  until you scroll; one to a heading the page does not have is marked and
  opens the page at its top. Taking an id closes
  the window onto that row (from the one list, inside that project). Links
  that lead nowhere, addresses outside the wiki and ids the tracker does not
  hold are marked and only say so. The page as written ({key:browse.raw}) marks no
  link — {key:wiki.next_link} says so. {key:wiki.search} searches the titles and bodies of the pages and
  narrows the list as you type, and {key:wiki.close} drops a picked link, then clears
  that search, and then closes the window, with the cursor, the filter and
  the detail as they were. SPC opens the menu over it with the screens and
  {key:browse.raw} (raw or rendered). The window is read fresh every time and nothing
  of it is kept.

  {key:browse.jot} opens the jot form anywhere inside a project. It keeps a backlog item
  with no epic. If an editor is there ($VISUAL, $EDITOR, or vi or nano on
  PATH) it opens like a git commit message: the first line is the title, then
  a blank line, then the body, and comment lines are guidance to be deleted.
  Leave the title empty, or end the editor with an error, and nothing is
  kept. With no editor the built-in form opens — one title line and a body,
  Tab moves between them (Enter in the title goes to the body) and Ctrl-S
  keeps it. Esc closes it, asking once if you had typed something ({key:confirm.yes} throws
  it away). Besides moving a plan on the gantt (above), this is the only
  write — moving, editing and noting are done on the CLI.
  `--json` prints only that directory's listing, with no screen (outside a
  `.moai`, the rows of the layer). Each folder row (a milestone, an epic,
  an issue with children) carries progress, {\"done\": 3, \"total\": 5} —
  what the counts column shows as 3/5, read from every row under it whatever
  the filters keep. The filters of `moai show` (-s, -t, -e, -g, --filter,
  --all, --archived, --deferred and the rest; the time filters read as in
  `moai show --help`) shape that listing: given any, a row stands when
  `moai show` with the same filters would list it, and a folder stands too
  when something under it does — so done is hidden unless asked for, as in
  `moai show`. Without them the listing is every row, as before. --sort and
  --reverse order it in the words of `moai show --sort`, and folders still come
  first and the baskets last. These shape --json only, and only inside a
  `.moai`. --deep (with --json, inside a `.moai`) lists every row under the
  place at once, in the order of the explorer's unfolded tree: a folder row,
  then what is in it, with the same filters and order as the one-level
  listing. Each row carries home, the --path handles from the root down to the
  directory it stands in ([] at the root, [none] or [lost] inside a basket).
  --no-archive leaves out the rows that moai show --all hides as archived, and
  a folder stays when a live row stands under it. The archive is part of the
  listing otherwise.

    moai tui --path <milestone> -g parser --all --json
    moai tui --sort updated --json
    moai tui --deep --json

  `--path` takes an issue id, or one of the two baskets by the word this
  tool uses for them: none (no milestone) and lost.")]
    Tui(TuiArgs),

    // **트래커를 읽는 서버다**(moai-0aa7) — 쓰는 것은 간트 막대의 계획 날 하나다(자식 `moai edit`, moai-d86e.3ud).
    // 데이터는 요청마다 띄우는 자식 `moai --json …` 의 stdout 이라 서버가 짓는 뜻이 없다 — 받고 옮기는 길은
    // `crate::web`, 묶고 첫 줄을 내는 길은 `cmd::web` 이다.
    /// Serve this tracker as a page on 127.0.0.1
    #[command(after_help = "  moai web                  http://127.0.0.1:6624/ - open it in a browser
  moai web --port 0         let the system pick a free port
  moai web --dry-run        say where it would serve; bind nothing
  moai web --port 0 --json  one line {url, host, port, root, here}, then serve

  The first line names the address and the tracker. Ctrl-C stops it; there
  is nothing to clean up. It never opens a browser.

  Each list, issue and wiki page it serves is the answer of `moai --json
  <command>`, run in a child process for that request, so the page sees
  exactly what the CLI says. Writing stays with the CLI and the explorer; the
  page writes two things, taken only from the page itself: a Gantt bar dragged
  to new days, run as `moai edit --shift`, and your read marks (`moai
  read`). Registered projects
  (`moai project ls`) can be read too, by `?project=<path>`.

  --port is 6624 unless given. --host is 127.0.0.1 unless given; another
  address than a loopback one is served with a warning, because anyone who
  reaches that port can read the tracker and move its planned days. Whatever
  it binds, it answers only requests whose Host is localhost or an IP address.

  A port already in use stops it with the code port_in_use. Run inside a
  linked worktree, it serves the main checkout's tracker, as other commands
  read it. --json prints the first line as one JSON value and nothing more
  on stdout; --dry-run adds dry_run, and with --port 0 there is no url.")]
    Web(WebArgs),

    /// Called by an agent's hook. Reads an event on stdin
    #[command(after_help = "  Nobody calls this by hand. The hooks `moai skill install` plants call it -
  Claude's plugin, Codex's .codex/hooks.json, Antigravity's .agents/hooks.json.

  **It blocks nothing, and whatever goes wrong the exit code is 0.** A hook
  that spits errors makes every session start noisy, and then people turn the
  hook off - a rule that is off is no rule.

  The directory (cwd) and the session id come from stdin. They are not in the
  environment.

  Events:
    session-start       Writes the baseline. Loads what is held after a compact
    user-prompt-submit  A person asked. Loads the board once
    pre-tool-use        Just before a tool call. The rules stand here
    stop                The turn ends. Checks the state

  --dialect says which agent's shapes come in and go out: claude (the
  default), codex or antigravity. The rules are the same for all three.
  Antigravity has no user-prompt-submit; its first model call of a turn
  (PreInvocation) stands in for it.

  echo '{\"session_id\":\"x\",\"cwd\":\"/repo\"}' | moai hook user-prompt-submit")]
    Hook {
        // 값은 글로 적는다 — clap 이 붙이는 `[possible values: …]` 가 `-h` 에서 105칸이 됐다
        // (moai-h0r2). 목록의 글은 `hook::Event` 의 doc 주석과 같다 — 시험이 둘을 견준다.
        /// Which place it was called from (see the list below)
        #[arg(value_name = "event", hide_possible_values = true)]
        event: crate::hook::Event,

        /// Whose shapes: claude (default), codex, antigravity
        #[arg(
            long,
            value_name = "agent",
            hide_possible_values = true,
            default_value = "claude",
            hide_default_value = true
        )]
        dialect: Dialect,
    },

    // **이 첫 줄은 `install.sh`·`install.ps1` 이 읽는 계약이다**(moai-8rmw, 리뷰). 깔린 moai 가 이 moai 인지를
    // `merge-driver --help` 가 이 글을 내는지로 가르고(`ours`), 나간 판마다 이 글이 박혀 있다.
    // 고치려면 `install.sh` 가 옛 글과 새 글을 다 받게 먼저 고친다 — 안 그러면 새 글을 단 판부터
    // 올리기가 모두 "이 moai 가 아니다" 로 거절된다. 진짜 바이너리로 재는 시험이
    // `the_real_binary_is_recognised_as_this_moai` 다.
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
  trace. A line this binary cannot read travels through byte for byte - paired
  by its id when it has one, so only a row both sides changed is handed over,
  and counted line by line when it has none. What still hands the whole file
  over inside conflict markers is a duplicated id among readable rows, and a
  file that is not text - keeping only what parsed would lose the rest.

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

    /// Plant skills for Claude, Codex, Antigravity (safe to run again)
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

    /// Raise priorities a deadline has caught up with
    #[command(after_help = "  moai due apply             raise what its deadline calls for, note each one
  moai due apply --dry-run   show what it would raise, write nothing

  A row follows the earliest deadline among its own and those of the rows it
  is drawn under - its milestone, its epic, its parent (a parent already
  closed lends none). By days left (today and a past deadline count as 0 or
  less):
    due_p0_days  = 1      at most this many days left - p0
    due_p1_days  = 2      at most this many - p1
    due_p2_days  = 7      at most this many - p2; more days left, untouched
  Only issues and backlog items that are open and in the plan are raised -
  never a group row, never a deferred one, never a row the write would refuse
  (a hand-edited date: fix it with `moai edit <id> --due`). It only raises: a
  priority that is already that urgent stays, and nothing is ever lowered back.

  Each raise leaves a note in the same write, which keeps the old value:
    Due-priority: p2 → p0 (1 days left, due 2026-10-17 from <epic>)
  With nothing to raise it writes nothing and asks nobody who you are.
  `due_priority = \"off\"` in .moai/config.toml stops it from changing
  anything from then on; what it raised before stays. `moai status` says
  when rows have fallen behind their deadline.

  The day is the reader's day - the timezone the board uses. --json gives
  {\"due_priority\",\"dry_run\",\"raised\":[{\"id\",\"title\",\"was\",\"to\",
  \"days_left\",\"due_on\",\"due_from\"}]} - due_from only when the deadline is
  an ancestor's.")]
    #[command(subcommand)]
    Due(DueCmd),

    /// Read this repository's settings, defaults filled in
    #[command(after_help = "  moai config           every setting, as you would write it in the file
  moai config --json    one object - the keys of .moai/config.toml

  Reads .moai/config.toml and fills in what it leaves out with the values moai
  uses: prefix, statuses (in board order), naming, archive_days, wiki_dir, the
  status_* thresholds and the due_* rule that `moai due apply` reads. These
  are the values this binary acts on, so a surface drawing the board reads
  them here instead of parsing the file again. --json adds path, the file it
  read. A [models] table is left to `moai models`, which lays it under your
  user config.

  It only reads; edit the file by hand to change a setting.")]
    Config,

    /// Read the model per role from your config and the repository's
    #[command(after_help = "  moai models           the model per role, where each value came from
  moai models --json    one object for skills, including written keys and paths

  Reads [models] in your user config. No .moai is needed. The place is
  MOAI_CONFIG, then $XDG_CONFIG_HOME/moai/config.toml, then
  ~/.config/moai/config.toml. Missing files use the built-in defaults.

  Inside a repository, a [models] table in .moai/config.toml recommends values
  for that repository: it overrides the defaults, and your user config
  overrides it. A line says repo when its value came from there, default when
  nothing set it. A broken .moai/config.toml leaves the table as your user
  config gives it.

  Model names are strings. review_grade, effort and effort_complex accept
  low, medium, high, xhigh or max. Invalid values fall back per key and warn;
  unknown keys warn. written lists the valid, known keys from your user config,
  repo the ones the repository table set that your config did not, and
  repo_path that file (null when it set none). The level lines name the roles
  they pair with. This command only reads; edit the files by hand.")]
    Models,

    /// Show the explorer's keys and what your keys.toml changed
    #[command(after_help = "  moai keys               every key the explorer runs on, changed ones marked
  moai keys --template    a keys.toml with every action, commented out
  moai keys --json        one object: the file, the dropped lines, every action

  The keys of `moai tui` live in their own file next to your user config:
  MOAI_KEYS, else keys.toml beside the config (MOAI_CONFIG, then
  $XDG_CONFIG_HOME/moai/config.toml, then ~/.config/moai/config.toml). It is
  read once when the explorer starts, when help is printed and by this
  command; no other command opens it.

    [browse]
    quit = \"SPC x\"            one key sequence
    grep = [\"/\", \"SPC /\"]     several: the first is the name shown
    jot  = \"none\"             unbind it; [] does the same
    [moves]
    line_down = \"n\"           j, k, gg and the rest move every window at once

  Write only what you change. A line replaces every key of that action, the
  hidden ones too (the arrows) — list them again to keep them. Key names are
  spelt as the explorer shows them: SPC, Enter, Esc, Tab, Shift-Tab, Bksp, Up,
  Down, Home, End, PageUp, PageDown, F1, Ctrl-x, Alt-x, and a capital letter
  for Shift; a sequence is names apart by spaces (SPC v d), or a doubled letter
  (gg).
  A line that is wrong is dropped alone and that action keeps its default:
  an unknown section or action, a name that is not a key, or a key that
  clashes with another or waits for a longer one. The explorer says so in one
  line and still opens; this command lists them on stderr and exits 0. A file
  that is not TOML is dropped whole. Ctrl-C, the digits 0-9, SPC v 1-9, SPC
  itself and keys inside text fields cannot be changed, and a window's close
  key can move but not be set to none.

  --template prints a starter file; save it yourself, nothing is written:
    moai keys --template > ~/.config/moai/keys.toml")]
    Keys(KeysArgs),

    // **받고 돌리는 길은 `cmd::update`, 고르는 길은 `update` 다**(moai-zsfr.2p9). 저장소의 `.moai` 를 안
    // 찾는다 — 올리는 것은 이 바이너리지 트래커가 아니다.
    /// Upgrade the moai you are running with install.sh
    #[command(after_help = "  moai update                    the latest release, over this binary
  moai update --version v0.9.0   that release instead
  moai update --dry-run          print what would be fetched and run

  It fetches install.sh from the main branch of the repository releases come
  from and runs it with sh, installing into the directory of the binary that is
  running, with --force:

    MOAI_REPO=<owner/name> sh -s -- --dir <that directory> --force

  install.sh's output streams through, and the exit code is sh's. The script
  is fetched by moai itself, not by curl; install.sh then downloads the
  release with curl or wget and checks it against the release checksums.

  On Windows it fetches install.ps1 instead, writes it to a temporary file and
  runs it with Windows PowerShell, removing the file afterwards:

    powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass
      -File <that file> -Dir <that directory> -Force

  The running moai.exe cannot be overwritten, so install.ps1 renames it to
  moai.exe.old and puts the new one in its place; the next install removes
  that .old.

  The repository is `buzzni/moai` unless your user config says otherwise -
  `repo = \"owner/name\"` under [update] - and MOAI_REPO wins over both. The
  repository's own .moai/config.toml is never read for it: what it names is
  piped into a shell, and a repository you cloned must not choose that. A
  value that is not owner/name is refused, not replaced with the default.

  Nothing is fetched where it cannot work, and it stops with one line:
  a binary cargo built (target/ or cargo install - rebuild from source
  instead), a binary not named moai, a directory you cannot write (run it as a
  user who can, or reinstall with install.sh --dir), and a machine the releases
  do not cover. A directory outside your home is fine when you can write it.
  On Windows the binary has to be named moai.exe.

  --dry-run fetches nothing and runs nothing. --json prints {repo, script,
  dir, command, dry_run} and, after a run, `code` - the shell's exit code; the
  script's own output then goes to stderr so stdout stays one JSON value.")]
    Update(UpdateArgs),

    /// Put a .moai/ into this repository (safe to run again)
    #[command(after_help = "  Run again where it is already installed and only the attached files
  (.gitattributes, .gitignore, AGENTS.md) are brought back in line. Issues
  and the journal are not touched.

  The prefix is decided once - every id already issued carries it.

  **In a terminal the first init asks.** It shows the prefix and each choice
  with its default picked, and Enter plants. A flag picks its row and locks
  it; give every row a flag and nothing is asked. --yes asks nothing and
  uses the old defaults - committed, with the guide block - for choices that
  neither a flag nor existing git rules and guide files settle. Nothing is
  asked where a script or an agent calls it - stdin or stdout is not a
  terminal, TERM=dumb, or --json - and there init plants the same as --yes.
  Running it again where .moai already stands never asks. Esc stops with nothing
  written. Even a first run reads existing git ignore rules and the moai
  guide block. A later run also recognizes installed moai hooks when no block
  stands. If git fails, init refuses rather than guessing commit mode.

  **Optional skills** (today moai-tmux, moai-cmux and moai-saycode, the
  supervisor's hands on tmux panes, cmux tabs and Saycode sessions) get a
  row each under hooks · skills, shown while that row installs. The screen
  checks one when it is already planted here, or when your shell says you
  use it ($TMUX set for moai-tmux, unless it is cmux's /tmp/cmux-… stand-in;
  $CMUX_SURFACE_ID for moai-cmux; SAYCODE_AGENT_ENV for moai-saycode); Enter
  passes the checked ones on to `moai skill install --with`, and unchecking
  one already planted passes --without. --with <skill> (repeatable, or
  names with a comma) checks and locks that row and turns installing on; it
  cannot go with --no-skill.
  Where nothing is asked, only what --with names is planted - nothing by
  what your shell says - and optional skills already planted are refreshed.

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
  decision and init leaves it alone. An explicit --driver with --tracking
  exclude or gitignore is refused.

  --json reports gitignore=true only when .gitignore was written; exclude=true
  means the ignore lines were written to .git/info/exclude. Its driver is
  planted, current (already the same line), off (the repository declares no
  driver, or is not a git repository), failed (driver_trouble says why),
  skipped (--no-driver, this run only) or untracked (a tracker kept out of
  git - git never merges it, so there is nothing to drive).

  --check writes nothing and only answers whether the AGENTS.md block is
  current, stale or missing, and where the merge driver stands. It is
  non-zero when a file cannot be read or git cannot determine tracking.

  --print only prints that block. That is where to copy it from when the file
  the agent reads is not AGENTS.md - --print and init write the same text.")]
    Init {
        /// id prefix (up to 8). Made from the directory name when absent
        prefix: Option<String>,
        /// Leave AGENTS.md alone (same as --guide none)
        #[arg(long, conflicts_with = "guide")]
        no_agents: bool,
        /// block, file (.moai/guide.md + link), hook or none
        #[arg(long, value_name = "how", hide_possible_values = true)]
        guide: Option<crate::init_choice::Guide>,
        /// Plant the merge driver in .git/config (the default)
        #[arg(long, conflicts_with = "no_driver")]
        driver: bool,
        /// Leave .git/config alone (plant no merge driver)
        #[arg(long)]
        no_driver: bool,
        /// Git tracks it (commit) or not (exclude, gitignore)
        #[arg(long, value_name = "how", hide_possible_values = true)]
        tracking: Option<crate::init_choice::Tracking>,
        /// Then run moai skill install --scope local
        #[arg(long, conflicts_with = "no_skill")]
        skill: bool,
        /// Do not install the hooks and skills
        #[arg(long)]
        no_skill: bool,
        // `skill install --with` 으로 그대로 건넨다(moai-3r7l.ocn) — 설치를 켜고 그 칸을 잠근다. 끄는 짝은 없다.
        /// Plant this optional skill too (repeatable)
        #[arg(long = "with", value_name = "skill", value_delimiter = ',', conflicts_with = "no_skill")]
        with: Vec<String>,
        /// Then add this repository to your project list
        #[arg(long, conflicts_with = "no_register")]
        register: bool,
        /// Do not add it to your project list
        #[arg(long)]
        no_register: bool,
        /// Ask nothing; unset rows plant as init always did
        #[arg(short = 'y', long)]
        yes: bool,
        /// Write nothing; say if the AGENTS.md block is stale
        #[arg(long, conflicts_with_all = ["prefix", "no_agents", "driver", "no_driver", "tracking", "guide", "skill", "no_skill", "with", "register", "no_register", "yes"])]
        check: bool,
        /// Write nothing; print that block (to paste it)
        #[arg(long, conflicts_with_all = ["prefix", "no_agents", "driver", "no_driver", "tracking", "guide", "skill", "no_skill", "with", "register", "no_register", "yes", "check"])]
        print: bool,
    },
}

/// 기한이 우선순위를 올린다(moai-dr8a.1q4). 동사는 하나지만 이름 밑에 두어, 읽는 동사가 생기면 같은 자리에 선다.
#[derive(Subcommand, Debug)]
pub enum DueCmd {
    /// Raise priorities their deadline calls for, with a note on each
    Apply {
        /// Show what would be raised; write nothing
        #[arg(long)]
        dry_run: bool,
    },
}

/// 위키를 읽는다(moai-ihu4). **페이지를 고치는 동사는 없다** — 페이지는 파일로 직접 쓰고 git 이 합친다. `put` 을
/// 두면 `store::with_write` 곁에 둘째 쓰기 경로가 선다(설계 노트 moai-qunn).
///
/// **`learn` 은 그 결정의 예외가 아니다**(moai-4s5n.n7j). 그것은 교훈 페이지를 **새로 짓기만** 한다 — 파일을
/// `create_new` 로 열어, 있으면 거절하고 덮어쓰지도 고치지도 않는다. 고칠 페이지는 여전히 파일로 고치고 git 이
/// 합친다. 함께 다는 `Lesson:` 노트는 트래커의 쓰기라 `store::with_write` 하나를 지난다 — 쓰기 경로는 둘이 되지
/// 않는다.
#[derive(Subcommand, Debug)]
pub enum WikiCmd {
    /// Every page - slug, title, and what does not resolve
    #[command(after_help = "  A page that cannot be read stays in the list with its file name as the
  title and says why - over 1 MB, a link out of the checkout, not a regular
  file, not UTF-8. Over 1 MB is left unread on purpose and the exit code
  stays 0; any other page left unread, or a name or directory the walk had
  to leave out (a line on stderr names it), makes it non-zero once the whole
  list is out. No `wiki_dir` directory yet is not an error: there is no
  wiki to list and the exit code is 0. A `wiki_dir` that is absolute, has
  `..`, leads out of the checkout or into `.git/`, or is not a directory is.

  Under the list, notices count pages with conflict markers, links leading to
  no page, links to a heading the page does not have, ids naming no issue,
  and `Lesson:` notes on open work naming a page this checkout does not have.
  They block nothing and leave the exit code alone. A lesson page
  (`lessons/<slug>`) shows `×N`, the number of different rows whose notes name
  it.

  --json gives {\"dir\",\"dir_abs\",\"pages\":[{\"slug\",\"title\",\"path\",
  \"bytes\",\"issues\",\"links\",\"linked_from\",\"conflict\",\"seen_count\"}]}.
  `dir_abs` is `dir` as an absolute path in this checkout - absent when
  `wiki_dir` points outside it or that path is not UTF-8. `seen_count` is that
  `×N` - always there, 0 on a page that is not a lesson. `issues` is
  [{\"id\",\"exists\"}] - the ids with this tracker's prefix the page names
  outside code blocks; the name of a skill moai plants (`moai-wiki`) is never
  one. `links` is
  [{\"text\",\"to\",\"anchor\",\"resolved\",\"anchor_resolved\"}] - relative links to a
  `.md` page of this wiki, `to` being the target slug (the page itself for a
  bare `#anchor`). `anchor` is the part after `#` and `anchor_resolved` says
  whether that page has the heading; both are absent on a link with no `#`,
  and `anchor_resolved` is absent when the page could not be read.
  `linked_from` is the slugs of the pages linking to this one.
  A page that could not be read carries `error` ({\"kind\",\"said\"}, kind
  too_large, refused or failed); absent, it was read whole. What the walk
  left out stands in `skipped` ([{\"path\",\"kind\",\"said\"}], kind dir_link,
  not_utf8 or unreadable); absent, nothing was left out. When the directory
  itself cannot be used, `pages` gives way to `error` with kind no_dir,
  outside, not_a_dir or failed.")]
    Ls,
    /// One page, drawn (`--json` gives the raw body)
    #[command(after_help = "  The slug is what `moai wiki ls` prints - matched letter for letter, so
  `Guide` is not `guide`.

  --json gives the page as `ls` does, with `seen` in place of `seen_count`,
  plus `body`, the file as written. `seen` is always there:
  [{\"id\",\"at\",\"by\",\"by_email\"}], the rows whose `Lesson:` notes name this
  lesson, latest note first ([] on a page that is not a lesson).
  A page that could not be read has no `body`, carries `error`, and the exit
  code is non-zero. When the wiki directory itself cannot be used - not there
  yet included - it gives {\"dir\",\"dir_abs\",\"error\"} as `ls` does, non-zero.

  `linked_from` counts only the pages that could be read. When another page
  could not be read, or the walk left a name or directory out, the count may
  be short: `--json` adds `linked_from_partial` (true) and a line under the
  page says how many places were not read. With every page read, neither
  stands. It leaves the exit code alone - `moai wiki ls` names those places.")]
    Show {
        /// The page - its path under the wiki directory without `.md`
        #[arg(value_name = "slug")]
        slug: String,
    },
    /// Find the sections that match some words, best first
    #[command(after_help = "  moai wiki search merge driver          several words, one query
  moai wiki search '병합 드라이버' -n 5   -n/--limit, 10 unless given
  moai wiki search merge --json

  Every heading, at any level, starts a section; the text before the first
  heading is one too. Words are lowercased and split at anything that is not a
  letter, digit or `_`, and Korean, Chinese and Japanese text also matches two
  letters at a time - such a word counts as matched where half its two-letter
  pieces are; there is no stemming. A title or heading counts more than the
  body, and a section matching more of the distinct words comes first. A
  screen line is `path:line  Page title › Heading`, with the best line of the
  section under it - the matched words in bold, which is decoration only. The
  last line says how many of how many are shown and the command that opens a
  page.

  Nothing matching is not an error: the exit code is 0, as it is when the wiki
  directory is not there yet. A page that could not be read makes it non-zero
  once everything found is out, as `moai wiki ls` does.

  --json gives {\"dir\",\"dir_abs\",\"query\",\"terms\",\"hits\":[{\"slug\",\"path\",\"title\",
  \"heading\",\"anchor\",\"line\",\"score\",\"snippet\",\"snippet_line\"}],
  \"total\"}. `hits` is always there, `[]` when nothing matched; `total` counts
  the matches before `--limit` cut the list. `line` and `snippet_line` count
  from 1, the line to open in an editor. `heading` and `anchor` are absent on
  the section before a page's first heading; `snippet` and `snippet_line` are
  absent when the page could not be read. `score` shows the ranking, not a
  contract - trust the order, not the values, and never compare them across
  queries. A page whose text could not be read is matched by its title alone
  and named in `unread`, [{\"slug\",\"path\",\"error\"}] with `error` as `ls`
  gives it; a place the walk left out is in `skipped`, as in `ls`. Each key
  stands only when it has something. When the wiki directory cannot be used
  it gives {\"dir\",\"dir_abs\",\"error\"} as `ls` does.")]
    Search {
        /// What to look for - several words are one query
        #[arg(value_name = "words", required = true, num_args = 1..)]
        words: Vec<String>,
        /// How many sections to show, best first
        #[arg(short = 'n', long, default_value_t = 10)]
        limit: usize,
    },
    /// Write a new lesson page and note it on the issue it came from
    #[command(after_help = "moai wiki learn 'End a check on the step that must pass' -b - <<'LESSON'
## What happened
...
## Why
...
## Do this instead
...
LESSON

  It writes a new page, <wiki_dir>/lessons/<slug>.md - `# <title>`, the body as
  given, and `Learned in: <id>` - and, in the same call, a note on that issue:
  `Lesson: lessons/<slug>`. The body needs a `## Do this instead` heading and
  does not open with a `# ` title of its own: the title is the argument.
  `## What happened` and `## Why` are worth writing too.

  --slug names the file, in lowercase ASCII kebab case. An ASCII title makes
  one by itself; any other title needs --slug. --issue is the issue the lesson
  came out of. Without it, the one row you have picked up is used - with none
  or several picked up, it is refused and asks for --issue. --issue none writes
  the page alone, and then who you are is not asked.

  It only creates: a page already standing at that path is refused and left
  as it is. If the lesson is the same, note it again - `moai note <id>
  'Lesson: lessons/<slug>'`; if the page is wrong, edit the file. The page is
  written in this checkout - in a linked worktree, in that worktree, to ride
  the branch - while the note goes to the main checkout's tracker. Nothing is
  committed: the last lines say how. When the note cannot be written the page
  stays, the exit code is non-zero, and the line to note it again is given.
  The wiki directory itself is never made - a missing one is refused.

  --dry-run prints the path and the text and writes nothing. --json gives
  {\"path\",\"slug\",\"issue\",\"noted\"}: `path` from the checkout, `slug` as
  `moai wiki show` takes it, `issue` absent under --issue none, and `noted`
  true once the note is written. --dry-run adds \"dry_run\":true and `text`.")]
    Learn(LearnArgs),
}

/// `moai wiki learn` 의 인자(moai-4s5n.n7j).
#[derive(Args, Debug)]
pub struct LearnArgs {
    /// The lesson in one line, the page's title. A lone `-x` needs `--`
    #[arg(value_name = "lesson", allow_hyphen_values = true)]
    pub title: String,
    /// The file name, in lowercase ASCII kebab case
    #[arg(long, value_name = "kebab")]
    pub slug: Option<String>,
    /// The page body, markdown. `-` reads stdin
    #[arg(short = 'b', long, value_name = "text")]
    pub body: Option<String>,
    /// The issue it came from; `none` for the page alone
    #[arg(long, value_name = "id")]
    pub issue: Option<String>,
    /// Print the path and the text; write nothing
    #[arg(long)]
    pub dry_run: bool,
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
    #[command(alias = "ls", after_help = SHOW_LIST)]
    Show(ShowArgs),
}

/// backlog 만 갖는 동사가 하나 있다 — 펼치기. 그래서 `Typed` 를 그대로 쓰지
/// 못하고, `Typed` 에 넣으면 `moai epic promote` 가 생긴다.
///
/// **공통 동사는 베끼지 않고 [`Typed`] 를 접어 넣는다**(moai-g33x). 손으로 옮겨 적었을
/// 때 `#[command(alias = "ls")]` 가 두 곳에 서고, `cmd/mod.rs` 가 `typed()` 를 안 지나고
/// 같은 두 줄을 다시 적었다 — `Typed` 에 동사를 더하는 날 `moai backlog` 만 조용히 안 따라오고
/// 컴파일 오류도 안 났다. 접어 넣으면 `moai backlog <동사>` 의 목록이 `Typed` 하나에서 나온다.
#[derive(Subcommand, Debug)]
pub enum BacklogCmd {
    /// `moai backlog add` and `moai backlog show` (`ls`) - the same verbs, kind pinned
    // **`Box` 에 담는다**(moai-efoc) — `ShowArgs` 가 차례·쪽 플래그로 자라 `Typed` 가 `Promote` 의
    // 세 배를 넘었고, clippy 의 `large_enum_variant` 가 그 차이를 잡는다. 실행마다 한 번 짓는 값이라
    // `Box` 하나의 비용은 없는 것과 같다.
    #[command(flatten)]
    Common(Box<Typed>),
    /// Unfold into one epic and several issues, and close that thought
    #[command(after_help = "  The markdown it takes is the same shape as `add --from`. With two shapes,
  you get the grammar wrong every single time.

moai backlog promote <id> --from - <<'PLAN'
# Epic title
- [p1] first issue #enhancement
- [p2] second issue
PLAN

  **A line in the plan becomes the issue title as it is.** A long backlog title
  carries its length over to the issue, so write a short title again when
  unfolding. The original text stays on that backlog and the history leads back.

  Unfolding closes it - that backlog goes to `done`. What came from what is kept
  in the journal (the history in `moai show <id>`).

  With the epic already standing, `-e <epic>` unfolds into it as members.
  That is where you take back something the epic needs that had gone out as
  a backlog item - the plan then holds `- issue` lines only.

moai backlog promote <id> -e <epic> --from - <<'PLAN'
- [p1] what the epic set out to do
PLAN

  `--dry-run` is where a person looks at the unfolded plan once and says yes.")]
    Promote(PromoteArgs),
}

#[derive(Args, Debug)]
pub struct PromoteArgs {
    /// The backlog to unfold
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
    /// One line. Wrap it in quotes. A lone `-x` needs `--` in front
    #[arg(value_name = "title", allow_hyphen_values = true)]
    pub title: Option<String>,

    /// Put it in this epic
    #[arg(short, long, value_name = "id")]
    pub epic: Option<String>,

    /// Put it in this milestone (with `--from`, on the epic it creates)
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

    /// Text, not a path. `-` reads stdin (with `--from`, on the first epic)
    ///
    /// **A file goes in as `--body - < <file>`.** `--body <path>` takes
    /// the path as the body, it does not open the file.
    #[arg(short, long, value_name = "text", verbatim_doc_comment)]
    pub body: Option<String>,

    /// Assignee. The creator when absent; `none` clears it, `me` is you
    #[arg(short, long, value_name = "who|none")]
    pub assignee: Option<String>,

    /// Start, `YYYY-MM-DD` or `+N` days from today. Any row
    #[arg(long, value_name = "date")]
    pub start: Option<String>,

    /// Deadline, `YYYY-MM-DD` or `+N` days from today. Not past a parent
    #[arg(long, value_name = "date")]
    pub due: Option<String>,

    // 설명이 없으면 `next_line_help` 가 공백만 든 줄을 그린다(리뷰 moai-5yq0).
    /// What kind to create (issue when absent, epic under `epic add`)
    #[arg(long = "type", value_name = "issue|epic|milestone|backlog")]
    pub kind: Option<Kind>,

    /// Create it as a child of this issue (the id gets a `.xxx`)
    #[arg(long, value_name = "id")]
    pub parent: Option<String>,

    // **못 지키는 깃발의 거절은 `clap` 이 아니라 [`crate::cmd::add::run`] 이 낸다**(moai-yhb1).
    // 여기 `conflicts_with_all` 로 달아 두던 자리다. 그것은 `add::run` 이 닿기도 전에 터져
    // 네임스페이스가 지은 거절문을 가렸다 — `moai backlog add --from - -b '글'` 이 "`--body` 를
    // 빼라" 를 듣고, 그것을 빼고 다시 친 뒤에야 동사가 틀렸다는 것을 알았다. 게다가 clap 의 글은
    // 평문 stderr 에 exit 2 라, 같은 명령의 형제 거절(`--milestone <모양 틀림>` 은 `code::ERROR`
    // 에 exit 1)과 계약이 갈렸다 — `--json` 으로 받는 쪽이 이 갈래만 아무 `code` 도 못 봤다.
    //
    // **`AddArgs` 는 네임스페이스 다섯이 같이 쓴다**(`add`·`issue add`·`epic add`·
    // `milestone add`·`backlog add`). 한 구조체에 달린 목록은 그 다섯에 똑같이 서므로, clap 에
    // 두는 한 어느 거절이 먼저인지를 네임스페이스마다 고를 길이 없다.
    /// Epic and issues from markdown at once. `-` is stdin
    #[arg(long, value_name = "file|-")]
    pub from: Option<String>,

    // 거절은 `clap` 이 아니라 `add::run` 이 한다 — 까닭은 바로 위 `from` 의 주석과 같다
    // (moai-yhb1). **한때 적어 둔 까닭은 이제 틀린 글이다**(리뷰): "`requires = \"from\"` 은
    // `from` 이 제목과 `conflicts` 라 조용히 건너뛴다" 였는데, 그 `conflicts_with_all` 을 이
    // 에픽이 걷었으므로 지금은 `requires` 가 실제로 걸린다. 걸려도 안 쓰는 것은 그 거절이
    // 평문 stderr 에 exit 2 라, `--json` 으로 받는 쪽이 `code` 를 못 보기 때문이다.
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

/// 목록의 차례를 한자리에서 댄다(moai-efoc) — `moai show` 와 `moai <종류> show` 가 같은 글을 싣는다.
/// 두 벌로 적으면 한쪽만 고쳐지는 날 같은 플래그가 명령마다 다른 뜻으로 읽힌다.
const SHOW_LIST: &str = "  Order: --sort takes fields separated by commas, like ORDER BY - the next
  field decides only where the ones before it tie. The fields: priority
  (the default: urgent first), created and updated (newest first), status
  (the column order of .moai/config.toml), assignee (the name the screen
  shows, unowned last), title (ignoring case), due (the earliest deadline
  first, none last), id. A field without a direction keeps the one named
  here; :asc puts the smaller value first (p0, the oldest, the first
  column, A) and :desc the larger - unowned or undated rows go first under
  assignee:desc or due:desc. A field is given once. When every field ties,
  a last field of created or updated falls to id alone, and any other to
  priority, then id. --reverse turns the whole order around, ties included.

    moai show --sort priority,updated:desc
    moai show --sort status:asc,title

  Paging: -n cuts the list, and --after <id> starts the next page after the
  last id of the page before. The cursor is that row's values in the order,
  not a position, so rows created or removed meanwhile never shift a page.
  A row whose place in the order changes between pages - the cursor row or
  any other, a priority edit included - can repeat or be skipped; id,
  created and lists of those two alone are the orders no edit moves.
  --archived pages the same way: rows moved into .moai/archive and live
  rows stand in one order, so a long list is read whole in id order with
  --sort id -n <count> --after <last id>.
  Lines sharing one id
  (twins a merge left behind) stand together and a page never splits them,
  so such a page can run past -n. --json stays an array - fewer rows than
  -n means the list has ended.

    moai show --sort id -n 100 --json
    moai show --sort id -n 100 --after <last id> --json

  Archive: a row that has stood in done for archive_days (14 unless
  .moai/config.toml says otherwise, 0 turns it off) is the archive - an
  epic or milestone counted from when it got to done, the clock --done
  reads. --all and -s done leave it out and the tail says how many;
  --archived brings it back, done and deferred with it. Asking by time
  (--since, --created, --done, --stale) finds it anyway, and so does -g
  once done is let in (-g --all). Nothing is stored: it is read from the
  column and the clock each time.

    moai show --archived -g parser

  Time: --since <when> keeps the rows whose own updated_at is at or after
  it. --created and --done take a range from..to or from~to with either
  side left open, or a single time, which is all it spans. <when> is
  YYYY-MM-DD, a day on your own clock - the time zone the screen and
  milestone deadlines use; YYYY-MM-DD HH:MM, one minute on that clock;
  or YYYY-MM-DDTHH:MM:SSZ, an instant in UTC that no time zone moves. The
  end of a range takes all it spans too, so ~2026-10-05 23:59 runs to
  23:59:59. Quote a range that opens with ~ ('~2026-10-05'): zsh takes a
  bare ~2026-10-05 for a named directory. --done looks at rows standing in
  done now, at the time they last got there - an epic or milestone when its
  last member got to done, or when the rest were deferred if that came
  later. Moving rows in or out and removing them leave no trace on the
  group: closed that way it counts from its last finished member, and a
  deferred row moved into a closed group dates it from that row's deferral.
  Asking by time opens what the list hides by default - done, deferred and
  backlog items - because a row closed meanwhile changed too. Narrow with
  -s (name the columns you want) or --type; --deferred keeps only what is
  deferred, and no flag leaves deferred rows out. A lone instant given to
  --created or --done is that one second, not a day.

  --filter takes each filter as item=value, one per flag, and adds four
  time items named after the --json fields: created_at, updated_at,
  started_at and done_at. Each reads the row's own field - a group's too,
  never its members' - in the forms above, and a row without that field
  is unknown and falls in no range. done_at stays when a row is reopened,
  so add status=done for what is closed now; done= keeps the meaning of
  --done. updated_at= with a single day is that day, where since= runs on
  from it. In the explorer ({key:browse.filter}) a value right after = may be quoted.
  due_on= is the row's own deadline and starts_on= its planned start, read
  as calendar days with both ends in (2026-10-01~2026-10-05, ~2026-10-05,
  or one day); a row without one falls in none. They are not time items
  and open nothing hidden.

    moai show --filter 'done_at=2026-10-03 00:00~2026-10-05 23:59'
    moai show -a raven -a joshep --filter started_at=2026-10-01~

  --since keys on each row's own stamp. It misses a removed row (`moai rm`
  leaves no row - --removed below gives those), a note (`moai note` writes
  the journal, not the row), a row whose derived value changed without a
  write of its own (a group's column, an inherited epic) and a row merged
  in with an older stamp. A stamp moai cannot read (fractions, an offset)
  falls in no time range. For a complete copy, pull the whole list
  (--archived, and `moai backlog show --archived` for backlog items)
  and compare row by row.

    moai show --since 2026-09-29T00:00:00Z --json
    moai show --done 2026-09-01..2026-09-30 --type issue

  Removed: `moai show --removed` lists the issues `moai rm` took out,
  oldest first, read from the journal beside the tracker - other
  worktrees are not overlaid, `rm --line` removed an unreadable line, not
  an issue, and `moai <kind> show` refuses it. With --since, those whose
  rm line is stamped at or after it - like --since on rows, that misses a
  removal merged in with an older stamp or never written to the journal,
  so keep the full compare. --json gives each line in the shape of
  `journal` in `moai show <id> --json`, without fields this build does not
  know. It lays the history out and holds it against nothing: an id there
  may live again - created anew, or brought back with an older stamp the
  row list misses - and only the snapshot says whether it lives now.
  A journal line it cannot read that may have held a removal - one cut
  short by a full disk or a crash - is named on stderr by file and line,
  and the exit code is not 0; with --since, only a line stamped in the
  range or with no stamp it can read. --since is the one flag it takes.

    moai show --removed --since 2026-09-29T00:00:00Z --json

  Unread: --json, a list or one row, adds unread:true to a row the
  explorer marks [NEW] - yours, your rows' children and your epics'
  members (not a milestone's), changed since you last read it. The key is
  there only then. Opening a row does not read it; `moai read` does. When
  moai cannot tell who you are, or cannot read your read marks, no row
  carries it.

  Fields: --fields id,title,status keeps only those keys of each --json
  row, in that order, each once - a key the row does not carry stays out.
  It picks keys, not rows: the filters pick the rows. The costly keys it
  leaves out are not worked out - without work a list skips the journals,
  and show <id> asks git only for commits. It stands only with --json.

    moai show -g parser --all --fields id --json

  There is no query language. The filters read derived values the file does
  not hold - a group's column, an inherited epic - so run SQL on the --json
  output, where they are worked out already: derived_status, derived_epic,
  derived_milestone (the milestone --milestone stands the row under),
  shelved_by (the nearest row that took it out of the plan - its own id when
  it was deferred itself; a closed row is in no plan, so --deferred picks
  it by deferred_at alone) and archived (true on a row --all hides as the
  archive). The last three stand only when they hold:

    moai show --type issue --json |
      jq -r '.[] | .derived_epic // \"none\"' | sort | uniq -c
    moai show --archived --json | duckdb -c \"SELECT kind, count(*)
      FROM read_json('/dev/stdin', columns = {kind: 'VARCHAR'}) GROUP BY 1\"";

// **`--json` 의 모양을 여기 적는다** — 에이전트가 읽는 계약이라(moai-1hka.k16) 도움말이 그 문서다. 모양을
// 바꾸면 이 글과 `report::stats::Stats` 를 함께 고치고, `docs/cli.md` 를 다시 짓는다.
const STATS_HELP: &str = "  Counts the rows the filters pick - the same filters as `moai show` -
  with done, deferred, backlog items and the archive in: it counts what
  happened, so nothing finished is hidden. Every number counts one kind, issue
  unless --type names another; a group is measured through its members
  (-e, --milestone). The kind axis alone counts every row picked, to show
  what was left out. --all and --archived are taken and change nothing.
  The time filters read as in `moai show --help`.

  Nothing is stored, and the journal's column moves are never folded in.
  The numbers come from the rows and from the `model:` lines in the notes -
  the same `work` that `moai show --json` gives.

  Axes - --by status,tag prints those in full (with --bucket or --last
  the flow follows them), and without --by the overview shows the first
  three:
    status      the column, a group's read from its members as on the board
    kind        issue, epic, milestone, backlog - every row picked
    priority    0 to 3
    tag         a row counts once per tag it carries
    assignee    name and email
    epic        the epic a row stands in (derived_epic)
    milestone   the milestone it stands in, through its epic too

  Flow: per day or per week (Monday first) on your clock - the day
  `--created <day>` reads. Created counts rows made then, done counts rows
  standing in done that got there then (the time --done reads). --last is
  how many buckets, the current one included (14 days or 8 weeks).

  Lead time runs from created to done, cycle time from started_at - the
  first move out of the first column - to done, over rows standing in done,
  in minutes. A row with no start is unknown, not zero: measured and
  unknown are counted apart, and the median and p90 cover the measured
  ones only. It is wall clock, not effort.

  AI work counts the `model:` lines, and recorded counts the rows that carry
  one. Tokens are not added up: what a window or an agent shows was not
  what the work cost. Reviews are the rows tagged review; the grade on
  their lines is the review grade.

  moai stats                           the overview
  moai stats -e moai-1hka              one epic's members
  moai stats --by tag,assignee         two axes in full
  moai stats --bucket day --last 30 --json

  --json gives one object. Every key is always there except three: `by`
  holds the axes asked (all seven without --by), an assignee carries email
  only when one is written, and journal_error stands only when a journal
  could not be read - then work and reviews are short. A null key means
  none - no tag, no epic, no assignee. An axis need not add up to rows: a
  row counts once per tag, and a row placed in no group (a twin's eclipsed
  line, as -e none leaves it out) stands under no epic or milestone at
  all. Durations are minutes.

    {\"kind\":\"issue\",\"rows\":42,
     \"by\":{\"status\":[{\"key\":\"todo\",\"rows\":9},...],
           \"priority\":[{\"key\":0,\"rows\":1},...],
           \"assignee\":[{\"key\":\"Kim\",\"email\":\"kim@example.com\",\"rows\":7},
                       {\"key\":null,\"rows\":2}],...},
     \"flow\":{\"bucket\":\"week\",\"zone\":\"UTC\",
             \"buckets\":[{\"start\":\"2026-09-28\",\"created\":3,\"done\":2},...]},
     \"lead_time\":{\"done\":30,\"measured\":30,\"unknown\":0,
                  \"median\":95,\"p90\":4100},
     \"cycle_time\":{...the same keys...},
     \"work\":{\"recorded\":25,\"lines\":31,
             \"by_model\":[{\"provider\":\"anthropic\",\"model\":\"opus-5\",
                          \"lines\":31}],
             \"by_grade\":[{\"grade\":\"high\",\"lines\":12,...},...]},
     \"reviews\":{\"rows\":6,\"work\":{...the same keys as work...}}}";

/// `moai keys` 가 받는 것(moai-so7w).
#[derive(Args, Debug)]
pub struct KeysArgs {
    /// Print a starter keys.toml (nothing is written)
    #[arg(long)]
    pub template: bool,
}

/// `moai stats` 가 받는 것(moai-1hka.k16). **거르개는 `show` 의 것을 그대로 받는다**(`FilterArgs`) — 차례와 쪽
/// (`PageArgs`)·겹쳐 보기(`WorktreeArg`)는 안 받는다: 세는 자리에 세울 차례도 자를 목록도 없고, 겹쳐 본 줄을
/// 세면 옆 가지에서 집은 일이 이 저장소의 흐름에 두 번 선다. 안 받는 플래그는 clap 이 이름을 대며 거절한다 —
/// 말없이 먹지 않는다.
#[derive(Args, Debug)]
pub struct StatsArgs {
    /// Print these axes in full (see below)
    #[arg(long, value_name = "axis", value_enum, value_delimiter = ',', hide_possible_values = true)]
    pub by: Vec<AxisArg>,

    /// What one row of the flow covers (week when absent)
    #[arg(long, value_name = "day|week", value_enum, hide_possible_values = true)]
    pub bucket: Option<BucketArg>,

    /// How many rows of flow, the current one included
    #[arg(long, value_name = "n", value_parser = clap::builder::RangedU64ValueParser::<usize>::new().range(1..=1000))]
    pub last: Option<usize>,

    #[command(flatten)]
    pub filter: FilterArgs,
}

#[derive(Args, Debug, Default)]
pub struct ArchiveArgs {
    /// Show what would move without changing files
    #[arg(long)]
    pub dry_run: bool,
    /// Remove archive copies; keep the live row
    #[arg(long, value_name = "ID", conflicts_with = "dry_run")]
    pub drop: Option<String>,
}

/// `--by` 의 낱말 — `report::stats::Axis` 의 이름과 같다. 잇는 것은 `cmd::stats` 다(`report` 는 clap 을 모른다).
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq)]
pub enum AxisArg {
    Status,
    Kind,
    Priority,
    Tag,
    Assignee,
    Epic,
    Milestone,
}

/// `--bucket` 의 낱말.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq)]
pub enum BucketArg {
    Day,
    Week,
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

    // 거르개 머리글 밑에 안 두는 까닭 — 무엇을 거르는 말이 아니라 **다른 목록**(저널의 `rm` 줄)을 고르는
    // 말이다(moai-7dmq). 받는 거르개도 `--since` 하나라, 그 머리글 밑에 서면 나머지도 받는 줄로 읽힌다.
    /// Removed issues, from the journal (see below)
    #[arg(long)]
    pub removed: bool,

    // **거르개가 아니다** — 무엇을 고르는지는 안 바꾸고 줄마다 어느 키를 내는지만 정한다(moai-wk7i.vsk).
    /// Keep only these keys of each --json row
    #[arg(long, value_name = "keys", value_delimiter = ',')]
    pub fields: Vec<String>,

    #[command(flatten)]
    pub worktree: WorktreeArg,

    #[command(flatten)]
    pub filter: FilterArgs,

    #[command(flatten)]
    pub page: PageArgs,
}

/// 목록의 차례와 쪽(moai-efoc). **거르개가 아니다** — 무엇을 고르는지는 안 바꾸고, 고른 것을 어떤
/// 차례로 몇 줄 내는지만 정한다. 그래서 `FilterArgs` 와 머리글이 따로다.
#[derive(Args, Debug, Default)]
#[command(next_help_heading = "Order and paging")]
pub struct PageArgs {
    /// Order the list by these fields (priority when absent)
    #[arg(long, value_name = "fields", value_parser = sort_fields)]
    pub sort: Option<SortFields>,

    /// Turn the order around, ties included
    #[arg(long)]
    pub reverse: bool,

    /// Give at most this many rows
    // **`usize` 로 바로 받는다**(moai-efoc 리뷰) — `u64` 로 받아 쓰는 자리마다 `usize::try_from` 을 되풀이하던
    // 자리다. 범위 검사와 오류 글은 `value_parser!(u64).range(1..)` 와 같다(`value_parser!(usize)` 에는
    // `range` 가 없다).
    #[arg(short = 'n', long, value_name = "count", value_parser = clap::builder::RangedU64ValueParser::<usize>::new().range(1..))]
    pub limit: Option<usize>,

    /// Start after this row: the last id of the page before
    #[arg(long, value_name = "id")]
    pub after: Option<String>,
}

/// `moai ready` 가 받는 것 — 워크트리 겹쳐 보기와 자르기(moai-efoc). `status`·`prime` 은 자를
/// 목록이 없어 [`WorktreeArg`] 만 받는다.
///
/// **`--after` 는 없다.** `ready` 의 차례는 우선순위·마일스톤·나이를 섞은 판단이라 그 값으로 줄을
/// 되짚을 키가 없고, 집을 일은 앞에서부터 집는 것이라 다음 쪽이 뜻이 없다.
#[derive(Args, Debug, Default)]
pub struct ReadyArgs {
    #[command(flatten)]
    pub worktree: WorktreeArg,

    /// Give at most this many rows (held stays whole)
    #[arg(short = 'n', long, value_name = "count", value_parser = clap::builder::RangedU64ValueParser::<usize>::new().range(1..))]
    pub limit: Option<usize>,
}

/// `--sort` 의 낱말. **탐색기가 설정에 적는 이름과 같다**(`tui::keys::Order::name`) — 한 낱말이
/// 두 표면에서 다른 차례를 가리키면 안 된다.
///
/// **필드의 쉼표 목록이고 필드마다 `:asc`·`:desc` 가 붙는다**(moai-r170.9es, 사람이 정한 꼴) — 그래서
/// `ValueEnum` 이 아니라 글을 [`crate::query::parse_order`] 로 읽는다. 낱말과 방향을 읽는 자는 설정에 적힐
/// 차례와 하나다 — 둘이 따로 읽으면 같은 글이 두 차례가 된다. 못 읽은 글은 clap 의 오류로 고를 낱말을 다 댄다.
/// 낱말마다의 방향은 `Show` 의 `after_help`(`SHOW_LIST`)가 한자리에서 댄다.
///
/// `Vec` 을 바로 받지 않는 것은 clap 의 derive 가 `Option<Vec<_>>` 를 "여러 번 받는 플래그" 로 읽어서다.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SortFields(pub Vec<crate::query::Field>);

fn sort_fields(raw: &str) -> Result<SortFields, String> {
    crate::query::parse_order(raw).map(SortFields).map_err(|e| e.to_string())
}

/// `status`·`ready`·`show` 가 함께 받고, `read` 도 받는다 — `read` 는 트래커가 아니라 읽음 파일에 쓰고, 겹친
/// 판에서 id 와 `updated_at` 을 찾을 뿐이다(moai-m4ez.gcp). **전역 플래그로 두지 않는다** — 트래커를 쓰는
/// 명령(`mv`·`edit`)에 붙으면 겹친 화면을 보고 쓴다고 믿게 되는데, 쓰기는 언제나
/// 제 워크트리 파일에만 간다.
#[derive(Args, Debug, Default, Clone, Copy)]
pub struct WorktreeArg {
    /// Also overlay other worktrees (no file changes)
    #[arg(long)]
    pub worktree: bool,
}

/// 쉼표는 "또는", 반복은 "그리고". 담당(`-a`)만 반복도 "또는"이다(moai-97tn) — 머리글이 대지 않는 그 하나는
/// 그 플래그의 도움말이 댄다.
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

    /// Child of this issue (`none` = top)
    #[arg(long, value_name = "id|none")]
    pub parent: Vec<String>,

    #[arg(short, long, value_name = "0-3")]
    pub priority: Vec<String>,

    /// That assignee (repeated = or)
    #[arg(short, long, value_name = "who|none|me")]
    pub assignee: Vec<String>,

    #[arg(long = "type", value_name = "issue|epic|milestone|backlog")]
    pub kind: Option<Kind>,

    /// In id, title, tag, body or notes
    #[arg(short = 'g', long, value_name = "text")]
    pub grep: Option<String>,

    /// Sitting in its column that long
    #[arg(long, value_name = "days")]
    pub stale: Option<i64>,

    /// Changed since that time (see below)
    #[arg(long, value_name = "when")]
    pub since: Vec<String>,

    /// Created in that range (see below)
    #[arg(long, value_name = "from..to")]
    pub created: Vec<String>,

    /// Closed in that range (see below)
    #[arg(long, value_name = "from..to")]
    pub done: Vec<String>,

    /// Only what is deferred
    #[arg(long)]
    pub deferred: bool,

    /// Include done/deferred, no archive
    #[arg(long)]
    pub all: bool,

    /// Include the archive too (old done)
    #[arg(long)]
    pub archived: bool,

    /// One filter string (`status=todo`)
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

    /// A note on this move (journal only). `-` reads stdin
    ///
    /// Text, not a path - a file goes in as `-m - < <file>`. A one-line
    /// text that names a file still goes in as those words, and one
    /// stderr line says so.
    #[arg(short, long, value_name = "text", allow_hyphen_values = true, verbatim_doc_comment)]
    pub msg: Option<String>,

    /// Only while still in this column (racing pickups)
    ///
    /// Without it nothing is blocked, as before. With it, the column is
    /// looked at again inside the lock, and a row whose column changed in
    /// the meantime is left untouched and stands as a partial failure.
    #[arg(long, value_name = "column", verbatim_doc_comment)]
    pub from: Option<String>,

    /// Become the assignee and note whose it was
    #[arg(long)]
    pub take: bool,
}

#[derive(Args, Debug)]
#[command(group(clap::ArgGroup::new("dates").args(["start", "due"]).multiple(true)))]
pub struct EditArgs {
    #[arg(value_name = "id")]
    pub id: String,

    /// One line. A lone `-x` needs `--title=-x`
    #[arg(long, value_name = "text", allow_hyphen_values = true)]
    pub title: Option<String>,

    /// Text, not a path. `-` reads stdin: `-b - < <file>`
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

    /// Give `name (email)`. `none` clears it, `me` is you
    #[arg(short, long, value_name = "who|none")]
    pub assignee: Option<String>,

    /// Start, a day or `+N` days. `none` clears it
    #[arg(long, value_name = "date|none")]
    pub start: Option<String>,

    /// Deadline, a day or `+N` days. `none` clears it
    #[arg(long, value_name = "date|none")]
    pub due: Option<String>,

    /// Move the planned days N days, as the row
    /// holds them when written
    #[arg(long, value_name = "±N", allow_negative_numbers = true, verbatim_doc_comment, value_parser = shift_days(),
          conflicts_with_all = ["start", "due", "shift_start", "shift_due", "if_unplanned"])]
    pub shift: Option<i64>,

    /// Move the start alone (none: from the due)
    #[arg(long, value_name = "±N", allow_negative_numbers = true, value_parser = shift_days(),
          conflicts_with_all = ["start", "due", "shift_due", "if_unplanned"])]
    pub shift_start: Option<i64>,

    /// Move the deadline alone (none: from start)
    #[arg(long, value_name = "±N", allow_negative_numbers = true, value_parser = shift_days(),
          conflicts_with_all = ["start", "due", "if_unplanned"])]
    pub shift_due: Option<i64>,

    /// Write `--start`/`--due` only on a row
    /// with no planned day; else refuse
    #[arg(long, requires = "dates", verbatim_doc_comment)]
    pub if_unplanned: bool,
}

/// `edit --shift*` 의 날 수 — **web 의 끌기가 받는 폭과 같다**(여섯 자리, `web::api::is_days`, moai-vj8w 리뷰). 막지 않으면
/// `Issue::shifted` 의 덧셈이 넘친다: 시험 판(overflow-checks)은 쓰기 안에서 넘어지고, 릴리스 판은 돌아 아무도 안 친
/// 해를 거절문에 댄다. 여섯 자리는 이천칠백 해 남짓이라 손으로 옮길 날은 다 든다.
fn shift_days() -> clap::builder::RangedI64ValueParser<i64> {
    clap::value_parser!(i64).range(-999_999..=999_999)
}

#[derive(Args, Debug)]
pub struct DeferArgs {
    #[arg(required = true, value_name = "id")]
    pub ids: Vec<String>,

    /// Pick it back up
    #[arg(long)]
    pub undo: bool,

    /// Why it is deferred (journal only). `-` reads stdin
    ///
    /// Text, not a path - a file goes in as `-m - < <file>`. A one-line
    /// text that names a file still goes in as those words, and one
    /// stderr line says so.
    #[arg(short, long, value_name = "text", allow_hyphen_values = true, verbatim_doc_comment)]
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

    // 읽음으로 적는 것은 겹쳐 보기가 보이는 그 판의 `updated_at` 이다(moai-m4ez.gcp) — 옆 워크트리에만 있는 줄도
    // 적고, 옆이 더 늦게 고친 줄은 그 늦은 `updated_at` 을 적는다. 옆 워크트리에는 아무것도 안 쓴다.
    #[command(flatten)]
    pub worktree: WorktreeArg,
}

#[derive(Args, Debug)]
pub struct RmArgs {
    #[arg(required_unless_present = "line", conflicts_with = "line", value_name = "id")]
    pub ids: Vec<String>,
    /// Remove the unreadable line at number <n>
    #[arg(long, value_name = "n")]
    pub line: Option<usize>,
    // `requires = "line"` 을 안 건다 — clap 은 `requires` 가 지목한 인자(`line`)가 이미 준 인자(`ids`)와
    // `conflicts` 면 그 `requires` 를 조용히 건너뛰어, `moai rm <id> --yes` 가 clap 을 지나 말없이
    // 지웠다(재 봤다). 거절은 `rm::run` 이 `BAD_INPUT` 으로 해서 `--json` 도 `code` 를 받는다 —
    // `add --dry-run` 이 제 거절을 `add::run` 에 두는 것과 같은 까닭이다(moai-yhb1).
    /// With --line and --match: remove it. Else only shown
    #[arg(long)]
    pub yes: bool,
    /// With --yes: the hash the preview printed
    // **`--yes` 를 보여 준 줄에 묶는다**(moai-6nha, 2026-09-29 사용자 결정). 번호만 보고 지우면
    // 줄 수가 바뀐 뒤 보여 준 것과 다른 못 읽는 줄이 간다. 거절은 `rm::run` 이 한다 — `yes` 와 같은
    // 까닭으로 `requires` 를 안 건다. 해시 꼴이 아닌 값(빈 값·오타)도 거기서 `bad_input` 으로 거절한다.
    #[arg(long = "match", value_name = "hash")]
    pub matches: Option<String>,
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

/// `moai update` 의 인자(moai-zsfr.2p9). **자리를 받는 인자가 없다** — 깔 자리는 도는 바이너리의 자리로
/// 정해진다. 다른 자리에 깔고 싶은 사람은 `install.sh --dir` 를 친다.
#[derive(Args, Debug)]
pub struct UpdateArgs {
    /// Release tag to install instead of the latest
    #[arg(long, value_name = "tag")]
    pub version: Option<String>,
    /// Print what would be fetched and run; touch nothing
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args, Debug)]
pub struct WebArgs {
    /// Port - 6624 unless given; 0 picks a free one
    #[arg(long, value_name = "n", default_value_t = 6624, hide_default_value = true)]
    pub port: u16,
    /// Address - 127.0.0.1 unless given
    #[arg(long, value_name = "addr")]
    pub host: Option<std::net::IpAddr>,
    /// Say where it would serve; bind nothing
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args, Debug)]
pub struct TuiArgs {
    // `none`·`lost` 는 화면 글이 아니라 **이 명령이 받는 낱말**이다(`cmd/tui.rs` 의
    // `NO_MILESTONE`·`LOST`). **`value_name` 에는 안 적는다** — 두 낱말이 옵션 열을 넓혀
    // 이 명령의 전역 옵션 설명까지 80칸을 넘겼다(moai-l5uf). 받는 낱말은 `after_help` 가 댄다.
    /// Open here - a directory opens inside it
    #[arg(long, value_name = "id|basket")]
    pub path: Option<String>,

    // **펼친 차례가 아니라 목록의 모양이다**(moai-0aa7.joy) — 거르개·차례 묶음보다 **앞에** 둔다. 뒤에 두면
    // `OrderArgs` 의 `next_help_heading` 을 물려받아 도움말이 이것을 `Order:` 밑에 세운다.
    /// With --json, list every row below, with its home
    #[arg(long)]
    pub deep: bool,

    // **`--json` 만 켠다** — 화면은 늘 옆 워크트리를 겹쳐 연다(`SPC v w` 로 끈다). 기계로 읽는 쪽의 모양은
    // `status`·`ready`·`show` 처럼 `--worktree` 를 줘야 겹친다(moai-m4ez.gcp). `shaped` 에 안 든다 — 화면에서는 받아도
    // 이미 그렇다.
    /// With --json, overlay other worktrees' rows
    #[arg(long)]
    pub worktree: bool,

    // **`--json` 의 목록에서 아카이브를 뺀다**(moai-m4ez.4zq) — `show --all` 이 숨기는 줄(`Where::archived`)이다. 웹 페이지가
    // 아카이브를 안 보이는 동안 쓰기마다 읽는 `/api/tree` 가 3천 줄(689KB) 가운데 95% 를 그 줄로 채웠다. 폴더는 밑에 걸린
    // 산 줄이 있으면 서니, 아카이브에 든 에픽 밑의 되살린 멤버는 제 자리에 선다. 플래그가 없으면 전처럼 다 낸다.
    /// With --json, leave out the archive
    #[arg(long)]
    pub no_archive: bool,

    /// Leave out gantt values - requires --json --deep
    #[arg(long)]
    pub no_gantt: bool,

    // **`--json` 의 목록만 거르고 늘어놓는다**(moai-wk7i.5wx) — 화면은 `SPC f`·`SPC s` 가 한다. 말은 `show` 의 것이다.
    #[command(flatten)]
    pub filter: FilterArgs,

    #[command(flatten)]
    pub order: OrderArgs,
}

/// 차례만 — 쪽(`-n`·`--after`)은 없다. 디렉터리 목록은 한 자리를 통째로 낸다(moai-wk7i.5wx).
#[derive(Args, Debug, Default)]
#[command(next_help_heading = "Order")]
pub struct OrderArgs {
    /// Order the listing by these fields (as `show --sort`)
    #[arg(long, value_name = "fields", value_parser = sort_fields)]
    pub sort: Option<SortFields>,

    /// Turn the order around, ties included
    #[arg(long)]
    pub reverse: bool,
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
    /// What the next person (or agent) should read. A lone `-x` needs `--`
    #[arg(value_name = "text", allow_hyphen_values = true)]
    pub text: Option<String>,

    /// Text, not a path. `-` reads stdin: `-b - < <file>`
    ///
    /// **It pushes the positional out.** Given both, nobody can remember
    /// which one wins, and a rule nobody remembers erases someone's text
    /// sooner or later.
    #[arg(short = 'b', long, value_name = "text", conflicts_with = "text", verbatim_doc_comment)]
    pub body: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum SkillCmd {
    /// Plant the skills for each agent; register Claude's with `claude`
    #[command(after_help = "  Which agents: --agent claude (the default), codex, antigravity or auto -
  repeat it for several. auto takes whichever of claude, codex and agy is on
  PATH, and claude when none is.

  Codex and Antigravity read the same `.agents/skills/` in this repository, so
  naming either plants it for both. There is nothing to register: commit the
  directory and the team has it. --scope is Claude's registration only. The
  text is the one Claude gets; the steps that differ per agent are in the
  `moai` skill's \"Words per agent\" table. The supervisor (moai-supervise),
  its hands on tmux panes, cmux tabs and Saycode sessions (moai-tmux,
  moai-cmux, moai-saycode) and the recovery of sessions that died
  (moai-recover) are planted for Claude Code only — Codex and Antigravity get
  no supervisor.

  Optional skills are planted only where you choose them: today moai-tmux,
  moai-cmux and moai-saycode, the supervisor's hands on tmux panes, on cmux
  tabs (cmux 0.65.0 or later) and on Saycode sessions. --with <skill>
  plants one and
  --without <skill> removes it; both repeat, and a comma separates names.
  What you chose is not written down anywhere - a skill planted in the tree
  is the answer. So a plain install refreshes the optional skills already
  planted and plants no new one, and a first install plants none. Naming a
  skill that is always planted, or one name in both, is refused.

  The hooks are each agent's own: --agent codex plants `.codex/hooks.json`,
  --agent antigravity plants `.agents/hooks.json` - commit them too. Codex
  runs project hooks only once you trust them: open /hooks in a codex session
  in this repository, and again whenever they change. A hooks file moai did
  not write is left as it is - merge moai's in by hand, or move it aside. Codex
  sends hooks only for its shell, apply_patch and MCP calls.

  For Claude, skills and hooks are installed into `.claude/moai-plugin/` and
  registered with `claude`. Your settings.json is not touched - putting the
  two keys in is `claude`'s job. The one exception is the old declarations
  below.

  **Running again only overwrites.** Deleting a hook file a running session
  holds would block every tool call of that session. The one thing deleted
  is a skill directory an earlier moai planted in the same tree and this one
  no longer plants (skills/moai-work/, and skills/moai-supervise/ in
  .agents/skills/), or an optional skill left out with --without, and only
  while it holds nothing but the files moai planted there; one with a file
  of yours in it is left, and one line names it.

  The version is a hash of what is installed. Same content, same version, so
  there are no empty updates.

  The two Korean text plugins an earlier moai installed alongside
  (korean-skills and humanize-korean) are **removed**, once per scope: only
  where this repository's moai stands as an earlier version (its installed
  skill still teaches them), and only when their marketplace is the one an
  earlier moai added. Once that scope holds this version, nothing there is
  removed again. Where that earlier version stands at a scope other than
  --scope, install first brings it up to this version there (claude plugin
  update --scope <that scope>), so the next install does not remove from it
  again; if that fails, nothing is removed there and the next install tries
  again. Nothing is removed or brought up when moai's own registration
  fails. A user-scope install is left behind when another repository's moai
  stands there, and then the command to remove it is printed in one line;
  that scope is not brought up either, so it is removed once that moai is
  gone. The marketplaces are left behind. If one cannot be removed, moai's own
  registration still stands and the command to remove it by hand is printed.

  At project scope an earlier moai also declared their marketplaces in the
  committed .claude/settings.json, which kept offering them to the team. moai
  deletes those entries itself - only theirs, only when they point where an
  earlier moai pointed them, and only when nothing in that file still enables
  a plugin from them. The rest of the file stays as it was; commit the
  change. `claude plugin marketplace remove --scope project` is not used: with
  no other declaration in sight it removes the marketplace from the whole
  machine. If the entry cannot be deleted, where to delete it is printed.

  moai skill install                  just me (the default. settings.local.json)
  moai skill install --scope user     every repository on this machine
  moai skill install --scope project  with the team (committed settings.json)
  moai skill install --dry-run        only show what would be done
  moai skill install --agent codex    .agents/skills/ for Codex and Antigravity
  moai skill install --agent auto     whichever agent is on PATH
  moai skill install --with moai-tmux     plant the tmux skill too
  moai skill install --without moai-tmux  take it out again
  moai skill install --with moai-cmux     plant the cmux skill
  moai skill install --with moai-saycode  plant the Saycode skill too

  A Claude session already open keeps the old version - reopen it to pick
  this one up.")]
    Install {
        // 값과 기본값은 `--color` 처럼 글로 적는다 — clap 이 붙이는 괄호가 `-h` 에서
        // 98칸이 됐다(moai-x18p).
        //
        // **기본값을 clap 에 안 맡긴다**(moai-xs2h.ylx) — 준 값인지 알아야 `--agent codex --scope user` 에 "범위는
        // Claude 의 것" 한 줄을 낸다. 기본값 `local` 은 부르는 쪽이 채운다.
        /// Where to register: local (default), project, user
        #[arg(long, value_name = "scope", hide_possible_values = true)]
        scope: Option<Scope>,

        /// Agent: claude (default), codex, antigravity, auto
        #[arg(long = "agent", value_name = "agent", hide_possible_values = true)]
        agents: Vec<Agent>,

        // **되풀이도 쉼표도 받는다**(moai-3r7l.5ja) — `--with moai-tmux,moai-cmux` 와 `--with a --with b` 가 같다. 이름은
        // 글로 받아 `cmd::skill::Pick::check` 가 잰다 — clap 의 값 목록으로 두면 늘 심는 이름에 "늘 심는다" 를 못 댄다.
        /// Also plant this optional skill (repeatable)
        #[arg(long = "with", value_name = "skill", value_delimiter = ',')]
        with: Vec<String>,

        /// Remove this optional skill (repeatable)
        #[arg(long = "without", value_name = "skill", value_delimiter = ',')]
        without: Vec<String>,

        /// Change nothing; only say what would be done
        #[arg(long)]
        dry_run: bool,
    },

    /// What is installed at which scope, and where it differs
    #[command(after_help = "  It **only reads** - `claude`'s registry (~/.claude/plugins/) and the planted
  files. Whatever is out of line the exit code is 0 - this is a command that
  shows, not one that blocks.

  What it looks at:
    marketplace   registered under this repository's name, not pointing
                  somewhere else
    install       which version at which scope, and whether it matches the
                  version that would be installed now
    hook          whether the executable the install calls is still there
    claude        whether it is on PATH (without it nothing can be installed
                  or removed)
    .agents       whether .agents/skills/ holds this version's skills
                  (codex and antigravity read it)
    codex hooks   whether .codex/hooks.json is this version's, and moai's
    agy hooks     whether .agents/hooks.json is this version's, and moai's
    codex, agy    whether they are on PATH")]
    Status,

    /// Remove the registration from `claude`. Installed files stay
    #[command(after_help = "  This repository's install is removed per scope with
  `claude plugin uninstall`, and the marketplace with
  `claude plugin marketplace remove`. Deleting the two keys from your
  settings is `claude`'s job - we do not touch someone else's JSON, except
  the old declarations below.

  **`.claude/moai-plugin/` is not deleted.** Deleting a file a running
  session holds can block that session's tool calls. Delete it after closing
  the session.

  The two Korean plugins an earlier moai installed alongside are removed **at
  the scope moai was removed from**, under the same conditions as install: an
  earlier version of moai stands there and the marketplace is the one it
  added. The marketplace is left behind - the name is global to one machine
  and another repository's install uses it. A user-scope install is also left
  behind when another repository's moai stands there, and then the command to
  remove it is printed in one line. At project scope the marketplace's
  declaration in the committed .claude/settings.json goes, as with install.

  A Claude session already open keeps calling the old hook - reopen it for
  the removal to land.

  For codex and antigravity (--agent) there is no registration to remove,
  and no file is deleted either: the `rm -r` lines for moai's skills in
  `.agents/skills/`, and the `rm` line for that agent's hooks file when moai
  wrote it, are printed, to run once no session holds them. Without that
  --agent, one line says when moai's skills are still there.

  --only <skill> removes one optional skill and nothing else: it is
  `moai skill install --without <skill>` under another name. That skill's
  directory goes (only while it holds nothing but moai's files) and the
  plugin is planted again; the registration and the other skills stay.
  A tree moai has not planted is left alone - no file and no hooks file
  is made there. Where this repository's moai is registered, that scope is
  brought up to the new version; where it is registered nowhere, only the
  files change - no registration is made. A skill that is always planted
  is refused.

  moai skill uninstall --dry-run      only show what would be called
  moai skill uninstall --agent codex  name what to delete in .agents/skills/
  moai skill uninstall --only moai-tmux   take the tmux skill out")]
    Uninstall {
        /// Agent: claude (default), codex, antigravity, auto
        #[arg(long = "agent", value_name = "agent", hide_possible_values = true)]
        agents: Vec<Agent>,

        // `install --without` 의 다른 이름이다(moai-3r7l.xr1) — 되풀이와 쉼표도 같다.
        /// Remove only this optional skill (repeatable)
        #[arg(long = "only", value_name = "skill", value_delimiter = ',')]
        only: Vec<String>,

        /// Call nothing; only say what would be called
        #[arg(long)]
        dry_run: bool,
    },
}

/// 스킬을 심을 에이전트(moai-xs2h, 2026-10-04 사용자 결정). **안 주면 `claude` 하나다** — 지금 부르는 사람에게는
/// 아무것도 안 바뀌고, 저장소에 새 디렉터리가 묻지 않고 서지 않는다.
///
/// Codex 와 Antigravity 는 **같은 자리**(`.agents/skills/`)를 읽어, 둘 중 하나를 고르면 둘 다 받는다.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Agent {
    /// Claude Code: the plugin in `.claude/moai-plugin/`, registered with `claude` (default)
    Claude,
    /// Codex: the skills in `.agents/skills/` and the hooks in `.codex/hooks.json`
    Codex,
    /// Antigravity: the same `.agents/skills/` Codex reads, and `.agents/hooks.json`
    Antigravity,
    /// Whichever of claude, codex and agy is on PATH - claude when none is
    Auto,
}

impl Agent {
    pub fn as_str(self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
            Agent::Antigravity => "antigravity",
            Agent::Auto => "auto",
        }
    }
}

/// 훅을 부른 에이전트의 말씨 — stdin 과 stdout 의 꼴(moai-u5wr). 판정은 셋이 같고 꼴만 다르다.
/// `auto` 가 없다 — 심은 줄이 제 에이전트를 안다.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Dialect {
    /// Claude Code (default)
    #[default]
    Claude,
    /// Codex - the same shape as Claude's, plus apply_patch
    Codex,
    /// Antigravity (agy) - camelCase in, a top-level decision out
    Antigravity,
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
