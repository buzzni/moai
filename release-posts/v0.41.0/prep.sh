#!/usr/bin/env bash
# Fake two sessions in a seeded Lantern repo: one silent for 2 h (lost), one alive.
# prep.sh <repo> -- needs MOAI, MOAI_CONFIG, DEMO_NOW in the environment.
set -euo pipefail
repo=$1
cd "$repo"
eval "$(grep -E '^(sync|search)=' .demo-ids)"
json() { "$MOAI" -C "$repo" show "$1" --json; }
retry=$("$MOAI" -C "$repo" show --all --json | jq -r '.[] | select(.title=="Retry with backoff on flaky networks") | .id')
fuzzy=$("$MOAI" -C "$repo" show --all --json | jq -r '.[] | select(.title=="Fuzzy matching for small typos") | .id')
# One real hook call makes the machine-local beats/picks key for this repo.
input=$(jq -nc --arg cwd "$repo" '{session_id:"jonas-retry",cwd:$cwd,tool_name:"Read",tool_input:{file_path:"/tmp/outside"}}')
env -u MOAI_NOW MOAI_NO_UPDATE_CHECK=1 "$MOAI" hook pre-tool-use <<<"$input" >/dev/null || true
home=$(dirname "$MOAI_CONFIG")
beats=$(ls -d "$home"/beats/*/ | head -1); key=$(basename "$beats")
picks="$home/picks/$key"; mkdir -p "$picks"
stamp_of() { date -u -d "$("$MOAI" -C "$repo" show "$1" --json | jq -r .updated_at)" +%s; }
# seeding inside an agent session records that session's picks; they would outrank the two demo sessions
find "$picks" -type f ! -name 'jonas-*' -delete
printf '100\t%s\t%s\n' "$retry" "$(stamp_of "$retry")" > "$picks/jonas-retry"
printf '101\t%s\t%s\n' "$fuzzy" "$(stamp_of "$fuzzy")" > "$picks/jonas-fuzzy"
printf 'claude\tPreToolUse\n' > "$beats/jonas-retry"
printf 'claude\tPreToolUse\n' > "$beats/jonas-fuzzy"
touch -d '2 hours ago' "$beats/jonas-retry"
touch "$beats/jonas-fuzzy"
printf 'lost=%s\nalive=%s\n' "$retry" "$fuzzy" > "$repo/.demo-lost"
