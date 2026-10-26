#!/usr/bin/env bash
# 사용 방법 페이지(guide/)와 릴리스 소식(release-posts/)의 셸 장면을 asciinema 로 녹화한다. 장면마다 데모 자리를 새로 짓는다.
#
#     MOAI=<moai 바이너리> scripts/site-demo/record-guide.sh <내보낼 디렉터리> [장면...]
#
# 장면은 shell.py 의 scenes(init, backlog, keys, wiki). init 은 아직 moai 가 없는 저장소에서, 나머지는 seed.sh 가
# 지은 Lantern 에서 찍는다. 장면을 주지 않으면 사용 방법 페이지의 둘(init, backlog)만 찍는다.
#
# init 은 저장소·바이너리·사용자 설정의 절대 경로를 화면에 찍는다. 그래서 데모 사람의 HOME 을 고정된
# 자리(DEMO_ROOT, 기본 /tmp/mira)에 짓고 moai 를 그 안에 복사해 둔다 — 녹화하는 기계의 경로가 공개
# 페이지에 실리지 않게. cast 머리의 command·env·timestamp 도 같은 까닭으로 지운다.
set -euo pipefail
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
out=$(mkdir -p "${1:?usage: record-guide.sh <out dir> [scene...]}" && cd "$1" && pwd); shift
if [ $# -gt 0 ]; then scenes=("$@"); else scenes=(init backlog); fi
size=${SIZE:-100x30}
moai=$(realpath -- "${MOAI:-$here/../../target/release/moai}")
root=${DEMO_ROOT:-/tmp/mira}
# 지우기 전에 우리가 지은 자리인지 본다 — 남의 디렉터리를 rm -rf 하지 않게
if [ -e "$root" ] && [ ! -e "$root/.moai-demo" ]; then
  echo "record-guide.sh: $root exists and is not a demo home; set DEMO_ROOT" >&2; exit 1
fi
export DEMO_HOME=$root
for scene in "${scenes[@]}"; do
  rm -rf "$root"
  mkdir -p "$root/.local/bin" "$root/.config/moai"
  touch "$root/.moai-demo"
  cp "$moai" "$root/.local/bin/moai"
  export MOAI=$root/.local/bin/moai
  # init 의 훅·스킬 칸은 moai skill install 을 부르고, 그것이 플러그인 등록을 claude 에 맡긴다. 녹화 자리에
  # 진짜 claude 를 들이면 그 사람의 설정·인증이 엮이니, 성공만 돌려주는 대역을 둔다 — 화면에 서는 것은
  # claude 가 깔린 사람이 보는 moai 의 출력 그대로다
  printf '#!/bin/sh\nexit 0\n' > "$root/.local/bin/claude" && chmod +x "$root/.local/bin/claude"
  printf '[update]\ncheck = false\n' > "$root/.config/moai/config.toml"
  repo="$root/lantern"
  if [ "$scene" = init ]; then
    # moai 를 들이기 전의 저장소: 커밋 하나와 데모 사람
    mkdir -p "$repo" && cd "$repo"
    git init -q -b main
    git config user.name 'Mira Chen' && git config user.email 'mira@example.com' && git config commit.gpgsign false
    printf '# Lantern\n\nA note app that works offline.\n' > README.md
    git add README.md && git commit -q -m 'first commit'
    cd - >/dev/null
  else
    bash "$here/seed.sh" "$repo" >/dev/null
  fi
  if [ "$scene" = wiki ]; then
    # 이미 선 교훈 하나: 두 줄의 노트가 가리켜 `moai wiki ls` 에 ×2 로 선다. 장면에서 적는 새 교훈은 ×1 이다
    ( cd "$repo" && . ./.demo-ids
      export MOAI_CONFIG=$repo/.demo-home/config.toml MOAI_LANG=en MOAI_NO_UPDATE_CHECK=1
      "$MOAI" wiki learn 'Keep the main thread off the index' --issue "$search" -b - >/dev/null <<'LESSON'
## What happened
Indexing on launch froze the note list for two seconds on large notebooks.

## Do this instead
Hand indexing to the search worker and let the list draw from what is there.
LESSON
      "$MOAI" note "$editor" 'Lesson: lessons/keep-the-main-thread-off-the-index' >/dev/null )
  fi
  if [ "$scene" = keys ]; then
    # 바꾼 줄 둘과, 기본 키(정렬)와 부딪혀 빠질 줄 하나
    printf '[browse]\nquit = "SPC x"\ngo_board = ["SPC g b", "B"]\nsort_due = "SPC s p"\n' > "$root/.config/moai/keys.toml"
  fi
  asciinema rec --headless --window-size "$size" --overwrite -q \
    -c "$(printf '%q ' "$here/shell.py" "$scene" "$repo")" "$root/$scene.cast" >/dev/null
  jq -c 'if type == "object" and has("version") then del(.command, .env, .timestamp) else . end' \
    "$root/$scene.cast" > "$out/guide-$scene.cast"
  echo "recorded $scene"
done
rm -rf "$root"
