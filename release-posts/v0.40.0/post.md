## moai v0.40.0
### What's new
- **`moai read --atomic`** writes none of the named marks when any of the IDs is missing (`not_found`) or held by a value written by hand
- `moai ready`, `moai prime`, and the next-work suggestion from `moai mv … done` now follow archive-aware placement and deadlines, reversing the live-only rule from 0.38.0
### Fixes
- Explorer control actions now reflect person-only restrictions, run against the visible stats or wiki window, and report held keys in normal spelling
- Control proposals keep the selected ID stable, release disconnected waiter slots, validate time zones before display, and survive short web reconnects
- `moai due apply` skips the deadlines of closed epics and milestones, including ones whose finished members have moved to archive storage

Release notes: <https://github.com/buzzni/moai/releases/tag/v0.40.0>
Update with `moai update` · install: <https://moai.buzzni.com/guide/#install>
