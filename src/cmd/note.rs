//! 이슈에 메모를 남긴다. 내용은 저널에만 쌓인다.
//!
//! 스냅샷에 **메모를 적지는 않지만**, 다른 쓰기와 같은 길을 지나므로 파일이
//! 표준형이 아니었다면(손으로 고쳤거나 머지가 남긴 모양) 그때 표준형으로
//! 맞춰진다. 파일이 한 가지 모양만 갖게 하려고 일부러 그렇게 둔다.
//!
//! 게이트를 없앤 자리를 실제로 메우는 것이 이것이다 — 다음 에이전트가 읽을
//! 발견사항을 남기는 채널이다. 승인을 구하는 것이 아니라 알리는 것이다.

use super::{Ctx, Fail, R};
use crate::cli::NoteArgs;
use crate::model::{self, JournalEntry};
use crate::style::{self, paint};

pub fn run(ctx: &Ctx, args: NoteArgs) -> R<Vec<String>> {
    let repo = super::open_repo(ctx)?;
    // **읽는 것을 락보다 먼저 한다.** `-b -` 는 stdin 을 기다린다. 락을 쥔
    // 뒤에 읽으면 파이프가 닫힐 때까지 남의 쓰기가 전부 멈춘다 — 저장소를
    // 찾는 일(`open_repo`)은 락을 안 잡으므로 그 앞뒤는 상관없다.
    //
    // 자리 인자와 `-b` 는 clap 이 이미 서로 밀어냈다. 여기 오는 것은 둘 중
    // 하나거나 아무것도 없는 경우뿐이다.
    // **"안 줬다" 와 "준 자리가 비었다" 는 다른 말이다.** 파일을 잘못 짚어
    // 빈 stdin 이 들어온 사람에게 "무엇을 적을지 안 줬다" 고 하면, 제가 준
    // 것을 도구가 못 본 줄 알고 같은 명령을 다시 친다.
    let asked = args.body.is_some();
    let given = match (args.text, super::add::read_body(args.body)?) {
        (Some(t), _) => t,
        (None, Some(b)) => b,
        (None, None) if asked => return Err(empty(ctx.lang())),
        (None, None) => {
            return Err(Fail::coded(crate::i18n::say(ctx.lang(), "refuse.note_nothing"), super::code::BAD_INPUT));
        }
    };
    let text = given.trim().to_string();
    if text.is_empty() {
        return Err(empty(ctx.lang()));
    }
    let at = model::now();
    let by = model::actor(ctx.user.as_deref(), &repo.root)?;
    let entry = JournalEntry::note(&args.id, &text, &at, &by);

    repo.with_write(
        || ctx.lang(),
        |issues, _, _| {
            if !issues.iter().any(|i| i.id == args.id) {
                return Err(Fail::not_found(&args.id, ctx.lang()));
            }
            Ok((vec![entry.clone()], ()))
        },
    )?;

    // 저널에 적은 **그 줄을 그대로** 낸다. `serde_json::json!` 은 `Value` 를
    // 거치고 `Value` 의 맵은 알파벳 순이라, 파일과 `--json` 이 서로 다른
    // 순서를 말하게 된다 (`json_line` 의 설명 참조). `kind` 도 그때 사라진다.
    if ctx.json {
        return super::json_line(&entry);
    }
    // 여러 줄 메모도 한 줄에 하나씩 낸다 — 원소 하나가 한 줄이라는 약속.
    Ok(text
        .split('\n')
        .enumerate()
        .map(|(n, l)| match n {
            0 => format!("{}  {}", paint(style::ID, &args.id), paint(style::DIM, &format!("note: {l}"))),
            _ => format!("{}  {}", " ".repeat(args.id.chars().count()), paint(style::DIM, l)),
        })
        .collect())
}

/// 빈 메모는 이력을 더럽히기만 한다. `defer` 의 빈 까닭을 안 적기로 한 것과
/// 같은 판단이다.
fn empty(lang: crate::i18n::Lang) -> Fail {
    Fail::coded(crate::i18n::say(lang, "refuse.note_empty"), super::code::BAD_INPUT)
}
