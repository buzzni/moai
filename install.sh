#!/bin/sh
# moai 를 GitHub 릴리스에서 받아 깐다.
#
#   curl -fsSL https://raw.githubusercontent.com/buzzni/moai/main/install.sh | sh
#   sh install.sh --version v0.1.0 --dir ~/bin
#
# **받는 사람이 밟는 가지는 `main` 이다.** 릴리스를 자르는 가지가 `main` 이고,
# `develop` 은 아직 안 나간 것이 섞이는 자리다 — 거기를 가리키면 받는 사람이
# 릴리스에 없는 스크립트로 릴리스를 깐다. `CONTRIBUTING.md` 의 `develop` 은
# 기여자 흐름이라 그대로다.
#
# **checksums 로 확인하지 못하면 깔지 않는다.** 확인을 건너뛰는 길은 없다 —
# 건너뛸 수 있는 확인은 아무도 안 하는 확인이고, 이 스크립트는 파이프로 셸에
# 바로 흘러 들어가는 자리에 선다.
#
# 다음 단계는 SBOM 과 서명(attestation)이다. 지금은 checksums 까지다.
#
#   --version <태그>   받을 판. 없으면 최신 릴리스
#   --dir <디렉터리>   깔 자리. 없으면 ~/.local/bin
#   --force            그 자리에 이미 있는 moai 를 덮는다
#   --print-target     이 기계의 타깃 이름만 찍고 끝낸다
set -eu

repo=${MOAI_REPO:-buzzni/moai}
base=${MOAI_BASE_URL:-https://github.com/$repo/releases/download}
api=${MOAI_API_URL:-https://api.github.com/repos/$repo/releases/latest}
version=${MOAI_VERSION:-}
# **`$HOME` 은 다 읽은 뒤에 편다.** 여기서 펴면 `set -u` 아래에서 집 없는 자리
# (`docker run --user`, `env -i`, 몇몇 CI 컨테이너)가 `--dir` 나 `--help` 를 준 판까지
# 셸의 날 오류로 죽는다 — 파이프로 흘러 들어온 사람이 보는 것이 이 스크립트의 말이 아니게 된다.
dir=${MOAI_INSTALL_DIR:-}
force=0
print_target=0

say() { printf 'moai: %s\n' "$1"; }
die() {
  printf 'moai: %s\n' "$1" >&2
  exit 1
}

while [ "$#" -gt 0 ]; do
  case $1 in
  --version)
    [ "$#" -ge 2 ] || die "--version 뒤에 태그를 준다"
    version=$2
    shift 2
    ;;
  --dir)
    [ "$#" -ge 2 ] || die "--dir 뒤에 디렉터리를 준다"
    dir=$2
    shift 2
    ;;
  --force)
    force=1
    shift
    ;;
  --print-target)
    print_target=1
    shift
    ;;
  -h | --help)
    # `$0` 를 되읽지 않는다 — 파이프로 흘러 들어온 판에는 읽을 파일이 없다.
    cat <<'USAGE'
moai 를 GitHub 릴리스에서 받아 깐다. checksums 로 확인하지 못하면 깔지 않는다.

  --version <태그>   받을 판. 없으면 최신 릴리스
  --dir <디렉터리>   깔 자리. 없으면 ~/.local/bin
  --force            그 자리에 이미 있는 moai 를 덮는다
  --print-target     이 기계의 타깃 이름만 찍고 끝낸다
USAGE
    exit 0
    ;;
  *) die "모르는 인자다 — $1" ;;
  esac
done

# 내는 판이 둘뿐이라 짝이 안 맞으면 바로 말한다. 조용히 비슷한 것을 깔면 받는
# 사람이 못 도는 바이너리를 쥔다.
case "$(uname -s)-$(uname -m)" in
Linux-x86_64 | Linux-amd64) target=x86_64-unknown-linux-musl ;;
Darwin-arm64 | Darwin-aarch64) target=aarch64-apple-darwin ;;
*) die "이 기계에 맞는 판이 없다 — $(uname -s) $(uname -m). 지금 내는 것은 x86_64-unknown-linux-musl 과 aarch64-apple-darwin 이다. 소스에서 짓는 길은 README 에 있다" ;;
esac

if [ "$print_target" = 1 ]; then
  printf '%s\n' "$target"
  exit 0
fi

if [ -z "$dir" ]; then
  [ -n "${HOME-}" ] || die "깔 자리를 모르겠다 — HOME 이 없다. --dir 이나 MOAI_INSTALL_DIR 로 준다"
  dir=$HOME/.local/bin
fi

# 받는 길과 재는 길이 둘 다 서야 시작한다. 받아 놓고 못 재는 것이 제일 나쁘다 —
# 그 자리에서 사람은 "이미 받았으니" 하고 넘어간다.
if command -v curl >/dev/null 2>&1; then
  fetch() { curl -fsSL -o "$2" "$1"; }
elif command -v wget >/dev/null 2>&1; then
  fetch() { wget -qO "$2" "$1"; }
else
  die "curl 도 wget 도 없다"
fi

if command -v sha256sum >/dev/null 2>&1; then
  digest() { sha256sum "$1" | cut -d' ' -f1; }
elif command -v shasum >/dev/null 2>&1; then
  digest() { shasum -a 256 "$1" | cut -d' ' -f1; }
else
  die "sha256sum 도 shasum 도 없다 — 확인하지 못하면 깔지 않는다"
fi

# **치울 자리를 먼저 만들고 덫을 건다.** 임시 파일을 여기저기서 따로 만들면 `die` 로
# 나가는 길마다 지우는 줄을 기억해야 하고, 한 번 잊으면 그것을 아무도 안 본다.
work=$(mktemp -d) || die "임시 디렉터리를 못 만들었다"
# `$half` 는 밑에서 깔 자리에 놓는 반쪽이다. `$work` 밖이라 같이 치워야 한다 —
# 디스크가 차거나 Ctrl-C 로 끊기면 `PATH` 에 선 디렉터리에 잘린 파일이 남고, 다시
# 칠 때마다 하나씩 는다. 신호를 받은 판은 **끝낸다**: 덫만 돌고 이어 가면 이미 지운
# `$work` 를 가지고 계속 돌아 성공했다고 말한다.
half=
trap 'rm -rf "$work"; [ -z "$half" ] || rm -f "$half"' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

if [ -z "$version" ]; then
  say "최신 릴리스를 묻는다"
  fetch "$api" "$work/latest.json" || die "최신 릴리스를 못 읽었다 — --version 으로 태그를 준다"
  version=$(sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$work/latest.json" | head -n 1)
  [ -n "$version" ] || die "릴리스 답에서 태그를 못 읽었다 — --version 으로 태그를 준다"
fi

name=moai-$version-$target

say "$repo $version $target 을 받는다"
fetch "$base/$version/$name.tar.gz" "$work/$name.tar.gz" || die "산출물을 못 받았다 — $base/$version/$name.tar.gz"
fetch "$base/$version/SHA256SUMS" "$work/SHA256SUMS" || die "SHA256SUMS 를 못 받았다 — 확인하지 못하면 깔지 않는다"

# 이름을 정규식으로 안 짠다 — 태그에 든 `.` 하나가 아무 글자나 맞히는 자리가 된다.
# 파일 이름은 낱말째 견주고, 바이너리 꼴이 붙이는 앞의 `*` 만 걷는다.
want=$(awk -v f="$name.tar.gz" '{ n = $2; sub(/^\*/, "", n); if (n == f) { print $1; exit } }' "$work/SHA256SUMS")
[ -n "$want" ] || die "SHA256SUMS 에 $name.tar.gz 줄이 없다 — 확인하지 못하면 깔지 않는다"
have=$(digest "$work/$name.tar.gz")
if [ "$want" != "$have" ]; then
  printf 'moai: 받은 것이 SHA256SUMS 와 다르다 — 깔지 않는다\n' >&2
  printf '  적힌 것  %s\n' "$want" >&2
  printf '  받은 것  %s\n' "$have" >&2
  exit 1
fi
say "checksums 가 맞다"

tar xzf "$work/$name.tar.gz" -C "$work"
[ -f "$work/$name/moai" ] || die "받은 것 안에 moai 가 없다"

# **이미 있는 것을 말없이 덮지 않는다.** 이 이름의 바이너리가 다른 뿌리에서 온
# 기계가 있고, 덮는 순간 그쪽의 훅과 lint 가 그 자리에서 깨진다.
mkdir -p "$dir" || die "$dir 을 못 만들었다"
if [ -e "$dir/moai" ] && [ "$force" != 1 ]; then
  die "$dir/moai 이 이미 있다 — 덮으려면 --force, 다른 자리에 깔려면 --dir"
fi

half=$dir/moai.tmp.$$
cp "$work/$name/moai" "$half"
chmod 755 "$half"
mv "$half" "$dir/moai"
half=
say "$dir/moai 에 깔았다 ($version)"

# PATH 에 다른 moai 가 먼저 서 있으면 방금 깐 것이 안 돈다. 이것은 말만 하고
# 막지 않는다 — 어느 쪽을 쓸지는 사람이 정한다.
found=$(command -v moai 2>/dev/null || true)
if [ -n "$found" ] && [ "$found" != "$dir/moai" ]; then
  printf 'moai: PATH 에는 %s 가 먼저 선다 — 방금 깐 것은 %s 이다\n' "$found" "$dir/moai" >&2
elif [ -z "$found" ]; then
  printf 'moai: PATH 에 %s 를 더한다\n' "$dir" >&2
fi
