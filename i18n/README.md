# 번역 보태기

화면 글자는 언어마다 JSON 하나다. 이 디렉터리의 파일만 고치면 되고, 러스트를 몰라도 된다.

    en.json   영어 — 기본이자 **기준**이다
    ko.json   한국어
    ja.json   일본어
    zh.json   중국어
    es.json   스페인어

## 고치는 법

1. 고칠 언어의 파일을 연다. 없는 언어를 새로 더하려면 아래 "언어 더하기" 를 본다.
2. 영어 파일에 있는 키를 찾아, 그 말로 옮긴 글을 적는다.
3. `cargo test` 를 돌린다. 시험이 아래 넷을 잰다.

```json
{
  "ready.count": "집을 수 있는 일  {n}건"
}
```

- **`{n}` 같은 자리는 그대로 둔다.** 이름이 든 중괄호는 프로그램이 값을 채우는 자리다.
  말차례가 달라지면 자리를 문장 안에서 **옮겨도 된다** — 이름으로 찾으므로 차례는 상관없다.
  빠뜨리면 그 말로 보는 사람에게만 수가 사라진다. 시험이 그것을 잡는다.
- **빈 글자로 두지 않는다.** 아직 못 옮긴 키는 **아예 빼면** 된다. 빠진 키는 영어로 나온다.
- **영어에 없는 키는 두지 않는다.** 영어가 버린 키가 남아 있으면 시험이 이름을 대며 잡는다.
- 한글·일본어·중국어는 한 글자가 두 칸이다. 표가 어긋날 걱정은 안 해도 된다 — 프로그램이
  칸 수로 재고, 다섯 말로 표가 서는지를 시험이 잰다.

## 골라 쓰는 법

쓰는 사람이 고른다. 둘 중 하나면 된다.

    MOAI_LANG=ja moai status        이번 한 번만

    ~/.config/moai/config.toml 에:
    [i18n]
    lang = "ja"

환경변수가 설정을 이긴다. 둘 다 없으면 **영어**다(moai-bn1j, 2026-09-20 사용자 결정 — v0.1.0 은
영어로 나간다). 한때 기본이 한국어였던 것은 말묶음에 든 글이 열 줄뿐이던 때의 대처였고, 그 조건은
`src/i18n.rs` 의 `#[default]` 가 `En` 으로 옮겨 가며 닫혔다 — 되돌리지 않는다. 한국어는
`MOAI_LANG=ko` 나 위의 설정으로 본다. `ja_JP.UTF-8` 처럼 로캘 모양으로 적어도 받는다. 모르는 값을 적으면 기본값으로 나오고, 설정 파일에 적은 값이 틀렸으면 `moai status` 가 한 줄로 댄다
(stderr 로 내고 막지는 않는다). `MOAI_LANG` 에 적은 모르는 값은 안 대고 설정으로 내려간다 —
이번 한 번의 값이라 다음 명령이면 없어질 것에 줄을 쓰지 않는다.

## 언어 더하기

새 언어는 파일 하나로 끝나지 않는다 — 세 자리를 함께 고친다.

1. `i18n/<코드>.json` 을 만든다. 영어 파일을 복사해 값을 옮기는 것이 쉽다.
2. `src/i18n.rs` 의 `Lang` 에 갈래를 더하고, `ALL`·`code()`·`bundle()` 셋에 그 갈래를 적는다.
   `bundle()` 이 `include_str!` 로 파일을 싣는다 — 번역은 바이너리에 함께 들어간다.
3. `cargo test` — 다섯 자리를 다 못 채웠으면 시험이 그 자리를 이름으로 댄다.

## 시험이 재는 것

    cargo test i18n

- 영어 표가 소스가 부르는 키를 다 갖는가 (`english_has_every_key_the_source_asks_for`).
  **읽는 자는 `say(…, "키")` 모양만 본다** — 그래서 키는 도우미에 넘기지 않고 `say` 부름에
  그대로 적는다(`view::says`). 키를 감추면 이 시험이 파란 채로 구멍이 뚫린다
- 번역이 영어의 자리(`{n}`)를 지키고, 빈 값이나 낡은 키가 없는가
  (`every_translation_keeps_the_places_english_marks`)
- 다섯 말에서 `ready` 표의 열이 같은 칸에 서는가 (`every_language_keeps_the_ready_table_in_line`)
- 실린 JSON 다섯이 다 읽히고 제 파일을 가리키는가 (`every_bundle_parses_and_is_wired_to_its_own_file`)
- 한국어 표가 영어 표의 키를 다 드는가 (`korean_carries_every_key_english_does`). ko 는 en 과 함께
  **다 찬** 말이라, 한 키가 빠지면 그 줄만 영어로 떨어져 화면이 비지 않는다 — 화면을 재는 시험은
  그것을 못 본다. 다른 셋은 빈 키를 영어가 받게 두므로 안 잰다

## 아직 다 안 옮긴 화면

글자는 표면 하나씩 옮긴다. 지금 말묶음에 든 것은 **저장소 안에서 부른** `status`·`ready`
두 화면이다 — 머리와 경고 글, 흐름 줄, 집을 수 있는 일의 표까지(moai-7cyf). 보드의 칸
이름은 그 저장소의 `config.toml` 에서 오는 낱말이라 번역하지 않는다.

**거절문과 몇 명령의 몸통도 들어왔다.** 모르는 칸을 대는 한 줄(`add -s`·`mv <칸>`·`mv --from`·
`show -s`·탐색기 거름망이 나눠 쓴다, moai-fdk7), 사용자 설정을 **고치다** 멈춘 까닭(moai-wflg),
그리고 `defer`·`edit`·`note`·`read`·`project add|rm|color|ls` 의 몸통과 `idea promote` 의 거절
셋(moai-95g1)이 말묶음에서 온다.
**`.moai` 를 못 찾았다는 첫 줄은 말묶음에 왔다**(moai-5j49) — `refuse.not_a_repo` 하나를
`cmd::open_repo` 와 `nothing_registered` 가 함께 쓴다. 등록한 것도 없을 때의 두 줄이 한 말로
서는 것을 `the_first_screen_outside_a_repo_stands_in_one_language` 가 잰다. 인자 없이 부른 맨
`moai` 의 두 줄(`opening.not_a_repo_yet`·`opening.register_to_view`)도 같은 시험이 잰다.
받은 사람이 그다음에 치는 `moai init` 의 화면(`init.*`·`refuse.init_*`)도 말묶음에 왔다(moai-9vwy) —
`the_init_screen_stands_in_one_language` 가 잰다. `init` 이 **심는** 파일(`.gitattributes`·
`config.toml`·`AGENTS.md` 블록)은 화면이 아니라 저장소의 내용이라 말묶음에 안 들고 영어 하나로 선다.

**읽음 표의 글 열도 말묶음에 왔다**(moai-rtji) — `read_marks` 는 자료만 내고(`SheetTrouble`·
`SheetRefusal`) `view::sheet_trouble`·`sheet_refusal` 이 `sheet.*` 로 편다. `moai read` 의 stderr 와
멈춘 까닭이 한 말로 서는 것을 `the_read_sheet_speaks_the_chosen_language` 가 잰다.

**설정을 읽다 멈춘 줄도 말묶음에 왔다**(moai-ivt9) — `config::Trouble`·`Refused`·`Want` 는
자료만 내고 `view::config_trouble`·`config_refused` 가 `config.*` 로 편다. 사람을 못 푼 줄
(`model::NoActor` → `view::no_actor`)과 git 이 이력을 못 낸 줄(`git::Error::said` →
`view::git_trouble`), 계획 파서의 거절(`draft.*`)도 같은 판에서 함께 왔다.

**남은 자리는 트래커가 든다** — 저장 계층에 남은 글(idea moai-uqxn, 쓰기 경로 안이다),
검증 글 열넷(idea moai-gbk3). 그것이 남은 동안은 한 명령이 두 말로 서는 자리가 있다:
못 읽는 프로젝트 한 줄이 영어 테두리 안에 `store` 의 한국어 까닭을 든다.

**같은 두 명령의 다른 자리는 아직 한국어다.** `.moai` 밖에서 부른 한눈 보기
(`view::projects_status`·`projects_ready`)는 머리 줄만 제 말이고 아래는 한국어다. `ready`
표의 에픽 칸에 서는 `(없는 에픽)` 은 `report.epic_gone` 의 낱말이다 — 여기는 옮겼다
(moai-ivt9). **괄호는 말묶음에 없다**(moai-snus) — `report::epic_labels` 는 찾았는가만
값으로 내고(`report::EpicLabel`), 감싸는 표기는 그리는 쪽이 정한다. **그 "그리는 쪽" 도 한
자리다**(리뷰) — `view::gone_epic` 이고, 네 표면(`list`·`ready`·`prime`·`detail`)이 그것을
부른다. 한때 상세만 제 괄호를 단 딴 키(`detail.missing_epic`)를 들어, 같은 자료를 두고
목록은 `(epic not there)` 라 하고 `show <id>` 는 `(no such epic)` 이라 했다. 그 밖의 화면
(`show` 의 상세와 이력, `tui`)도 아직 한국어로 박혀 있다 — 한 낱말이 두 화면에 서면
(`미룸`·`자식 없음`) 한쪽만 옮긴 동안은 같은 줄이 두 말로 보인다. 남은 자리는 트래커에 있다.

**훅은 반만 옮겼다**(moai-8d49). 세션에 싣는 글(`hook.*` — 보드 머리말, 접힌 뒤 싣는 줄,
`Stop` 이 붙드는 글)과 명령이 잘못 부른 것을 거절하는 다섯 줄(`refuse.*`)은 말묶음에 있다.
**규칙 1~4 의 거절문과 그 곁의 걸음(`guide::REVIEW_STEPS`·`handoff`)은 아직 아니다** — 그
글자는 `moai skill install` 이 심는 스킬·AGENTS.md 의 규칙 제목과 같아야 막힌 쪽이 무엇을
어겼는지 찾으니, 심는 문서를 어느 말로 심을지부터 정할 자리다.

**stderr 로 나가는 줄은 옮겼다**(moai-dpbi). `cmd::status`·`cmd::mod` 의 워크트리 알림
(`trouble.*`)과 `user_config::Doc::lang` 의 `i18n.lang` 오타 안내(`warn.lang_*`)가 말묶음에서
온다 — `warn.user_config` 와 `status.worktree_trouble` 이 "한 줄씩은 stderr 에 냈다" 고 제 말로
말하는 그 줄들이다.

**그 두 자리는 글을 안 짓는다.** 옆 워크트리의 문제는 `worktree::Trouble` 로, 설정의 탈은
`user_config::LangTrouble` 로 자료만 내고 `view::trouble_line`·`view::problem` 이 편다.
설정 쪽이 그런 까닭은 `Ctx::lang` 의 주석에 있다 — 설정을 읽는 길이 화면 말을 도로 물으면
`OnceLock` 이 제 초기화 안에서 다시 열린다.

**`--json` 의 사람 글도 고른 말을 따른다**(moai-dpbi 리뷰). `status`·`ready` 의 한눈 보기가 내는
`trouble` 과 `problems` 는 사람이 읽을 한 줄이라, 이제 `MOAI_LANG`·설정이 고른 말로 나간다(전에는
늘 한국어였다). `moai read --json` 의 `problems` 도 그렇다(moai-rtji). **기계가 가르는 값은 여기가
아니다** — `broken_worktrees`·`unreadable_worktrees` 처럼 `kind` 와 수를 든 곁의 키들이고, 경고도
`report::Warning` 의 `kind` 로 가른다. 그 두 배열을 글자로 맞춰 읽는 고리가 있으면 말이 바뀔 때 같이
깨진다 — 모양(배열·키)은 그대로다.

**`[[project]]` 항목의 탈은 `moai project` 와 함께 왔다.** 색 오타·경로 없음은 읽기도 쓰기도
같은 자(`hue_choice`·`entry_path`)를 쓰므로 한 자리에서 편다(`view::entry_problem`·
`view::write_trouble`). **탐색기의 배너는 아직 한국어다** — 화면 전체가 한국어라 같은 말로
편다(`tui::SAID`). 탐색기를 옮길 때 그 이름을 쫓으면 고칠 자리가 한눈에 선다.

**en·ko 말고는 새 키가 아직 비어 있다.** ja·zh·es 는 머리 몇 줄만 제 말이고 나머지는
영어로 떨어진다 — 다섯을 함께 채우게 하지 않는 것이 결정이라(위 "고치는 법"), 아는 사람이
제 말의 파일만 채우면 그 줄부터 선다.

## 시스템 로캘은 안 읽는다

`LANG`·`LC_ALL`·`LC_MESSAGES` 를 보지 않는다(2026-09-20 사용자 결정, moai-gv9n). 고르는 것은
`MOAI_LANG` 과 설정뿐이다. `ko_KR.UTF-8` 모양을 받아 주는 것은 `MOAI_LANG` 에 로캘을 그대로
붙여 넣는 사람을 받자는 것이지 로캘을 읽는다는 뜻이 아니다.

까닭은 기본을 한국어로 둔 것과 같다 — 옮긴 글이 화면을 다 덮기 전에 로캘로 말을 고르면,
아무것도 안 고른 `ja_JP` 기계의 사람이 일본어 몇 줄과 영어·한국어가 섞인 화면을 본다.
**여는 조건도 같다**: `src/i18n.rs` 의 `#[default]` 를 `En` 으로 옮기는 날 이 층을 함께 연다.
