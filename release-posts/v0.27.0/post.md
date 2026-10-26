## moai v0.27.0
### What's new
- **`moai wiki ls --json` and `moai wiki search --json` carry `dir_abs`**, the wiki directory as an absolute path in this checkout, beside `dir`
- **The explorer's detail and the `moai web` issue panel show the wiki about the issue**, after its commits
- **`moai edit --shift <±N>`, `--shift-start <±N>` and `--shift-due <±N>`** move a row's planned days by whole days, as a Gantt drag does
- **The wiki search in the explorer (`/` in `SPC g w`) and in `moai web`'s wiki tab puts pages in `moai wiki search`'s order**
- **The web Gantt's drag writes the move, not the days**
- **The web Gantt judges lateness in the zone it draws in**
- …and 2 more in the release notes
### Fixes
- **`moai note` takes an issue `moai archive` moved away**, and so does `moai wiki learn --issue`
- **`moai note` and `moai wiki learn` no longer pass a note the journal lost as written**
- **`moai wiki learn` writes the title so it reads back as given**
- **`moai wiki learn` takes a lesson that starts with `-`** when it has a space in it
- **A `moai wiki learn` killed halfway no longer leaves a page blocking that name**
- **`moai wiki search` and the wiki in `moai show <id>` no longer repeat a page's title as its heading** when the title line carries markup

Release notes: <https://github.com/buzzni/moai/releases/tag/v0.27.0>
Update with `moai update` · install: <https://moai.buzzni.com/guide/#install>
