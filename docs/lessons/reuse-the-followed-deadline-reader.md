# Reuse the existing derived deadline reader

## What happened
- A new sort calculated an inherited deadline from the write-validation ceiling.
- The existing ready and due-apply paths instead exclude closed ancestor groups and completed parents, so a past deadline could incorrectly promote follow-up work.

## Why
- The two readers answered different questions with similarly named values. Comparing the earliest dates alone missed the closure rule.

## Do this instead
- Reuse the report layer's followed-deadline reader and pass the caller's derived group columns; do not rebuild the ancestor rule in another surface.
- Test a closed group with a past deadline and an open follow-up, then open the group and show that the same deadline is inherited again.

Learned in: moai-ctxc.5bm
