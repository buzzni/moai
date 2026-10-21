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
- **jsdom is pinned to 30.1.1.** Upstream's lock had 30.1.1; on 30.1.3 three focus tests in
  `moai-keys-wiring.test.mjs` fail (184, 187, 193 on 2026-10-21). Raise it on purpose, with those tests read.
