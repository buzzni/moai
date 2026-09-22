//! 제목·본문·태그·에픽·우선순위·담당을 고친다.
//!
//! **저널에 적지 않는다.** 필드 변경까지 적기 시작하면 저널은 이벤트 로그가
//! 되고, 그러면 "스냅샷 대신 저걸 접으면 되지 않나" 가 반드시 돌아온다.
//! 이전 시도가 거기서 시작했다. 상태 전이만 저널이 갖는다.

use super::{Ctx, Fail, R};
use crate::cli::EditArgs;
use crate::model::{self, Issue};
use crate::view;

/// 락 안에서 챙겨 나오는 것. **이름을 붙여 둔다** — 같은 모양의 지도 둘을 튜플로
/// 늘어놓으면 자리를 바꿔 적어도 컴파일러가 안 잡는다.
struct Edited {
    issue: Issue,
    epic: Option<Issue>,
    children: Vec<Issue>,
    /// 계획에서 빠진 줄 → 뺀 줄. 고친 줄과 그 자식만.
    shelved: Vec<(String, String)>,
    /// 묶음 → 멤버에서 읽은 칸. 고친 줄과 그 자식만.
    read: super::Read,
    changed: bool,
    /// `-e none` 을 받았는데도 남은 소속 — 에픽과 그것을 넘긴 id 부모.
    kept: Option<Inherited>,
    /// `--milestone` 을 받았는데도 적은 대로 안 선 마일스톤 — 그것을 정한 에픽이나 id 부모.
    kept_milestone: Option<InheritedMilestone>,
    /// 고친 줄의 막음을 가른 답. 상세가 `show <id>` 와 같은 막음 줄을 그린다(moai-xe74).
    blocked: Blocked,
}

/// 락 밖으로 들고 나오는 막음 답. **여기서 챙긴다** — `report::Block` 은 줄 전부를 빌리는데
/// 락을 놓으면 줄 전부가 없고, 다시 읽으면 위 `epic`·`shelved` 가 피한 두 번 파싱과 틈이
/// 돌아온다. 그래서 막는 줄의 복사본과 답을 소유한 모양으로 옮긴다.
#[derive(Default)]
struct Blocked {
    /// 막는 줄 가운데 있는 것들 — 제목과 미룬 시각을 그린다.
    issues: Vec<Issue>,
    /// 적힌 차례대로 (막는 id, 답, 그 줄을 계획에서 뺀 줄, 미뤄 뺀 멤버).
    answers: Vec<(String, crate::report::Blocker, Option<String>, Vec<String>)>,
}

impl Blocked {
    /// 막음이 없으면 소속 지도를 안 세운다(`report::blocks_of`).
    fn of(all: &[Issue], cfg: &crate::config::Config, i: &Issue) -> Blocked {
        let found = crate::report::blocks_of(all, cfg, i);
        Blocked {
            issues: found.iter().filter_map(|b| b.issue.cloned()).collect(),
            answers: found
                .iter()
                .map(|b| {
                    (
                        b.id.to_string(),
                        b.blocker,
                        b.root.map(str::to_string),
                        b.aside.iter().map(|s| s.to_string()).collect(),
                    )
                })
                .collect(),
        }
    }

    fn blocks(&self) -> Vec<crate::report::Block<'_>> {
        self.answers
            .iter()
            .map(|(id, blocker, root, aside)| crate::report::Block {
                id: id.as_str(),
                issue: self.issues.iter().find(|x| x.id == *id),
                blocker: *blocker,
                root: root.as_deref(),
                aside: aside.iter().map(String::as_str).collect(),
            })
            .collect()
    }
}

/// `-e none` 이 못 끊은 소속. `--json` 에는 `inherited_epic` 으로 선다 — 키가 없다는
/// 것이 "끊겼거나 끊을 뜻이 없었다" 는 뜻이다.
#[derive(serde::Serialize)]
struct Inherited {
    epic: String,
    parent: String,
}

/// `--milestone` 이 적은 대로 안 선 마일스톤. `--json` 에는 `inherited_milestone` 으로
/// 선다. `none` 이 못 끊은 것(moai-0lmn)과, 다른 마일스톤을 적었는데 에픽·조상이 이긴
/// 것(moai-mhxf)이다. 정한 자리는 `epic` 이나 `parent` 둘 중 하나만 선다 — 옮기는 길이
/// 달라서다. `milestone` 은 실제로 선 마일스톤이고, 마일스톤 없는 에픽이 이겼으면 `null` 이다.
#[derive(serde::Serialize)]
struct InheritedMilestone {
    milestone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    epic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent: Option<String>,
    /// 옮기는 길 — 사람 화면의 안내만 쓴다. `report::Above` 를 락 밖으로 들고 나온 모양이다.
    #[serde(skip)]
    way: Way,
}

/// [`crate::report::Above`] 의 갈래. 자리 id 는 `InheritedMilestone` 의 `epic`·`parent` 가 든다.
#[derive(Clone, Copy)]
enum Way {
    Epic,
    Lost,
    Parent,
    Pinned,
}

/// 기계 출력 — 줄 하나에 남은 소속을 곁들인다. 기존 키는 그대로 두고 더하기만 한다.
/// 필드를 더하면 그 이름을 `cmd::OURS` 에도 더한다 — 되써 넣은 줄의 같은 이름은 거기서 걷힌다.
#[derive(serde::Serialize)]
struct Out<'a> {
    #[serde(flatten)]
    row: super::Row<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    inherited_epic: Option<&'a Inherited>,
    #[serde(skip_serializing_if = "Option::is_none")]
    inherited_milestone: Option<&'a InheritedMilestone>,
}

pub fn run(ctx: &Ctx, args: EditArgs) -> R<Vec<String>> {
    fail_if_nothing(&args, ctx)?;
    if let Some(t) = &args.title {
        super::refuse_if_flag_like(t.trim(), ctx.lang())?;
    }
    let repo = super::open_repo(ctx)?;
    let body = super::add::read_body(args.body.clone())?;
    let at = model::now();
    // **말도 락 밖에서 묻는다**(리뷰) — `ctx.lang()` 의 첫 부름은 사용자 설정을 열어 파싱한다.
    // 락 안에서 부르면 그 읽기가 트래커 락을 쥔 채로 서서, 옆 세션의 집기가 그만큼 기다린다.
    // `cmd/mv.rs` 가 `model::actor` 를 밖으로 뺀 것과 같은 자다.
    let lang = ctx.lang();

    let done: Edited = repo.with_write(
        || ctx.lang(),
        |issues, cfg, _| {
            let Some(i) = issues.iter_mut().find(|i| i.id == args.id) else {
                return Err(Fail::not_found(&args.id, lang));
            };
            let before = i.clone();

            if let Some(t) = &args.title {
                i.title = t.trim().to_string();
            }
            // `--body` 를 적었으면 적은 대로 된다. **빈 것도 적은 것이다** —
            // `-b ""` 든 빈 stdin 이든 지운다는 뜻이고, 둘이 갈리면 파이프로
            // 본문을 만들어 넣는 쪽이 옛 본문을 지우지 못한다.
            if args.body.is_some() {
                i.body = body.clone();
            }
            for t in &args.tag {
                let t = model::normalize_tag(t);
                if !t.is_empty() && !i.tags.contains(&t) {
                    i.tags.push(t);
                }
            }
            if !args.untag.is_empty() {
                let drop: Vec<String> = args.untag.iter().map(|t| model::normalize_tag(t)).collect();
                i.tags.retain(|t| !drop.contains(t));
            }
            if let Some(e) = &args.epic {
                i.epic = super::clearable(e);
            }
            if let Some(m) = &args.milestone {
                i.milestone = super::clearable(m);
            }
            if let Some(p) = args.priority {
                i.priority = Some(p);
            }
            // 기한 둘(moai-tfcp). **`none` 은 비우는 것이다** — 소속·담당과 같은 낱말을 쓴다.
            // 꼴과 종류와 앞뒤 차례는 아래 `validate_keeping` 이 한자리에서 거절한다.
            if let Some(d) = &args.start {
                i.starts_on = super::clearable(d);
            }
            if let Some(d) = &args.due {
                i.due_on = super::clearable(d);
            }
            if let Some(a) = &args.assignee {
                (i.assignee, i.assignee_email) = match super::clearable(a) {
                    Some(v) => model::split_assignee(&v),
                    None => (None, None),
                };
            }

            i.normalize();
            // **안 바꾼 칸은 다시 안 묻는다 — `store::with_write` 와 한 자다**(moai-hym7).
            // 여기서만 엄하면 그쪽을 푼 것이 헛일이 된다: `config` 에서 칸 이름을 고친 뒤
            // 옛 이름에 선 줄은 제목 하나 못 고쳐 도구 안에서 영영 못 만진다 — 풀려던 바로
            // 그 자리다. 탐색기는 `with_write` 만 지나므로 두 표면이 갈리기까지 했다.
            // **칸을 견줘서 정한다** — `true` 로 박으면 여기에 칸을 고치는 길이 나는 날
            // 그 오타가 조용히 지나간다.
            // **말은 여기서 편다**(moai-yve0) — 검사는 자료로 거절하고(`model::Invalid`), 이미 선
            // 줄이라 가리키는 말은 그 id 다. `lang` 은 위에서 락 밖에 풀어 둔 값이다.
            i.validate_keeping(cfg, i.status == before.status)
                .map_err(|why| Fail::new(crate::view::invalid(lang, &crate::store::At::Id(i.id.clone()), &why)))?;
            // 바뀐 것이 없어도 실패가 아니다. `mv` 가 이미 그 칸일 때 0 으로
            // 끝나는 것과 같아야 한다 — 되풀이해 부르는 것이 흔하고, 그때
            // 한쪽만 1 로 끝나면 받는 쪽이 재시도를 못 짠다.
            let changed = *i != before;
            // **`-e none` 이 못 끊는 소속을 묻는다** (moai-w5gz). 이슈의 뜻은 `report` 가
            // 판단한다 — 여기서는 비우라고 적었는지만 본다. 바뀐 것이 없어도 묻는다: 필드가
            // 원래 비어 있던 에픽 밑 자식이 가장 흔한 자리다.
            let cut = args.epic.as_deref().is_some_and(|e| super::clearable(e).is_none());
            let kept = |issues: &[Issue]| {
                cut.then(|| crate::report::epic_from_parent(issues, &args.id))
                    .flatten()
                    .map(|(e, p)| Inherited { epic: e.to_string(), parent: p.to_string() })
            };
            // `--milestone none` 도 같다(moai-0lmn) — 에픽과 부모가 마일스톤을 이긴다. 다른
            // 마일스톤을 적어도 진다(moai-mhxf): 필드는 X 가 되는데 줄은 에픽·조상이 선 곳에
            // 그대로 서, `show --milestone X` 가 조용히 그 줄을 못 낸다. 졌는지와 옮길 길은
            // `report` 가 가른다 — 여기서는 무엇을 적었는지만 넘긴다.
            let wrote_milestone = args.milestone.as_deref().map(super::clearable);
            let kept_milestone = |issues: &[Issue]| {
                use crate::report::Above;
                let wrote = wrote_milestone.as_ref()?;
                let (m, above) = crate::report::milestone_from_above(issues, &args.id, wrote.as_deref())?;
                let (epic, parent, way) = match above {
                    Above::Epic(e) => (Some(e), None, Way::Epic),
                    Above::Lost(e) => (Some(e), None, Way::Lost),
                    Above::Parent(p) => (None, Some(p), Way::Parent),
                    Above::Pinned(p) => (None, Some(p), Way::Pinned),
                };
                Some(InheritedMilestone {
                    milestone: m.map(str::to_string),
                    epic: epic.map(str::to_string),
                    parent: parent.map(str::to_string),
                    way,
                })
            };
            if !changed {
                // **읽은 칸은 바뀐 것이 없어도 낸다.** 되풀이해 부르는 것이 흔한데, 그때만
                // 키가 사라지면 받는 쪽은 그 줄이 묶음이 아닌 줄 알고 적힌 칸을 읽는다.
                let read = super::read_of(issues, cfg, &[before.id.as_str()]);
                let kept = kept(issues);
                let kept_milestone = kept_milestone(issues);
                return Ok((
                    vec![],
                    Edited {
                        issue: before,
                        epic: None,
                        children: Vec::new(),
                        shelved: Vec::new(),
                        read,
                        changed: false,
                        kept,
                        kept_milestone,
                        // 바뀐 것이 없으면 상세를 안 그린다.
                        blocked: Blocked::default(),
                    },
                ));
            }
            i.updated_at = at.clone();
            let out = i.clone();

            // 상세를 그릴 재료를 **여기서** 챙긴다. 락을 놓은 뒤 파일을 다시 읽으면
            // 1만 줄을 두 번 파싱하고(측정: 한 번 더 읽는 데만 25%), 그 틈에 남이
            // 쓴 것이 섞여 방금 쓴 이슈와 주변이 어긋난다.
            let epic = out.epic.as_ref().and_then(|e| issues.iter().find(|x| &x.id == e).cloned());
            // 없는 에픽은 막지 않고 알려만 준다 — 끊긴 참조는 `moai status` 가 드러낸다.
            if let Some(e) = &out.epic
                && epic.is_none()
            {
                eprintln!("moai: {}", crate::i18n::fill(crate::i18n::say(lang, "edit.no_such_epic"), &[("id", e)]));
            }
            let children: Vec<Issue> =
                issues.iter().filter(|c| crate::id::parent_of(&c.id) == Some(out.id.as_str())).cloned().collect();
            // 상세가 그리는 줄 — 고친 줄과 그 자식. 미룸과 읽은 칸을 같은 자로 고른다.
            let near: Vec<&str> =
                std::iter::once(out.id.as_str()).chain(children.iter().map(|c| c.id.as_str())).collect();
            // 상세가 미룸을 말하려면 **물려받은 것까지** 필요하다 — 미룬 에픽으로 옮기는
            // 순간 그 줄이 계획에서 빠진다. 같은 까닭으로 락 안에서 본 모습으로 잰다.
            let shelved: Vec<(String, String)> = crate::report::deferred_roots(issues)
                .into_iter()
                .filter(|(id, _)| near.contains(id))
                .map(|(id, root)| (id.to_string(), root.to_string()))
                .collect();
            // 묶음의 칸도 멤버에서 읽는다 — 제 줄만 들고 나가면 상세가 손으로 둔 칸을 그린다.
            let read = super::read_of(issues, cfg, &near);
            let kept = kept(issues);
            let kept_milestone = kept_milestone(issues);
            // 막음도 **락 안에서 본 모습으로** 가른다 — `ready` 의 자(`report::blocks_of`)다.
            let blocked = Blocked::of(issues, cfg, &out);
            Ok((
                vec![],
                Edited { issue: out, epic, children, shelved, read, changed: true, kept, kept_milestone, blocked },
            ))
        },
    )?;

    let Edited { issue: edited, epic, children, shelved, read, changed, kept, kept_milestone, blocked } = done;
    if ctx.json {
        // **이 키는 우리 것이다** — `--json` 을 되써 넣어 모르는 필드로 든 줄이면 한 객체에 같은
        // 키가 둘 서거나, 끊긴 줄이 안 끊긴 것처럼 읽힌다. `Row::of` 가 `cmd::OURS` 로 걷는다.
        return super::json_line(&Out {
            row: super::Row::from(&edited, &read),
            inherited_epic: kept.as_ref(),
            inherited_milestone: kept_milestone.as_ref(),
        });
    }
    if let Some(k) = &kept {
        eprintln!(
            "moai: {}",
            crate::i18n::fill(
                crate::i18n::say(ctx.lang(), "edit.kept_epic"),
                &[("id", &edited.id), ("epic", &k.epic), ("parent", &k.parent)],
            )
        );
    }
    if let Some(k) = &kept_milestone {
        milestone_kept_line(&edited.id, k, args.milestone.as_deref().unwrap_or("none"), ctx.lang());
    }
    if !changed {
        return Ok(vec![format!(
            "{}  {}",
            crate::style::paint(crate::style::ID, &edited.id),
            crate::style::paint(crate::style::DIM, crate::i18n::say(ctx.lang(), "edit.nothing_changed"))
        )]);
    }
    let children: Vec<&Issue> = children.iter().collect();
    let seen = view::Seen {
        roots: shelved.iter().map(|(id, root)| (id.as_str(), root.as_str())).collect(),
        states: read.iter().map(|(id, col)| (id.as_str(), col.as_str())).collect(),
        // 쓰는 길은 옆 워크트리를 겹쳐 보지 않는다 — 겹칠 것이 없는 화면이다.
        screen: view::Screen::new(ctx.lang()).at(ctx.zone()),
        blocks: blocked.blocks(),
        places: None,
    };
    Ok(view::detail(&edited, epic.as_ref(), &children, &seen, &repo.config, &at, false))
}

/// `--milestone` 이 적은 대로 안 선 것을 한 줄로 말한다.
///
/// 정한 자리마다 옮기는 길이 다르다(`report::Above`) — 에픽 멤버는 에픽을 옮기거나 에픽의
/// 마일스톤을 고치고, 못 쓸 에픽의 멤버는 에픽을 옮기는 길만 있고, 부모 밑 자식은 id 를 못
/// 옮기니 조상의 마일스톤을 고친다. id 가 마일스톤 줄이나 뿌리로 올라간 생각 밑에 서 있으면
/// 필드로는 못 옮긴다 — 그 줄의 필드를 고치라고 대면 아무것도 안 바뀐다.
///
/// `none` 은 "안 끊긴다" 로, 다른 마일스톤은 "필드에만 적혔다" 로 말한다(moai-mhxf) —
/// 앞의 것은 필드가 비워졌는데 소속이 남았고, 뒤의 것은 필드가 바뀌었는데 소속이 안 따라왔다.
fn milestone_kept_line(id: &str, k: &InheritedMilestone, wrote: &str, lang: crate::i18n::Lang) {
    use crate::i18n::{fill, say};
    let cut = wrote == "none";
    // **갈래마다 제 `say` 를 적는다** — 키를 도우미로 고르면 소스를 훑는 시험(`i18n::tests::keys_in`)이
    // 그 키를 못 본다. 조각을 이어 한 줄로 세우는 것은 그대로다: 자리마다 옮기는 길이 다르다.
    let stood = match &k.milestone {
        Some(m) => fill(say(lang, "edit.stood_in"), &[("milestone", m)]),
        None => say(lang, "edit.stood_none").to_string(),
    };
    let lost = match cut {
        true => say(lang, "edit.lost_none").to_string(),
        false => fill(say(lang, "edit.lost_field"), &[("wrote", wrote)]),
    };
    let at = k.epic.as_deref().or(k.parent.as_deref()).unwrap_or_default();
    let (from, way) = match k.way {
        Way::Pinned => {
            let said =
                fill(say(lang, "edit.kept_pinned"), &[("id", id), ("stood", &stood), ("at", at), ("lost", &lost)]);
            return eprintln!("moai: {said}");
        }
        Way::Epic => (
            fill(say(lang, "edit.from_epic"), &[("at", at)]),
            fill(say(lang, "edit.go_epic"), &[("id", id), ("at", at), ("wrote", wrote)]),
        ),
        Way::Lost => (fill(say(lang, "edit.from_lost"), &[("at", at)]), fill(say(lang, "edit.go_lost"), &[("id", id)])),
        Way::Parent => (
            fill(say(lang, "edit.from_parent"), &[("at", at)]),
            fill(say(lang, "edit.go_parent"), &[("at", at), ("wrote", wrote)]),
        ),
    };
    let verb = match cut {
        true => say(lang, "edit.verb_cut"),
        false => say(lang, "edit.verb_move"),
    };
    let said = fill(
        say(lang, "edit.kept_milestone"),
        &[("id", id), ("stood", &stood), ("from", &from), ("lost", &lost), ("verb", verb), ("way", &way)],
    );
    eprintln!("moai: {said}");
}

/// **말은 거절할 때만 푼다**([`Ctx::lang`]) — `ctx.lang()` 을 인자로 넘기면 그것이 부르는 쪽에서
/// 먼저 풀려, 아무것도 안 거절하는 판(`--json` 과 "바뀐 것이 없다" 로 일찍 나가는 길까지)이
/// 사람의 설정 파일을 읽는다. 곁의 넷(`defer`·`link`·`init`)은 거절하는 가지 안에서 푼다.
fn fail_if_nothing(args: &EditArgs, ctx: &Ctx) -> R<()> {
    let touched = args.title.is_some()
        || args.body.is_some()
        || !args.tag.is_empty()
        || !args.untag.is_empty()
        || args.epic.is_some()
        || args.milestone.is_some()
        || args.priority.is_some()
        || args.assignee.is_some()
        || args.start.is_some()
        || args.due.is_some();
    touched.then_some(()).ok_or_else(|| Fail::new(crate::i18n::say(ctx.lang(), "refuse.edit_nothing")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **`Out` 이 줄 곁에 다는 키는 `cmd::OURS` 에 있다**(moai-qn5d) — 없으면 옛 `edit --json` 을
    /// 되써 넣은 줄의 같은 이름이 안 걷혀, 그 소속이 이제 안 남아도 옛 값이 선다. `Out` 에 필드를
    /// 더하면 여기서 붉어진다(목록은 주석으로만 매여 있어 잡을 곳이 여기뿐이다).
    #[test]
    fn every_key_out_adds_is_in_ours() {
        let i = Issue::new(
            "argos-0001".into(),
            "제목".into(),
            model::Kind::Issue,
            model::Status::new("todo"),
            "2026-09-11T04:12:03Z",
        );
        let read = super::super::Read::new();
        let epic = Inherited { epic: "argos-0002".into(), parent: "argos-0003".into() };
        let milestone = InheritedMilestone {
            milestone: Some("argos-0004".into()),
            epic: Some("argos-0002".into()),
            parent: None,
            way: Way::Epic,
        };
        let out = Out {
            row: super::super::Row::from(&i, &read),
            inherited_epic: Some(&epic),
            inherited_milestone: Some(&milestone),
        };
        let added = super::super::keys_beyond(&i, &out);
        assert!(!added.is_empty(), "곁들인 키를 못 셌다");
        for k in &added {
            assert!(super::super::OURS.contains(&k.as_str()), "`edit --json` 이 곁들이는 {k} 가 `OURS` 에 없다");
        }
    }
}
