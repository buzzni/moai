# Codex workers — shared rules

The "Codex workers" section of `moai-tmux` or `moai-cmux` sends you here. The sections named below
("When a worker stalls", "What never happens") and the paste path are that skill's; `references/`
paths are this skill's (`moai-supervise`).

### Model

Read `moai models --json` once per round; `<codex>` is its `codex` value, configured by
the person under `[models] codex`. Default example: `gpt-6.1-sol`.
Once per round use `WebSearch` for `OpenAI Codex recommended models <codex>
site:developers.openai.com/codex site:learn.chatgpt.com`, then open the official models
page. Compare the configured name, Codex support and retirement notices. If it is outdated,
tell the person in one line; they change the config. A stronger model existing alone does
not make the configured model outdated. If the config or official page cannot be read,
tell the person; do not guess or change the value.

### Review and the second assignment

Codex tests and commits its work, writes `Review-ready:` as an epic note, and stops.
It waits for the supervisor's review and the second assignment; it does not review itself.

When `Review-ready:` arrives, read its whole note. Recompute the grade from `review_grade` and `review_raise` as the shared epic review rule says; if its proposed grade differs, use the configured result and record the correction and reason in the review issue angle. Pick the review issue up
(`moai -C "<root>" mv <review id> in_progress`) and run a review subagent in the reported
worktree with its configured `review` model and "The review brief" in
`references/worker.md`; refer to that brief, do not copy it here. Once the review has ended and its words are on the review issue, send
"merge and close — review <id> done" with the same file-paste path and fresh input checks.
Leave the review issue open: Codex takes the fixes in and closes it (`worker.md` step 7 and worker.md 10).
Do not clear the conversation between these two assignments. Watch for the final report,
then apply the supervisor's 5 and write `Report-checked:` after the checks pass.

When Codex stalls, follow "When a worker stalls": read that named tab or pane once, tell
the person what stands there, and leave permission prompts and questions to them. The note
monitor is the only repeated read; never poll the screen or discover sessions in a loop.
All of "What never happens" applies to Codex too: no closing, focus changes or typing over
the person's words. Labels do not grant permission to type.
