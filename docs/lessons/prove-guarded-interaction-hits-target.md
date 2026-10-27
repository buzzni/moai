# Prove a guarded interaction hits its target

## What happened

- A mouse refusal test asserted that no drag started on a sibling row, but the same result would follow if its coordinates missed the bar entirely.
- The review added a local bar as a positive control using the same coordinate calculation, and verified that pressing it starts a drag.

## Do this instead

- Before asserting that a guard blocks a mouse or keyboard action, prove the same input reaches and starts that action on an allowed target.
- Keep the positive control in the regression test, so a layout or input change cannot silently turn the refusal into a missed interaction.

Learned in: moai-724f.p03
