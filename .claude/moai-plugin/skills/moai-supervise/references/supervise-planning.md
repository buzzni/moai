**2-2. Unfold with the `plan` role.** A backlog item goes to its worker already unfolded.
Splitting work into an epic and its issues is the `plan` role's (2-1) — a worker on `develop`
or `complex` would otherwise plan on its own model. So before you send a backlog item, raise one
subagent on the `plan` role's model and `effort_plan` on the brief
below, every slot filled as in 3:

    Agent(subagent_type: "general-purpose", model: <plan model>, effort: <effort_plan>, description: "plan <id>", prompt: <the plan brief>)

**Mark the backlog the moment you raise it** — `moai note <id> 'Sent: plan <agentId>'`, as 1 says
for a send: until the plan hands back, that backlog still stands in `moai backlog ls`, and a
second plan subagent raised for it unfolds it twice (`promote` unfolds a closed backlog too).
The word is `plan`, not `subagent` — no worker holds this backlog, so it is not stalled work.

    You unfold backlog <id> — <title> — for the supervisor of this repository. You split it
    into an epic and its issues; you do not develop it, pick anything up, raise a worktree or
    change a repository file — the tracker and your scratchpad are yours. Create `<scratchpad>/<backlog id>/` before writing any plan files. Use that directory until the epic exists; then create `<scratchpad>/<epic>/` and use it for all subsequent bodies and review files. Never share a task directory across concurrent work.
    Root: <root>
    Milestone: <milestone>
    Due: <due>
    Person: <person>
    Writing: <writing>

    Read the request, its evidence and enough code and tracker context to ground the split. Write short titles and concrete, actionable bodies with file and function locations, constraints, acceptance tests and review focus. For detailed writing use, for the epic: why this bundle · the person's decisions · promises · order and the files it shares · review focus; for a development member: goal · context (files and lines, the precedent) · what is decided · steps · acceptance (which tests) · out of scope · order and the files it shares · role, adding where and how (files, functions, structures) · important contracts and pitfalls after `what is decided`. Name the epic review section `Review focus / 리뷰에서 볼 것` (choose the English or Korean heading). For brief writing give what, where and tests. Classify each member using the difficulty rubric and explain why. Do not ask the person questions.
    Keep the person's decisions and exclusions, storage, concurrency, path security and output contracts, numbers and budgets, member order and review focus. Give entry-point files and functions as navigation hints; line numbers, proposed signatures and types are hints, not contracts. Describe acceptance tests by the failure conditions they must catch, not exhaustive test function names. Under `acceptance (which tests)`, write those failure conditions; test names are optional navigation hints. Make example commands executable with configured column names, the first column, spaces and shell quoting; when unknown use a placeholder such as `<first column>`. Put shared files and order in the epic once; a member's `order and the files it shares` names only its own place and files and points to the epic — do not repeat file lists or explanations in every member.

    1. Read it — moai -C "<root>" show <id> — and as much of the code as the split needs. If it
       already stands done, hand back `already unfolded: <id>` and stop: unfolding again puts up
       two epics.
    2. Write the plan: one `# <epic title>` line and one `- [pN] <member title>` line per issue. A plan
       line becomes the title verbatim, so write short new titles. Look first, then unfold:
       moai -C "<root>" backlog promote <id> --dry-run --from -
       moai -C "<root>" backlog promote <id> --from -
    3. Hang the release on the epic, then the deadline, each unless its slot says `none` — the
       release first, as a dead one's earlier deadline refuses the person's. `Milestone: none`
       clears nothing `promote` carried, but for a dead release: when a line `promote` printed
       names the release it carried as already closed or deferred, clear it — left on, it
       re-opens a release that is out, or takes the whole plan out of the plan:
       moai -C "<root>" edit <epic> --milestone <milestone>
       moai -C "<root>" edit <epic> --milestone none
       moai -C "<root>" edit <epic> --due <due>
       A refusal because the day is later than the milestone's leaves the epic as it stands.
    4. Give the epic a body, as `Writing:` says — `detailed`: the epic headings of the shared instructions above; combine it with the backlog context already carried onto it in a scratch file and write that combined body with `moai -C "<root>" edit <epic> -b - < <epic body file>`. Give every member a body the worker can start from, as `Writing:` says. `detailed`: these
       headings, each a list, with the two the shared instructions add after `what is decided` —
       goal · context (files and lines, the precedent) · what is decided · steps · acceptance (which tests) · out of scope · order and the files it shares · role.
       `brief`: what to change · where (file and function) · how to test it. A design question the
       backlog and its notes do not settle goes in it, open, for the worker to ask — do not
       settle it here. Write each body under `<scratchpad>/<epic>/`, using a distinct filename per member, and run
       moai -C "<root>" edit <member> -b - < "<scratchpad>/<epic>/<member>.md"
    5. Mark the epic, last — a supervisor that starts again sends an epic it finds marked so, so
       mark it only once 3 and 4 stand:
       moai -C "<root>" note <epic> 'Sent: plan <id>'
       Then hand back. The first line is Model ran: anthropic/<your real model name> — the name in
       your system prompt, not an alias. The second is epic: <epic id>. Then one line per member,
       <member> <low|medium|high> — <reason>, on this rubric:
       - `low` — text, comments, a one-line fix; behaviour unchanged
       - `medium` — a behaviour change inside one file, ringed by tests
       - `high` — several files, the write path, concurrency, the storage format, hooks; hard to undo
       Open the reason with `docs` for a member that only touches the wiki or the CHANGELOG, and
       with `bulk` for sorting, summing up or the same mechanical fix many times. Then a line
       `not hung: <what and why>` for each release or deadline 3 left as it was or cleared.
       If you could not unfold, the first line after Model ran: is `not unfolded: <reason>`.

It hands back the epic it unfolded and a difficulty per member. Then

- **write its `model:` line on the epic** — `moai note <epic> 'model: anthropic/<model ran> (<heaviest difficulty> — plan: unfolded <id>)'`;
  `<model ran>` is the name after `anthropic/` on its `Model ran:` line — copy the vendor once
- **measure the role (2-1) on the members it handed back**, not on the backlog — the members
  are written now: `docs` or `bulk` when every member's reason opens with that word, else the
  heaviest of them picks `develop` or `complex`
- **tell the person each `not hung:` line** — a deadline they gave that did not land, or a dead
  release the plan cleared off the epic; nothing else carries it to them
- **send that epic `from step 2`**: the message of 3 with its first two lines swapped for the two below,
  and the `Sent:` mark (1) on the epic. The backlog is done once unfolded and leaves the candidates by itself

      You are handed epic <epic> — <title>. It is already unfolded; do not promote. Read <steps file> and follow its steps from step 2.
      Read first: moai show <epic>

**When you start again (4)**, a backlog whose last `Sent:` is `plan <agentId>`, with no
`Plan stopped:` after it, lost its plan subagent with the round: once the person agrees that the
session it ran in is gone, raise a new one for it. An epic whose last `Sent:` is `plan <backlog id>`
— the plan marks it last, once the epic is whole — was unfolded and never sent: measure the role
from its members' bodies (2-1) and send it from step 2. Its plan's `model:` line went with the
notification; do not write one from memory.

If it hands back `already unfolded:` or `not unfolded:`, send nothing for that backlog — someone
unfolded it, or the plan subagent could not; read its reason and tell the person. On
`not unfolded:` also close the backlog's mark — `moai note <id> 'Plan stopped: <its reason>'` —
or every start again sends it to a new plan; it is a candidate again when the person says so. **A worker
you send a backlog to directly** (the message of 3 as it stands, `from step 1`) still unfolds it
itself — that is the path when the person asks for it, not the usual one.

