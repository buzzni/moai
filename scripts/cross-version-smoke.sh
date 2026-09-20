#!/usr/bin/env bash
# 옛 바이너리가 쓴 스냅샷을 새 바이너리가 만져도 남의 줄이 그대로인지 본다.
#
#     OLD=/path/to/old/moai NEW=target/release/moai scripts/cross-version-smoke.sh
#
# 쓰기는 매번 파일 전체를 다시 쓴다. 그래서 새 바이너리가 이슈 하나를 옮기는
# 동안 **나머지 줄의 바이트가 그대로여야 한다.** 안 그러면 두 판을 번갈아 쓰는
# 저장소가 명령마다 헛 diff 를 만들고, 모르는 필드를 든 줄은 한 번 만지는
# 것만으로 그 필드를 잃는다.
#
# 이 시험은 릴리스가 하나라도 있어야 뜻이 있다. 무엇을 옛 판으로 쓸지는 부르는
# 쪽이 정한다 — 여기서는 받아 오지 않는다.
set -euo pipefail

old=${OLD:?OLD 에 옛 바이너리 경로를 준다}
new=${NEW:?NEW 에 새 바이너리 경로를 준다}
for bin in "$old" "$new"; do
  [ -x "$bin" ] || {
    printf 'cross-version-smoke: %s 를 실행할 수 없다\n' "$bin" >&2
    exit 2
  }
done

# 밑에서 임시 자리로 옮겨 가니 경로를 지금 절대 경로로 굳힌다.
abs() {
  case $1 in
  /*) printf '%s' "$1" ;;
  *) printf '%s/%s' "$PWD" "$1" ;;
  esac
}
old=$(abs "$old")
new=$(abs "$new")

export MOAI_ACTOR=${MOAI_ACTOR:-'크로스 버전 (smoke@example.com)'}
export NO_COLOR=1
# **여기에 쓴다.** `MOAI_HERE` 를 끄면 워크트리 옮김이 살아 있어, `TMPDIR` 이 어쩌다
# 딸린 워크트리 안을 가리키면 이 시험이 주 체크아웃의 진짜 `.moai/issues.jsonl` 에
# 이슈를 만들고 모르는 필드를 심는다. 켜 두면 쓰는 자리가 밑의 `$work` 하나로 못 박힌다.
export MOAI_HERE=1

work=$(mktemp -d) || {
  printf 'cross-version-smoke: 임시 디렉터리를 못 만들었다\n' >&2
  exit 2
}
trap 'rm -rf "$work"' EXIT

printf '옛 판  %s\n' "$("$old" --version)"
printf '새 판  %s\n' "$("$new" --version)"

cd "$work"
"$old" init smoke >/dev/null

# 한 벌을 옛 판으로 만든다 — 에픽·이슈 둘·미룸·노트까지, 한 번은 쓰이는 모양을
# 고루 담는다.
made=$(printf '# 에픽 하나\n- [p1] 첫 일\n- [p2] 둘째 일\n' | "$old" add --from - --json)
ids=$(printf '%s' "$made" | grep -o '"id":"[^"]*"' | cut -d'"' -f4)
epic=$(printf '%s\n' "$ids" | sed -n 1p)
one=$(printf '%s\n' "$ids" | sed -n 2p)
two=$(printf '%s\n' "$ids" | sed -n 3p)
# **셋을 다 못 받았으면 여기서 말한다.** 옛 판의 출력 모양은 이쪽이 정하는 것이 아니다 —
# 빈 id 로 밀고 나가면 밤 시험이 `note: 모르는 id` 로 떨어져, 무엇이 어긋났는지가 아니라
# 이 스크립트가 어긋난 것처럼 보인다.
[ -n "$epic" ] && [ -n "$one" ] && [ -n "$two" ] || {
  printf 'cross-version-smoke: 옛 판의 `add --from - --json` 에서 id 셋을 못 읽었다\n' >&2
  printf '%s\n' "$made" >&2
  exit 2
}
"$old" note "$one" '옛 판이 남긴 메모' >/dev/null
"$old" defer "$two" -m '옛 판이 미뤘다' >/dev/null

snapshot=.moai/issues.jsonl

# **모르는 필드를 심는다.** 새 판이 그것을 지우면 지금은 아무도 안 보는 사이
# 옛 판의 필드가 사라지고, 다음 판에서 같은 일이 반대로 일어난다.
awk -v id="$epic" '
  index($0, "\"id\":\"" id "\"") { sub(/}$/, ",\"x_from_the_future\":{\"kept\":true}}") }
  { print }
' "$snapshot" >"$snapshot.seeded"
mv "$snapshot.seeded" "$snapshot"
grep -q 'x_from_the_future' "$snapshot" || {
  printf 'cross-version-smoke: 모르는 필드를 못 심었다 — 시험 자체가 서지 않는다\n' >&2
  exit 2
}

cp "$snapshot" "$work/before.jsonl"

# 새 판으로 **한 줄만** 옮긴다. 이 한 번이 파일 전체를 다시 쓴다.
"$new" mv "$one" in_progress --from todo >/dev/null

bad=0
while IFS= read -r line; do
  id=$(printf '%s' "$line" | grep -o '"id":"[^"]*"' | head -n 1 | cut -d'"' -f4)
  [ "$id" != "$one" ] || continue
  if ! grep -qxF -- "$line" "$snapshot"; then
    printf 'cross-version-smoke: %s 의 줄이 바뀌었다\n' "$id" >&2
    printf '  옛 판이 쓴 것  %s\n' "$line" >&2
    printf '  지금 있는 것   %s\n' "$(grep -F "\"id\":\"$id\"" "$snapshot" || printf '(없다)')" >&2
    bad=1
  fi
done <"$work/before.jsonl"

if ! grep -q '"x_from_the_future":{"kept":true}' "$snapshot"; then
  printf 'cross-version-smoke: 모르는 필드가 사라졌다\n' >&2
  bad=1
fi

# 옮긴 줄도 살아 있어야 한다 — 남의 줄을 지키느라 제 일을 안 한 것이 아니다.
if ! grep -q "\"id\":\"$one\"" "$snapshot"; then
  printf 'cross-version-smoke: 옮긴 줄이 사라졌다 — %s\n' "$one" >&2
  bad=1
fi

if [ "$bad" != 0 ]; then
  exit 1
fi
printf 'cross-version-smoke: 남의 줄도 모르는 필드도 그대로다 (%s 줄)\n' "$(wc -l <"$snapshot" | tr -d '[:space:]')"
