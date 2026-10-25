# Move every reader of a derived value, not only the ones the issue names

## What happened

A change moved a derived value — which epic a row belongs to — to a new rule. The six surfaces the issue named were all fixed, and the epic-end review found nine more places asking the same question: a `--milestone` filter, board warnings, the running-milestone door of `ready`, explorer counters. One of them silently handed out no work: `ready` reported nothing to pick up for rows `show <milestone>` counted as members. (moai-7iyc)

## Why

An issue names the surfaces someone looked at, which are the ones that count and draw. The readers that filter, warn and refuse ask the same question in their own words and are easy to miss. A value inherited along an axis (epic, then milestone) moves with it.

## Do this instead

- Before moving a derived value, `grep` every reader and sort them into four kinds: count, draw, filter, warn or refuse
- Move all four, then grep again to see that none is left on the old rule
- Where the value is inherited down another axis, move the inheriting readers too
