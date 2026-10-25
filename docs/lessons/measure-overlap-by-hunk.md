# Measure overlap with a sibling branch by hunk, not by file name

## What happened

A brief said not to touch a file the work running alongside held. One line in that file had to change or the tests stayed red, and stopping would have left a branch that could not merge. (moai-mxvn)

## Why

The rule exists so two branches do not change the same place and wait on each other at the merge. A file name is a coarse stand-in for that; the real boundary is the changed lines.

## Do this instead

- First decide whether the line has to change for the tests to pass; prose and comments that do not can wait for a member of their own
- Measure the sibling branch's hunks: `git diff --stat <base>...<sibling>` and `git diff <base>...<sibling> -- <file> | grep '^@@'`
- If the ranges overlap, create a member and leave it in the first column; if not, change only that line
- In the report, give both line ranges so the other side can see why they do not touch
