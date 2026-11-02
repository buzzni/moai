# Pictures in issues and the wiki

Add a picture to an issue with `moai image add shot.png --to <id>`. The command prints a markdown reference; to put it in a note instead, use `moai note <id> "$(moai image add shot.png)"`. Pass several files together, use `--alt` to supply a description, or `--original` to keep the input bytes. `--json` reports the stored files and warnings.

Use `moai image add shot.png --wiki` for a wiki picture. With a nested page, give its slug, for example `--wiki lessons/example`: the printed reference is relative to that page. Paste it into the page yourself.

Issue pictures live under `.moai/images/` in the tracker root, even when added from a linked worktree. Commit them with the tracker. Wiki pictures live under the checkout's `<wiki_dir>/images/` and travel with the wiki branch. Names use the stored content's SHA-256 hash, so the same bytes are reused.

PNG and JPEG pictures are compressed by default, with a maximum long edge of 1920 pixels and JPEG quality 85. Transparent PNG remains PNG; an opaque PNG uses the smaller of PNG and JPEG. A small image is never enlarged, and if re-encoding without resizing would make it larger the original is kept. Re-encoding removes EXIF metadata, including GPS; originals retain it. GIF and WebP are stored unchanged. Files over 2 MiB produce a warning, not a refusal.

The repository's `.moai/config.toml` controls `image_compress = "on"` or `"off"`, `image_max_edge = 1920`, `image_jpeg_quality = 85`, and `image_warn_kb = 2048`. `--original` disables compression for one invocation.

`moai show` and `moai wiki show` show a picture in supporting terminals: kitty and Ghostty use kitty graphics, iTerm2 and WezTerm use iTerm2 graphics, and Windows Terminal, foot and mlterm use sixel. A terminal launched from a tmux shell can inherit `TMUX`; moai compares stdout with the server's pane ttys so that a separate kitty window still draws. A failed check keeps graphics off. Other terminals, including real tmux panes and screen, show a text placeholder and a clickable local file link. `MOAI_IMAGES=off|kitty|iterm|sixel|auto` overrides detection; the user setting `[images] draw` accepts the same values. Pipes, JSON and raw output contain no image controls. In the shell GIF and WebP always stay a placeholder with its link, because moai decodes only PNG and JPEG; the web view displays all four.

The explorer shows placeholders and local links. The [web view](web.md) displays issue and wiki pictures in the browser. Windows needs no additional image software.

Run `moai init` again in an existing repository to add the `**/images/.lock` ignore line for the shelf lock file. Existing repositories can keep working before doing so.

Decided in: moai-7eeq
