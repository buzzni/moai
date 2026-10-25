# Ask whether the value belongs there before offering ways to keep it fresh

## What happened

A stale computation after a time-zone change was put to the person as two options, both measured: drop the cache when the zone changes, or stamp each row with the zone it was counted in. The person picked neither — storage is UTC, so nothing should change with the zone; apply the zone only when drawing. Doing that removed the fork itself, along with three places that copied the zone across threads. (moai-fgjj)

## Why

Both options kept the premise that the value was folded into the cache. That premise was the bug, so either option would have left room for the same kind of leak.

## Do this instead

- Before asking which way to invalidate or sync a value, write one line on whether the value belongs there at all
- Push an existing principle (here: settings only change what is drawn) into the computation too — that is often the third option
- Put that line in front of the options when you ask
