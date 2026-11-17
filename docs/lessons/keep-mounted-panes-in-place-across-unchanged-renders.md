# Keep mounted panes in place across unchanged renders

## What happened
- A routine render reinserted panes that were already positioned within the correct container.
- Browser focus and scroll positions were reset even though the layout remained unchanged.

## Why
- Reparenting an existing DOM node can trigger browser state resets independently of node content.

## Do this instead
- Keep a pane mounted in place when its parent container and DOM order are already correct.
- Verify focus and scroll retention across unrelated store updates using real-browser tests in addition to DOM-level tests.

Learned in: moai-qm5t.5r3
