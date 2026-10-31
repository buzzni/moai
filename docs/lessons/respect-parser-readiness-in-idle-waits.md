# Respect the parser's readiness model when adding an idle wait

## What happened
- An explorer wait used level-triggered terminal readiness alongside a separate control notification socket, while crossterm's parser used an edge-triggered source.
- Input beyond crossterm's current read could leave the terminal readable while no parsed event was available. Repeated immediate poll returns then kept the owner loop busy.

## Why
- Raw descriptor readiness and a parser's next event are different observations. Wrapping an event parser with another readiness loop must preserve how it handles unread bytes and hang-up.

## Do this instead
- Check for buffered parser events before waiting, and keep the parser as the only reader of terminal input.
- When readiness remains asserted without a parsed event, wait out the remaining interval on the independent wake source instead of polling the same terminal again in a tight loop.
- Test persistent readiness with no parsed event, hang-up, successful parsing, and notification interruption; also test the real owner loop so a correct helper cannot hide a broken call site.

Decided in: moai-4km9
