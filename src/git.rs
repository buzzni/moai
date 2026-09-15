//! git 을 부르는 층.
//!
//! **이슈의 뜻은 여기서 판단하지 않는다.** `report`·`query` 는 `&[Issue]` 에 대한 순수
//! 함수로 남고, git 에서 읽어 오는 것은 이 층을 거쳐 `cmd` 가 받는다.
//!
//! 커밋은 **저장하지 않는다**(moai-1w2l). 이슈에 닿은 커밋은 커밋 제목에 적힌 id 와
//! 본문의 트레일러 줄([`trailed`], moai-dig5)에서 그때그때 읽는다 — 이슈를 닫는 트래커
//! 커밋은 제 해시를 미리 알 수 없고, 적어 둔 해시는 squash·rebase 한 번에 낡는다. id 로
//! 다시 찾으면 둘 다 없다.

use crate::git_leaks::swept;
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
/// 어느 저장소를 볼지는 **`-C root` 가 정한다** — 물려받은 `GIT_DIR` 무리를 걷는 것은 [`command`] 고,
/// 걷지 않으면 무엇이 깨지는지는 [`crate::git_leaks`] 에 있다(moai-ztdf). 여기서 다시 적지 않는다.
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

/// `git log` 을 띄우고 **레코드를 하나씩 흘려 보낸다**(moai-iol3).
///
/// 이력을 통째로 들고 있지 않는다. `%b` 를 더한 뒤로 이력 전체는 커밋 하나에 수백 바이트씩
/// 붙어, 커밋 2만 개짜리 저장소에서 `show` 한 번이 13MB 를 읽고 그것을 다시 `String` 으로
/// 옮겼다 — 화면에 서는 것은 그중 커밋 몇 줄뿐이다. 이제 드는 것은 **레코드 하나**뿐이다.
///
/// **글자가 깨져도 읽는다**(읽기는 관대하고 쓰기는 엄하다). `i18n.logOutputEncoding` 을 legacy 로
/// 둔 저장소나 옛 커밋 하나의 인코딩 때문에 이력 전체를 못 읽으면 커밋 칸이 까닭도 없이 영영
/// 빈다 — 제목은 어차피 `text::sanitize` 를 지나 그려지고, 해시는 [`records_of`] 가 모양으로 한 번
/// 더 거른다.
///
/// **git 이 비영으로 끝나면 실패다.** 레코드를 흘려 받는 동안은 그것을 모르므로, 다 읽은 뒤에
/// 끝을 보고 판단한다 — 반쯤 읽은 이력을 답으로 내면 커밋 칸이 조용히 짧아진다.
fn stream_log<T>(root: &Path, args: &[&str], mut take: impl FnMut(&str, &str, &str) -> Option<T>) -> Result<Vec<T>, Error> {
    use std::io::Read;
    let mut child = command()
        .arg("-C")
        .arg(root)
        .args(["-c", "i18n.logOutputEncoding=UTF-8"])
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(Error::Spawn)?;
    let mut out = child.stdout.take().expect("stdout 을 파이프로 달라고 했다");
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 16 * 1024];
    let mut got = Vec::new();
    let mut keep = |record: &[u8], got: &mut Vec<T>| {
        let text = String::from_utf8_lossy(record);
        if let Some((hash, subject, body)) = records_of(&text) {
            got.extend(take(hash, subject, body));
        }
    };
    loop {
        let read = out.read(&mut chunk).map_err(Error::Spawn)?;
        if read == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..read]);
        // 레코드는 NUL 로 끝난다(`-z`). 마지막 조각은 아직 덜 왔을 수 있으니 버퍼에 남긴다.
        while let Some(at) = buf.iter().position(|b| *b == 0) {
            keep(&buf[..at], &mut got);
            buf.drain(..=at);
        }
    }
    keep(&buf, &mut got);
    let end = child.wait_with_output().map_err(Error::Spawn)?;
    if !end.status.success() {
        return Err(Error::Failed(String::from_utf8_lossy(&end.stderr).trim().to_string()));
    }
    Ok(got)
}

/// git 을 띄울 명령 — **moai 가 부르는 git 은 모두 여기서 시작한다.** moai 가 띄우는 **다른** 프로그램
/// (`skill install` 의 `claude`, 편집기)은 환경을 그대로 물려받는다.
///
/// **저장소를 가리키는 변수는 릴리스에서도 걷는다**([`REPO`](crate::git_leaks::REPO), moai-ztdf). 그것들은
/// `git -C <경로>` 를 이기므로, 걷지 않으면 훅 안에서 부른 `moai -C <다른 프로젝트>` 가 훅 저장소를 읽는다
/// — 커밋 칸과 워크트리 겹쳐 보기가 남의 이력을 내고, `model::git_config` 는 **남의 이름을 그 프로젝트
/// 저널에 영구히** 적는다. 사람·시계·해시([`TEST`](crate::git_leaks::TEST))는 시험 빌드에서만 걷는다 —
/// 커밋 훅에서 사람이 일부러 준 값을 릴리스가 지울 까닭이 없다.
///
/// **이 걷기와 `-C` 는 한 겹이 아니라 두 겹이다 — 서로를 가리지 않는다.** 걷기는 물려받은 변수가 `-C` 를
/// **이기지 못하게** 하고, `-C` 는 어느 저장소를 볼지를 **정한다.** 한쪽만 서면 그만큼만 샌다: 걷기 없이
/// `-C` 만 대면 훅이 내보낸 `GIT_DIR`·`GIT_CONFIG_PARAMETERS` 가 그것을 덮고(moai-ztdf), `-C` 없이 걷기만
/// 하면 프로세스 자리가 답을 정해 뿌리 **밑**의 겹친 저장소가 사람을 갈아 치웠다(moai-d3sy). 이제 사람도
/// 커밋 칸도 `-C <.moai 뿌리>` 한 자에서 온다 — `model::git_config` 와 [`output`] 이 같은 자를 댄다.
///
/// **환경으로 준 대체 객체 저장소는 버린다**(`GIT_OBJECT_DIRECTORY`·`GIT_ALTERNATE_OBJECT_DIRECTORIES`).
/// moai 가 읽는 것은 이미 받아들여진 `HEAD` 의 이력뿐이라 잃을 값이 없고, 남기면 `-C` 로 댄 저장소가
/// 바깥 객체 저장소에 쓴다.
///
/// [`run`] 과 임시 저장소를 만드는 도우미(`isolated`)가 따로 걷으면 걷는 목록이 갈라진다. 한때
/// 도우미는 셋만 걷고 `run` 은 열을 걷어, pre-receive 훅 안에서는 도우미가 바깥 객체 저장소에 쓰고
/// `run` 은 임시 저장소를 읽었다(moai-g1a3). `#[cfg(test)]` 가 아니라 `cfg!(test)` 인 것은 릴리스
/// 빌드에서도 이 코드가 타입 검사를 받게 하려는 것이다.
pub fn command() -> std::process::Command {
    let mut cmd = std::process::Command::new("git");
    for var in swept(cfg!(test)) {
        cmd.env_remove(var);
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

/// 제목이나 본문 트레일러에 그 이슈 id 가 적힌 커밋 하나. `moai show --json` 의 `commits`
/// 가 이 모양 그대로다.
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

/// 해시·제목·본문을 가르는 글자.
///
/// **레코드는 이 글자로 가르지 않는다 — `-z` 가 넣는 NUL 로 가른다.** 커밋 제목에는
/// 제어 문자가 들 수 있고(git 은 `%s` 로 그대로 낸다), 제목 안에 들 수 있는 글자로
/// 레코드를 가르면 그런 제목 하나가 레코드를 둘로 쪼개 **저장소에 없는 해시**를 지어낸다 —
/// 남이 보낸 커밋 한 줄로 남의 이슈 커밋 칸에 가짜 줄을 세울 수 있었다. NUL 은 커밋
/// 메시지에 들지 않는다. 필드는 **앞에서부터** 가르므로 제목에 FS 가 들어도 해시 자리는
/// 못 건드리고, 그 뒤로 밀려난 글이 본문 행세를 못 하게 [`records`] 가 필드 수를 센다.
const FS: char = '\u{1f}';

/// `git log` 에 줄 서식. **[`records`] 가 가르는 자와 한 자리에서 짓는다** — 한쪽만 고치면
/// 제목 자리에 다른 필드가 들어오고, 그것을 알려 주는 것이 없다.
///
/// 본문(`%b`)까지 받는 것은 트레일러 줄 때문이다([`trailed`], moai-dig5).
fn format_arg() -> String {
    format!("--format=%H{FS}%s{FS}%b")
}

/// 본문에서 id 를 세는 **유일한 자리** — `Refs:`·`Closes:`·`Fixes:` 로 시작하는 줄(moai-dig5,
/// 2026-09-15 사용자 결정).
///
/// squash 병합은 합친 커밋들의 제목을 본문의 `* <제목>` 줄로 옮긴다. 제목만 세면 그 커밋들이
/// 통째로 사라지고, 본문을 다 세면 트래커 커밋이 나열한 id 와 "넘긴 것은 …" 같은 문장이 전부
/// 걸린다 — 이 저장소에서 재니 커밋-이슈 연결이 931 에서 1514 로 붇었다. 트레일러는 squash 를
/// 그대로 지나가면서 그 둘 사이를 가른다.
///
/// **들여쓴 줄도 트레일러다**(`trim_start`) — `git merge --squash` 는 합친 메시지를 네 칸씩
/// 들여써 본문에 담으므로, 들여쓰기를 안 봐주면 squash 를 지나가는 길이 그 자리에서 끊긴다.
/// 이 자를 그대로 읽는 것은 `guide` 의 시험이다(`the_commit_guide_teaches_what_the_commit_column_reads`).
pub(crate) fn trailed(body: &str) -> impl Iterator<Item = &str> {
    body.lines().filter(|l| is_trailer(l.trim_start())).flat_map(words)
}

/// 트레일러 줄인가. 낱말은 대소문자를 안 가린다 — `refs:` 로 적는 사람이 흔하다.
fn is_trailer(line: &str) -> bool {
    // **글자 경계로 자른다** — 바이트로 자르면 한글로 시작하는 줄에서 panic 한다(`get` 은 None 이다).
    ["refs:", "closes:", "fixes:"].iter().any(|head| line.get(..head.len()).is_some_and(|h| h.eq_ignore_ascii_case(head)))
}

/// 글을 id 가 될 수 있는 낱말로 가른다 — **경계를 정하는 자는 이것 하나다.**
///
/// 낱말은 id 에 드는 글자(영숫자·`_`·`-`·`.`)가 이어진 한 덩어리고, 끝의 `.` 은 문장 부호로 떼어 낸다.
/// 그래서 앞뒤가 id 의 일부로 이어지면 다른 낱말이다 — `moai-rvcb` 는 자식 `moai-rvcb.7u5` 도, 가지 이름
/// `worktree-moai-rvcb` 도 아니고, 문장 끝 `(moai-rvcb).` 는 그 id 다. 한글 조사가 붙어도 그 id 다.
fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')))
        .map(|t| t.trim_end_matches('.'))
        .filter(|t| !t.is_empty())
}

/// 글에 적힌 **형식에 맞는** id 낱말들 — [`words`] 중 `id::is_valid` 를 지나는 것.
///
/// 커밋을 찾는 길([`table_of`])은 이것을 안 쓴다. 그쪽은 **파일에 있는 id** 와 낱말을
/// 견주므로 형식을 물을 까닭이 없고, 물으면 형식이 어긋난 줄의 칸이 영영 빈다(moai-ynhj).
/// 이 자는 "이 글이 id 를 적었는가" 를 파일 없이 물어야 하는 곳 — 가르치는 글의 예시가 실제로
/// 읽히는지 보는 `guide` 의 시험 — 이 쓴다.
///
/// **`-`·`.` 로 시작하는 낱말은 뺀다.** 접두어가 그 한 글자뿐인 것을 `id::is_valid` 는
/// 통과시켜(`--json` 은 접두어 `-` 에 본체 `json`), 제목에 적힌 플래그가 id 로 읽힌다.
///
/// 파일을 안 보고 묻는 자리가 시험뿐이라 시험 빌드에만 선다 — 쓰는 곳이 생기면 그때 연다.
#[cfg(test)]
pub fn ids_in(text: &str) -> impl Iterator<Item = &str> {
    words(text).filter(|t| !t.starts_with(['-', '.']) && crate::id::is_valid(t))
}

/// 지금 가지(`HEAD`)의 이력 **전부**를 한 번 걸어 제목과 본문 트레일러에 적힌 id → 커밋 표를
/// 짓는다. 새것이 먼저다.
///
/// **두 표면이 이 하나를 쓴다**(moai-hws2) — 탐색기는 이슈 전부의 id 로 한 번(moai-a4i0), `show` 는
/// 펼친 id 하나로 부른다. 탐색기 쪽은 다시 읽기 스레드가 지어 두므로 커서를 옮길 때마다 git 이 안 뜬다.
///
/// **이력을 끝까지 걷는다.** 한때 `show` 만 이슈의 생성일에서 끊었는데, `git log --since` 는 거르기가
/// 아니라 **끊기**라 날짜가 거꾸로 선 커밋(rebase·늦은 시계) 하나가 그 밑을 통째로 가렸다 — 같은
/// 물음에 두 표면이 다른 답을 냈다. 대가는 긴 이력에서의 걷기 값이다(흉내 낸 2만 커밋 저장소에서
/// `show` 한 번이 10ms → 164ms, 2026-09-15 사용자 결정).
///
/// **`ids` 는 파일에 있는 줄의 id 다**(moai-ynhj). 형식으로 거르는 대신 있는 id 와 견주므로,
/// 형식이 어긋난 줄도 제 커밋을 찾고 표에는 id 아닌 낱말이 안 선다.
pub fn table(root: &Path, ids: &[&str]) -> Result<BTreeMap<String, Vec<Commit>>, Error> {
    // **찾을 id 가 없으면 git 도 안 부른다**(옛 `commits_of` 가 지키던 자리). 표가 반드시
    // 빌 걷기라, 이력이 긴 저장소에서 줄 없는 프로젝트를 열면 그 걷기를 통째로 버린다.
    if ids.is_empty() {
        return Ok(BTreeMap::new());
    }
    let format = format_arg();
    let known: std::collections::HashSet<&str> = ids.iter().copied().collect();
    // **레코드를 하나씩 받아 걸러 담는다**(moai-iol3) — 이력 전체가 아니라 맞은 커밋만 든다.
    let hits = stream_log(root, &["log", "-z", "--no-show-signature", format.as_str(), "HEAD", "--"], |hash, subject, body| {
        let named = named_in(subject, body, &known);
        (!named.is_empty()).then(|| (named.into_iter().map(str::to_string).collect::<Vec<_>>(), commit_of(hash, subject)))
    })?;
    let mut out: BTreeMap<String, Vec<Commit>> = BTreeMap::new();
    for (ids, commit) in hits {
        for id in ids {
            out.entry(id).or_default().push(commit.clone());
        }
    }
    Ok(out)
}

/// 이 커밋이 대는 id 들 — 제목의 낱말과 **트레일러 줄의 낱말**을 같은 자로 본다(moai-dig5).
/// 한 커밋이 같은 id 를 두 번 적어도 한 번만 든다.
fn named_in<'a>(subject: &'a str, body: &'a str, known: &std::collections::HashSet<&str>) -> Vec<&'a str> {
    let mut named: Vec<&str> = Vec::new();
    for id in words(subject).chain(trailed(body)).filter(|w| known.contains(w)) {
        if !named.contains(&id) {
            named.push(id);
        }
    }
    named
}

fn commit_of(hash: &str, subject: &str) -> Commit {
    Commit { hash: hash.to_string(), subject: subject.to_string(), tracker: subject.starts_with(TRACKER) }
}

/// [`table`] 의 **git 을 안 부르는 반쪽.**
///
/// 낱말을 있는 id 집합에서 찾는다. 이력 전부와 이슈 전부가 만나는 자리라 id 마다 제목을 다시
/// 훑으면 곱이 된다 — 커밋 1천 × 이슈 1천이면 백만 번이다.
/// 시험 빌드에만 선다 — 진짜 길은 [`table`] 이 흘려 읽는다(moai-iol3). 가르는 자([`named_in`]·
/// [`records_of`])는 둘이 같은 것을 쓰므로, 이 자리로 못 박은 뜻은 흘려 읽는 쪽에도 그대로 선다.
#[cfg(test)]
pub fn table_of(log: &str, ids: &[&str]) -> BTreeMap<String, Vec<Commit>> {
    let known: std::collections::HashSet<&str> = ids.iter().copied().collect();
    let mut out: BTreeMap<String, Vec<Commit>> = BTreeMap::new();
    for (hash, subject, body) in records(log) {
        for id in named_in(subject, body, &known) {
            out.entry(id.to_string()).or_default().push(commit_of(hash, subject));
        }
    }
    out
}

/// `git log -z` 가 낸 레코드를 (해시, 제목, 본문) 으로. 가르는 자의 까닭은 [`FS`] 에 있다.
///
/// **해시 자리가 해시 모양이 아니면 버린다.** `-z` 가 이미 레코드를 지키지만, 지키는 것이
/// 하나뿐이면 그것이 어긋난 날(옛 git, 다른 서식) 지어낸 해시가 화면과 `--json` 으로 그대로
/// 나간다. 여기서 한 번 더 보면 그 길이 막힌다 — `%H` 는 언제나 16진수다.
///
/// **제목이 본문을 지어내지 못한다.** [`format_arg`] 가 내는 [`FS`] 는 **둘**뿐이라 멀쩡한
/// 레코드의 필드는 언제나 셋이다 — 넷째가 있으면 제목(이나 본문)이 그 글자를 담은 것이고,
/// 그때 본문을 버린다. 안 버리면 제목에 `\u{1f}Refs: <남의 id>` 를 담은 커밋 하나가 제
/// 제목에는 그 id 를 안 적고도 남의 커밋 칸에 선다 — 화면에 그리는 제목은 첫 `FS` 에서
/// 잘리므로 **왜 거기 섰는지 보이지도 않는다.** 본문에 그 글자를 담은 커밋은 트레일러를
/// 잃을 뿐이라, 모르는 쪽으로 기우는 값이 싸다.
#[cfg(test)]
fn records(log: &str) -> impl Iterator<Item = (&str, &str, &str)> {
    log.split('\0').filter_map(records_of)
}

/// 레코드 **하나**를 가른다 — 흘려 읽는 쪽([`stream_log`])과 통째로 읽는 쪽([`records`])이 같은 자를 쓴다.
fn records_of(record: &str) -> Option<(&str, &str, &str)> {
    let mut fields = record.split(FS);
    let hash = fields.next()?;
    let subject = fields.next()?;
    // 본문이 없는 커밋도 있다 — 그때 `%b` 는 빈 글자다.
    let body = fields.next().unwrap_or("");
    let sane = !hash.is_empty() && hash.bytes().all(|b| b.is_ascii_hexdigit());
    sane.then_some((hash, subject, if fields.next().is_some() { "" } else { body }))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::git_leaks::{REPO, TEST};

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

    /// 본문 없는 커밋 한 줄 — **git 이 내는 모양 그대로 FS 를 둘 놓는다**([`format_arg`]).
    /// 하나만 놓으면 제목에 `FS` 를 담는 시험이 진짜 로그에 없는 모양을 재게 된다.
    fn rec(hash: &str, subject: &str) -> String {
        rec_body(hash, subject, "")
    }

    fn rec_body(hash: &str, subject: &str, body: &str) -> String {
        format!("{hash}{FS}{subject}{FS}{body}\0")
    }

    /// **제목이 레코드를 못 쪼갠다**(`-z`). 제어 문자를 담은 제목 하나로 저장소에 없는
    /// 해시를 지어내 남의 이슈 커밋 칸에 세울 수 있었다.
    #[test]
    fn a_subject_cannot_forge_a_record() {
        let forged = "\u{1e}deadbeefdeadbeefdeadbeefdeadbeefdeadbeef\u{1f}feat: 가짜 (moai-bbbb)";
        let got = table_of(&rec("c1", &format!("chore: 멀쩡한 것 (moai-aaaa){forged}")), &["moai-aaaa", "moai-bbbb"]);
        assert_eq!(got["moai-aaaa"].len(), 1);
        assert_eq!(got["moai-aaaa"][0].hash, "c1", "지어낸 해시가 들었다");
        assert!(!got.contains_key("moai-bbbb"), "제목이 밀어 넣은 글이 남의 이슈에 붙었다");
    }

    /// **제목에 담은 트레일러는 본문이 아니다.** 필드를 앞에서부터 가르므로 제목에 든 [`FS`]
    /// 뒤는 본문 자리로 밀리는데, 밀려난 글을 본문으로 읽으면 제 제목에 남의 id 를 **안 적고도**
    /// 그 이슈의 커밋 칸에 서는 길이 남는다 — 화면에 그리는 제목은 첫 `FS` 에서 잘리니 왜 거기
    /// 섰는지도 안 보인다. 멀쩡한 레코드의 필드는 언제나 셋이라 넷째로 붙잡는다([`records`]).
    #[test]
    fn a_subject_cannot_forge_a_trailer() {
        let got = table_of(&rec("c1", &format!("chore: 멀쩡한 것{FS}Refs: moai-bbbb")), &["moai-bbbb"]);
        assert!(!got.contains_key("moai-bbbb"), "제목이 밀어 넣은 트레일러가 남의 이슈에 붙었다");
        // 본문에 정말로 적은 트레일러는 그대로 읽는다 — 막은 것은 밀려난 글뿐이다.
        let real = table_of(&rec_body("c2", "chore: 멀쩡한 것", "Refs: moai-bbbb"), &["moai-bbbb"]);
        assert_eq!(real["moai-bbbb"].len(), 1, "본문에 적은 트레일러까지 잃었다");
    }

    /// 위의 막음이 **진짜 git 이 내는 모양**에서 선다 — 손으로 지은 레코드만 재면 필드 수가
    /// 실제와 다를 수 있다.
    #[test]
    fn a_real_subject_cannot_forge_a_trailer() {
        let dir = std::env::temp_dir().join(format!("moai-git-forge-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let git = |args: &[&str]| run_git(&dir, None, args);
        git(&["init", "-q"]);
        git(&["commit", "-q", "--allow-empty", "-m", &format!("chore: 멀쩡한 것{FS}Refs: moai-bbbb")]);
        git(&["commit", "-q", "--allow-empty", "-m", "fix: 제대로 적은 것\n\nRefs: moai-cccc"]);
        let got = table(&dir, &["moai-bbbb", "moai-cccc"]).unwrap();
        assert!(!got.contains_key("moai-bbbb"), "제목이 밀어 넣은 트레일러가 남의 이슈에 붙었다");
        assert_eq!(got["moai-cccc"].len(), 1, "본문에 적은 트레일러를 안 셌다");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **찾을 id 가 없으면 git 을 안 부른다.** 있지도 않은 자리를 주고 잰다 — 불렀으면 git 이
    /// 그리로 못 가 `Err` 다.
    #[test]
    fn an_empty_id_list_never_walks() {
        let nowhere = std::env::temp_dir().join(format!("moai-git-none-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&nowhere);
        assert!(table(&nowhere, &[]).unwrap().is_empty(), "줄이 없는데 이력을 걸었다");
    }

    /// 낱말 경계 — 찾는 쪽([`table_of`])이 이 자로 id 를 가른다.
    #[test]
    fn an_id_is_named_only_as_itself() {
        let names = |text: &str, id: &str| words(text).any(|w| w == id);
        assert!(names("merge: 막음 줄 (moai-rvcb)", "moai-rvcb"));
        assert!(names("moai-rvcb 를 닫는다", "moai-rvcb"));
        assert!(names("닫았다 moai-rvcb.", "moai-rvcb"), "문장 끝의 점은 자식이 아니다");
        assert!(!names("리뷰 moai-rvcb.7u5 를 닫는다", "moai-rvcb"), "자식의 커밋을 부모가 가져간다");
        assert!(!names("Merge branch 'worktree-moai-rvcb'", "moai-rvcb"), "가지 이름이 id 로 읽혔다");
        assert!(!names("moai-rvcbx", "moai-rvcb"));
        assert!(names("리뷰 moai-rvcb.7u5 를 닫는다", "moai-rvcb.7u5"));
    }

    /// **squash 병합이 본문으로 옮긴 id 는 트레일러 줄에서만 센다**(moai-dig5, 2026-09-15 사용자 결정).
    ///
    /// GitHub 의 기본 단추인 squash 는 합친 커밋들의 제목을 전부 본문의 `* <제목>` 줄로 옮긴다 —
    /// 제목만 세면 다섯 중 넷의 커밋 칸이 말없이 빈다. 그렇다고 본문을 다 세면 이 저장소에서만
    /// 커밋-이슈 연결이 931 → 1514 로 붇는다(트래커 커밋이 본문에 나열한 id, "넘긴 것은 …" 같은
    /// 문장이 전부 걸린다). `Refs:`·`Closes:`·`Fixes:` 줄은 squash 를 그대로 지나가고 제목 예산도
    /// 안 쓰므로 그 줄만 센다.
    ///
    /// **진짜 squash 로 잰다** — 손으로 지은 본문은 git 이 실제로 무엇을 옮기는지를 안 보여 준다.
    #[test]
    fn a_squashed_body_counts_only_its_trailers() {
        let dir = std::env::temp_dir().join(format!("moai-git-squash-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let git = |args: &[&str]| run_git(&dir, None, args);
        git(&["init", "-q"]);
        git(&["commit", "-q", "--allow-empty", "-m", "첫 커밋"]);
        git(&["checkout", "-q", "-b", "feat"]);
        git(&["commit", "-q", "--allow-empty", "-m", "feat: 첫 걸음 (moai-aaaa)"]);
        git(&["commit", "-q", "--allow-empty", "-m", "fix: 둘째 걸음 (moai-bbbb)\n\nRefs: moai-cccc"]);
        git(&["checkout", "-q", "main"]);
        git(&["merge", "-q", "--squash", "feat"]);
        // squash 가 지어 준 본문 그대로 커밋한다 — 제목만 이쪽이 적는다.
        let made = std::fs::read_to_string(dir.join(".git/SQUASH_MSG")).unwrap();
        git(&["commit", "-q", "--allow-empty", "-m", &format!("feat: 가지를 합친다 (moai-dddd)\n\n{made}")]);

        let ids = ["moai-aaaa", "moai-bbbb", "moai-cccc", "moai-dddd"];
        let got = table(&dir, &ids).unwrap();
        let has = |id: &str| got.get(id).is_some_and(|c: &Vec<Commit>| c.iter().any(|c| c.subject.contains("가지를 합친다")));
        assert!(has("moai-dddd"), "제목의 id 를 못 찾았다");
        assert!(has("moai-cccc"), "본문의 Refs: 트레일러를 안 셌다 — squash 를 쓰면 여기만 남는다");
        assert!(!has("moai-aaaa") && !has("moai-bbbb"), "본문에 옮겨진 제목까지 셌다 — 트래커 커밋이 나열한 id 가 죄다 걸린다");
        // `show` 도 같은 답이다.
        let one = table(&dir, &["moai-cccc"]).unwrap();
        assert_eq!(one["moai-cccc"].len(), 1, "show 가 트레일러를 안 셌다 — 두 표면이 갈렸다");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **형식에 안 맞는 id 의 줄도 커밋 칸을 읽는다**(moai-ynhj). `Issue::validate` 는 `is_valid` 를
    /// **쓸 때만** 건다 — 들여오거나 손으로 고친 줄은 `argos-4ae`(본체가 짧다)처럼 어긋난 id 를 들 수
    /// 있고, 그런 줄도 목록·탐색기에는 멀쩡히 선다. 커밋이 그 id 를 그대로 적었는데 칸만 영영 비면
    /// "읽기는 관대하고 쓰기는 엄하다" 가 깨진다.
    ///
    /// **두 표면이 같은 답을 내야 한다** — 둘 다 [`table`] 로 찾는다(moai-hws2).
    #[test]
    fn a_malformed_id_still_finds_its_commits() {
        let odd = "argos-4ae";
        let log = [rec("c2", &format!("feat: 들여온 줄을 고친다 ({odd})")), rec("c1", "chore: 딴 일")].concat();
        // 두 표면이 이 한 자를 부르므로(`show` 도 탐색기도 [`table`]) 한 번 재면 둘 다 잰 것이다.
        let got = table_of(&log, &[odd]);
        assert_eq!(got[odd].len(), 1, "형식에 안 맞는 id 의 커밋을 못 찾는다");
        // 그래도 **아무 낱말이나 id 가 되지는 않는다** — 표는 있는 id 로만 선다.
        assert!(!got.contains_key("chore"), "id 가 아닌 낱말이 표에 섰다");
        assert!(!got.contains_key("feat"), "id 가 아닌 낱말이 표에 섰다");
    }

    #[test]
    fn the_table_reads_subjects_marks_tracker_commits_and_keeps_order() {
        let log = [
            rec("c3", "chore(tracker): moai-aaaa 를 main 머지와 함께 닫는다"),
            rec("c2", "merge: 무엇 (moai-aaaa)"),
            rec("c1", "feat: 다른 것 (moai-bbbb, moai-aaaa)"),
        ]
        .concat();
        let got = table_of(&log, &["moai-aaaa", "moai-bbbb", "moai-cccc"]);
        let a: Vec<(&str, bool)> = got["moai-aaaa"].iter().map(|c| (c.hash.as_str(), c.tracker)).collect();
        assert_eq!(a, [("c3", true), ("c2", false), ("c1", false)]);
        assert_eq!(got["moai-bbbb"].len(), 1);
        assert!(!got.contains_key("moai-cccc"), "커밋 없는 id 에 빈 칸이 섰다");
    }

    /// 트레일러가 아닌 본문 줄에만 id 가 든 커밋은 그 이슈의 커밋이 아니다(moai-dig5) —
    /// 곁다리로 남을 언급한 글이 그 이슈의 커밋 칸에 서면 안 된다.
    #[test]
    fn a_real_log_matches_subjects_and_trailers_not_plain_bodies() {
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

        let got = table(&dir, &["moai-aaaa", "moai-aaaa.b1c"]).unwrap();
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
    /// **심는 값은 [`REPO`]·[`TEST`] 가 아니라 git 이 훅에 내보낸 모양이다** — 목록에서 한 이름이 빠지면
    /// 여기서 드러나야 하므로, 목록을 그대로 심으면 아무것도 못 잰다. 걷기가 빠지면 도우미의 커밋이 격리
    /// 경로(`GIT_QUARANTINE_PATH`)나 바깥 설정의 서명(`GIT_CONFIG_PARAMETERS`)에 막혀 안쪽이 깨진다.
    /// 가리키는 곳은 이 시험의 임시 디렉터리라, 걷기가 빠져도 바깥 저장소는 안 건드린다.
    ///
    /// **이 시험은 시험 빌드만 잰다.** 안쪽도 `cfg!(test)` 라 `REPO` 와 `TEST` 를 다 걷는다 — 릴리스가
    /// `TEST` 를 안 걷는다는 것은 여기서 안 드러난다. 그쪽은 tests/cli.rs 가 진짜 바이너리로 본다.
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
        for ran in
            ["a_real_log_matches_subjects_and_trailers_not_plain_bodies", "a_moved_head_in_any_worktree_changes_a_watched_stamp"]
        {
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

    /// **걷는 목록이 git 이 대는 것보다 좁으면 안 된다**(moai-ztdf 리뷰).
    ///
    /// `git rev-parse --local-env-vars` 는 git 이 스스로 "저장소 지역 환경" 으로 세는 이름을 낸다 —
    /// 서브모듈로 들어갈 때 git 이 제 손으로 걷는 바로 그 목록이다. [`REPO`]·[`TEST`] 는 손으로 적은
    /// 것이라, git 이 이름을 하나 더하면 조용히 낡는다. 낡은 것이 드러나는 자리가 남의 커밋 칸이나
    /// 남의 저널뿐이면 그때는 이미 늦으니, git 에게 직접 물어 여기서 먼저 터지게 한다. 처음 물었을 때
    /// 여섯이 빠져 있었다(`GIT_CONFIG`·`GIT_SHALLOW_FILE`·`GIT_GRAFT_FILE` 등).
    ///
    /// **어느 목록인지는 안 본다 — 경계는 여기서 안 붙잡힌다.** 릴리스가 걷는 것은 `REPO` 뿐인데
    /// `GIT_CONFIG_PARAMETERS`·`GIT_CONFIG_COUNT` 는 git 이 대는데도 `TEST` 에 있어, 여기서 목록을
    /// 가리면 오늘 당장 빨갛다. 그래서 이 시험은 **빠진 이름이 없다** 만 지키고, 릴리스가 걷느냐는
    /// tests/cli.rs 의 `an_inherited_git_dir_does_not_beat_the_project_we_were_given` 이 `REPO` 를
    /// 통째로 심어서 붙잡는다. 새 이름을 `TEST` 에 넣어 이 시험을 달래는 것은 그쪽을 못 속인다.
    #[test]
    fn the_leak_list_covers_what_git_calls_local() {
        // **`isolated` 로 띄운다** — `git::tests` 의 git 은 모두 그렇다. 맨 `command()` 면 돌리는 사람의
        // 전역·시스템 설정을 읽고 카고의 자리에서 돈다.
        let out = isolated(Path::new(env!("CARGO_MANIFEST_DIR")))
            .args(["rev-parse", "--local-env-vars"])
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let said = String::from_utf8_lossy(&out.stdout).into_owned();
        let missing: Vec<&str> = said
            .lines()
            .map(str::trim)
            .filter(|n| !n.is_empty() && !REPO.contains(n) && !TEST.contains(n))
            .collect();
        assert!(missing.is_empty(), "git 이 대는 지역 환경이 목록에 없다 — git_leaks.rs 에 더한다: {missing:?}");
    }

    /// [`ids_in`] 은 [`words`] 와 **같은 경계**로 가르고 형식만 더 본다 — `guide` 의 시험이
    /// 가르치는 예시 제목을 이 자로 읽는다.
    #[test]
    fn ids_in_splits_words_the_way_the_table_reads_them() {
        let got: Vec<&str> = ids_in("fix: 리뷰 (moai-aaaa.b1c, moai-bbbb). worktree-moai-cccc 에서 moai-dddd를 봤다").collect();
        assert_eq!(got, ["moai-aaaa.b1c", "moai-bbbb", "worktree-moai-cccc", "moai-dddd"]);
        assert_eq!(ids_in("chore(tracker): 없음 v1.2 a-b").count(), 0, "id 모양이 아닌 낱말을 id 로 읽었다");
    }

    /// 표는 **이력 전부를 걷는다** — 새것이 먼저고, 트래커 커밋은 표시만 하고, 날짜가 거꾸로 선
    /// 커밋 밑의 커밋도 놓치지 않는다(moai-hws2). `show` 도 이 표를 쓰므로 두 표면의 답이 같다 —
    /// CLI 쪽은 tests/cli.rs 의 `show_sees_commits_under_a_backdated_one` 이 따로 본다.
    #[test]
    fn the_table_walks_the_whole_history() {
        let dir = std::env::temp_dir().join(format!("moai-git-table-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let git = |at: &str, args: &[&str]| run_git(&dir, Some(at), args);
        git("2026-01-03T09:00:00Z", &["init", "-q"]);
        git("2026-01-03T09:00:00Z", &["commit", "-q", "--allow-empty", "-m", "feat: 고친다 (moai-aaaa)"]);
        git("2026-01-03T10:00:00Z", &["commit", "-q", "--allow-empty", "-m", "fix: 리뷰 (moai-aaaa.b1c) moai-aaaa.b1c"]);
        // rebase 로 옛 작성 시각이 커밋 시각이 된 커밋 — `--since` 는 여기서 걷기를 끊는다.
        git("2025-12-01T00:00:00Z", &["commit", "-q", "--allow-empty", "-m", "chore(tracker): moai-aaaa 를 닫는다"]);

        let table = table(&dir, &["moai-aaaa", "moai-aaaa.b1c"]).unwrap();
        let subjects = |c: &[Commit]| c.iter().map(|c| (c.subject.clone(), c.tracker)).collect::<Vec<_>>();
        assert_eq!(
            subjects(&table["moai-aaaa"]),
            [("chore(tracker): moai-aaaa 를 닫는다".to_string(), true), ("feat: 고친다 (moai-aaaa)".to_string(), false)],
            "날짜가 거꾸로 선 커밋 밑을 못 봤다"
        );
        assert_eq!(table["moai-aaaa.b1c"].len(), 1, "한 제목에 두 번 적은 id 를 두 커밋으로 셌다");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
