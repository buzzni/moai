#!/usr/bin/env bash
# `web/tests/fixtures/moai/*.json` 를 이 바이너리로 짓는다 — 페이지 시험이 읽는 끝점의 답이다.
#
#     cargo build && MOAI=target/debug/moai scripts/gen-web-fixtures.sh
#
# 손으로 적은 자료는 바이너리와 갈라진다 — 갈라진 자료로 초록인 시험은 없는 시험보다 나쁘다. 그래서 줄은
# 스크래치 저장소에 고정된 id·시각으로 세우고(`MOAI_NOW`), 각 끝점이 부르는 명령의 출력을 그대로 담는다.
# 갈라지면 `tests/web_bundle.rs` 의 `the_web_fixtures_match_the_binary` 가 잡는다.
#
# 줄: 마일스톤·에픽·이슈·자식·backlog·미룸(에픽째)·lost·사흘 전 닫은 줄(`archive_days = 2` 라 아카이브)·
# 어제 닫은 줄·노트에만 든 낱말. 위키는 쪽 둘(하나는 디렉터리 밑)이다.
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
moai=${MOAI:-$root/target/debug/moai}
out=${1:-$root/web/tests/fixtures/moai}

[ -x "$moai" ] || {
  printf 'gen-web-fixtures: %s 가 없다 — cargo build 를 먼저 돌린다\n' "$moai" >&2
  exit 2
}

# `/api/rows` 가 싣는 키는 서버 한 곳에 있다(src/web/api.rs ROW_FIELDS) — 여기 다시 적으면 둘로 갈린다.
fields=$(sed -n 's/^pub const ROW_FIELDS: &str = "\(.*\)";$/\1/p' "$root/src/web/api.rs")
[ -n "$fields" ] || {
  printf 'gen-web-fixtures: src/web/api.rs 에서 ROW_FIELDS 를 못 읽었다\n' >&2
  exit 2
}

scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
repo=$scratch/fx
mkdir -p "$repo/.moai" "$scratch/home"
export HOME=$scratch/home MOAI_CONFIG=$scratch/home/config.toml MOAI_LANG=en NO_COLOR=1
export MOAI_NOW=2026-10-20T09:00:00Z MOAI_ACTOR='Fixture (fixture@example.com)'
unset MOAI_HERE
: >"$MOAI_CONFIG"

cat >"$repo/.moai/config.toml" <<'EOF'
prefix = "fx"
statuses = "todo,in_progress,review,done"
archive_days = 2
EOF
t='"created_at":"2026-10-10T09:00:00Z","updated_at":"2026-10-10T09:00:00Z"'
who='"assignee":"Fixture","assignee_email":"fixture@example.com"'
cat >"$repo/.moai/issues.jsonl" <<EOF
{"id":"fx-th01","title":"Think about an index","kind":"idea","status":"todo",$who,$t,"status_since":"2026-10-10T09:00:00Z"}
{"id":"fx-dn01","title":"Closed yesterday","status":"done","priority":1,$who,$t,"status_since":"2026-10-19T09:00:00Z","done_at":"2026-10-19T09:00:00Z"}
{"id":"fx-ep01","title":"Storage layer","kind":"epic","status":"todo","milestone":"fx-ms01",$who,$t,"status_since":"2026-10-10T09:00:00Z"}
{"id":"fx-ep01.aaa","title":"Write path","status":"in_progress","priority":1,"tags":["store"],$who,$t,"status_since":"2026-10-12T09:00:00Z","started_at":"2026-10-12T09:00:00Z"}
{"id":"fx-ep01.aaa.bbb","title":"Lock file","status":"todo",$who,$t,"status_since":"2026-10-10T09:00:00Z"}
{"id":"fx-ep01.bbb","title":"Read path","status":"review","due_on":"2026-10-25",$t,"status_since":"2026-10-14T09:00:00Z"}
{"id":"fx-ep02","title":"Later work","kind":"epic","status":"todo","deferred_at":"2026-10-15T09:00:00Z",$who,$t,"status_since":"2026-10-10T09:00:00Z"}
{"id":"fx-ep02.aaa","title":"Put off with its epic","status":"todo",$who,$t,"status_since":"2026-10-10T09:00:00Z"}
{"id":"fx-is01","title":"Grid layout","status":"todo","priority":0,"body":"Columns line up.",$who,$t,"status_since":"2026-10-10T09:00:00Z"}
{"id":"fx-is02","title":"Closed three days ago","status":"done",$who,$t,"status_since":"2026-10-17T09:00:00Z","done_at":"2026-10-17T09:00:00Z"}
{"id":"fx-is03","title":"Names a gone epic","status":"todo","epic":"fx-gone",$who,$t,"status_since":"2026-10-10T09:00:00Z"}
{"id":"fx-ms01","title":"v1","kind":"milestone","status":"todo","due_on":"2026-10-30",$who,$t,"status_since":"2026-10-10T09:00:00Z"}
EOF
(cd "$repo" && git init -q && git config user.name Fixture && git config user.email fixture@example.com)
# `moai init` 이 쓰는 규칙을 미리 세운다 — 없으면 `status` 가 고칠 명령에 스크래치의 경로를 실어 자료가 판마다 달라진다.
"$moai" -C "$repo" init >/dev/null
"$moai" -C "$repo" note fx-is01 'the parser drops the last column' >/dev/null
mkdir -p "$repo/docs/guide"
printf '# Overview\n\nStart at the [board](guide/board.md).\n\n## Keys\n\nPress `?`.\n' >"$repo/docs/overview.md"
printf '# The board\n\nBack to the [overview](../overview.md#keys).\n' >"$repo/docs/guide/board.md"

mkdir -p "$out"
run() {
  local name=$1
  shift
  "$moai" -C "$repo" --json "$@" >"$out/$name.json"
}
run tree tui --deep
run tree-updated tui --deep --sort=updated --reverse
run rows show --all --fields="$fields"
run backlog backlog show --all --fields="$fields"
run rows-archived show --all --archived --fields="$fields"
run backlog-archived backlog show --all --archived --fields="$fields"
run status status
run issue show -- fx-ep01.aaa
run match-todo show --all --filter=status=todo --fields=id
run match-todo-backlog backlog show --all --filter=status=todo --fields=id
run match-grep show --all --grep=parser --fields=id
run wiki-ls wiki ls
run wiki-show wiki show -- overview
printf 'gen-web-fixtures: %s\n' "$out"
