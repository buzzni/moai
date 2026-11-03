## moai v0.45.0
### What's new
- Pictures now draw inside tmux on kitty and Ghostty, over SSH too — set tmux `allow-passthrough all` (tmux 3.4+); a Ghostty client reported as `xterm-256color` also needs `terminal-features ',xterm-256color:RGB'`
- Plan date ranges accept `～` and `〜` alongside `~`
- `moai edit --if-start/--if-due` supports atomic date preconditions
- `ctl state.menu` uses the same string/null key spelling on the explorer and web, and proposal refusals share translated words
- Deadline-priority and overflow calculations retain the archive placement context they need
- Supervisor backlog plans share the decision and contract guidance used by `moai-plan`
- …and 2 more in the release notes
### Fixes
- Explorer read-mark retries redraw only when unread markers or the displayed notice change, and terminal resizes wake the idle explorer promptly
- Proposal acceptance reserves the suggestion before its follow-up runs, on the web server and in the explorer, so an expiry during the action no longer…
- `init` preserves merge-driver settings when a shell wrapper may have relayed a signal exit
- Hook pickup suggestions ignore stale archived copies of unreadable live rows, matching `ready` and `prime`
- Gantt undo targets its original tracker and refuses stale absolute restores
- Image placeholder paths remain clickable across wrapped lines and when their labels scroll offscreen

Release notes: <https://github.com/buzzni/moai/releases/tag/v0.45.0>
Update with `moai update` · install: <https://moai.buzzni.com/guide/#install>
