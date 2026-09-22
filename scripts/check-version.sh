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
# **판은 미는 커밋에서 읽는다**(moai-ler5). 작업본만 보던 판은 이미 다음 판으로
# 넘어간 자리에서 옛 태그를 다시 미는 것을 막았다 — 그 태그의 커밋에는 맞는 판이
# 들어 있는데 작업본이 아니라고 말하는 것이라, 막는 쪽이 틀린 자리다. 꺼내지
# 못하면(얕은 클론·git 이 없는 자리) 작업본으로 내려앉는다. 어느 쪽을 읽었는지
# 줄에 적어, 지나갔든 막혔든 무엇과 견줬는지가 로그에 남는다.
#
# 훅으로 걸려면 `scripts/install-git-hooks.sh` 를 클론마다 한 번 친다. 그쪽은
# 앞서 있던 훅을 지우지 않고 이어 부르며, 이것이 없거나 오래 걸리면 푸시를
# 그냥 보낸다. 이름만 걸어 두는 길(`ln -s`)도 되지만, 그러면 그 두 가지가 없다.
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
#
# **늘 stdin 을 읽는다.** 미는 커밋의 `Cargo.toml` 은 파일로 안 서고 `git show` 가
# 흘려 주므로 부르는 자리가 둘인데, 읽는 자를 그쪽에 한 번 더 적으면 `[package]` 표를
# 가리는 줄이 두 군데가 된다. 파일은 부르는 쪽이 `<` 로 먹인다 — `awk … "$@"` 로 파일과
# stdin 을 겸하던 판은 인자 없이 부르는 쪽이 bash 4.4 아래(맥의 `/bin/bash` 3.2)에서
# 빈 `"$@"` 를 `set -u` 위반으로 읽어, 미는 커밋을 못 꺼내고 조용히 작업본으로
# 내려앉았다 (리뷰 moai-6mk3.lgj).
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
  '
}

# 작업본의 판. `--print` 와, 커밋에서 못 꺼냈을 때의 내려앉는 자리다.
working_version() {
  local said
  [ -f "$manifest" ] || die "Cargo.toml 을 못 찾았다 — $manifest"
  said=$(manifest_version <"$manifest")
  [ -n "$said" ] || die "Cargo.toml 의 [package] 에서 version 을 못 읽었다"
  printf '%s\n' "$said"
}

# 미는 커밋의 판. 못 꺼내면 1 로 물러나고 부른 쪽이 작업본으로 내려앉는다.
#
# **빈 이름으로는 안 부른다** — `git show :Cargo.toml` 은 인덱스를 읽어, 아무도
# 안 시킨 자리를 답으로 낸다.
#
# **길은 `:./` 로 댄다** (리뷰 moai-6mk3.lgj). `<판>:<길>` 의 길은 `git -C` 가 선 자리가
# 아니라 **저장소 꼭대기**에서 푼다 — moai 가 큰 저장소의 하위 디렉터리에 든 클론에서는
# 남의 `Cargo.toml` 을 답으로 내, 맞는 태그를 어긋났다며 막고 이미 굳은 커밋에 태그를
# 다시 달라고 댄다. `:./` 는 선 자리에서 푼다.
#
# **커밋으로 못박는다** (같은 리뷰). 못박지 않으면 git 은 이름을 제 마음대로 푼다 — 짧은
# 16진수 넉 자(시험이 자리 채우개로 쓰던 `bbbb`·`cccc` 가 이 저장소에서 실제로 푼다)나
# 낱말 하나가 트리·블롭으로 닿아, 아무도 안 시킨 자리가 답이 된다. `^{commit}` 은 태그
# 객체(딸린 태그를 밀 때 pre-push 가 주는 것)는 그대로 벗겨 주고 그 둘만 물린다.
at_commit() {
  local said
  [ -n "${1:-}" ] || return 1
  command -v git >/dev/null 2>&1 || return 1
  said=$(git -C "$root" show "$1^{commit}:./Cargo.toml" 2>/dev/null) || return 1
  said=$(printf '%s\n' "$said" | manifest_version)
  [ -n "$said" ] || return 1
  printf '%s\n' "$said"
}

# 태그 이름에서 버전을 뽑는다. `refs/tags/v0.1.0` 과 `v0.1.0` 과 `0.1.0` 을 다 받는다.
# 둘째 인자는 그 태그가 가리키는 자리다 — pre-push 는 줄마다 미는 커밋을 주고,
# 인자로 받은 태그는 그 이름을 그대로 쓴다.
compare() {
  local tag=${1#refs/tags/} at=${2:-} want have where from
  want=${tag#v}
  have=$(at_commit "$at") || have=
  if [ -n "$have" ]; then
    from=commit
    where="$at 의 Cargo.toml"
  else
    # **못 읽으면 여기서 끝낸다** (리뷰 moai-6mk3.lgj). `working_version` 의 `die` 는 명령
    # 치환의 하위 셸에서 돌아 스스로는 스크립트를 못 끝내고, `compare … || bad=1` 이
    # `set -e` 까지 꺼 빈 판을 들고 계속 갔다 — 그러면 "못 읽었다"(2)가 "어긋난다"(1)로
    # 둔갑해, 1 만 막기로 한 pre-push shim 이 도구 사정으로 사람의 푸시를 막는다
    # (`scripts/git-hooks/pre-push` 의 머리글).
    have=$(working_version) || exit $?
    from=worktree
    where="작업본의 Cargo.toml"
  fi
  if [ "$want" != "$have" ]; then
    printf 'check-version: 태그와 Cargo.toml 이 어긋난다\n' >&2
    printf '  태그         %s  (버전 %s)\n' "$tag" "$want" >&2
    printf '  %s   %s\n' "$where" "$have" >&2
    # **고르는 것은 어디서 읽었나지 그 글이 아니다** (리뷰 moai-6mk3.lgj) — 화면 글을 다시
    # 읽어 갈래를 고르던 판은 그 글자 하나만 손봐도 엉뚱한 길을 댄다.
    if [ "$from" = worktree ]; then
      printf '  고치는 길    scripts/bump-version.sh %s 로 맞추고 태그를 다시 단다\n' "$want" >&2
    else
      # 그 커밋은 이미 굳었다 — 여기서 작업본을 올려도 미는 것은 안 바뀐다. 그렇다고 **옮길
      # 자리만** 대면 못 따르는 길이 된다 (리뷰 moai-6mk3.lgj): 판을 올리기 전에 태그를 단
      # 흔한 실수에는 그 판을 든 커밋이 아직 없어, 먼저 만들 것부터 대야 한다.
      printf '  고치는 길    scripts/bump-version.sh %s 로 그 판을 든 커밋을 만들고 태그를 그 커밋에 다시 단다\n' "$want" >&2
    fi
    return 1
  fi
  printf 'check-version: %s 와 %s 이 %s 로 같다\n' "$tag" "$where" "$have"
}

if [ "$#" -gt 0 ]; then
  case $1 in
  # 릴리스 워크플로가 산출물 이름을 여기서 받는다 — 판을 읽는 자를 워크플로에
  # 따로 적으면 `[package]` 표를 가리는 줄이 두 군데가 되고, 한쪽만 고쳐도 아무도
  # 모른다.
  # **`--print` 은 작업본을 읽는다.** 이것을 부르는 자리(`bump-version.sh`·릴리스의
  # 산출물 이름)가 보는 것이 작업본이고, 태그를 받지도 않는다.
  --print)
    working_version
    exit
    ;;
  refs/tags/* | v[0-9]* | [0-9]*)
    # 태그 이름을 자리로도 준다 — 이 클론에 그 태그가 서 있으면 그것이 가리키는
    # 커밋을 읽고, 없으면(`0.1.0` 처럼 태그가 아닌 꼴로 받은 판) 작업본으로 내려앉는다.
    compare "$1" "$1"
    exit
    ;;
  esac
  # **인자를 무조건 태그로 읽지 않는다.** git 은 pre-push 훅을 `<remote> <url>` 로
  # 부르고 밀 줄은 stdin 으로 준다 — 위의 `ln -s` 로 이 파일을 바로 훅에 걸면 `$1` 이
  # `origin` 이라, 태그로 재면 태그 없는 푸시까지 전부 막힌다. 꼴이 태그가 아니면
  # 훅으로 불린 것으로 보고 stdin 으로 내려간다.
  [ ! -t 0 ] || die "모르는 인자다 — $1. 태그는 v0.1.0 · refs/tags/v0.1.0 · 0.1.0 꼴로 준다"
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
  # 미는 커밋을 함께 준다 — 재는 것은 그 커밋이 든 판이지 작업본이 아니다.
  compare "$remote_ref" "$local_sha" || bad=1
done
exit "$bad"
