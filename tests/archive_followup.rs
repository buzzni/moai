use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

struct Repo(PathBuf);
impl Repo {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "moai-followup-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        let repo = Self(path);
        assert!(repo.run(&["init", "argos"]).status.success());
        std::fs::create_dir_all(repo.0.join(".moai/archive")).unwrap();
        repo
    }
    fn command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_moai"));
        cmd.args(args)
            .current_dir(&self.0)
            .env("MOAI_ACTOR", "Tester (tester@example.com)")
            .env("MOAI_CONFIG", self.0.join("config.toml"))
            .env("MOAI_NOW", "2026-09-11T04:12:03Z")
            .env("NO_COLOR", "1")
            .env("MOAI_NO_UPDATE_CHECK", "1");
        cmd
    }
    fn run(&self, args: &[&str]) -> Output {
        self.command(args).output().unwrap()
    }
    fn write(&self, path: &str, data: &str) {
        std::fs::write(self.0.join(path), data).unwrap();
    }
}
impl Drop for Repo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn row(id: &str, kind: &str, status: &str) -> serde_json::Value {
    serde_json::json!({"id":id,"title":id,"kind":kind,"status":status,
        "created_at":"2026-09-01T00:00:00Z","updated_at":"2026-09-01T00:00:00Z","status_since":"2026-09-01T00:00:00Z"})
}
fn text(out: Output) -> String {
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}
#[test]
fn ordinary_edit_reads_archived_members_of_live_blocker() {
    let repo = Repo::new("edit");
    let blocker = row("argos-bbbb", "epic", "todo");
    let mut issue = row("argos-xxxx", "issue", "todo");
    issue["blocked_by"] = serde_json::json!(["argos-bbbb"]);
    repo.write(".moai/issues.jsonl", &format!("{blocker}\n{issue}\n"));
    let member = row("argos-bbbb.aaa", "issue", "done");
    repo.write(".moai/archive/2026.jsonl", &format!("{member}\n"));
    let shown = text(repo.run(&["edit", "argos-xxxx", "--title", "renamed"]));
    let line = shown.lines().find(|l| l.contains("argos-bbbb")).unwrap_or_default();
    assert!(line.contains('✓') && !line.contains("no members"), "{shown}");
}
#[test]
fn rule_five_ignores_an_opaque_live_rows_archived_owner() {
    let repo = Repo::new("hook");
    let mut issue = row("argos-aaaa", "future-kind", "todo");
    issue["assignee"] = "Other".into();
    issue["assignee_email"] = "other@example.com".into();
    repo.write(".moai/issues.jsonl", &format!("{issue}\n"));
    issue["kind"] = "issue".into();
    repo.write(".moai/archive/2026.jsonl", &format!("{issue}\n"));
    let event = serde_json::json!({"session_id":"opaque-owner", "cwd":Path::new(&repo.0), "tool_name":"Bash", "tool_input":{"command":"moai mv argos-aaaa in_progress"}});
    let mut child =
        repo.command(&["hook", "pre-tool-use"]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(event.to_string().as_bytes()).unwrap();
    let result = text(child.wait_with_output().unwrap());
    assert!(!result.contains("Rule 5"), "{result}");
}

#[test]
fn edit_and_show_agree_on_blocker_members_in_each_live_state() {
    for state in ["done", "reopened", "deferred", "opaque"] {
        let repo = Repo::new(state);
        let blocker = row("argos-bbbb", "epic", "todo");
        let mut issue = row("argos-xxxx", "issue", "todo");
        issue["blocked_by"] = serde_json::json!(["argos-bbbb"]);
        let mut live = format!("{blocker}\n{issue}\n");
        let archived = row("argos-bbbb.aaa", "issue", "done");
        match state {
            "reopened" => live.push_str(&format!("{}\n", row("argos-bbbb.aaa", "issue", "in_progress"))),
            "deferred" => {
                let mut member = row("argos-bbbb.aaa", "issue", "todo");
                member["deferred_at"] = "2026-09-10T00:00:00Z".into();
                live.push_str(&format!("{member}\n"));
            }
            "opaque" => live.push_str(&format!("{}\n", row("argos-bbbb.aaa", "future-kind", "todo"))),
            _ => (),
        }
        repo.write(".moai/issues.jsonl", &live);
        repo.write(".moai/archive/2026.jsonl", &format!("{archived}\n"));
        if state == "opaque" {
            let edited = text(repo.run(&["edit", "argos-xxxx", "--title", "renamed"]));
            let blocker = edited.lines().find(|l| l.contains("argos-bbbb")).unwrap_or_default();
            assert!(!blocker.contains('✓'), "opaque live member must suppress its archived copy: {edited}");
            continue;
        }
        let shown = text(repo.run(&["show", "argos-xxxx"]));
        let edited = text(repo.run(&["edit", "argos-xxxx", "--title", "renamed"]));
        let line = |out: &str| out.lines().find(|l| l.contains("argos-bbbb")).unwrap_or_default().to_owned();
        assert!(!line(&shown).is_empty(), "{state}: {shown}");
        assert_eq!(line(&shown), line(&edited), "{state}: show={shown}; edit={edited}");
        if state == "done" {
            assert!(line(&shown).contains('✓'), "{shown}");
        }
        if state == "reopened" {
            assert!(!line(&shown).contains('✓'), "{shown}");
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
fn due_rejudges_after_waiting_for_the_write_lock() {
    use fs2::FileExt;
    use std::os::unix::fs::MetadataExt;
    use std::time::{Duration, Instant};

    let repo = Repo::new("due-lock");
    let mut issue = row("argos-aaaa", "issue", "todo");
    issue["due_on"] = "2026-09-11".into();
    repo.write(".moai/issues.jsonl", &format!("{issue}\n"));
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(repo.0.join(".moai/lock"))
        .unwrap();
    lock.lock_exclusive().unwrap();
    let mut child = repo.command(&["due", "apply"]).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let held = lock.metadata().unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        let opened = std::fs::read_dir(format!("/proc/{}/fd", child.id()))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| std::fs::metadata(entry.path()).ok())
            .any(|file| file.dev() == held.dev() && file.ino() == held.ino());
        if opened {
            break;
        }
        assert!(child.try_wait().unwrap().is_none(), "due exited before reaching the write lock");
        assert!(Instant::now() < until, "due never reached the write lock");
        std::thread::sleep(Duration::from_millis(10));
    }
    issue["due_on"] = "2026-12-01".into();
    let snapshot = format!("{issue}\n");
    repo.write(".moai/issues.jsonl", &snapshot);
    FileExt::unlock(&lock).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(std::fs::read(repo.0.join(".moai/issues.jsonl")).unwrap(), snapshot.as_bytes());
    let journal = repo.0.join(".moai/journal");
    assert!(
        !journal.exists() || std::fs::read_dir(journal).unwrap().next().is_none(),
        "a stale preview wrote a journal entry"
    );
}
