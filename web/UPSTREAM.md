# Upstream

The page `moai web` serves started as the Saycode Desktop board panel, version 2.0.

- Repository: `buzzni/saycode-extensions`
- Commit: `90f545bd71fdbb1696a9ac636a8305a963dbe475` (branch `milestone/se-9inv`, 2026-10-15)
- License: MIT, `Copyright (c) 2026 Buzzni Inc.` on both sides — same owner, same license

## What came over

| Upstream | Here |
|---|---|
| `packages/moai/panel/**` | `web/panel/` |
| `packages/moai/build.mjs` | `web/build.mjs` |
| `tests/moai-*.test.mjs`, `tests/moai-panel-modules.mjs` | `web/tests/` |
| `tests/fixtures/moai/`, `tests/fixtures/moai-wiki/` | `web/tests/fixtures/` |
| root `tsconfig.base.json` + `packages/moai/panel/tsconfig.json` | `web/tsconfig.json` (merged) |
| root `package.json` devDependencies | `web/package.json` |

Left behind: the Desktop extension (`packages/moai/src/index.ts`, `extension.json`, `README.md`), the packed
`*.saycode-extension`, the SDK and the test host.

The first commit on this side (`chore(web): Saycode 패널 2.0 을 saycode-extensions@90f545b 에서 그대로 들인다`)
is those bytes unchanged, so every later diff is what was changed for moai.

## A one-way fork

Nothing flows back. To bring an upstream fix over, read

    git diff 90f545bd..<new> -- packages/moai/panel

and carry it across by hand, then move the commit above.

## What changed

- **Build.** `build.mjs` writes `src/web/dist/app.js`, `app.css`, `index.html` and `SOURCES` instead of one
  inlined `dist/panel.html`: the server's CSP allows scripts from its own origin only. The 512 KiB limit was
  Desktop's and is gone; `tests/web_bundle.rs` holds the bundle to 1 MiB.
- **Tests.** `moai.test.mjs` (packing the extension) and `moai-panel-size.test.mjs` (512 KiB) did not come
  over. The parts of `moai-source.test.mjs` that packed the extension and read its manifest are gone.
- **The bridge (moai-jshy.jih).** `window.saycodePanel` and its machine runs are gone; `data/client.ts` fetches
  `moai web`'s endpoints and reads the envelope (`{exit, out, err, said}`). Desktop's refusal codes, confirmation,
  busy retries, compatibility probe (`data/compat.ts`), `prime` fallback, archive paging and the clock hold are gone
  with it. Read marks (`r`, `SPC m a`, `SPC m g`) are gone — the page only reads; NEW still shows. The words come
  from `/api/boot`'s `lang`.
- **The second set of rules (moai-jshy.hfr).** The panel had rebuilt moai's `nav.rs`, `query.rs` and parts of
  `report.rs` in TS. Those are gone; the page reads what moai gives:
  - where a row stands, whether it opens, its `n/n` and the order of every listing: `/api/tree`
    (`tui --json --deep`, its `home`, `dir`, `progress`). The index keeps only the inverse image of `home`; the
    steps of a path keep their `m:`/`e:`/`i:` words, read from the kind of the row a handle names
  - deferral, epic, milestone, the column of a group: `shelved_by`, `derived_epic`, `derived_milestone`,
    `derived_status`
  - the archive: a row no list printed (`show --all` hides it by `archive_days`); `SPC v o` reads the lists with
    `archived=1`. The hard-wired `ARCHIVE_DAYS = 14` is gone
  - the filter, the search and the order: sent as typed (`filter=`, `q=` → `--grep`, `sort=`); moai's refusal is
    shown under the box in its own words. The search scope select is gone (moai has none; `--grep` reads notes too).
    `due` is a sort field (`SPC s w`, as the explorer) and `due_on` a filter key — both from `/api/boot`
  - the board's columns in the config's order (`statuses`), its glyphs (`glyphs`, `columns`: the deferred column
    is `‖`, not `⏸`), the per-column counts of a group (`status --json`)
  What stays TS is drawing: the board column of a card (`tui::board::column_of`), the filter box's completion,
  the wiki anchors (`src/wiki.rs` gives none in JSON).
  The test fixtures are this binary's answers (`scripts/gen-web-fixtures.sh`, held by
  `the_web_fixtures_match_the_binary`); the v0.16/v0.17/v0.8 ones are gone.
- **jsdom is pinned to 30.1.1.** Upstream's lock had 30.1.1; on 30.1.3 three focus tests in
  `moai-keys-wiring.test.mjs` fail (184, 187, 193 on 2026-10-21). Raise it on purpose, with those tests read.
