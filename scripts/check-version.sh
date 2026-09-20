#!/usr/bin/env bash
# 태그와 `Cargo.toml` 의 버전이 어긋나면 0 아닌 코드로 멈춘다.
#
# `v0.1.0` 태그를 달았는데 `Cargo.toml` 이 아직 `0.0.x` 이면 받는 사람이 다른
# 버전을 깐다 — 릴리스 파이프라인은 태그를 믿고 굴러가고, 바이너리 안의 버전은
# `Cargo.toml` 에서 온다. 어긋난 것을 푸시 뒤에 알면 그때는 되돌릴 자리가 없다.
#
# **이것은 게이트가 맞다.** 막는 것이 릴리스이지 사람의 일상 명령이 아니다 —
# 태그를 안 미는 푸시는 읽기만 하고 그대로 지나간다.
#
# 두 자리에서 부른다.
#
#   scripts/check-version.sh v0.1.0    릴리스 워크플로의 첫 스텝
#   scripts/check-version.sh           pre-push 훅. git 이 주는 줄을 stdin 에서 읽는다
#   scripts/check-version.sh --print   Cargo.toml 의 판을 찍는다 (파일 이름을 짓는 자리)
#
# 훅으로 거는 가장 싼 길은 이름을 걸어 두는 것이다. 클론마다 한 번 친다.
#
#   ln -s ../../scripts/check-version.sh .git/hooks/pre-push
set -euo pipefail

# 저장소 뿌리는 **이 파일의 실제 자리**에서 읽는다. 위처럼 훅으로 걸면
# `${BASH_SOURCE[0]}` 은 `.git/hooks/pre-push` 라, 링크를 안 풀면 뿌리를 `.git`
# 으로 잡고 `Cargo.toml` 을 못 찾는다.
self=${BASH_SOURCE[0]}
while [ -L "$self" ]; do
  link=$(readlink -- "$self")
  case $link in
  /*) self=$link ;;
  *) self=$(dirname -- "$self")/$link ;;
  esac
done
root=$(cd -- "$(dirname -- "$self")/.." && pwd)
manifest=$root/Cargo.toml

die() {
  printf 'check-version: %s\n' "$1" >&2
  exit 2
}

# `[package]` 안의 첫 `version` 만 읽는다. 의존성 표에도 같은 낱말이 서 있어,
# 표를 안 가리면 아무 크레이트의 판이나 집는다.
manifest_version() {
  awk '
    /^[[:space:]]*\[/ { pkg = ($0 ~ /^[[:space:]]*\[package\][[:space:]]*$/); next }
    pkg && /^[[:space:]]*version[[:space:]]*=/ {
      sub(/^[^=]*=[[:space:]]*/, "")
      sub(/[[:space:]]*#.*$/, "")
      gsub(/^[[:space:]]*"|"[[:space:]]*$/, "")
      print
      exit
    }
  ' "$1"
}

# 태그 이름에서 버전을 뽑는다. `refs/tags/v0.1.0` 과 `v0.1.0` 과 `0.1.0` 을 다 받는다.
compare() {
  local tag=${1#refs/tags/} want
  want=${tag#v}
  if [ "$want" != "$have" ]; then
    printf 'check-version: 태그와 Cargo.toml 이 어긋난다\n' >&2
    printf '  태그         %s  (버전 %s)\n' "$tag" "$want" >&2
    printf '  Cargo.toml   %s\n' "$have" >&2
    printf '  고치는 길    scripts/bump-version.sh %s 로 맞추고 태그를 다시 단다\n' "$want" >&2
    return 1
  fi
  printf 'check-version: %s 와 Cargo.toml 이 %s 로 같다\n' "$tag" "$have"
}

[ -f "$manifest" ] || die "Cargo.toml 을 못 찾았다 — $manifest"
have=$(manifest_version "$manifest")
[ -n "$have" ] || die "Cargo.toml 의 [package] 에서 version 을 못 읽었다"

if [ "$#" -gt 0 ]; then
  # 릴리스 워크플로가 산출물 이름을 여기서 받는다 — 판을 읽는 자를 워크플로에
  # 따로 적으면 `[package]` 표를 가리는 줄이 두 군데가 되고, 한쪽만 고쳐도 아무도
  # 모른다.
  if [ "$1" = --print ]; then
    printf '%s\n' "$have"
    exit
  fi
  compare "$1"
  exit
fi

# pre-push 는 줄마다 `<local ref> <local sha> <remote ref> <remote sha>` 를 준다.
# 태그가 아닌 줄과 태그를 지우는 줄(local sha 가 전부 0)은 건너뛴다.
bad=0
while read -r _local_ref local_sha remote_ref _remote_sha; do
  case $remote_ref in
  refs/tags/v*) ;;
  *) continue ;;
  esac
  # 전부 0 이면 지우는 줄이다. 0 아닌 글자가 하나라도 있어야 본다.
  case $local_sha in
  *[!0]*) ;;
  *) continue ;;
  esac
  compare "$remote_ref" || bad=1
done
exit "$bad"
