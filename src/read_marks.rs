//! 적어 둔 읽음 — **프로젝트마다 제 파일**(moai-f31d, 사용자 결정 2026-09-19).
//!
//! 이슈 id → 마지막으로 본 줄의 `updated_at`(moai-50mn·moai-lyc1). 한때 사용자 설정 한 파일의 `[read]`
//! 표에 모두 모았는데, 그 자리가 둘을 깨뜨렸다.
//!
//! - **키가 이슈 id 뿐이라 저장소가 섞였다**(moai-omx7). 접두어는 디렉터리 이름에서 오고 뿌리 몸통은
//!   base36 넉 자라, 이름이 같은 디렉터리 둘은 같은 공간에서 id 를 뽑는다 — 각 300줄이면 5% 남짓
//!   겹쳐, 한쪽에서 읽은 것이 다른 저장소의 같은 id 를 말없이 읽음으로 만들었다
//! - **표가 안 줄어 설정 쓰기마다 느려졌다**(moai-dt5q). 보기 토글·`project add` 처럼 읽음과 아무
//!   상관 없는 쓰기도 그 표를 통째로 읽고 되썼다 — 1만 항목에 35~100ms
//!
//! 프로젝트마다 파일을 가르면 둘이 함께 풀린다. 파일이 갈리니 키는 지금처럼 이슈 id 로 두고, 설정
//! 쓰기는 이 파일을 아예 안 만진다.
//!
//! **트래커에 안 쓴다는 옛 결정은 그대로다**(사용자 결정 2026-09-15) — 읽음은 사람마다 다른 값이라
//! `.moai/issues.jsonl` 에 적으면 읽기만 해도 남과 부딪히고, 남의 읽음이 내 diff 에 섞인다. 자리가
//! 설정 한 파일에서 그 곁의 디렉터리로 바뀔 뿐이다.
//!
//! **옛 `[read]` 는 겹쳐 보기만 한다**(사용자 결정 3) — 옮기지 않는다. 그 표는 이슈 id 로만 키를 잡으니
//! 위의 섞임이 **옛 줄에 한해** 그대로 남는다. 새로 적는 것은 모두 제 파일로 가므로 섞임은 자라지 않지만,
//! 사라지지도 않는다 — 걷으려면 그것은 설정이 아니라 마이그레이션이라, 그 값을 따로 물어야 한다.
//!
//! **락과 쓰기는 [`crate::user_config::update`] 보다 단순하다.** 그쪽은 설정 **파일** 자체가 심볼릭
//! 링크인 경우(dotfiles 가 흔히 그렇게 건다)를 풀어야 하는데, 이 파일은 도구가 짓는 것이라 링크일 수
//! 없다. 설정 디렉터리가 링크인 것은 커널이 알아서 지나가므로 여기서 풀 것이 없다. 문서 자체는
//! [`crate::user_config::Doc`] 을 그대로 쓴다([`Sheet`]) — 바이트를 지키는 자를 둘로 두지 않는다.

use crate::fail::{Fail, R};
use crate::store::{Lock, dir_of, lock_beside, write_atomic};
use crate::user_config::{Doc, write_value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use toml_edit::{Item, Table};

/// 읽음이 사는 표의 이름. 옛 `[read]` 와 같은 낱말이다 — 자리가 바뀌었을 뿐 뜻은 그대로다.
const READ: &str = "read";
/// 이 파일이 어느 프로젝트의 것인지 적는 키([`path_for`] 의 해시가 부딪혔는지 여기서 본다).
const PATH: &str = "path";

/// 그 프로젝트의 읽음 파일 — `<설정 디렉터리>/read/<뿌리 해시>.toml`.
///
/// **설정 곁에 둔다.** `MOAI_CONFIG` 가 가리키는 자리를 따라가므로 시험과 격리가 설정 하나만 옮기면
/// 읽음도 함께 옮겨 간다. 자리를 따로 정하면 그 둘이 갈려, 시험이 제 임시 설정을 주고도 돌리는 사람의
/// 읽음을 읽는다.
///
/// **이름은 뿌리 경로의 해시다.** 경로를 그대로 파일 이름에 넣으면 깊은 경로가 이름 길이 상한을 넘고,
/// 글자를 걷어 짧게 만들면 서로 다른 뿌리가 한 이름으로 모여 지금 고치는 바로 그 섞임이 돌아온다.
/// 해시가 부딪혔는지는 파일 안의 `path` 로 본다([`Sheet::owns`]) — 글자로 적어 두고 견준다.
///
/// 셈은 [`crate::text::fnv1a64`] 다. **표준 해셔를 안 쓴다** — `DefaultHasher` 는 러스트 버전마다 값이
/// 달라질 수 있다고 문서가 밝혀, 올리기 한 번에 모든 사람의 읽음 파일 이름이 바뀐다. 상수와 그 까닭은
/// 이미 `text` 에 한 벌 있어 여기 다시 적지 않는다(moai-2vrw 가 못박은 그 값이다, 리뷰).
pub fn path_for(config: &Path, root: &Path) -> PathBuf {
    dir_of(config).join("read").join(format!("{:016x}.toml", crate::text::fnv1a64(root.as_os_str().as_encoded_bytes())))
}

/// 그 프로젝트의 읽음. **실패하지 않는다** — 못 읽거나 깨졌으면 빈 표와 까닭 한 줄이다. 읽음 하나
/// 때문에 제 저장소를 보던 명령이 넘어지면 도구가 고장 난 것으로 보인다([`crate::user_config::read`] 와
/// 같은 자다).
///
/// **옛 `[read]` 를 겹쳐 본다**(사용자 결정 3, 2026-09-19). 옮기지 않고 읽기만 한다 — 옛 줄을 건드리면
/// 그것은 설정이 아니라 마이그레이션이고, 그 사이 도는 옛 바이너리나 옆 세션이 읽음을 잃는다. 겹칠
/// 때는 **이 파일이 이긴다**: 옛 표는 이 바이너리가 다시 안 적는 지나간 값이고, 새 자리의 값이 그 뒤에
/// 적힌 것이다.
pub fn read(config: &Path, root: &Path, legacy: &BTreeMap<String, String>) -> (BTreeMap<String, String>, Vec<String>) {
    let path = path_for(config, root);
    let (mut marks, problems) = match std::fs::read_to_string(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (BTreeMap::new(), Vec::new()),
        Err(e) => (BTreeMap::new(), vec![format!("{}: {e}", path.display())]),
        Ok(src) => match Sheet::parse(&src) {
            Err(e) => (BTreeMap::new(), vec![format!("{}: {e}", path.display())]),
            Ok(sheet) if !sheet.owns(root) => (
                BTreeMap::new(),
                vec![format!("{}: {} 의 읽음이 아니라 안 읽는다 — 손으로 지운다", path.display(), root.display())],
            ),
            Ok(sheet) => sheet.marks(),
        },
    };
    overlay(&mut marks, legacy);
    (marks, problems)
}

/// 옛 `[read]` 를 겹친다 — **새 자리가 이긴다**(사용자 결정 3). 겹치는 자가 둘이 되면 그 규칙도 둘이
/// 되고, 옛 표를 걷는 날 한쪽만 걷힌다(리뷰).
///
/// 이미 있는 id 는 **키를 안 짓는다** — `entry` 로 넘기면 1만 항목짜리 옛 표가 읽기마다 버릴 문자열
/// 1만 개를 짓는다.
pub fn overlay(marks: &mut BTreeMap<String, String>, legacy: &BTreeMap<String, String>) {
    for (id, when) in legacy {
        if !marks.contains_key(id) {
            marks.insert(id.clone(), when.clone());
        }
    }
}

/// `[read]` 표를 **관대하게** 읽는다 — 낱말이 아닌 값은 까닭 한 줄로 대고 건너뛴다. 표가 없으면 빈 표다.
///
/// **읽는 자를 하나로 둔다**(리뷰) — 옛 `[read]`([`crate::user_config::Doc::read_marks`])와 이 파일이 같은
/// 모양이라, 둘이 저마다 읽으면 모양이 자라는 날 한쪽만 따라가고 그 한쪽은 남은 읽음을 조용히 버린다.
pub(crate) fn read_table(root: &Table) -> (BTreeMap<String, String>, Vec<String>) {
    let mut problems = Vec::new();
    let Some(item) = root.get(READ) else {
        return (BTreeMap::new(), problems);
    };
    let Some(t) = item.as_table_like() else {
        problems.push(format!("`{READ}` 는 `[{READ}]` 표여야 한다 — 지금은 {}", item.type_name()));
        return (BTreeMap::new(), problems);
    };
    let mut marks = BTreeMap::new();
    for (id, at) in t.iter() {
        match at.as_str() {
            Some(when) => {
                marks.insert(id.to_string(), when.to_string());
            }
            None => problems.push(format!("`{READ}.{id}` 는 때를 적은 낱말이어야 한다 — 지금은 {}", at.type_name())),
        }
    }
    (marks, problems)
}

/// 읽음을 고치는 **유일한 길**. 락 → 락 안에서 읽기 → 고치기 → 바뀌었으면 temp+rename.
/// [`crate::user_config::update`] 와 같은 모양이고 같은 까닭이다 — 둘이 동시에 적으면 나중에 `rename`
/// 한 쪽이 앞의 읽음을 조용히 지운다.
///
/// **깨진 파일에는 쓰지 않는다**(`broken`). 도구가 짓는 파일이라 깨질 일이 드물지만, 깨졌으면 사람이
/// 볼 수 있게 두고 멈춘다 — 덮으면 그 안의 읽음이 통째로 사라진다.
pub fn update<T>(config: &Path, root: &Path, f: impl FnOnce(&mut Sheet) -> R<T>) -> R<T> {
    let path = path_for(config, root);
    let dir = dir_of(&path);
    let err = |e: std::io::Error| Fail::new(format!("{}: {e}", path.display()));
    std::fs::create_dir_all(dir).map_err(|e| Fail::new(format!("{}: {e}", dir.display())))?;
    // 락 자리는 [`crate::store::lock_beside`] 가 센다 — 같은 디렉터리의 두 파일이 저마다 세면 자리
    // 규칙이 바뀌는 날 한쪽만 따라가 둘이 서로를 안 막는다(리뷰).
    let _lock = Lock::acquire(&lock_beside(&path))?;

    // 락을 잡은 **뒤에** 읽는다. 밖에서 읽으면 두 프로세스가 같은 옛 표를 고친다.
    let src = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(err(e)),
    };
    let mut sheet = Sheet::parse(&src)
        .map_err(|e| Fail::coded(format!("{}: {e} — 고치기 전까지 쓰지 않는다", path.display()), crate::fail::code::BROKEN))?;
    // **남의 읽음 위에 쓰지 않는다** — 해시가 부딪혔다. 재어 본 일이 없는 만큼 드문 자리지만, 조용히
    // 섞는 것이 이 에픽이 고치는 바로 그 해라 멈춘다.
    if !sheet.owns(root) {
        return Err(Fail::coded(
            format!("{}: {} 의 읽음이 아니라 쓰지 않는다 — 손으로 지운다", path.display(), root.display()),
            crate::fail::code::BROKEN,
        ));
    }
    // **손으로 고칠 거절에는 어느 파일인지 붙인다** — [`crate::user_config::update`] 와 같은 자리이고 같은
    // 까닭이다(리뷰). 이 파일의 이름은 뿌리의 해시라 사람이 짐작할 수 없어, 붙이지 않으면 "손으로
    // 고친다" 가 갈 곳 없는 말이 된다. 붙이는 곳을 부르는 쪽마다 두면 붙인 곳과 잊은 곳이 갈린다.
    let out = f(&mut sheet).map_err(|e| match e.code {
        crate::fail::code::BROKEN => Fail::coded(format!("{}: {}", path.display(), e.message), e.code),
        _ => e,
    })?;
    // **바뀐 것이 없으면 파일을 안 짓는다.** 어느 프로젝트의 것인지 적는 줄([`Sheet::claim`])도 그때
    // 함께 적는다 — 먼저 적던 판은 그 한 줄이 쓰기를 세워, 적을 것이 없는 `moai read` 하나가 아직
    // 아무것도 안 한 사람의 집에 빈 읽음 파일을 지었다.
    if sheet.changed() {
        sheet.claim(root);
        write_atomic(&path, sheet.render().as_bytes())?;
    }
    Ok(out)
}

/// 읽음 파일 하나. 모르는 키와 주석은 그대로 들고 간다 — 도구가 짓는 파일이어도 사람이 곁에 한 줄
/// 적어 둘 수 있고, 새 바이너리가 적은 키를 옛 바이너리가 한 번 만져 지우면 안 된다.
///
/// **[`Doc`] 을 그대로 두른다**(리뷰). 그쪽이 이미 바이트를 지킨다 — BOM, 줄바꿈 없이 끝난 파일,
/// CRLF(moai-r9qa·moai-lb0u), 값 뒤의 주석([`write_value`]), 주석만 있던 파일의 머리 주석
/// ([`Doc::new_table`]). 그 넷은 저마다 값을 치르고 고친 자리라, 문서를 새로 두르면 넷이 한꺼번에
/// 돌아온다. 갈라 두던 판이 실제로 그랬다.
pub struct Sheet {
    doc: Doc,
}

impl Sheet {
    fn parse(src: &str) -> Result<Sheet, String> {
        Ok(Sheet { doc: Doc::parse(src)? })
    }

    fn changed(&self) -> bool {
        self.doc.changed()
    }

    /// 이 파일이 그 뿌리의 것인가. `path` 가 없으면(처음 짓는 파일) 참이다 — 그때 [`Sheet::claim`] 이 적는다.
    ///
    /// **낱말이 아닌 `path` 는 남의 것으로 읽는다**(리뷰). 참으로 읽던 판은 [`Sheet::claim`] 이 그것을
    /// 안 덮는 것과 어긋나, `path = 3` 하나가 붙은 파일을 해시가 닿는 **모든** 뿌리가 제 것으로 여겨
    /// 함께 적었다 — 막으려던 섞임이 도리어 조용해졌다.
    fn owns(&self, root: &Path) -> bool {
        match self.doc.root().get(PATH) {
            None => true,
            Some(item) => item.as_str().is_some_and(|at| Path::new(at) == root),
        }
    }

    /// 어느 프로젝트의 것인지 적는다 — 없을 때만. 이미 적혀 있으면 [`Sheet::owns`] 가 같은 것을 보았다.
    ///
    /// **글자로 못 적는 뿌리는 안 적는다**(리뷰). `display()` 는 UTF-8 이 아닌 바이트를 U+FFFD 로 바꿔
    /// 적는데, [`Sheet::owns`] 는 그것을 바이트째 견줘 남의 것으로 읽는다 — 제가 방금 지은 파일을 다음
    /// 명령이 거절하고, 거절문은 그 파일을 손으로 지우라 하고, 지우면 같은 줄이 다시 적히는 고리였다.
    /// 이름의 해시는 바이트를 그대로 세므로 안 적어도 파일은 제 것이다.
    fn claim(&mut self, root: &Path) {
        if self.doc.root().get(PATH).is_some() || root.to_str().is_none() {
            return;
        }
        if write_value(self.doc.root_mut(), PATH, root.display().to_string().into()) {
            self.doc.touched();
        }
    }

    /// 적어 둔 읽음. **관대하게 읽는다** — 낱말이 아닌 값은 까닭 한 줄로 대고 건너뛴다
    /// ([`read_table`], 옛 `[read]` 와 같은 자).
    pub fn marks(&self) -> (BTreeMap<String, String>, Vec<String>) {
        read_table(self.doc.root())
    }

    /// 읽음을 적는다 — **준 id 만 손댄다**. 같은 때가 이미 적혀 있으면 아무것도 안 한다. 돌려주는 것은
    /// 실제로 바뀐 id 다(부르는 쪽이 "무엇을 적었나" 를 락 안에서 잰 그대로 댄다, moai-j038.vna).
    ///
    /// 준 id 의 자리에 때가 아닌 것이 있으면 **하나도 안 적는다** — 손으로 적은 맨 점 키(`a-0002.rv = …`)는
    /// `a-0002` 표 밑의 `rv` 로 읽히는데, 그 위에 때를 덮으면 자식의 읽음과 그 위 주석이 말없이 사라진다.
    ///
    /// **값만 바꾼다**([`write_value`], 리뷰) — 키 위의 주석·값 뒤의 주석·키 모양은 그대로다.
    /// `Table::insert` 로 갈아 끼우면 키를 새로 지어 셋이 다 사라진다(`Doc::set_hue` 가 적어 둔 그대로다).
    pub fn mark(&mut self, marks: &BTreeMap<String, String>) -> R<Vec<String>> {
        if marks.is_empty() {
            return Ok(Vec::new());
        }
        if let Some(item) = self.doc.root().get(READ) {
            let Some(t) = item.as_table_like() else {
                return Err(Fail::coded(
                    format!("`{READ}` 가 `[{READ}]` 표가 아니라({}) 읽음을 적지 않는다 — 손으로 고친다", item.type_name()),
                    crate::fail::code::BROKEN,
                ));
            };
            if let Some((id, odd)) = marks.keys().find_map(|id| t.get(id).filter(|v| v.as_str().is_none()).map(|v| (id, v))) {
                return Err(Fail::coded(
                    format!("`{READ}` 의 `{id}` 가 때가 아니라({}) 읽음을 적지 않는다 — 손으로 고친다", odd.type_name()),
                    crate::fail::code::BROKEN,
                ));
            }
        } else {
            // 주석만 있던 파일이면 머리 주석을 머리에 둔다 — 안 두면 사람이 적어 둔 줄이 `[read]` 밑으로
            // 밀려 마지막 읽음에 붙은 말로 읽힌다(moai-gmdu 에픽 리뷰가 설정에서 고친 그 자리다, 리뷰).
            let t = self.doc.new_table();
            self.doc.root_mut().insert(READ, Item::Table(t));
        }
        let t = self.doc.root_mut().get_mut(READ).and_then(Item::as_table_like_mut).expect("방금 표로 섰다");
        let mut written = Vec::new();
        for (id, when) in marks {
            if write_value(t, id, when.as_str().into()) {
                written.push(id.clone());
            }
        }
        if !written.is_empty() {
            self.doc.touched();
        }
        Ok(written)
    }

    /// **트래커에 없는 id 의 줄을 걷는다**(moai-dt5q, 사용자 결정 2 — 2026-09-19). 지운 이슈의 읽음은
    /// 아무도 다시 안 본다.
    ///
    /// **닫힌 줄은 안 걷는다.** 걷으면 그 줄이 다시 목록에 설 때 [NEW] 가 되살아나, 읽음의 뜻이
    /// "본 적 있다" 에서 "최근에 본 적 있다" 로 바뀐다.
    ///
    /// **때가 적힌 줄만 걷는다**(리뷰) — 손으로 적은 맨 점 키(`a-0002.rv = …`)는 `a-0002` 표로 읽히는데,
    /// 그 표를 지우면 자식의 읽음과 그 위 주석이 말없이 사라진다. [`Sheet::mark`] 가 그 자리를 안 덮는
    /// 것과 같은 자다 — 무엇인지 모르는 값은 읽기가 까닭을 대고 사람이 푼다.
    ///
    /// 읽음을 적는 그 자리에서만 부른다 — 거기는 트래커를 이미 들고 있다. 설정 쓰기가 트래커를 읽어야
    /// 하는 일을 안 만들자는 것이 이 자리의 까닭이다.
    pub fn prune(&mut self, known: &BTreeSet<&str>) -> usize {
        let Some(t) = self.doc.root_mut().get_mut(READ).and_then(Item::as_table_like_mut) else {
            return 0;
        };
        // 걷을 것만 짓는다 — 먼저 모두 베끼던 판은 걷을 것이 없는 흔한 판에서도 키 수만큼 문자열을 지었다.
        let gone: Vec<String> =
            t.iter().filter(|(id, at)| at.as_str().is_some() && !known.contains(id)).map(|(id, _)| id.to_string()).collect();
        for id in &gone {
            t.remove(id);
        }
        if !gone.is_empty() {
            self.doc.touched();
        }
        gone.len()
    }

    fn render(&self) -> String {
        self.doc.render()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    fn marks(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs.iter().map(|(a, b)| ((*a).to_string(), (*b).to_string())).collect()
    }

    /// **프로젝트마다 제 파일이다**(moai-omx7) — 이름이 같은 디렉터리 둘이 같은 id 를 써도 서로의
    /// 읽음을 안 민다. 그 섞임이 이 에픽을 연 까닭이다.
    #[test]
    fn two_projects_with_the_same_id_do_not_push_each_other() {
        let s = Scratch::new("read-marks-split");
        let cfg = s.join("config.toml");
        let (one, two) = (s.join("a/api"), s.join("b/api"));

        update(&cfg, &one, |sheet| sheet.mark(&marks(&[("argos-0001", "A")]))).unwrap();
        update(&cfg, &two, |sheet| sheet.mark(&marks(&[("argos-0001", "B")]))).unwrap();

        assert_eq!(read(&cfg, &one, &BTreeMap::new()).0, marks(&[("argos-0001", "A")]));
        assert_eq!(read(&cfg, &two, &BTreeMap::new()).0, marks(&[("argos-0001", "B")]));
        assert_ne!(path_for(&cfg, &one), path_for(&cfg, &two));
    }

    /// **설정 곁에 선다** — `MOAI_CONFIG` 를 옮기면 읽음도 따라간다. 자리를 따로 정하면 제 임시 설정을
    /// 준 시험이 돌리는 사람의 읽음을 읽는다.
    #[test]
    fn the_file_stands_beside_the_config_it_was_given() {
        let s = Scratch::new("read-marks-beside");
        let root = s.join("proj");
        for dir in ["one", "two"] {
            let cfg = s.join(dir).join("config.toml");
            assert!(path_for(&cfg, &root).starts_with(s.join(dir)), "{}", path_for(&cfg, &root).display());
        }
    }

    /// **옛 `[read]` 는 겹쳐 보고 안 지운다**(사용자 결정 3). 겹치면 새 자리가 이긴다 — 옛 표는 이
    /// 바이너리가 다시 안 적는 지나간 값이다.
    #[test]
    fn the_old_table_is_read_alongside_and_the_new_place_wins() {
        let s = Scratch::new("read-marks-legacy");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        update(&cfg, &root, |sheet| sheet.mark(&marks(&[("argos-0001", "새것")]))).unwrap();

        let legacy = marks(&[("argos-0001", "옛것"), ("argos-0009", "옛것뿐")]);
        let (seen, problems) = read(&cfg, &root, &legacy);
        assert_eq!(seen, marks(&[("argos-0001", "새것"), ("argos-0009", "옛것뿐")]));
        assert!(problems.is_empty(), "{problems:?}");
    }

    /// **트래커에 없는 id 만 걷는다**(moai-dt5q, 사용자 결정 2). 닫힌 줄은 남는다 — 걷으면 [NEW] 가
    /// 되살아나 읽음의 뜻이 바뀐다.
    #[test]
    fn pruning_drops_only_what_the_tracker_no_longer_has() {
        let s = Scratch::new("read-marks-prune");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        let all = marks(&[("argos-0001", "A"), ("argos-0002", "B"), ("argos-0003", "C")]);
        update(&cfg, &root, |sheet| sheet.mark(&all)).unwrap();

        let known: BTreeSet<&str> = ["argos-0001", "argos-0003"].into_iter().collect();
        let gone = update(&cfg, &root, |sheet| Ok(sheet.prune(&known))).unwrap();
        assert_eq!(gone, 1);
        assert_eq!(read(&cfg, &root, &BTreeMap::new()).0, marks(&[("argos-0001", "A"), ("argos-0003", "C")]));
    }

    /// **같은 때를 다시 안 적는다** — 헛 쓰기가 없고, 적은 id 만 돌려준다.
    #[test]
    fn writing_the_same_stamp_again_changes_nothing() {
        let s = Scratch::new("read-marks-idempotent");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        assert_eq!(update(&cfg, &root, |sh| sh.mark(&marks(&[("argos-0001", "A")]))).unwrap(), ["argos-0001"]);
        let was = std::fs::read_to_string(path_for(&cfg, &root)).unwrap();
        assert!(update(&cfg, &root, |sh| sh.mark(&marks(&[("argos-0001", "A")]))).unwrap().is_empty());
        assert_eq!(std::fs::read_to_string(path_for(&cfg, &root)).unwrap(), was, "같은 때를 다시 적어 파일이 바뀌었다");
        assert_eq!(update(&cfg, &root, |sh| sh.mark(&marks(&[("argos-0001", "B")]))).unwrap(), ["argos-0001"]);
    }

    /// **`.` 이 든 자식 id 는 따옴표에 싼 낱말 키로 적는다**(moai-j038.vna) — 맨 키로 적히면 다음 읽기가
    /// 점 찍은 키로 보아 `argos-0003` 표 밑의 `rv` 로 읽고, 그 줄의 읽음이 통째로 사라진다. 리뷰 이슈의
    /// id 가 늘 이 꼴이라 흔한 자리다.
    #[test]
    fn a_child_id_with_a_dot_is_written_as_one_quoted_key() {
        let s = Scratch::new("read-marks-dotted");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        update(&cfg, &root, |sh| sh.mark(&marks(&[("argos-0003.rv", "A")]))).unwrap();
        let text = std::fs::read_to_string(path_for(&cfg, &root)).unwrap();
        assert!(text.contains("\"argos-0003.rv\""), "맨 키로 적었다 — 다음 읽기가 못 찾는다\n{text}");
        assert_eq!(read(&cfg, &root, &BTreeMap::new()).0, marks(&[("argos-0003.rv", "A")]));
    }

    /// **다시 적는 줄의 주석과 키 모양도 그대로다**(리뷰) — `Table::insert` 로 갈아 끼우던 판은 키를 새로
    /// 지어 키 위의 주석·값 뒤의 주석·따옴표가 한꺼번에 사라졌다. 도장은 줄이 바뀔 때마다 다시 적히므로
    /// 드문 자리가 아니라 **늘 지나는 자리**다.
    #[test]
    fn re_marking_a_line_keeps_its_comments_and_key_shape() {
        let s = Scratch::new("read-marks-decor");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        let at = path_for(&cfg, &root);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        let src = format!(
            "path = {:?}\n\n[read]\n# 손으로 적은 까닭\n\"argos-0001\" = \"A\"  # 뒤 주석\n",
            root.display().to_string()
        );
        std::fs::write(&at, &src).unwrap();
        update(&cfg, &root, |sh| sh.mark(&marks(&[("argos-0001", "B")]))).unwrap();
        let now = std::fs::read_to_string(&at).unwrap();
        assert!(now.contains("# 손으로 적은 까닭"), "키 위의 주석이 사라졌다\n{now}");
        assert!(now.contains("\"argos-0001\" = \"B\"  # 뒤 주석"), "뒤 주석이나 따옴표가 사라졌다\n{now}");
    }

    /// **주석만 있던 파일의 머리 주석은 머리에 남는다**(moai-gmdu 에픽 리뷰가 설정에서 고친 자리, 리뷰) —
    /// 맨 `Table::new()` 로 표를 세우면 그 주석이 `[read]` 밑으로 밀려 마지막 읽음에 붙은 말로 읽힌다.
    #[test]
    fn a_head_comment_stays_above_the_first_read_table() {
        let s = Scratch::new("read-marks-head");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        let at = path_for(&cfg, &root);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(&at, "# 이 파일에 적어 둔 까닭\n").unwrap();
        update(&cfg, &root, |sh| sh.mark(&marks(&[("argos-0001", "A")]))).unwrap();
        let now = std::fs::read_to_string(&at).unwrap();
        assert!(now.starts_with("# 이 파일에 적어 둔 까닭\n"), "머리 주석이 표 밑으로 밀렸다\n{now}");
    }

    /// **줄 끝과 끝 줄바꿈은 원문대로 돌려준다**(moai-lb0u·moai-r9qa, 리뷰) — 라이브러리는 줄 끝을 `\n`
    /// 으로 접어, 안 되돌리면 CRLF 로 든 dotfiles 저장소에서 `moai read` 한 번이 모든 줄을 바꾼다.
    #[test]
    fn crlf_and_a_missing_final_newline_survive_a_write() {
        let s = Scratch::new("read-marks-crlf");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        let at = path_for(&cfg, &root);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(&at, format!("path = {:?}\r\n\r\n[read]\r\n\"argos-0001\" = \"A\"", root.display().to_string())).unwrap();
        update(&cfg, &root, |sh| sh.mark(&marks(&[("argos-0002", "B")]))).unwrap();
        let now = std::fs::read_to_string(&at).unwrap();
        assert!(!now.replace("\r\n", "").contains('\n'), "줄 끝이 LF 로 접혔다 — {now:?}");
        assert!(!now.ends_with('\n'), "없던 끝 줄바꿈을 더했다 — {now:?}");
    }

    /// **낱말이 아닌 `path` 는 남의 것으로 읽는다**(리뷰) — 참으로 읽던 판은 `claim` 이 그 자리를 안 덮는
    /// 것과 어긋나, `path = 3` 하나가 붙은 파일을 해시가 닿는 모든 뿌리가 제 것으로 여겨 함께 적었다.
    #[test]
    fn a_path_that_is_not_a_word_is_not_owned() {
        let s = Scratch::new("read-marks-odd-path");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        let at = path_for(&cfg, &root);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(&at, "path = 3\n\n[read]\n\"argos-0001\" = \"A\"\n").unwrap();
        assert!(read(&cfg, &root, &BTreeMap::new()).0.is_empty(), "남의 읽음을 들었다");
        let e = update(&cfg, &root, |sh| sh.mark(&marks(&[("argos-0002", "B")]))).unwrap_err();
        assert_eq!(e.code, crate::fail::code::BROKEN, "{e}");
    }

    /// **글자로 못 적는 뿌리도 제 읽음을 든다**(리뷰) — `display()` 가 U+FFFD 로 바꿔 적은 줄을 `owns` 가
    /// 바이트째 견주면, 제가 방금 지은 파일을 다음 명령이 남의 것으로 읽고 거절한다. 그 거절은 손으로
    /// 지우라 하는데, 지우면 같은 줄이 다시 적혀 고리가 안 끊긴다.
    #[cfg(unix)]
    #[test]
    fn a_root_that_is_not_utf8_can_still_write_twice() {
        use std::os::unix::ffi::OsStrExt;
        let s = Scratch::new("read-marks-nonutf8");
        let cfg = s.join("config.toml");
        let root = s.path().join(std::ffi::OsStr::from_bytes(b"re\xffpo"));
        update(&cfg, &root, |sh| sh.mark(&marks(&[("argos-0001", "A")]))).unwrap();
        update(&cfg, &root, |sh| sh.mark(&marks(&[("argos-0002", "B")]))).expect("제가 지은 파일을 남의 것으로 읽었다");
        assert_eq!(read(&cfg, &root, &BTreeMap::new()).0, marks(&[("argos-0001", "A"), ("argos-0002", "B")]));
    }

    /// **걷기도 때가 적힌 줄만 손댄다**(리뷰) — 손으로 적은 맨 점 키는 부모 id 의 표로 읽히는데, 부모가
    /// 트래커에 없다고 그 표를 지우면 자식의 읽음과 주석이 함께 사라진다. `mark` 가 그 자리를 안 덮는
    /// 것과 같은 자여야 한다.
    #[test]
    fn pruning_leaves_alone_what_marking_refuses_to_touch() {
        let s = Scratch::new("read-marks-prune-odd");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        let at = path_for(&cfg, &root);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        let src = format!("path = {:?}\n\n[read]\n\"argos-0002\".rv = \"A\"\n", root.display().to_string());
        std::fs::write(&at, &src).unwrap();
        let known: BTreeSet<&str> = BTreeSet::new();
        assert_eq!(update(&cfg, &root, |sh| Ok(sh.prune(&known))).unwrap(), 0, "때가 아닌 자리를 걷었다");
        assert_eq!(std::fs::read_to_string(&at).unwrap(), src, "걷을 것이 없는데 파일을 고쳤다");
    }

    /// **모르는 키와 주석은 그대로 간다** — 새 바이너리가 적은 것을 옛 바이너리가 한 번 만져 지우면 안 된다.
    #[test]
    fn unknown_keys_and_comments_survive_a_write() {
        let s = Scratch::new("read-marks-keep");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        let at = path_for(&cfg, &root);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        let src = format!("# 손으로 적은 줄\npath = {:?}\nnote = \"나중 바이너리의 키\"\n\n[read]\n\"argos-0001\" = \"A\"\n", root.display().to_string());
        std::fs::write(&at, &src).unwrap();
        update(&cfg, &root, |sh| sh.mark(&marks(&[("argos-0002", "B")]))).unwrap();
        let now = std::fs::read_to_string(&at).unwrap();
        assert!(now.contains("# 손으로 적은 줄"), "{now}");
        assert!(now.contains("note = \"나중 바이너리의 키\""), "{now}");
        assert_eq!(read(&cfg, &root, &BTreeMap::new()).0, marks(&[("argos-0001", "A"), ("argos-0002", "B")]));
    }

    /// **남의 읽음 위에 쓰지 않는다** — 해시가 부딪히면 읽기는 빈 표와 까닭 한 줄, 쓰기는 멈춘다.
    /// 조용히 섞는 것이 이 에픽이 고치는 바로 그 해다.
    #[test]
    fn a_sheet_that_belongs_to_another_root_is_never_touched() {
        let s = Scratch::new("read-marks-collision");
        let cfg = s.join("config.toml");
        let (mine, other) = (s.join("proj"), s.join("남의 것"));
        let at = path_for(&cfg, &mine);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(&at, format!("path = {:?}\n\n[read]\n\"argos-0001\" = \"A\"\n", other.display().to_string())).unwrap();

        let (seen, problems) = read(&cfg, &mine, &BTreeMap::new());
        assert!(seen.is_empty(), "남의 읽음을 들었다 — {seen:?}");
        assert_eq!(problems.len(), 1, "{problems:?}");
        let e = update(&cfg, &mine, |sh| sh.mark(&marks(&[("argos-0002", "B")]))).unwrap_err();
        assert_eq!(e.code, crate::fail::code::BROKEN, "{e}");
        assert!(std::fs::read_to_string(&at).unwrap().contains("argos-0001"), "남의 파일을 덮었다");
    }

    /// **깨진 파일에는 안 쓴다** — 읽기는 까닭을 대고 빈 표로 지나간다.
    #[test]
    fn a_broken_sheet_is_told_on_read_and_refused_on_write() {
        let s = Scratch::new("read-marks-broken");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        let at = path_for(&cfg, &root);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(&at, "[read\n\"argos-0001\" = ").unwrap();
        assert!(read(&cfg, &root, &BTreeMap::new()).0.is_empty());
        assert_eq!(read(&cfg, &root, &BTreeMap::new()).1.len(), 1);
        let e = update(&cfg, &root, |sh| sh.mark(&marks(&[("argos-0001", "A")]))).unwrap_err();
        assert_eq!(e.code, crate::fail::code::BROKEN, "{e}");
    }

    /// **때가 아닌 값이 있으면 하나도 안 적는다** — 맨 점 키(`a-0002.rv = …`)는 표로 읽히는데, 그 위에
    /// 때를 덮으면 자식의 읽음과 주석이 말없이 사라진다.
    #[test]
    fn a_value_that_is_not_a_stamp_stops_the_whole_write() {
        let s = Scratch::new("read-marks-odd");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        let at = path_for(&cfg, &root);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        let src = format!("path = {:?}\n\n[read]\n\"argos-0002\".rv = \"A\"\n", root.display().to_string());
        std::fs::write(&at, &src).unwrap();
        let e = update(&cfg, &root, |sh| sh.mark(&marks(&[("argos-0002", "B")]))).unwrap_err();
        assert_eq!(e.code, crate::fail::code::BROKEN, "{e}");
        assert_eq!(std::fs::read_to_string(&at).unwrap(), src, "거절하고도 파일을 고쳤다");
    }

    /// **적을 것이 없으면 파일을 안 만든다** — 빈 쓰기 하나 때문에 아직 아무것도 안 한 사람의 집에
    /// 디렉터리와 락 파일이 생긴다.
    #[test]
    fn an_empty_write_leaves_no_file_behind() {
        let s = Scratch::new("read-marks-empty");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        update(&cfg, &root, |sh| sh.mark(&BTreeMap::new())).unwrap();
        assert!(!path_for(&cfg, &root).exists(), "빈 쓰기가 파일을 지었다");
    }

    /// **동시에 적어도 서로를 안 지운다**(CLAUDE.md — 조용한 손실이 이 도구가 못 견디는 유일한 실패다).
    /// 락 없이 읽고 되쓰면 나중에 `rename` 한 갈래가 앞의 읽음을 통째로 덮는다.
    #[test]
    fn concurrent_marks_all_survive() {
        let s = Scratch::new("read-marks-concurrent");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        let (threads, each) = (8, 5);
        std::thread::scope(|scope| {
            for t in 0..threads {
                let (cfg, root) = (cfg.clone(), root.clone());
                scope.spawn(move || {
                    for i in 0..each {
                        let id = format!("argos-{t}{i:02}");
                        update(&cfg, &root, |sh| sh.mark(&marks(&[(&id, "A")]))).unwrap();
                    }
                });
            }
        });
        assert_eq!(read(&cfg, &root, &BTreeMap::new()).0.len(), threads * each, "동시에 적은 읽음이 서로를 지웠다");
    }

    /// **읽고 그대로 쓰면 바이트가 같다** — 헛 diff 가 없다. 도구가 짓는 파일이어도 dotfiles 저장소에
    /// 들어가면 사람이 그 diff 를 본다.
    #[test]
    fn reading_and_writing_back_changes_no_bytes() {
        let s = Scratch::new("read-marks-idem-bytes");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        update(&cfg, &root, |sh| sh.mark(&marks(&[("argos-0001", "A"), ("argos-0002", "B")]))).unwrap();
        let was = std::fs::read_to_string(path_for(&cfg, &root)).unwrap();
        let known: BTreeSet<&str> = ["argos-0001", "argos-0002"].into_iter().collect();
        update(&cfg, &root, |sh| {
            sh.mark(&marks(&[("argos-0001", "A")]))?;
            Ok(sh.prune(&known))
        })
        .unwrap();
        assert_eq!(std::fs::read_to_string(path_for(&cfg, &root)).unwrap(), was);
    }

    /// **이름은 안 바뀐다** — 바뀌면 모든 사람의 읽음이 한 번에 사라진다. 셈은 [`crate::text::fnv1a64`]
    /// 가 제 시험값으로 못박으므로, 여기서는 **그 셈에 매였다는 것**을 이름으로 못박는다.
    #[test]
    fn the_hash_is_pinned_so_the_names_never_move() {
        let name = |root: &str| path_for(Path::new("/c/config.toml"), Path::new(root)).file_name().unwrap().to_owned();
        assert_eq!(name("/a/api"), "c812cb6e42a00af2.toml");
        assert_ne!(name("/a/api"), name("/b/api"));
        assert_eq!(path_for(Path::new("/c/config.toml"), Path::new("/a/api")).parent().unwrap(), Path::new("/c/read"));
    }
}
