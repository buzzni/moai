//! git 을 부르는 층.
//!
//! **이슈의 뜻은 여기서 판단하지 않는다.** `report`·`query` 는 `&[Issue]` 에 대한 순수
//! 함수로 남고, git 에서 읽어 오는 것은 이 층을 거쳐 `cmd` 가 받는다.
//!
//! 커밋은 **저장하지 않는다**(moai-1w2l). 이슈에 닿은 커밋은 커밋 제목에 적힌 id 에서
//! 그때그때 읽는다 — 이슈를 닫는 트래커 커밋은 제 해시를 미리 알 수 없고, 적어 둔
//! 해시는 squash·rebase 한 번에 낡는다. id 로 다시 찾으면 둘 다 없다.

use crate::git_leaks::LEAKS;
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

/// `root` 에서 git 을 한 번 부르고 표준 출력을 바이트로 받는다.
///
/// **저장소를 가리키는 환경 변수는 릴리스에서도 걷는다**(moai-1w2l 단계 리뷰). `-C root` 로 어느
/// 저장소인지 이미 댔는데 물려받은 `GIT_DIR` 이 남으면 git 은 그쪽을 읽는다 — moai 는 훅 안에서도
/// 딸린 워크트리의 `rebase -x` 안에서도 돈다. 걷지 않으면 뿌리마다 따로 읽는 커밋 표가 뿌리와
/// 상관없이 **같은 저장소**를 재고, 옆 워크트리의 커밋이 이쪽 것으로 서며 `show --worktree` 는
/// 엉뚱한 가지를 읽는다(리뷰 moai-v9ai.q6f 가 짚은 자리이기도 하다). 시험 빌드는 [`command`] 가
/// [`LEAKS`] 전부를 걷는다 — 여기서 걷는 셋은 릴리스에서도 걷어야 하는 것이다.
///
/// **출력은 UTF-8 로 달라고 한다.** `i18n.logOutputEncoding` 을 cp949 같은 것으로 둔 사람에게는
/// 한글 제목 한 줄이 깨져 — ASCII 제목까지 같이 — 커밋 칸이 통째로 빌 수 있다. 그래도 깨진 채로
/// 오는 이력은 [`read_log`] 가 관대하게 읽는다.
fn output(root: &Path, args: &[&str]) -> Result<Vec<u8>, Error> {
    let out = command()
        .arg("-C")
        .arg(root)
        .args(["-c", "i18n.logOutputEncoding=UTF-8"])
        .args(args)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_COMMON_DIR")
        .output()
        .map_err(Error::Spawn)?;
    if !out.status.success() {
        return Err(Error::Failed(String::from_utf8_lossy(&out.stderr).trim().to_string()));
    }
    Ok(out.stdout)
}

/// `root` 에서 git 을 한 번 부르고 표준 출력을 받는다. 글자가 깨졌으면 실패다 —
/// 경로를 읽는 자리(`worktree`)는 깨진 채 읽으면 없는 디렉터리를 가리킨다.
pub fn run(root: &Path, args: &[&str]) -> Result<String, Error> {
    String::from_utf8(output(root, args)?).map_err(Error::NotUtf8)
}

/// `git log` 의 출력. **글자가 깨져도 읽는다**(읽기는 관대하고 쓰기는 엄하다).
/// `i18n.logOutputEncoding` 을 legacy 로 둔 저장소나 옛 커밋 하나의 인코딩 때문에 이력
/// 전체를 못 읽으면 커밋 칸이 까닭도 없이 영영 빈다 — 제목은 어차피 `text::sanitize`
/// 를 지나 그려지고, 해시는 [`records`] 가 모양으로 한 번 더 거른다.
fn read_log(root: &Path, args: &[&str]) -> Result<String, Error> {
    Ok(String::from_utf8_lossy(&output(root, args)?).into_owned())
}

/// git 을 띄울 명령 — **시험의 git 은 모두 여기서 시작한다.** 시험 빌드만 [`LEAKS`] 를 걷는다.
///
/// [`run`] 과 임시 저장소를 만드는 도우미(`isolated`)가 따로 걷으면 걷는 목록이 갈라진다. 한때
/// 도우미는 셋만 걷고 `run` 은 열을 걷어, pre-receive 훅 안에서는 도우미가 바깥 객체 저장소에 쓰고
/// `run` 은 임시 저장소를 읽었다(moai-g1a3). `#[cfg(test)]` 가 아니라 `cfg!(test)` 인 것은 릴리스
/// 빌드에서도 이 코드가 타입 검사를 받게 하려는 것이다.
pub fn command() -> std::process::Command {
    let mut cmd = std::process::Command::new("git");
    if cfg!(test) {
        for var in LEAKS {
            cmd.env_remove(var);
        }
    }
    cmd
}

/// 시험이 임시 저장소를 만들 때 쓰는 git — `dir` 에서 돌고, 커밋할 이름과 기본 가지를 준다.
///
/// **돌리는 사람의 git 설정도 안 읽는다**(tests/cli.rs 의 `isolated` 와 같은 자). 전역
/// `commit.gpgsign` 이면 임시 저장소의 커밋이 서명을 못 해 멈추고, 전역 `core.hooksPath` 면 그 사람의
/// 훅이 `-m a` 같은 커밋 제목을 막는다.
///
/// 걷기는 여기서 끝난다 — 시험이 제 값을 줄 것은 이 뒤에 `.env` 로 덮는다.
#[cfg(test)]
pub fn isolated(dir: &Path) -> std::process::Command {
    let mut cmd = command();
    cmd.args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "init.defaultBranch=main"])
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1");
    cmd
}

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
pub const TRACKER: &str = "chore(tracker)";

/// 해시와 제목을 가르는 글자.
///
/// **레코드는 이 글자로 가르지 않는다 — `-z` 가 넣는 NUL 로 가른다.** 커밋 제목에는
/// 제어 문자가 들 수 있고(git 은 `%s` 로 그대로 낸다), 제목 안에 들 수 있는 글자로
/// 레코드를 가르면 그런 제목 하나가 레코드를 둘로 쪼개 **저장소에 없는 해시**를 지어낸다 —
/// 남이 보낸 커밋 한 줄로 남의 이슈 커밋 칸에 가짜 줄을 세울 수 있었다. NUL 은 커밋
/// 메시지에 들지 않는다. 필드는 **첫** FS 에서만 가르므로([`records`] 의 `split_once`)
/// 제목에 FS 가 들어도 해시 자리는 못 건드린다.
const FS: char = '\u{1f}';

/// `git log` 에 줄 서식. **[`records`] 가 가르는 자와 한 자리에서 짓는다** — 한쪽만 고치면
/// 제목 자리에 다른 필드가 들어오고, 그것을 알려 주는 것이 없다.
fn format_arg() -> String {
    format!("--format=%H{FS}%s")
}

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
    let format = format_arg();
    // `log.showSignature` 를 켠 사람이면 git 이 서명 검사 줄을 레코드 앞 표준 출력에 끼워
    // 해시 자리에 `No signature\n<hash>` 가 들어온다 — 설정과 무관하게 끈다.
    let mut args = vec!["log", "-z", "--no-show-signature", "--fixed-strings", format.as_str()];
    // **1970 앞의 상한은 안 단다.** git 의 날짜 파서는 못 읽은 글을 **오류가 아니라 `now`**
    // 로 읽어, 그런 상한을 주면 커밋이 하나도 안 걸리고 그것이 "커밋이 없다" 로 보인다.
    // 그 시각의 이슈는 어차피 이력 전체보다 오래됐으니 상한이 줄일 것도 없다.
    let since = born.filter(|b| *b >= SKEW).map(|b| format!("--since={}", crate::model::format_rfc3339(b - SKEW)));
    args.extend(since.as_deref());
    let greps: Vec<String> = ids.iter().map(|id| format!("--grep={id}")).collect();
    args.extend(greps.iter().map(String::as_str));
    // `--` 를 단다 — 저장소에 `HEAD` 라는 **파일**이 있으면 git 이 가지인지 경로인지 모른다며
    // 128 로 끝나고, 그러면 커밋 칸이 까닭도 없이 영영 빈다.
    args.extend(["HEAD", "--"]);
    Ok(split(&read_log(root, &args)?, ids))
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
    for (hash, subject) in records(log) {
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

/// `text` 가 `id` 를 **그 id 로** 적었는가 — [`ids_in`] 이 뽑는 낱말 하나가 곧 그 id 일 때.
fn names(text: &str, id: &str) -> bool {
    ids_in(text).any(|t| t == id)
}

/// 글에 적힌 id 모양의 낱말들. **id 를 가르는 자는 이것 하나다** — `show` 가 id 하나를 찾을
/// 때([`split`])도, 탐색기가 이력 전부를 한 번에 가를 때([`table`])도 이 자로 가른다.
///
/// 낱말은 id 에 드는 글자(영숫자·`_`·`-`·`.`)가 이어진 한 덩어리고, 끝의 `.` 은 문장 부호로
/// 떼어 낸다. 그래서 앞뒤가 id 의 일부로 이어지면 다른 낱말이다 — `moai-rvcb` 는 자식
/// `moai-rvcb.7u5` 도, 가지 이름 `worktree-moai-rvcb` 도 아니고, 문장 끝 `(moai-rvcb).` 는
/// 그 id 다. 한글 조사가 붙어도(`moai-rvcb를`) 그 id 다.
///
/// **`-`·`.` 로 시작하는 낱말은 id 가 아니다.** 접두어가 그 한 글자뿐인 것을
/// `id::is_valid` 는 통과시켜(`--json` 은 접두어 `-` 에 본체 `json`), 제목에 적힌 플래그가
/// id 로 읽힌다. 찾는 쪽은 있는 id 로만 찾아 답이 틀리지는 않지만, 표가 그만큼 헛되이 부푼다.
pub fn ids_in(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')))
        .map(|t| t.trim_end_matches('.'))
        .filter(|t| !t.starts_with(['-', '.']) && crate::id::is_valid(t))
}

/// 지금 가지(`HEAD`)의 이력 **전부**를 한 번 걸어 제목에 적힌 id → 커밋 표를 짓는다. 새것이 먼저다.
///
/// 탐색기가 쓴다(moai-a4i0). 커서를 옮길 때마다 [`commits_of`] 를 부르면 걸음마다 git 이
/// 뜨고 그리는 루프가 그만큼 멈칫한다 — 표는 다시 읽기 스레드에서 한 번 짓고 상세는 찾기만
/// 한다. 전부 걸으므로 `commits_of` 의 생성일 상한이 가리는 모서리(moai-g8cd)도 여기서는 없다.
/// 표에는 없는 id 의 낱말도 드는데, 찾는 쪽이 있는 id 로만 찾으므로 해가 없다.
pub fn table(root: &Path) -> Result<BTreeMap<String, Vec<Commit>>, Error> {
    let format = format_arg();
    let log = read_log(root, &["log", "-z", "--no-show-signature", format.as_str(), "HEAD", "--"])?;
    let mut out: BTreeMap<String, Vec<Commit>> = BTreeMap::new();
    for (hash, subject) in records(&log) {
        let mut seen: Vec<&str> = Vec::new();
        for id in ids_in(subject) {
            // 한 제목에 같은 id 를 두 번 적어도 커밋은 한 번이다.
            if seen.contains(&id) {
                continue;
            }
            seen.push(id);
            out.entry(id.to_string()).or_default().push(Commit {
                hash: hash.to_string(),
                subject: subject.to_string(),
                tracker: subject.starts_with(TRACKER),
            });
        }
    }
    Ok(out)
}

/// `git log -z` 가 낸 레코드를 (해시, 제목) 으로. 가르는 자의 까닭은 [`FS`] 에 있다.
///
/// **해시 자리가 해시 모양이 아니면 버린다.** `-z` 가 이미 레코드를 지키지만, 지키는 것이
/// 하나뿐이면 그것이 어긋난 날(옛 git, 다른 서식) 지어낸 해시가 화면과 `--json` 으로 그대로
/// 나간다. 여기서 한 번 더 보면 그 길이 막힌다 — `%H` 는 언제나 16진수다.
fn records(log: &str) -> impl Iterator<Item = (&str, &str)> {
    log.split('\0')
        .filter_map(|record| record.split_once(FS))
        .filter(|(hash, _)| !hash.is_empty() && hash.bytes().all(|b| b.is_ascii_hexdigit()))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// 시험이 쓰는 git. **바깥 저장소와 바깥 설정을 함께 끊는다** — `tests/cli.rs` 의
    /// `isolated` 와 같은 자다. 물려받은 `GIT_DIR` 이 남으면 여기서의 git 이 바깥 저장소를
    /// 건드리고, 전역 설정에 `commit.gpgsign`·`core.hooksPath` 를 둔 기계에서는 `git commit`
    /// 이 그냥 실패해 이 시험이 보려던 것과 아무 상관 없이 빨개진다. `at` 을 주면 작성·커밋
    /// 시각을 고정한다 — 시각이 답을 가르는 시험은 기계 시계에 매이면 안 된다.
    pub(crate) fn run_git(dir: &Path, at: Option<&str>, args: &[&str]) {
        // 걷기와 격리는 `isolated` 하나가 안다(moai-g1a3) — 여기서 목록을 다시 적으면 둘이 갈라진다.
        let mut cmd = isolated(dir);
        cmd.args(args);
        if let Some(at) = at {
            cmd.env("GIT_AUTHOR_DATE", at).env("GIT_COMMITTER_DATE", at);
        }
        let out = cmd.output().unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    }

    fn rec(hash: &str, subject: &str) -> String {
        format!("{hash}{FS}{subject}\0")
    }

    /// **제목이 레코드를 못 쪼갠다**(`-z`). 제어 문자를 담은 제목 하나로 저장소에 없는
    /// 해시를 지어내 남의 이슈 커밋 칸에 세울 수 있었다.
    #[test]
    fn a_subject_cannot_forge_a_record() {
        let forged = "\u{1e}deadbeefdeadbeefdeadbeefdeadbeefdeadbeef\u{1f}feat: 가짜 (moai-bbbb)";
        let got = split(&rec("c1", &format!("chore: 멀쩡한 것 (moai-aaaa){forged}")), &["moai-aaaa", "moai-bbbb"]);
        assert_eq!(got["moai-aaaa"].len(), 1);
        assert_eq!(got["moai-aaaa"][0].hash, "c1", "지어낸 해시가 들었다");
        // 제목이 `moai-bbbb` 를 정말로 적었으니 그 이슈에도 붙는다 — 다만 해시는 진짜다.
        assert_eq!(got["moai-bbbb"][0].hash, "c1", "지어낸 해시가 들었다");
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
        let git = |args: &[&str]| run_git(&dir, None, args);
        git(&["init", "-q"]);
        git(&["commit", "-q", "--allow-empty", "-m", "feat: 고친다 (moai-aaaa)"]);
        git(&["commit", "-q", "--allow-empty", "-m", "chore: 딴 일\n\nmoai-aaaa 를 곁에 봤다"]);
        git(&["commit", "-q", "--allow-empty", "-m", "fix: 리뷰 (moai-aaaa.b1c)"]);
        // 로그를 딴 인코딩으로 달라는 사람의 설정이 있어도 제목을 읽는다 — `run` 이 UTF-8 로 달라고 한다.
        git(&["config", "i18n.logOutputEncoding", "EUC-KR"]);

        let got = commits_of(&dir, &["moai-aaaa", "moai-aaaa.b1c"], None).unwrap();
        let subjects = |id: &str| got[id].iter().map(|c| c.subject.clone()).collect::<Vec<_>>();
        assert_eq!(subjects("moai-aaaa"), ["feat: 고친다 (moai-aaaa)"]);
        assert_eq!(subjects("moai-aaaa.b1c"), ["fix: 리뷰 (moai-aaaa.b1c)"]);
        assert_eq!(got["moai-aaaa"][0].hash.len(), 40, "해시를 줄여 받았다");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **git 훅 안에서 돌아도 시험의 git 은 제 임시 저장소만 본다**(moai-g1a3).
    ///
    /// 물려받은 환경을 바꿔 보려면 시험을 한 겹 더 띄워야 한다 — 병렬로 도는 시험 안에서 환경을
    /// 바꾸면 남의 시험까지 바뀐다(tests/cli.rs 의 `tests_do_not_read_the_runners_home` 과 같은 까닭).
    /// 그래서 훅이 실제로 내보내는 것을 심은 채 이 바이너리를 다시 불러 git 을 부르는 시험들만 돌린다.
    ///
    /// **심는 값은 [`LEAKS`] 가 아니라 git 이 훅에 내보낸 모양이다** — 목록에서 한 이름이 빠지면 여기서
    /// 드러나야 하므로, 목록을 그대로 심으면 아무것도 못 잰다. 걷기가 빠지면 도우미의 커밋이 격리
    /// 경로(`GIT_QUARANTINE_PATH`)나 바깥 설정의 서명(`GIT_CONFIG_PARAMETERS`)에 막혀 안쪽이 깨진다.
    /// 가리키는 곳은 이 시험의 임시 디렉터리라, 걷기가 빠져도 바깥 저장소는 안 건드린다.
    #[test]
    fn git_tests_see_their_own_repos_inside_a_hook() {
        let dir = std::env::temp_dir().join(format!("moai-git-hook-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let outer = dir.join("outer.git");
        let incoming = outer.join("objects/tmp_objdir-incoming");
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            // 이 시험 자신은 뺀다 — 안 그러면 제가 저를 다시 부른다.
            .args(["git::tests::", "worktree::tests::", "--skip", "git_tests_see_their_own_repos_inside_a_hook"])
            .env("GIT_DIR", &outer)
            .env("GIT_INDEX_FILE", outer.join("index"))
            .env("GIT_OBJECT_DIRECTORY", &incoming)
            .env("GIT_ALTERNATE_OBJECT_DIRECTORIES", outer.join("objects"))
            .env("GIT_QUARANTINE_PATH", &incoming)
            .env("GIT_CONFIG_PARAMETERS", "'commit.gpgsign=true' 'gpg.program=false'")
            .output()
            .unwrap();
        let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(out.status.success(), "훅의 환경에서 git 을 부르는 시험이 깨졌다\n{said}");
        // 안쪽이 아무것도 안 돌고 초록으로 끝나면 이 시험은 아무것도 안 본 것이다.
        for ran in ["a_real_log_matches_subjects_not_bodies", "a_moved_head_in_any_worktree_changes_a_watched_stamp"] {
            assert!(said.contains(&format!("{ran} ... ok")), "안쪽에서 {ran} 가 돌지 않았다\n{said}");
        }
    }

    /// **git 은 [`command`] 에서만 띄운다.** 도우미가 따로 띄우면 시험 빌드의 걷기를 못 받아, 훅 안에서
    /// 그 도우미만 바깥 저장소를 본다 — 0a7b828 이 `run` 만 고쳤을 때 임시 저장소를 만드는 도우미 셋이
    /// 그렇게 남아 90a0be9 가 셋을 따로 고쳐야 했다. 새 도우미가 또 그러지 않게 소스를 읽어 이름을 댄다.
    #[test]
    fn git_is_spawned_only_through_command() {
        let needle = concat!("Command::new(", "\"git\")");
        let mut found = Vec::new();
        let mut dirs = vec![std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/src"))];
        while let Some(dir) = dirs.pop() {
            for entry in std::fs::read_dir(&dir).unwrap().filter_map(Result::ok) {
                let path = entry.path();
                if path.is_dir() {
                    dirs.push(path);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    let text = std::fs::read_to_string(&path).unwrap();
                    for (n, line) in text.lines().enumerate() {
                        if line.contains(needle) {
                            found.push(format!("{}:{}", path.display(), n + 1));
                        }
                    }
                }
            }
        }
        assert!(found.len() == 1 && found[0].contains("git.rs:"), "git 을 `git::command` 밖에서 띄운다 — {found:#?}");
    }

    /// 이슈가 생기기 전의 커밋은 걷지 않는다 — 단, 시계가 늦은 기계의 커밋은 하루까지 받는다.
    #[test]
    fn walking_stops_before_the_issue_was_born_but_forgives_a_slow_clock() {
        let dir = std::env::temp_dir().join(format!("moai-git-since-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let git = |at: &str, args: &[&str]| run_git(&dir, Some(at), args);
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

    #[test]
    fn ids_in_splits_words_the_way_names_reads_them() {
        let got: Vec<&str> = ids_in("fix: 리뷰 (moai-aaaa.b1c, moai-bbbb). worktree-moai-cccc 에서 moai-dddd를 봤다").collect();
        assert_eq!(got, ["moai-aaaa.b1c", "moai-bbbb", "worktree-moai-cccc", "moai-dddd"]);
        assert_eq!(ids_in("chore(tracker): 없음 v1.2 a-b").count(), 0, "id 모양이 아닌 낱말을 id 로 읽었다");
    }

    /// 탐색기의 표는 [`commits_of`] 와 **같은 답**을 낸다 — 같은 자로 가르고, 새것이 먼저고,
    /// 트래커 커밋은 표시만 한다. 날짜가 거꾸로 선 커밋 밑의 커밋도 놓치지 않는다(moai-g8cd).
    #[test]
    fn the_table_answers_as_commits_of_without_the_date_cut() {
        let dir = std::env::temp_dir().join(format!("moai-git-table-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let git = |at: &str, args: &[&str]| run_git(&dir, Some(at), args);
        git("2026-01-03T09:00:00Z", &["init", "-q"]);
        git("2026-01-03T09:00:00Z", &["commit", "-q", "--allow-empty", "-m", "feat: 고친다 (moai-aaaa)"]);
        git("2026-01-03T10:00:00Z", &["commit", "-q", "--allow-empty", "-m", "fix: 리뷰 (moai-aaaa.b1c) moai-aaaa.b1c"]);
        // rebase 로 옛 작성 시각이 커밋 시각이 된 커밋 — `--since` 는 여기서 걷기를 끊는다.
        git("2025-12-01T00:00:00Z", &["commit", "-q", "--allow-empty", "-m", "chore(tracker): moai-aaaa 를 닫는다"]);

        let table = table(&dir).unwrap();
        let born = crate::model::parse_rfc3339("2026-01-03T00:00:00Z");
        let subjects = |c: &[Commit]| c.iter().map(|c| (c.subject.clone(), c.tracker)).collect::<Vec<_>>();
        assert_eq!(
            subjects(&table["moai-aaaa"]),
            [("chore(tracker): moai-aaaa 를 닫는다".to_string(), true), ("feat: 고친다 (moai-aaaa)".to_string(), false)]
        );
        assert_eq!(table["moai-aaaa.b1c"].len(), 1, "한 제목에 두 번 적은 id 를 두 커밋으로 셌다");
        assert!(commits_of(&dir, &["moai-aaaa"], born).unwrap().is_empty(), "이 시험이 흉내 낸 모서리가 사라졌다");
        assert_eq!(subjects(&table["moai-aaaa"]), subjects(&commits_of(&dir, &["moai-aaaa"], None).unwrap()["moai-aaaa"]));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
