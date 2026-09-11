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
    /// 이 저장소에 .moai/ 를 심는다
    Init {
        /// id 접두어. 없으면 디렉터리 이름에서 만든다
        prefix: Option<String>,
    },
    /// 이슈를 만든다
    Add(AddArgs),
    /// 하나를 펼치거나 목록을 낸다
    Show(ShowArgs),

    /// 위 동사를 `--type issue` 로 고정해 부른다
    #[command(subcommand)]
    Issue(Typed),
    /// 위 동사를 `--type epic` 으로 고정해 부른다
    #[command(subcommand)]
    Epic(Typed),
}

/// `moai <종류> <동사>` ≡ `moai <동사> --type <종류>`.
/// 규칙 하나로 네임스페이스가 생기므로 명사별 코드가 없다.
#[derive(Subcommand, Debug)]
pub enum Typed {
    /// 만든다
    Add(AddArgs),
    /// 펼치거나 목록을 낸다
    Show(ShowArgs),
}

#[derive(Args, Debug)]
pub struct AddArgs {
    /// 한 줄. 따옴표로 감싼다
    #[arg(value_name = "제목")]
    pub title: String,

    /// 이 에픽에 넣는다
    #[arg(short, long, value_name = "id")]
    pub epic: Option<String>,

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

    /// id 만 낸다 (스크립트용)
    #[arg(short, long)]
    pub quiet: bool,
}

#[derive(Args, Debug)]
pub struct ShowArgs {
    /// 이슈 id, 또는 종류(issue·epic). 없으면 전체 목록
    #[arg(value_name = "대상")]
    pub target: Option<String>,

    /// done 을 포함한다
    #[arg(long)]
    pub all: bool,
}
