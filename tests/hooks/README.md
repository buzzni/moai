# 훅 입력 시험 자료

`moai hook --dialect codex|antigravity` 가 받는 stdin 이다(moai-u5wr). 시험은 이 글을 그대로 읽고, 자리(`cwd`·
`workspacePaths`·`Cwd`·`TargetFile`)만 시험 저장소로 바꿔 넣는다.

- **어디서 왔나**: 2026-10-04, 사람이 저장소 뿌리에서 띄운 대화형 세션 둘 — codex 0.160·agy 1.2.16 — 에 받은 stdin 을
  파일로 남기기만 하는 훅을 사용자 범위에 걸고, 사람에게 명령 몇 개를 시켜 달라고 해서 남겼다. 헤드리스로 돌리지 않았다.
  무엇을 쟀는지는 `moai show moai-u5wr.amg` 의 노트에 있다
- **`codex/pre-tool-use-apply-patch.json` 만 기록이 아니다** — 두 판 다 모델이 셸로 파일을 고쳐 `apply_patch` 가 안
  불렸다. 기록한 `pre-tool-use-bash.json` 위에 Codex 훅 문서의 꼴(`tool_input.command` 가 패치 글)을 얹었다
- Claude 의 꼴은 따로 두지 않는다 — `tests/cli.rs` 의 훅 시험 전부가 그 꼴이다

다시 기록하면 같은 이름으로 갈아 넣는다. 꼴이 바뀌었으면 `src/cmd/hook.rs` 의 `arrived` 가 붉어지는 시험부터 본다.

## Codex 의 `/hooks` 갈무리 — `codex/config/`

훅 파일을 읽은 Codex 가 `/hooks` 에 낸 글이다(moai-o9tg). 짝이 둘이다 — `<이름>.json` 은 그때 Codex 가 읽은 훅 파일,
`<이름>.txt` 는 Codex 가 낸 글 그대로다. `src/skill.rs` 의 `the_planted_codex_hooks_pass_codex_s_own_checks` 가 갈무리마다
Codex 의 경고와 시험의 본뜸(`codex_issues`)이 낸 경고를 하나하나 견주고, 심는 파일에는 본뜸이 아무 말이 없는지 본다.

- **`before-moai-t6hl`**: 2026-10-05 02:20(UTC), 사람의 codex 0.160 창이 저장소 뿌리의 `.codex/hooks.json` 을 읽고 낸
  글이다. 사람이 그 글을 codex 에 옮겨 붙인 것이 Codex 의 로그(`~/.codex/logs_2.sqlite`)에 남아 거기서 꺼냈다. 파일은
  그때 커밋된 판(`0f199cc1`~`db8de3c3^`)이다. **마지막 경고는 앞만 남았다** — 옮겨 붙이며 끊겼다. 시험은 갈무리의
  마지막 경고 하나만 앞머리로 맞춘다
- **`after-moai-t6hl`**: 2026-10-05 08:10(UTC), 같은 창이 moai-t6hl 로 고쳐 다시 심은 파일(`db8de3c3`)을 읽고 낸 글이다.
  경고가 없는 판이다. 사람이 이 창에 옮겨 붙였는데 입력 칸이 줄바꿈을 한 칸으로 접어 한 줄이 되었고, 표는
  `UserPromptSubmit` 줄에서 끊겼다. 그 아래와 표의 위아래에 ⚠ 경고도 Issues 토막도 없었다고 사람이 따로 답했다
- 경고의 글은 같은 판 실행 파일의 `hooks/src/engine/discovery.rs` 가 든 글과 같다. 본뜸이 안 다루는 그 자리의 나머지
  경고와, 처리기가 아는 키(`HookHandlerConfig::Command`)도 그 실행 파일에서 읽었다

**새로 갈무리하려면** 사람이 제 codex 창에서 `/hooks` 를 열어 나온 글을 옮겨 받는다. 시험이 codex 를 띄우지 않는다 —
moai 는 에이전트를 안 띄운다. 그 글을 `<이름>.txt` 로, 그때의 훅 파일을 `<이름>.json` 으로 둔다. 화면이 줄을 접은
자리는 그대로 둔다(시험이 잇는다). 경고가 없는 판도 갈무리가 된다 — Issues 가 없는 글이면 본뜸도 아무 말이 없어야
한다. 본뜸이 모르는 경고가 든 갈무리를 넣으면 시험이 그 경고를 대며 붉어지니, 그 규칙을 `codex_issues` 에 더한다.
