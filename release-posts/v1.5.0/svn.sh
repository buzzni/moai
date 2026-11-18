#!/usr/bin/env bash
# Build the fictional Lantern demo as a Subversion working copy.   svn.sh <empty dir for the working copy>
# scripts/site-demo/seed.sh seeds the tracker (it makes a git repository); this moves the files into a local
# svn repository (svnadmin create + file://), checks out trunk, and extend-svn.sh adds revisions and changes.
set -euo pipefail
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(cd -- "$here/../.." && pwd)
wc=${1:?usage: svn.sh <empty dir>}
mkdir -p "$wc"; wc=$(cd -- "$wc" && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
export TZ=UTC
bash "$root/scripts/site-demo/seed.sh" "$tmp/seed" >/dev/null
rm -rf "$tmp/seed/.git"
svnadmin create "$tmp/svnrepo"
printf '#!/bin/sh\nexit 0\n' > "$tmp/svnrepo/hooks/pre-revprop-change"; chmod +x "$tmp/svnrepo/hooks/pre-revprop-change"
url=file://$tmp/svnrepo
svn mkdir -q -m 'layout' "$url/trunk" "$url/branches" >/dev/null
# the working copy is the demo's home: the repository sits beside it and must outlive this script
rm -rf "$wc.svnrepo"; mv "$tmp/svnrepo" "$wc.svnrepo"
url=file://$wc.svnrepo
svn checkout -q "$url/trunk" "$wc"
cp -a "$tmp/seed/." "$wc/"
cd "$wc"
svn propset -q svn:ignore $'.demo-ids\n.demo-card\n.demo-home' . 
svn add -q --force . >/dev/null
svn commit -q -m 'chore: import the tracker' . >/dev/null
for rev in 1 2; do
  svn propset -q --revprop -r "$rev" svn:author mira . >/dev/null
  svn propset -q --revprop -r "$rev" svn:date "$(date -u -d "${DEMO_NOW:-now} -21 days" +%Y-%m-%dT%H:%M:%S.000000Z)" . >/dev/null
done
bash "$here/extend-svn.sh" "$wc"
