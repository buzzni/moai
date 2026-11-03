//! Pictures cross storage, worktree redirection, markdown and terminal output.
use std::{
    path::Path,
    process::{Command, Output},
};
// The shared harness exposes helpers used by other integration suites.
#[allow(dead_code)]
#[path = "../src/scratch.rs"]
mod scratch;
const BIN: &str = env!("CARGO_BIN_EXE_moai");
fn command(root: &Path) -> Command {
    std::fs::create_dir_all(root.join("test-home")).unwrap();
    let mut c = Command::new(BIN);
    c.env("HOME", root.join("test-home"))
        .env("USERPROFILE", root.join("test-home"))
        .env("XDG_CONFIG_HOME", root.join("test-home/.config"));
    c.current_dir(root)
        .env("MOAI_CONFIG", root.join("user.toml"))
        .env("MOAI_CTL_DIR", root.join("ctl"))
        .env("MOAI_ACTOR", "Image Tester (image@example.test)")
        .env("MOAI_LANG", "en")
        .env("MOAI_NO_UPDATE_CHECK", "1")
        .env("GIT_CONFIG_GLOBAL", root.join("no-gitconfig"))
        .env("GIT_CONFIG_SYSTEM", root.join("no-system-gitconfig"))
        .env_remove("MOAI_HERE")
        .env_remove("MOAI_NOW")
        .env_remove("MOAI_KEYS");
    c
}
fn run(root: &Path, args: &[&str]) -> Output {
    command(root).args(args).output().unwrap()
}
fn ok(root: &Path, args: &[&str]) -> String {
    let o = run(root, args);
    assert!(o.status.success(), "{args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8(o.stdout).unwrap()
}
fn init(name: &str) -> scratch::Scratch {
    let s = scratch::Scratch::new(name);
    ok(&s, &["init", "pic", "--no-skill", "--no-driver", "--no-register"]);
    s
}
fn json(s: &str) -> serde_json::Value {
    serde_json::from_str(s).unwrap()
}
fn picture(root: &Path, w: u32, h: u32) -> std::path::PathBuf {
    let p = root.join("shot.wrong");
    let i = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(w, h, image::Rgba([30, 80, 150, 128])));
    i.save_with_format(&p, image::ImageFormat::Png).unwrap();
    p
}
fn add(root: &Path, p: &Path, extra: &[&str]) -> serde_json::Value {
    let mut args = vec!["image", "add", p.to_str().unwrap(), "--json"];
    args.extend(extra);
    json(&ok(root, &args))
}
#[test]
fn image_add_stores_a_shrunk_copy_reuses_and_keeps_original() {
    let s = init("image-add");
    let p = picture(&s, 3000, 1000);
    let a = add(&s, &p, &[]);
    assert_eq!(a[0]["width"], 1920);
    assert_eq!(a[0]["height"], 640);
    assert_eq!(a[0]["reused"], false);
    let path = std::path::PathBuf::from(a[0]["path"].as_str().unwrap());
    assert!(path.starts_with(s.path()));
    assert!(path.extension().unwrap() == "png");
    assert_eq!(a[0]["name"].as_str().unwrap().len(), 20);
    assert_eq!(add(&s, &p, &[])[0]["reused"], true);
    let original = add(&s, &p, &["--original"]);
    assert_eq!(std::fs::read(original[0]["path"].as_str().unwrap()).unwrap(), std::fs::read(p).unwrap());
}
#[test]
fn image_add_to_appends_and_preserves_unknown_fields_and_other_rows() {
    let s = init("image-body");
    let p = picture(&s, 4, 4);
    let id = json(&ok(&s, &["add", "picture", "-b", "existing body", "--json"]))["id"].as_str().unwrap().to_string();
    let other = json(&ok(&s, &["add", "other", "--json"]));
    let before = std::fs::read_to_string(s.join(".moai/issues.jsonl")).unwrap();
    let line = before.lines().find(|l| l.contains(other["id"].as_str().unwrap())).unwrap().to_string();
    let a = add(&s, &p, &["--to", &id, "--alt", "look [here]\nnow"]);
    let shown = json(&ok(&s, &["show", &id, "--json"]));
    assert_eq!(shown["body"], format!("existing body\n\n{}", a[0]["markdown"].as_str().unwrap()));
    assert!(std::fs::read_to_string(s.join(".moai/issues.jsonl")).unwrap().lines().any(|l| l == line));
    assert!(!a[0]["markdown"].as_str().unwrap().contains("[here]"));
}
#[test]
fn concurrent_image_appends_keep_every_reference() {
    let s = init("image-concurrent");
    let p = picture(&s, 3, 3);
    let id = json(&ok(&s, &["add", "concurrent", "--json"]))["id"].as_str().unwrap().to_string();
    std::thread::scope(|scope| {
        let tasks: Vec<_> = (0..6)
            .map(|n| {
                let (root, p, id) = (s.path(), &p, &id);
                scope.spawn(move || {
                    add(root, p, &["--to", id, "--alt", &format!("shot {n}")]);
                })
            })
            .collect();
        for t in tasks {
            t.join().unwrap();
        }
    });
    let body = json(&ok(&s, &["show", &id, "--json"]))["body"].as_str().unwrap().to_string();
    for n in 0..6 {
        assert!(body.contains(&format!("![shot {n}]")));
    }
}
#[test]
fn image_add_validates_every_input_before_writing() {
    let s = init("image-invalid");
    let p = picture(&s, 3, 3);
    let bad = s.join("bad.txt");
    std::fs::write(&bad, b"not a picture").unwrap();
    let o = run(&s, &["image", "add", p.to_str().unwrap(), bad.to_str().unwrap()]);
    assert!(!o.status.success());
    assert!(!s.join(".moai/images").exists());
}
#[test]
fn image_compress_off_and_warnings_do_not_change_exit_status() {
    let s = init("image-config");
    let p = picture(&s, 300, 100);
    std::fs::write(s.join(".moai/config.toml"), "prefix = \"pic\"\nimage_compress = \"off\"\nimage_warn_kb = 0\n")
        .unwrap();
    let o = run(&s, &["image", "add", p.to_str().unwrap(), "--json"]);
    assert!(o.status.success());
    let a = json(std::str::from_utf8(&o.stdout).unwrap());
    assert_eq!(a[0]["compressed"], false);
    assert_eq!(std::fs::read(a[0]["path"].as_str().unwrap()).unwrap(), std::fs::read(p).unwrap());
    assert!(!o.stderr.is_empty());
    assert_eq!(a[0]["warnings"].as_array().unwrap().len(), 1);
    std::fs::write(s.join(".moai/config.toml"), "prefix = \"pic\"\nimage_compress = \"typo\"\n").unwrap();
    assert!(run(&s, &["status"]).status.success());
    assert!(!run(&s, &["image", "add", s.join("shot.wrong").to_str().unwrap()]).status.success());
}
#[test]
fn image_add_wiki_links_from_nested_pages_and_pipe_never_draws() {
    let s = init("image-wiki");
    std::fs::create_dir_all(s.join("docs/lessons")).unwrap();
    let p = picture(&s, 4, 4);
    let a = add(&s, &p, &["--wiki", "lessons/x"]);
    let md = a[0]["markdown"].as_str().unwrap();
    assert!(md.contains("../images/"));
    assert!(Path::new(a[0]["path"].as_str().unwrap()).starts_with(s.join("docs/images")));
    std::fs::write(s.join("docs/lessons/x.md"), format!("# Picture\n\n{md}\n\n## After\n")).unwrap();
    let o = command(&s).env("MOAI_IMAGES", "kitty").args(["wiki", "show", "lessons/x"]).output().unwrap();
    assert!(o.status.success());
    assert!(!o.stdout.contains(&27));
    assert!(String::from_utf8(o.stdout).unwrap().contains("[image: shot] ../images/"));
    let shown = json(&ok(&s, &["wiki", "show", "lessons/x", "--json"]));
    assert!(shown["body"].as_str().unwrap().contains(md));
}
#[test]
fn show_has_localized_missing_placeholders_but_raw_and_json_stay_markdown() {
    let s = init("image-show");
    let body = "![gone](.moai/images/0123456789abcdef.png)";
    let id = json(&ok(&s, &["add", "missing", "-b", body, "--json"]))["id"].as_str().unwrap().to_string();
    let o = command(&s).env("MOAI_IMAGES", "kitty").args(["show", &id]).output().unwrap();
    assert!(!o.stdout.contains(&27));
    assert!(String::from_utf8(o.stdout).unwrap().contains("[image missing: gone]"));
    let o = command(&s).env("MOAI_LANG", "ko").args(["show", &id]).output().unwrap();
    assert!(String::from_utf8(o.stdout).unwrap().contains("[그림 없음: gone]"));
    assert!(ok(&s, &["show", &id, "--raw"]).contains(body));
    assert_eq!(json(&ok(&s, &["show", &id, "--json"]))["body"], body);
}
#[test]
fn image_add_without_an_actor_can_store_but_cannot_attach() {
    let s = init("image-actor");
    let p = picture(&s, 3, 3);
    let id = json(&ok(&s, &["add", "actor", "--json"]))["id"].as_str().unwrap().to_string();
    let o =
        command(&s).env_remove("MOAI_ACTOR").args(["image", "add", p.to_str().unwrap(), "--to", &id]).output().unwrap();
    assert!(!o.status.success());
    assert!(!s.join(".moai/images").exists());
    let o = command(&s).env_remove("MOAI_ACTOR").args(["image", "add", p.to_str().unwrap()]).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}
#[test]
fn image_add_in_a_worktree_writes_tracker_in_root_and_wiki_in_checkout() {
    let s = init("image-worktree");
    let p = picture(&s, 3, 3);
    let git = |args: &[&str]| {
        let o = Command::new("git").current_dir(s.path()).args(args).output().unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    };
    git(&["init"]);
    git(&["add", "."]);
    git(&["-c", "user.name=Tester", "-c", "user.email=t@example.test", "commit", "-m", "base"]);
    let w = s.join("work");
    git(&["worktree", "add", "-b", "picture", w.to_str().unwrap()]);
    let a = add(&w, &p, &[]);
    assert!(Path::new(a[0]["path"].as_str().unwrap()).starts_with(s.join(".moai/images")));
    assert!(!w.join(".moai/images").exists());
    std::fs::create_dir(w.join("docs")).unwrap();
    let a = add(&w, &p, &["--wiki"]);
    assert!(Path::new(a[0]["path"].as_str().unwrap()).starts_with(w.join("docs/images")));
}

#[test]
fn image_add_wiki_creates_a_missing_wiki_directory() {
    let s = init("image-new-wiki");
    let p = picture(&s, 3, 3);
    let a = add(&s, &p, &["--wiki"]);
    assert!(Path::new(a[0]["path"].as_str().unwrap()).starts_with(s.join("docs/images")));
}

#[test]
fn image_add_to_a_missing_issue_stores_nothing() {
    let s = init("image-no-issue");
    let p = picture(&s, 3, 3);
    let o = run(&s, &["image", "add", p.to_str().unwrap(), "--to", "pic-zzzz"]);
    assert!(!o.status.success());
    assert!(!s.join(".moai/images").exists());
}

#[test]
fn a_long_alt_still_gets_its_localized_placeholder_in_show() {
    let s = init("image-long-alt");
    // A macOS screenshot name: label and path together pass the 76-column body width.
    let body = "![Screenshot 2026-11-02 at 10.23.45 AM](.moai/images/0123456789abcdef.png)";
    let id = json(&ok(&s, &["add", "long", "-b", body, "--json"]))["id"].as_str().unwrap().to_string();
    let o = command(&s).env("MOAI_LANG", "ko").args(["show", &id]).output().unwrap();
    let out = String::from_utf8(o.stdout).unwrap();
    assert!(
        out.contains("[그림 없음: Screenshot 2026-11-02 at 10.23.45 AM] .moai/images/0123456789abcdef.png"),
        "{out}"
    );
}

/// A real stdout pty exercises the shared tty check without reaching the person's tmux server.
#[cfg(unix)]
#[test]
fn inherited_tmux_uses_stdout_device_and_skips_queries_without_pictures() {
    use std::{
        fs::File,
        io::Read,
        os::{fd::FromRawFd, unix::fs::PermissionsExt},
    };
    let s = init("image-tmux-tty");
    let plain = json(&ok(&s, &["add", "plain", "-b", "no pictures", "--json"]))["id"].as_str().unwrap().to_owned();
    let id = json(&ok(&s, &["add", "picture", "--json"]))["id"].as_str().unwrap().to_owned();
    let p = picture(&s, 4, 4);
    add(&s, &p, &["--to", &id]);
    let bin = s.join("bin");
    std::fs::create_dir(&bin).unwrap();
    let tmux = bin.join("tmux");
    std::fs::write(
        &tmux,
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$TEST_QUERY_LOG\"\nprintf '%s\\nMOAI_CLIENT\\n%s\\n' \"$TEST_PANE_TTY\" \"$TEST_CLIENT\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&tmux, std::fs::Permissions::from_mode(0o755)).unwrap();
    let log = s.join("queries");
    // `panes` answers what the fake server lists, given the pty this run writes to.
    let run_pty = |id: &str, panes: &dyn Fn(&str) -> String, forced: Option<&str>, client: Option<&str>| {
        let (mut master, mut slave) = (-1, -1);
        // `c_char` is `u8` on aarch64 Linux, and Apple's `openpty` takes `*mut` termios/winsize.
        let mut name: [libc::c_char; 256] = [0; 256];
        // SAFETY: openpty writes valid descriptors and a tty name to the supplied buffers.
        assert_eq!(
            unsafe {
                libc::openpty(&mut master, &mut slave, name.as_mut_ptr(), std::ptr::null_mut(), std::ptr::null_mut())
            },
            0
        );
        // Tests in this binary spawn children in parallel; a child that inherited the slave would hold off the
        // master's end of file until it exits.
        for fd in [master, slave] {
            // SAFETY: fd is one of the descriptors openpty just returned.
            unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) };
        }
        // SAFETY: successful openpty transferred ownership of both fresh descriptors.
        let (mut master, slave) = unsafe { (File::from_raw_fd(master), File::from_raw_fd(slave)) };
        // SAFETY: openpty supplied a NUL-terminated pathname in name.
        let tty = unsafe { std::ffi::CStr::from_ptr(name.as_ptr()) }.to_str().unwrap();
        let mut cmd = command(&s);
        cmd.args(["show", id])
            .env("TMUX", "/tmp/moai-blkz-only-test,1,0")
            .env("TERM", "xterm-kitty")
            .env("KITTY_WINDOW_ID", "5")
            .env("TERM_PROGRAM", "tmux")
            .env("PATH", &bin)
            .env("TEST_QUERY_LOG", &log)
            .env("TEST_PANE_TTY", panes(tty))
            .env("TEST_CLIENT", client.unwrap_or(""))
            .env_remove("MOAI_IMAGES")
            .stdout(slave);
        if client.is_some() {
            // Inside tmux, choose graphics from its attached client, without inherited kitty hints.
            cmd.env("TERM", "tmux-256color").env_remove("KITTY_WINDOW_ID");
        }
        if let Some(mode) = forced {
            cmd.env("MOAI_IMAGES", mode);
        }
        let status = cmd.status().unwrap();
        assert!(status.success());
        drop(cmd);
        let mut bytes = Vec::new();
        let mut chunk = [0; 4096];
        loop {
            match master.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => bytes.extend_from_slice(&chunk[..n]),
            }
        }
        String::from_utf8(bytes).unwrap()
    };
    let other = |_: &str| "/dev/null".to_owned();
    let own = |tty: &str| tty.to_owned();
    assert!(!run_pty(&plain, &other, None, None).contains("\x1b_G"));
    assert!(!log.exists(), "picture-free body queried tmux");
    assert!(run_pty(&id, &other, None, None).contains("\x1b_Ga=T"));
    assert_eq!(std::fs::read_to_string(&log).unwrap().matches("list-panes").count(), 1);
    assert!(std::fs::read_to_string(&log).unwrap().contains("; display-message"));
    assert!(
        std::fs::read_to_string(&log).unwrap().contains("-S /tmp/moai-blkz-only-test list-panes -a -F #{pane_tty}")
    );
    // A dead pane (remain-on-exit) lists a tty that no longer stats: it cannot be our stdout, so it decides nothing.
    let dead = "/nonexistent/moai-blkz-dead-pane";
    assert!(
        run_pty(&id, &|_| format!("{dead}\n/dev/null"), None, None).contains("\x1b_Ga=T"),
        "a dead pane turned graphics off"
    );
    // Only when no pane could be checked at all is the answer unknown, and unknown keeps graphics off.
    assert!(!run_pty(&id, &|_| dead.to_owned(), None, None).contains("\x1b_G"));
    let text = run_pty(&id, &own, None, None);
    assert!(!text.contains("\x1b_G"));
    assert!(text.contains("\x1b]8;;file://"));
    for terminal in ["xterm-kitty", "xterm-ghostty"] {
        for permission in ["on", "all"] {
            let client = format!("{terminal}\n{permission}");
            let graphics = run_pty(&id, &own, None, Some(&client));
            assert!(graphics.contains("\x1bPtmux;\x1b\x1b_Ga=T,U=1"), "{client:?}");
            assert!(graphics.contains('\u{10eeee}'), "{client:?}");
        }
    }
    for client in ["xterm-kitty\noff", "xterm-ghostty\noff", "xterm-256color\non"] {
        let fallback = run_pty(&id, &own, None, Some(client));
        assert!(!fallback.contains("\x1b_G"), "{client:?}");
        assert!(!fallback.contains("\x1bPtmux;"), "{client:?}");
        assert!(fallback.contains("\x1b]8;;file://"), "{client:?}");
    }
    std::fs::remove_file(&log).unwrap();
    assert!(run_pty(&id, &own, Some("kitty"), None).contains("\x1b_Ga=T"));
    assert!(!log.exists(), "explicit graphics queried tmux");
}
