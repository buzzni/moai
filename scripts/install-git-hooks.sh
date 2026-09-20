#!/usr/bin/env bash
# `scripts/git-hooks/` 의 shim 을 이 클론의 훅 자리에 심는다. 다시 불러도 된다.
#
#     scripts/install-git-hooks.sh
#     scripts/install-git-hooks.sh --uninstall
#
# 훅은 커밋되지 않으니 **클론마다 한 번** 친다. 안 치면 지금까지와 같다 — 심는
# 것이 늘리는 것은 안전망이지 규칙이 아니다.
#
# 이미 남의 훅이 있으면 `<이름>.moai-before` 로 **옮겨 두고 이어 부른다.** 지우지
# 않는다. shim 은 제 블록 밖을 건드리지 않아, 블록을 쓰는 훅 매니저와 한 파일에서
# 같이 살 수도 있다.
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
here=$root/scripts/git-hooks

die() {
  printf 'install-git-hooks: %s\n' "$1" >&2
  exit 2
}

hooks=$(git -C "$root" rev-parse --git-path hooks 2>/dev/null) || die "git 저장소가 아니다 — $root"
case $hooks in
/*) ;;
*) hooks=$root/$hooks ;;
esac
mkdir -p "$hooks"

mark_head() { printf '# >>> moai:%s >>>' "$1"; }
mark_foot() { printf '# <<< moai:%s <<<' "$1"; }

uninstall_one() {
  name=$1
  path=$hooks/$name
  [ -f "$path" ] || return 0
  # 블록만 걷는다. 남의 줄은 그대로 둔다.
  awk -v head="$(mark_head "$name")" -v foot="$(mark_foot "$name")" '
    index($0, head) == 1 { skip = 1; next }
    index($0, foot) == 1 { skip = 0; next }
    !skip { print }
  ' "$path" >"$path.tmp"
  mv -- "$path.tmp" "$path"
  chmod 755 "$path"
  # 블록을 걷고 나니 `#!` 한 줄뿐이면 그 파일은 우리가 만든 것이다. 앞서 있던
  # 훅을 제자리로 돌려놓는다.
  if [ "$(grep -cv '^#!' "$path" || true)" = 0 ]; then
    rm -f -- "$path"
    if [ -f "$path.moai-before" ]; then
      mv -- "$path.moai-before" "$path"
      printf 'install-git-hooks: %s — 앞서 있던 훅을 제자리로 돌렸다\n' "$name"
      return 0
    fi
  fi
  printf 'install-git-hooks: %s — 블록을 걷었다\n' "$name"
}

install_one() {
  name=$1
  src=$here/$name
  path=$hooks/$name
  [ -f "$src" ] || die "$src 가 없다"

  if [ -f "$path" ] && grep -qF -- "$(mark_head "$name")" "$path"; then
    # 이미 우리 블록이 있다. 그 자리만 새것으로 갈아 끼운다.
    awk -v head="$(mark_head "$name")" -v foot="$(mark_foot "$name")" -v src="$src" '
      index($0, head) == 1 { skip = 1; while ((getline line < src) > 0) if (line !~ /^#!/) print line; next }
      index($0, foot) == 1 { skip = 0; next }
      !skip { print }
    ' "$path" >"$path.tmp"
    mv -- "$path.tmp" "$path"
    chmod 755 "$path"
    printf 'install-git-hooks: %s — 블록을 다시 맞췄다\n' "$name"
    return 0
  fi

  if [ -f "$path" ]; then
    # 남의 훅이다. 옆으로 옮겨 두면 shim 이 그것을 먼저 부른다.
    [ ! -e "$path.moai-before" ] || die "$path.moai-before 가 이미 있다 — 먼저 치우고 다시 부른다"
    mv -- "$path" "$path.moai-before"
    chmod 755 "$path.moai-before"
    printf 'install-git-hooks: %s — 앞서 있던 훅을 %s.moai-before 로 옮기고 이어 부른다\n' "$name" "$name"
  fi

  cp -- "$src" "$path"
  chmod 755 "$path"
  printf 'install-git-hooks: %s — 심었다\n' "$name"
}

what=install
if [ "$#" -gt 0 ]; then
  case $1 in
  --uninstall) what=uninstall ;;
  *) die "모르는 인자다 — $1" ;;
  esac
fi

for name in pre-push; do
  if [ "$what" = install ]; then
    install_one "$name"
  else
    uninstall_one "$name"
  fi
done
