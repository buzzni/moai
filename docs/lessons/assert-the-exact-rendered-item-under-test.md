# Assert the exact rendered item under test

## What happened

- A task-list test checked that `[ ]` remained in the rendered output for an empty task marker.
- Another ordinary list item also contained `[ ]`, so the check could pass even if the empty-marker item disappeared or changed.

## Why

- A substring shared by unrelated output does not identify the element whose behavior the test claims to protect.

## Do this instead

- Assert the complete rendered item, or select that exact element before checking its text and attributes.
- Include nearby items with similar text so an assertion cannot accidentally be satisfied by a different item.
- Check that changing the intended item's behavior would make the assertion fail.

Learned in: moai-2g1f.u7c
