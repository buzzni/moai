//! 저장소를 찾고 읽고 쓴다. **파일을 만지는 곳은 여기뿐이다** (`cmd::init` 제외).
//!
//! `issues.jsonl` 을 바꾸는 길은 [`Repo::with_write`] 하나다. 명령마다 쓰기
//! 경로가 갈라지면 락·정렬·검증·원자적 쓰기를 저마다 반쯤 구현하게 된다.

use crate::config::Config;
use crate::fail::{Fail, R, code};
use crate::model::{Issue, JournalEntry};
use fs2::FileExt;
use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// 락을 못 잡으면 **아무것도 쓰지 않고** 물러난다.
const LOCK_TIMEOUT: Duration = Duration::from_secs(5);

pub struct Repo {
    pub root: PathBuf,
    pub config: Config,
}

/// 읽다가 만난 잘못된 줄. **한 줄이 깨졌다고 파일을 통째로 거부하지 않는다** —
/// 거부하면 무엇이 잘못됐는지 볼 방법까지 같이 사라진다.
#[derive(Debug)]
pub struct LoadError {
    pub line: usize,
    pub message: String,
    /// 못 읽은 줄의 **원문 그대로**. 이것이 있어야 되쓸 때 그 줄을 잃지 않는다.
    pub text: String,
    /// 그 줄이 쓰고 있는 id. **줄을 `Issue` 로 못 읽는 것과 그 안의 `id` 를
    /// 못 읽는 것은 다른 일이다** — 한 단 낮게(`serde_json::Value`) 읽으면
    /// 대개 나온다. JSON 도 아닌 줄에서는 `None` 이고, 그때는 지어내지 않는다.
    pub id: Option<String>,
}

#[derive(Debug, Default)]
pub struct Load {
    pub issues: Vec<Issue>,
    pub errors: Vec<LoadError>,
}

impl Load {
    pub fn get(&self, id: &str) -> Option<&Issue> {
        self.issues.iter().find(|i| i.id == id)
    }

    /// 못 읽는 줄이 이미 쓰고 있는 id. **새 id 를 여기서 피해 뽑는다.**
    ///
    /// 안 피하면 못 읽는 동안은 아무 데도 안 보이는 중복이 생기고, 그 줄이
    /// 읽히게 되는 날(새 바이너리로 갈아타면) `duplicate_id` 가 서서 모든
    /// 쓰기가 막힌다 — 그때는 어느 줄을 고쳐야 하는지도 사람이 알아내야 한다.
    pub fn reserved_ids(&self) -> BTreeSet<String> {
        self.errors.iter().filter_map(|e| e.id.clone()).collect()
    }
}

impl Repo {
    /// `.moai/` 를 가진 디렉터리를 위로 찾는다. 깊이를 코드에 박지 않는다.
    pub fn discover() -> R<Repo> {
        let mut dir = std::env::current_dir().map_err(|e| Fail::new(e.to_string()))?;
        loop {
            if dir.join(".moai").is_dir() {
                let config = Config::load(&dir)?;
                return Ok(Repo { root: dir, config });
            }
            if !dir.pop() {
                return Err("moai 저장소가 아니다 (.moai/ 를 못 찾았다). `moai init` 으로 시작한다".into());
            }
        }
    }

    pub fn dir(&self) -> PathBuf {
        self.root.join(".moai")
    }
    pub fn issues_path(&self) -> PathBuf {
        self.dir().join("issues.jsonl")
    }
    pub fn journal_path(&self) -> PathBuf {
        self.dir().join("journal.jsonl")
    }

    /// 전부 메모리로 읽는다. 디스크 인덱스는 두지 않는다 — 이전 시도가
    /// SQLite 인덱스를 만들어 재 보고 **순수 손해**임을 확인했다.
    ///
    /// **읽기는 락을 잡지 않는다.** 쓰기가 `rename` 으로 갈아끼우므로 독자는
    /// 옛 파일 아니면 새 파일을 보지, 찢어진 파일을 볼 수 없다.
    pub fn read(&self) -> R<Load> {
        let path = self.issues_path();
        let src = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(Fail::new(format!("{}: {e}", path.display()))),
        };
        Ok(parse_issues(&src))
    }

    /// `issues.jsonl` 을 바꾸는 **유일한 경로**.
    ///
    /// 락 → (락 안에서) 읽기 → 고치기 → 정규화·검증·정렬 → 원자적 교체 →
    /// 저널 추가. 순서가 중요하다: **스냅샷 먼저, 저널 나중.** 중간에 죽으면
    /// 저널에 줄이 하나 비는데(이력 공백), 반대 순서면 저널이 일어나지 않은
    /// 일을 주장한다. 빠진 일기가 거짓말하는 일기보다 싸다.
    /// 닫는 함수는 `(이슈들, 설정, 못 읽는 줄이 이미 쓰는 id)` 를 받는다.
    /// 셋째 것을 **인자로 주는 까닭**은 안 쓰는 쪽이 잊을 수 없게 하려는
    /// 것이다 — 락 안에서 읽은 것이라 밖에서 다시 구하면 그 사이에 달라진다.
    pub fn with_write<T, F>(&self, f: F) -> R<T>
    where
        F: FnOnce(&mut Vec<Issue>, &Config, &BTreeSet<String>) -> R<(Vec<JournalEntry>, T)>,
    {
        let _lock = Lock::acquire(&self.dir().join("lock"))?;

        // 락을 잡은 **뒤에** 읽는다. 밖에서 읽으면 두 프로세스가 같은 옛 상태를
        // 고쳐 쓰고, 나중에 rename 한 쪽이 앞의 이슈를 조용히 지운다.
        let load = self.read()?;
        // **못 읽은 줄 때문에 쓰기를 막지 않는다.** 들고 있다가 그대로 되쓴다.
        //
        // 한때 여기서 통째로 거절했다. 그러면 뒷 단계 바이너리가 쓴 줄 하나가
        // 앞 단계 사람의 `add`·`mv`·`edit` 을 전부 막아, 되돌릴 방법이 도구
        // 밖에만 남는다 — CLAUDE.md 가 이름 붙여 둔 실패다. 엄함은 *지금 쓰는
        // 줄*에 대한 것이지 파일 전체에 대한 것이 아니다.
        //
        // 잃지도 않고 막지도 않는 대신 **시끄럽다**: `moai status` 가
        // `unreadable_line` 을 치명으로 내고 거기서만 비영 종료한다.
        let opaque: Vec<&str> = load.errors.iter().map(|e| e.text.as_str()).collect();
        CARRIED.store(opaque.len(), std::sync::atomic::Ordering::Relaxed);
        let before = render_issues(&load.issues, &opaque);

        // 정규화한 원본을 들고 있다가 **바뀐 줄만** 검사한다.
        //
        // 전부 검사하면 남의 낡은 줄 하나가 모든 쓰기를 막는다 — config 에서
        // 칸 이름을 하나 고치는 순간 그 칸에 있던 이슈 때문에 `moai add` 조차
        // 안 된다. 읽기는 관대하고 쓰기는 엄하다는 규칙은 **지금 쓰는 줄**에
        // 대한 것이지, 파일 전체에 대한 것이 아니다.
        let mut original = load.issues.clone();
        for o in original.iter_mut() {
            o.normalize();
        }

        let reserved = load.reserved_ids();
        let mut issues = load.issues;
        let (entries, out) = f(&mut issues, &self.config, &reserved)?;

        for i in issues.iter_mut() {
            i.normalize();
            if original.iter().find(|o| o.id == i.id) != Some(&*i) {
                i.validate(&self.config)?;
            }
        }
        issues.sort_by(|a, b| a.id.cmp(&b.id));
        if let Some(dup) = first_duplicate(&issues) {
            return Err(Fail::coded(format!("id 가 두 번 있다 — {dup}"), code::BROKEN));
        }

        // 내용이 그대로면 스냅샷은 건드리지 않는다 (헛 diff 방지).
        // 저널은 따로다 — `moai note` 처럼 스냅샷을 안 바꾸는 기록이 있다.
        let after = render_issues(&issues, &opaque);
        if after != before {
            write_atomic(&self.issues_path(), after.as_bytes())?;
        }
        if !entries.is_empty() {
            self.append_journal(&entries)?;
        }
        Ok(out)
    }

    fn append_journal(&self, entries: &[JournalEntry]) -> R<()> {
        let path = self.journal_path();
        let mut buf = String::new();
        for e in entries {
            buf.push_str(&serde_json::to_string(e).map_err(|e| Fail::new(e.to_string()))?);
            buf.push('\n');
        }
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| Fail::new(format!("{}: {e}", path.display())))?;
        f.write_all(buf.as_bytes()).map_err(|e| Fail::new(format!("{}: {e}", path.display())))?;
        f.sync_all().map_err(|e| Fail::new(format!("{}: {e}", path.display())))
    }

    /// `moai show <id>` 의 이력 전용. **접지 않는다.**
    ///
    /// 이 함수가 `Vec<Issue>` 를 돌려주게 되는 날이 저널을 상태의 원천으로
    /// 삼기 시작한 날이고, 이전 시도가 거기서 복잡해졌다.
    pub fn journal_of(&self, id: &str) -> R<Vec<JournalEntry>> {
        let path = self.journal_path();
        let src = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(Fail::new(format!("{}: {e}", path.display()))),
        };
        let src = src.strip_prefix('\u{feff}').unwrap_or(&src);
        let mut out: Vec<JournalEntry> = src
            .lines()
            .filter(|l| !l.trim().is_empty())
            // 모르는/깨진 줄은 건너뛴다. 저널은 상태를 만들지 않으므로
            // 여기서 관대해도 답이 틀리지 않는다.
            .filter_map(|l| serde_json::from_str::<JournalEntry>(l).ok())
            .filter(|e| e.id == id)
            .collect();
        out.sort_by(|a, b| a.ts.cmp(&b.ts));
        Ok(out)
    }
}

/// 방금 쓰기가 **그대로 들고 넘어간** 못 읽는 줄의 수.
///
/// `store` 는 터미널을 모르므로 찍지 않는다 — 세어 두기만 하고 `main` 이
/// 말한다. `cmd::had_partial` 과 같은 모양이고, 같은 까닭이다: 쓰기 명령마다
/// 파일을 한 번 더 읽어 보고하게 하면 그 읽기가 락 밖이라 락 안에서 본 것과
/// 다를 수 있다.
static CARRIED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub fn carried_unreadable() -> usize {
    CARRIED.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn parse_issues(src: &str) -> Load {
    let src = src.strip_prefix('\u{feff}').unwrap_or(src);
    let mut load = Load::default();
    for (i, line) in src.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Issue>(line) {
            Ok(issue) => load.issues.push(issue),
            Err(e) => load.errors.push(LoadError {
                line: i + 1,
                message: e.to_string(),
                text: line.to_string(),
                // **둘째 파서를 만들지 않는다.** 같은 `serde_json` 을 한 겹
                // 아래로 부를 뿐이라 본체와 어긋날 자리가 없다 — 정규식으로
                // 긁었으면 그 순간 파서가 둘이 되고, 둘은 언젠가 갈라진다.
                id: serde_json::from_str::<serde_json::Value>(line)
                    .ok()
                    .and_then(|v| v.get("id")?.as_str().map(str::to_string)),
            }),
        }
    }
    load.issues.sort_by(|a, b| a.id.cmp(&b.id));
    load
}

/// 정렬은 `id` 바이트 오름차순이다. `-`(0x2D) < `.`(0x2E) < 숫자 < 소문자 라서
/// 자식이 부모 바로 밑에 붙고, 랜덤 id 가 삽입 위치를 파일 전체에 흩뿌려
/// git 충돌 확률을 떨어뜨린다 (파일 끝 append 는 두 브랜치가 **항상** 부딪친다).
///
/// **못 읽은 줄은 뒤에 그대로 붙는다.** 제자리에 둘 수가 없다 — 차례를 정하는
/// 것이 `id` 인데 그 줄은 `id` 를 못 읽어서 못 읽은 줄이다. 첫 쓰기 한 번만
/// 자리가 밀리고 그 뒤로는 움직이지 않는다.
fn render_issues(issues: &[Issue], opaque: &[&str]) -> String {
    let mut out = String::new();
    for i in issues {
        out.push_str(&serde_json::to_string(i).expect("Issue 는 언제나 직렬화된다"));
        out.push('\n');
    }
    for line in opaque {
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn first_duplicate(sorted: &[Issue]) -> Option<&str> {
    sorted.windows(2).find(|w| w[0].id == w[1].id).map(|w| w[0].id.as_str())
}

/// 이미 쓰이고 있는 id 전부 — 읽은 줄의 것과 **못 읽는 줄의 것까지.**
///
/// 둘을 한 함수로 합쳐 두는 까닭은 한쪽만 넘기는 것이 불가능해야 하기
/// 때문이다. 갈라 두면 부르는 쪽이 언젠가 하나를 잊고, 잊은 그날은 아무
/// 증상도 없다.
pub fn taken_ids(issues: &[Issue], reserved: &BTreeSet<String>) -> BTreeSet<String> {
    issues.iter().map(|i| i.id.clone()).chain(reserved.iter().cloned()).collect()
}

/// temp 에 쓰고 `rename` 으로 갈아끼운다. 독자는 옛 파일 아니면 새 파일만 본다.
fn write_atomic(path: &Path, bytes: &[u8]) -> R<()> {
    let dir = path.parent().ok_or_else(|| Fail::new("경로에 디렉터리가 없다"))?;
    let tmp = dir.join(format!(
        "{}.tmp.{}",
        path.file_name().and_then(|s| s.to_str()).unwrap_or("out"),
        std::process::id()
    ));
    let err = |e: std::io::Error| Fail::new(format!("{}: {e}", tmp.display()));
    {
        let mut f = std::fs::File::create(&tmp).map_err(err)?;
        f.write_all(bytes).map_err(err)?;
        f.sync_all().map_err(err)?;
    }
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        Fail::new(format!("{}: {e}", path.display()))
    })?;
    // rename 자체는 원자적이지만 디렉터리 엔트리는 아직 디스크에 없을 수 있다.
    #[cfg(unix)]
    if let Ok(d) = std::fs::File::open(dir) {
        let _ = d.sync_all();
    }
    Ok(())
}

/// `flock(2)`. 프로세스가 죽으면 커널이 놓아 준다.
///
/// 직접 만든 락 파일(`O_EXCL`)을 쓰지 않는 이유가 이것이다 — 죽으면 찌꺼기가
/// 남아 **사람이 손으로 지워야 한다.** 사람 손이 덜 가게 하려고 만드는 도구에
/// "락 파일 좀 지워주세요" 를 넣을 수는 없다.
struct Lock(std::fs::File);

impl Lock {
    fn acquire(path: &Path) -> R<Lock> {
        let f = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(path)
            .map_err(|e| Fail::new(format!("{}: {e}", path.display())))?;
        let start = Instant::now();
        loop {
            match f.try_lock_exclusive() {
                Ok(()) => return Ok(Lock(f)),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.raw_os_error() == fs2::lock_contended_error().raw_os_error() => {
                    if start.elapsed() >= LOCK_TIMEOUT {
                        return Err(Fail::coded(
                            format!(
                                "{} 초 동안 다른 moai 가 쓰고 있어 물러난다. 아무것도 바뀌지 않았다",
                                LOCK_TIMEOUT.as_secs()
                            ),
                            code::LOCKED,
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(e) => return Err(Fail::new(format!("{}: {e}", path.display()))),
            }
        }
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Kind, Status};

    const T: &str = "2026-09-11T04:12:03Z";

    fn issue(id: &str) -> Issue {
        Issue::new(id.into(), format!("{id} 의 제목"), Kind::Issue, Status::new("todo"), T)
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("moai-store-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".moai")).unwrap();
        std::fs::write(dir.join(".moai/config.toml"), "prefix = \"argos\"\n").unwrap();
        std::fs::write(dir.join(".moai/issues.jsonl"), "").unwrap();
        dir
    }

    fn repo(name: &str) -> (Repo, PathBuf) {
        let root = scratch(name);
        let config = Config::load(&root).unwrap();
        (Repo { root: root.clone(), config }, root)
    }

    #[test]
    fn writes_and_reads_back() {
        let (r, _d) = repo("rw");
        r.with_write(|issues, _, _| {
            issues.push(issue("argos-4aex"));
            Ok((vec![JournalEntry::create("argos-4aex", "t", T, &crate::model::someone("raven"))], ()))
        })
        .unwrap();
        let load = r.read().unwrap();
        assert_eq!(load.issues.len(), 1);
        assert!(load.errors.is_empty());
        assert_eq!(r.journal_of("argos-4aex").unwrap().len(), 1);
    }

    /// 임의 순서로 넣어도 파일은 언제나 id 순이고, 자식이 부모 밑에 붙는다.
    #[test]
    fn output_is_sorted_and_deterministic() {
        let (r, d) = repo("sorted");
        r.with_write(|issues, _, _| {
            for id in ["argos-4aey", "argos-4aex.ae3", "argos-0001", "argos-4aex"] {
                issues.push(issue(id));
            }
            Ok((vec![], ()))
        })
        .unwrap();
        let src = std::fs::read_to_string(d.join(".moai/issues.jsonl")).unwrap();
        let ids: Vec<&str> = src
            .lines()
            .map(|l| l.split('"').nth(3).unwrap())
            .collect();
        assert_eq!(ids, ["argos-0001", "argos-4aex", "argos-4aex.ae3", "argos-4aey"]);
    }

    /// 바뀐 게 없으면 파일을 건드리지 않는다 — 헛 diff 를 만들지 않는다.
    #[test]
    fn unchanged_write_leaves_the_file_alone() {
        let (r, d) = repo("idem");
        r.with_write(|i, _, _| {
            i.push(issue("argos-4aex"));
            Ok((vec![], ()))
        })
        .unwrap();
        let path = d.join(".moai/issues.jsonl");
        let before = std::fs::metadata(&path).unwrap().modified().unwrap();
        std::thread::sleep(Duration::from_millis(20));
        r.with_write(|_, _, _| Ok((vec![], ()))).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), before);
    }

    /// 스냅샷을 안 바꾸는 기록(`note`)도 저널에는 남아야 한다.
    #[test]
    fn journal_only_writes_still_land() {
        let (r, _d) = repo("note");
        r.with_write(|i, _, _| {
            i.push(issue("argos-4aex"));
            Ok((vec![], ()))
        })
        .unwrap();
        r.with_write(|_, _, _| Ok((vec![JournalEntry::note("argos-4aex", "발견", T, &crate::model::someone("raven"))], ())))
            .unwrap();
        let j = r.journal_of("argos-4aex").unwrap();
        assert_eq!(j.len(), 1);
        assert_eq!(j[0].text.as_deref(), Some("발견"));
    }

    /// 41번 줄이 깨져도 나머지가 읽히고, 어느 줄인지 보고된다.
    #[test]
    fn a_broken_line_does_not_sink_the_file() {
        let mut src = String::new();
        for n in 0..5 {
            src.push_str(&serde_json::to_string(&issue(&format!("argos-000{n}"))).unwrap());
            src.push('\n');
        }
        let mut lines: Vec<&str> = src.lines().collect();
        lines[2] = "{\"id\": 깨짐";
        let load = parse_issues(&lines.join("\n"));
        assert_eq!(load.issues.len(), 4);
        assert_eq!(load.errors.len(), 1);
        assert_eq!(load.errors[0].line, 3);
    }

    /// 마지막 줄이 잘린 파일 — 프로세스가 죽었을 때 나올 수 있는 모양.
    #[test]
    fn a_truncated_tail_is_reported_not_fatal() {
        let good = serde_json::to_string(&issue("argos-0001")).unwrap();
        let load = parse_issues(&format!("{good}\n{{\"id\":\"argos-00"));
        assert_eq!(load.issues.len(), 1);
        assert_eq!(load.errors.len(), 1);
    }

    #[test]
    fn bom_and_blank_lines_are_tolerated() {
        let good = serde_json::to_string(&issue("argos-0001")).unwrap();
        let load = parse_issues(&format!("\u{feff}{good}\n\n"));
        assert_eq!(load.issues.len(), 1);
        assert!(load.errors.is_empty());
        assert!(parse_issues("").issues.is_empty());
    }

    /// 깨진 줄이 있어도 **쓴다.** 그 줄은 글자 하나 안 바뀌고 남는다.
    ///
    /// 한때 여기서 통째로 거절했다 — "쓰면 그 줄이 사라진다" 는 걱정이었다.
    /// 거절 대신 들고 있으면 걱정도 없고 막히지도 않는다. 막는 쪽이 비싼
    /// 이유는 뒷 단계 바이너리가 쓴 줄 하나가 앞 단계 사람의 모든 쓰기를
    /// 막아, 되돌릴 방법이 도구 밖에만 남기 때문이다.
    #[test]
    fn a_broken_line_is_carried_not_a_wall() {
        let (r, d) = repo("broken");
        let path = d.join(".moai/issues.jsonl");
        std::fs::write(&path, "{깨짐\n").unwrap();
        r.with_write(|i, _, _| {
            i.push(issue("argos-4aex"));
            Ok((vec![], ()))
        })
        .expect("깨진 줄 하나가 쓰기를 막았다");

        let after = std::fs::read_to_string(&path).unwrap();
        assert!(after.contains("{깨짐"), "모르는 줄을 잃었다 — {after}");
        assert!(after.contains("argos-4aex"), "쓰겠다고 해 놓고 안 썼다 — {after}");

        // 두 번째 쓰기에서 줄이 또 움직이지 않는다 (멱등).
        let once = std::fs::read_to_string(&path).unwrap();
        r.with_write(|_, _, _| Ok((vec![], ()))).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), once);
    }

    #[test]
    fn refuses_duplicate_ids() {
        let (r, _d) = repo("dup");
        let e = r.with_write(|i, _, _| {
            i.push(issue("argos-4aex"));
            i.push(issue("argos-4aex"));
            Ok((vec![], ()))
        })
        .unwrap_err().message;
        assert!(e.contains("두 번"), "{e}");
    }

    /// 남의 낡은 줄 하나가 모든 쓰기를 막지 않는다.
    ///
    /// config 에서 칸 이름을 고치면 그 칸에 있던 이슈는 더 이상 유효하지
    /// 않다. 그때도 새 이슈는 만들 수 있어야 한다 — 못 만들면 되돌릴 방법이
    /// 도구 밖에만 남는다.
    #[test]
    fn a_stale_row_does_not_block_unrelated_writes() {
        let (r, d) = repo("stale_row");
        let mut old = issue("argos-0001");
        old.status = Status::new("옛날칸");
        std::fs::write(
            d.join(".moai/issues.jsonl"),
            format!("{}\n", serde_json::to_string(&old).unwrap()),
        )
        .unwrap();

        r.with_write(|issues, _, _| {
            issues.push(issue("argos-0002"));
            Ok((vec![], ()))
        })
        .expect("낡은 줄 때문에 새 이슈를 못 넣었다");

        let load = r.read().unwrap();
        assert_eq!(load.issues.len(), 2);
        // 그리고 낡은 줄은 지워지지 않고 그대로 남는다
        assert_eq!(load.get("argos-0001").unwrap().status.as_str(), "옛날칸");
    }

    /// 그 줄을 직접 건드리면 그때는 검사한다.
    #[test]
    fn touching_a_stale_row_still_validates_it() {
        let (r, d) = repo("stale_touch");
        let mut old = issue("argos-0001");
        old.status = Status::new("옛날칸");
        std::fs::write(
            d.join(".moai/issues.jsonl"),
            format!("{}\n", serde_json::to_string(&old).unwrap()),
        )
        .unwrap();

        let e = r
            .with_write(|issues, _, _| {
                issues[0].title = "고친 제목".into();
                Ok((vec![], ()))
            })
            .unwrap_err().message;
        assert!(e.contains("라는 칸이 없다"), "{e}");
    }

    /// 쓰기 도중 실패하면 원본이 그대로다 — 반쯤 쓰인 파일이 남지 않는다.
    #[test]
    fn a_rejected_write_leaves_the_file_untouched() {
        let (r, d) = repo("rollback");
        r.with_write(|i, _, _| {
            i.push(issue("argos-4aex"));
            Ok((vec![], ()))
        })
        .unwrap();
        let before = std::fs::read_to_string(d.join(".moai/issues.jsonl")).unwrap();
        let _ = r.with_write(|i, _, _| {
            i.push(issue("argos-4aey"));
            i[0].status = Status::new("없는칸");
            Ok((vec![], ()))
        });
        assert_eq!(std::fs::read_to_string(d.join(".moai/issues.jsonl")).unwrap(), before);
    }
    /// 못 읽는 줄도 **id 는 내놓는다.** 줄을 `Issue` 로 못 읽는 것과 그 안의
    /// `id` 를 못 읽는 것은 다른 일이다 — 한 단 낮게 읽으면 나온다.
    ///
    /// **둘째 파서를 만들지 않는다.** `serde_json` 한 겹 아래로 내려갈 뿐이라
    /// 본체와 어긋날 자리가 없다. 정규식으로 긁었으면 그 순간 파서가 둘이 된다.
    #[test]
    fn an_unreadable_line_still_yields_its_id() {
        let load = parse_issues(
            "{\"id\":\"argos-9999\",\"title\":\"몰라\",\"kind\":\"몰라\",\"status\":\"todo\"}\n",
        );
        assert_eq!(load.errors.len(), 1);
        assert_eq!(load.errors[0].id.as_deref(), Some("argos-9999"));
        assert_eq!(load.reserved_ids().iter().next().map(String::as_str), Some("argos-9999"));
    }

    /// JSON 도 아닌 줄에는 내놓을 id 가 없다. 없는 것을 지어내지 않는다.
    #[test]
    fn a_line_that_is_not_even_json_yields_no_id() {
        let load = parse_issues("{깨짐\n");
        assert_eq!(load.errors.len(), 1);
        assert_eq!(load.errors[0].id, None);
        assert!(load.reserved_ids().is_empty());
    }

    /// **쓰기는 그 id 를 피해서 뽑는다.** 안 피하면 못 읽는 동안은 아무 데도
    /// 안 보이는 중복이 생기고, 그 줄이 읽히게 되는 날 모든 쓰기가 막힌다.
    #[test]
    fn a_write_reserves_the_ids_it_cannot_read() {
        let (r, d) = repo("reserve");
        std::fs::write(
            d.join(".moai/issues.jsonl"),
            "{\"id\":\"argos-9999\",\"title\":\"몰라\",\"kind\":\"몰라\",\"status\":\"todo\"}\n",
        )
        .unwrap();

        let minted = r
            .with_write(|issues, cfg, reserved| {
                assert!(reserved.contains("argos-9999"), "못 읽는 줄의 id 를 안 줬다 — {reserved:?}");
                // 그 줄의 id 를 그대로 노리는 씨앗이라도 다른 것이 나와야 한다.
                let taken = taken_ids(issues, reserved);
                let id = crate::id::generate(&cfg.prefix, &taken, "argos-9999");
                issues.push(issue(&id));
                Ok((vec![], id))
            })
            .unwrap();
        assert_ne!(minted, "argos-9999", "못 읽는 줄과 같은 id 를 뽑았다");
    }
}
