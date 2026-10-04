---
name: moai-wiki
description: Use when the repository's manual (its wiki) has to catch up with what the work changed — at the end of an epic, or when a person asks. "update the wiki", "write the manual", "document this", "위키 갱신", "매뉴얼 써", "문서화해 줘", "wiki 정리". The pages are markdown files in the repository, listed by `moai wiki ls`; nothing checks them.
---

# moai-wiki — keep the manual in step with the work

The wiki is the repository's manual: markdown pages committed beside the code. It
follows the work — **when an epic changes what a person does, the page that teaches
it changes in the same merge.** Nothing checks this and nothing is blocked. An epic
that changed nothing a person does writes nothing.

## What the wiki is

    moai wiki ls                           the pages: slug, title, path
    moai wiki ls --json                    the same, for a machine
    moai wiki show <slug>                  one page

- **One directory.** `dir` in `moai wiki ls --json` names it — `docs` unless
  `wiki_dir` in `.moai/config.toml` says otherwise. When it cannot be read, `error`
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
- **An issue is named by its bare id**, as a whole word — not a link. moai finds it
  in prose and in inline code; an id inside a fenced or indented code block is read
  as an example and is not counted
- **The pages ride the branch.** Unlike `.moai/`, nothing moves them to the main
  checkout: a page written in a worktree stands in that worktree and is merged with
  the code, by git's own 3-way merge

Every row under `pages` also says what to fix: `issues` (each `id`, and `exists`
false for an id that names no issue), `links` (the links to other pages — each `text`,
`to` as the slug it lands on, and `resolved` false when no such page stands) and
`conflict` (true while merge conflict markers stand in the page). A page that could
not be read still stands in the list, under its file name, with an `error` of its own
whose `kind` says why — `too_large`, `refused` or `failed`. Branch on `kind`, not on
the words beside it. Look at all of these after you write.

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
  `moai idea add` for later
- **No token counts.** What did the work and what it cost is a note on the issue,
  never a page — and never an estimate
