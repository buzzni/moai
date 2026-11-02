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
