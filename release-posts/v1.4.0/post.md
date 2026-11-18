## moai v1.4.0
### What's new
- The explorer has a file window (`SPC g f`): the checkout's files as a tree with change marks (`M`·`A`·`D`·`R`) and syntax-coloured text, a checkout picker (`SPC w`), and a changed-files-only view that brings up the diff pane
- The explorer's list, board and gantt have a diff pane (`SPC v D`) showing an issue's changes, unified or side by side (`SPC o s`)
- The web interface has a file tab with the same tree, marks and colours, and an issue diff pane beside the detail; large diffs show every file's head at once and read each opened file on its own
- `moai diff <id>` reads an issue's commit, branch, or working-tree changes as a patch or JSON
- The web interface moves between panes with `SPC x …` instead of `SPC w …`

Release notes: <https://github.com/buzzni/moai/releases/tag/v1.4.0>
Update with `moai update` · install: <https://moai.buzzni.com/guide/#install>
