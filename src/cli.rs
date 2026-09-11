//! argv 의 모양만 정의한다. **로직은 여기 없다.**

use crate::model::Kind;
use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
#[command(
    name = "moai",
    version,
    about = "이슈 트래커. 승인 게이트 없음. 규율은 `moai status` 가 비춘다.",
    max_term_width = 100
)]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Cmd,

    /// 기계가 읽는 출력. 사람 출력은 전부 사라진다
    #[arg(long, global = true)]
    pub json: bool,

    /// 색을 끈다 (`--color never` 와 같다)
    #[arg(long, global = true, conflicts_with = "color")]
    pub no_color: bool,

    /// 언제 색을 쓸까 (NO_COLOR·파이프는 자동으로 꺼진다)
    #[arg(long, global = true, value_name = "어떻게", default_value = "auto")]
    pub color: ColorArg,

    /// 이 디렉터리에서 실행한다 (`git -C` 와 같다)
    #[arg(short = 'C', long = "dir", global = true, value_name = "경로")]
    pub dir: Option<std::path::PathBuf>,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq)]
pub enum ColorArg {
    Auto,
    Always,
    Never,
}

#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// 보드 · 경고 · 흐름. 세션은 여기서 시작한다
    #[command(after_help = "\
  아무것도 막지 않는다. 승인도 통과도 없다.
  대신 에픽에 안 붙은 이슈, 오래 멈춘 review, 한 번에 벌여 놓은 것을 드러낸다.
  종료 코드는 데이터가 깨졌을 때만 0 이 아니다.")]
    Status,

    /// 지금 집을 수 있는 일
    #[command(after_help = "\
  에픽 자체, 끝난 에픽의 멤버, 아직 안 끝난 자식을 가진 부모는 뺀다.
  급한 것 → 끝나가는 에픽 → 오래된 것 차례로 낸다.")]
    Ready,

    /// 이 저장소에 .moai/ 를 심는다
    Init {
        /// id 접두어. 없으면 디렉터리 이름에서 만든다
        prefix: Option<String>,
        /// AGENTS.md 를 건드리지 않는다
        #[arg(long)]
        no_agents: bool,
    },
    /// 이슈를 만든다
    #[command(after_help = "\
예시:
  moai add \"파서가 BOM 에서 죽는다\" -t bug -p 1
  moai add \"저장 계층\" --type epic
  moai add \"부모에 딸린 일\" --parent moai-4aex
  moai add \"본문은 stdin 에서\" -b -

한 번에 여럿 (`--from`):
  moai add --from - <<'EOF'
  # 저장 계층                    `#` 줄은 에픽
  - [p1] 원자적으로 쓴다 #bug    `-` 줄은 바로 위 에픽의 이슈
  - 잘린 줄을 복구한다           [pN] 과 #태그 는 없어도 된다
  EOF

  --dry-run 이 heredoc 오타로 여섯 개를 잘못 만드는 것을 막는다.

제목이 `--` 로 시작해도 된다. 아는 플래그가 아니면 제목으로 읽는다.")]
    Add(AddArgs),
    /// 하나를 펼치거나 목록을 낸다
    Show(ShowArgs),
    /// 상태를 옮긴다
    #[command(after_help = "\
  마지막 인자가 갈 칸이고, 그 앞이 전부 옮길 이슈다.
  칸 이름과 차례는 .moai/config.toml 의 statuses 가 정한다 (기본: todo,
  in_progress, review, done).

  순서를 건너뛰어도, 되돌려도, 막지 않는다. 이 도구에 승인은 없다.
  되감긴 것과 오래 멈춘 것은 `moai status` 가 드러낸다.

  moai mv moai-4aex in_progress
  moai mv moai-4aex moai-9k2p done
  moai mv moai-4aex review -m \"테스트는 다음 이슈로 뺐다\"")]
    Mv(MvArgs),
    /// 제목·본문·태그·에픽·우선순위를 고친다
    Edit(EditArgs),
    /// 지운다
    Rm(RmArgs),
    /// 이슈에 메모를 남긴다 (저널에만 쌓인다)
    Note(NoteArgs),

    /// 위 동사를 `--type issue` 로 고정해 부른다
    #[command(subcommand)]
    Issue(Typed),
    /// 위 동사를 `--type epic` 으로 고정해 부른다
    #[command(subcommand)]
    Epic(Typed),
    /// 위 동사를 `--type milestone` 으로 고정해 부른다
    #[command(subcommand)]
    Milestone(Typed),
}

/// `moai <종류> <동사>` ≡ `moai <동사> --type <종류>`.
///
/// 규칙 하나로 네임스페이스가 생기므로 명사별 코드가 없다. `mv`·`edit`·`rm`
/// 은 여기 없다 — id 가 대상을 정확히 가리켜서 종류를 덧붙일 자리가 없다.
#[derive(Subcommand, Debug)]
pub enum Typed {
    /// 만든다
    Add(AddArgs),
    /// 펼치거나 목록을 낸다
    Show(ShowArgs),
}

#[derive(Args, Debug)]
pub struct AddArgs {
    /// 한 줄. 따옴표로 감싼다. `--` 로 시작해도 된다
    #[arg(value_name = "제목", allow_hyphen_values = true)]
    pub title: Option<String>,

    /// 이 에픽에 넣는다
    #[arg(short, long, value_name = "id")]
    pub epic: Option<String>,

    /// 이 마일스톤에 넣는다
    #[arg(long, value_name = "id")]
    pub milestone: Option<String>,

    /// 쉼표로 잇거나 여러 번 쓴다
    #[arg(short, long, value_name = "태그", value_delimiter = ',')]
    pub tag: Vec<String>,

    /// 0 이 가장 높다
    #[arg(short, long, value_name = "0-3")]
    pub priority: Option<u8>,

    /// 처음 놓일 칸. 없으면 첫 칸
    #[arg(short, long, value_name = "상태")]
    pub status: Option<String>,

    /// 본문. `-` 이면 stdin 에서 읽는다
    #[arg(short, long, value_name = "글")]
    pub body: Option<String>,

    /// 담당
    #[arg(short, long, value_name = "이름")]
    pub assignee: Option<String>,

    #[arg(long = "type", value_name = "issue|epic")]
    pub kind: Option<Kind>,

    /// 이 이슈의 자식으로 만든다 (id 가 `.xxx` 로 붙는다)
    #[arg(long, value_name = "id")]
    pub parent: Option<String>,

    /// 마크다운에서 에픽과 이슈를 한 번에. `-` 이면 stdin
    #[arg(long, value_name = "파일|-", conflicts_with_all = ["title", "epic", "tag", "priority", "parent"])]
    pub from: Option<String>,

    /// 만들지 않고 무엇이 만들어질지만 낸다
    #[arg(long)]
    pub dry_run: bool,

    /// id 만 낸다 (스크립트용)
    #[arg(short, long)]
    pub quiet: bool,
}

#[derive(Args, Debug)]
pub struct ShowArgs {
    /// 이슈 id, 또는 종류(issue·epic). 없으면 전체 목록
    #[arg(value_name = "대상")]
    pub target: Option<String>,

    /// 에픽 → 이슈 → 자식으로 접어 낸다
    #[arg(long)]
    pub tree: bool,

    #[command(flatten)]
    pub filter: FilterArgs,
}

/// 쉼표는 "또는", 반복은 "그리고".
///
/// 쉼표를 clap 에게 맡기지 않는 이유가 있다 — `-s todo,review` 와
/// `-s todo -s review` 가 구별돼야 뒤엣것에 친절한 오류를 낼 수 있다.
#[derive(Args, Debug)]
#[command(next_help_heading = "필터  (쉼표 = 또는,  반복 = 그리고)")]
pub struct FilterArgs {
    /// 그 칸에 있는 것
    #[arg(short, long, value_name = "상태")]
    pub status: Vec<String>,

    /// 그 태그를 가진 것
    #[arg(short, long, value_name = "태그")]
    pub tag: Vec<String>,

    /// 그 태그가 없는 것
    #[arg(long = "no-tag", value_name = "태그")]
    pub no_tag: Vec<String>,

    /// 그 에픽 소속 (`none` = 에픽 없는 것)
    #[arg(short, long, value_name = "id|none")]
    pub epic: Vec<String>,

    /// 그 마일스톤 소속 (`none` = 마일스톤 없는 것)
    #[arg(long, value_name = "id|none")]
    pub milestone: Vec<String>,

    /// 그 이슈의 자식 (`none` = 최상위만)
    #[arg(long, value_name = "id|none")]
    pub parent: Vec<String>,

    #[arg(short, long, value_name = "0-3")]
    pub priority: Vec<String>,

    #[arg(long = "type", value_name = "issue|epic")]
    pub kind: Option<Kind>,

    /// 제목·본문에 이 글이 든 것
    #[arg(short = 'g', long, value_name = "글")]
    pub grep: Option<String>,

    /// 지금 칸에 그만큼 머문 것
    #[arg(long, value_name = "일")]
    pub stale: Option<i64>,

    /// done 을 포함한다
    #[arg(long)]
    pub all: bool,

    /// 위 필터를 한 문자열로. `--filter status=todo,review`
    #[arg(long, value_name = "항목=값")]
    pub filter: Vec<String>,
}

#[derive(Args, Debug)]
pub struct MvArgs {
    /// 옮길 이슈들, 그리고 맨 끝에 갈 칸
    ///
    /// 개수는 clap 이 아니라 `mv` 가 본다 — `2 values required by '<id> <id>...'`
    /// 는 무엇을 빠뜨렸는지 말해 주지 않는다.
    #[arg(required = true, num_args = 1.., value_name = "id")]
    pub args: Vec<String>,

    /// 이 이동에 한 줄 메모 (저널에만 남는다)
    #[arg(short, long, value_name = "글", allow_hyphen_values = true)]
    pub msg: Option<String>,
}

#[derive(Args, Debug)]
pub struct EditArgs {
    #[arg(value_name = "id")]
    pub id: String,

    /// 한 줄. `--` 로 시작해도 된다
    #[arg(long, value_name = "글", allow_hyphen_values = true)]
    pub title: Option<String>,

    /// 본문. `-` 이면 stdin 에서 읽는다
    #[arg(short, long, value_name = "글", allow_hyphen_values = true)]
    pub body: Option<String>,

    /// 태그를 더한다
    #[arg(short, long, value_name = "태그", value_delimiter = ',')]
    pub tag: Vec<String>,

    /// 태그를 뺀다
    #[arg(long, value_name = "태그", value_delimiter = ',')]
    pub untag: Vec<String>,

    /// 에픽을 옮긴다 (`none` 이면 뺀다)
    #[arg(short, long, value_name = "id|none")]
    pub epic: Option<String>,

    /// 마일스톤을 옮긴다 (`none` 이면 뺀다)
    #[arg(long, value_name = "id|none")]
    pub milestone: Option<String>,

    #[arg(short, long, value_name = "0-3")]
    pub priority: Option<u8>,

    /// 담당 (`none` 이면 뺀다)
    #[arg(short, long, value_name = "이름|none")]
    pub assignee: Option<String>,
}

#[derive(Args, Debug)]
pub struct RmArgs {
    #[arg(required = true, value_name = "id")]
    pub ids: Vec<String>,
}

#[derive(Args, Debug)]
pub struct NoteArgs {
    #[arg(value_name = "id")]
    pub id: String,
    /// 다음 사람(또는 다음 에이전트)이 읽을 발견사항. `--` 로 시작해도 된다
    #[arg(value_name = "글", allow_hyphen_values = true)]
    pub text: String,
}
