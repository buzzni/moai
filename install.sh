#!/bin/sh
# moai 를 GitHub 릴리스에서 받아 깐다.
#
#   curl -fsSL https://raw.githubusercontent.com/buzzni/moai/main/install.sh | sh
#   curl -fsSL https://raw.githubusercontent.com/buzzni/moai/main/install.sh | sh -s -- --dir ~/bin
#   sh install.sh --version v0.1.0 --dir ~/bin
#
# **같은 한 줄이 판을 올린다**(moai-8rmw). 깔 자리에 이미 선 것이 이 moai 면 `--force`
# 없이 덮고 판이 어떻게 바뀌었는지 말한다. 같은 판이면 받지 않고, 최신 릴리스보다 앞선
# 판은 `--version` 으로 판을 준 때만 내린다. `--force` 는 남의 바이너리나 링크를 덮을 때,
# 같은 판을 다시 받을 때 든다.
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
#   --force            그 자리에 선 남의 moai·링크를 덮는다. 같은 판도 다시 받는다
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
  --force            그 자리에 선 남의 moai·링크를 덮는다. 같은 판도 다시 받는다
  --print-target     이 기계의 타깃 이름만 찍고 끝낸다

깔 자리에 이 moai 가 이미 서 있으면 --force 없이 판을 올린다. 파이프로 흘려 부를 때는
인자 앞에 `sh -s --` 를 둔다.

  curl -fsSL https://raw.githubusercontent.com/buzzni/moai/main/install.sh | sh -s -- --dir ~/bin
USAGE
    exit 0
    ;;
  *) die "모르는 인자다 — $1" ;;
  esac
done

# 받는 사람이 **준** 것만 적어 둔다 — 인자와 환경 어느 쪽으로 줬든 여기까지 온 값이다.
# 거절문이 다시 칠 줄에 이것만 싣는다: 기본값까지 실으면 집 자리가 줄에 박혀, 남의 기계로
# 옮겨 적은 줄이 엉뚱한 자리를 가리킨다. 판을 줬는지는 내릴지도 가른다 — 안 줬으면 뒤에서
# 최신 판을 물어 `version` 을 채우므로, 채우기 전인 여기서 적는다.
given_version=$version
given_dir=$dir

# 내는 판이 둘뿐이라 짝이 안 맞으면 바로 말한다. 조용히 비슷한 것을 깔면 받는
# 사람이 못 도는 바이너리를 쥔다.
#
# **`MOAI_TARGET` 은 이 표를 건너뛴다**(moai-utya). 시험이 쓰는 자리다 — 설치 시험은
# 가짜 릴리스 하나를 지어 **설치 길**을 재는데, 이름을 이 표에서 받으면 판을 안 내는
# 기계(arm64 리눅스·인텔 맥)에서 그 표가 먼저 죽어 재려던 길에 닿지도 못한다. 그 기계에서도
# 도구는 소스로 지어 잘 돈다.
#
# **가짜 릴리스를 가리킬 때만 듣는다** (리뷰 moai-6mk3.lgj). 받는 사람이 밟는 길에서도 듣던
# 판은 셋을 열었다 — 어쩌다 내보내 둔 이름 하나가 이 표를 꺼 "이 기계에 맞는 판이 없다" 대신
# 404 를 주고(`--print-target` 까지 그 이름을 되읊어 "이 기계의 타깃" 이 아니게 된다), 합계는
# 제 파일 이름과 맞춰 보는 것이라 진짜 Darwin 산출물이 리눅스에 그대로 깔리며, 이름에 든
# `/` 나 `..` 는 받는 자리를 `$work` 밖으로 옮겨 덫이 못 치운다. `MOAI_BASE_URL` 을 함께
# 요구하면 시험은 그대로 돌고 표는 내는 길의 유일한 자로 남는다.
if [ -n "${MOAI_TARGET:-}" ] && [ -n "${MOAI_BASE_URL:-}" ]; then
  case $MOAI_TARGET in
  */* | *..*) die "MOAI_TARGET 에 길 조각이 들었다 — $MOAI_TARGET" ;;
  esac
  target=$MOAI_TARGET
else
  case "$(uname -s)-$(uname -m)" in
  Linux-x86_64 | Linux-amd64) target=x86_64-unknown-linux-musl ;;
  Darwin-arm64 | Darwin-aarch64) target=aarch64-apple-darwin ;;
  *) die "이 기계에 맞는 판이 없다 — $(uname -s) $(uname -m). 지금 내는 것은 x86_64-unknown-linux-musl 과 aarch64-apple-darwin 이다. 소스에서 짓는 길은 README 에 있다" ;;
  esac
fi

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

# **깔 자리는 받기 전에 본다**(moai-8rmw.2mo). 거절할 줄 알면서 받고 재는 것은 받는
# 사람의 시간만 쓴다 — 만들 수 없는 자리, 덮을 수 없는 자리는 여기서 끝낸다.
mkdir -p "$dir" || die "$dir 을 못 만들었다"
# **디렉터리는 `--force` 로도 안 덮는다**(리뷰). `mv` 가 받은 것을 그 **안으로** 옮기고
# 깔았다고 말해, 그 자리에는 돌릴 것이 하나도 안 남는다. 거절문이 주는 `--force` 줄이
# 사람을 바로 그 길로 보낸다.
[ ! -d "$dir/moai" ] || die "$dir/moai 는 디렉터리다 — 덮지 않는다. 다른 자리에 깔려면 --dir 을 준다"

# **치울 자리를 먼저 만들고 덫을 건다.** 임시 파일을 여기저기서 따로 만들면 `die` 로
# 나가는 길마다 지우는 줄을 기억해야 하고, 한 번 잊으면 그것을 아무도 안 본다. 깔린
# 바이너리에 물은 답도 여기 받으므로(`ours`) 그 물음보다 앞선다.
work=$(mktemp -d) || die "임시 디렉터리를 못 만들었다"
# `$half` 는 밑에서 깔 자리에 놓는 반쪽이다. `$work` 밖이라 같이 치워야 한다 —
# 디스크가 차거나 Ctrl-C 로 끊기면 `PATH` 에 선 디렉터리에 잘린 파일이 남고, 다시
# 칠 때마다 하나씩 는다. 신호를 받은 판은 **끝낸다**: 덫만 돌고 이어 가면 이미 지운
# `$work` 를 가지고 계속 돌아 성공했다고 말한다.
half=
trap 'rm -rf "$work"; [ -z "$half" ] || rm -f "$half"' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

# 이 moai 가 아니면 1. 이 moai 면 깔린 판(`0.1.3`)을 찍는다.
#
# **`--version` 하나로는 못 가른다.** 옛 moai 프로젝트의 바이너리도 같은 이름과 같은
# `moai <판>` 꼴을 쓰고, 그쪽을 덮으면 그쪽의 훅과 lint 가 그 자리에서 깨진다. 그래서 이
# moai 만 답하는 것을 하나 더 묻는다 — `merge-driver --help` 의 첫 줄은 0.1.0 부터 같은
# 글이다. 옛 moai 는 `merge=union` 을 썼으니 이 명령이 없다. 그 글은 `src/cli.rs` 의
# `MergeDriver` 설명이고, 진짜 바이너리가 이 물음을 지나는지는 시험이 잰다
# (`the_real_binary_is_recognised_as_this_moai`).
#
# 입력은 `/dev/null` 로 막는다. 파이프로 흘러 들어온 경우 표준 입력은 이 스크립트
# 자체라, 물은 바이너리가 그것을 읽으면 스크립트의 남은 줄이 사라진다.
#
# **답은 파이프가 아니라 파일로 받는다**(리뷰). `$(…)` 는 파이프가 닫히기를 기다리는데,
# 물은 것이 띄운 하위 프로세스가 표준 출력을 쥐고 살아 있으면 마감이 지나도 영영 안
# 끝난다 — `merge_driver::probe` 가 같은 까닭으로 파일로 받는다. 마감은 `KILL` 로 건다:
# TERM 을 무시하는 것도 마감 안에 끝난다.
#
# 마감은 `timeout` 이 **그 꼴로 돌 때만** 건다 — macOS 에는 기본으로 없고, BusyBox 1.30
# 전의 `timeout` 은 `-t 초` 를 받아 `10` 을 프로그램으로 돌린다. 그대로 쓰면 물음이 늘
# 실패해 이 moai 까지 남의 것으로 읽힌다.
limit=
if command -v timeout >/dev/null 2>&1 && timeout -s KILL 10 true >/dev/null 2>&1; then
  limit="timeout -s KILL 10"
fi
ours() {
  [ -f "$1" ] && [ -x "$1" ] || return 1
  $limit "$1" --version </dev/null >"$work/answer" 2>/dev/null || return 1
  said=$(cat "$work/answer")
  case $said in "moai "[0-9]*) ;; *) return 1 ;; esac
  said=${said#moai }
  case $said in *[!0-9A-Za-z.+-]*) return 1 ;; esac
  $limit "$1" merge-driver --help </dev/null >"$work/answer" 2>/dev/null || return 1
  case $(cat "$work/answer") in *"Merges issues.jsonl per issue"*) ;; *) return 1 ;; esac
  printf '%s\n' "$said"
}

# 판 둘을 견준다 — `lt`·`eq`·`gt`. 앞의 세 마디를 수로 견주고, 수가 같은데 글이 다르면
# (`0.1.4-rc1` 과 `0.1.4`) `ne` 다. 오르는지 내리는지 모를 때 틀리게 말하느니 안 가른다.
order() {
  if [ "$1" = "$2" ]; then
    echo eq
    return
  fi
  awk -v a="$1" -v b="$2" 'BEGIN {
    split(a, x, /[.+-]/); split(b, y, /[.+-]/)
    for (i = 1; i <= 3; i++) {
      if (x[i] + 0 < y[i] + 0) { print "lt"; exit }
      if (x[i] + 0 > y[i] + 0) { print "gt"; exit }
    }
    print "ne"
  }'
}

# 거절문이 싣는 값을 셸이 그대로 읽게 싼다. 싸지 않아도 되는 값은 그대로 둔다 — 늘
# 싸면 흔한 줄이 따옴표투성이가 되어 옮겨 적기 어렵다.
quote() {
  case $1 in
  '' | *[!A-Za-z0-9_./:@%+=,-]*) printf "'%s'" "$(printf '%s' "$1" | sed "s/'/'\\\\''/g")" ;;
  *) printf '%s' "$1" ;;
  esac
}

# **거절할 때는 그대로 칠 줄을 준다**(moai-8rmw.t8y). 파일로 불렀는지 파이프로 불렀는지는
# 스크립트가 못 가르니 두 꼴을 다 준다. 받는 사람이 준 `--dir`·`--version` 은 같은 줄에
# 싣는다 — 빠뜨리면 그 줄이 엉뚱한 자리에 엉뚱한 판을 깐다. 저장소가 기본과 다르면
# 주소만 바꿔서는 모자라다. 그 스크립트도 `MOAI_REPO` 없이는 기본 저장소에서 받는다.
# 인자는 무엇이 섰는지 대는 첫 줄의 앞머리다.
refuse() {
  again=" --force"
  [ -z "$given_dir" ] || again="$again --dir $(quote "$given_dir")"
  [ -z "$given_version" ] || again="$again --version $(quote "$given_version")"
  from=
  [ "$repo" = buzzni/moai ] || from="MOAI_REPO=$(quote "$repo") "
  {
    printf 'moai: %s — 덮지 않는다. 다른 자리에 깔려면 --dir 을 준다\n' "$1"
    printf 'moai: 덮으려면 --force 를 준다. 부른 꼴대로 하나를 그대로 친다\n\n'
    printf '  curl -fsSL https://raw.githubusercontent.com/%s/main/install.sh | %ssh -s --%s\n' "$repo" "$from" "$again"
    printf '  %ssh install.sh%s\n' "$from" "$again"
  } >&2
  exit 1
}

# PATH 에 다른 moai 가 먼저 서 있으면 이 자리의 것이 안 돈다. 이것은 말만 하고
# 막지 않는다 — 어느 쪽을 쓸지는 사람이 정한다. **받지 않고 끝나는 길에서도 댄다**(리뷰):
# 새 판 알림을 보고 친 사람이 "이미 … 이다" 만 듣고 끝나면, PATH 에서 먼저 서는 옛
# moai 를 계속 돌리며 같은 알림을 다시 본다. 인자는 이 자리의 것을 부르는 말이다.
#
# **철자가 아니라 파일로 견준다**(`-ef`, 리뷰). 글자로 견주면 PATH 의 `~/.local/bin/` 처럼
# 끝에 `/` 가 붙었거나 `--dir` 이 상대 경로일 때, 같은 파일을 두고 딴 moai 가 먼저 선다고 말한다.
on_path() {
  found=$(command -v moai 2>/dev/null || true)
  if [ -n "$found" ] && ! [ "$found" -ef "$dir/moai" ]; then
    printf 'moai: PATH 에는 %s 가 먼저 선다 — %s %s 이다\n' "$found" "$1" "$dir/moai" >&2
  elif [ -z "$found" ]; then
    printf 'moai: PATH 에 %s 를 더한다\n' "$dir" >&2
  fi
}

# **이미 있는 것을 말없이 덮지 않는다** — 단 그것이 이 moai 면 덮는 것이 곧 판 올리기다
# (moai-8rmw.lj5). 남의 것인지 가를 수 없으면(돌지 않고, 답이 다르고, 마감을 넘기면)
# 남의 것으로 읽는다. `--force` 는 묻지도 않는다 — 덮기로 한 바이너리를 돌릴 까닭이 없다.
#
# **링크는 이 moai 로 읽지 않는다**(리뷰). 링크 너머가 이 moai 여도 `mv` 는 링크 자체를
# 보통 파일로 바꾸고, 링크가 가리키던 것(소스에서 지은 판 따위)은 옛 판 그대로 남는다 —
# 올린 것은 없고 사람이 걸어 둔 링크만 잃는다. 가리키는 것이 사라진 링크도 같다 — `-e` 는
# 링크를 따라가 거기서 거짓이 되므로 `-L` 로 따로 본다.
had=
if { [ -e "$dir/moai" ] || [ -L "$dir/moai" ]; } && [ "$force" != 1 ]; then
  [ ! -L "$dir/moai" ] || refuse "$dir/moai 는 다른 자리를 가리키는 링크다"
  had=$(ours "$dir/moai") || refuse "$dir/moai 은 이 moai 가 아니다"
fi

if [ -z "$version" ]; then
  say "최신 릴리스를 묻는다"
  fetch "$api" "$work/latest.json" || die "최신 릴리스를 못 읽었다 — --version 으로 태그를 준다"
  version=$(sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$work/latest.json" | head -n 1)
  [ -n "$version" ] || die "릴리스 답에서 태그를 못 읽었다 — --version 으로 태그를 준다"
fi

# 깔린 판과 견줄 꼴 — 태그에서 `v` 를 뗀 것이다(`moai --version` 이 그렇게 답한다).
# 주소와 이름에는 태그를 그대로 쓴다.
bare=${version#v}

# 최신 판은 물어야 아니, 받을지는 여기서 — 물은 뒤, 받기 전에 — 가른다.
way=
if [ -n "$had" ]; then
  way=$(order "$had" "$bare")
  # awk 가 아무것도 못 냈으면 오르는지 내리는지 모르는 것이다(`ne`).
  [ -n "$way" ] || way=ne
  case $way in
  eq)
    say "$dir/moai 는 이미 $bare 이다 — 받지 않는다"
    on_path "여기 선 것은"
    exit 0
    ;;
  gt)
    # **내리는 것은 판을 준 때뿐이다**(리뷰, moai-8rmw.lj5 의 "`--version` 으로 더 옛 판을
    # 고른 경우"). 판을 안 준 한 줄은 최신 릴리스를 물어 채운 것이라, 그보다 앞선 판(소스에서
    # 지은 판, 먼저 골라 깐 앞판)을 그 한 줄이 되돌리면 안 된다 — `latest::Seen::Ahead` 를
    # 새 판과 가른 까닭과 같다.
    if [ -z "$given_version" ]; then
      say "$dir/moai 는 최신 릴리스 $bare 보다 앞선 $had 이다 — 받지 않는다. 내리려면 --version $version 을 준다"
      on_path "여기 선 것은"
      exit 0
    fi
    ;;
  esac
fi

# **쓸 수 없는 자리도 받기 전에 끝낸다**(리뷰). 안 보면 받고 재기까지 다 한 뒤 `cp` 의 날
# 오류로 죽는다. 같은 판·앞선 판은 쓸 일이 없으니 그 뒤에 본다 — 거기서 멈추면 0 으로
# 끝날 자리가 실패가 된다. root 에게는 늘 참이라 root 는 막지 않는다.
[ -w "$dir" ] || die "$dir 에 쓸 수 없다 — 쓸 수 있는 사용자로 부르거나 --dir 로 다른 자리를 준다"

# `way` 가 비었으면 깔린 것이 없거나 `--force` 다 — 견줄 판이 없다.
case $way in
'') ;;
lt) say "$dir/moai 를 $had 에서 $bare 로 올린다" ;;
gt) say "$dir/moai 를 $had 에서 $bare 로 내린다" ;;
# 수가 같은데 글이 다르면(`ne`) 오르는지 내리는지 모른다 — 틀리게 말하느니 안 가른다.
*) say "$dir/moai 를 $had 에서 $bare 로 바꾼다" ;;
esac

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

half=$dir/moai.tmp.$$
cp "$work/$name/moai" "$half"
chmod 755 "$half"
mv "$half" "$dir/moai"
half=
if [ -n "$had" ]; then
  say "$dir/moai 를 $had 에서 $bare 로 바꿨다"
else
  say "$dir/moai 에 깔았다 ($version)"
fi
on_path "방금 깐 것은"
