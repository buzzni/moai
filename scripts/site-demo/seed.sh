#!/usr/bin/env bash
# 홈페이지 녹화용 데모 저장소를 처음부터 짓는다 — 가상의 노트 앱 Lantern.
#
#     MOAI=<moai 바이너리> scripts/site-demo/seed.sh <빈 디렉터리>
#
# 녹화(record-tui.sh, record-web.mjs)는 이것이 지은 저장소를 띄워 찍는다. 다시 찍을 때마다
# 같은 모양이 나오게 스크립트로 둔다 — 화면에 서는 글은 영어, 사람은 example.com 의 가상 인물이다.
#
# 날짜는 MOAI_NOW 로 지난 3주에 흩는다. 그래야 목록의 나이·완료 시각이 실제 팀처럼 보인다.
set -euo pipefail

dir=${1:?usage: seed.sh <empty dir>}
# 이 저장소의 빌드가 기본이다. 상대 경로도 받도록 cd 전에 절대 경로로 굳힌다 — 그냥 moai 를 부르면
# PATH 의 옛 moai 가 조용히 찍힌다
moai=${MOAI:-$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)/target/release/moai}
case $moai in */*) moai=$(realpath -- "$moai") ;; *) moai=$(command -v -- "$moai") ;; esac
[ -x "$moai" ] || { echo "seed.sh: no moai binary at $moai" >&2; exit 1; }
# 녹화하는 사람의 환경이 화면에 새지 않게: 정체(MOAI_ACTOR)·키·색·말·시간대는 데모의 것으로
unset MOAI_ACTOR MOAI_KEYS NO_COLOR
export TZ=UTC MOAI_LANG=en
mkdir -p "$dir"
cd "$dir"
[ -z "$(ls -A .)" ] || { echo "seed.sh: $dir is not empty" >&2; exit 1; }

MIRA='Mira Chen (mira@example.com)'
JONAS='Jonas Weber (jonas@example.com)'
AIKO='Aiko Tanaka (aiko@example.com)'
SAM='Sam Rivera (sam@example.com)'

# 오늘에서 N일 H시 전의 RFC3339
ago() { date -u -d "-$1 days -${2:-0} hours" +%Y-%m-%dT%H:%M:%SZ; }

git init -q -b main
# 녹화용 부속은 데모 저장소의 이력에 안 싣는다 — 첫 커밋의 git add -A 보다 먼저 적는다
printf '.demo-ids\n.demo-card\n.demo-home/\n' >> .git/info/exclude
git config user.name 'Mira Chen'
git config user.email 'mira@example.com'
git config commit.gpgsign false

# 이 기계 사람의 설정(등록된 프로젝트·읽음 표시)이 화면에 새지 않게 데모만의 설정을 쓴다.
# 녹화 스크립트도 같은 자리를 MOAI_CONFIG 로 준다
mkdir -p .demo-home
printf '[update]\ncheck = false\n' > .demo-home/config.toml
export MOAI_CONFIG=$PWD/.demo-home/config.toml
export MOAI_NOW MOAI_NO_UPDATE_CHECK=1
MOAI_NOW=$(ago 21)
"$moai" init lan --yes --guide none --no-skill --no-register --tracking commit >/dev/null

# id 를 돌려주는 add. 첫 인자는 누가(--user), 나머지는 moai add 에 그대로 간다
add() { local who=$1; shift; "$moai" add "$@" --user "$who" --json | jq -r '.id'; }
mv_() { local who=$1 id=$2 to=$3; "$moai" mv "$id" "$to" --user "$who" >/dev/null; }
note() { local who=$1 id=$2; shift 2; "$moai" note "$id" "$@" --user "$who" >/dev/null; }
# 하나를 지난 날짜들을 따라 옮긴다: walk <누가> <id> <칸>:<며칠 전> ...
walk() { local who=$1 id=$2; shift 2; for step in "$@"; do MOAI_NOW=$(ago "${step#*:}"); mv_ "$who" "$id" "${step%%:*}"; done; }

MOAI_NOW=$(ago 21)
m14=$("$moai" milestone add 'v1.4 — Sync' --start "$(date -u -d '-20 days' +%F)" --due "$(date -u -d '+6 days' +%F)" --user "$MIRA" --json | jq -r .id)
m15=$("$moai" milestone add 'v1.5 — Sharing' --start "$(date -u -d '+7 days' +%F)" --due "$(date -u -d '+34 days' +%F)" --user "$MIRA" --json | jq -r .id)

e_sync=$("$moai" epic add 'Offline sync' --milestone "$m14" --user "$MIRA" --json -b - <<'EOF' | jq -r .id
Notes written on a plane should be there when you land, on every device.

- Edits made offline are queued and replayed in order
- Two devices editing the same note never lose either side
- The sidebar always says whether you are in sync
EOF
)
e_search=$("$moai" epic add 'Search that feels instant' --milestone "$m14" --user "$JONAS" --json -b - <<'EOF' | jq -r .id
Typing in the search box should show results before you finish the word.

- Titles are searchable the moment the app opens
- Full text is indexed in the background, never on the main thread
- Small typos still find the note
EOF
)
MOAI_NOW=$(ago 12)
e_share=$("$moai" epic add 'Shared notebooks' --milestone "$m15" --user "$AIKO" --json -b - <<'EOF' | jq -r .id
Let a team keep one notebook together without emailing exports around.

- Invite people by email, with read or write access
- A read-only link for people outside the team
- An activity feed so you can see what changed while you were away
EOF
)
e_editor=$("$moai" epic add 'Editor polish' --milestone "$m15" --user "$SAM" --json -b - <<'EOF' | jq -r .id
Small things in the editor that people ask about every week.
EOF
)

# --- Offline sync ---------------------------------------------------------
MOAI_NOW=$(ago 20)
read -r -d '' B <<'EOF' || true
## What
Edits made without a connection go into a local queue and are sent in order once the app is back online.

## Why
Today an edit made offline is kept only in memory. Closing the app loses it.

## Done when
- The queue survives an app restart
- Replaying the queue twice changes nothing (each entry carries its revision)
- The queue is capped at 5,000 entries, oldest first out, with a warning
EOF
s1=$(add "$MIRA" 'Queue edits while offline' -e "$e_sync" -p 1 -t sync -b "$B")
read -r -d '' B <<'EOF' || true
## What
When the server has a newer revision than the one an edit was based on, merge paragraph by paragraph. If the same paragraph changed on both sides, keep both and mark the note.

## Why
Last-write-wins silently threw away a paragraph for two people in the beta (support tickets 412, 418).

## Done when
- No edit is ever dropped without a trace
- A conflicted note shows a banner with both versions side by side
- See the protocol notes in the wiki: sync-protocol
EOF
s2=$(add "$MIRA" 'Resolve conflicts when two devices edit the same note' -e "$e_sync" -p 1 -t sync -b "$B")
read -r -d '' B <<'EOF' || true
## What
Retry failed sync requests with exponential backoff and jitter, starting at 2 s and capped at 5 min.

## Done when
- Hotel wifi that drops every minute still syncs everything within the hour
- The retry timer resets as soon as one request succeeds
EOF
s3=$(add "$JONAS" 'Retry with backoff on flaky networks' -e "$e_sync" -p 2 -t sync -b "$B")
read -r -d '' B <<'EOF' || true
## What
A small line at the bottom of the sidebar: "Synced", "Syncing 3 notes" or "Offline — 12 changes waiting".
EOF
s4=$(add "$AIKO" 'Show sync status in the sidebar' -e "$e_sync" -p 2 -t ui -b "$B")
read -r -d '' B <<'EOF' || true
## What
Upload big attachments in 4 MB chunks so a dropped connection resumes instead of starting over.
EOF
s5=$(add "$JONAS" 'Sync attachments larger than 10 MB' -e "$e_sync" -p 3 -t sync -b "$B")

# --- Search ---------------------------------------------------------------
MOAI_NOW=$(ago 19)
read -r -d '' B <<'EOF' || true
## What
Build an in-memory index of titles while the splash screen is up. 20k notes should take under 150 ms.
EOF
q1=$(add "$JONAS" 'Index note titles on launch' -e "$e_search" -p 1 -t search -b "$B")
read -r -d '' B <<'EOF' || true
## What
Move full-text indexing off the main thread. The editor must never stutter while the index catches up.
EOF
q2=$(add "$JONAS" 'Full-text index in a background worker' -e "$e_search" -p 1 -t search -b "$B")
read -r -d '' B <<'EOF' || true
## What
Bold the matching words in each result and show the line around the first match.
EOF
q3=$(add "$SAM" 'Highlight matches in results' -e "$e_search" -p 2 -t search -t ui -b "$B")
read -r -d '' B <<'EOF' || true
## What
"meeting ntoes" should still find "Meeting notes". One edit per word, only for words of four letters or more.
EOF
q4=$(add "$JONAS" 'Fuzzy matching for small typos' -e "$e_search" -p 2 -t search -b "$B")
read -r -d '' B <<'EOF' || true
## What
Extract text from attached PDFs in the background worker and add it to the full-text index.
EOF
q5=$(add "$JONAS" 'Search inside PDF attachments' -e "$e_search" -p 3 -t search -b "$B")

# --- Shared notebooks -----------------------------------------------------
MOAI_NOW=$(ago 11)
read -r -d '' B <<'EOF' || true
## What
Invite by email with read or write access. Pending invites expire after 14 days.
EOF
h1=$(add "$AIKO" 'Invite people to a notebook by email' -e "$e_share" -p 1 -t sharing -b "$B")
read -r -d '' B <<'EOF' || true
## What
A link anyone can open without an account. It can be turned off at any time, which breaks the old link.
EOF
h2=$(add "$AIKO" 'Read-only links' -e "$e_share" -p 2 -t sharing -b "$B")
read -r -d '' B <<'EOF' || true
## What
Owner, editor and viewer roles per notebook. Only the owner can delete the notebook or change roles.
EOF
h3=$(add "$MIRA" 'Per-notebook permissions' -e "$e_share" -p 2 -t sharing -b "$B")
read -r -d '' B <<'EOF' || true
## What
"Aiko edited Release checklist · 2 h ago". Grouped by day, newest first.
EOF
h4=$(add "$SAM" 'Activity feed for shared notebooks' -e "$e_share" -p 3 -t sharing -t ui -b "$B")

# --- Editor polish --------------------------------------------------------
MOAI_NOW=$(ago 14)
read -r -d '' B <<'EOF' || true
## What
Type a pipe table and it renders. Tab moves between cells, Enter adds a row.
EOF
d1=$(add "$SAM" 'Markdown tables' -e "$e_editor" -p 2 -t editor -b "$B")
read -r -d '' B <<'EOF' || true
## What
Type "/" at the start of a line for a menu: heading, list, checklist, table, image, divider.
EOF
d2=$(add "$SAM" 'Slash command menu' -e "$e_editor" -p 1 -t editor -b "$B")
read -r -d '' B <<'EOF' || true
## What
A handle on hover lets you drag a paragraph, list or image to a new place.
EOF
d3=$(add "$SAM" 'Drag to reorder blocks' -e "$e_editor" -p 2 -t editor -b "$B")
read -r -d '' B <<'EOF' || true
## What
Pasting an image stores it as an attachment and shows it inline.
EOF
d4=$(add "$AIKO" 'Paste images from the clipboard' -e "$e_editor" -p 2 -t editor -b "$B")

# --- 칸으로 흩는다 ----------------------------------------------------------
walk "$MIRA"  "$s1" in_progress:18 review:15 done:14
walk "$JONAS" "$q1" in_progress:18 review:17 done:16
walk "$JONAS" "$q2" in_progress:16 review:12 done:10
walk "$SAM"   "$d1" in_progress:13 review:9 done:8
walk "$AIKO"  "$d4" in_progress:9 review:6 done:5
walk "$MIRA"  "$s2" in_progress:13 review:2
walk "$SAM"   "$q3" in_progress:7 review:1
walk "$SAM"   "$d2" in_progress:6 review:0
walk "$JONAS" "$s3" in_progress:4
walk "$JONAS" "$q4" in_progress:3
walk "$AIKO"  "$h1" in_progress:2

# --- 노트: 다음 세션이 읽는 메모, 일한 AI ----------------------------------
MOAI_NOW=$(ago 14 2); note "$MIRA" "$s1" 'Queue lives in SQLite next to the notes; a JSON file lost entries when the app was killed mid-write.'
MOAI_NOW=$(ago 14 1); note "$MIRA" "$s1" 'model: anthropic/opus-5.5 (high — storage format)'
MOAI_NOW=$(ago 2 3);  note "$MIRA" "$s2" 'Paragraph ids must survive a paste, or the merge sees a delete plus an insert. Fixed in the editor, see the wiki page sync-protocol.'
MOAI_NOW=$(ago 2 2);  note "$MIRA" "$s2" 'model: anthropic/opus-5.5 (high — merge rules)'
MOAI_NOW=$(ago 1 5);  note "$MIRA" "$s2" 'Next: review the banner copy with Aiko, then merge.'
MOAI_NOW=$(ago 3 4);  note "$JONAS" "$q4" 'Trigram index is fast enough; no need for a separate typo dictionary.'
MOAI_NOW=$(ago 0 3);  note "$JONAS" "$s3" 'Next: add jitter, the retries from 40 test devices line up after a server restart.'

# --- backlog: 지금은 안 할 생각 -------------------------------------------
MOAI_NOW=$(ago 9);  "$moai" backlog add 'Dark mode for the web clipper' --user "$SAM" >/dev/null
MOAI_NOW=$(ago 6);  "$moai" backlog add 'Export a notebook as a static website' --user "$MIRA" >/dev/null
MOAI_NOW=$(ago 3);  "$moai" backlog add 'Handwriting with Apple Pencil' --user "$AIKO" >/dev/null

# 마감이 가까운 일의 우선순위를 올린다 — 팀이 하는 대로
MOAI_NOW=$(ago 0 1); "$moai" due apply --user "$MIRA" >/dev/null

# --- 위키 -----------------------------------------------------------------
mkdir -p docs
cat > docs/index.md <<EOF
# Lantern handbook

How the Lantern team builds and ships the app. Start here, then follow the links.

- [Architecture](architecture.md) — the pieces and how they talk
- [Sync protocol](sync-protocol.md) — offline queue, revisions, conflicts
- [Search](search.md) — what is indexed, and when
- [Release process](release-process.md) — from a milestone to the app stores

Current milestone work lives on the board. The sync work is tracked under $e_sync, search under $e_search.
EOF
cat > docs/architecture.md <<EOF
# Architecture

Lantern is a local-first notes app. Every device keeps a full copy of your notes, and the server only relays changes.

## The pieces

- **Editor** — blocks (paragraphs, lists, tables, images), each with a stable id
- **Local store** — SQLite on the device: notes, attachments and the outgoing queue
- **Sync worker** — sends the queue and applies changes from the server, see [Sync protocol](sync-protocol.md)
- **Search worker** — keeps the full-text index up to date, see [Search](search.md)

## Rules we keep

- The main thread never waits on the network or the index
- Nothing a user typed is ever dropped without a trace ($s2)
EOF
cat > docs/sync-protocol.md <<EOF
# Sync protocol

Every note carries a revision number. An edit is sent together with the revision it was based on.

## Offline queue

Edits made without a connection wait in the local queue and are replayed in order ($s1). Each entry carries its revision, so replaying twice changes nothing.

## Conflicts

If the server has moved past the base revision, the client merges paragraph by paragraph ($s2):

1. A paragraph changed on one side only takes that side
2. A paragraph changed on both sides keeps both, and the note shows a conflict banner
3. Paragraph ids must survive copy and paste, or a move looks like a delete plus an insert

## Retries

Failed requests back off exponentially with jitter ($s3). See [Architecture](architecture.md) for where the sync worker sits.
EOF
cat > docs/search.md <<EOF
# Search

Search has two indexes.

- **Titles** — built in memory at launch ($q1)
- **Full text** — built by the search worker in the background ($q2)

Results highlight the matching words ($q3), and short typos still match ($q4). PDF text is next ($q5).

The worker shares the local store with sync, described in [Architecture](architecture.md).
EOF
cat > docs/release-process.md <<EOF
# Release process

1. A milestone closes when every epic in it is done
2. The release branch is cut from main and goes to the beta group for three days
3. Crash-free sessions above 99.5% in beta means we ship

Read the [Lantern handbook](index.md) for the rest.
EOF

# --- 커밋: 이슈 상세가 그 일의 커밋을 찾아 보인다 -------------------------
commit() { local who_name=$1 who_mail=$2 days=$3 msg=$4; shift 4
  GIT_AUTHOR_NAME=$who_name GIT_AUTHOR_EMAIL=$who_mail GIT_COMMITTER_NAME=$who_name GIT_COMMITTER_EMAIL=$who_mail \
  GIT_AUTHOR_DATE=$(ago "$days") GIT_COMMITTER_DATE=$(ago "$days") git commit -q --allow-empty -m "$msg"; }
git add -A
commit 'Mira Chen' mira@example.com 21 'chore: start tracking work with moai'
commit 'Mira Chen' mira@example.com 15 "feat(sync): queue edits while offline ($s1)"
commit 'Jonas Weber' jonas@example.com 16 "feat(search): index note titles on launch ($q1)"
commit 'Jonas Weber' jonas@example.com 11 "feat(search): move full-text indexing to a worker ($q2)"
commit 'Mira Chen' mira@example.com 4 "feat(sync): merge concurrent edits paragraph by paragraph ($s2)"
commit 'Mira Chen' mira@example.com 2 "fix(editor): keep paragraph ids across paste ($s2)"
commit 'Jonas Weber' jonas@example.com 1 "feat(sync): retry with exponential backoff ($s3)"

# 녹화 스크립트가 읽는 id 들. 저장소 이력에는 안 싣는다
printf 'sync=%s\nsearch=%s\nshare=%s\neditor=%s\nconflict=%s\nm14=%s\nm15=%s\n' \
  "$e_sync" "$e_search" "$e_share" "$e_editor" "$s2" "$m14" "$m15" > .demo-ids
# 모두 읽은 채로 둔다 — 녹화에서 새로 생긴 카드만 NEW 로 선다
MOAI_NOW=$(ago 0); "$moai" read --all --user "$MIRA" >/dev/null

printf 'seed: %s\n' "$dir"
printf 'ids: sync=%s search=%s share=%s editor=%s conflict=%s\n' "$e_sync" "$e_search" "$e_share" "$e_editor" "$s2"
