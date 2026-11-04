**3. Send.** Send **one** backlog to one worker. The message is the lines below, every
slot filled. The worker knows nothing of this conversation, so what is not in the message
does not reach it — except the worker's steps: they stand in this skill's
`references/worker.md`, and the first line tells the worker to `Read` that file and follow
it. Its rare branches stand beside it in `references/worker-branches.md`, and the worker
reads the section it needs by itself — you name only the first file. The worker stands in this same repository with the same plugin loaded, so the file is
there for it. **Do not copy the file into the message** — name it.

To a subagent the message is the `prompt` of the `Agent` call in 2, with `Worker: subagent`.
To a session it goes with `Worker: session`:

    SendMessage(to: <worker>, message: <the lines below>, notify_when_idle: true)

`notify_when_idle` is for a session only — a subagent's end comes as a notification by itself.

`<steps file>` is that file's absolute path: this skill's base directory — Claude Code shows
it as "Base directory for this skill" when the skill loads — followed by
`/references/worker.md`. Write it out whole; `@path` attaches nothing.
**If that base directory lies outside `<root>`** — the plugin installed at user scope, in
Claude Code's plugin cache — the worker's read of it is a read outside its working directory,
and in the default permission mode Claude Code asks its person first; the plugin cache is no
exception. A worker whose person is away waits on that prompt and sends nothing. Tell the
person once, before the first send, and let them choose: a person in the worker's window
answers it, or `permissions.additionalDirectories` in their settings holding that plugin
directory lets it through. The settings are theirs — do not write them.
Inside tmux, label the worker's pane as you send and, if the message does not arrive, deliver
it into the pane — `moai-tmux`, "Label the pane" and "When a message does not arrive".
Inside cmux, label the worker's tab as you send, once the person said yes to labels this round,
and if the message does not arrive, deliver it into the tab —
`moai-cmux`, "Label the tab" and "When a message does not arrive".
Inside Saycode, a worker Saycode lists gets the message as a prompt instead of `SendMessage` —
`moai-saycode`, "Send work".
Fill in `<id>`, `<title>`, `<steps file>`, `<root branch>`, `<base branch>`, `<milestone>`, `<due>`, `<model>`, `<difficulty>`, `<why>`, `<effort>`, `<worker kind>`, `<subagent ladder>`, `<writing>`, `<other work>`, `<root>`, `<person>` and — only for a subdirectory project — `<subdir>`.
Copy `<writing>` from this round's `moai writing --json` `detail`, without choosing it yourself.
`<root>` is the `root dir` from 2. **Leave it unfilled** and a worker that stands in a
worktree reads its own place as the root. With no `subdir` line in 2, leave the `Subdir:` line out
of the message.
`<milestone>` is the release that backlog already stands under **and that is still alive**,
read in 1 — `none` when it stands under none, `none` when the one it stands under has
shipped or been deferred (the worker would otherwise re-open a release that is already
out, which is what `promote` itself declines to carry), and `none` when nothing is
running. It is never a release you picked for it: work is not pulled into a running
milestone (1). **Leave it unfilled** and the worker hangs the placeholder itself on the
epic, which the tool refuses because it is not an id at all. **A wrong id it does not
refuse** — the check is the shape, not whether that milestone stands, so a stale one goes
in with exit 0: one line on stderr says there is no such milestone, and `moai status`
counts the epic as `dangling_milestone`. Copy it off
the release `moai show --milestone` stands that backlog under; do not write it from memory.
`<due>` is when the person wants this work done, as `YYYY-MM-DD` — the backlog's own
`due_on` when it carries one (`promote` carries that onto the epic anyway), else the day
the person gave when you asked. **Ask when it is due as you pick work whose backlog
carries none** — never required, always asked; the backlog's own is an answer the person
already gave. A day given as "in three days" you write as the day it lands on.
It is `none` when neither stands — with `Person: away` there is nobody to ask, so it is
the backlog's own `due_on` or `none`: a deadline is the person's, and you never make one
up. The plan subagent (2-2) — or a worker you send the backlog itself — hangs it on the epic it
unfolds; the epic's deadline then bounds every member's, and `moai due apply`
raises the members' priority as it nears.
`<model>`, `<difficulty>` and `<why>` are the role's model you read in 2-1, the difficulty
you measured and your reason; `<effort>` is that role's effort (2-1), the same value you pass
to `Agent`. **Leave them unfilled** and those placeholders travel as they are, so the note
left at the close says `<model>` instead of what actually did the work.
`<worker kind>` is `subagent` or `session` — how you raised it in 2.
The `Worktree:` line goes as written to both: a session's `EnterWorktree` into
`.worktrees/` asks its person to approve every call, and no allow rule turns that off.
`<subagent ladder>` is the models the worker may hand pieces of the work to, copied from the
`moai models --json` of this round, each role with the effort 2-1 reads for it —
`bulk <model> (<effort>) · develop <model> (<effort>) · complex <model> (<effort>)`, the
person's table as it stands. The worker does not read the table again. It is `none` for a Codex
worker — `Agent` is Claude Code's — and otherwise only when the person said the workers do
their work themselves; a word of your own about subagents does not go in the message beyond
this line.
Do not use a single quote inside `<why>` — it closes the single quote in the worker's 9-1
and the rest of the text leaks into the shell.
`<other work>` is the sibling worktrees you measured in 1, the work you send in this same
round, and the files that work holds — `none` if there is none. What the supervisor
measured before sending cannot cover a file that turns out to be needed mid-epic, so when
the worker meets such a file it does not fix it: it leaves it as a member and reports it
(its 4-3).
`<person>` is `here`, or `away` when the person told you they are stepping away — the
worker then settles a design question by its own recommendation instead of waiting on an
answer, writes down what it decided, and stops at what cannot be undone.
Do not fill `<grade>` — that is the review grade the worker picks in 7, after developing.
Do not fill `<vendor>` either — it belongs to the `model:` line written at the close (5, and
a session worker's 9-1).

    You are handed backlog <id> — <title>. Read <steps file> and follow its steps from step 1.
    Read first: moai show <id>
    Model: <model> (<difficulty> — <why>)
    Effort: <effort>
    Worker: <worker kind>
    Subagents: <subagent ladder>
    Writing: <writing>
    Work running alongside: <other work> — do not touch those files (4-3)
    Root branch: <root branch>
    Base branch: <base branch>
    Milestone: <milestone>
    Due: <due>
    Root: <root>
    Subdir: <subdir>
    Person: <person>
    Worktree: stay in the root and reach it by absolute path — never EnterWorktree, every call into .worktrees/ asks the person to approve it

