#!/usr/bin/env bash
# `Cargo.toml` 과 `CHANGELOG.md` 의 버전을 한 번에 움직인다.
#
#     scripts/bump-version.sh 0.2.0
#
# **태그는 사람이 단다.** 이 스크립트는 커밋도 태그도 하지 않고 파일만 고친다 —
# 무엇이 바뀌었는지 사람이 보고 커밋하는 자리를 남긴다. 태그를 여기서 달면
# `check-version.sh` 가 재는 두 값을 한 손이 같이 써, 어긋남을 잡을 눈이 없어진다.
#
# 날짜는 `MOAI_NOW` 가 서 있으면 그것을 쓴다 (시험이 시계를 고정하는 자리).
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
manifest=$root/Cargo.toml
changelog=$root/CHANGELOG.md

die() {
  printf 'bump-version: %s\n' "$1" >&2
  exit 2
}

[ "$#" -eq 1 ] || die "올릴 버전을 하나 준다 — scripts/bump-version.sh 0.2.0"
want=$1
case $want in
v*) die "앞의 v 를 빼고 준다 — ${want#v}" ;;
esac
[[ $want =~ ^[0-9]+\.[0-9]+\.[0-9]+([-+][0-9A-Za-z.-]+)?$ ]] || die "버전 꼴이 아니다 — $want"
[ -f "$manifest" ] || die "Cargo.toml 을 못 찾았다 — $manifest"

have=$(awk '
  /^[[:space:]]*\[/ { pkg = ($0 ~ /^[[:space:]]*\[package\][[:space:]]*$/); next }
  pkg && /^[[:space:]]*version[[:space:]]*=/ {
    sub(/^[^=]*=[[:space:]]*/, ""); sub(/[[:space:]]*#.*$/, ""); gsub(/^[[:space:]]*"|"[[:space:]]*$/, "")
    print; exit
  }
' "$manifest")
[ -n "$have" ] || die "Cargo.toml 의 [package] 에서 version 을 못 읽었다"
[ "$have" != "$want" ] || die "이미 $want 다"

# `[package]` 의 첫 `version` 줄 하나만 바꾼다. 의존성 표의 같은 낱말은 안 건드린다.
tmp=$(mktemp "$manifest.tmp.XXXXXX")
trap 'rm -f -- "$tmp"' EXIT
awk -v want="$want" '
  /^[[:space:]]*\[/ { pkg = ($0 ~ /^[[:space:]]*\[package\][[:space:]]*$/); print; next }
  pkg && !done && /^[[:space:]]*version[[:space:]]*=/ {
    sub(/=[[:space:]]*"[^"]*"/, "= \"" want "\"")
    done = 1
  }
  { print }
' "$manifest" >"$tmp"
mv -- "$tmp" "$manifest"
printf 'bump-version: Cargo.toml  %s → %s\n' "$have" "$want"

# 잠금 파일의 자기 줄도 같이 움직인다. 안 맞으면 `--locked` 로 도는 CI 가 떨어진다.
#
# 여기서 죽지 않는다. `Cargo.toml` 은 이미 고쳤고, 여기서 멈추면 CHANGELOG 만
# 안 움직인 반쪽이 남는다 — 어긋난 잠금 파일은 CI 가 다시 잡지만 반쪽은 사람이
# 손으로 되짚어야 한다.
lock_said='bump-version: Cargo.lock 을 못 고쳤다 — cargo metadata 를 손으로 한 번 돌린다'
if ! command -v cargo >/dev/null; then
  printf '%s\n' "$lock_said" >&2
elif (cd -- "$root" && cargo metadata --no-deps --format-version 1 --offline >/dev/null 2>&1) \
  || (cd -- "$root" && cargo metadata --no-deps --format-version 1 >/dev/null); then
  printf 'bump-version: Cargo.lock  자기 줄을 다시 적었다\n'
else
  printf '%s\n' "$lock_said" >&2
fi

# CHANGELOG 의 `[Unreleased]` 밑에 이번 판을 연다. 위의 `[Unreleased]` 는 다음
# 판이 쌓일 자리로 그대로 남는다.
if [ -f "$changelog" ]; then
  today=${MOAI_NOW:-$(date -u +%F)}
  today=${today:0:10}
  tmp=$(mktemp "$changelog.tmp.XXXXXX")
  awk -v want="$want" -v today="$today" '
    !done && /^##[[:space:]]*\[Unreleased\]/ {
      print
      print ""
      print "## [" want "] - " today
      done = 1
      next
    }
    { print }
    END { if (!done) exit 3 }
  ' "$changelog" >"$tmp" || die 'CHANGELOG.md 에 ## [Unreleased] 줄이 없다'
  mv -- "$tmp" "$changelog"
  printf 'bump-version: CHANGELOG.md  [%s] - %s 를 열었다\n' "$want" "$today"
else
  printf 'bump-version: CHANGELOG.md 가 없어 건너뛴다\n' >&2
fi

cat <<NEXT

다음
  git commit -m "chore(release): $want" -- Cargo.toml Cargo.lock CHANGELOG.md
  git tag v$want
  git push && git push origin v$want
NEXT
