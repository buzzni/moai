# CLI reference

**Generated from `moai --help`. Do not edit by hand** — run
`scripts/gen-cli-docs.sh` after `cargo build --release`, and commit the result.
A test compares this file against the binary's own help, so a reference that
drifts fails the build rather than misleading a reader.

The screen language is pinned to `ko` here, which is what the tool defaults to
today. Pass `MOAI_LANG=en` at runtime for English.

## `moai`

```
이슈 트래커. 승인 게이트 없음. 규율은 `moai status` 가 비춘다.

Usage: moai [OPTIONS] [COMMAND]

Commands:
  status        보드 · 경고 · 흐름. 세션은 여기서 시작한다
  ready         지금 집을 수 있는 일
  add           이슈를 만든다
  show          하나를 펼치거나 목록을 낸다
  mv            상태를 옮긴다
  edit          제목·본문·태그·에픽·우선순위를 고친다
  rm            지운다
  note          이슈에 메모를 남긴다 (저널에만 쌓인다)
  defer         지금 안 할 일을 계획에서 잠시 뺀다 (또는 도로 집는다)
  read          읽었다고 표시한다 (내 설정에만 남는다)
  link          하나가 다른 것을 막는다 (또는 그 막음을 없앤다)
  issue         위 동사를 `--type issue` 로 고정해 부른다
  epic          위 동사를 `--type epic` 으로 고정해 부른다
  milestone     위 동사를 `--type milestone` 으로 고정해 부른다
  idea          반짝 떠오른 것을 그 자리에서 담는다 (`--type idea`)
  tui           탐색기 화면을 띄운다 (이슈에 쓰는 것은 `SPC n` 생각 담기 하나)
  hook          Claude 의 훅이 부른다. stdin 으로 이벤트를 받아 낼 것만 낸다
  merge-driver  git 이 부른다. issues.jsonl 을 이슈마다 3-way 로 합친다
  skill         Claude 에 스킬과 훅을 심는다 (다시 불러도 된다)
  project       여러 프로젝트를 한 moai 에서 보려고 디렉터리를 등록한다
  init          이 저장소에 .moai/ 를 심는다 (다시 불러도 된다)
  help          Print this message or the help of the given subcommand(s)

Options:
      --json                기계가 읽는 출력. 사람 출력은 전부 사라진다
      --no-color            색을 끈다 (`--color never` 와 같다)
      --color <어떻게>      auto|always|never (기본 auto, 파이프면 끈다)
  -C, --dir <경로>          이 디렉터리에서 실행한다 (`git -C` 와 같다)
      --user <이름 (메일)>  누가 하는가 (없으면 `git config`)
  -h, --help                Print help
  -V, --version             Print version

세션은 이렇게 시작한다:

  moai status                   보드 · 경고 · 흐름. 인자 없이도 이것이 나온다
  moai ready                    지금 집을 수 있는 일
  moai show <id>                본문과 이력 — 왜 그렇게 정했는지가 여기 있다
  moai mv <id> in_progress      집는다.  끝나면 done
  moai note <id> '발견한 것'    다음 사람이 읽을 메모
  moai tui                      탐색기 — 에픽이 디렉터리처럼 열린다.
                                SPC n 으로 생각을 담는다

지금 할 일은 아닌 것이 떠오르면:

  moai idea add '반짝 떠오른 것'   담는다. 제목 하나면 된다 — 일로 세지 않는다
  moai idea promote <id> --from -  때가 되면 에픽과 이슈로 펼친다

이미 있는 일을 지금 안 할 때:

  moai defer <id> -m '다음 분기에'  계획에서 잠시 뺀다. 칸도 종류도 안 바뀐다
  moai defer <id> --undo           도로 집는다

여러 프로젝트를 한곳에서 볼 때:

  moai project add <dir>        등록하면 `.moai` 밖의 moai·status·ready 가
                                등록한 프로젝트를 한눈에 낸다
  moai -C <dir> <명령>          그 밖의 명령은 어느 프로젝트인지 댄다

화면의 말을 바꿀 때:

  MOAI_LANG=en moai status      영어 화면으로. en·ko·zh·ja·es 가 된다
                                늘 쓰려면 사용자 설정의 [i18n] 에 lang = "en"

계획을 한 번에 세울 때:

moai add --from - <<'PLAN'
# 에픽 제목
- [p1] 첫 이슈 #bug
PLAN

승인 게이트가 없다. 무엇이든 만들고 무엇이든 옮길 수 있다. 대신 `moai status` 가
에픽 없는 이슈·오래 멈춘 review·한 번에 벌여 놓은 것을 비춘다.

`moai <명령> --help` 가 그 명령의 전부를 낸다. 저장소에 AGENTS.md 가 있으면
그 저장소에서 일하는 절차가 거기 있다.
```

## `moai status`

```
보드 · 경고 · 흐름. 세션은 여기서 시작한다

Usage: moai status [OPTIONS]

Options:
      --worktree            다른 워크트리의 이슈도 겹쳐 본다 (파일은 안 바뀐다)
      --json                기계가 읽는 출력. 사람 출력은 전부 사라진다
      --no-color            색을 끈다 (`--color never` 와 같다)
      --color <어떻게>      auto|always|never (기본 auto, 파이프면 끈다)
  -C, --dir <경로>          이 디렉터리에서 실행한다 (`git -C` 와 같다)
      --user <이름 (메일)>  누가 하는가 (없으면 `git config`)
  -h, --help                Print help

  아무것도 막지 않는다. 승인도 통과도 없다.
  대신 에픽에 안 붙은 이슈, 오래 멈춘 review, 한 번에 벌여 놓은 것을 드러낸다.
  종료 코드는 데이터가 깨졌을 때만 0 이 아니다.

  --worktree 는 다른 git 워크트리의 이슈도 겹쳐 본다. 같은 id 는 칸을 옮기거나
  미루고 도로 집은 때가 늦은 줄이 서고(같으면 updated_at 이 늦은 줄), 지금
  브랜치가 아닌 줄은 제목 앞에 ⎇ <브랜치> 가 붙는다.
  여기서 rm 한 줄은 갈라진 뒤 옆에서 만지지 않았으면 되살아나지 않는다.
  옆 워크트리를 못 읽으면 보드가 "옆 워크트리 문제 N건" 으로 말한다 —
  종료 코드는 그대로다.
  보여줄 때만 겹친다 — 어느 파일도 바뀌지 않는다.

  무엇부터 잔소리할지는 .moai/config.toml 이 정한다. 안 적으면 아래 값이고,
  수는 따옴표 없이 적는다. 어느 값으로도 막지 않는다 — 낮추면 더 비출 뿐이다.
    status_review_days   = 3      review 에 이 날수를 넘겨 머물면 썩는 것으로
    status_wip_days      = 2      집어 놓고 이 날수를 넘겨 안 건드리면 잊은 것
    status_blocked_days  = 3      막힌 채 이 날수를 넘겨 서 있으면 멈춘 자리로
    status_wip_limit     = 3      한 번에 이보다 많이 벌이면
    status_no_epic_ratio = 0.15   에픽 없는 이슈가 이 비율부터
    status_no_epic_min   = 5      비율이 낮아도 이 수부터
    status_flow_days     = 7      흐름을 재는 창
    status_idea_pile     = 5      담아 둔 생각이 이만큼 쌓이면
```

## `moai ready`

```
지금 집을 수 있는 일

Usage: moai ready [OPTIONS]

Options:
      --worktree            다른 워크트리의 이슈도 겹쳐 본다 (파일은 안 바뀐다)
      --json                기계가 읽는 출력. 사람 출력은 전부 사라진다
      --no-color            색을 끈다 (`--color never` 와 같다)
      --color <어떻게>      auto|always|never (기본 auto, 파이프면 끈다)
  -C, --dir <경로>          이 디렉터리에서 실행한다 (`git -C` 와 같다)
      --user <이름 (메일)>  누가 하는가 (없으면 `git config`)
  -h, --help                Print help

  에픽 자체, 미뤄 둔 것과 그 밑, 아직 안 끝난 자식을 가진 부모는 뺀다.
  급한 것 → 끝나가는 에픽 → 오래된 것 차례로 낸다.

  --worktree 면 다른 워크트리에서 이미 집은 일은 여기서 빠지고, 잡고 있는
  것에 ⎇ <브랜치> 와 함께 선다. 같은 id 는 칸을 옮기거나 미루고 도로 집은
  때가 늦은 줄로 읽으므로, 옆에서 집은 뒤 여기서 제목·우선순위만 고쳐도
  옆에서 집은 것이 풀리지 않고, 옆에서 늦게 미룬 일을 여기서 집으라고 내지
  않는다.
```

## `moai add`

```
이슈를 만든다

Usage: moai add [OPTIONS] [제목]

Arguments:
  [제목]
          한 줄. 따옴표로 감싼다. `--` 로 시작해도 된다

Options:
  -e, --epic <id>
          이 에픽에 넣는다

      --milestone <id>
          이 마일스톤에 넣는다

  -t, --tag <태그>
          쉼표로 잇거나 여러 번 쓴다

  -p, --priority <0-3>
          0 이 가장 높다

  -s, --status <상태>
          처음 놓일 칸. 없으면 첫 칸

  -b, --body <글>
          본문. `-` 이면 stdin 에서 읽는다

  -a, --assignee <누구|none>
          담당. 안 주면 만든 사람, `이름 (메일)` 로 준다. `none` 이면 비운다

      --type <issue|epic|milestone|idea>
          만들 것의 종류 (없으면 issue, `epic add` 면 epic)

      --parent <id>
          이 이슈의 자식으로 만든다 (id 가 `.xxx` 로 붙는다)

      --from <파일|->
          마크다운에서 에픽과 이슈를 한 번에. `-` 이면 stdin

      --dry-run
          만들지 않고 무엇이 만들어질지만 낸다 (`--from` 과 함께)
          
          **`--from` 이 있어야 뜻이 있다.** 한때 없이도 받았고, 그때
          `moai add '제목' --dry-run` 은 연습이라고 적힌 줄을 찍은 다음 그것을
          실제로 만들었다 — 막는 줄 알고 부른 명령이 쓰는 것이 가장 나쁘다.

      --var <이름=값>
          계획 템플릿의 `{{이름}}` 을 채운다 (`--from` 과 함께, 여러 번 준다)
          
          **변수는 전부 필수다** — 못 채운 이름·빈 값·줄바꿈이 든 값·계획에
          없는 이름·같은 이름 두 번은 거절하고 아무것도 안 만든다. 이름은
          영문·숫자·`_`·`-` 이고, 값은 늘 제목 글자라 변수는 제목 자리에만
          둔다. `--dry-run` 과 같은 까닭으로 `--from` 없이 주면 거절한다.

  -q, --quiet
          id 만 낸다 (스크립트용)

      --json
          기계가 읽는 출력. 사람 출력은 전부 사라진다

      --no-color
          색을 끈다 (`--color never` 와 같다)

      --color <어떻게>
          auto|always|never (기본 auto, 파이프면 끈다)

  -C, --dir <경로>
          이 디렉터리에서 실행한다 (`git -C` 와 같다)

      --user <이름 (메일)>
          누가 하는가 (없으면 `git config`)

  -h, --help
          Print help (see a summary with '-h')

예시:
  moai add '파서가 BOM 에서 죽는다' -t bug -p 1
  moai add '저장 계층' --type epic
  moai add '부모에 딸린 일' --parent moai-4aex
  moai add '본문은 stdin 에서' -b -
  moai add '남에게' -a "철수 (chulsoo@example.com)"    안 주면 만든 이가 담당
  moai add '임자 없이' -a none

제목과 본문은 따로 넘긴다. 제목은 무엇이 어긋났는지 한 줄이다 — 보드와
`ready` 와 탐색기 목록은 제목만 보여 준다. 긴 글은 제목에 밀어 넣지 말고
본문으로 가른다. 본문은 마크다운이고 `-b -` 가 stdin 에서 읽는다:

moai add '파서가 BOM 에서 죽는다' -t bug -b - <<'BODY'
- 무엇이 어긋났는가: 앞머리 세 바이트를 제목으로 읽는다
- 어디를 고치는가: src/store.rs 의 읽기
BODY

한 번에 여럿 (`--from`):

moai add --from - <<'PLAN'
# 저장 계층
- [p1] 원자적으로 쓴다 #bug
- 잘린 줄을 복구한다
- \[WIP] 이슈 \#12
PLAN

  `#` 줄은 에픽, `-` 줄은 바로 위 에픽의 이슈다. [pN] 과 #태그 는 없어도 된다.
  제목의 앞머리 [ 와 끝 #낱말 은 \ 를 앞에 붙인다.
  --dry-run 이 heredoc 오타로 여섯 개를 잘못 만드는 것을 막는다.

계획 템플릿 (`{{이름}}` 을 --var 로 채운다, 변수는 전부 필수):
  moai add --from .moai/templates/release.md --var version=1.2

제목이 `--` 로 시작해도 된다. 아는 플래그가 아니면 제목으로 읽는다.
```

## `moai show`

```
하나를 펼치거나 목록을 낸다

Usage: moai show [OPTIONS] [대상]

Arguments:
  [대상]  이슈 id, 또는 종류(issue·epic). 없으면 전체 목록

Options:
      --raw                 본문을 그리지 않고 파일에 있는 그대로 낸다
      --tree                에픽 → 이슈 → 자식으로 접어 낸다
      --as-plan             에픽을 `add --from` 이 받는 마크다운으로 되뽑는다
      --worktree            다른 워크트리의 이슈도 겹쳐 본다 (파일은 안 바뀐다)
      --json                기계가 읽는 출력. 사람 출력은 전부 사라진다
      --no-color            색을 끈다 (`--color never` 와 같다)
      --color <어떻게>      auto|always|never (기본 auto, 파이프면 끈다)
  -C, --dir <경로>          이 디렉터리에서 실행한다 (`git -C` 와 같다)
      --user <이름 (메일)>  누가 하는가 (없으면 `git config`)
  -h, --help                Print help

필터  (쉼표 = 또는,  반복 = 그리고):
  -s, --status <상태>                     그 칸에 있는 것
  -t, --tag <태그>                        그 태그를 가진 것
      --no-tag <태그>                     그 태그가 없는 것
  -e, --epic <id|none>                    그 에픽 소속 (`none` = 에픽 없는 것)
      --milestone <id|none>               그 마일스톤 소속 (`none` = 없는 것)
      --parent <id|none>                  그 이슈의 자식 (`none` = 최상위만)
  -p, --priority <0-3>                    
  -a, --assignee <이름|메일|none|me>      그 담당 (`none`·`me` = 없음·나)
      --type <issue|epic|milestone|idea>  
  -g, --grep <글>                         id·제목·태그·본문에 이 글이 든 것
      --stale <일>                        지금 칸에 그만큼 머문 것
      --deferred                          미뤄 둔 것만
      --all                               done 과 미뤄 둔 것을 포함한다
      --filter <항목=값>                  필터를 한 문자열로 (`status=todo`)
```

## `moai mv`

```
상태를 옮긴다

Usage: moai mv [OPTIONS] <id>...

Arguments:
  <id>...
          옮길 이슈들, 그리고 맨 끝에 갈 칸

Options:
  -m, --msg <글>
          이 이동에 한 줄 메모 (저널에만 남는다)

      --from <칸>
          아직 이 칸에 있을 때만 옮긴다 (겨루는 집기)
          
          안 주면 지금까지처럼 무엇도 막지 않는다. 주면 락 안에서 다시 보고,
          그 사이에 칸이 달라진 줄은 건드리지 않은 채 부분 실패로 선다.

      --json
          기계가 읽는 출력. 사람 출력은 전부 사라진다

      --no-color
          색을 끈다 (`--color never` 와 같다)

      --color <어떻게>
          auto|always|never (기본 auto, 파이프면 끈다)

  -C, --dir <경로>
          이 디렉터리에서 실행한다 (`git -C` 와 같다)

      --user <이름 (메일)>
          누가 하는가 (없으면 `git config`)

  -h, --help
          Print help (see a summary with '-h')

  마지막 인자가 갈 칸이고, 그 앞이 전부 옮길 이슈다.
  칸 이름과 차례는 .moai/config.toml 의 statuses 가 정한다 (기본: todo,
  in_progress, review, done).

  순서를 건너뛰어도, 되돌려도, 막지 않는다. 이 도구에 승인은 없다.
  되감긴 것과 오래 멈춘 것은 `moai status` 가 드러낸다.

  moai mv moai-4aex in_progress
  moai mv moai-4aex moai-9k2p done
  moai mv moai-4aex review -m '테스트는 다음 이슈로 뺐다'

  여럿이 같은 .moai 를 쓰면 본 칸을 함께 준다. `--from` 은 락 안에서 다시 보고
  그 칸일 때만 옮긴다 — 진 쪽은 stderr 한 줄과 0 아닌 코드를 받는다. 그때는
  **id 를 하나만** 준다: 여럿이면 이긴 줄과 진 줄이 한 코드에 섞인다.

  moai mv moai-4aex in_progress --from todo
```

## `moai edit`

```
제목·본문·태그·에픽·우선순위를 고친다

Usage: moai edit [OPTIONS] <id>

Arguments:
  <id>  

Options:
      --title <글>            한 줄. `--` 로 시작해도 된다
  -b, --body <글>             본문. `-` 이면 stdin 에서 읽는다
  -t, --tag <태그>            태그를 더한다
      --untag <태그>          태그를 뺀다
  -e, --epic <id|none>        에픽을 옮긴다 (`none` 이면 제 필드만 뺀다)
      --milestone <id|none>   마일스톤을 옮긴다 (`none` 이면 제 필드만 뺀다)
  -p, --priority <0-3>        
  -a, --assignee <누구|none>  `이름 (메일)` 로 준다. `none` 이면 뺀다
      --json                  기계가 읽는 출력. 사람 출력은 전부 사라진다
      --no-color              색을 끈다 (`--color never` 와 같다)
      --color <어떻게>        auto|always|never (기본 auto, 파이프면 끈다)
  -C, --dir <경로>            이 디렉터리에서 실행한다 (`git -C` 와 같다)
      --user <이름 (메일)>    누가 하는가 (없으면 `git config`)
  -h, --help                  Print help

  `--epic none`·`--milestone none` 은 그 줄에 적힌 필드만 뺀다. 부모나
  에픽에게서 물려받는 소속은 남는다.

  제목과 본문은 따로 간다 — `--title` 은 한 줄, `-b` 는 마크다운 본문이고
  `-b -` 가 stdin 에서 읽는다. **`-b` 는 본문을 통째로 바꾼다.** 끝에 한 줄을
  더하려면 지금 본문을 먼저 읽어 이어 붙인다:

{ moai show <id> --json | jq -r '.body // empty'
printf '\n한 줄 더\n'; } | moai edit <id> -b -
```

## `moai rm`

```
지운다

Usage: moai rm [OPTIONS] <id>...

Arguments:
  <id>...  

Options:
      --json                기계가 읽는 출력. 사람 출력은 전부 사라진다
      --no-color            색을 끈다 (`--color never` 와 같다)
      --color <어떻게>      auto|always|never (기본 auto, 파이프면 끈다)
  -C, --dir <경로>          이 디렉터리에서 실행한다 (`git -C` 와 같다)
      --user <이름 (메일)>  누가 하는가 (없으면 `git config`)
  -h, --help                Print help
```

## `moai note`

```
이슈에 메모를 남긴다 (저널에만 쌓인다)

Usage: moai note [OPTIONS] <id> [글]

Arguments:
  <id>
          

  [글]
          다음 사람(또는 다음 에이전트)이 읽을 발견사항. `--` 로 시작해도 된다

Options:
  -b, --body <글>
          긴 글. `-` 이면 stdin 에서 읽는다
          
          **자리 인자와 서로 밀어낸다.** 둘 다 받으면 어느 쪽이 이기는지 아무도
          못 외우고, 외우지 못하는 규칙은 언젠가 남의 글을 지운다.

      --json
          기계가 읽는 출력. 사람 출력은 전부 사라진다

      --no-color
          색을 끈다 (`--color never` 와 같다)

      --color <어떻게>
          auto|always|never (기본 auto, 파이프면 끈다)

  -C, --dir <경로>
          이 디렉터리에서 실행한다 (`git -C` 와 같다)

      --user <이름 (메일)>
          누가 하는가 (없으면 `git config`)

  -h, --help
          Print help (see a summary with '-h')

  moai note moai-4aex '파서가 BOM 에서 죽는다'
  moai note moai-4aex -b - < review.txt        긴 글은 stdin 에서

moai note moai-4aex -b - <<'NOTE'
여러 줄의 긴 글
NOTE

  짧은 발견은 자리 인자로, 리뷰 전문처럼 긴 글은 `-b -` 로 넣는다. 둘은 서로
  밀어낸다 — 둘 다 받으면 어느 쪽이 이기는지 아무도 못 외운다.

  메모는 저널에만 쌓이고 스냅샷을 안 바꾼다. `moai show <id>` 가 이력으로 낸다.
```

## `moai defer`

```
지금 안 할 일을 계획에서 잠시 뺀다 (또는 도로 집는다)

Usage: moai defer [OPTIONS] <id>...

Arguments:
  <id>...
          

Options:
      --undo
          도로 집는다

  -m, --msg <글>
          왜 미루는가 (저널에만 남는다)

      --from <칸>
          이 칸에 있을 때만 미루거나 도로 집는다 (겨루는 집기)
          
          `mv --from` 과 같은 자다. 옆에서 집어 일하기 시작한 줄을 뒤늦게
          계획 밖으로 빼지 않는다. 안 주면 지금까지처럼 아무것도 막지 않는다.

      --json
          기계가 읽는 출력. 사람 출력은 전부 사라진다

      --no-color
          색을 끈다 (`--color never` 와 같다)

      --color <어떻게>
          auto|always|never (기본 auto, 파이프면 끈다)

  -C, --dir <경로>
          이 디렉터리에서 실행한다 (`git -C` 와 같다)

      --user <이름 (메일)>
          누가 하는가 (없으면 `git config`)

  -h, --help
          Print help (see a summary with '-h')

  moai defer moai-4aex                       미룬다
  moai defer moai-4aex moai-9k2p -m '다음 분기'   여럿을, 까닭과 함께
  moai defer moai-4aex --undo                도로 집는다

  **칸도 종류도 안 바꾼다.** 어느 칸에 있었는지는 도로 집을 때 그대로
  필요하고, 같은 줄이 그대로 돌아와야 한다. 미룬 것은 `moai ready` 와 보드와
  경고에서 빠지고, 쌓이면 `moai status` 가 한 줄로 비춘다. 에픽·마일스톤·
  부모를 미루면 그 밑의 일도 같이 빠진다.

  `moai show --deferred` 로 미뤄 둔 것만 본다.

  `--from <칸>` 은 `mv --from` 과 같은 자다 — 옆에서 집어 **칸이 움직인** 줄을
  뒤늦은 미루기가 계획 밖으로 빼지 않는다. 미루기는 칸을 안 바꾸므로, 겨루는
  둘이 **둘 다 미루는** 것은 이것으로 안 갈린다.

  moai defer moai-4aex -m '다음 분기' --from todo
```

## `moai read`

```
읽었다고 표시한다 (내 설정에만 남는다)

Usage: moai read [OPTIONS] [id]...

Arguments:
  [id]...
          읽음으로 적을 이슈들
          
          무엇을 읽었는지는 언제나 댄다 — 인자 없이 부르면 아무 줄도 안
          적으면서 성공으로 끝나, 사람은 다 적힌 줄 알고 넘어간다.
          `--all`·`-e` 가 그 자리를 채운다.

Options:
      --all
          내게 온 것 가운데 안 읽은 것 전부

  -e, --epic <에픽>
          그 에픽(또는 묶음)의 멤버와 그 밑까지

      --json
          기계가 읽는 출력. 사람 출력은 전부 사라진다

      --no-color
          색을 끈다 (`--color never` 와 같다)

      --color <어떻게>
          auto|always|never (기본 auto, 파이프면 끈다)

  -C, --dir <경로>
          이 디렉터리에서 실행한다 (`git -C` 와 같다)

      --user <이름 (메일)>
          누가 하는가 (없으면 `git config`)

  -h, --help
          Print help (see a summary with '-h')

  moai read moai-4aex              이 줄을 읽음으로
  moai read --all                  내게 온 것 가운데 안 읽은 것 전부
  moai read -e moai-9k2p           그 에픽의 멤버와 그 밑까지

  **트래커에 안 쓴다.** 읽음은 사람마다 다른 값이라 이슈 줄에 적으면 읽기만 해도
  남과 부딪힌다. 설정 곁의 <설정 디렉터리>/read/ 에 **프로젝트마다 한 파일**을
  두고 이슈 id 와 본 줄의 수정 때(updated_at)를 적는다 — 그 뒤에 그 줄이 바뀌면
  다시 안 읽음이 된다. 이름은 저장소 뿌리의 해시고, 어느 뿌리의 것인지는 그 안의
  path 가 댄다. 설정 파일의 옛 [read] 표는 겹쳐 읽기만 하고 다시 안 적는다.

  안 읽은 줄은 탐색기 목록에서 제목 앞에 [NEW] 로 선다 — 내게 할당된 것과 그 밑
  (자식·리뷰·에픽 멤버)만 센다.
```

## `moai link`

```
하나가 다른 것을 막는다 (또는 그 막음을 없앤다)

Usage: moai link [OPTIONS] <id>

Arguments:
  <id>  

Options:
      --blocks <id>         이 이슈(들)을 막는다
      --unblocks <id>       이 이슈(들)에 대한 막음을 없앤다
      --json                기계가 읽는 출력. 사람 출력은 전부 사라진다
      --no-color            색을 끈다 (`--color never` 와 같다)
      --color <어떻게>      auto|always|never (기본 auto, 파이프면 끈다)
  -C, --dir <경로>          이 디렉터리에서 실행한다 (`git -C` 와 같다)
      --user <이름 (메일)>  누가 하는가 (없으면 `git config`)
  -h, --help                Print help

  moai link moai-4aex --blocks moai-9k2p     4aex 가 9k2p 를 막는다
  moai link moai-4aex --unblocks moai-9k2p   그 막음을 없앤다

  막는 쪽이 아니라 막히는 쪽에 `blocked_by` 를 적는다. 고리(A 가 B 를
  막는데 B 도 이미 A 를 막고 있는 것)는 쓰기 전에 막는다.
```

## `moai issue`

```
위 동사를 `--type issue` 로 고정해 부른다

Usage: moai issue [OPTIONS] <COMMAND>

Commands:
  add   만든다
  show  펼치거나 목록을 낸다 (`ls` 도 같다)
  help  Print this message or the help of the given subcommand(s)

Options:
      --json                기계가 읽는 출력. 사람 출력은 전부 사라진다
      --no-color            색을 끈다 (`--color never` 와 같다)
      --color <어떻게>      auto|always|never (기본 auto, 파이프면 끈다)
  -C, --dir <경로>          이 디렉터리에서 실행한다 (`git -C` 와 같다)
      --user <이름 (메일)>  누가 하는가 (없으면 `git config`)
  -h, --help                Print help
```

## `moai issue add`

```
만든다

Usage: moai issue add [OPTIONS] [제목]

Arguments:
  [제목]
          한 줄. 따옴표로 감싼다. `--` 로 시작해도 된다

Options:
  -e, --epic <id>
          이 에픽에 넣는다

      --milestone <id>
          이 마일스톤에 넣는다

  -t, --tag <태그>
          쉼표로 잇거나 여러 번 쓴다

  -p, --priority <0-3>
          0 이 가장 높다

  -s, --status <상태>
          처음 놓일 칸. 없으면 첫 칸

  -b, --body <글>
          본문. `-` 이면 stdin 에서 읽는다

  -a, --assignee <누구|none>
          담당. 안 주면 만든 사람, `이름 (메일)` 로 준다. `none` 이면 비운다

      --type <issue|epic|milestone|idea>
          만들 것의 종류 (없으면 issue, `epic add` 면 epic)

      --parent <id>
          이 이슈의 자식으로 만든다 (id 가 `.xxx` 로 붙는다)

      --from <파일|->
          마크다운에서 에픽과 이슈를 한 번에. `-` 이면 stdin

      --dry-run
          만들지 않고 무엇이 만들어질지만 낸다 (`--from` 과 함께)
          
          **`--from` 이 있어야 뜻이 있다.** 한때 없이도 받았고, 그때
          `moai add '제목' --dry-run` 은 연습이라고 적힌 줄을 찍은 다음 그것을
          실제로 만들었다 — 막는 줄 알고 부른 명령이 쓰는 것이 가장 나쁘다.

      --var <이름=값>
          계획 템플릿의 `{{이름}}` 을 채운다 (`--from` 과 함께, 여러 번 준다)
          
          **변수는 전부 필수다** — 못 채운 이름·빈 값·줄바꿈이 든 값·계획에
          없는 이름·같은 이름 두 번은 거절하고 아무것도 안 만든다. 이름은
          영문·숫자·`_`·`-` 이고, 값은 늘 제목 글자라 변수는 제목 자리에만
          둔다. `--dry-run` 과 같은 까닭으로 `--from` 없이 주면 거절한다.

  -q, --quiet
          id 만 낸다 (스크립트용)

      --json
          기계가 읽는 출력. 사람 출력은 전부 사라진다

      --no-color
          색을 끈다 (`--color never` 와 같다)

      --color <어떻게>
          auto|always|never (기본 auto, 파이프면 끈다)

  -C, --dir <경로>
          이 디렉터리에서 실행한다 (`git -C` 와 같다)

      --user <이름 (메일)>
          누가 하는가 (없으면 `git config`)

  -h, --help
          Print help (see a summary with '-h')

  제목과 본문은 따로 넘긴다 — 제목은 무엇이 어긋났는지 한 줄이고, 긴 글은
  `-b -` 로 stdin 에서 흘리는 마크다운 본문이다. 예시는 `moai add --help`.
```

## `moai issue show`

```
펼치거나 목록을 낸다 (`ls` 도 같다)

Usage: moai issue show [OPTIONS] [대상]

Arguments:
  [대상]  이슈 id, 또는 종류(issue·epic). 없으면 전체 목록

Options:
      --raw                 본문을 그리지 않고 파일에 있는 그대로 낸다
      --tree                에픽 → 이슈 → 자식으로 접어 낸다
      --as-plan             에픽을 `add --from` 이 받는 마크다운으로 되뽑는다
      --worktree            다른 워크트리의 이슈도 겹쳐 본다 (파일은 안 바뀐다)
      --json                기계가 읽는 출력. 사람 출력은 전부 사라진다
      --no-color            색을 끈다 (`--color never` 와 같다)
      --color <어떻게>      auto|always|never (기본 auto, 파이프면 끈다)
  -C, --dir <경로>          이 디렉터리에서 실행한다 (`git -C` 와 같다)
      --user <이름 (메일)>  누가 하는가 (없으면 `git config`)
  -h, --help                Print help

필터  (쉼표 = 또는,  반복 = 그리고):
  -s, --status <상태>                     그 칸에 있는 것
  -t, --tag <태그>                        그 태그를 가진 것
      --no-tag <태그>                     그 태그가 없는 것
  -e, --epic <id|none>                    그 에픽 소속 (`none` = 에픽 없는 것)
      --milestone <id|none>               그 마일스톤 소속 (`none` = 없는 것)
      --parent <id|none>                  그 이슈의 자식 (`none` = 최상위만)
  -p, --priority <0-3>                    
  -a, --assignee <이름|메일|none|me>      그 담당 (`none`·`me` = 없음·나)
      --type <issue|epic|milestone|idea>  
  -g, --grep <글>                         id·제목·태그·본문에 이 글이 든 것
      --stale <일>                        지금 칸에 그만큼 머문 것
      --deferred                          미뤄 둔 것만
      --all                               done 과 미뤄 둔 것을 포함한다
      --filter <항목=값>                  필터를 한 문자열로 (`status=todo`)
```

## `moai epic`

```
위 동사를 `--type epic` 으로 고정해 부른다

Usage: moai epic [OPTIONS] <COMMAND>

Commands:
  add   만든다
  show  펼치거나 목록을 낸다 (`ls` 도 같다)
  help  Print this message or the help of the given subcommand(s)

Options:
      --json                기계가 읽는 출력. 사람 출력은 전부 사라진다
      --no-color            색을 끈다 (`--color never` 와 같다)
      --color <어떻게>      auto|always|never (기본 auto, 파이프면 끈다)
  -C, --dir <경로>          이 디렉터리에서 실행한다 (`git -C` 와 같다)
      --user <이름 (메일)>  누가 하는가 (없으면 `git config`)
  -h, --help                Print help
```

## `moai epic add`

```
만든다

Usage: moai epic add [OPTIONS] [제목]

Arguments:
  [제목]
          한 줄. 따옴표로 감싼다. `--` 로 시작해도 된다

Options:
  -e, --epic <id>
          이 에픽에 넣는다

      --milestone <id>
          이 마일스톤에 넣는다

  -t, --tag <태그>
          쉼표로 잇거나 여러 번 쓴다

  -p, --priority <0-3>
          0 이 가장 높다

  -s, --status <상태>
          처음 놓일 칸. 없으면 첫 칸

  -b, --body <글>
          본문. `-` 이면 stdin 에서 읽는다

  -a, --assignee <누구|none>
          담당. 안 주면 만든 사람, `이름 (메일)` 로 준다. `none` 이면 비운다

      --type <issue|epic|milestone|idea>
          만들 것의 종류 (없으면 issue, `epic add` 면 epic)

      --parent <id>
          이 이슈의 자식으로 만든다 (id 가 `.xxx` 로 붙는다)

      --from <파일|->
          마크다운에서 에픽과 이슈를 한 번에. `-` 이면 stdin

      --dry-run
          만들지 않고 무엇이 만들어질지만 낸다 (`--from` 과 함께)
          
          **`--from` 이 있어야 뜻이 있다.** 한때 없이도 받았고, 그때
          `moai add '제목' --dry-run` 은 연습이라고 적힌 줄을 찍은 다음 그것을
          실제로 만들었다 — 막는 줄 알고 부른 명령이 쓰는 것이 가장 나쁘다.

      --var <이름=값>
          계획 템플릿의 `{{이름}}` 을 채운다 (`--from` 과 함께, 여러 번 준다)
          
          **변수는 전부 필수다** — 못 채운 이름·빈 값·줄바꿈이 든 값·계획에
          없는 이름·같은 이름 두 번은 거절하고 아무것도 안 만든다. 이름은
          영문·숫자·`_`·`-` 이고, 값은 늘 제목 글자라 변수는 제목 자리에만
          둔다. `--dry-run` 과 같은 까닭으로 `--from` 없이 주면 거절한다.

  -q, --quiet
          id 만 낸다 (스크립트용)

      --json
          기계가 읽는 출력. 사람 출력은 전부 사라진다

      --no-color
          색을 끈다 (`--color never` 와 같다)

      --color <어떻게>
          auto|always|never (기본 auto, 파이프면 끈다)

  -C, --dir <경로>
          이 디렉터리에서 실행한다 (`git -C` 와 같다)

      --user <이름 (메일)>
          누가 하는가 (없으면 `git config`)

  -h, --help
          Print help (see a summary with '-h')

  제목과 본문은 따로 넘긴다 — 제목은 무엇이 어긋났는지 한 줄이고, 긴 글은
  `-b -` 로 stdin 에서 흘리는 마크다운 본문이다. 예시는 `moai add --help`.
```

## `moai epic show`

```
펼치거나 목록을 낸다 (`ls` 도 같다)

Usage: moai epic show [OPTIONS] [대상]

Arguments:
  [대상]  이슈 id, 또는 종류(issue·epic). 없으면 전체 목록

Options:
      --raw                 본문을 그리지 않고 파일에 있는 그대로 낸다
      --tree                에픽 → 이슈 → 자식으로 접어 낸다
      --as-plan             에픽을 `add --from` 이 받는 마크다운으로 되뽑는다
      --worktree            다른 워크트리의 이슈도 겹쳐 본다 (파일은 안 바뀐다)
      --json                기계가 읽는 출력. 사람 출력은 전부 사라진다
      --no-color            색을 끈다 (`--color never` 와 같다)
      --color <어떻게>      auto|always|never (기본 auto, 파이프면 끈다)
  -C, --dir <경로>          이 디렉터리에서 실행한다 (`git -C` 와 같다)
      --user <이름 (메일)>  누가 하는가 (없으면 `git config`)
  -h, --help                Print help

필터  (쉼표 = 또는,  반복 = 그리고):
  -s, --status <상태>                     그 칸에 있는 것
  -t, --tag <태그>                        그 태그를 가진 것
      --no-tag <태그>                     그 태그가 없는 것
  -e, --epic <id|none>                    그 에픽 소속 (`none` = 에픽 없는 것)
      --milestone <id|none>               그 마일스톤 소속 (`none` = 없는 것)
      --parent <id|none>                  그 이슈의 자식 (`none` = 최상위만)
  -p, --priority <0-3>                    
  -a, --assignee <이름|메일|none|me>      그 담당 (`none`·`me` = 없음·나)
      --type <issue|epic|milestone|idea>  
  -g, --grep <글>                         id·제목·태그·본문에 이 글이 든 것
      --stale <일>                        지금 칸에 그만큼 머문 것
      --deferred                          미뤄 둔 것만
      --all                               done 과 미뤄 둔 것을 포함한다
      --filter <항목=값>                  필터를 한 문자열로 (`status=todo`)
```

## `moai milestone`

```
위 동사를 `--type milestone` 으로 고정해 부른다

Usage: moai milestone [OPTIONS] <COMMAND>

Commands:
  add   만든다
  show  펼치거나 목록을 낸다 (`ls` 도 같다)
  help  Print this message or the help of the given subcommand(s)

Options:
      --json                기계가 읽는 출력. 사람 출력은 전부 사라진다
      --no-color            색을 끈다 (`--color never` 와 같다)
      --color <어떻게>      auto|always|never (기본 auto, 파이프면 끈다)
  -C, --dir <경로>          이 디렉터리에서 실행한다 (`git -C` 와 같다)
      --user <이름 (메일)>  누가 하는가 (없으면 `git config`)
  -h, --help                Print help
```

## `moai milestone add`

```
만든다

Usage: moai milestone add [OPTIONS] [제목]

Arguments:
  [제목]
          한 줄. 따옴표로 감싼다. `--` 로 시작해도 된다

Options:
  -e, --epic <id>
          이 에픽에 넣는다

      --milestone <id>
          이 마일스톤에 넣는다

  -t, --tag <태그>
          쉼표로 잇거나 여러 번 쓴다

  -p, --priority <0-3>
          0 이 가장 높다

  -s, --status <상태>
          처음 놓일 칸. 없으면 첫 칸

  -b, --body <글>
          본문. `-` 이면 stdin 에서 읽는다

  -a, --assignee <누구|none>
          담당. 안 주면 만든 사람, `이름 (메일)` 로 준다. `none` 이면 비운다

      --type <issue|epic|milestone|idea>
          만들 것의 종류 (없으면 issue, `epic add` 면 epic)

      --parent <id>
          이 이슈의 자식으로 만든다 (id 가 `.xxx` 로 붙는다)

      --from <파일|->
          마크다운에서 에픽과 이슈를 한 번에. `-` 이면 stdin

      --dry-run
          만들지 않고 무엇이 만들어질지만 낸다 (`--from` 과 함께)
          
          **`--from` 이 있어야 뜻이 있다.** 한때 없이도 받았고, 그때
          `moai add '제목' --dry-run` 은 연습이라고 적힌 줄을 찍은 다음 그것을
          실제로 만들었다 — 막는 줄 알고 부른 명령이 쓰는 것이 가장 나쁘다.

      --var <이름=값>
          계획 템플릿의 `{{이름}}` 을 채운다 (`--from` 과 함께, 여러 번 준다)
          
          **변수는 전부 필수다** — 못 채운 이름·빈 값·줄바꿈이 든 값·계획에
          없는 이름·같은 이름 두 번은 거절하고 아무것도 안 만든다. 이름은
          영문·숫자·`_`·`-` 이고, 값은 늘 제목 글자라 변수는 제목 자리에만
          둔다. `--dry-run` 과 같은 까닭으로 `--from` 없이 주면 거절한다.

  -q, --quiet
          id 만 낸다 (스크립트용)

      --json
          기계가 읽는 출력. 사람 출력은 전부 사라진다

      --no-color
          색을 끈다 (`--color never` 와 같다)

      --color <어떻게>
          auto|always|never (기본 auto, 파이프면 끈다)

  -C, --dir <경로>
          이 디렉터리에서 실행한다 (`git -C` 와 같다)

      --user <이름 (메일)>
          누가 하는가 (없으면 `git config`)

  -h, --help
          Print help (see a summary with '-h')

  제목과 본문은 따로 넘긴다 — 제목은 무엇이 어긋났는지 한 줄이고, 긴 글은
  `-b -` 로 stdin 에서 흘리는 마크다운 본문이다. 예시는 `moai add --help`.
```

## `moai milestone show`

```
펼치거나 목록을 낸다 (`ls` 도 같다)

Usage: moai milestone show [OPTIONS] [대상]

Arguments:
  [대상]  이슈 id, 또는 종류(issue·epic). 없으면 전체 목록

Options:
      --raw                 본문을 그리지 않고 파일에 있는 그대로 낸다
      --tree                에픽 → 이슈 → 자식으로 접어 낸다
      --as-plan             에픽을 `add --from` 이 받는 마크다운으로 되뽑는다
      --worktree            다른 워크트리의 이슈도 겹쳐 본다 (파일은 안 바뀐다)
      --json                기계가 읽는 출력. 사람 출력은 전부 사라진다
      --no-color            색을 끈다 (`--color never` 와 같다)
      --color <어떻게>      auto|always|never (기본 auto, 파이프면 끈다)
  -C, --dir <경로>          이 디렉터리에서 실행한다 (`git -C` 와 같다)
      --user <이름 (메일)>  누가 하는가 (없으면 `git config`)
  -h, --help                Print help

필터  (쉼표 = 또는,  반복 = 그리고):
  -s, --status <상태>                     그 칸에 있는 것
  -t, --tag <태그>                        그 태그를 가진 것
      --no-tag <태그>                     그 태그가 없는 것
  -e, --epic <id|none>                    그 에픽 소속 (`none` = 에픽 없는 것)
      --milestone <id|none>               그 마일스톤 소속 (`none` = 없는 것)
      --parent <id|none>                  그 이슈의 자식 (`none` = 최상위만)
  -p, --priority <0-3>                    
  -a, --assignee <이름|메일|none|me>      그 담당 (`none`·`me` = 없음·나)
      --type <issue|epic|milestone|idea>  
  -g, --grep <글>                         id·제목·태그·본문에 이 글이 든 것
      --stale <일>                        지금 칸에 그만큼 머문 것
      --deferred                          미뤄 둔 것만
      --all                               done 과 미뤄 둔 것을 포함한다
      --filter <항목=값>                  필터를 한 문자열로 (`status=todo`)
```

## `moai idea`

```
반짝 떠오른 것을 그 자리에서 담는다 (`--type idea`)

Usage: moai idea [OPTIONS] <COMMAND>

Commands:
  add      만든다
  show     펼치거나 목록을 낸다 (`ls` 도 같다)
  promote  에픽 하나 + 이슈 여럿으로 펼치고, 그 생각을 닫는다
  help     Print this message or the help of the given subcommand(s)

Options:
      --json                기계가 읽는 출력. 사람 출력은 전부 사라진다
      --no-color            색을 끈다 (`--color never` 와 같다)
      --color <어떻게>      auto|always|never (기본 auto, 파이프면 끈다)
  -C, --dir <경로>          이 디렉터리에서 실행한다 (`git -C` 와 같다)
      --user <이름 (메일)>  누가 하는가 (없으면 `git config`)
  -h, --help                Print help

  todo 보다 한 칸 낮은 자리다. **담는 비용이 0 에 가까워야 담는다** —
  우선순위도 에픽도 묻지 않는다. 제목과 본문은 그래도 가른다: 제목은 한 줄로
  짧게 적고, 긴 생각은 `-b -` 로 본문에 흘린다. 펼칠 때 이 제목이 이슈 제목이
  되니, 여기 적은 긴 제목은 이슈로 그대로 옮겨 간다.

  moai idea add '반짝 떠오른 것'      담기
  moai idea ls                        쌓인 것 보기 (`idea show` 와 같다)

moai idea add '머지 드라이버를 클론마다 손으로 심는다' -b - <<'IDEA'
지금은 `moai merge-driver --install` 을 사람이 한 번 쳐야 한다.
IDEA

  idea 는 일이 아니다 — `moai ready` 에도 보드의 셈에도 들지 않고, 에픽 없이
  사는 것이 정상이라 "에픽 없는 이슈" 경고에 안 걸린다.

  고치고 버리는 것은 이미 있는 동사가 한다: `moai edit <id>`, `moai rm <id>`.
```

## `moai idea add`

```
만든다

Usage: moai idea add [OPTIONS] [제목]

Arguments:
  [제목]
          한 줄. 따옴표로 감싼다. `--` 로 시작해도 된다

Options:
  -e, --epic <id>
          이 에픽에 넣는다

      --milestone <id>
          이 마일스톤에 넣는다

  -t, --tag <태그>
          쉼표로 잇거나 여러 번 쓴다

  -p, --priority <0-3>
          0 이 가장 높다

  -s, --status <상태>
          처음 놓일 칸. 없으면 첫 칸

  -b, --body <글>
          본문. `-` 이면 stdin 에서 읽는다

  -a, --assignee <누구|none>
          담당. 안 주면 만든 사람, `이름 (메일)` 로 준다. `none` 이면 비운다

      --type <issue|epic|milestone|idea>
          만들 것의 종류 (없으면 issue, `epic add` 면 epic)

      --parent <id>
          이 이슈의 자식으로 만든다 (id 가 `.xxx` 로 붙는다)

      --from <파일|->
          마크다운에서 에픽과 이슈를 한 번에. `-` 이면 stdin

      --dry-run
          만들지 않고 무엇이 만들어질지만 낸다 (`--from` 과 함께)
          
          **`--from` 이 있어야 뜻이 있다.** 한때 없이도 받았고, 그때
          `moai add '제목' --dry-run` 은 연습이라고 적힌 줄을 찍은 다음 그것을
          실제로 만들었다 — 막는 줄 알고 부른 명령이 쓰는 것이 가장 나쁘다.

      --var <이름=값>
          계획 템플릿의 `{{이름}}` 을 채운다 (`--from` 과 함께, 여러 번 준다)
          
          **변수는 전부 필수다** — 못 채운 이름·빈 값·줄바꿈이 든 값·계획에
          없는 이름·같은 이름 두 번은 거절하고 아무것도 안 만든다. 이름은
          영문·숫자·`_`·`-` 이고, 값은 늘 제목 글자라 변수는 제목 자리에만
          둔다. `--dry-run` 과 같은 까닭으로 `--from` 없이 주면 거절한다.

  -q, --quiet
          id 만 낸다 (스크립트용)

      --json
          기계가 읽는 출력. 사람 출력은 전부 사라진다

      --no-color
          색을 끈다 (`--color never` 와 같다)

      --color <어떻게>
          auto|always|never (기본 auto, 파이프면 끈다)

  -C, --dir <경로>
          이 디렉터리에서 실행한다 (`git -C` 와 같다)

      --user <이름 (메일)>
          누가 하는가 (없으면 `git config`)

  -h, --help
          Print help (see a summary with '-h')

  제목과 본문은 따로 넘긴다 — 제목은 무엇이 어긋났는지 한 줄이고, 긴 글은
  `-b -` 로 stdin 에서 흘리는 마크다운 본문이다. 예시는 `moai add --help`.
```

## `moai idea show`

```
펼치거나 목록을 낸다 (`ls` 도 같다)

Usage: moai idea show [OPTIONS] [대상]

Arguments:
  [대상]  이슈 id, 또는 종류(issue·epic). 없으면 전체 목록

Options:
      --raw                 본문을 그리지 않고 파일에 있는 그대로 낸다
      --tree                에픽 → 이슈 → 자식으로 접어 낸다
      --as-plan             에픽을 `add --from` 이 받는 마크다운으로 되뽑는다
      --worktree            다른 워크트리의 이슈도 겹쳐 본다 (파일은 안 바뀐다)
      --json                기계가 읽는 출력. 사람 출력은 전부 사라진다
      --no-color            색을 끈다 (`--color never` 와 같다)
      --color <어떻게>      auto|always|never (기본 auto, 파이프면 끈다)
  -C, --dir <경로>          이 디렉터리에서 실행한다 (`git -C` 와 같다)
      --user <이름 (메일)>  누가 하는가 (없으면 `git config`)
  -h, --help                Print help

필터  (쉼표 = 또는,  반복 = 그리고):
  -s, --status <상태>                     그 칸에 있는 것
  -t, --tag <태그>                        그 태그를 가진 것
      --no-tag <태그>                     그 태그가 없는 것
  -e, --epic <id|none>                    그 에픽 소속 (`none` = 에픽 없는 것)
      --milestone <id|none>               그 마일스톤 소속 (`none` = 없는 것)
      --parent <id|none>                  그 이슈의 자식 (`none` = 최상위만)
  -p, --priority <0-3>                    
  -a, --assignee <이름|메일|none|me>      그 담당 (`none`·`me` = 없음·나)
      --type <issue|epic|milestone|idea>  
  -g, --grep <글>                         id·제목·태그·본문에 이 글이 든 것
      --stale <일>                        지금 칸에 그만큼 머문 것
      --deferred                          미뤄 둔 것만
      --all                               done 과 미뤄 둔 것을 포함한다
      --filter <항목=값>                  필터를 한 문자열로 (`status=todo`)
```

## `moai idea promote`

```
에픽 하나 + 이슈 여럿으로 펼치고, 그 생각을 닫는다

Usage: moai idea promote [OPTIONS] --from <파일|-> <id>

Arguments:
  <id>  펼칠 idea

Options:
      --from <파일|->       마크다운에서 에픽과 이슈를. `-` 이면 stdin
  -e, --epic <에픽>         새 에픽 대신 이미 선 이 에픽에 멤버로 펼친다
      --var <이름=값>       템플릿의 `{{이름}}` 을 채운다 (여러 번 준다)
      --dry-run             만들지 않고 무엇이 만들어질지만 낸다
      --json                기계가 읽는 출력. 사람 출력은 전부 사라진다
      --no-color            색을 끈다 (`--color never` 와 같다)
      --color <어떻게>      auto|always|never (기본 auto, 파이프면 끈다)
  -C, --dir <경로>          이 디렉터리에서 실행한다 (`git -C` 와 같다)
      --user <이름 (메일)>  누가 하는가 (없으면 `git config`)
  -h, --help                Print help

  받는 마크다운은 `add --from` 과 같은 형식이다. 형식이 둘이 되면 어느 쪽
  문법인지 매번 틀린다.

moai idea promote <id> --from - <<'PLAN'
# 에픽 제목
- [p1] 첫 이슈 #enhancement
- [p2] 둘째 이슈
PLAN

  **계획에 적은 줄이 그대로 이슈 제목이 된다.** idea 의 제목이 길면 그 길이가
  이슈로 옮겨 가니, 펼칠 때 제목을 짧게 새로 적는다. 원래 글은 그 idea 에
  그대로 남아 이력에서 찾아간다.

  펼치면 닫힌다 — 그 idea 는 `done` 으로 간다. 무엇이 무엇에서 나왔는지는
  저널에 남는다 (`moai show <id>` 의 이력).

  에픽이 이미 서 있으면 `-e <에픽>` 으로 그 에픽의 멤버로 펼친다. 에픽이
  내건 것이 idea 로 밖에 나가 있던 것을 되찾는 자리다 — 계획에는
  `- 이슈` 만 적는다.

moai idea promote <id> -e <에픽> --from - <<'PLAN'
- [p1] 에픽이 내건 것
PLAN

  `--dry-run` 이 펼친 안을 사람이 한 번 보고 "좋다" 하는 자리다.
```

## `moai tui`

```
탐색기 화면을 띄운다 (이슈에 쓰는 것은 `SPC n` 생각 담기 하나)

Usage: moai tui [OPTIONS]

Options:
      --path <id|없음|길잃음>  여기서 연다 — 디렉터리면 그 안, 아니면 든 곳
      --json                   기계가 읽는 출력. 사람 출력은 전부 사라진다
      --no-color               색을 끈다 (`--color never` 와 같다)
      --color <어떻게>         auto|always|never (기본 auto, 파이프면 끈다)
  -C, --dir <경로>             이 디렉터리에서 실행한다 (`git -C` 와 같다)
      --user <이름 (메일)>     누가 하는가 (없으면 `git config`)
  -h, --help                   Print help

  마일스톤과 에픽이 디렉터리처럼 동작한다. 왼쪽에서 돌아다니면 커서가 머문
  것의 정보가 오른쪽에 나온다.

  j·k 나 ↑↓ 로 이동, Enter 로 들어가고 Backspace 로 나온다.
  마일스톤·에픽 줄에서 l·→ 은 그 자리에서 한 단계 펼치고 h·← 은 접는다.
  펼친 멤버 줄의 h 는 그 부모를 접고 부모 줄에 서며, 접을 것이 없으면 한 층
  나간다. 프로젝트 머리줄에서도 l·→ 이 펼치고 h·← 이 접는다. Tab 은 그 밑을
  재귀로 다 펼치고, 다시 누르면 접는다. 펼친 멤버는 제목 칸에 가지(├─·└─)로
  서고, 그 펼침은 설정에 안 남는다. gg·Home 이 맨 위, G·End 가 맨 아래,
  Ctrl-d·Ctrl-u 가 반 쪽, Ctrl-f·Ctrl-b(PageDown·PageUp)가 한 쪽이다.
  Ctrl-w w 가 목록과 상세 사이로
  포커스를 옮기고(Ctrl-w W 가 거꾸로, Ctrl-w h·Ctrl-w l 이 왼쪽·오른쪽 칸),
  이동키는 모두 포커스 있는 칸을 움직인다 — 상세를 굴리려면 Ctrl-w w 로 간다.
  / 가 검색, Esc 가 걸어 둔 거름망을 푼다. r 은 커서가 선 줄을 읽음으로
  적는다(아래 [NEW]).
  검색·거름망 칸은 Enter 로 걸고 Esc 로 그만두며, 검색은 치는 대로 목록을
  거르고 Tab·Shift-Tab 이 찾을 자리를 전체·id·제목·태그·본문으로 돌린다. 맨
  위 헤더가 등록한 프로젝트마다 번호를 대고, 그 숫자를 SPC 없이 그대로 누르면 그
  프로젝트로 바로 간다 — 0 은 전체, 곧 모든 프로젝트를 한 목록으로 보는 자리다.

  그 밖의 동작은 SPC 를 누르면 곧바로 뜨는 메뉴에 있다. 메뉴는 그 자리에서 되는
  것만 세우고, 모르는 키는 무시하며, Esc 나 SPC 로 닫고 Backspace 로 한 층
  올라간다. 켜고 끄는 것과 정렬(SPC v·SPC c·SPC s)은 눌러도 안 닫힌다 — 눌러
  보며 상태를 맞추고 Esc 로 나간다. 그 층은 아랫줄 오른쪽에 Esc 닫기 가 서서
  기다린다고 알린다.
    SPC /    검색               SPC f    거름망             SPC n    생각 담기
    SPC q    끝내기
    SPC p a  등록               SPC p d  목록에서 빼기
  보기 — 목록의 열(SPC c) 말고 켜고 끄는 것은 모두 여기 있다:
    SPC v d  done [보임/숨김]   SPC v l  미룸               SPC v a  모두 보이기
    SPC v 1  설정의 첫 칸 [보임/숨김] — 둘째 칸부터 번호가 차례로 는다
    SPC v p  상세 칸 [보임/숨김]
    SPC v w  워크트리 겹쳐 보기 [켜짐/꺼짐]
    SPC v r  원문↔그리기
  정렬과 열은 우선순위·생성·수정·담당을 같은 글자로 부른다 — 제목(SPC s t)과
  태그(SPC c t)만 한 글자에 뜻이 갈린다:
    SPC s p  우선순위           SPC s c  생성               SPC s u  수정
    SPC s s  칸                 SPC s a  담당               SPC s t  제목
    SPC c i  id                 SPC c p  우선순위           SPC c a  담당
    SPC c c  생성               SPC c u  수정               SPC c n  셈
    SPC c t  태그               SPC c h  열 이름 줄 [보임/숨김]
    SPC c w  ⎇ 옆 가지 표시 [보임/숨김] — SPC v w 로 겹쳐 봐야 선다
  읽음:
    SPC m a  안 읽은 것 전부    SPC m g  이 묶음의 멤버 전부
  바로 끝내는 키는 Ctrl-C 하나다 — 어디서든, 글을 적는 중에도 끝낸다.
  화면은 저절로 다시 읽는다 — 옆에서 쓴 이슈도, 옆 터미널의 `moai read` 와
  `moai project add` 도 누르지 않고 선다.

  내게 온 것(담당이 나이거나 그 밑) 가운데 마지막으로 본 뒤에 바뀐 줄은
  제목 앞에 [NEW] 가 선다. 읽음은 내 설정에만 남고 트래커는 안 바뀐다 —
  CLI 로는 `moai read` 다.

  목록은 처음에 done 을 숨긴다 — 경로 줄의 [done 숨김] 이 그것을 댄다. 보기는
  거름망과 따로라 Esc 로 안 풀리고, 둘은 함께 걸린다.
  정렬은 급한 것·새것·앞 칸·가나다가 위고, 고른 것을 다시 누르면 거꾸로 선다.
  기본(우선순위)이 아니면 경로 줄이 그 차례를 댄다.
  열(SPC c)은 [보임/숨김] 으로 켜고 끈다. 담당·태그·생성·수정 날짜는 줄
  오른쪽에 서고, 좁으면 날짜 → 담당 → 태그 차례로 걷혀 제목 몫을 남긴다.
  보기·정렬·열은 누를 때마다 사용자 설정의 [tui] 표에 적혀 다음 실행과 다른
  프로젝트로 이어진다(`moai project add` 가 쓰는 파일과 같다).

  등록한 프로젝트(`moai project add`)가 있으면 0 이 그것들을 한 목록으로
  낸다 — 프로젝트마다 머리줄이 서고 그 밑에 그 프로젝트의 줄이 선다. `.moai`
  밖에서 띄우면 거기서 시작하고, 안에서 띄우면 그 프로젝트 안에서 시작한다.
  0 으로 나온 뒤에는 있던 프로젝트가 펼쳐진 채 서고 나머지는 머리줄만 선다 —
  펼치는 그때 그 프로젝트를 읽는다(읽는 동안 머리줄이 돈다).
  머리줄의 Enter 는 그 프로젝트 안으로 들어가고, Backspace 는 디렉터리만
  올라간다. 그 밑의 묶음 줄에서 누른 Enter 는 그 프로젝트로 들어가 그 자리에
  선다 — 한 목록에서 파고들지 않고, 파고드는 곳은 언제나 프로젝트 안이다.
  보기·정렬·열은 펼친 프로젝트 전부에 걸리고 검색·거름망은 프로젝트 안에서만
  건다 — 담기(SPC n)와 읽음(r)은 커서가 선 줄의 프로젝트로 간다. 등록한 것이
  없는데 밖에서 띄우면 빈 목록이 서서 SPC p a 로 첫 프로젝트를 더하라고
  댄다(`--json` 은 등록 없음으로 멈춘다).

  SPC p a 는 디렉터리를 골라 프로젝트로 등록하는 창을 연다 — 띄운 자리에서
  한 층씩 드나들고(Enter·Backspace, 이동은 목록과 같은 j·k·gg·G), `.moai` 가
  있는 것과 이미 등록한 것에 표시가 붙는다. 창 안에서 a 가 커서의 디렉터리를
  등록하고, `.` 이 숨은 디렉터리를 보이거나 감추고, g p 가 경로를 직접
  적는 칸을 연다(Enter 로 가고 Esc 로 그만둔다). 창은 Esc 로 닫는다. 모노레포
  하위도 고른 그대로 따로 선다. `.moai` 가 없어도 받는다. 프로젝트 안에서도
  열리므로 등록이 없어도 첫 프로젝트를 더할 수 있다.
  머리줄에서 SPC p d 는 한 번 물은 뒤 목록에서만 뺀다 — y 가 예고 다른 키는
  그만둔다. 디렉터리와 `.moai` 는 그대로다.
  쓰는 곳은 `moai project add|rm` 과 같다.

  SPC n 은 프로젝트 안 어디서든 생각 담기를 연다 — idea 로 담긴다(에픽 없이).
  편집기($VISUAL, $EDITOR, 없으면 PATH 의 vi 나 nano)가 있으면 git 커밋
  메시지처럼 그것이 뜬다: 첫 줄이 제목, 한 줄 띄우고 본문, 주석 줄은 안내라
  지운다. 제목을 비우거나 편집기를 오류로 끝내면 담지 않는다. 편집기가 없으면
  안의 폼이 열린다 — 제목 한 줄과 본문, Tab 이 둘 사이를 옮기고(제목 칸의
  Enter 는 본문으로 간다) Ctrl-S 가 담는다. Esc 는 닫되 적던 것이 있으면 한 번
  묻는다(y 로 버린다). 이슈에 쓰는 것은 이것 하나다 — 고치는 것은 CLI 로 한다.
  `--json` 은 화면 없이 그 디렉터리의 목록만 낸다(`.moai` 밖이면 층의 줄).
```

## `moai hook`

```
Claude 의 훅이 부른다. stdin 으로 이벤트를 받아 낼 것만 낸다

Usage: moai hook [OPTIONS] <이벤트>

Arguments:
  <이벤트>  어느 자리에서 불렸나 (아래 목록)

Options:
      --json                기계가 읽는 출력. 사람 출력은 전부 사라진다
      --no-color            색을 끈다 (`--color never` 와 같다)
      --color <어떻게>      auto|always|never (기본 auto, 파이프면 끈다)
  -C, --dir <경로>          이 디렉터리에서 실행한다 (`git -C` 와 같다)
      --user <이름 (메일)>  누가 하는가 (없으면 `git config`)
  -h, --help                Print help

  사람이 손으로 부를 일은 없다. Claude 에 심은 플러그인이 이것을 부른다.

  **아무것도 막지 않고, 무엇이 어긋나도 종료 코드는 0 이다.** 훅이 에러를
  뱉으면 매 세션 시작이 시끄럽고, 그러면 사람이 훅을 꺼 버린다 — 꺼진 규칙은
  없는 규칙이다.

  자리(cwd)와 세션 id 는 stdin 이 준 것을 쓴다. 환경변수에는 없다.

  이벤트:
    session-start       기준선을 적는다. 접힌 뒤면 집은 것을 싣는다
    user-prompt-submit  사람이 시켰다. 보드를 세션당 한 번 싣는다
    pre-tool-use        도구를 부르기 직전. 규칙이 여기서 선다
    stop                턴이 끝난다. 상태가 실제와 맞는지 본다

  echo '{"session_id":"x","cwd":"/repo"}' | moai hook user-prompt-submit
```

## `moai merge-driver`

```
git 이 부른다. issues.jsonl 을 이슈마다 3-way 로 합친다

Usage: moai merge-driver [OPTIONS] [기준] [이쪽] [저쪽] [표식] [경로]

Arguments:
  [기준]  `%O` — 갈라진 자리의 파일
  [이쪽]  `%A` — 이쪽 파일. **답도 여기 쓴다**
  [저쪽]  `%B` — 저쪽 파일
  [표식]  `%L` — 충돌 표식의 길이 (기본 7)
  [경로]  `%P` — 합치는 파일의 이름. 말할 때만 쓴다

Options:
      --install             이 저장소의 `.git/config` 에 드라이버를 심는다
      --as <명령>           심을 때 적을 명령 (기본: 이 바이너리의 절대 경로)
      --json                기계가 읽는 출력. 사람 출력은 전부 사라진다
      --no-color            색을 끈다 (`--color never` 와 같다)
      --color <어떻게>      auto|always|never (기본 auto, 파이프면 끈다)
  -C, --dir <경로>          이 디렉터리에서 실행한다 (`git -C` 와 같다)
      --user <이름 (메일)>  누가 하는가 (없으면 `git config`)
  -h, --help                Print help

  사람이 손으로 부를 일은 `--install` 하나다. 나머지 자리는 git 이 준다.

  moai merge-driver --install        이 저장소의 .git/config 에 심는다
  moai merge-driver --install --as <경로>  그 명령으로 심는다

  **적은 자리가 사라지면 기본 머지로 내려앉는다.** git 은 못 돈 드라이버를
  "충돌" 로 읽으면서 이쪽 파일을 그대로 두는데, 거기에 표식이 없으면 그것을
  `git add` 하는 사람이 저쪽을 통째로 버린다. 그래서 심는 줄은 드라이버가 답도
  표식도 안 쓰고 실패한 판을 `git merge-file` 로 다시 합친다 — 표식은 서고,
  최악이 안 심은 클론과 같아진다. 그래도 자리는 지키는 편이 낫다: 내려앉은
  판은 이슈마다 푸는 값을 잃는다. 기본값은 지금 도는 바이너리의 절대 경로인데,
  워크트리의 `target/` 을 가리키면 그 워크트리를 지울 때 같이 죽는다 —
  `--local` 은 클론이 함께 쓰는 자리라 그 순간 모든 체크아웃이 그 상태가 된다.
  `--as` 는 PATH 의 낱말도 받지만 이 저장소에서 맨 `moai` 를 주지 않는다:
  그것은 옛 moai 의 바이너리라 이 명령을 모른다.

  한 줄이 이슈 하나고 id 로 정렬돼 있어서, 서로 다른 이슈를 고친 두 가지가 그
  줄들이 이웃이라는 이유로 부딪친다. 여기서는 id 로 짝지어 이슈마다 3-way 로
  푼다. 같은 이슈의 다른 필드를 고친 것도 합친다 — 태그와 막음은 더한 것을
  더하고 뺀 것을 뺀다.

  **같은 필드를 다르게 고쳤으면 사람이 푼다.** 한쪽을 말없이 고르면 다른 쪽의
  고침이 아무 자취 없이 사라진다. 못 읽는 줄이나 겹친 id 가 있으면 파일을
  통째로 충돌 표식에 넣어 넘긴다 — 읽은 것만 골라 쓰면 못 읽은 줄이 사라진다.

  심는 것은 클론마다 한 번이다. git 은 드라이버 명령을 설정에서만 읽고 설정은
  커밋되지 않는다. 안 심은 클론에서는 `.gitattributes` 의 merge=moai 가 그냥
  무시되고 git 의 기본 머지가 돈다 — 즉 안 심으면 지금까지와 똑같다.
```

## `moai skill`

```
Claude 에 스킬과 훅을 심는다 (다시 불러도 된다)

Usage: moai skill [OPTIONS] <COMMAND>

Commands:
  install    플러그인 트리를 심고 `claude` 에 등록한다
  status     무엇이 어느 범위에 심겼나, 저장소와 설치본이 어긋났나
  uninstall  `claude` 에서 등록을 걷어낸다. 심은 파일은 남긴다
  help       Print this message or the help of the given subcommand(s)

Options:
      --json                기계가 읽는 출력. 사람 출력은 전부 사라진다
      --no-color            색을 끈다 (`--color never` 와 같다)
      --color <어떻게>      auto|always|never (기본 auto, 파이프면 끈다)
  -C, --dir <경로>          이 디렉터리에서 실행한다 (`git -C` 와 같다)
      --user <이름 (메일)>  누가 하는가 (없으면 `git config`)
  -h, --help                Print help
```

## `moai skill install`

```
플러그인 트리를 심고 `claude` 에 등록한다

Usage: moai skill install [OPTIONS]

Options:
      --scope <범위>        어디에 등록할까: local(기본)·project·user
      --dry-run             심지 않고 무엇이 심길지만 낸다
      --json                기계가 읽는 출력. 사람 출력은 전부 사라진다
      --no-color            색을 끈다 (`--color never` 와 같다)
      --color <어떻게>      auto|always|never (기본 auto, 파이프면 끈다)
  -C, --dir <경로>          이 디렉터리에서 실행한다 (`git -C` 와 같다)
      --user <이름 (메일)>  누가 하는가 (없으면 `git config`)
  -h, --help                Print help

  `.claude/moai-plugin/` 에 스킬과 훅을 심고 `claude` 에 등록한다. 사람의
  settings.json 은 건드리지 않는다 — 두 키를 넣는 것은 `claude` 다.

  **지우지 않는다.** 다시 불러도 덮어쓰기만 한다. 돌고 있는 세션이 물고 있는
  훅 파일을 지우면 그 세션의 도구 호출이 전부 막힌다.

  판은 심는 내용의 해시다. 내용이 같으면 판도 같아 헛 업데이트가 없다.

  한국어 글을 다듬는 플러그인 둘도 **같은 범위로** 함께 깐다
  (korean-skills·humanize-korean). 못 깔아도 moai 의 등록은 그대로 서고,
  걷을 때 함께 걷는다.

  moai skill install                  나만 (기본. settings.local.json)
  moai skill install --scope user     이 기계의 모든 저장소에
  moai skill install --scope project  팀과 함께 (커밋되는 settings.json)
  moai skill install --dry-run        무엇이 심길지만 본다

  이미 열려 있는 Claude 세션은 옛 판을 계속 쓴다 — 다시 열어야 든다.
```

## `moai skill status`

```
무엇이 어느 범위에 심겼나, 저장소와 설치본이 어긋났나

Usage: moai skill status [OPTIONS]

Options:
      --json                기계가 읽는 출력. 사람 출력은 전부 사라진다
      --no-color            색을 끈다 (`--color never` 와 같다)
      --color <어떻게>      auto|always|never (기본 auto, 파이프면 끈다)
  -C, --dir <경로>          이 디렉터리에서 실행한다 (`git -C` 와 같다)
      --user <이름 (메일)>  누가 하는가 (없으면 `git config`)
  -h, --help                Print help

  `claude` 의 장부(~/.claude/plugins/)를 **읽기만** 한다. 무엇이 어긋나도
  종료 코드는 0 이다 — 보이는 명령이지 막는 명령이 아니다.

  보는 것:
    마켓플레이스  이 저장소 이름으로 등록됐나, 남의 자리를 가리키지 않나
    설치          어느 범위에 어느 판이, 지금 심을 판과 같은가
    곁 플러그인   한국어 글을 다듬는 둘이 이 저장소에 깔렸나
    훅            설치본이 부르는 실행 파일이 아직 있나
    claude        PATH 에 있나 (없으면 심을 수도 걷을 수도 없다)
```

## `moai skill uninstall`

```
`claude` 에서 등록을 걷어낸다. 심은 파일은 남긴다

Usage: moai skill uninstall [OPTIONS]

Options:
      --dry-run             부르지 않고 무엇을 부를지만 낸다
      --json                기계가 읽는 출력. 사람 출력은 전부 사라진다
      --no-color            색을 끈다 (`--color never` 와 같다)
      --color <어떻게>      auto|always|never (기본 auto, 파이프면 끈다)
  -C, --dir <경로>          이 디렉터리에서 실행한다 (`git -C` 와 같다)
      --user <이름 (메일)>  누가 하는가 (없으면 `git config`)
  -h, --help                Print help

  이 저장소의 설치를 범위마다 `claude plugin uninstall` 하고, 마켓플레이스를
  `claude plugin marketplace remove` 한다. 사람의 settings 에서 두 키를 지우는
  것은 `claude` 가 한다 — 우리는 남의 JSON 을 만지지 않는다.

  **`.claude/moai-plugin/` 은 지우지 않는다.** 돌고 있는 세션이 물고 있는
  파일을 지우면 그 세션의 도구 호출이 막힐 수 있다. 세션을 닫은 뒤 지운다.

  함께 깐 한국어 플러그인 둘도 **moai 를 걷은 범위에서** 함께 걷는다.
  마켓플레이스는 두고 간다 — 이름은 기계 하나에서 전역이라 다른 저장소의
  설치가 그것을 쓴다.
  사용자 범위의 설치도 다른 저장소의 moai 가 거기 서 있으면 두고 가고, 그때는
  걷는 명령을 한 줄로 낸다.

  이미 열려 있는 Claude 세션은 옛 훅을 계속 부른다 — 다시 열어야 걷힌다.

  moai skill uninstall --dry-run      무엇을 부를지만 본다
```

## `moai project`

```
여러 프로젝트를 한 moai 에서 보려고 디렉터리를 등록한다

Usage: moai project [OPTIONS] <COMMAND>

Commands:
  add    디렉터리를 등록한다 (이미 있으면 그대로)
  ls     등록한 것을 낸다 — 이름·경로·`.moai` 유무
  rm     목록에서 뺀다. 디렉터리와 그 `.moai` 는 그대로 둔다
  color  한눈 보기와 탐색기에서 그 프로젝트가 입을 색을 정한다
  help   Print this message or the help of the given subcommand(s)

Options:
      --json                기계가 읽는 출력. 사람 출력은 전부 사라진다
      --no-color            색을 끈다 (`--color never` 와 같다)
      --color <어떻게>      auto|always|never (기본 auto, 파이프면 끈다)
  -C, --dir <경로>          이 디렉터리에서 실행한다 (`git -C` 와 같다)
      --user <이름 (메일)>  누가 하는가 (없으면 `git config`)
  -h, --help                Print help

예시:
  moai project add ~/work/argos         등록한다. .moai 가 아직 없어도 받는다
  moai project add repo/apps/a          모노레포는 하위 디렉터리를 따로 등록한다
  moai project ls                       등록한 것과 그 상태
  moai project rm ~/work/argos          목록에서만 뺀다. 디렉터리는 안 건드린다
  moai project color ~/work/argos green 색을 정한다 (auto 면 경로로 고른다)

  등록하면 `.moai` 밖에서 부른 `moai`·`moai status`·`moai ready` 가 등록한
  프로젝트를 프로젝트마다 한눈에 낸다 (`--json` 은 `projects` 배열).
  `--worktree` 를 붙이면 프로젝트마다 옆 워크트리도 겹친다. 그 밖의 명령은
  어느 프로젝트인지 모르니 `moai -C <dir> <명령>` 으로 부른다.

  저장소가 아니라 **사람의** 설정이다 — `.moai` 밖 어디서 불러도 된다. 자리는
  MOAI_CONFIG → $XDG_CONFIG_HOME/moai/config.toml →
  ~/.config/moai/config.toml. 상대경로는 지금 자리(`-C` 를 줬으면 그 디렉터리)에
  붙이고 심볼릭 링크를 풀어 적는다.

  누가 했는지 묻지 않는다. 이력이 남는 파일이 아니다.
```

## `moai project add`

```
디렉터리를 등록한다 (이미 있으면 그대로)

Usage: moai project add [OPTIONS] <디렉터리>

Arguments:
  <디렉터리>  등록할 디렉터리. 있어야 하지만 `.moai` 는 없어도 된다

Options:
      --json                기계가 읽는 출력. 사람 출력은 전부 사라진다
      --no-color            색을 끈다 (`--color never` 와 같다)
      --color <어떻게>      auto|always|never (기본 auto, 파이프면 끈다)
  -C, --dir <경로>          이 디렉터리에서 실행한다 (`git -C` 와 같다)
      --user <이름 (메일)>  누가 하는가 (없으면 `git config`)
  -h, --help                Print help

예시:
  moai project add .                    지금 디렉터리
  moai project add ~/work/argos         .moai 가 없어도 등록한다 ("init 전")

  다시 불러도 된다 — 이미 있으면 "이미 등록돼 있다" 로 0 종료한다.
```

## `moai project ls`

```
등록한 것을 낸다 — 이름·경로·`.moai` 유무

Usage: moai project ls [OPTIONS]

Options:
      --json                기계가 읽는 출력. 사람 출력은 전부 사라진다
      --no-color            색을 끈다 (`--color never` 와 같다)
      --color <어떻게>      auto|always|never (기본 auto, 파이프면 끈다)
  -C, --dir <경로>          이 디렉터리에서 실행한다 (`git -C` 와 같다)
      --user <이름 (메일)>  누가 하는가 (없으면 `git config`)
  -h, --help                Print help

  이름은 디렉터리 이름이고, 겹치면 위 조각을 붙여 가른다 (`apps/a`·`libs/a`).
  언제나 0 으로 끝난다 — 설정 파일이 깨졌으면 stderr 에 한 줄로 비추고 계속한다
  (`--json` 이면 stderr 대신 `problems` 배열에 선다).
```

## `moai project rm`

```
목록에서 뺀다. 디렉터리와 그 `.moai` 는 그대로 둔다

Usage: moai project rm [OPTIONS] <디렉터리>

Arguments:
  <디렉터리>  뺄 디렉터리. 이미 사라졌어도 적힌 경로로 찾는다

Options:
      --json                기계가 읽는 출력. 사람 출력은 전부 사라진다
      --no-color            색을 끈다 (`--color never` 와 같다)
      --color <어떻게>      auto|always|never (기본 auto, 파이프면 끈다)
  -C, --dir <경로>          이 디렉터리에서 실행한다 (`git -C` 와 같다)
      --user <이름 (메일)>  누가 하는가 (없으면 `git config`)
  -h, --help                Print help
```

## `moai project color`

```
한눈 보기와 탐색기에서 그 프로젝트가 입을 색을 정한다

Usage: moai project color [OPTIONS] <디렉터리> <색>

Arguments:
  <디렉터리>  등록한 디렉터리. 이미 사라졌어도 적힌 경로로 찾는다
  <색>        cyan · green · blue · auto

Options:
      --json                기계가 읽는 출력. 사람 출력은 전부 사라진다
      --no-color            색을 끈다 (`--color never` 와 같다)
      --color <어떻게>      auto|always|never (기본 auto, 파이프면 끈다)
  -C, --dir <경로>          이 디렉터리에서 실행한다 (`git -C` 와 같다)
      --user <이름 (메일)>  누가 하는가 (없으면 `git config`)
  -h, --help                Print help

예시:
  moai project color ~/work/argos green   경로로 고른 색 대신 초록으로
  moai project color ~/work/argos auto    정한 것을 지우고 경로로 고른다

  고를 수 있는 색은 cyan·green·blue 셋뿐이다. 빨강·노랑·자홍은 이미
  오류·집은 일·review 를 뜻해 id 곁에서 거짓 뜻이 되고, 밝은 색과 회색은 어느
  한쪽 바탕에서 사라진다. 색은 곁들이다 — 이름이 늘 곁에 선다. 두 프로젝트가
  같은 색으로 겹칠 때 쓴다.

  사용자 설정의 `[[project]]` 에 `color = "green"` 로 적힌다. 손으로 적어도
  된다 — 틀린 값은 `moai project ls` 가 한 줄로 비추고 경로로 고른 색을 쓴다.
```

## `moai init`

```
이 저장소에 .moai/ 를 심는다 (다시 불러도 된다)

Usage: moai init [OPTIONS] [PREFIX]

Arguments:
  [PREFIX]  id 접두어(8자까지). 없으면 디렉터리 이름에서 만든다

Options:
      --no-agents           AGENTS.md 를 건드리지 않는다
      --check               아무것도 안 쓰고 AGENTS.md 블록이 낡았는지만 본다
      --print               아무것도 안 쓰고 그 블록을 찍는다 (붙여 넣을 때)
      --json                기계가 읽는 출력. 사람 출력은 전부 사라진다
      --no-color            색을 끈다 (`--color never` 와 같다)
      --color <어떻게>      auto|always|never (기본 auto, 파이프면 끈다)
  -C, --dir <경로>          이 디렉터리에서 실행한다 (`git -C` 와 같다)
      --user <이름 (메일)>  누가 하는가 (없으면 `git config`)
  -h, --help                Print help

  이미 심긴 곳에서 다시 부르면 딸린 파일(.gitattributes·.gitignore·AGENTS.md)
  만 다시 맞춘다. 이슈와 저널은 건드리지 않는다.

  접두어는 처음 한 번만 정한다 — 이미 발급된 id 가 전부 그것을 달고 있다.

  새 접두어는 8자까지다 — id 를 칠 때마다 붙는다. 긴 것을 주면 거절하고
  짧은 후보를 댄다. 안 주면 디렉터리 이름에서 만들고, 8자를 넘으면 하이픈을
  빼서 들어가면 그것(moa-issue → moaissue), 아니면 하이픈 낱말
  머리글자(my-company-backend → mcb), 낱말이 하나면 앞 8자로 줄인다. 이미 긴
  접두어로 심긴 저장소는 그대로 읽고 쓴다.

  --check 는 아무것도 안 쓰고 AGENTS.md 블록이 current·stale·missing 인지만
  답한다. 파일을 못 읽을 때만 0 이 아니다.

  --print 는 그 블록을 찍기만 한다. 에이전트가 읽는 파일이 AGENTS.md 가 아닐
  때 거기에 붙여 넣는 자리다 — 글은 --print 와 init 이 같은 것을 쓴다.
```
