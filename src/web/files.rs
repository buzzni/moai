//! Read-only file and change endpoints for the web panel.
use super::{Response, http, refuse};
use crate::{fail::code, files, highlight, i18n::Lang, report::changes::Checkout, store::Repo, worktree};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::Path,
    time::{Duration, Instant, UNIX_EPOCH},
};

const BUDGET: Duration = Duration::from_secs(10);
/// Kept back from [`BUDGET`] for the work: a walk or Git read cut short must still reach the connection, which stops
/// waiting at the budget itself, so the caller gets the partial answer instead of a timeout.
const MARGIN: Duration = Duration::from_millis(500);
fn bad() -> Response {
    refuse(400, "invalid query", code::BAD_INPUT)
}
fn missing() -> Response {
    refuse(404, "not found", code::NOT_FOUND)
}
fn denied() -> Response {
    refuse(403, "path refused", code::REFUSED)
}
fn ok(out: Value) -> Response {
    Response::new(200, http::JSON, json!({"exit":0,"out":out}).to_string().into_bytes())
}
fn stamp(s: files::Stamp) -> String {
    let t = s.modified.and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_nanos()).unwrap_or(0);
    format!("{t}:{}", s.size)
}
fn path_ok(p: &str) -> bool {
    // A drive prefix is only a prefix on Windows; `a:b` is an ordinary Unix name the listing shows.
    let drive = cfg!(windows) && p.as_bytes().get(1) == Some(&b':');
    !p.starts_with('/') && !drive && !p.contains('\\') && !p.split('/').any(|x| x == "..")
}
fn failure(e: files::Why) -> Response {
    match e {
        files::Why::Outside | files::Why::Git => denied(),
        _ => missing(),
    }
}
fn spans(text: &str, spans: &[highlight::Span]) -> Value {
    Value::Array(
        spans
            .iter()
            .filter_map(|s| {
                Some(json!([
                    text.get(..s.range.start)?.encode_utf16().count(),
                    text.get(..s.range.end)?.encode_utf16().count(),
                    s.kind.name()
                ]))
            })
            .collect(),
    )
}
fn checkout<'a>(repo: &Repo, all: &'a [Checkout], key: Option<&str>) -> Option<&'a Checkout> {
    match key {
        Some(k) => all.iter().find(|c| c.key == k),
        None => all.iter().find(|c| crate::path::canonical(&c.path).ok() == crate::path::canonical(repo.here()).ok()),
    }
}

static ACTIVE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Keep slow filesystem calls off the connection thread and bound abandoned workers.
pub fn answer(path: &str, cwd: &Path, lang: Lang, query: &[(String, String)]) -> Response {
    use std::sync::atomic::Ordering;
    if ACTIVE.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| (n < 32).then_some(n + 1)).is_err() {
        return refuse(503, "file reader busy", code::BUSY);
    }
    let started = Instant::now();
    let path = path.to_owned();
    let cwd = cwd.to_owned();
    let query = query.to_vec();
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    let worker = std::thread::Builder::new().spawn(move || {
        struct Guard;
        impl Drop for Guard {
            fn drop(&mut self) {
                ACTIVE.fetch_sub(1, Ordering::SeqCst);
            }
        }
        let _guard = Guard;
        let _ = tx.send(answer_inner(&path, &cwd, lang, &query, started));
    });
    if worker.is_err() {
        ACTIVE.fetch_sub(1, Ordering::SeqCst);
        return refuse(503, "file reader busy", code::BUSY);
    }
    rx.recv_timeout(BUDGET).unwrap_or_else(|_| refuse(504, "request timed out", code::TIMEOUT))
}
fn answer_inner(path: &str, cwd: &Path, lang: Lang, query: &[(String, String)], started: Instant) -> Response {
    // One budget for the whole request, as the connection counts it: each step gets what the earlier ones left.
    let left = || BUDGET.saturating_sub(MARGIN).saturating_sub(started.elapsed());
    let allowed: &[&str] = match path {
        "/api/checkouts" => &[],
        "/api/files" => &["root", "dir", "limit"],
        "/api/file" => &["root", "path"],
        "/api/files-stamp" => &["root", "dir", "path"],
        "/api/find" => &["root", "dir", "q", "vcs"],
        "/api/ignored" => &["root", "vcs"],
        "/api/changes" => &["root", "id", "scope", "worktree", "vcs"],
        "/api/diff" => &["root", "id", "scope", "path", "worktree", "vcs"],
        _ => return missing(),
    };
    let mut args: HashMap<&str, Vec<&str>> = HashMap::new();
    for (k, v) in query {
        if !allowed.contains(&k.as_str()) || v.is_empty() || v.len() > 512 || v.chars().any(char::is_control) {
            return bad();
        }
        let values = args.entry(k).or_default();
        if !values.is_empty() && !(path == "/api/files-stamp" && k == "dir") {
            return bad();
        }
        values.push(v);
        if values.len() > 64 {
            return bad();
        }
    }
    let get = |k| args.get(k).and_then(|v| v.first().copied());
    let Some(repo) = Repo::find_from(cwd, || lang).ok().flatten() else { return missing() };
    let all = worktree::checkouts(repo.here());
    if path == "/api/checkouts" {
        let load = repo.read().ok();
        let workplaces = worktree::workplaces(repo.here(), true);
        let mut out = json!({"checkouts": all.iter().map(|c| json!({"key":c.key,"branch":c.branch,"main":c.main,"milestone":c.milestone,"issue":load.as_ref().and_then(|l| crate::report::changes::issue_of(&l.issues,c,&workplaces)).map(|i| &i.id),"current":crate::path::canonical(&c.path).ok()==crate::path::canonical(repo.here()).ok()})).collect::<Vec<_>>() });
        // Git and svn both standing here: the one key the file tab draws its git·svn control from (moai-0d0s.qg0). A
        // root under one backend answers as it did, byte for byte.
        let (found, default, _) = crate::changes::chosen(repo.here(), &repo.config, None, None);
        if found.git
            && found.svn
            && let (Some(map), Some(default)) = (out.as_object_mut(), default)
        {
            map.insert("vcs".into(), json!({"found":["git","svn"],"default":default.word()}));
        }
        return ok(out);
    }
    // `vcs=git|svn` — the file tab's pick for a checkout's reads (moai-0d0s.qg0). Someone's text: a known word, of a
    // backend found under that checkout, or the request is refused.
    let vcs = match get("vcs") {
        None => None,
        Some(word) => match crate::vcs::Backend::of_word(word) {
            Some(b) => Some(b),
            None => return bad(),
        },
    };
    if matches!(path, "/api/changes" | "/api/diff") && get("id").is_some() {
        if get("root").is_some() || vcs.is_some() {
            return bad();
        }
        let overlay = match get("worktree") {
            None => false,
            Some("1") => true,
            _ => return bad(),
        };
        return issue_changes(path, &repo, lang, get("id").unwrap(), get("scope"), get("path"), overlay, &left);
    }
    let Some(chosen) = checkout(&repo, &all, get("root")) else { return missing() };
    if vcs.is_some_and(|b| !crate::vcs::detected(&chosen.path).has(b)) {
        return bad();
    }
    let backend = crate::changes::chosen(&chosen.path, &repo.config, None, vcs).1;
    if matches!(path, "/api/changes" | "/api/diff") {
        if get("scope").is_some() || get("worktree").is_some() {
            return bad();
        }
        return checkout_changes(path, chosen, backend, get("path"), lang, &left);
    }
    let Some(jail) = files::Jail::of(chosen) else { return missing() };
    if path == "/api/files-stamp" {
        let dirs = args
            .get("dir")
            .into_iter()
            .flatten()
            .map(|p| (*p, if path_ok(p) { files::stamp(&jail, p).map(stamp) } else { None }))
            .collect::<HashMap<_, _>>();
        let file = get("path").and_then(|p| if path_ok(p) { files::stamp(&jail, p).map(stamp) } else { None });
        return ok(json!({"dirs":dirs,"file":file}));
    }
    let dir = get("dir").unwrap_or(".");
    if !path_ok(dir) {
        return denied();
    }
    match path {
        "/api/files" => {
            let limit = match get("limit") {
                Some(value) => match value.parse::<usize>() {
                    Ok(n) if (1..=5000).contains(&n) => Some(n),
                    _ => return bad(),
                },
                None => None,
            };
            match files::list(&jail, dir) {
                Ok(list) => ok(
                    json!({"dir":dir,"stamp":stamp(list.stamp),"truncated":limit.is_some_and(|n| list.entries.len() > n),"entries":list.entries.iter().take(limit.unwrap_or(usize::MAX)).map(|e| {
                let (kind, link_dir, inside) = match e.kind { files::Kind::Dir => ("dir",false,false), files::Kind::File => ("file",false,false), files::Kind::Link {dir,inside} => ("link",dir,inside), files::Kind::Other => ("other",false,false) };
                json!({"name":e.name,"kind":kind,"size":e.size,"dir":link_dir,"inside":inside,"openable":e.openable})
            }).collect::<Vec<_>>() }),
                ),
                Err(e) => failure(e),
            }
        }
        "/api/file" => {
            let Some(p) = get("path") else { return bad() };
            if !path_ok(p) {
                return denied();
            }
            match files::read(&jail, p) {
                Ok(file) => {
                    let rows: Vec<_> = file.text.split_inclusive('\n').collect();
                    let highlighted = highlight::lines(p, &file.text)
                        .map(|ss| ss.iter().zip(rows).map(|(s, t)| spans(t, s)).collect::<Vec<_>>());
                    ok(
                        json!({"path":p,"size":file.size,"stamp":stamp(file.stamp),"truncated":file.truncated,"text":file.text,"spans":highlighted,"binary":false}),
                    )
                }
                Err(files::Why::NotText) => {
                    let meta = files::stamp(&jail, p);
                    ok(
                        json!({"path":p,"binary":true,"size":meta.as_ref().map(|s| s.size),"truncated":meta.as_ref().is_some_and(|s| s.size > 1024 * 1024),"stamp":meta.map(stamp),"spans":null}),
                    )
                }
                Err(e) => failure(e),
            }
        }
        "/api/ignored" | "/api/find" => {
            let ignored = match backend {
                Some(v) => match v.ignored_dirs(&chosen.path, Some(left())) {
                    Ok(dirs) => Some(dirs),
                    Err(crate::vcs::Error::Timeout) if path == "/api/ignored" => {
                        return refuse(504, "request timed out", code::TIMEOUT);
                    }
                    Err(_) => None,
                },
                None => None,
            };
            if path == "/api/ignored" {
                return ok(
                    json!({"dirs":ignored.clone().unwrap_or_default(),"truncated":false,"vcs_error":if ignored.is_none(){Some(json!({"kind":"unavailable","said":"version control unavailable"}))}else{None}}),
                );
            }
            let Some(q) = get("q") else { return bad() };
            let normalized = dir.trim_start_matches("./").trim_end_matches('/');
            if ignored.as_ref().is_some_and(|dirs| {
                dirs.iter().any(|item| {
                    let item = item.trim_end_matches('/');
                    normalized == item || normalized.strip_prefix(item).is_some_and(|tail| tail.starts_with('/'))
                })
            }) {
                return ok(
                    json!({"matches":[],"total":0,"truncated":false,"walk_truncated":false,"ignored_unavailable":false}),
                );
            }
            let mut found = Vec::new();
            let mut total = 0;
            let walked = files::walk_files(&jail, dir, ignored.as_deref().unwrap_or(&[]), |batch| {
                for p in batch {
                    if let Some(m) = files::matches(q, p) {
                        total += 1;
                        if found.len() < 500 {
                            found.push(json!({"path":p,"start":p[..m.start].encode_utf16().count(),"end":p[..m.end].encode_utf16().count()}));
                        }
                    }
                }
                !left().is_zero()
            });
            match walked {
                Ok(w) => ok(
                    json!({"matches":found,"total":total,"truncated":total>500,"walk_truncated":w.truncated || left().is_zero(),"ignored_unavailable":ignored.is_none()}),
                ),
                Err(e) => failure(e),
            }
        }
        _ => missing(),
    }
}

/// Kept back from the highlighting's share for turning the answer into text and sending it: a 2 MiB patch answers with
/// several megabytes, and the connection stops waiting at [`BUDGET`].
const SEND: Duration = Duration::from_secs(1);
/// Each file row's mark, from the row it was written from — `vcs::Mark`'s fold, which the TUI's own eight letters do
/// not use yet.
fn put_marks<'a>(out: &mut Value, changes: impl IntoIterator<Item = &'a crate::vcs::Change>) {
    if let Some(files) = out.get_mut("files").and_then(Value::as_array_mut) {
        for (entry, change) in files.iter_mut().zip(changes) {
            if let Some(entry) = entry.as_object_mut() {
                entry.insert("mark".into(), json!(crate::vcs::Mark::of(change.status)));
            }
        }
    }
}
/// The answer as a value, every file marked and highlighted until `until`. Highlighting never holds the answer back:
/// past the deadline the files left keep their text and hunks with `spans: null`, and `highlight` says how far it got —
/// `full`, `partial` or `none`, a closed set the page picks its words by.
fn add_highlights(answer: &crate::changes::Answer, until: Instant) -> Value {
    let mut out = serde_json::to_value(answer).unwrap_or(Value::Null);
    put_marks(&mut out, answer.files.iter().map(|f| &f.change));
    let (mut done, mut late) = (0usize, 0usize);
    if let Some(files) = out.get_mut("files").and_then(Value::as_array_mut) {
        for (entry, source) in files.iter_mut().zip(&answer.files) {
            let Some(hunks) = &source.hunks else { continue };
            let file = crate::diff::FileDiff {
                old_path: source.change.old_path.clone().or_else(|| Some(source.change.path.clone())),
                new_path: (source.change.status != crate::vcs::Status::Deleted).then(|| source.change.path.clone()),
                binary: source.change.binary,
                hunks: hunks.clone(),
                ..Default::default()
            };
            // A file with nothing to colour (binary, too large, an unknown language) counts on neither side: a reply
            // whose only finished file was such a one coloured nothing, and says `none`, not `partial`.
            let highlighted = match highlight::diff_until(&file, Some(until)) {
                Ok(h) => {
                    done += usize::from(h.is_some());
                    h
                }
                Err(highlight::Late) => {
                    late += 1;
                    None
                }
            };
            if let Some(rows) = entry.get_mut("hunks").and_then(Value::as_array_mut) {
                for (hi, hunk) in rows.iter_mut().enumerate() {
                    if let Some(lines) = hunk.get_mut("lines").and_then(Value::as_array_mut) {
                        for (li, line) in lines.iter_mut().enumerate() {
                            let value = highlighted
                                .as_ref()
                                .and_then(|h| h.get(hi))
                                .and_then(|h| h.get(li))
                                .map(|s| spans(&hunks[hi].lines[li].text, s))
                                .unwrap_or(Value::Null);
                            line["spans"] = value;
                        }
                    }
                }
            }
        }
    }
    let word = match (done, late) {
        (_, 0) => "full",
        (0, _) => "none",
        _ => "partial",
    };
    if let Some(map) = out.as_object_mut() {
        map.insert("highlight".into(), json!(word));
    }
    out
}
/// When highlighting has to stop: what the request has left, less the time to send the answer.
fn highlight_until(left: &dyn Fn() -> Duration) -> Instant {
    Instant::now() + left().saturating_sub(SEND)
}
fn issue_changes(
    path: &str,
    repo: &Repo,
    lang: Lang,
    id: &str,
    scope: Option<&str>,
    file: Option<&str>,
    overlay: bool,
    left: &dyn Fn() -> Duration,
) -> Response {
    if super::api::issue_id(id).is_err() {
        return bad();
    }
    let scope = match scope {
        None => None,
        Some("commits") => Some(crate::report::changes::Scope::Commits),
        Some("branch") => Some(crate::report::changes::Scope::Branch),
        Some("working") => Some(crate::report::changes::Scope::Working),
        _ => return bad(),
    };
    let Ok(gathered) = worktree::gather(repo, overlay) else { return missing() };
    let Ok(load) = crate::archive::read_all(&repo.root, gathered.load) else { return missing() };
    if load.get(id).is_none() {
        return missing();
    }
    let prepared = crate::changes::offers(repo, &load.issues, id, Some(left()), lang, None);
    match prepared.read(scope, file, path == "/api/changes", Some(left())) {
        Ok(answer) => {
            if answer.vcs_error.as_ref().is_some_and(|e| e.kind == "timeout") {
                return refuse(504, "request timed out", code::TIMEOUT);
            }
            if path == "/api/diff" {
                return ok(add_highlights(&answer, highlight_until(left)));
            }
            // Without a scope the answer is the list of the scope the reading chose — the one `/api/diff` would
            // show — so a page draws every file's head from one request before it asks for any patch.
            let mut out = serde_json::to_value(&answer).unwrap_or(Value::Null);
            put_marks(&mut out, answer.files.iter().map(|f| &f.change));
            if scope.is_none()
                && let Some(map) = out.as_object_mut()
            {
                map.entry("vcs_error").or_insert(Value::Null);
            }
            ok(out)
        }
        Err(_) => missing(),
    }
}
/// A checkout's whole-branch list or patch. An svn failure answers with `changes::told`'s kind and line, as the issue's
/// reads do (moai-0d0s.qg0); Git's answers stay as they were.
fn checkout_changes(
    path: &str,
    checkout: &Checkout,
    backend: Option<crate::vcs::Backend>,
    file: Option<&str>,
    lang: Lang,
    left: &dyn Fn() -> Duration,
) -> Response {
    let svn = |e: crate::vcs::Error| {
        let t = crate::changes::told(&checkout.path, lang, backend, e);
        ok(json!({"scope":"whole","files":[],"total":0,"truncated":false,"vcs_error":{"kind":t.kind,"said":t.said}}))
    };
    let Some(backend) = backend else {
        return ok(
            json!({"scope":"whole","files":[],"total":0,"truncated":false,"vcs_error":{"kind":"unavailable","said":"version control unavailable"}}),
        );
    };
    let changes = match backend.changes(&checkout.path, &crate::vcs::Scope::Whole, Some(left())) {
        Ok(c) => c,
        Err(crate::vcs::Error::Timeout) => return refuse(504, "request timed out", code::TIMEOUT),
        Err(e) if backend == crate::vcs::Backend::Svn => return svn(e),
        Err(_) => {
            return ok(
                json!({"scope":"whole","files":[],"total":0,"truncated":false,"vcs_error":{"kind":"failed","said":"version control unavailable"}}),
            );
        }
    };
    if file.is_some_and(|p| !changes.files.iter().any(|c| c.path == p)) {
        return missing();
    }
    if path == "/api/changes" {
        // The directories' facts take a share of what is left, never the list's own time: past it they stand `M`.
        let marks = backend.marks(&checkout.path, &changes, Some(left() / 2));
        let mut out = json!({"scope":"whole","files":changes.files,"total":changes.total,"truncated":changes.truncated,"dirs":marks.dirs});
        put_marks(&mut out, &changes.files);
        return ok(out);
    }
    // A renamed file is read by both names, the old one from the list: the new name alone reads as a whole new file.
    let names: Option<Vec<String>> = file.map(|p| {
        std::iter::once(p.to_owned())
            .chain(changes.files.iter().filter(|c| c.path == p).filter_map(|c| c.old_path.clone()))
            .collect()
    });
    let whole = crate::vcs::Scope::Whole;
    let read = match &names {
        Some(names) => backend.patch_of(&checkout.path, &whole, names, Some(left()), Some(&changes)),
        None => backend.patch_with_changes(&checkout.path, &whole, None, Some(left()), &changes),
    };
    let patch = match read {
        Ok(p) => p,
        Err(crate::vcs::Error::Timeout) => return refuse(504, "request timed out", code::TIMEOUT),
        Err(e) if backend == crate::vcs::Backend::Svn => return svn(e),
        Err(_) => return missing(),
    };
    let parsed = crate::diff::parse(&patch.text);
    let entries = changes
        .files
        .into_iter()
        .filter(|c| file.is_none_or(|p| p == c.path))
        .map(|change| {
            let hunks = parsed
                .iter()
                .filter(|f| {
                    f.new_path.as_deref() == Some(&change.path)
                        || (f.new_path.is_none() && f.old_path.as_deref() == Some(&change.path))
                })
                .flat_map(|f| f.hunks.iter().cloned())
                .collect();
            crate::changes::File { change, hunks: Some(hunks) }
        })
        .collect();
    let answer = crate::changes::Answer {
        id: String::new(),
        scope: None,
        offers: vec![],
        files: entries,
        total: changes.total,
        truncated: changes.truncated || patch.truncated,
        vcs_error: None,
        patch: String::new(),
        notice: None,
        warning: None,
    };
    let mut out = add_highlights(&answer, highlight_until(left));
    out["scope"] = json!("whole");
    ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn highlighting_offsets_are_utf16() {
        let line = "😀한a";
        let parts = vec![highlight::Span { kind: highlight::Kind::Keyword, range: 4..7 }];
        assert_eq!(spans(line, &parts), json!([[2, 3, "keyword"]]));
    }

    fn body(tag: &str) -> String {
        (0..20).map(|n| format!("let {tag}{n} = {n};\n")).collect()
    }
    fn tracker(name: &str) -> crate::scratch::Scratch {
        let root = crate::scratch::Scratch::new(name);
        std::fs::create_dir(root.join(".moai")).unwrap();
        std::fs::write(root.join(".moai/config.toml"), "prefix = \"web\"\n").unwrap();
        std::fs::write(root.join(".moai/issues.jsonl"), "").unwrap();
        root
    }
    fn get(root: &Path, path: &str, query: &[(&str, &str)]) -> (u16, Value) {
        let query: Vec<_> = query.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
        let r = answer(path, root, Lang::En, &query);
        (r.status, serde_json::from_slice(&r.body).unwrap_or(Value::Null))
    }
    #[test]
    fn checkout_answers_carry_marks_for_files_and_directories() {
        let root = tracker("web-marks");
        let git = |args: &[&str]| crate::git::tests::run_git(&root, None, args);
        git(&["init", "-q"]);
        std::fs::create_dir_all(root.join("grow")).unwrap();
        std::fs::write(root.join("grow/a.rs"), body("a")).unwrap();
        std::fs::write(root.join("old.rs"), body("o")).unwrap();
        git(&["add", "."]);
        git(&["commit", "-qm", "base"]);
        std::fs::write(root.join("grow/b.rs"), "let b = 1;\n").unwrap();
        std::fs::create_dir_all(root.join("fresh/inner")).unwrap();
        std::fs::write(root.join("fresh/inner/c.rs"), "let c = 1;\n").unwrap();
        git(&["mv", "old.rs", "renamed.rs"]);
        std::fs::write(root.join("renamed.rs"), body("o").replacen("o3 = 3", "o3 = 33", 1)).unwrap();
        let (status, list) = get(&root, "/api/changes", &[]);
        assert_eq!(status, 200, "{list}");
        let out = &list["out"];
        let mark = |p: &str| out["files"].as_array().unwrap().iter().find(|f| f["path"] == p).unwrap()["mark"].clone();
        assert_eq!(mark("grow/b.rs"), "A");
        assert_eq!(mark("renamed.rs"), "R");
        assert_eq!(mark("fresh/inner/c.rs"), "A");
        assert_eq!(out["dirs"], json!({"fresh":"A","fresh/inner":"A","grow":"M"}));
        assert!(
            out["files"].as_array().unwrap().iter().all(|f| matches!(f["mark"].as_str(), Some("M" | "A" | "D" | "R")))
        );
        // The same tree reads the same text: the page compares answers as text.
        assert_eq!(get(&root, "/api/changes", &[]).1.to_string(), list.to_string());
        // One renamed file's diff carries its mark and only the lines it changed.
        let (status, diff) = get(&root, "/api/diff", &[("path", "renamed.rs")]);
        assert_eq!(status, 200, "{diff}");
        let file = &diff["out"]["files"][0];
        assert_eq!(file["mark"], "R");
        assert_eq!(diff["out"]["highlight"], "full");
        let lines: Vec<_> =
            file["hunks"].as_array().unwrap().iter().flat_map(|h| h["lines"].as_array().unwrap()).collect();
        let adds = lines.iter().filter(|l| l["kind"] == "add").count();
        assert_eq!(json!(adds), file["adds"], "{file}");
        assert!(adds < 5, "the renamed file read as a whole new one");
        // The gate stays as narrow: only a listed path, by its new name.
        for refused in ["old.rs", "grow", "../x", "/etc/passwd"] {
            assert_eq!(get(&root, "/api/diff", &[("path", refused)]).0, 404, "{refused}");
        }
    }
    #[test]
    fn highlighting_stops_at_its_deadline_and_says_how_far_it_got() {
        let patch = |name: &str, n: usize| {
            let mut p = format!("diff --git a/{name} b/{name}\n--- a/{name}\n+++ b/{name}\n@@ -0,0 +1,{n} @@\n");
            p.extend((0..n).map(|i| format!("+let v{i} = {i};\n")));
            p
        };
        let file = |name: &str, n: usize| {
            let hunks = crate::diff::parse(&patch(name, n)).remove(0).hunks;
            crate::changes::File {
                change: crate::vcs::Change {
                    path: name.into(),
                    old_path: None,
                    status: crate::vcs::Status::Untracked,
                    adds: Some(n),
                    dels: Some(0),
                    binary: false,
                },
                hunks: Some(hunks),
            }
        };
        let answer = |files| crate::changes::Answer {
            id: String::new(),
            scope: None,
            offers: vec![],
            files,
            total: 2,
            truncated: false,
            vcs_error: None,
            patch: String::new(),
            notice: None,
            warning: None,
        };
        let line = |v: &Value, f: usize| v["files"][f]["hunks"][0]["lines"][0].clone();
        let far = Instant::now() + Duration::from_secs(60);
        let full = add_highlights(&answer(vec![file("a.rs", 3), file("b.rs", 3)]), far);
        assert_eq!(full["highlight"], "full");
        assert!(line(&full, 1)["spans"].is_array());
        assert_eq!(full["files"][0]["mark"], "A");
        // No time left: every file still has its text and hunks, none has spans.
        let none = add_highlights(&answer(vec![file("a.rs", 3), file("b.rs", 3)]), Instant::now());
        assert_eq!(none["highlight"], "none");
        assert_eq!(line(&none, 1)["text"], "let v0 = 0;");
        assert!(line(&none, 0)["spans"].is_null() && line(&none, 1)["spans"].is_null());
        // A file with nothing to colour finishes nothing: the one after it cut, the answer coloured none.
        let blind = add_highlights(&answer(vec![file("a.veryunlikely", 3), file("b.rs", 3)]), Instant::now());
        assert_eq!(blind["highlight"], "none");
        // A highlighter slower than the budget (20 ms a line, 1,000 lines) is cut at the deadline, not waited out.
        let started = Instant::now();
        let cut = add_highlights(
            &answer(vec![file("a.rs", 3), file("slow-highlight.rs", 1000)]),
            started + Duration::from_millis(200),
        );
        assert!(started.elapsed() < Duration::from_secs(2), "{:?}", started.elapsed());
        assert_eq!(cut["highlight"], "partial");
        assert!(line(&cut, 0)["spans"].is_array());
        assert!(line(&cut, 1)["spans"].is_null());
        assert_eq!(cut["files"][1]["hunks"][0]["lines"].as_array().unwrap().len(), 1000);
    }
}
