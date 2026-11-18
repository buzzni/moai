## moai v1.5.0
### What's new
- Subversion working copies: the explorer's file window and diff pane, the web Files tab and `moai diff <id>` now read changes and diffs from svn (1.9 or later) in all four scopes — `working`, `commits`, `branch` and `whole`
- When a checkout has both `.git` and `.svn`, git stays the default; pick svn with `moai diff --vcs svn`, `[vcs] backend` in `.moai/config.toml`, or `SPC w` / the web Files tab
- The web header drops the Tip line and puts the project list right beside User and Version, as the explorer does; the tips page opens from the help dialog

Release notes: <https://github.com/buzzni/moai/releases/tag/v1.5.0>
Update with `moai update` · install: <https://moai.buzzni.com/guide/#install>
