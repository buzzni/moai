# Drive state-change tests from the actual initial values

## What happened

- A settings regression test dispatched `hideDeferred: true`, then expected it to have been saved. That setting was already true in the initial state, so no change or write occurred.
- A prior assertion also treated omitted default settings as different from the same settings restored through normalization.

## Do this instead

- Read the initial state before choosing an action that must cause a transition; assert that the value actually changes.
- Test restored behavior through the normalizer when missing keys represent defaults. Check raw storage separately for meaningful distinctions such as deletion, explicit null, and preserved unknown keys.

Learned in: moai-ijjd.46f
