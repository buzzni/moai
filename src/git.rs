//! git 을 부르는 층.
//!
//! **이슈의 뜻은 여기서 판단하지 않는다.** `report`·`query` 는 `&[Issue]` 에 대한 순수
//! 함수로 남고, git 에서 읽어 오는 것은 이 층을 거쳐 `cmd` 가 받는다.
//!
//! 커밋은 **저장하지 않는다**(moai-1w2l). 이슈에 닿은 커밋은 커밋 제목에 적힌 id 에서
//! 그때그때 읽는다 — 이슈를 닫는 트래커 커밋은 제 해시를 미리 알 수 없고, 적어 둔
//! 해시는 squash·rebase 한 번에 낡는다. id 로 다시 찾으면 둘 다 없다.

use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug)]
pub enum Error {
    /// git 을 띄우지 못했다 — 설치돼 있지 않은 기계가 가장 흔하다.
    Spawn(std::io::Error),
    /// git 이 비영으로 끝났다. 저장소 밖이거나 커밋이 하나도 없는 가지다.
    Failed(String),
    NotUtf8(std::string::FromUtf8Error),
}

/// `root` 에서 git 을 한 번 부르고 표준 출력을 받는다.
///
/// 물려받은 환경은 **그대로 넘긴다** — 사용자가 git 훅 안에서 moai 를 부르면 그 저장소를
/// 읽는 것이 맞다. 시험 빌드만 [`LEAKS`] 를 걷는다(moai-g1a3).
pub fn run(root: &Path, args: &[&str]) -> Result<String, Error> {
    let mut cmd = std::process::Command::new("git");
    cmd.arg("-C").arg(root).args(args);
    #[cfg(test)]
    for var in LEAKS {
        cmd.env_remove(var);
    }
    let out = cmd.output().map_err(Error::Spawn)?;
    if !out.status.success() {
        return Err(Error::Failed(String::from_utf8_lossy(&out.stderr).trim().to_string()));
    }
    String::from_utf8(out.stdout).map_err(Error::NotUtf8)
}

/// 물려받으면 git 이 바깥 저장소나 바깥 설정을 보게 되는 변수들. git 훅이나
/// `git rebase -x 'cargo test'` 안에서 git 이 이것들을 내보내고, 그러면 `git -C <임시 저장소>`
/// 도 `GIT_DIR` 이 가리키는 바깥 저장소를 읽는다. tests/cli.rs 의 `GIT_LEAKS` 와 같은 목록이다 —
/// 그쪽은 따로 된 크레이트라 이것을 못 가져다 쓴다.
#[cfg(test)]
pub const LEAKS: &[&str] = &[
    "GIT_CONFIG_COUNT",
    "GIT_CONFIG_PARAMETERS",
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_COMMON_DIR",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_NAMESPACE",
    "GIT_PREFIX",
];

/// 이슈 제목에 id 가 적힌 커밋 하나. `moai show --json` 의 `commits` 가 이 모양 그대로다.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Commit {
    pub hash: String,
    pub subject: String,
    /// `chore(tracker)` — 집기·닫기만 적은 커밋. 거르지 않고 표시만 한다: 무엇을 그릴지는
    /// 그리는 쪽이 정하고, 트래커 커밋만 있는 이슈(아직 코드가 안 들어간 것)도 그것대로 답이다.
    pub tracker: bool,
}

/// 트래커 커밋의 머리. `moai init` 이 쓰는 안내(`guide`)가 이 머리를 가르친다.
const TRACKER: &str = "chore(tracker)";

/// 레코드와 필드를 가르는 글자. 커밋 제목에는 제어 문자가 들지 않는다.
const RS: char = '\u{1e}';
const FS: char = '\u{1f}';

/// 지금 가지(`HEAD`)에서 제목에 `ids` 중 하나가 적힌 커밋을 id 별로 모은다. 새것이 먼저다.
///
/// **git 은 한 번만 부른다.** id 마다 부르면 탐색기가 줄을 옮길 때마다 프로세스가
/// id 수만큼 뜬다. `--grep` 을 여럿 주면 git 은 그중 하나라도 맞는 커밋을 낸다.
/// git 의 `--grep` 은 본문까지 훑으므로 거른 뒤 [`split`] 이 제목과 id 경계를 다시 본다.
///
/// **걷는 범위는 `born`(epoch 초) 에서 막는다**(moai-mauw). 비용은 `--grep` 이 아니라 커밋을
/// 걷는 일 자체다 — 이 저장소 커밋 1천 개에서 `--grep` 을 빼도, 맞은 수를 `-n` 으로 막아도
/// 45ms 그대로였고 `--since` 만 4ms 로 줄였다. 그 이슈의 id 는 이슈가 생기기 전에는 없었으니
/// 그보다 오래된 커밋이 그 id 를 적었을 리 없어, 커밋 시각이 조상에서 자손으로 (하루 안쪽으로)
/// 늘어나는 한 이 상한은 **아무것도 가리지 않는다** — 그 전제가 깨지는 모서리는 [`SKEW`] 에 적었다.
/// 개수(`-n`)나 기간으로 막으면 오래된 이슈의 커밋이 사라지는 대가가 있다.
pub fn commits_of(root: &Path, ids: &[&str], born: Option<i64>) -> Result<BTreeMap<String, Vec<Commit>>, Error> {
    if ids.is_empty() {
        return Ok(BTreeMap::new());
    }
    let format = format!("--format=%H{FS}%s{RS}");
    // `log.showSignature` 를 켠 사람이면 git 이 서명 검사 줄을 레코드 앞 표준 출력에 끼워
    // 해시 자리에 `No signature\n<hash>` 가 들어온다 — 설정과 무관하게 끈다.
    let mut args = vec!["log", "--no-show-signature", "--fixed-strings", format.as_str()];
    let since = born.map(|b| format!("--since={}", crate::model::format_rfc3339(b - SKEW)));
    args.extend(since.as_deref());
    let greps: Vec<String> = ids.iter().map(|id| format!("--grep={id}")).collect();
    args.extend(greps.iter().map(String::as_str));
    args.push("HEAD");
    Ok(split(&run(root, &args)?, ids))
}

/// 이슈의 `created_at` 과 커밋 시각이 다른 시계에서 올 때의 여유 — 하루.
///
/// 커밋 시각은 커밋한 기계의 시계고 `created_at` 은 이슈를 만든 기계의 시계다. 시계가 늦은
/// 기계에서 한 커밋은 이슈보다 먼저 한 것처럼 찍힌다.
///
/// **이 여유가 못 받는 모서리가 하나 있다.** 걷기가 경로를 제한하지 않는 이 `git log` 에서
/// `--since` 는 거르기가 아니라 **끊기**다 — 커밋 시각이 상한보다 이른 커밋을 하나 만나면 그
/// 커밋의 조상으로는 더 걷지 않는다(참아 주는 몇 개는 경로를 제한한 걷기에만 있다). 그래서
/// 이슈의 커밋 **위에** 커밋 시각이 하루 넘게 이른 커밋이 얹히면 — 시계가 하루 넘게 늦은 기계의
/// 커밋, `rebase`·`am --committer-date-is-author-date` 로 옛 작성 시각을 커밋 시각에 옮긴 커밋 —
/// 그 밑의 커밋 칸이 말없이 빈다. `--since-as-filter` 는 끊지 않고 거르지만 그러려고 이력을
/// 끝까지 걸어 이 상한이 줄인 값을 통째로 버리므로, 드문 모서리보다 매 `show` 의 값을 골랐다.
const SKEW: i64 = 86_400;

/// `git log` 이 낸 레코드를 id 별로 가른다. git 을 부르지 않는 순수한 반쪽이다.
pub fn split(log: &str, ids: &[&str]) -> BTreeMap<String, Vec<Commit>> {
    let mut out: BTreeMap<String, Vec<Commit>> = BTreeMap::new();
    for record in log.split(RS) {
        let Some((hash, subject)) = record.trim_start_matches('\n').split_once(FS) else { continue };
        for id in ids {
            if names(subject, id) {
                out.entry((*id).to_string()).or_default().push(Commit {
                    hash: hash.to_string(),
                    subject: subject.to_string(),
                    tracker: subject.starts_with(TRACKER),
                });
            }
        }
    }
    out
}

/// `text` 가 `id` 를 **그 id 로** 적었는가.
///
/// 앞뒤가 id 의 일부로 이어지면 다른 id 다. `moai-rvcb` 는 자식 `moai-rvcb.7u5` 도,
/// 가지 이름 `worktree-moai-rvcb` 도 아니다 — 앞은 그래서 `-`·`.` 까지 막는다. 뒤의
/// `.` 은 자식으로 이어질 때만 막는다: 문장 끝 `(moai-rvcb).` 는 그 id 다.
fn names(text: &str, id: &str) -> bool {
    let joins = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-';
    text.match_indices(id).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let mut after = text[at + id.len()..].chars();
        let next = after.next();
        let free_before = before.is_none_or(|c| !(joins(c) || c == '.'));
        let free_after = match next {
            None => true,
            Some('.') => after.next().is_none_or(|c| !c.is_ascii_alphanumeric()),
            Some(c) => !joins(c),
        };
        free_before && free_after
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(hash: &str, subject: &str) -> String {
        format!("{hash}{FS}{subject}{RS}\n")
    }

    #[test]
    fn an_id_is_named_only_as_itself() {
        assert!(names("merge: 막음 줄 (moai-rvcb)", "moai-rvcb"));
        assert!(names("moai-rvcb 를 닫는다", "moai-rvcb"));
        assert!(names("닫았다 moai-rvcb.", "moai-rvcb"), "문장 끝의 점은 자식이 아니다");
        assert!(!names("리뷰 moai-rvcb.7u5 를 닫는다", "moai-rvcb"), "자식의 커밋을 부모가 가져간다");
        assert!(!names("Merge branch 'worktree-moai-rvcb'", "moai-rvcb"), "가지 이름이 id 로 읽혔다");
        assert!(!names("moai-rvcbx", "moai-rvcb"));
        assert!(names("리뷰 moai-rvcb.7u5 를 닫는다", "moai-rvcb.7u5"));
    }

    #[test]
    fn split_reads_subjects_marks_tracker_commits_and_keeps_order() {
        let log = [
            rec("c3", "chore(tracker): moai-aaaa 를 main 머지와 함께 닫는다"),
            rec("c2", "merge: 무엇 (moai-aaaa)"),
            rec("c1", "feat: 다른 것 (moai-bbbb, moai-aaaa)"),
        ]
        .concat();
        let got = split(&log, &["moai-aaaa", "moai-bbbb", "moai-cccc"]);
        let a: Vec<(&str, bool)> = got["moai-aaaa"].iter().map(|c| (c.hash.as_str(), c.tracker)).collect();
        assert_eq!(a, [("c3", true), ("c2", false), ("c1", false)]);
        assert_eq!(got["moai-bbbb"].len(), 1);
        assert!(!got.contains_key("moai-cccc"), "커밋 없는 id 에 빈 칸이 섰다");
    }

    /// git 의 `--grep` 은 본문도 훑는다 — 본문에만 id 가 든 커밋은 그 이슈의 커밋이 아니다.
    #[test]
    fn a_real_log_matches_subjects_not_bodies() {
        let dir = std::env::temp_dir().join(format!("moai-git-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // 물려받은 `GIT_DIR` 이 남으면 여기서의 git 이 바깥 저장소를 건드린다(tests/cli.rs 의 `git` 과 같은 까닭).
        let git = |args: &[&str]| {
            let mut cmd = std::process::Command::new("git");
            cmd.args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "init.defaultBranch=main"])
                .args(args)
                .current_dir(&dir);
            for var in LEAKS {
                cmd.env_remove(var);
            }
            let out = cmd.output().unwrap();
            assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        };
        git(&["init", "-q"]);
        git(&["commit", "-q", "--allow-empty", "-m", "feat: 고친다 (moai-aaaa)"]);
        git(&["commit", "-q", "--allow-empty", "-m", "chore: 딴 일\n\nmoai-aaaa 를 곁에 봤다"]);
        git(&["commit", "-q", "--allow-empty", "-m", "fix: 리뷰 (moai-aaaa.b1c)"]);

        let got = commits_of(&dir, &["moai-aaaa", "moai-aaaa.b1c"], None).unwrap();
        let subjects = |id: &str| got[id].iter().map(|c| c.subject.clone()).collect::<Vec<_>>();
        assert_eq!(subjects("moai-aaaa"), ["feat: 고친다 (moai-aaaa)"]);
        assert_eq!(subjects("moai-aaaa.b1c"), ["fix: 리뷰 (moai-aaaa.b1c)"]);
        assert_eq!(got["moai-aaaa"][0].hash.len(), 40, "해시를 줄여 받았다");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 이슈가 생기기 전의 커밋은 걷지 않는다 — 단, 시계가 늦은 기계의 커밋은 하루까지 받는다.
    #[test]
    fn walking_stops_before_the_issue_was_born_but_forgives_a_slow_clock() {
        let dir = std::env::temp_dir().join(format!("moai-git-since-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let git = |at: &str, args: &[&str]| {
            let mut cmd = std::process::Command::new("git");
            cmd.args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "init.defaultBranch=main"])
                .args(args)
                .current_dir(&dir)
                .env("GIT_AUTHOR_DATE", at)
                .env("GIT_COMMITTER_DATE", at);
            for var in LEAKS {
                cmd.env_remove(var);
            }
            let out = cmd.output().unwrap();
            assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        };
        git("2026-01-01T00:00:00Z", &["init", "-q"]);
        // 같은 id 가 이슈보다 이틀 먼저 적혔다 — 그 id 는 아직 없었으니 다른 저장소에서 온 우연이다.
        git("2026-01-01T00:00:00Z", &["commit", "-q", "--allow-empty", "-m", "옛것 (moai-aaaa)"]);
        git("2026-01-02T12:00:00Z", &["commit", "-q", "--allow-empty", "-m", "시계 늦은 기계 (moai-aaaa)"]);
        git("2026-01-03T09:00:00Z", &["commit", "-q", "--allow-empty", "-m", "고친다 (moai-aaaa)"]);

        let born = crate::model::parse_rfc3339("2026-01-03T00:00:00Z");
        let got = commits_of(&dir, &["moai-aaaa"], born).unwrap();
        let subjects: Vec<&str> = got["moai-aaaa"].iter().map(|c| c.subject.as_str()).collect();
        assert_eq!(subjects, ["고친다 (moai-aaaa)", "시계 늦은 기계 (moai-aaaa)"]);
        assert_eq!(commits_of(&dir, &["moai-aaaa"], None).unwrap()["moai-aaaa"].len(), 3, "상한 없이는 다 걷는다");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
