# Make each regression test fail when its reader reverts

## What happened

- The epic-end review reverted individual default-fold readers and removed the spinner background branch. Several tests stayed green even though their names claimed to cover those paths.
- The fixtures used closed epics that now start open, or actions that became no-ops. They did not reach the selected milestone, overview header, or old band shape whose behavior mattered.

## Why

- A passing test can exercise the screen without exercising the changed condition. A shared predicate makes this easy to miss when one reader retains an old rule.
- Comments and test names can preserve an old premise after the behavior changes.

## Do this instead

- Name every reader of a changed rule, then temporarily restore the old condition or remove that reader's branch one at a time. Require a targeted test to fail for each behavior that must remain.
- Build fixtures with both sides of the condition and assert the exact state after the action: selected and unselected groups, a closed milestone and a closed epic, or foreground and background color paths.
- Rewrite test names and comments when their old premise no longer exists. Restore each mutation byte for byte before continuing.

Learned in: moai-i07x.t56
