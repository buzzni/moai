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
