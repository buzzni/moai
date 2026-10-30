## moai v0.37.0
### What's new
- Explorer `SPC o o` chooses the archive retention and writes `archive_days` to the shared repository configuration
- Explorer board milestone lane headers can be selected and folded; closed lanes and epic cards start folded
- Web list and Gantt fold closed groups by default, and web board milestone lanes and closed epic cards can be folded
- Explorer list `Tab` and Gantt entry leave closed epics and milestones folded
- Archive hiding and `moai archive` now wait for a row's parent, epic and milestone to reach archive age before removing the row from the active view or snapshot
### Fixes
- Explorer rows already moved to archive storage stay hidden when `archive_days` changes

Release notes: <https://github.com/buzzni/moai/releases/tag/v0.37.0>
Update with `moai update` · install: <https://moai.buzzni.com/guide/#install>
