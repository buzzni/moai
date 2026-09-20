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

# 앞서 있던 훅이 남았는데 우리가 이어 부를 자리가 없어진 때. **말없이 버려두지 않는다.**
stray_before() {
  printf 'install-git-hooks: %s — 블록이 없는데 %s.moai-before 가 남아 있다.\n' "$1" "$1" >&2
  printf '  %s 를 부를 것이 없다 — 손으로 되돌리거나 치운다\n' "$2.moai-before" >&2
}

uninstall_one() {
  name=$1
  path=$hooks/$name
  [ -f "$path" ] || return 0
  # **우리 블록이 없으면 그 파일은 우리 것이 아니다.** 걷을 것이 없는데도 awk 로 베껴 쓰던
  # 판은 두 가지를 부쉈다 — 심볼릭 링크(husky·lefthook 이 거는 자리)가 복사본으로 굳어 그쪽
  # 업데이트가 안 닿고, `chmod -x` 로 꺼 둔 남의 훅이 `chmod 755` 로 되살아났다. `install_one`
  # 이 실행 비트를 안 건드리는 것과 같은 까닭이다: **안전망을 걷는 일이 남의 게이트를 다시
  # 켜는 일이 되면 안 된다.** 게다가 그러고서 "블록을 걷었다" 고 댔다 — 안 한 일을 했다고 한다.
  if ! grep -qF -- "$(mark_head "$name")" "$path"; then
    [ ! -e "$path.moai-before" ] || stray_before "$name" "$path"
    return 0
  fi
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
  if ! grep -qv '^#!' "$path"; then
    rm -f -- "$path"
    if [ -f "$path.moai-before" ]; then
      mv -- "$path.moai-before" "$path"
      printf 'install-git-hooks: %s — 앞서 있던 훅을 제자리로 돌렸다\n' "$name"
      return 0
    fi
  elif [ -e "$path.moai-before" ]; then
    # **말없이 버려두지 않는다.** 블록 밖에 남의 줄이 있으면(심은 뒤 lefthook 같은
    # 것이 같은 파일에 붙은 자리다) 어느 쪽이 먼저 돌아야 하는지를 도구가 못 정한다.
    # 그렇다고 입을 다물면 앞서 있던 훅이 영영 안 도는 것을 아무도 모른다.
    printf 'install-git-hooks: %s — 블록은 걷었지만 %s.moai-before 는 그대로 둔다.\n' "$name" "$name" >&2
    printf '  %s 에 남의 줄이 있어 어느 것이 먼저인지 도구가 못 정한다 — 손으로 합치거나 되돌린다\n' "$path" >&2
    return 0
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
    # **실행 비트를 손대지 않는다.** `chmod -x` 는 훅을 끄는 표준 손잡이다 — 여기서
    # 755 를 씌우면 사람이 꺼 둔 훅이 심는 순간 되살아나고, shim 이 그것을 불러 푸시를
    # 막는다. 안전망을 까는 일이 남의 게이트를 다시 켜는 일이 되면 안 된다.
    mv -- "$path" "$path.moai-before"
    printf 'install-git-hooks: %s — 앞서 있던 훅을 %s.moai-before 로 옮기고 이어 부른다\n' "$name" "$name"
  fi

  cp -- "$src" "$path"
  chmod 755 "$path"
  # **어디에 심었는지 댄다.** 훅 자리는 클론이 함께 쓴다 — 딸린 워크트리에서 쳐도
  # `git rev-parse --git-path hooks` 는 주 체크아웃의 자리를 낸다. 이 저장소는 일을
  # 워크트리에서 하라고 적혀 있어 첫 판이 거기서 돌기 쉬운데, 그 한 번이 모든
  # 체크아웃의 훅을 정한다. 말을 안 하면 그것을 아무도 모른다.
  printf 'install-git-hooks: %s — %s 에 심었다 (이 클론의 모든 워크트리가 함께 쓴다)\n' "$name" "$path"
}

what=install
if [ "$#" -gt 0 ]; then
  case $1 in
  --uninstall) what=uninstall ;;
  *) die "모르는 인자다 — $1" ;;
  esac
fi

for name in pre-push prepare-commit-msg; do
  if [ "$what" = install ]; then
    install_one "$name"
  else
    uninstall_one "$name"
  fi
done
