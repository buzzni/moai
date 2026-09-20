#!/usr/bin/env bash
# 바이너리가 15MB 예산 안에 드는지 잰다.
#
#     scripts/check-size.sh target/release/moai
#
# 예산은 이 도구가 지고 가는 것이다 (CLAUDE.md 의 표). 재는 자리가 둘이라
# (PR 의 `size-budget`, 태그의 `build`) 자를 양쪽에 적으면 갈라진다 — 실제로
# 한쪽만 "lto 를 낮추지 말라" 는 말을 달고 있었다.
set -euo pipefail

limit=${MOAI_SIZE_LIMIT:-$((15 * 1024 * 1024))}
bin=${1:-target/release/moai}

[ -f "$bin" ] || {
  printf 'check-size: %s 가 없다\n' "$bin" >&2
  exit 2
}

# `stat` 의 인자가 리눅스와 macOS 에서 다르다. `wc -c` 는 양쪽에서 같고, BSD 가
# 붙이는 앞의 빈칸만 걷으면 된다.
size=$(wc -c <"$bin" | tr -d '[:space:]')
printf '%s  %s bytes / %s bytes\n' "$bin" "$size" "$limit"
if [ "$size" -gt "$limit" ]; then
  printf '::error file=Cargo.toml::바이너리가 예산을 넘었다 (%s > %s bytes). lto 를 낮추거나 codegen-units 를 올리지 말고 무엇이 들어왔는지 먼저 본다 — 예산이 먼저다.\n' "$size" "$limit"
  exit 1
fi
