#!/usr/bin/env bash
# The v0.44.0 pictures need a terminal that draws images (kitty or Ghostty), so they
# are taken by a person on such a machine, not by CI or an agent in tmux.
#
#   MOAI=<v0.44.0 binary> release-posts/v0.44.0/make-kitty.sh
#
# Run it in a kitty or Ghostty window that is NOT inside a tmux pane. It seeds the
# fictional Lantern project, attaches a picture to one issue and one wiki page, then
# opens the explorer there. Take two screenshots (macOS: Cmd-Shift-4, Space, click the
# window) and save them as:
#   release-posts/v0.44.0/media/01-tui-picture-detail.png   the issue detail with the picture
#   release-posts/v0.44.0/media/02-tui-picture-wiki.png     the wiki page `sync-protocol` with it
# and write one English line beside each as NN-name.md.
set -euo pipefail
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(cd -- "$here/../.." && pwd)
MOAI=$(realpath -- "${MOAI:-$root/target/release/moai}")
export DEMO_NOW=2026-11-02T12:00:00Z
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
# seed.sh counts days with GNU date (`date -d`). macOS's date has no -d, so lend it
# Homebrew's gdate (brew install coreutils) under that name for this run.
if ! date -d now >/dev/null 2>&1; then
  gdate=$(command -v gdate) || { echo 'make-kitty.sh: needs GNU date — brew install coreutils' >&2; exit 1; }
  mkdir "$scratch/bin" && ln -s "$gdate" "$scratch/bin/date" && PATH="$scratch/bin:$PATH"
fi
repo="$scratch/lantern"
bash "$root/scripts/site-demo/seed.sh" "$repo" >/dev/null
export MOAI_CONFIG="$repo/.demo-home/config.toml" MOAI_NO_UPDATE_CHECK=1 MOAI_LANG=en
eval "$(grep -E '^conflict=' "$repo/.demo-ids")"
picture="$root/.moai/images/6f38c14b84b49156.jpg"
"$MOAI" -C "$repo" image add "$picture" --to "$conflict" --alt 'Conflict banner on the packing-list note' --user 'Mira Chen (mira@example.com)' >/dev/null
ref=$("$MOAI" -C "$repo" image add "$picture" --wiki sync-protocol --alt 'Conflict banner on the packing-list note')
printf '\n%s\n' "$ref" >> "$repo/docs/sync-protocol.md"
printf 'Issue with the picture: %s. In the explorer, search (/) for "Resolve conflicts", open its detail;\nthen open the wiki pane on the page sync-protocol. Quit with q when both shots are saved.\n' "$conflict"
read -r -p 'Press Enter to open the explorer… ' _
cd "$repo" && "$MOAI" tui
