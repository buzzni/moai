---
name: moai-plan
description: Plan a request, split an epic, or unfold a backlog with the planning model. Use for "plan this", "계획 세워", and "에픽으로 나눠".
---

# moai-plan

The host interviews the person, then a planning subagent drafts a grounded plan. The moai CLI never launches an agent.

1. Read `moai models --json` and `moai writing --json`. Compare the `plan` identifier case-insensitively: a name containing `fable` qualifies; a name containing `opus` qualifies only when `effort_plan` is `xhigh` or `max`. All other names fail. On failure stop before interviewing or creating anything; print one line naming the keys to change (`plan` and, when needed, `effort_plan`), recommending `fable` or `opus` with `effort_plan = "xhigh"`. Identify the source from `written`, then `repo`, else default; the user config overrides repository recommendations. Never substitute a model automatically.
2. Inspect the repository, tracker and wiki for what can be learned without asking. Use `AskUserQuestion` to complete the interview: goal and done state, scope and exclusions, acceptance, protected areas and settled design, milestone (including explicit consent to join a running release or a new version), deadline and start, priority, review focus and risks. Ask follow-ups until decisions are settled. Never invent a start or deadline. No answer means no deadline.
3. Save the interview, including the person's answers verbatim, in a scratchpad file. Pass its absolute path, root and `Writing:` detail to the subagent. Call `Agent(subagent_type: "general-purpose", model: <plan>, effort: <effort_plan>, prompt: <draft brief>)` with this brief:

    Draft a plan from the interview record at <interview path> and the repository at <root>. Writing: <writing>. Work only in the scratchpad; do not write to the tracker or change repository files.
    Read the request, its evidence and enough code and tracker context to ground the split. Write short titles and concrete, actionable bodies with file and function locations, constraints, acceptance tests and review focus. For detailed writing use: Epic: why this bundle · the person's decisions · promises · order and shared files · review focus. Development member: goal · context (files and lines, precedent) · decisions · where and how (files, functions, structures) · important contracts and pitfalls · steps · acceptance tests · out of scope · order and shared files · role. Name the epic review section `Review focus / 리뷰에서 볼 것` (choose the English or Korean heading). For brief writing give what, where and tests. Classify each member using the difficulty rubric and explain why. Do not ask the person questions; report missing decisions as gaps.
    Write a plan file, an epic body file and one body file per member in the scratchpad. Preserve the mapping from each plan title to its body file. Return first `Model ran: anthropic/<your real model name>`, then `plan: <absolute file>`, `epic body: <absolute file>`, and for each member `<title> <low|medium|high> — <reason>; body: <absolute file>`. Return each unresolved decision as `gap: <what>`; do not ask the user.
    `low` — text, comments, a one-line fix; behaviour unchanged
    `medium` — a behaviour change inside one file, ringed by tests
    `high` — several files, the write path, concurrency, the storage format, hooks; hard to undo

4. Present the epic and member titles, difficulty and any `gap:` lines once. If gaps remain, ask only about those and resume the same `agentId` with the answers; the subagent never asks the person. Present the revised plan and obtain approval before writing the tracker. The person decides whether to attach a running milestone; omit `--milestone` without explicit consent. Encode dates in the plan or use `--due` and `--start` only as answered.
5. After approval, the host registers new work with `moai add --from <plan file> --body -` (stream the epic body), adding `--milestone <id>` only when approved. For a backlog use `moai backlog promote <id> --from <plan file>`; preserve its inherited milestone and body and add the approved epic details to it. Map returned member IDs to titles and apply each body with `moai edit <member> -b - < <body file>`. Record `model: anthropic/<Model ran> (<highest difficulty> — plan: interview and detailed plan)` on the epic, using the actual model name without repeating the vendor. Do not pick up the work.
