# Keep a person-opened descendant open when expanding its ancestor

## What happened
- In the web list, Tab removed closed groups a person had opened from the expanded set; review finding 6 corrected it.
- In both Gantt views, reopening a folded ancestor with Tab reset a manually opened closed descendant; review finding 15 corrected it.

## Why
- Expand-all code treated the descendant's default closed state as an instruction to overwrite the person's current fold choice.
- A test that only checked the initial closed state missed the sequence: open descendant, fold ancestor, reopen ancestor.

## Do this instead
- Seed a closed group's default fold once when entering a view. After that, expanding an ancestor preserves each descendant's current fold state, including a closed group the person opened.
- Test the full interaction sequence in each surface that implements the fold: manually open a closed descendant, fold its ancestor, then expand the ancestor and assert the descendant stays open.

Learned in: moai-mkzo.8wa
