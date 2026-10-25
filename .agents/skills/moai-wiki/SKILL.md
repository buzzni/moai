---
name: moai-wiki
description: Use when the repository's manual (its wiki) has to catch up with what the work changed — at the end of an epic, or when a person asks. "update the wiki", "write the manual", "document this", "record a lesson", "위키 갱신", "매뉴얼 써", "문서화해 줘", "wiki 정리", "교훈 남겨", "실수 기록". The pages are markdown files in the repository, listed by `moai wiki ls`; nothing checks them.
---

# moai-wiki — keep the manual in step with the work

The wiki is the repository's manual: markdown pages committed beside the code. It
follows the work — **when an epic changes what a person does, the page that teaches
it changes in the same merge.** Nothing checks this and nothing is blocked. An epic
that changed nothing a person does writes nothing.

## What the wiki is

    moai wiki ls                           the pages: slug and title, and what does not resolve
    moai wiki ls --json                    the same with each page's path, for a machine
    moai wiki show <slug>                  one page, and the pages that link to it
    moai wiki search <words>               the sections that hold those words, best first (`--json`)

- **One directory.** `dir` in `moai wiki ls --json` names it — `docs` unless
  `wiki_dir` in `.moai/config.toml` says otherwise — relative to the checkout, and
  `dir_abs` is the same directory as an absolute path in this checkout (absent for
  `outside`, and where that path is not UTF-8). When it cannot be read, `error`
  stands in place of `pages`, and its `kind` says why: `no_dir` is a repository with
  no wiki yet, `outside` and `not_a_dir` are a `wiki_dir` pointing where moai will
  not read — fix the key, not the pages — and `failed` is the disk refusing to open it
- **One page is one `.md` file** — `path` in the list. Its `slug` is that path below
  `dir` without `.md`. Name a new file in lowercase ASCII kebab case
  (`merge-driver.md`) so its slug reads the same on every machine. `README.md` or
  `index.md` at the top of `dir` is the home page
- **One `# Title` line opens the page.** It is the `title` the list shows; without it
  the file name stands in
- **Pages link with plain relative links** — `[the explorer](explorer.md)`. Not
  `[[wiki links]]`: GitHub does not draw them, and one link with two spellings is one
  vocabulary too many
- **A link can land on a heading** — `[epic](glossary.md#epic)`, or `[above](#epic)`
  on the same page. The part after `#` is the heading's anchor as GitHub makes it:
  lowercase, punctuation dropped, each space a `-` (`## The --json contract` is
  `#the---json-contract`), and a repeated heading `-1`, `-2`. The same link lands on
  that heading on GitHub and in the explorer's wiki window. Link to the heading that
  says it, not to the top of a long page
- **An issue is named by its bare id**, as a whole word — not a link. moai finds it
  in prose and in inline code; an id inside a fenced or indented code block is read
  as an example and is not counted
- **The pages ride the branch.** Unlike `.moai/`, nothing moves them to the main
  checkout: a page written in a worktree stands in that worktree and is merged with
  the code, by git's own 3-way merge

Every row under `pages` also says what to fix: `issues` (each `id`, and `exists`
false for an id that names no issue), `links` (the links to other pages — each `text`,
`to` as the slug it lands on, and `resolved` false when no such page stands; a link
with a `#` also carries `anchor`, the part after it, and `anchor_resolved` false when
that page has no such heading — absent when the page could not be read, so nobody can
tell — and a same-page `#anchor` stands with `to` naming its own page) and
`conflict` (true while merge conflict markers stand in the page). `linked_from` turns
`links` around: the slugs of the pages that link to this one, in list order, `[]` when
none does. A page other than the home page that nothing links to is found only through
the list — link it from the page that should lead there. Judge that from
`moai wiki ls --json`, not from one page: a page that could not be read links nowhere,
and only the list shows it, with its `error`. `moai wiki show <slug> --json` says when
its own count may be short — `linked_from_partial` stands `true` when some other page
could not be read or the walk left a spot out, and is absent when every page was
counted. A page that could
not be read still stands in the list, under its file name, with an `error` of its own
whose `kind` says why — `too_large`, `refused` or `failed`. What the walk had to
leave out stands under `skipped`, each with its `path` and a `kind` — `dir_link` (a
link to a directory, not followed), `not_utf8` (a file name that cannot be a slug) or
`unreadable` (a directory it could not open or list to the end, or a name whose kind it
could not read); with nothing left out the key is absent,
and with it the exit code is non-zero although `pages` is whole. Branch on `kind`, not
on the words beside it. Look at all of these after you write.

## Two kinds of page

- **Reference — generated.** A page whose first paragraph says it is generated and
  not to be edited by hand ("Generated from … Do not edit by hand") is written by
  the generator that paragraph names. Never edit it: when its source changed, run
  the generator and commit what it wrote
- **Guides — written by hand.** A guide teaches what a person does: what they came
  to do, the steps in order, why, and what goes wrong. **Do not copy `--help` into
  a guide** — name the command and say when to reach for it. The help is the truth
  for flags, and a copy goes stale the day a flag changes

## At the end of an epic

Most pages are written here — the window that did the epic is the only one that knows
what changed and why. It runs on that epic's branch — in its worktree, where it has
one — after the review and the CHANGELOG line and before the merge.

1. Read four things: `moai show <epic> --json` (the body and the notes say why it was
   decided), the CHANGELOG line the epic wrote if the repository keeps one, the
   `--help` of each command the epic touched, and `moai wiki ls --json`
2. Ask once: **did this epic change what a person does** —
   a key, a command, a flag, a file, a format, a procedure. A refactor
   inside, or a fix that left the behaviour as it was, changed nothing a person
   does. If nothing changed, stop — write nothing
3. Change the page that teaches it — find it in `moai wiki ls`, or grep `dir` for the
   command or the key. If no page covers it, write one from the template below. A
   page ends with one `Decided in:` line naming the epics whose decisions it carries;
   put this epic's id on it
4. Commit on that branch, before the merge — the pages then ride the same merge, and
   the merge diff is where they get read

       git add -- <wiki dir>
       git commit -m "docs(wiki): <what changed> (<epic>)" -- <wiki dir>

   `<wiki dir>` is `dir` from `moai wiki ls --json`. **Do not skip the `add`**: with a
   path, `git commit` leaves out a file git does not know yet, so a new page stays
   behind without a word
5. Name the pages you changed in your report, or say that none changed

## Lessons

A lesson is a mistake written down so the next window does not make it. It is a wiki
page of its own, `lessons/<slug>.md` below `dir`, one lesson to a file — so lessons
written on two branches never meet in the same lines. There is no index page: the list
is `moai wiki ls`, the search is `moai wiki search`. `moai show <id>`
puts the lessons that bear on a row under its `Wiki` block (`lessons` in `wiki`, and
`lesson: true` on a search hit); read them when you pick the row up.

**Write one when**, and only when:

- a review finding was one a single rule would have stopped
- a person corrected you — so they never have to say it twice
- the same hook refusal caught you twice
- a test failed because of a premise that was wrong

**Search first**: `moai wiki search '<a word of the mistake>'`. If the lesson stands
already, write nothing new — it was met again, and that goes on the issue, not the page:

    moai note <id> 'Lesson: lessons/<slug>'

If none stands, write it. The same call makes the page and then puts the note on the
issue:

    moai wiki learn 'End a check on the step that must pass' --issue <id> -b - <<'LESSON'
    ## What happened
    ...
    ## Why
    ...
    ## Do this instead
    ...
    LESSON

- The title is one line, as an instruction. The body must hold `## Do this instead` —
  it is the part the next window reads. `## What happened` and `## Why` are worth a
  few lines. Do not tell it as a narrative: a list, as in an issue body
- `--slug` names the file in lowercase ASCII kebab case when the title is not ASCII; `--issue`
  names the row it came from — pass it every time: without it the note goes to the one row
  held under your name, and sessions sharing one git identity hold each other's rows. `--dry-run` shows
  the page first
- Nothing is committed. The page rides the branch like any other: commit it with the
  same `docs(wiki):` commit as the end of an epic, `add` first
- A page already standing is never overwritten. Edit the file if it is wrong

**Do not write**:

- a failure that was the machine's — a binary that was not installed, credentials that
  were missing
- a flat "this tool does not work" — a negative claim goes stale the day the tool is fixed
- a transient error that a retry cleared
- a story tied to one PR or one error string
- logs or code pasted whole
- what CLAUDE.md, AGENTS or the wiki already say

**The page is never marked when a lesson is met again** — the note on the issue is the
mark, so two branches never edit one line. How often a lesson stood is read from those
notes: `seen_count` on each page of `moai wiki ls --json` (`0` for a lesson nobody has
noted), and `seen` on `moai wiki show lessons/<slug> --json` — one entry per issue,
newest first, each with the issue's `id` and the day (`at`) and person (`by`) of its
latest `Lesson:` note.

## When a person asks you to sweep

A person calls this skill to catch the wiki up — before a release, or after work
merged without touching it.

1. Find when the last release went out — `git tag --sort=-creatordate` lists the tags
   newest first; take the newest release tag, and `git log -1 --format=%cs <tag>`
   gives its day. Not `git describe`: it sees only the tags this branch can reach,
   and a release tagged on another branch (a `main` that only releases merge into)
   is not one of them. With no tag, ask the person how far back to go
2. List the epics closed since the day before that, and the issues closed outside any
   epic — `%cs` is the day on the tag's own clock and `--done` reads yours, so a day
   earlier keeps what closed in between; one row too many is only one more to ask about

       moai show --type epic --done '<day>..' --json
       moai show --type issue -e none --done '<day>..' --json

3. Ask each of them the question from the end of an epic (step 2 there), against
   the pages `moai wiki ls --json` lists, and gather what to change — one line each:
   the page, what changes in it, which epic or issue
4. **Show the person that list once** and wait for a yes. They may cut it
5. Pick the work up before you write — the pages are files in the repository, so
   the hook counts them as a change, by rule 2:
   "Pick something up before you change the repository"

       moai add 'wiki: <what>' -t docs
       moai mv <id> in_progress

   If you still hold other work, the hook refuses that `add`, by rule 1:
   "New issues stay inside what you picked up". The sweep is not part of that work, so
   finish it first or ask the person
6. Write the pages where this repository does its work — in a worktree if it uses
   them — commit with `docs(wiki): <what> (<id>)`, a `git add` first as in step 4 of
   the end of an epic, and move the issue to `done`
7. Sweep the lessons too. `moai wiki ls --json` gives each `lessons/…` page a `seen_count`;
   read the pages with `moai wiki show <slug> --json` (`seen`). Then
   - merge two lessons that say one thing into one page, and remove the other. A note that
     names the removed slug stays as it is — it is history
   - take out a lesson the code now prevents: the mistake can no longer be made
   - a lesson with a `seen_count` over three has outgrown a page. **Ask the person**
     whether it becomes a rule — in CLAUDE.md, in the AGENTS block, a hook or a test.
     Promoting it is the person's decision, never yours

## A new page

```markdown
# <Title — what the reader came to do>

<One paragraph: what this page is for and when to reach for it.>

## <A task>

<The steps in order. Each command in a code span, with what it is for — not its flags.>

## When it goes wrong

<What the reader sees, and what to do about it.>

Decided in: <epic>
```

## What this skill does not do

- **No gate.** Nothing checks that a page was written, and no warning stands for a
  stale one. Do not add a check — a check here is a gate, and a release does not
  wait on prose
- **No page per epic.** Pages follow what a person does, not the order things were
  built in. The history is the tracker and the CHANGELOG
- **No to-do list in the wiki.** Work not done goes into the tracker — `moai add`, or
  `moai backlog add` for later
- **No token counts.** What did the work and what it cost is a note on the issue,
  never a page — and never an estimate
