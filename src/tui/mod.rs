//! 탐색기 화면. **읽기 전용이다.**
//!
//! 상태([`App`])와 그림([`draw`])을 나눈다. 상태 전이는 터미널 없이 시험되고,
//! 그림은 `TestBackend` 로 시험된다 — 둘 다 TTY 를 켜지 않는다.

pub mod draw;

use crate::config::Config;
use crate::model::Issue;
use crate::nav::{Entry, Index, Path, Seg};
use crate::query::{Filter, Raw, Where};
use crate::store::{Load, Repo};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::ListState;

/// 목록의 한 줄. `..` 은 이슈가 아니므로 [`Entry`] 로는 못 담는다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Row {
    /// 한 층 위로. 뿌리가 아닐 때만 맨 앞에 선다 — MC 와 같다.
    Up,
    Item(Entry),
}

/// 커서가 선 줄의 **정체**. 첨자(`Entry::at`)와 줄 번호는 다시 읽으면 바뀐다 —
/// 커서 위에 줄 하나가 생기거나 칸이 바뀌어 차례가 달라지면 같은 번호가 다른
/// 이슈를 가리키고, 그러면 가만히 보던 커서가 옆 줄로 튄다.
///
/// 이슈는 id 로 붙든다. **중복 id 면 첫 줄로 간다** — 둘 중 어느 쪽인지는 id 로는
/// 못 가르고, `duplicate_id` 는 이미 배너가 말한다. 바구니는 제 줄이 없어 `Seg` 로.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Anchor {
    Up,
    Issue(String),
    Bucket(Seg),
}

/// 무엇을 받고 있는가. 글을 받는 동안에는 이동키가 글자가 된다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Browse,
    /// `/` 로 연 빠른 검색.
    Grep(String),
    /// `f` 로 연 거름망. **CLI 와 같은 `항목=값` 문법이다.**
    Filter(String),
}

fn states_of(issues: &[Issue], cfg: &Config) -> std::collections::BTreeMap<String, String> {
    crate::report::group_states(issues, cfg)
        .into_iter()
        .map(|(id, col)| (id.to_string(), col.to_string()))
        .collect()
}

pub struct App {
    pub issues: Vec<Issue>,
    pub index: Index,
    /// 묶음 id → 멤버에서 읽은 칸 (`report::group_states`). **적재 때 한 번 센다**
    /// — 프레임마다 세면 줄 하나 그리는 데 저장소를 걷는다.
    states: std::collections::BTreeMap<String, String>,
    pub cfg: Config,
    pub path: Path,
    pub cursor: usize,
    pub mode: Mode,
    /// 지금 걸린 거름망. 사람이 적은 글 그대로도 들고 있어야 화면에 되비친다.
    pub filter_text: Option<String>,
    /// 어디서 읽어 왔나. 시험은 저장소 없이 App 을 세우므로 없을 수 있다.
    pub repo: Option<Repo>,
    /// 읽은 그 순간의 시각. **프레임마다가 아니라 적재마다 잡는다** — 매번
    /// 다시 잡으면 "3일 넘게" 같은 판정이 초 단위로 깜빡인다.
    pub now: String,
    /// 읽다 만난 못 읽는 줄 — 줄마다 **그 줄이 쓰는 id** (읽어 낼 수 있었던
    /// 것만). 대체 화면 안에서는 stderr 로 못 알린다. 수를 따로 들지 않는다 —
    /// 둘로 들면 어긋날 수 있고, 산 줄과의 중복을 `moai status` 와 같은 자로
    /// 세려면 수만으로는 모자란다(moai-4dk4).
    pub unreadable: Vec<Option<String>>,
    /// 마지막 갱신이 **실패한** 까닭. 조용히 삼키면 F5 가 아무 일도 안 하는데
    /// "바뀌었다" 배너는 붙어 있어, 사람은 누르고 또 누르며 까닭을 못 얻는다.
    pub trouble: Option<String>,
    /// `moai status` 가 드러낼 것의 수. 자세한 화면은 나중에 얹는다.
    pub warnings: usize,
    /// 상세를 몇 줄 굴렸는가. **왼쪽 커서를 옮기면 0 으로 돌아간다** — 다른
    /// 이슈를 보는데 굴린 자리가 남아 있으면 첫 줄부터 못 본다.
    pub scroll: u16,
    /// 본문을 그리지 않고 원문 그대로 보는가. 그린 글은 기호가 지워져
    /// 되돌릴 수 없다 — 긁어 붙이거나 마크다운을 고칠 때 이 길이 필요하다.
    pub raw: bool,
    /// 마지막으로 읽은 파일의 (고친 때, 길이).
    stamp: Stamp,
    /// 겹쳐 보는 동안 함께 지켜보는 옆 워크트리 스냅샷과 그 표식(`worktree::gather`
    /// 가 읽기 **전에** 잰 것). 꺼져 있으면 비었다.
    watched: Vec<(std::path::PathBuf, Stamp)>,
    /// 이슈 첨자 → 걸렸는가. **거름망이 바뀔 때만 다시 센다** — 매 프레임
    /// `Filter::matches` 를 돌리면 `Where::of` 가 프레임마다 지도를 다시 만든다.
    keep: Vec<bool>,
    /// 층마다 커서를 기억한다. 들어갔다 나오면 **있던 자리로 돌아온다** —
    /// 매번 맨 위로 튕기면 형제 여럿을 훑는 일이 못 할 짓이 된다.
    remembered: Vec<usize>,
    /// 목록이 훑고 있는 자리. **프레임을 넘어 산다** — 매 프레임 새로 만들면
    /// 위젯이 0 번 줄부터 다시 세어 커서를 늘 맨 아랫줄에 붙이고, 그러면
    /// 커서 아래를 한 줄도 못 본다.
    pub list: ListState,
    pub quit: bool,
    /// 도는 글리프의 걸음. **그린 횟수가 아니라 시계가 올린다** — 그릴 때마다
    /// 올리면 키를 누르는 동안에는 타이핑 속도로 돌고 가만히 두면 파일을 보러
    /// 깨는 걸음(700ms)으로 느려진다. 둘 다 "도는 것" 으로 읽히지 않는다.
    /// 올리는 것은 `cmd::tui` 의 루프 하나뿐이고, 그래서 한 프레임 안의
    /// 목록·상세·롤업이 같은 걸음을 본다.
    pub spin: usize,
    /// 다른 워크트리를 겹쳐 보는가. `w` 가 켜고 끈다 — CLI 의 `--worktree` 와 같은
    /// 길(`worktree::gather`)로 읽는다. **꺼진 채로 시작한다**: 켜면 `git` 을 부르고
    /// 옆 파일을 읽는데, 시키지 않은 일을 여는 순간마다 하면 탐색기가 무거워진다.
    pub worktree: bool,
    /// 겹쳐 본 줄의 출처. 꺼져 있으면 비었다.
    pub origin: crate::worktree::Origin,
    /// 옆 워크트리에서 만난 문제. **배너로 말만 한다** — CLI 가 stderr 로 흘리는
    /// 말인데, 대체 화면 안에서는 그 길을 못 쓴다.
    pub elsewhere: Vec<String>,
}

impl App {
    /// 저장소 없이 세운다 — 시험과 눈으로 보는 길이 이것을 쓴다. 진짜 길은
    /// [`App::open`] 이고, 그쪽은 색인을 부른 쪽에서 받는다.
    #[cfg(test)]
    pub fn new(issues: Vec<Issue>, cfg: Config, path: Path) -> App {
        let index = Index::of(&issues);
        App::build(issues, index, cfg, path, Vec::new())
    }

    /// 저장소에서 읽어 세운다.
    ///
    /// **색인은 부른 쪽이 이미 만든 것을 받는다** — 여는 데서 다시 만들면
    /// 같은 훑기를 두 번 하고, 그 훑기는 이슈 수에 비례한다.
    /// **표식도 부른 쪽이 읽기 전에 잰 것을 받는다** — 읽고 나서 재면 그
    /// 사이에 떨어진 쓰기가 "이미 본 것" 으로 적혀 영영 안 보인다.
    pub fn open(repo: Repo, load: Load, index: Index, path: Path, stamp: Stamp) -> App {
        let cfg = repo.config.clone();
        let ids = load.errors.iter().map(|e| e.id.clone()).collect();
        let mut app = App::build(load.issues, index, cfg, path, ids);
        app.stamp = stamp;
        app.repo = Some(repo);
        app
    }

    fn build(
        issues: Vec<Issue>,
        index: Index,
        cfg: Config,
        path: Path,
        unreadable_ids: Vec<Option<String>>,
    ) -> App {
        // 들어간 채로 시작하면(`--path`) 나올 층마다 기억 자리를 만들어 둔다.
        let remembered = vec![0; path.len()];
        let keep = vec![true; issues.len()];
        let states = states_of(&issues, &cfg);
        let mut app = App {
            issues,
            index,
            states,
            cfg,
            path,
            cursor: 0,
            mode: Mode::Browse,
            scroll: 0,
            raw: false,
            filter_text: None,
            repo: None,
            now: crate::model::now(),
            unreadable: unreadable_ids,
            trouble: None,
            warnings: 0,
            stamp: None,
            watched: Vec::new(),
            keep,
            remembered,
            list: ListState::default(),
            quit: false,
            spin: 0,
            worktree: false,
            origin: crate::worktree::Origin::default(),
            elsewhere: Vec::new(),
        };
        // 한 번만 센다. `report::status` 는 이슈 수에 비례한 훑기라, 못 읽는 줄
        // 수를 나중에 넣겠다고 두 번 부르면 그 절반이 버려진다.
        app.count_warnings();
        app
    }

    /// 다시 읽는다. **거름망과 있던 자리는 지키려 애쓴다** — 갱신 한 번에
    /// 하던 일이 흩어지면 F5 를 안 누르게 되고, 그러면 낡은 화면을 본다.
    pub fn reload(&mut self) {
        let Some(repo) = &self.repo else { return };
        // 읽기 **전에** 잰다. 뒤에 재면 읽고 재는 사이의 쓰기를 놓치고, 놓친
        // 것은 영영 안 돌아온다. 먼저 재면 최악이 헛 알림 하나다.
        let stamp = stamp_of(repo);
        match crate::worktree::gather(repo, self.worktree) {
            Ok(g) => {
                self.trouble = None;
                self.stamp = stamp;
                // 옆에서만 온 줄과 겹친 id 는 중복으로 세지 않는다 (`Origin::unreadable`).
                self.unreadable = g
                    .origin
                    .unreadable(g.load.errors.iter().map(|e| e.id.as_deref()))
                    .into_iter()
                    .map(|id| id.map(str::to_string))
                    .collect();
                self.origin = g.origin;
                self.elsewhere = g.trouble;
                self.watched = g.watched;
                self.adopt(g.load.issues);
            }
            // **소리 없이 넘기지 않는다.** 삼키면 F5 는 아무 일도 안 하고
            // 배너는 그대로 붙어 있어, 사람은 누르고 또 누르며 까닭을 못 얻는다.
            Err(e) => self.trouble = Some(e.to_string()),
        }
    }

    /// 돌 것이 한 줄이라도 있는가. **화면에 보이는지까지는 따지지 않는다** —
    /// 보이는 줄만 가리려면 루프가 그림의 결과를 알아야 하고 그 값은 그린 뒤에야
    /// 나온다. 틀리는 쪽은 "있는데 안 보인다" 하나뿐이고, 그때 손해는 안 보이는
    /// 것을 위해 걸음을 재는 것이다. 반대쪽은 안 틀린다 — 화면에 도는 글리프가
    /// 있으면 그 이슈는 `issues` 에 있으므로 여기가 참이다. 스피너를 그려 놓고
    /// 아무도 안 깨우는 조합은 그래서 못 생긴다.
    /// **묶음은 안 센다.** 묶음이 읽은 칸은 `in_progress` 라도 그 자리에서 누가
    /// 손대고 있다는 뜻이 아니다 — 멤버 하나가 끝나고 하나가 남기만 해도 그렇게
    /// 읽힌다. 그것으로 깨우면 아무 일도 안 벌어진 저장소에서 탐색기가 120ms 마다
    /// 깨어 도는 글리프를 그린다. 그리는 쪽(`draw::row_line`)도 묶음은 안 돌린다.
    pub fn spinning(&self) -> bool {
        self.issues.iter().any(|i| !crate::report::is_group(i) && crate::style::spins(i.status.as_str()))
    }

    /// 그 줄이 **서 있는** 칸 — 묶음이면 멤버에서 읽은 칸, 아니면 제 칸. CLI 가
    /// 묻는 `report::column` 과 같은 답이다 — 묶음만 읽은 칸을 받는 것까지 같다.
    pub fn column(&self, at: usize) -> &str {
        let i = &self.issues[at];
        crate::report::is_group(i)
            .then(|| self.states.get(&i.id).map(String::as_str))
            .flatten()
            .unwrap_or(i.status.as_str())
    }

    /// 새 자료를 받아들이고 어긋난 것을 손본다. 시험이 저장소 없이 부른다.
    ///
    /// **커서는 번호가 아니라 정체로 따라간다**([`Anchor`]). 보던 줄이 아직 보이면
    /// 그 줄에 서고, 사라졌으면(지워졌거나 거름망에 빠졌으면) 전처럼 그 번호를 목록
    /// 안으로 자른 자리에 선다.
    pub fn adopt(&mut self, issues: Vec<Issue>) {
        // **옛 자료로 잰다** — 줄의 첨자는 옛 `issues` 를 가리킨다.
        let held = self.current().map(|r| self.anchor_of(&r));
        self.issues = issues;
        self.index = Index::of(&self.issues);
        self.states = states_of(&self.issues, &self.cfg);
        self.now = crate::model::now();
        self.repair_path();
        // 거름망은 이슈 첨자에 매인 것이라 반드시 다시 센다.
        match self.filter_text.clone() {
            Some(t) => {
                let mode = if let Some(q) = t.strip_prefix('/') {
                    Mode::Grep(q.to_string())
                } else {
                    Mode::Filter(t)
                };
                if self.apply(&mode).is_err() {
                    self.clear_filter();
                }
            }
            None => self.keep = vec![true; self.issues.len()],
        }
        let rows = self.rows();
        let found = held.and_then(|a| rows.iter().position(|r| self.anchor_of(r) == a));
        // **굴린 자리는 같은 줄일 때만 둔다.** 다른 이슈로 옮겨 섰는데 굴린 수가 남으면
        // 그 이슈를 첫 줄부터 못 본다 — 커서를 옮길 때 0 으로 되돌리는 것(`move_to`)과
        // 같은 까닭이다. 같은 줄이면 본문이 바뀌었어도 두고, 넘치면 그림이 자른다.
        if found.is_none() {
            self.scroll = 0;
        }
        self.cursor = found.unwrap_or(self.cursor.min(rows.len().saturating_sub(1)));
        self.count_warnings();
    }

    /// 그 줄의 정체. 지금 `issues` 에 대해 잰다.
    fn anchor_of(&self, row: &Row) -> Anchor {
        match row {
            Row::Up => Anchor::Up,
            Row::Item(Entry::Dir { seg, at: None }) => Anchor::Bucket(seg.clone()),
            Row::Item(Entry::Dir { at: Some(at), .. } | Entry::Leaf { at }) => {
                Anchor::Issue(self.issues[*at].id.clone())
            }
        }
    }

    /// 들고 있던 경로가 아직 갈 수 있는 길인가.
    ///
    /// 마일스톤이 **처음 생기는 순간** 트리가 한 층 깊어져 경로가 통째로
    /// 낡는다. 지운 에픽도 마찬가지다. 갈 수 있는 데까지만 남기고 자른다 —
    /// 없는 자리에 서 있으면 빈 목록이 나오고, 사람은 자료가 사라진 줄 안다.
    fn repair_path(&mut self) {
        let mut good = Path::new();
        for seg in self.path.clone() {
            let here = self.index.entries(&self.issues, &good);
            let ok = here.iter().any(|e| matches!(e, Entry::Dir { seg: s, .. } if *s == seg));
            if !ok {
                break;
            }
            good.push(seg);
        }
        if good.len() == self.path.len() {
            self.path = good;
            return;
        }
        // **옮겨진 것과 지워진 것은 다르다.** 서 있던 마디가 아직 살아 있으면
        // 자리만 바뀐 것이니 그 새 자리로 따라간다 — 마일스톤이 처음 생기면
        // 에픽이 한 층 깊어지는데, 거기서 뿌리로 내려놓으면 가장 흔한 갱신이
        // 하필 자리를 가장 크게 잃는 갱신이 된다. `home_of` 가 그 새 자리를
        // 이미 알고, `cmd/tui.rs::resolve` 도 같은 셈을 쓴다.
        if let Some(at) = self.path.last().and_then(|s| self.index.find(seg_id(s)?))
            && self.index.is_dir(&self.issues, at)
        {
            let mut moved = self.index.home_of(at).clone();
            moved.push(self.index.seg_of(&self.issues, at));
            if moved != self.path {
                self.remembered = vec![0; moved.len()];
                self.cursor = 0;
                self.path = moved;
                return;
            }
        }
        self.remembered.truncate(good.len());
        self.cursor = 0;
        self.path = good;
    }

    /// **알림은 안 센다.** 배너는 "드러난 것 N건" 이라고 말하는데, 담아 둔
    /// 생각이 쌓였다는 알림을 거기 더하면 생각을 담을수록 화면이 고쳐야 할
    /// 것이 늘었다고 말한다 — 그러면 안 담게 된다. 무엇이 알림인지는
    /// `report` 가 `notices` 로 따로 내므로 여기서 다시 판단하지 않는다.
    fn count_warnings(&mut self) {
        // 못 읽는 줄의 id 까지 넘긴다 — 산 줄과의 중복을 `moai status` 와 같은
        // 자로 센다.
        let lines: Vec<crate::report::Unreadable> = self
            .unreadable
            .iter()
            .map(|id| crate::report::Unreadable { id: id.as_deref() })
            .collect();
        let st = crate::report::status(&self.issues, &lines, &self.cfg, &self.now);
        // 알림은 `notices` 에 따로 있다 — `warnings` 가 곧 고칠 것이다.
        self.warnings = st.warnings.len();
    }

    /// 파일이 우리가 읽은 뒤로 바뀌었으면 **저절로 다시 읽는다.**
    ///
    /// 한때 말만 하고 F5 를 기다렸다(moai-6qdx) — 읽으면 커서가 튀었기 때문이다.
    /// 커서·기억 자리·굴린 자리가 줄의 정체를 따라가게 된 뒤로(moai-cera) 그 까닭이
    /// 없어졌고, 남은 것은 사람이 배너를 보고 키를 눌러야 하는 수고뿐이었다.
    ///
    /// **고친 때만 보면 놓친다** — rename 으로 갈아끼우는 쓰기는 같은 초에 떨어질 수
    /// 있어 길이도 함께 본다. **`stamp` 이 없다고 멈추지 않는다.** 아직 파일이 없는
    /// 저장소는 `stamp_of` 가 `None` 을 내는데, 거기서 걸러 버리면 파일이 생긴 뒤에도
    /// 영영 바뀐 줄 모른다. `None != Some(..)` 이 이미 바르게 답한다.
    ///
    /// **읽기가 실패하면 표식을 안 올린다**(`reload`) — 다음 걸음에 다시 해 본다.
    ///
    /// **겹쳐 보는 동안에는 옆 워크트리 스냅샷도 본다**(`watched`). 옆에서 `mv` 가
    /// 떨어진 것을 모르면 겹쳐 보기를 켠 뜻이 절반만 선다. 스냅샷이 아직 없던 옆
    /// 워크트리도 지켜보므로 거기서 `moai init` 하면 알아챈다. 새로 생긴 워크트리는
    /// `git worktree list` 를 다시 불러야 알아서 여기서는 못 보고, F5 가 그 길이다 —
    /// 걸음마다 git 을 띄우는 것은 700ms 마다 프로세스 하나를 만드는 일이다.
    pub fn follow(&mut self) {
        let Some(repo) = &self.repo else { return };
        let moved = stamp_of(repo) != self.stamp
            || self.watched.iter().any(|(path, was)| crate::store::stamp(path) != *was);
        if moved {
            self.reload();
        }
    }

    /// 거름망을 건다. 빈 글은 "거름망 없음" 이다.
    ///
    /// **`all` 을 켠다.** `Filter` 의 기본값은 done 을 숨기는데, 탐색기가
    /// 시키지도 않은 줄을 숨기면 파일이 사라진 것처럼 보인다. 열린 것만 보려면
    /// `status=todo` 라고 적으면 된다.
    pub fn apply(&mut self, mode: &Mode) -> Result<(), String> {
        let text = match mode {
            Mode::Grep(q) | Mode::Filter(q) => q.clone(),
            Mode::Browse => String::new(),
        };
        if text.trim().is_empty() {
            self.filter_text = None;
            self.keep = vec![true; self.issues.len()];
            return Ok(());
        }
        let filter = self.build_filter(mode)?;
        // 칸 이름은 `Filter::build` 가 모른다 — 저장소가 정하는 것이라
        // `config` 에 있다. `cmd/show.rs` 와 같은 자로 잰다: 조용히 0건을 내면
        // `status=in-progress` 같은 오타가 "그 칸은 비었다" 와 구별되지 않는다.
        for s in &filter.status {
            self.cfg.require_known(s)?;
        }
        // 시계는 **적재마다** 고정한 것을 쓴다. 여기서 다시 잡으면 `stale=`
        // 같은 물음이 화면의 나머지와 다른 시각으로 판정된다.
        let now = self.now.clone();
        let wh = Where::of(&self.issues, &self.cfg);
        self.keep = self.issues.iter().map(|i| filter.matches(i, &now, &wh)).collect();
        self.filter_text = Some(match mode {
            Mode::Grep(_) => format!("/{text}"),
            _ => text,
        });
        Ok(())
    }

    /// 적은 글을 거름망으로. **적는 곳과 물어보는 곳이 같은 것을 쓴다** —
    /// 갈라지면 프롬프트 밑의 오류가 Enter 가 판정할 글과 다른 글을 판정한다.
    fn build_filter(&self, mode: &Mode) -> Result<Filter, String> {
        let raw = match mode {
            Mode::Grep(q) => Raw { grep: Some(q.clone()), all: true, ..Raw::default() },
            Mode::Filter(q) => Raw { filter: split_filter(q), all: true, ideas: true, ..Raw::default() },
            Mode::Browse => Raw::default(),
        };
        // **`Filter::build` 를 지난다.** 소문자 접기·태그 정규화·`항목=값` 해석이
        // 전부 거기 있고, 건너뛰면 CLI 와 TUI 가 같은 글을 다르게 읽는다.
        Filter::build(raw)
    }

    pub fn clear_filter(&mut self) {
        self.filter_text = None;
        self.keep = vec![true; self.issues.len()];
    }

    /// 지금 디렉터리의 줄들.
    pub fn rows(&self) -> Vec<Row> {
        let mut rows: Vec<Row> = Vec::new();
        if !self.path.is_empty() {
            rows.push(Row::Up);
        }
        let keep = &self.keep;
        rows.extend(
            self.index
                .entries_where(&self.issues, &self.path, &|at| keep[at])
                .into_iter()
                .map(Row::Item),
        );
        rows
    }

    /// 커서가 가리키는 줄.
    pub fn current(&self) -> Option<Row> {
        self.rows().get(self.cursor).cloned()
    }

    /// 이미 센 목록에서 커서가 가리키는 줄. **그림은 한 프레임에 목록을 한 번만
    /// 센다** — 목록 패널과 상세 패널이 각자 세면 이슈 전부를 훑고 정렬하는 일이
    /// 한 키 누름에 두 번 더 돈다.
    pub fn current_of(&self, rows: &[Row]) -> Option<Row> {
        rows.get(self.cursor).cloned()
    }

    pub fn key(&mut self, k: KeyEvent) {
        // 글을 받는 동안에는 이동키가 글자다. 먼저 가로챈다.
        if !matches!(self.mode, Mode::Browse) {
            self.typing(k);
            return;
        }
        // **목록은 필요할 때만 센다.** 세는 데 이슈 전부를 훑고 정렬까지 하므로,
        // 끝내기·검색 같은 키에도 미리 세면 그 값이 그대로 버려진다.
        let len = || self.rows().len();
        match k.code {
            // raw mode 에서는 Ctrl-C 가 신호로 오지 않는다. 안 받으면 길이 막힌다.
            KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => self.quit = true,
            KeyCode::Char('q') | KeyCode::F(10) => self.quit = true,
            KeyCode::Up => self.move_to(self.cursor.saturating_sub(1)),
            KeyCode::Down => self.move_to((self.cursor + 1).min(len().saturating_sub(1))),
            KeyCode::Home => self.move_to(0),
            KeyCode::End => self.move_to(len().saturating_sub(1)),
            KeyCode::PageUp => self.move_to(self.cursor.saturating_sub(10)),
            KeyCode::PageDown => self.move_to((self.cursor + 10).min(len().saturating_sub(1))),
            KeyCode::Enter | KeyCode::Right => self.enter(),
            KeyCode::Backspace | KeyCode::Left => self.leave(),
            KeyCode::Char('/') => self.mode = Mode::Grep(String::new()),
            KeyCode::Char('f') | KeyCode::F(7) => self.mode = Mode::Filter(String::new()),
            // 거름망이 걸려 있으면 Esc 가 그것을 푼다. 아니면 아무 일도 없다 —
            // Esc 로 화면이 꺼지면 실수 한 번에 하던 것이 날아간다.
            KeyCode::Esc => self.clear_filter(),
            KeyCode::F(5) | KeyCode::Char('r') => self.reload(),
            // 켜고 끄는 것은 **다시 읽는 것**이다. 겹친 줄은 적재 때 한 번 세는 것이라
            // (`states`·거름망·경고), 들고 있는 것에 덧칠하면 셈이 옛 줄로 남는다.
            KeyCode::Char('w') => {
                self.worktree = !self.worktree;
                self.reload();
            }
            KeyCode::F(3) | KeyCode::Char('m') => {
                self.raw = !self.raw;
                // 그린 것과 원문은 줄 수가 다르다. 굴린 자리를 들고 가면
                // 엉뚱한 데가 나온다.
                self.scroll = 0;
            }
            // 상세를 굴린다. **왼쪽은 그대로 둔다** — 오른쪽만 길어서 못 보는
            // 것이므로, 굴리려고 커서를 옮기게 하면 보던 이슈를 잃는다.
            KeyCode::Char('j') => self.scroll = self.scroll.saturating_add(1),
            KeyCode::Char('k') => self.scroll = self.scroll.saturating_sub(1),
            KeyCode::Char(' ') => self.scroll = self.scroll.saturating_add(10),
            KeyCode::Char('b') => self.scroll = self.scroll.saturating_sub(10),
            _ => {}
        }
    }

    /// 커서를 옮기고 **상세를 첫 줄로 되돌린다.** 다른 것을 보는데 굴린 자리가
    /// 남아 있으면 그 이슈의 첫 줄부터 못 본다.
    fn move_to(&mut self, at: usize) {
        if at != self.cursor {
            self.scroll = 0;
        }
        self.cursor = at;
    }

    /// 글을 받는 중.
    fn typing(&mut self, k: KeyEvent) {
        // **Ctrl 은 글자가 아니다.** raw mode 에서는 Ctrl-C 가 신호로 오지
        // 않으므로, 여기서 글자로 먹으면 검색칸에 `c` 가 찍히고 나갈 길이
        // Esc 하나로 줄어든다. Ctrl-U 는 적던 것을 통째로 지운다.
        if k.modifiers.contains(KeyModifiers::CONTROL) {
            match k.code {
                KeyCode::Char('c') => self.quit = true,
                KeyCode::Char('u') => {
                    if let Mode::Grep(b) | Mode::Filter(b) = &mut self.mode {
                        b.clear();
                    }
                }
                _ => {}
            }
            return;
        }
        let buf = match &mut self.mode {
            Mode::Grep(b) | Mode::Filter(b) => b,
            Mode::Browse => return,
        };
        match k.code {
            KeyCode::Char(c) => buf.push(c),
            KeyCode::Backspace => {
                buf.pop();
            }
            KeyCode::Enter => {
                let mode = self.mode.clone();
                // 잘못 적은 것은 버리지 않고 그 자리에 둔다 — 지우고 다시 치게
                // 하면 긴 거름망일수록 고치기가 벌이 된다.
                if self.apply(&mode).is_ok() {
                    self.mode = Mode::Browse;
                    self.cursor = self.cursor.min(self.rows().len().saturating_sub(1));
                }
            }
            KeyCode::Esc => self.mode = Mode::Browse,
            _ => {}
        }
    }

    /// 지금 적고 있는 글에 대한 오류. 없으면 `None`.
    ///
    /// **Enter 가 밟을 길을 그대로 밟는다.** 칸 이름 검사까지 여기서 해야
    /// `status=in-progress` 같은 오타가 "그 칸은 비었다" 로 보이지 않는다.
    pub fn input_error(&self) -> Option<String> {
        match &self.mode {
            Mode::Filter(q) if !q.trim().is_empty() => match self.build_filter(&self.mode) {
                Err(e) => Some(e),
                Ok(f) => f.status.iter().find_map(|s| self.cfg.require_known(s).err()),
            },
            _ => None,
        }
    }

    fn enter(&mut self) {
        match self.current() {
            Some(Row::Up) => self.leave(),
            Some(Row::Item(Entry::Dir { seg, .. })) => {
                self.remembered.push(self.cursor);
                self.path.push(seg);
                self.cursor = 0;
                self.scroll = 0;
            }
            // 잎은 들어갈 데가 없다. 상세는 오른쪽이 이미 보여 주고 있다.
            _ => {}
        }
    }

    /// 한 층 위로. **방금 나온 디렉터리에 선다** — 그것이 곧 돌아갈 자리다.
    ///
    /// 기억한 번호(`remembered`)는 들어가 있는 동안 파일이 바뀌면 낡는다: 위에 에픽
    /// 하나가 생기면 같은 번호가 옆 에픽을 가리킨다. 그래서 번호는 나온 디렉터리를
    /// 못 찾을 때만(거름망에 빠졌거나 `--path` 로 시작했거나) 쓴다.
    fn leave(&mut self) {
        if let Some(from) = self.path.pop() {
            self.scroll = 0;
            let fallback = self.remembered.pop().unwrap_or(0);
            let rows = self.rows();
            self.cursor = rows
                .iter()
                .position(|r| matches!(r, Row::Item(Entry::Dir { seg, .. }) if *seg == from))
                .unwrap_or(fallback.min(rows.len().saturating_sub(1)));
        }
    }

    /// 지금 어디인가. 뿌리는 `/`.
    pub fn crumbs(&self) -> String {
        if self.path.is_empty() {
            return "/".into();
        }
        let mut out = String::new();
        for seg in &self.path {
            out.push('/');
            out.push_str(&self.seg_label(seg));
        }
        out
    }

    /// id 를 제목으로 푼다. 없으면 **끊겼다고 적는다** — id 만 내면 그것이
    /// 그저 제목 없는 줄인지 없는 것을 가리키는 참조인지 알 길이 없다.
    /// 훑지 않는다. 이 함수는 막는 것마다·소속마다·프레임마다 불린다.
    pub fn title_of(&self, id: &str) -> String {
        match self.index.find(id) {
            Some(at) => self.issues[at].title.clone(),
            None => format!("{id}  {MISSING}"),
        }
    }

    fn seg_label(&self, seg: &Seg) -> String {
        let id = match seg {
            Seg::Milestone(None) => return "(마일스톤 없음)".into(),
            Seg::Lost => return "(길 잃음)".into(),
            Seg::Milestone(Some(id)) | Seg::Epic(id) | Seg::Issue(id) => id,
        };
        match self.index.find(id) {
            Some(at) => self.issues[at].title.clone(),
            None => format!("{id}  {MISSING}"),
        }
    }
}

/// 없는 것을 가리키는 참조에 붙이는 말. **한 낱말로 통일한다** — 자리마다
/// 다른 말을 쓰면 같은 깨짐을 서로 다른 일로 읽는다.
const MISSING: &str = "(없다)";

/// 그 마디가 가리키는 줄의 id. 바구니는 제 줄이 없으므로 `None`.
fn seg_id(seg: &Seg) -> Option<&str> {
    match seg {
        Seg::Milestone(Some(id)) | Seg::Epic(id) | Seg::Issue(id) => Some(id),
        Seg::Milestone(None) | Seg::Lost => None,
    }
}

pub use crate::store::Stamp;

pub fn stamp_of(repo: &Repo) -> Stamp {
    crate::store::stamp(&repo.issues_path())
}

/// 한 줄을 `--filter` 토큰들로 쪼갠다.
///
/// **`항목=` 이 시작하는 데서만 쪼갠다.** 그냥 띄어쓰기로 쪼개면 값에 빈칸이
/// 든 것(`grep=원자적 쓰기`, `status=to do`)을 이 칸에서는 아예 적을 수 없다 —
/// CLI 는 그것을 인자 하나로 받으므로, "CLI 와 같은 문법" 이라던 약속이 거기서
/// 깨진다. 항목 이름이 없는 조각은 앞 토큰의 값에 마저 붙는다.
fn split_filter(q: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for w in q.split_whitespace() {
        match out.last_mut() {
            Some(prev) if !w.contains('=') => {
                prev.push(' ');
                prev.push_str(w);
            }
            _ => out.push(w.to_string()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Kind, Status};

    fn cfg() -> Config {
        Config::parse("prefix = \"argos\"\n").unwrap()
    }

    fn make(id: &str, kind: Kind) -> Issue {
        Issue::new(id.into(), format!("{id} 제목"), kind, Status::new("todo"), "2026-09-01T00:00:00Z")
    }

    fn member(id: &str, epic: &str) -> Issue {
        let mut i = make(id, Kind::Issue);
        i.epic = Some(epic.into());
        i
    }

    fn app() -> App {
        let issues = vec![
            make("argos-0001", Kind::Epic),
            make("argos-0002", Kind::Epic),
            member("argos-0003", "argos-0001"),
            member("argos-0004", "argos-0001"),
            make("argos-0009", Kind::Issue),
        ];
        App::new(issues, cfg(), Path::new())
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    /// 루프가 빠른 걸음으로 깰지를 이 답으로 정한다. **거짓을 내면 도는
    /// 글리프가 첫 칸에 멈춘 채 7초에 한 번만 움직인다** — 스피너가 있는데
    /// 아무도 깨우지 않는 그 조합이 눈에는 버그로 보이고 코드로는 안 보인다.
    #[test]
    fn the_loop_only_wakes_fast_when_something_spins() {
        let mut a = app();
        assert!(!a.spinning(), "todo 뿐인데 돈다고 한다");
        let mut issues = a.issues.clone();
        // 에픽의 적힌 칸으로는 안 돈다 — 묶음의 칸은 멤버에서 읽는다.
        issues[0].status = Status::new("in_progress");
        a.adopt(issues.clone());
        assert!(!a.spinning(), "멤버가 안 움직인 에픽이 적힌 칸으로 돈다");
        // 읽은 칸으로도 안 돈다 — 멤버 하나가 끝나기만 해도 묶음은 `in_progress` 로
        // 읽히는데, 그것으로 깨우면 아무도 손대지 않은 저장소에서 화면이 계속 깬다.
        issues[2].status = Status::new("done");
        a.adopt(issues.clone());
        assert!(!a.spinning(), "일은 멈췄는데 묶음의 읽은 칸으로 돈다");
        issues[3].status = Status::new("in_progress");
        a.adopt(issues);
        assert!(a.spinning(), "in_progress 가 있는데 안 돈다고 한다");
    }

    /// 진짜 파일을 쓰는 시험이 쓰는 임시 자리. **터져도 치운다** — 바로
    /// `remove_dir_all` 을 부르면 assert 하나가 터질 때마다 찌꺼기가 남고,
    /// 이름이 pid 라 다음 실행이 그것을 치우지도 못한다.
    struct Scratch(std::path::PathBuf);

    impl Scratch {
        fn new(name: &str) -> Scratch {
            let dir = std::env::temp_dir().join(format!(
                "moai-tui-{name}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(dir.join(".moai")).unwrap();
            std::fs::write(dir.join(".moai/config.toml"), "prefix = \"argos\"\n").unwrap();
            Scratch(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// 지금 보이는 줄의 id 들 (`..` 은 뺀다).
    fn shown(a: &App) -> Vec<String> {
        a.rows()
            .iter()
            .filter_map(|r| match r {
                Row::Item(e) => e.at().map(|at| a.issues[at].id.clone()),
                Row::Up => None,
            })
            .collect()
    }

    /// 뿌리에는 `..` 이 없다. 들어가면 생긴다.
    #[test]
    fn up_appears_only_below_the_root() {
        let mut a = app();
        assert!(!a.rows().contains(&Row::Up));
        a.key(key(KeyCode::Enter));
        assert_eq!(a.rows().first(), Some(&Row::Up));
    }

    /// 들어갔다 나오면 **있던 자리로 돌아온다**. 매번 맨 위로 튕기면 못 쓴다.
    #[test]
    fn leaving_puts_the_cursor_back_where_it_was() {
        let mut a = app();
        a.key(key(KeyCode::Down)); // 두 번째 에픽
        assert_eq!(a.cursor, 1);
        a.key(key(KeyCode::Enter));
        assert_eq!((a.cursor, a.path.len()), (0, 1));
        a.key(key(KeyCode::Backspace));
        assert_eq!((a.cursor, a.path.len()), (1, 0), "있던 자리로 안 돌아왔다");
    }

    /// 들어가 있는 동안 **위에 줄이 생겨도** 나오면 방금 나온 디렉터리에 선다.
    /// 기억한 번호로 돌아가면 옆 에픽에 선다.
    #[test]
    fn leaving_finds_the_directory_it_came_from_even_after_a_reload() {
        let mut a = app();
        a.key(key(KeyCode::Down));
        a.key(key(KeyCode::Enter));
        assert_eq!(a.path, [Seg::Epic("argos-0002".into())]);

        let mut more = a.issues.clone();
        more.push(make("argos-0000", Kind::Epic));
        a.adopt(more);
        a.key(key(KeyCode::Backspace));
        assert_eq!(
            a.current(),
            Some(Row::Item(Entry::Dir { seg: Seg::Epic("argos-0002".into()), at: a.index.find("argos-0002") })),
            "나온 디렉터리가 아니라 기억한 번호에 섰다"
        );
    }

    /// 커서는 목록 밖으로 못 나간다.
    #[test]
    fn the_cursor_stays_inside_the_list() {
        let mut a = app();
        for _ in 0..50 {
            a.key(key(KeyCode::Up));
        }
        assert_eq!(a.cursor, 0);
        for _ in 0..50 {
            a.key(key(KeyCode::Down));
        }
        assert_eq!(a.cursor, a.rows().len() - 1);
        a.key(key(KeyCode::Home));
        assert_eq!(a.cursor, 0);
        a.key(key(KeyCode::End));
        assert_eq!(a.cursor, a.rows().len() - 1);
    }

    /// 잎에서 Enter 는 아무 일도 하지 않는다 — 들어갈 데가 없다.
    #[test]
    fn entering_a_leaf_does_nothing() {
        let mut a = app();
        a.key(key(KeyCode::End)); // 소속 없는 이슈 (잎)
        let before = a.path.clone();
        a.key(key(KeyCode::Enter));
        assert_eq!(a.path, before);
    }

    /// `..` 에서 Enter 는 나가기다.
    #[test]
    fn entering_the_up_row_leaves() {
        let mut a = app();
        a.key(key(KeyCode::Enter));
        assert_eq!(a.path.len(), 1);
        a.cursor = 0; // `..`
        a.key(key(KeyCode::Enter));
        assert!(a.path.is_empty());
    }

    #[test]
    fn quitting_works_every_documented_way() {
        for k in [
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            key(KeyCode::Char('q')),
            key(KeyCode::F(10)),
        ] {
            let mut a = app();
            a.key(k);
            assert!(a.quit, "{k:?} 로 못 나갔다");
        }
    }

    /// 뿌리는 `/`, 들어가면 **제목**으로 적는다 — id 를 적으면 사람이 그걸
    /// 다시 찾아봐야 한다.
    #[test]
    fn crumbs_read_as_titles() {
        let mut a = app();
        assert_eq!(a.crumbs(), "/");
        a.key(key(KeyCode::Enter));
        assert_eq!(a.crumbs(), "/argos-0001 제목");
    }

    fn typed(a: &mut App, text: &str) {
        for c in text.chars() {
            a.key(key(KeyCode::Char(c)));
        }
        a.key(key(KeyCode::Enter));
    }

    /// `/` 로 검색하면 안 걸린 잎은 사라지고, 걸린 것을 품은 디렉터리는 남는다.
    #[test]
    fn slash_searches_titles() {
        let mut a = app();
        a.key(key(KeyCode::Char('/')));
        assert!(matches!(a.mode, Mode::Grep(_)));
        typed(&mut a, "0004");
        assert_eq!(a.mode, Mode::Browse);
        assert_eq!(a.filter_text.as_deref(), Some("/0004"));

        // 뿌리에는 그것을 품은 에픽만 남는다
        let ids = shown(&a);
        assert_eq!(ids, ["argos-0001"], "{ids:?}");
        // 그 안에 걸린 것이 있다
        a.key(key(KeyCode::Enter));
        assert_eq!(shown(&a), ["argos-0004"]);
    }

    /// 거름망은 **CLI 와 같은 문법**이다. 없는 항목은 그 자리에서 나무란다.
    #[test]
    fn f_takes_the_same_grammar_as_the_cli() {
        let mut a = app();
        a.key(key(KeyCode::Char('f')));
        typed(&mut a, "type=epic");
        assert_eq!(a.filter_text.as_deref(), Some("type=epic"));
        assert_eq!(shown(&a), ["argos-0001", "argos-0002"]);

        // 잘못 적으면 걸리지 않고 그 자리에 남는다 — 지우고 다시 치게 하지 않는다
        let mut a = app();
        a.key(key(KeyCode::Char('f')));
        typed(&mut a, "statu=todo");
        assert!(matches!(a.mode, Mode::Filter(_)), "잘못 적었는데 넘어갔다");
        assert!(a.input_error().is_some());
        assert_eq!(a.filter_text, None);
    }

    /// **값에 빈칸이 들어간다.** 띄어쓰기로 죄다 쪼개면 `grep=원자적 쓰기` 를
    /// 이 칸에서는 아예 못 적는다 — CLI 는 인자 하나로 받으니 "같은 문법" 이
    /// 아니게 된다. 항목 이름이 시작하는 데서만 쪼갠다.
    #[test]
    fn a_filter_value_may_contain_spaces() {
        let mut issues = vec![make("argos-0001", Kind::Epic), make("argos-0009", Kind::Issue)];
        issues[1].title = "원자적 쓰기를 고친다".into();
        let mut a = App::new(issues, cfg(), Path::new());
        a.key(key(KeyCode::Char('f')));
        typed(&mut a, "grep=원자적 쓰기");
        assert!(a.input_error().is_none(), "{:?}", a.input_error());
        assert_eq!(shown(&a), ["argos-0009"]);

        // 여러 조건은 여전히 띄어쓰기로 잇는다
        let mut a = app();
        a.key(key(KeyCode::Char('f')));
        typed(&mut a, "type=epic status=todo");
        assert_eq!(shown(&a), ["argos-0001", "argos-0002"]);
    }

    /// **모르는 칸은 거절한다.** `cmd/show.rs` 가 쓰는 것과 같은 자다 — 조용히
    /// 0건을 내면 `status=in-progress` 같은 오타가 "그 칸은 비었다" 와
    /// 구별되지 않는다.
    #[test]
    fn an_unknown_column_is_refused_not_silently_empty() {
        let mut a = app();
        a.key(key(KeyCode::Char('f')));
        typed(&mut a, "status=in-progress");
        assert!(matches!(a.mode, Mode::Filter(_)), "오타인데 걸렸다");
        assert!(a.input_error().is_some_and(|e| e.contains("칸")), "{:?}", a.input_error());
        assert_eq!(a.filter_text, None);
    }

    /// raw mode 에서는 Ctrl-C 가 신호로 오지 않는다. **글을 받는 중에도** 받아야
    /// 한다 — 글자로 먹으면 검색칸에 `c` 가 찍히고 나갈 길이 하나로 줄어든다.
    #[test]
    fn ctrl_c_quits_even_while_typing() {
        for opener in [KeyCode::Char('/'), KeyCode::Char('f')] {
            let mut a = app();
            a.key(key(opener));
            a.key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
            assert!(a.quit, "{opener:?} 중에 Ctrl-C 를 글자로 먹었다");
            assert!(matches!(&a.mode, Mode::Grep(b) | Mode::Filter(b) if b.is_empty()));
        }
    }

    /// 파일이 **아직 없는** 저장소에서도 생긴 것을 알아챈다. 표식이 없다고
    /// 한 번 걸러 버리면 그 뒤로 영영 못 알아챈다.
    #[test]
    fn a_file_that_appears_later_is_still_noticed() {
        let scratch = Scratch::new("appear");
        let dir = scratch.0.clone();
        let repo = Repo { root: dir.clone(), config: cfg() };
        let stamp = stamp_of(&repo);
        let load = repo.read().unwrap();
        assert!(stamp.is_none() && load.issues.is_empty(), "판이 다르다");
        let index = Index::of(&load.issues);
        let mut a = App::open(repo, load, index, Path::new(), stamp);

        std::fs::write(
            dir.join(".moai/issues.jsonl"),
            format!("{}\n", serde_json::to_string(&make("argos-0001", Kind::Epic)).unwrap()),
        )
        .unwrap();
        a.follow();
        assert_eq!(a.issues.len(), 1, "없던 파일이 생긴 것을 못 알아챘다");
        assert!(a.trouble.is_none());
    }

    /// **거름망이 찾으려던 것을 숨기지 않는다.** 저 자신은 안 걸리는 에픽도 걸린
    /// 멤버가 있으면 남는다. `Filter` 의 기본값이 done 을 숨기는 것에 걸려들지 않아야 한다.
    ///
    /// **에픽 자신은 정말로 안 걸려야 한다.** 묶음의 칸은 멤버에서 읽으므로(moai-j3b3)
    /// 적힌 칸을 `done` 에 둬도 멤버가 `todo` 뿐이면 에픽이 `todo` 로 서서 제 손으로
    /// 걸린다 — 그러면 이 시험은 자손으로 남는 길을 한 번도 안 지난다. 끝난 멤버를
    /// 하나 둬 에픽을 `in_progress` 에 세운다.
    #[test]
    fn an_unmatched_epic_does_not_swallow_its_matching_members() {
        let mut closed = member("argos-0002", "argos-0001");
        closed.status = Status::new("done");
        let issues = vec![
            make("argos-0001", Kind::Epic),
            closed,
            member("argos-0003", "argos-0001"),
        ];
        let mut a = App::new(issues, cfg(), Path::new());
        assert_eq!(a.column(0), "in_progress", "에픽이 제 손으로 걸린다 — 시험이 자손 길을 안 지난다");
        a.key(key(KeyCode::Char('f')));
        typed(&mut a, "status=todo");
        assert_eq!(shown(&a), ["argos-0001"], "안 걸린 에픽이 걸린 멤버를 데리고 사라졌다");
    }

    /// Esc 는 거름망을 푼다. 화면을 끄지는 않는다 — 실수 한 번에 하던 것이
    /// 날아가면 안 된다.
    #[test]
    fn esc_clears_the_filter_but_never_quits() {
        let mut a = app();
        a.key(key(KeyCode::Char('/')));
        typed(&mut a, "0004");
        assert!(a.filter_text.is_some());
        a.key(key(KeyCode::Esc));
        assert_eq!(a.filter_text, None);
        assert!(!a.quit);
        assert_eq!(shown(&a).len(), 3);
    }

    /// 글을 받는 동안에는 이동키가 글자다 — `q` 를 쳤다고 꺼지면 못 쓴다.
    #[test]
    fn typing_does_not_trigger_browse_keys() {
        let mut a = app();
        a.key(key(KeyCode::Char('/')));
        for c in "quit".chars() {
            a.key(key(KeyCode::Char(c)));
        }
        assert!(!a.quit);
        assert_eq!(a.mode, Mode::Grep("quit".into()));
        a.key(key(KeyCode::Esc));
        assert_eq!(a.mode, Mode::Browse);
    }

    /// 들고 있던 길이 사라지면 **갈 수 있는 데까지만** 남긴다. 없는 자리에 서
    /// 있으면 빈 목록이 나오고, 사람은 자료가 사라진 줄 안다.
    #[test]
    fn reload_repairs_a_path_that_no_longer_exists() {
        let mut a = app();
        a.key(key(KeyCode::Enter)); // 에픽 안으로
        assert_eq!(a.path.len(), 1);

        // 그 에픽이 사라진 자료로 갈아탄다
        a.adopt(vec![make("argos-0002", Kind::Epic), make("argos-0009", Kind::Issue)]);
        assert!(a.path.is_empty(), "없는 자리에 그대로 서 있다");
        assert_eq!(a.cursor, 0);
        assert!(!a.rows().is_empty());
    }

    /// 마일스톤이 **처음 생기면** 트리가 한 층 깊어져 경로가 통째로 낡는다.
    /// 그래도 **서 있던 것이 아직 있으면 따라간다** — 자리만 바뀐 것을 지워진
    /// 것과 같이 다루면, 가장 흔한 갱신이 하필 자리를 가장 크게 잃는다.
    #[test]
    fn a_first_milestone_deepens_the_tree_and_the_cursor_follows_the_epic() {
        let mut a = app();
        a.key(key(KeyCode::Enter));
        let was = a.path.clone();

        let mut with = a.issues.clone();
        with.push(make("argos-9999", Kind::Milestone));
        a.adopt(with);
        // 길은 낡았지만 뿌리로 내려놓지는 않는다 — 에픽이 간 자리로 따라간다
        assert_ne!(a.path, was, "트리가 깊어졌는데 길이 그대로다");
        assert_eq!(a.path.last(), was.last(), "서 있던 에픽을 놓쳤다");
        assert_eq!(a.path.len(), 2, "{:?}", a.path);
        assert_eq!(a.remembered.len(), a.path.len());
    }

    /// 갱신해도 걸어 둔 거름망은 살아 있다. 갱신 한 번에 하던 일이 흩어지면
    /// F5 를 안 누르게 되고, 그러면 낡은 화면을 본다.
    #[test]
    fn reloading_keeps_the_filter() {
        let mut a = app();
        a.key(key(KeyCode::Char('f')));
        typed(&mut a, "type=epic");
        assert_eq!(shown(&a).len(), 2);

        let mut more = a.issues.clone();
        more.push(make("argos-0007", Kind::Epic));
        a.adopt(more);
        assert_eq!(a.filter_text.as_deref(), Some("type=epic"));
        assert_eq!(shown(&a).len(), 3, "거름망이 새 자료에 다시 걸리지 않았다");
    }

    /// 진짜 파일을 두고 **바뀐 것을 알아채고 저절로 다시 읽는지** 본다. 고친 때만
    /// 보면 rename 으로 갈아끼우는 쓰기를 같은 초에 놓치므로 길이도 함께 본다.
    /// 다시 읽어도 커서는 보던 줄에 남는다.
    #[test]
    fn it_notices_a_changed_file_and_rereads_it() {
        let scratch = Scratch::new("reload");
        let dir = scratch.0.clone();
        let line = |i: &Issue| format!("{}\n", serde_json::to_string(i).unwrap());
        std::fs::write(dir.join(".moai/issues.jsonl"), line(&make("argos-0001", Kind::Epic))).unwrap();

        let repo = Repo { root: dir.clone(), config: cfg() };
        let stamp = stamp_of(&repo);
        let load = repo.read().unwrap();
        let index = Index::of(&load.issues);
        let mut a = App::open(repo, load, index, Path::new(), stamp);
        assert_eq!(a.issues.len(), 1);

        // 아직 아무도 안 건드렸다 — 읽지 않는다. 읽으면 표식이 같아도 매 걸음
        // 저장소를 통째로 다시 세는 것이다.
        let stamp_before = a.stamp;
        a.follow();
        assert_eq!(a.stamp, stamp_before);

        // 에픽 안에 들어가 있는 동안 밖에서 한 줄 더한다
        a.key(key(KeyCode::Enter));
        let mut src = std::fs::read_to_string(dir.join(".moai/issues.jsonl")).unwrap();
        src.push_str(&line(&member("argos-0004", "argos-0001")));
        std::fs::write(dir.join(".moai/issues.jsonl"), src).unwrap();

        a.follow();
        assert_eq!(a.issues.len(), 2, "바뀐 것을 저절로 안 읽었다");
        assert_eq!(a.path, [Seg::Epic("argos-0001".into())], "읽고 나서 자리를 잃었다");
        assert!(a.trouble.is_none());
    }

    /// **다시 읽어도 커서는 보던 줄에 선다.** 위에 줄이 생기거나 사라져도, 칸이
    /// 바뀌어 차례가 달라져도 번호가 아니라 그 줄을 따라간다. 보던 줄이 사라지면
    /// 목록 밖으로 나가지 않는 이웃 자리에 선다.
    #[test]
    fn reloading_keeps_the_cursor_on_the_same_line() {
        let mut a = app();
        a.key(key(KeyCode::Enter)); // argos-0001 안 — `..`, 0003, 0004
        a.key(key(KeyCode::End));
        let at = |a: &App| match a.current() {
            Some(Row::Item(e)) => a.issues[e.at().unwrap()].id.clone(),
            other => format!("{other:?}"),
        };
        assert_eq!(at(&a), "argos-0004");

        // 위에 줄이 하나 생긴다 — 같은 번호는 이제 0003 이다.
        let mut more = a.issues.clone();
        more.push(member("argos-0000", "argos-0001"));
        a.adopt(more);
        assert_eq!(at(&a), "argos-0004", "위에 줄이 생기자 커서가 옆 줄로 튀었다");

        // 위의 줄이 사라진다.
        let fewer: Vec<Issue> = a.issues.iter().filter(|i| i.id != "argos-0000" && i.id != "argos-0003").cloned().collect();
        a.adopt(fewer);
        assert_eq!(at(&a), "argos-0004", "위의 줄이 사라지자 커서가 튀었다");

        // 보던 줄이 사라지면 목록 안의 이웃 자리로.
        let gone: Vec<Issue> = a.issues.iter().filter(|i| i.id != "argos-0004").cloned().collect();
        a.adopt(gone);
        assert!(a.cursor < a.rows().len(), "목록 밖에 섰다");

        // `..` 에 서 있으면 `..` 에 남는다.
        let mut b = app();
        b.key(key(KeyCode::Enter));
        let mut more = b.issues.clone();
        more.push(member("argos-0000", "argos-0001"));
        b.adopt(more);
        assert_eq!(b.current(), Some(Row::Up));
    }

    /// 다시 읽어도 **같은 줄을 보고 있으면 굴린 자리를 둔다.** 보던 줄이 사라져
    /// 다른 줄에 서면 첫 줄부터 보인다.
    #[test]
    fn reloading_keeps_the_scroll_only_on_the_same_line() {
        let mut a = app();
        a.key(key(KeyCode::Enter));
        a.key(key(KeyCode::End)); // argos-0004
        a.scroll = 7;
        let mut more = a.issues.clone();
        more.push(member("argos-0000", "argos-0001"));
        a.adopt(more);
        assert_eq!(a.scroll, 7, "같은 줄인데 굴린 자리를 잃었다");

        let gone: Vec<Issue> = a.issues.iter().filter(|i| i.id != "argos-0004").cloned().collect();
        a.adopt(gone);
        assert_eq!(a.scroll, 0, "다른 줄에 섰는데 굴린 자리가 남았다");
    }

    /// 겹쳐 보는 동안에는 **옆 워크트리 스냅샷이 바뀐 것도** 다시 읽을 까닭이다.
    /// 안 바뀌었으면 읽지 않는다.
    #[test]
    fn a_change_in_a_watched_worktree_snapshot_rereads_too() {
        let scratch = Scratch::new("watched");
        let dir = scratch.0.clone();
        std::fs::write(dir.join(".moai/issues.jsonl"), "").unwrap();
        let repo = Repo { root: dir.clone(), config: cfg() };
        let stamp = stamp_of(&repo);
        let load = repo.read().unwrap();
        let index = Index::of(&load.issues);
        let mut a = App::open(repo, load, index, Path::new(), stamp);

        let other = dir.join("other.jsonl");
        std::fs::write(&other, "").unwrap();
        a.watched = vec![(other.clone(), crate::store::stamp(&other))];
        a.now = "읽기 전".into();
        a.follow();
        assert_eq!(a.now, "읽기 전", "아무것도 안 바뀌었는데 다시 읽었다");

        std::fs::write(&other, "{}\n").unwrap();
        a.follow();
        assert_ne!(a.now, "읽기 전", "옆 스냅샷이 바뀐 것을 못 알아챘다");
    }

    /// 빈 디렉터리에서도 무너지지 않는다.
    #[test]
    fn an_empty_repo_is_safe() {
        let mut a = App::new(Vec::new(), cfg(), Path::new());
        assert!(a.rows().is_empty());
        assert_eq!(a.current(), None);
        for k in [KeyCode::Down, KeyCode::Enter, KeyCode::Backspace, KeyCode::End] {
            a.key(key(k));
        }
        assert_eq!(a.cursor, 0);
    }
}
