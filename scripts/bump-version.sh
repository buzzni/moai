#!/usr/bin/env bash
# `Cargo.toml` 과 `CHANGELOG.md` 의 버전을 한 번에 움직인다.
#
#     scripts/bump-version.sh auto      [Unreleased] 의 절을 읽고 판을 고른다
#     scripts/bump-version.sh minor     한 칸을 손으로 고른다 (patch 도 같다)
#     scripts/bump-version.sh 0.2.0     판을 그대로 준다 — 1.0 은 이 길로만 연다
#     scripts/bump-version.sh --next    auto 가 고를 판만 찍는다. 아무것도 안 쓴다
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
check=$root/scripts/check-version.sh

die() {
  printf 'bump-version: %s\n' "$1" >&2
  exit 2
}

# **판을 고르는 자는 `[Unreleased]` 의 절이다**(moai-ug3j, 2026-10-01 사용자 결정).
# 1.0 전에는 Fixed·Security 만 있으면 patch, Added·Changed·Deprecated·Removed 가 하나라도
# 있으면 minor 다 — 동작이 바뀌었으면 고친 것이 아니다. AI 든 사람이든 고르는 것은 줄을
# 어느 절에 넣는가이고, 번호는 여기서 센다. 세션마다 손으로 셈하던 판은 0.1.1 부터
# 내리 patch 였다 — 그 판들 대부분에 `### Added` 가 있었는데도.
#
# 모르는 절은 어느 칸인지 못 가르므로 고르지 않고 멈춘다. 어림으로 patch 를 주면 기능이
# 든 판이 고친 판으로 나간다.
#
# **첫 절 앞의 머리글은 세지 않는다**(리뷰 moai-ug3j.rrc 4번). 0.1.2 는 깨지는 것 둘을
# 머리글에 모아 두고 같은 것을 아래 절에 다시 적었다 — 그 꼴을 막으면 판을 다시 손으로
# 주는 수밖에 없다. 대신 머리글만 있고 절이 비었으면 멈추고, 머리글을 건너뛰었다는 것을
# 까닭 줄에 적는다. 항목은 절 안에 적는다.
pick() {
  [ -f "$changelog" ] || die "auto 는 CHANGELOG.md 의 [Unreleased] 를 읽는다 — 파일이 없다"
  local class head sorted minor='' patch='' odd='' lead=''
  # awk 를 process substitution 으로 받으면 그 실패가 종료 코드째 사라져 "비었다" 로 읽힌다.
  # 먼저 받아 두고, 실패하면 그 자리에서 멈춘다.
  #
  # `####` 는 절이 아니라 그 절의 글이다 — 절의 이름으로 읽으면 `# Detail` 같은 모르는
  # 절이 되어 고르지 못한다. 울타리(```) 안의 `#` 은 셸 주석이지 제목이 아니다 — 거기서
  # 끊으면 그 뒤의 절을 못 읽어 minor 가 patch 로 나간다.
  sorted=$(awk '
    !on && /^##[[:space:]]*\[Unreleased\]/ { on = 1; found = 1; next }
    !on { next }
    /^[[:space:]]*(```|~~~)/ { fence = !fence }
    !fence && (/^#$/ || /^#[^#]/ || /^##$/ || /^##[^#]/) { exit }
    !fence && (/^###$/ || /^###[^#]/) { h = $0; sub(/^###[[:space:]]*/, "", h); sub(/[[:space:]]+$/, "", h); next }
    /^[[:space:]]*$/ { next }
    {
      if (h in seen) next
      seen[h] = 1
      k = tolower(h)
      if (h == "") print "lead\t"
      else if (k ~ /^(added|changed|deprecated|removed)$/) print "minor\t" h
      else if (k ~ /^(fixed|security)$/) print "patch\t" h
      else print "odd\t" h
    }
    END { if (!found) print "missing\t" }
  ' "$changelog") || die "CHANGELOG.md 의 [Unreleased] 를 못 읽었다"
  while IFS=$'\t' read -r class head; do
    case $class in
    '') ;;
    lead) lead=1 ;;
    minor) minor="$minor${minor:+·}$head" ;;
    patch) patch="$patch${patch:+·}$head" ;;
    missing) die 'CHANGELOG.md 에 ## [Unreleased] 줄이 없다' ;;
    *) odd="$odd${odd:+, }$head" ;;
    esac
  done <<<"$sorted"
  [ -z "$odd" ] || die "[Unreleased] 에서 칸을 못 가른다 — $odd. Added·Changed·Deprecated·Removed·Fixed·Security 중 하나로 옮기거나 판을 직접 준다"
  if [ -n "$minor" ]; then
    level=minor why="[Unreleased] 에 $minor 가 있다"
  elif [ -n "$patch" ]; then
    level=patch why="[Unreleased] 에 $patch 뿐이다"
  elif [ -n "$lead" ]; then
    die "[Unreleased] 에 첫 절 앞의 글만 있다 — 항목을 Added·Changed·Deprecated·Removed·Fixed·Security 절 안에 적는다"
  else
    die "[Unreleased] 가 비었다 — 낼 것이 없다"
  fi
  [ -z "$lead" ] || why="$why (첫 절 앞의 머리글은 세지 않았다)"
}

# `have` 에서 `level` 한 칸을 올린다. 꼬리(`-rc.1`)가 붙은 판에서 한 칸이 무엇인지는 정한
# 적이 없으니 판을 직접 받는다.
step() {
  [[ $have =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]] ||
    die "지금 판 $have 에서 한 칸을 못 잰다 — 판을 직접 준다"
  local x=${BASH_REMATCH[1]} y=${BASH_REMATCH[2]} z=${BASH_REMATCH[3]}
  case $level in
  minor) want=$x.$((y + 1)).0 ;;
  patch) want=$x.$y.$((z + 1)) ;;
  esac
}

[ "$#" -eq 1 ] || die "올릴 판을 하나 준다 — auto · minor · patch · 0.2.0 (보기만 하려면 --next)"
want=$1 level='' why='' next=''
case $want in
v*) die "앞의 v 를 빼고 준다 — ${want#v}" ;;
auto | minor | patch) level=$want ;;
--next) level=auto next=1 ;;
major) die "major 는 판을 직접 준다 — 1.0 은 사람이 연다" ;;
*) [[ $want =~ ^[0-9]+\.[0-9]+\.[0-9]+([-+][0-9A-Za-z.-]+)?$ ]] || die "버전 꼴이 아니다 — $want" ;;
esac
[ -f "$manifest" ] || die "Cargo.toml 을 못 찾았다 — $manifest"

# **판을 읽는 자는 `check-version.sh` 하나다.** 같은 awk 를 여기 한 번 더 적으면
# `[package]` 표를 가리는 줄이 두 군데가 되고, 한쪽만 고쳐도 아무도 모른다 — 그쪽
# 파일이 `--print` 를 든 까닭이 그것이다.
[ -x "$check" ] || die "scripts/check-version.sh 를 못 찾았다 — $check"
have=$("$check" --print) || die "Cargo.toml 의 [package] 에서 version 을 못 읽었다"
[ -n "$have" ] || die "Cargo.toml 의 [package] 에서 version 을 못 읽었다"

# 위의 규칙은 1.0 전의 것이다. 1.0 뒤에는 Removed 와 동작 변경이 major 일 수 있는데 그
# 금은 아직 안 그었다 — 그은 척 minor 를 내느니 사람에게 넘긴다.
case $level in
auto)
  [[ $have == 0.* ]] || die "1.0 뒤의 규칙은 아직 안 정했다 — minor · patch · 판을 직접 준다"
  pick
  step
  ;;
minor | patch)
  why="손으로 고른 $level"
  step
  ;;
esac

# **`--next` 는 판 하나만 stdout 에 낸다**(moai-ug3j.w0c). 릴리스 때 마일스톤의 잠정
# 제목과 견주는 자리라 `v$(scripts/bump-version.sh --next)` 로 그대로 받게 하고, 까닭은
# 사람이 읽도록 stderr 로 보낸다. 고르는 길은 `auto` 와 한 길이다 — 따로 셈하면 미리
# 본 판과 실제로 올린 판이 갈릴 수 있다.
if [ -n "$next" ]; then
  printf 'bump-version: %s — %s, %s → %s\n' "$why" "$level" "$have" "$want" >&2
  printf '%s\n' "$want"
  exit 0
fi
[ -z "$why" ] || printf 'bump-version: %s — %s, %s → %s\n' "$why" "$level" "$have" "$want"
[ "$have" != "$want" ] || die "이미 $want 다"

# **CHANGELOG 를 먼저 본다.** 뒤에서 죽으면 `Cargo.toml` 만 움직인 반쪽이 남고, 그
# 반쪽은 위의 `이미 $want 다` 가 다시 부르는 것을 막아 손으로 되짚는 수밖에 없다.
if [ -f "$changelog" ]; then
  grep -q '^##[[:space:]]*\[Unreleased\]' "$changelog" || die 'CHANGELOG.md 에 ## [Unreleased] 줄이 없다'
fi

# `[package]` 의 첫 `version` 줄 하나만 바꾼다. 의존성 표의 같은 낱말은 안 건드린다.
#
# `mktemp` 이 낸 파일은 0600 이고 `mv` 가 그 모드를 그대로 얹는다 — 그대로 두면
# 판을 올린 커밋에서 `Cargo.toml` 이 주인만 읽는 파일이 되고, git 은 실행 비트만
# 보므로 아무도 못 본다. 옮기기 전에 되돌린다.
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
chmod 644 -- "$tmp"
mv -- "$tmp" "$manifest"
printf 'bump-version: Cargo.toml  %s → %s\n' "$have" "$want"

# 잠금 파일의 자기 줄도 같이 움직인다. 안 맞으면 `--locked` 로 도는 CI 가 떨어진다.
#
# 여기서 죽지 않는다. `Cargo.toml` 은 이미 고쳤고, 여기서 멈추면 CHANGELOG 만
# 안 움직인 반쪽이 남는다 — 어긋난 잠금 파일은 CI 가 다시 잡지만 반쪽은 사람이
# 손으로 되짚어야 한다.
lock_said='bump-version: Cargo.lock 을 못 고쳤다 — cargo metadata 를 손으로 한 번 돌린다'
locked=1
if ! command -v cargo >/dev/null; then
  locked=0
  printf '%s\n' "$lock_said" >&2
elif (cd -- "$root" && cargo metadata --no-deps --format-version 1 --offline >/dev/null 2>&1) \
  || (cd -- "$root" && cargo metadata --no-deps --format-version 1 >/dev/null); then
  printf 'bump-version: Cargo.lock  자기 줄을 다시 적었다\n'
else
  locked=0
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
  chmod 644 -- "$tmp"
  mv -- "$tmp" "$changelog"
  printf 'bump-version: CHANGELOG.md  [%s] - %s 를 열었다\n' "$want" "$today"
else
  printf 'bump-version: CHANGELOG.md 가 없어 건너뛴다\n' >&2
fi

# **잠금 파일을 못 고쳤으면 태그를 대지 않는다.** 릴리스 워크플로는 `--locked` 로
# 짓고, 어긋난 잠금 파일은 태그가 이미 나간 뒤에 거기서 터진다 — 그때는 되돌릴
# 자리가 없다. 여기서 막지는 않되, 다음에 칠 것은 태그가 아니라 그 고침이다.
if [ "$locked" = 0 ]; then
  cat <<NEXT >&2

다음
  cargo metadata --no-deps >/dev/null      먼저 Cargo.lock 을 맞춘다
  그 뒤에 커밋하고 태그를 단다 — 릴리스는 \`--locked\` 로 짓는다
NEXT
  exit 0
fi

cat <<NEXT

다음
  git commit -m "chore(release): $want" -- Cargo.toml Cargo.lock CHANGELOG.md
  git tag v$want
  git push && git push origin v$want
NEXT
