//! 프로젝트 등록·해제 — 층의 `a`·`d`(moai-plvy).
//!
//! 조각이 아니다 — 디렉터리를 읽고([`list_dir`]) 사용자 설정을 쓴다. 창의 상태와 키는
//! 조각([`super::picker`])이 들고, 여기는 그 창이 시킨 것([`Act`])을 디스크에 한다.
//!
//! **쓰는 길은 CLI 와 하나다**(`projects::add`·`projects::remove`). 설정 자리도 새로 찾지
//! 않는다 — 층이 읽은 파일(`Layer::config`)이나, 층이 없으면 띄울 때 받은 자리
//! (`App::user_config`)다. 환경을 여기서 다시 읽으면 시험이 돌리는 사람의 설정을 쓴다.

use super::keys::{BROWSE, Browse, CONFIRM, Confirm, Lookup, label, lookup};
use super::picker::{Act, Dent, Listing, Picker};
use super::{App, Mode, Row};
use ratatui::crossterm::event::KeyEvent;
use std::path::{Path, PathBuf};

/// 한 층에 세우는 하위 디렉터리의 상한. 넘으면 이름순 앞만 세우고 창이 그 밖의 수를 댄다 —
/// `node_modules` 같은 곳에 잘못 들어가도 줄마다 `.moai` 를 재느라 화면이 멈추지 않게.
/// 거기 있는 것을 고르는 길은 `g p`(경로 적기)다.
pub const SHOWN_MAX: usize = 2000;

/// `d` 로 해제를 묻는 중. **정체는 경로다** — 묻는 동안 층이 다시 읽혀 줄 차례가 바뀌어도
/// 사람이 본 그 프로젝트를 뺀다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unregister {
    pub path: PathBuf,
    pub name: String,
}

/// 디렉터리 한 층을 읽는다. **재귀로 훑지 않는다.**
///
/// - 링크를 푼다 — 창이 보이는 경로가 곧 `resolve_dir` 이 적을 철자다
/// - 링크도 따라가 디렉터리면 세운다(모노레포를 링크로 건 사람이 있다)
/// - 이름이 UTF-8 이 아닌 것은 세우지 않는다 — 사용자 설정(TOML)에 적을 수 없어 골라도
///   거절된다. 고를 수 없는 줄을 세우면 고른 뒤에야 안 된다는 것을 안다
/// - 점 디렉터리는 `show_hidden` 이 아니면 감추고 그 수만 센다
/// - 하나씩 못 읽는 항목은 건너뛴다. 디렉터리 자체를 못 읽으면 `Err` 한 줄
///
/// `registered` 는 등록한 경로와 그 푼 철자들이다([`registered_paths`]).
pub fn list_dir(dir: &Path, registered: &[PathBuf], show_hidden: bool) -> Result<Listing, String> {
    let dir = std::fs::canonicalize(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    if !dir.is_dir() {
        return Err(format!("디렉터리가 아니다 — {}", dir.display()));
    }
    let read = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut found: Vec<(String, bool)> = Vec::new();
    let mut hidden = 0;
    for entry in read.flatten() {
        let Ok(name) = entry.file_name().into_string() else { continue };
        let (is_dir, link) = match entry.file_type() {
            Ok(t) if t.is_dir() => (true, false),
            // 링크는 따라가 봐야 디렉터리인지 안다 — 링크인 것만 `stat` 한다.
            Ok(t) if t.is_symlink() => (entry.path().is_dir(), true),
            _ => (false, false),
        };
        if !is_dir {
            continue;
        }
        if name.starts_with('.') && !show_hidden {
            hidden += 1;
            continue;
        }
        found.push((name, link));
    }
    found.sort();
    let cut = found.len().saturating_sub(SHOWN_MAX);
    found.truncate(SHOWN_MAX);
    let marked = |p: &Path| registered.iter().any(|r| r == p);
    let entries = found
        .into_iter()
        .map(|(name, link)| {
            let path = dir.join(&name);
            // 링크면 등록은 푼 경로로 적혔다. 링크가 아니면 `dir` 이 이미 풀렸으니 그대로다.
            let real = if link { std::fs::canonicalize(&path).ok() } else { None };
            Dent {
                moai: path.join(".moai").is_dir(),
                registered: marked(&path) || real.as_deref().is_some_and(marked),
                name,
            }
        })
        .collect();
    Ok(Listing { moai: dir.join(".moai").is_dir(), registered: marked(&dir), dir, entries, cut, hidden })
}

/// 등록한 경로와, 손으로 적은 링크 철자면 그 푼 경로까지. 등록은 몇 개뿐이라 한 번씩 푼다.
pub fn registered_paths(config: &Path) -> Vec<PathBuf> {
    let reg = crate::user_config::read(Some(config));
    let mut out = Vec::new();
    for p in reg.projects {
        if let Ok(real) = std::fs::canonicalize(&p.path)
            && real != p.path
        {
            out.push(real);
        }
        out.push(p.path);
    }
    out
}

fn shown(path: &Path) -> String {
    crate::text::one_line(&path.display().to_string())
}

/// `~`·`~/…` 를 홈으로 푼다. **여기서 푸는 까닭**은 창([`super::picker`])이 조각이라
/// 환경을 안 보기 때문이다 — 창은 `~` 로 시작하는 철자를 붙이지 않고 그대로 넘긴다.
/// 홈을 모르면 준 철자 그대로 두고, 없는 디렉터리라는 말이 그대로 선다.
fn expand_home(p: &Path) -> PathBuf {
    let Ok(rest) = p.strip_prefix("~") else { return p.to_path_buf() };
    match std::env::var_os("HOME").filter(|h| !h.is_empty()) {
        Some(home) => PathBuf::from(home).join(rest),
        None => p.to_path_buf(),
    }
}

impl App {
    /// 쓸 사용자 설정 파일. 층이 있으면 층이 읽은 그 파일이다 — 둘이 갈리면 등록한 것이 층에 안 선다.
    fn config_file(&self) -> Option<PathBuf> {
        match &self.layer {
            Some(l) => l.config.clone(),
            None => self.user_config.clone(),
        }
    }

    /// `a` — 디렉터리 고르기 창을 연다.
    ///
    /// 시작 자리는 **마지막으로 창에서 본 디렉터리**, 처음이면 **띄운 자리**(`App::launched_at`),
    /// 그것도 모르면 지금 프로젝트의 뿌리다. 띄운 자리인 까닭: CLI 의 `moai project add .` 과
    /// 상대경로가 거기 붙고, 모노레포 안에서 띄운 사람이 `apps/a` 를 고르려고 홈부터 파고들
    /// 까닭이 없다. 앞의 자리가 그새 사라졌으면 다음 자리로 넘어간다.
    pub(super) fn open_picker(&mut self) {
        let Some(config) = self.config_file() else {
            self.notice = Some("! 사용자 설정의 자리를 모른다 — MOAI_CONFIG·XDG_CONFIG_HOME·HOME 중 하나를 준다".into());
            return;
        };
        let starts: Vec<PathBuf> = [self.pick_from.clone(), self.launched_at.clone(), self.repo.as_ref().map(|r| r.root.clone())]
            .into_iter()
            .flatten()
            .collect();
        let registered = registered_paths(&config);
        let mut why = "어디서 고를지 모른다".to_string();
        for start in starts {
            match list_dir(&start, &registered, false) {
                Ok(at) => {
                    self.mode = Mode::Pick(Picker::new(at));
                    return;
                }
                Err(e) => why = e,
            }
        }
        self.notice = Some(format!("! 고르기 창을 못 열었다 — {}", crate::text::one_line(&why)));
    }

    /// 창이 열린 동안의 키. **무엇을 할지는 창이 정하고**([`Picker::key`]) 여기는 그대로 한다.
    /// Ctrl-C 는 여기 오기 전에 [`App::key`] 가 받았다.
    pub(super) fn pick(&mut self, k: KeyEvent) {
        let Mode::Pick(p) = &mut self.mode else { return };
        match p.key(k) {
            Act::Stay => {}
            Act::Close => {
                self.pick_from = Some(p.at.dir.clone());
                self.mode = Mode::Browse;
            }
            Act::Go(to) => self.relist(&to, None),
            Act::Hidden(show) => {
                let here = p.at.dir.clone();
                self.relist(&here, Some(show));
            }
            Act::Register(dir) => self.register(&dir),
        }
    }

    /// 창에 그 디렉터리 한 층을 읽어 넣는다. 못 읽으면 **그 자리에 선 채** 까닭 한 줄.
    ///
    /// `hidden` 이 있으면 점 디렉터리 보이기를 그 값으로 읽고, **읽혔을 때만 목록과 함께 넣는다**
    /// (moai-v2jf, 사용자와 정함). 못 읽으면 설정도 목록도 그대로다 — 먼저 넣으면 창은 "보이기
    /// 켬" 인데 화면은 감춘 목록이라, `.` 을 한 번 더 눌러야 풀리던 어긋남이 생긴다.
    fn relist(&mut self, to: &Path, hidden: Option<bool>) {
        let to = expand_home(to);
        let registered = self.config_file().map(|c| registered_paths(&c)).unwrap_or_default();
        let Mode::Pick(p) = &mut self.mode else { return };
        let show = hidden.unwrap_or(p.show_hidden);
        match list_dir(&to, &registered, show) {
            Ok(at) => {
                p.show_hidden = show;
                p.show(at);
            }
            Err(e) => p.error = Some(format!("못 연다 — {}", crate::text::one_line(&e))),
        }
    }

    /// 창의 `a`. **CLI `moai project add` 와 같은 함수를 부른다**(`projects::add`).
    ///
    /// 되면 창은 **열린 채** 그 줄에 등록 표시가 서고, 층은 그 자리에서 다시 선다 — 층에
    /// 섰으면 커서가 새 프로젝트에 가 있어 Esc 로 닫으면 거기 서 있다. 모노레포 하위를 여럿
    /// 등록하는 사람이 하나마다 창을 다시 열지 않는다. 이미 있으면 그렇다고만 한다(멱등).
    fn register(&mut self, dir: &Path) {
        let Some(config) = self.config_file() else { return };
        match crate::projects::add(&config, dir, dir) {
            Ok(added) => {
                self.relayer(Some(&added.path));
                let what = if added.added { "✓ 등록함" } else { "이미 등록돼 있다" };
                // 못 읽는 저장소면 CLI `project add` 처럼 그렇다고 댄다 — 조용히 "등록함" 만 서면
                // 층에서 "못 읽는다" 를 처음 만난다 (moai-9omq).
                let bad = added.unreadable.as_deref().map(|e| format!(" · ! 못 읽는다 — {}", crate::text::one_line(e))).unwrap_or_default();
                let bare = if added.initialized { "" } else { " · init 전 — .moai 가 아직 없다" };
                // **층으로 가는 키를 대는 자리다** — 뿌리의 `..` 을 걷은 뒤(moai-i784) Bksp 는
                // 디렉터리만 올라간다. 옛 글대로 Bksp 를 대면 방금 등록한 프로젝트를 보러 가는
                // 바로 그 화면이 아무 일도 안 하는 키를 대고, 없는 키를 적어 두면 그것부터
                // 도구를 못 믿게 된다(키 바와 같은 까닭).
                let back = if self.on_layer() || self.layer.is_none() {
                    String::new()
                } else {
                    format!(" · {} 로 층에 올라가면 보인다", label(BROWSE, Browse::Project(0)))
                };
                self.notice = Some(format!("{what} · {}{bad}{bare}{back}", shown(&added.path)));
                if let Mode::Pick(p) = &self.mode {
                    let here = p.at.dir.clone();
                    self.relist(&here, None);
                }
            }
            // **창에 선 채 말한다.** 깨진 설정이면 `user_config::update` 가 한 글자도 안 쓰고
            // 거절했다 — 층의 배너가 그 설정 문제를 이미 비추고 있다.
            Err(e) => {
                if let Mode::Pick(p) = &mut self.mode {
                    p.error = Some(format!("등록하지 못했다 — {}", crate::text::one_line(&e.message)));
                }
            }
        }
    }

    /// `d` — 커서가 선 층의 줄을 목록에서 뺄지 묻는다. **띄운 자리(등록 안 됨)는 묻지 않는다** —
    /// 뺄 것이 없다.
    pub(super) fn ask_unregister(&mut self) {
        let Some(Row::Project(at)) = self.current() else { return };
        let Some(place) = self.layer.as_ref().and_then(|l| l.places.get(at)) else { return };
        if !place.registered {
            self.notice = Some("등록돼 있지 않다 — 여기서 띄워 층에 섰을 뿐이라 뺄 것이 없다".into());
            return;
        }
        self.mode = Mode::Unregister(Unregister { path: place.path.clone(), name: place.name.clone() });
    }

    /// 해제를 묻는 동안의 키. **`y` 만 뺀다** — 폼의 "버릴까" 와 같다. 다른 키는 그만두고 글자로도
    /// 이동으로도 쓰지 않는다: 물음을 못 보고 누른 `↓` 가 커서를 옮기면 무엇을 그만뒀는지 헷갈린다.
    pub(super) fn settle_unregister(&mut self, k: KeyEvent) {
        let Mode::Unregister(u) = std::mem::replace(&mut self.mode, Mode::Browse) else { return };
        if let Lookup::Run(Confirm::Yes) = lookup(CONFIRM, &[k]) {
            self.unregister(&u.path);
        }
    }

    /// 목록에서만 뺀다 — **CLI `moai project rm` 과 같은 함수다**(`projects::remove`). 그
    /// 디렉터리와 `.moai` 는 건드리지 않는다. 뺀 뒤 층을 다시 세우고 커서는 그 자리에 둔다.
    fn unregister(&mut self, path: &Path) {
        let Some(config) = self.config_file() else {
            self.notice = Some("! 사용자 설정의 자리를 모른다 — 뺄 곳이 없다".into());
            return;
        };
        match crate::projects::remove(&config, path, path) {
            Ok(r) => {
                self.relayer(None);
                self.notice = Some(if r.removed.is_empty() {
                    // 그새 밖에서 뺐다. 층은 방금 다시 읽어 그 줄이 사라졌다.
                    format!("이미 목록에 없다 · {}", shown(path))
                } else {
                    // 뺀 것을 **다** 댄다 — 손으로 링크 철자와 푼 철자를 둘 다 적었으면 둘이 함께
                    // 빠진다(CLI `rm` 도 뺀 줄마다 적는다). 하나만 대면 사라진 다른 줄을 모른다.
                    let gone: Vec<String> = r.removed.iter().map(|p| shown(p)).collect();
                    format!("✓ 뺌 · {} — 목록에서만 뺐다, 디렉터리와 .moai 는 그대로다", gone.join(", "))
                });
            }
            Err(e) => self.notice = Some(format!("! 빼지 못했다 — {}", crate::text::one_line(&e.message))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};
    use crate::nav::Path as NavPath;
    use crate::store::Repo;
    use crate::tui::layer::{At, Layer, Look, Shut};
    use crate::tui::stamp_of;

    /// 진짜 디렉터리와 사용자 설정 한 벌. **돌리는 사람의 홈·설정은 안 읽는다** — 창은
    /// `App::launched_at` 에서, 쓰기는 층이 읽은 임시 설정 파일에 한다.
    ///
    /// 이 묶음만 쓰는 손놀림. 자리를 만들고 지우는 일은 [`Scratch`] 가 한다.
    trait Places {
        fn dir(&self, rel: &str) -> PathBuf;
        fn project(&self, rel: &str) -> PathBuf;
        fn config(&self) -> PathBuf;
        fn register(&self, dirs: &[&Path]) -> PathBuf;
        fn registered(&self) -> Vec<PathBuf>;
    }

    impl Places for Scratch {
        fn dir(&self, rel: &str) -> PathBuf {
            let d = self.join(rel);
            std::fs::create_dir_all(&d).unwrap();
            d
        }

        fn project(&self, rel: &str) -> PathBuf {
            let d = self.dir(rel);
            std::fs::create_dir_all(d.join(".moai")).unwrap();
            std::fs::write(d.join(".moai/config.toml"), "prefix = \"argos\"\n").unwrap();
            std::fs::write(d.join(".moai/issues.jsonl"), "").unwrap();
            d
        }

        fn config(&self) -> PathBuf {
            self.join("user/config.toml")
        }

        fn register(&self, dirs: &[&Path]) -> PathBuf {
            let path = self.config();
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let body: String = dirs.iter().map(|d| format!("[[project]]\npath = {:?}\n", d.to_str().unwrap())).collect();
            std::fs::write(&path, body).unwrap();
            path
        }

        fn registered(&self) -> Vec<PathBuf> {
            crate::user_config::read(Some(&self.config())).projects.into_iter().map(|p| p.path).collect()
        }
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn press(a: &mut App, codes: &[KeyCode]) {
        for c in codes {
            a.key(key(*c));
        }
    }

    fn picker(a: &App) -> &Picker {
        match &a.mode {
            Mode::Pick(p) => p,
            other => panic!("고르기 창이 안 열렸다 — {other:?}"),
        }
    }

    /// 커서를 창의 그 이름 줄에 둔다.
    fn point(a: &mut App, name: &str) {
        let Mode::Pick(p) = &mut a.mode else { panic!("창이 없다") };
        let at = p.at.entries.iter().position(|d| d.name == name).unwrap_or_else(|| panic!("{name} 가 창에 없다 — {:?}", p.at));
        p.cursor = p.rows().iter().position(|r| *r == crate::tui::picker::Row::Dir(at)).unwrap();
    }

    fn place_at_cursor(a: &App) -> PathBuf {
        match a.current() {
            Some(Row::Project(at)) => a.layer.as_ref().unwrap().places[at].path.clone(),
            other => panic!("커서가 층의 줄에 안 섰다 — {other:?}"),
        }
    }

    /// 층에서 연 탐색기 — 등록 하나(`argos`), 창은 `work` 에서 연다.
    fn on_layer(s: &Scratch) -> App {
        let argos = s.project("work/argos");
        let cfg = s.register(&[&argos]);
        let mut a = App::on_projects(Layer::read(Some(&cfg), None));
        a.launched_at = Some(s.join("work"));
        a
    }

    /// **창은 한 층씩 읽고 표시를 낱말로 단다** — `.moai` 가 있는 것, 이미 등록한 것(링크로
    /// 가리켜도), 감춘 점 디렉터리의 수. 파일은 줄로 안 선다.
    ///
    /// 링크를 만드는 시험이라 unix 에서만 돈다 — 가리지 않으면 unix 가 아닌 곳에서 이 한
    /// 시험이 아니라 **바이너리의 시험 전부**가 컴파일되지 않는다(`user_config` 의 링크 시험과 같다).
    #[cfg(unix)]
    #[test]
    fn the_picker_marks_moai_and_registered_and_hides_dot_directories() {
        let s = Scratch::real("marks");
        let mut a = on_layer(&s);
        s.dir("work/mono/apps/a");
        s.dir("work/.cache");
        std::fs::write(s.join("work/README"), "").unwrap();
        std::os::unix::fs::symlink(s.join("work/argos"), s.join("work/link")).unwrap();

        a.hit("SPC p a");
        let p = picker(&a);
        assert_eq!(p.at.dir, s.join("work"));
        let names: Vec<&str> = p.at.entries.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["argos", "link", "mono"], "파일이 섰거나 점 디렉터리가 안 감춰졌다");
        assert_eq!(p.at.hidden, 1);
        let argos = &p.at.entries[0];
        assert!(argos.moai && argos.registered, "{argos:?}");
        assert!(p.at.entries[1].registered, "등록한 디렉터리를 가리키는 링크에 표시가 없다");
        let mono = &p.at.entries[2];
        assert!(!mono.moai && !mono.registered, "{mono:?}");
        assert_eq!(s.registered().len(), 1, "여는 것만으로 설정을 고쳤다");

        // 점 디렉터리 보이기
        a.key(key(KeyCode::Char('.')));
        assert!(picker(&a).at.entries.iter().any(|d| d.name == ".cache"));
        assert_eq!(picker(&a).at.hidden, 0);
    }

    /// **열어 둔 디렉터리를 못 읽게 된 뒤 `.` 을 눌러도 보이기 설정과 목록이 안 어긋난다**
    /// (moai-v2jf, 사용자와 정함). 다시 읽기에 실패하면 설정도 목록도 그대로 두고 까닭 한 줄만
    /// 선다 — 한때 설정만 먼저 뒤집혀 화면은 점 디렉터리를 감춘 목록인데 창은 "보이기 켬" 이었다.
    /// 다시 읽을 수 있게 되면 `.` 이 그때 설정과 목록을 함께 바꾼다.
    #[cfg(unix)]
    #[test]
    fn dot_in_an_unreadable_directory_keeps_the_setting_and_the_list_together() {
        use std::os::unix::fs::PermissionsExt;
        let s = Scratch::real("dotlocked");
        let mut a = on_layer(&s);
        s.dir("work/.cache");
        let work = s.join("work");

        a.hit("SPC p a");
        let before = picker(&a).at.clone();
        assert!(!picker(&a).show_hidden);
        assert_eq!(before.hidden, 1, "{before:?}");

        std::fs::set_permissions(&work, std::fs::Permissions::from_mode(0o000)).unwrap();
        // root 는 권한 000 도 읽는다 — 못 읽게 만들 수 없으면 이 시험은 볼 것이 없다.
        if std::fs::read_dir(&work).is_ok() {
            std::fs::set_permissions(&work, std::fs::Permissions::from_mode(0o755)).unwrap();
            return;
        }
        a.key(key(KeyCode::Char('.')));
        let locked = (picker(&a).show_hidden, picker(&a).at.clone(), picker(&a).error.clone());
        std::fs::set_permissions(&work, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(!locked.0, "못 읽었는데 보이기 설정이 뒤집혔다 — 목록은 옛것이다");
        assert_eq!(locked.1, before, "못 읽었는데 목록이 바뀌었다");
        assert!(locked.2.as_deref().is_some_and(|e| e.contains("못 연다")), "까닭을 안 댔다 — {:?}", locked.2);

        // 다시 읽을 수 있으면 `.` 이 설정과 목록을 함께 바꾼다.
        a.key(key(KeyCode::Char('.')));
        assert!(picker(&a).show_hidden);
        assert!(picker(&a).at.entries.iter().any(|d| d.name == ".cache"), "{:?}", picker(&a).at);
        assert_eq!(picker(&a).at.hidden, 0);
        assert_eq!(picker(&a).error, None);
    }

    /// **디렉터리를 드나들어 모노레포 하위를 등록한다** — `.git` 에서 멈추지 않고 고른 그
    /// 디렉터리가 푼 경로로 적힌다. 창은 열린 채 표시가 서고, 층에 새 줄이 서고 커서가 거기
    /// 있다. `.moai` 없는 것도 받아 층에 "init 전" 으로 선다. 다시 등록하면 멱등이다.
    #[test]
    fn browsing_into_a_monorepo_registers_its_subdirectories_one_by_one() {
        let s = Scratch::real("mono");
        let mut a = on_layer(&s);
        s.dir("work/mono/.git");
        let app_a = s.project("work/mono/apps/a");
        let app_b = s.dir("work/mono/apps/b");

        a.hit("SPC p a");
        point(&mut a, "mono");
        press(&mut a, &[KeyCode::Enter]);
        assert_eq!(picker(&a).at.dir, s.join("work/mono"));
        assert_eq!(picker(&a).at.hidden, 1, ".git 이 줄로 섰다");
        press(&mut a, &[KeyCode::Enter]);
        assert_eq!(picker(&a).at.dir, s.join("work/mono/apps"));
        point(&mut a, "a");
        a.key(key(KeyCode::Char('a')));

        assert_eq!(s.registered(), [s.join("work/argos"), app_a.clone()]);
        assert!(a.notice.as_deref().is_some_and(|n| n.contains("✓ 등록함")), "{:?}", a.notice);
        let p = picker(&a);
        assert!(p.at.entries.iter().find(|d| d.name == "a").is_some_and(|d| d.registered && d.moai), "창의 표시가 안 고쳐졌다");
        assert_eq!(p.error, None);
        assert_eq!(place_at_cursor(&a), app_a, "층의 커서가 새 프로젝트에 안 섰다");
        // 새 줄은 스레드가 읽는다(moai-ezwu) — 등록하는 키가 저장소 읽기를 기다리지 않는다.
        crate::tui::settle_reads(&mut a);
        assert!(matches!(a.layer.as_ref().unwrap().places[1].look, Look::Open { .. }), "새 줄을 안 읽었다");

        // 창을 닫으면 그 줄에 서 있다. 다시 열면 마지막 디렉터리에서 연다.
        press(&mut a, &[KeyCode::Esc]);
        assert_eq!(a.mode, Mode::Browse);
        assert_eq!(place_at_cursor(&a), app_a);
        a.hit("SPC p a");
        assert_eq!(picker(&a).at.dir, s.join("work/mono/apps"), "마지막으로 본 디렉터리에서 안 열었다");

        // `.moai` 없는 디렉터리 — 받고, init 전이라 말하고, 층에 그렇게 선다.
        point(&mut a, "b");
        a.key(key(KeyCode::Char('a')));
        assert!(a.notice.as_deref().is_some_and(|n| n.contains("init 전")), "{:?}", a.notice);
        press(&mut a, &[KeyCode::Esc]);
        assert_eq!(place_at_cursor(&a), app_b);
        crate::tui::settle_reads(&mut a);
        let lines = crate::tui::draw::tests::render(&mut a, 100, 16);
        assert!(lines.iter().any(|l| l.contains("> b ") && l.contains("init 전")), "{}", lines.join("\n"));

        // 다시 등록해도 한 줄이다.
        a.hit("SPC p a");
        point(&mut a, "a");
        let before = std::fs::read(s.config()).unwrap();
        a.key(key(KeyCode::Char('a')));
        assert!(a.notice.as_deref().is_some_and(|n| n.contains("이미 등록돼 있다")), "{:?}", a.notice);
        assert_eq!(std::fs::read(s.config()).unwrap(), before, "멱등이 아니다 — 설정을 다시 썼다");

        // 지금 디렉터리(`./`)도 고를 수 있다 — 모노레포 뿌리.
        press(&mut a, &[KeyCode::Backspace, KeyCode::Home]);
        assert_eq!(picker(&a).at.dir, s.join("work/mono"));
        a.key(key(KeyCode::Char('a')));
        assert_eq!(s.registered().last(), Some(&s.join("work/mono")));
    }

    /// **해제는 한 번 묻고 `y` 만 뺀다.** 목록에서만 빼고 디렉터리와 `.moai` 는 그대로다.
    /// 다른 키는 그만두고 아무것도 안 쓴다.
    #[test]
    fn unregistering_asks_once_and_only_removes_the_entry() {
        let s = Scratch::real("unregister");
        let one = s.project("work/one");
        let two = s.dir("work/two");
        let cfg = s.register(&[&one, &two]);
        let mut a = App::on_projects(Layer::read(Some(&cfg), None));
        press(&mut a, &[KeyCode::Down]);
        assert_eq!(place_at_cursor(&a), two);

        let before = std::fs::read(&cfg).unwrap();
        a.hit("SPC p d");
        assert!(matches!(&a.mode, Mode::Unregister(u) if u.path == two), "{:?}", a.mode);
        a.key(key(KeyCode::Char('n')));
        assert_eq!(a.mode, Mode::Browse);
        assert_eq!(std::fs::read(&cfg).unwrap(), before, "그만뒀는데 설정을 고쳤다");
        a.hit("SPC p d");
        a.key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL));
        assert_eq!(std::fs::read(&cfg).unwrap(), before, "Ctrl-Y 로 뺐다");

        // 옮긴 키(moai-7sjm) — 바로 누르던 `d`·Delete 는 아무것도 안 묻는다.
        for k in [KeyCode::Char('d'), KeyCode::Delete] {
            a.key(key(k));
            assert_eq!(a.mode, Mode::Browse, "{k:?} 가 해제를 물었다");
        }
        a.hit("SPC p d");
        a.key(key(KeyCode::Char('y')));
        assert_eq!(s.registered(), [one.clone()]);
        assert!(two.is_dir(), "디렉터리를 지웠다");
        assert!(one.join(".moai/config.toml").is_file());
        assert_eq!(a.layer.as_ref().unwrap().places.iter().map(|p| p.path.clone()).collect::<Vec<_>>(), [one.clone()]);
        assert_eq!(a.current(), Some(Row::Project(0)), "뺀 뒤 커서가 줄 밖에 섰다");
        assert!(a.notice.as_deref().is_some_and(|n| n.contains("✓ 뺌") && n.contains("그대로")), "{:?}", a.notice);

        // 상세에 포커스가 있으면 `SPC p d` 는 메뉴에 안 서고 아무것도 안 뺀다.
        a.key(key(KeyCode::Tab));
        a.hit("SPC p d");
        assert_eq!(a.mode, Mode::Browse);
        assert!(super::super::menu::open(&a.chord), "안 선 `d` 가 메뉴를 닫았다");
    }

    /// **손으로 적은 철자(`a/../b`)도 층의 `d` 로 빠진다.** 글자 정리·링크 풀기가 그 철자를
    /// 못 만들어, 줄은 남은 채 "이미 목록에 없다" 고 말하던 자리다.
    #[test]
    fn a_hand_written_spelling_with_dot_dot_is_removed_by_d() {
        let s = Scratch::real("dotdot");
        s.dir("work/a");
        let b = s.project("work/b");
        let odd = s.join("work/a/../b");
        let cfg = s.register(&[&odd]);
        let mut a = App::on_projects(Layer::read(Some(&cfg), None));
        assert_eq!(place_at_cursor(&a), odd);
        a.hit("SPC p d");
        a.key(key(KeyCode::Char('y')));
        assert!(s.registered().is_empty(), "손으로 적은 철자를 못 뺐다 — {:?}", a.notice);
        assert!(a.notice.as_deref().is_some_and(|n| n.contains("✓ 뺌")), "{:?}", a.notice);
        assert!(b.is_dir());
    }

    /// 띄운 자리(등록 안 됨) 줄의 `d` 는 묻지 않고 뺄 것이 없다고만 한다.
    #[test]
    fn the_launched_but_unregistered_row_has_nothing_to_unregister() {
        let s = Scratch::real("launched");
        let one = s.project("work/one");
        let here = s.project("work/here");
        let cfg = s.register(&[&one]);
        let mut layer = Layer::read(Some(&cfg), Some(&here));
        layer.at = At::Layer;
        let mut a = App::on_projects(layer);
        assert_eq!(place_at_cursor(&a), here);
        let before = std::fs::read(&cfg).unwrap();
        a.hit("SPC p d");
        assert_eq!(a.mode, Mode::Browse);
        assert!(a.notice.as_deref().is_some_and(|n| n.contains("등록돼 있지 않다")), "{:?}", a.notice);
        assert_eq!(std::fs::read(&cfg).unwrap(), before);
    }

    /// **깨진 설정이면 한 글자도 안 쓰고 창에 선 채 까닭 한 줄.** 창도 층도 그대로 돈다.
    #[test]
    fn a_broken_config_refuses_the_write_and_says_so_in_one_line() {
        let s = Scratch::real("broken");
        let cfg = s.config();
        std::fs::create_dir_all(cfg.parent().unwrap()).unwrap();
        let broken = "[[project]\npath = \"/a\"\n";
        std::fs::write(&cfg, broken).unwrap();
        let mut a = App::on_projects(Layer::read(Some(&cfg), None));
        a.launched_at = Some(s.dir("work"));
        s.dir("work/x");

        a.hit("SPC p a");
        a.key(key(KeyCode::Char('a')));
        let p = picker(&a);
        let e = p.error.as_deref().unwrap_or_else(|| panic!("까닭이 없다 — {:?}", a.notice));
        assert!(e.contains("등록하지 못했다") && e.contains("쓰지 않는다") && !e.contains('\n'), "{e}");
        assert_eq!(std::fs::read_to_string(&cfg).unwrap(), broken, "깨진 설정을 덮어썼다");
        let bar = crate::tui::draw::tests::render(&mut a, 80, 12).last().cloned().unwrap_or_default();
        assert!(bar.contains("등록하지 못했다"), "{bar:?}");
        press(&mut a, &[KeyCode::Down]);
        assert_eq!(picker(&a).error, None);
        press(&mut a, &[KeyCode::Esc]);
        assert!(a.on_layer());

        // 설정 자리를 모르면 창을 안 연다.
        let mut none = App::on_projects(Layer::read(None, None));
        none.launched_at = Some(s.path().to_path_buf());
        none.hit("SPC p a");
        assert_eq!(none.mode, Mode::Browse);
        assert!(none.notice.as_deref().is_some_and(|n| n.contains("자리를 모른다")), "{:?}", none.notice);
    }

    /// **등록이 0 인 채 `.moai` 밖에서 띄우면 빈 층이 서고 `SPC p a` 를 댄다**(moai-r8kl, 사용자와
    /// 정함). 빈 화면이 실수를 성공으로 읽히지 않게 무엇을 할지 한 줄로 말하고, 그 키로 첫 등록을
    /// 하면 그 자리에서 층에 줄이 선다.
    #[test]
    fn with_nothing_registered_the_empty_layer_says_how_and_a_registers_the_first() {
        let s = Scratch::real("empty");
        let cfg = s.register(&[]);
        let argos = s.project("work/argos");
        let mut a = App::on_projects(Layer::read(Some(&cfg), None));
        a.launched_at = Some(s.join("work"));
        assert!(a.on_layer() && a.repo.is_none());

        let screen = crate::tui::draw::tests::render(&mut a, 80, 12).join("\n");
        assert!(screen.contains("등록한 프로젝트가 없다") && screen.contains("SPC p a"), "{screen}");

        a.hit("SPC p a");
        point(&mut a, "argos");
        a.key(key(KeyCode::Char('a')));
        assert_eq!(s.registered(), [argos.clone()]);
        press(&mut a, &[KeyCode::Esc]);
        assert!(a.on_layer());
        assert_eq!(place_at_cursor(&a), argos);
        let screen = crate::tui::draw::tests::render(&mut a, 80, 12).join("\n");
        assert!(!screen.contains("등록한 프로젝트가 없다"), "등록했는데 안내가 남았다 — {screen}");
    }

    /// **등록이 0 인 채 `.moai` 안에서 띄워도 `a` 로 첫 등록을 한다.** 층이 그 자리에서 서되
    /// 지금 프로젝트는 그대로다 — 뿌리에 `..` 이 새로 서도 커서는 보던 줄에 선다. 그 뒤
    /// Bksp 로 올라가면 띄운 자리와 새 프로젝트가 층에 선다.
    #[test]
    fn without_a_layer_the_first_registration_raises_one_and_stays_in_the_project() {
        use crate::model::{Issue, Kind, Status};
        let s = Scratch::real("bootstrap");
        let here = s.project("work/here");
        let lines: String = [("argos-0001", "첫 줄"), ("argos-0002", "둘째 줄")]
            .iter()
            .map(|(id, t)| {
                let i = Issue::new((*id).into(), (*t).into(), Kind::Issue, Status::new("todo"), "2026-09-01T00:00:00Z");
                format!("{}\n", serde_json::to_string(&i).unwrap())
            })
            .collect();
        std::fs::write(here.join(".moai/issues.jsonl"), lines).unwrap();
        let other = s.dir("work/other");
        let repo = Repo { root: here.clone(), config: crate::config::Config::parse("prefix = \"argos\"\n").unwrap() };
        let stamp = stamp_of(&repo);
        let load = repo.read().unwrap();
        let (index, ground) = crate::tui::measure(&load.issues, &repo.config);
        let mut a = App::open(repo, load, index, ground, NavPath::new(), stamp);
        a.user_config = Some(s.config());
        a.launched_at = Some(here.clone());
        assert!(a.layer.is_none());
        press(&mut a, &[KeyCode::Down]);
        let held = a.current();
        assert_eq!(a.cursor, 1);

        a.hit("SPC p a");
        assert_eq!(picker(&a).at.dir, here, "띄운 자리에서 안 열었다");
        press(&mut a, &[KeyCode::Backspace]);
        point(&mut a, "other");
        a.key(key(KeyCode::Char('a')));
        // **알림이 대는 키는 층으로 가는 키다**(리뷰) — 뿌리의 `..` 을 걷은 뒤(moai-i784) Bksp
        // 는 디렉터리만 올라간다. 방금 등록한 것을 보러 갈 화면이 안 듣는 키를 대면 안 된다.
        let said = a.notice.clone().unwrap_or_default();
        assert!(said.contains("0 로 층에 올라가면"), "층으로 가는 키를 안 댔다 — {said:?}");
        assert!(!said.contains("Bksp"), "걷어 낸 키를 아직 댄다 — {said:?}");
        press(&mut a, &[KeyCode::Esc]);

        assert_eq!(s.registered(), [other.clone()]);
        let layer = a.layer.as_ref().expect("등록했는데 층이 안 섰다");
        assert_eq!(layer.at, At::Project(here.clone()), "등록하다 프로젝트에서 튕겨 나왔다");
        assert_eq!(a.repo.as_ref().map(|r| r.root.clone()), Some(here.clone()));
        // 층이 서도 뿌리에 `..` 은 없다 — 층으로는 `0` 이 간다(moai-i784).
        assert!(!a.rows().contains(&Row::Up), "뿌리에 `..` 이 섰다");
        assert_eq!(a.current(), held, "층이 서면서 커서가 옆 줄로 밀렸다");

        press(&mut a, &[KeyCode::Home]);
        a.hit("0");
        assert!(a.on_layer());
        // 올라오면 남의 줄은 스레드가 읽는다(moai-ezwu) — 끝날 때까지 받는다.
        crate::tui::settle_reads(&mut a);
        let places: Vec<(PathBuf, bool)> =
            a.layer.as_ref().unwrap().places.iter().map(|p| (p.path.clone(), p.registered)).collect();
        assert_eq!(places, [(here, false), (other, true)]);
        assert!(matches!(a.layer.as_ref().unwrap().places[1].look, Look::Shut { state: Shut::Uninit, .. }));
    }

    /// 한 층에 너무 많으면 앞만 세우고 그 밖의 수를 댄다. 없는 디렉터리는 한 줄 까닭이다.
    #[test]
    fn a_huge_directory_is_cut_and_says_how_much() {
        let s = Scratch::real("huge");
        for i in 0..SHOWN_MAX + 3 {
            std::fs::create_dir(s.join(format!("d{i:05}"))).unwrap();
        }
        let l = list_dir(s.path(), &[], false).unwrap();
        assert_eq!((l.entries.len(), l.cut), (SHOWN_MAX, 3));
        assert_eq!(l.entries[0].name, "d00000");
        assert!(list_dir(&s.join("없음"), &[], false).is_err());
    }
}
