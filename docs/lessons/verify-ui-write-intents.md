# Verify UI write intents through the server

## What happened

- A Gantt drag test asserted that the view sent a request, although the real server would reject the requested date.
- Read-only assertions also claimed to protect persistence without exercising a write.

## Do this instead

- When a UI write intent changes, test the real endpoint and the stored result, including any concurrency rule and undo behavior.
- Keep mocked callbacks for drawing and interaction checks; a captured callback alone does not prove that a write succeeds.
- Put persistence assertions after an actual write or undo so that reverting the behavior can make them fail.

Learned in: moai-e9hi.442
