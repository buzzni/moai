# Preserve the reduction order of derived group state

## What happened
- A priority ceiling converted closed group rows directly into an ID set.
- With duplicate group IDs and differing deferrals, any closed row could suppress the deadline even when the board's last row read open.

## Why
- The board reduces group state into a map in file order. A set of positive results discards the later negative result and answers a different question.

## Do this instead
- When reusing a derived state, preserve its key and reduction order, including negative results.
- For duplicate group IDs, reduce every eligible row in file order before selecting the closed IDs; check the same final row used by ancestor lookup.

Learned in: moai-w30o.m70
