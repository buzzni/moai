#!/usr/bin/env bash
# `docs/cli.md` 를 `--help` 에서 짓는다.
#
#     cargo build --release && scripts/gen-cli-docs.sh
#
# 레퍼런스를 손으로 적으면 도움말과 갈라진다 — 갈라진 레퍼런스는 없는 것보다
# 나쁘다. 읽는 사람이 그것을 믿고 치기 때문이다. 그래서 글은 한 곳(`--help`)에만
# 있고 이 파일은 그것을 옮겨 적는다.
#
# 갈라지면 시험(`the_cli_reference_matches_the_help`)이 잡는다.
#
# 화면 말을 못 박는 것은 산출물이 기계마다 달라지지 않게 하기 위해서다. 기본
# 화면 말이 바뀌는 날 여기도 같이 바꾸고 다시 짓는다.
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
moai=${MOAI:-$root/target/release/moai}
out=${1:-$root/docs/cli.md}

[ -x "$moai" ] || {
  printf 'gen-cli-docs: %s 가 없다 — cargo build --release 를 먼저 돌린다\n' "$moai" >&2
  exit 2
}

export MOAI_LANG=ko NO_COLOR=1 COLUMNS=80

# `Commands:` 칸의 이름만 집는다. `help` 는 clap 이 저절로 붙이는 것이라 뺀다.
subcommands() {
  "$moai" "$@" --help | awk '
    /^Commands:/ { on = 1; next }
    on && /^[^[:space:]]/ { exit }
    on && /^  [a-z]/ && $1 != "help" { print $1 }
  '
}

walk() {
  printf '\n## `moai %s`\n\n```\n' "$*"
  "$moai" "$@" --help
  printf '```\n'
  for sub in $(subcommands "$@"); do
    walk "$@" "$sub"
  done
}

{
  cat <<'HEAD'
# CLI reference

**Generated from `moai --help`. Do not edit by hand** — run
`scripts/gen-cli-docs.sh` after `cargo build --release`, and commit the result.
A test compares this file against the binary's own help, so a reference that
drifts fails the build rather than misleading a reader.

The screen language is pinned to `ko` here, which is what the tool defaults to
today. Pass `MOAI_LANG=en` at runtime for English.

## `moai`

```
HEAD
  "$moai" --help
  printf '```\n'
  for sub in $(subcommands); do
    walk "$sub"
  done
} >"$out"

printf 'gen-cli-docs: %s\n' "$out"
