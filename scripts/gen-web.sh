#!/usr/bin/env bash
# `src/web/dist/` 를 `web/` 에서 짓는다 — `moai web` 이 내는 페이지다.
#
#     scripts/gen-web.sh
#
# 번들은 커밋한다. `cargo build` 가 node 없이 돌아야 해서다. 그래서 소스와 번들이 갈라질 수
# 있고, 갈라지면 `tests/web_bundle.rs` 의 `the_web_bundle_is_built_from_these_sources` 가
# (node 없이) 잡고, CI 의 web 잡이 다시 지어 diff 로 잡는다.
#
# `npm ci` 라서 `web/package-lock.json` 그대로의 판으로 짓는다 — 같은 입력이면 같은 바이트다.
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root/web" && npm ci --no-audit --no-fund && npm run build
printf 'gen-web: %s\n' "$root/src/web/dist"
