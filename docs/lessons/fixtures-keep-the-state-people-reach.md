# Do not let a fixture tidy the state the tool reads

## What happened

- The svn `branch` scope compared the fork point against the working copy's URL at the root's revision (moai-0d0s.x27).
- `svn commit` does not update the working copy root's revision, so a commit made from that working copy was missing from `branch` until `svn update` was run.
- The test fixture executed `svn update` after every commit, so every test ran against a synchronized root revision that a real working copy rarely has. The epic-end review (moai-0d0s.h81, finding 1) reproduced this discrepancy manually.

## Why

- A fixture step that normalizes state read by the tool masks the actual states users encounter.
- Subversion working copies are mixed-revision by default; the root revision does not reflect the latest commit.

## Do this instead

- Construct fixtures using the exact commands a user runs, in the order they run them; include normalizing steps (`svn update`, `git gc`, cache rebuilds) only in tests specifically designed to verify them.
- For each value read from a working copy, write a test where that value is stale and verify that the expected behavior still holds.

Learned in: moai-0d0s.x27
