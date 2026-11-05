## moai v0.51.0
### What's new
- Sort and group in one window: `SPC s` in the explorer and the Sort button on the web open the same presets (Release, Urgent, Recent, Due, People), grouping chips, and a per-layer order for milestones, epics, and work
- Group work by status, assignee, priority, tag, kind, milestone, or epic with `--group`, `moai ctl group`, or the window; `--sort` takes layered expressions such as `milestone=created:desc;item=priority,due`
- New sort fields: kind, milestone, epic, tag, started, done, starts, since, and progress
- The default order is per layer: milestones by due date then newest, epics by priority, work by priority then due date. Empty values always sort last
- New wiki page: analyse the tracker with SQL (DuckDB)
### Fixes
- Rows pointing at a missing epic or milestone group the same way in `moai show` and the explorer
- The web reads less when you only change the sort or grouping, and a manual refresh always fetches fresh data

Release notes: <https://github.com/buzzni/moai/releases/tag/v0.51.0>
Update with `moai update` · install: <https://moai.buzzni.com/guide/#install>
