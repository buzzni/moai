#!/usr/bin/env bash
# 감독·일꾼 장면을 찍는다: 한 tmux 창에 moai tui 와 에이전트 셋(Claude Code 감독, Codex·Antigravity 일꾼).
#
#     MOAI=<moai 바이너리> scripts/site-demo/record-team.sh <작업 디렉터리>
#
# **실제 에이전트가 일한다.** 각 벤더 계정의 사용량을 쓰고, 결과는 매번 다르다. 그래서 키를 흘려
# 찍는 다른 장면과 달리 반쯤 손으로 찍는다 — 이 스크립트는 판을 차리고 녹화를 걸 뿐이고, 처음 뜨는
# 확인 창(폴더 신뢰, 업데이트 안내)과 감독에게 보내는 말은 사람이 넣는다. 끝에 그 걸음을 찍어 준다.
#
# 에이전트는 자동 승인 모드로 뜬다(데모 저장소에서만, 2026-10-23 사용자 허락). tmux 는 제 서버(-L)다 —
# 사람의 tmux 에는 닿지 않는다. 다 찍으면 데모 저장소에서 `moai skill uninstall` 로 Claude 플러그인
# 등록을 걷는다(local 범위로 사용자 설정에 들어간다).
set -euo pipefail
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
work=$(mkdir -p "${1:?usage: record-team.sh <work dir>}" && cd "$1" && pwd)
moai=${MOAI:?MOAI=<moai binary>}
repo=$work/lantern
mkdir -p "$work/bin" && ln -sf "$moai" "$work/bin/moai"
bash "$here/seed.sh" "$repo" >/dev/null
export PATH=$work/bin:$PATH MOAI_CONFIG=$repo/.demo-home/config.toml MOAI_NO_UPDATE_CHECK=1
. <(sed 's/^/export /' "$repo/.demo-ids")
# 일꾼 둘에게 하나씩 줄, 서로 겹치지 않는 작은 일
(cd "$repo" && moai add --from - --milestone "$m14" --body 'Two wiki pages people keep asking for. Small, independent, one per worker.' >/dev/null <<'PLAN'
# Docs for v1.4
- [p1] Write the wiki page for the sync status line
- [p1] Write the wiki page for offline editing
PLAN
)
(cd "$repo" && moai skill install --agent auto --with moai-tmux >/dev/null)

tmx() {
  env -i HOME="$HOME" USER="${USER:-coder}" SHELL=/bin/bash LANG=C.UTF-8 TERM=xterm-256color COLORTERM=truecolor \
    PATH="$PATH" MOAI_CONFIG="$MOAI_CONFIG" MOAI_NO_UPDATE_CHECK=1 tmux -L moai-demo -f /dev/null "$@"
}
# 왼쪽 한 칸 전체가 moai tui, 오른쪽을 셋으로: Claude Code(감독) · Codex · Antigravity
tmx new-session -d -s demo -x 200 -y 56 -c "$repo" 'moai tui'
tmx set -g status off; tmx set -g pane-border-status top; tmx set -g pane-border-format ' #{pane_title} '
tmx set -g focus-events on; tmx set -g main-pane-width 100
tmx split-window -h -t demo:0.0 -c "$repo" 'claude --dangerously-skip-permissions'
tmx split-window -v -t demo:0.1 -c "$repo" 'codex --dangerously-bypass-approvals-and-sandbox --dangerously-bypass-hook-trust'
tmx split-window -v -t demo:0.2 -c "$repo" 'agy --dangerously-skip-permissions'
tmx select-layout -t demo main-vertical
for p in 0 1 2 3; do
  tmx select-pane -t "demo:0.$p" -T "$(echo 'moai tui|Claude Code — supervisor|Codex — worker|Antigravity — worker' | cut -d'|' -f$((p + 1)))"
done

cat <<EOF
판이 섰다. 이어서 손으로:
  1. 확인 창을 닫는다:  tmux -L moai-demo attach -t demo  (폴더 신뢰·업데이트 안내, 닫고 detach)
  2. moai tui 칸을 보드로:  SPC g b, SPC v d, Esc
  3. 녹화를 건다:  asciinema rec --headless --window-size 200x56 -c 'tmux -L moai-demo attach -t demo' team.cast
  4. 감독 칸(demo:0.1)에 보낸다:
     /moai:moai-supervise This is a short recorded demo, keep it light. Two idle workers sit in this tmux
     window: the Codex pane and the Antigravity pane below you. Hand ${m14%%-*}-… (the two Docs for v1.4 issues)
     one to Codex and one to Antigravity. Each worker writes one new wiki page under docs/, commits it with
     the issue id in the subject on main, and moves its issue to done. Skip the model check, web search,
     worktrees and review for this demo. Tell me when both are done.
  5. 끝나면 detach 하고, cast 머리의 command·env·timestamp 를 지운다(record-tui.sh 와 같은 jq)
  6. 정리:  (cd $repo && moai skill uninstall);  tmux -L moai-demo kill-server
EOF
