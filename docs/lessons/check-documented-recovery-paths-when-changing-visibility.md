# Check documented recovery paths when changing visibility

## What happened
- A live-first display fix hid archived twins from the documented command for comparing conflicting copies.

## Why
- Counting context and inspection output serve different purposes; removing a row from both can remove a recovery path.

## Do this instead
- Check the manual and test how a person inspects conflicting versions before changing visibility. Keep recovery access distinct from the rows used for counts.

Learned in: moai-b2jt.s2z
