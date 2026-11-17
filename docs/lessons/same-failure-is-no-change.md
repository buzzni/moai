# Count the same failure again as no change

## What happened

The file window previously re-read its changed-files list only while in changed files only mode. Epic moai-p7nr extended the refresh loop to read it across the full tree as well to maintain change marks. However, the reply's "same as before" comparison evaluated two read lists while treating consecutive failures as new states. As a result, when viewing a checkout that git could not read, the window rebuilt its tree every two seconds and the git refresh pace never backed off (review moai-p7nr.zn7, finding 1).

## Do this instead

- When expanding the conditions under which a background read runs, check every branch of the reply's "nothing changed" logic, including the failure case: receiving the same error again constitutes no change.
- Verify this behavior with a test that supplies the same failure twice and asserts that no rebuild occurs while the background read continues.

Learned in: moai-p7nr.zn7
