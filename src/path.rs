//! **자리와 경로를 푸는 자들.** 한 자리를 가리키는 여러 철자를 한 철자로 모으고, `..` 을
//! 접고, 아직 없는 파일의 자리를 고른다.
//!
//! **저장소를 만지지 않는다** — 여기 있는 자는 경로 하나를 받아 경로 하나를 낸다. 그래서
//! `store`(스냅샷을 읽고 쓴다)·`hook`(툴 부름을 판정한다)·`user_config`·`tz` 가 다 같이
//! 부를 수 있고, 어느 쪽에 얹어도 나머지가 그 모듈에 딸리게 되는 자리였다(moai-xvyz).
//!
//! **자리를 푸는 자는 셋이다**(moai-i7b6) — 통째로 푸는 [`real`], 글자로만 접는 [`lexical`],
//! 아직 있는 윗자리까지만 푸는 [`real_prefix`]. 넷째로 보이지만 아닌 것이 둘 있고, 둘 다
//! **일부러 제 몸으로 든다.**
//!
//! - `crate::scratch`(시험에만 선다) 의 `real` 은 [`real`] 과 같은 글인데, 그 파일을 `tests/cli.rs` 가
//!   `#[path]` 로 함께 들어 그쪽에서는 `crate` 가 시험 크레이트라 이 모듈이 없다
//! - [`crate::user_config`] 의 `resolve_config` 는 링크의 사슬을 제 손으로 따라가고,
//!   가리키는 자리의 **부모가 없으면 일부러 탈로 낸다** — 여기 셋은 못 풀면 받은 철자로
//!   떨어진다. 묻는 것이 "이 자리가 어디인가" 가 아니라 "여기에 써도 되는가" 라 답이
//!   갈리는 자리다. 모으려 들면 그 거절이 사라진다
//!
//! [`dir_of`] 는 넷째 푸는 자가 아니다 — 아무것도 안 풀고 철자를 한 조각 뗄 뿐이다. 여기 사는
//! 것은 그 물음도 "경로 하나를 받아 경로 하나를 낸다" 라서고, 세는 자리에는 안 든다(리뷰).

use std::path::{Path, PathBuf};

/// 같은 자리를 가리키는 철자를 **하나로 모은다** — 링크와 `..`·끝 `/` 를 걷는다. **못 풀면 받은
/// 철자 그대로다.**
///
/// **떨어지는 자를 하나로 둔다**(moai-8csx). 이 여섯 줄이 저마다 적혀 있었다 —
/// `worktree::canonical`, `cmd::hook` 의 `picks_dir`, `cmd::skill` 의 `which` 와 `same_dir`,
/// `scratch` 의 `Scratch::real`·`inside_checkout`, 그리고 `cmd::merge_driver` 의 `chosen_command`
/// (목록에서 빠져 있던 여섯째다, 리뷰). 나중에 정할 것(윈도의 `\\?\` 접두어를 걷을 것인가, 못 푼
/// 자리를 어떻게 셀 것인가)이 여섯 중 하나에만 닿으면 나머지 다섯은 옛 답을 낸다.
///
/// **까닭을 대야 하는 자리는 따로 선다**([`crate::read_marks::settle`]). 거기는 못 푼 까닭이
/// 사람에게 가는 값이라 [`Settled`](crate::read_marks::Settled) 로 갈라 내고, 여기는 **자리를 못
/// 고르는 것보다 받은 철자가 낫다** 는 쪽이다(moai-f5e3, 사용자 결정 2026-09-20). 두 물음이 달라
/// 한 함수로 접지 않는다 — 접으면 부르는 쪽마다 `unwrap_or` 를 다시 적게 되어 지금 자리로 돌아온다.
///
/// 견주는 데 쓸 때는 [`crate::user_config::same_dir`] 이 이것의 짝이다 — 둘 다 못 풀면 받은 철자로
/// 견준다.
///
/// **아직 없는 파일을 판정하는 자리는 [`real_prefix`] 를 쓴다** — 이것은 거기서 실패해 준 철자를
/// 그대로 돌려준다. 셋이 어떻게 갈리고 넷째로 보이는 것이 왜 아닌지는 [모듈 머리글](self) 에 한 번
/// 적혀 있다. 여기 다시 적었더니 두 벌이 서로 다른 수를 대기 시작했다(리뷰).
pub(crate) fn real(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// `.` 을 버리고 `..` 은 앞 조각을 뗀다. **파일 시스템을 안 본다** — 아직 없는 자리도 접어야 하고
/// (훅은 이제 만들 파일을 판정한다), 접는 값이 얻는 값보다 크면 안 된다(시간대 이름 하나).
///
/// **뿌리 위로는 못 올라간다**(`/..` 은 `/`). 이 줄은 뜻을 글로 못박은 것이지 고침이 아니다 —
/// `PathBuf::pop` 은 뿌리에서 이미 아무것도 안 한다(`Path::parent` 가 `None` 이라 `false` 를 내고
/// 버퍼를 안 건드리고, 빈 경로도 같다). 걷어도 답은 한 자도 안 바뀐다.
///
/// **절대 경로를 받는다.** 상대 철자에서 위로 넘치는 `..` 은 남지 않고 사라진다(`a/../../b` 는
/// `b`) — 부르는 쪽이 먼저 뿌리나 `cwd` 를 붙인다. 지금 부르는 셋이 다 그렇게 한다.
///
/// **세 벌이 저마다 적혀 있던 것을 모았다**(moai-i7b6) — `hook::resolve` 의 속, `user_config` 의
/// `lexical`, `tz` 의 `flatten`. 막음을 글로 든 것은 `user_config` 뿐이었지만 **셋이 같은 답을
/// 냈다** — 리뷰가 1,296가지 꼴을 훑어 다른 답을 하나도 못 찾았다. 갈려 보이던 것은 글뿐이라,
/// 모으면서 고쳐진 버릇은 없다.
pub(crate) fn lexical(p: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if !matches!(out.components().next_back(), None | Some(Component::RootDir | Component::Prefix(_))) {
                    out.pop();
                }
            }
            other => out.push(other),
        }
    }
    out
}

/// **아직 있는 가장 깊은 조상을 풀고 남은 조각을 그대로 붙인다.** 못 풀면 받은 철자 그대로다
/// ([`real`] 과 같은 쪽 — 자리를 못 고르는 것보다 받은 철자가 낫다).
///
/// [`real`] 이 통째로 `canonicalize` 하는 것과 갈리는 자리는 **없는 파일**이다. 통째 풀기는 거기서
/// 실패해 준 철자를 그대로 돌려주는데, 조상에 링크가 있으면(`TMPDIR` 이 링크인 기계, macOS 의
/// `/tmp`·`/var`, 링크로 건 프로젝트) 그 철자는 이미 풀린 철자와 안 맞는다. 그래서 새로 만드는
/// 파일과 이미 있는 파일이 서로 다른 답을 받는다 — 훅의 규칙 2 는 만드는 쪽에서만 꺼지고, 등록
/// 목록에서 빼는 길은 사라진 디렉터리를 못 찾는다.
///
/// **`..` 이 없는 경로를 받는다**([`lexical`] 을 먼저 지난다). 남은 조각에 `..` 이 있으면 링크를
/// 푼 뒤의 뜻이 달라진다.
///
/// **두 벌이던 것을 모았다**(moai-i7b6) — `hook::settled`(지금 [`crate::hook`] 의 `real_path`)
/// 와 `user_config::real_prefix` 가 같은 알고리즘을 저마다 적고 있었다(리뷰 moai-1upp.nwq 가 두
/// 벌을 돌려 모든 표본에서 같은 답을 봤다). **둘이 꼭 같지는 않았다** — 저쪽은 `Option` 을 내
/// `..` 이 남은 경로를 거절했고, 그 자리는 위의 `debug_assert` 가 이어받는다.
pub(crate) fn real_prefix(p: &Path) -> PathBuf {
    // **글로만 둔 조건은 다음 부르는 이가 못 본다.** 세 모듈이 함께 쓰는 자가 된 뒤로 어기는 값이
    // 조용히 틀린 답이라, 시험에서는 여기서 멈춘다(내보내는 판에는 이 줄이 없다).
    debug_assert!(
        !p.components().any(|c| c == std::path::Component::ParentDir),
        "`..` 가 남은 채로 왔다 ({}) — [`lexical`] 을 먼저 지나야 한다",
        p.display()
    );
    for head in p.ancestors() {
        if let Ok(real) = std::fs::canonicalize(head) {
            // **떼어 내기는 실패하지 않는다** — `head` 는 `p` 의 조상이다. 이것을 `if let` 으로 받아
            // 넘기면 못 뗀 자리에서 한 칸 더 짧은 조상으로 내려가, 엉뚱한 윗자리로 푼 경로를 아무 말
            // 없이 답으로 낸다.
            let rest = p.strip_prefix(head).expect("조상에서 떼어 낸다");
            // **남은 조각이 비면 붙이지 않는다.** `real.join("")` 은 끝에 가름선만 더한다 — `Path`
            // 의 견주기와 `strip_prefix` 는 조각으로 돌아 안 걸리지만 `display`·`to_str` 는 갈리고,
            // [`crate::user_config::spellings`] 는 그 값을 목록에 실어 내보낸다. 모으기 전의
            // `user_config::real_prefix` 는 안 붙였으니, 붙이면 그것만 말없이 달라진다.
            return if rest.as_os_str().is_empty() { real } else { real.join(rest) };
        }
    }
    p.to_path_buf()
}

/// 파일이 든 디렉터리. 디렉터리 조각이 없는 상대 철자(`config.toml`)면 `.` 이다.
pub(crate) fn dir_of(path: &Path) -> &Path {
    path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."))
}

/// **재는 자는 재는 것 곁에 선다**(리뷰 moai-47yo.era). 이 셋을 여기로 옮기던 날(moai-xvyz) 재는
/// 시험은 `store` 에 남아, `store` 에 남긴 다리 한 줄을 타고서야 이름이 풀렸다 — 리뷰가 그것을
/// 짚어 시험도 이리로 왔다. 그 덕에 다리를 걷는 날(moai-fnd0) 그것이 지고 있던 것은
/// `src/hook.rs`·`src/cmd/hook.rs` 뿐이었다. 거기 선 부름 셋과 그것을 가리키던 글 다섯이
/// `crate::path::` 를 바로 들자, `store.rs` 에서 걷을 것은 그 한 줄뿐이었다.
///
/// **다음에 옮기는 이도 시험을 함께 옮긴다.** 재는 자를 옛 모듈에 두고 오면 옮긴 자리와 아무
/// 상관 없는 모듈의 시험이 붉어지고, 걷는 이는 그것을 "부름을 덜 옮겼다" 로 읽는다.
///
/// **다 옮겼다는 증거를 컴파일에서만 찾지 않는다**(리뷰). 부름을 덜 옮기면 재수출을 지우는
/// 순간 컴파일이 떨어지지만, 글 속의 rustdoc 이음은 없는 이름을 가리켜도 `cargo doc` 의 경고
/// 한 줄로만 서고 그 경고는 CI 에 없다 — 걷는 날 글 다섯을 손으로 세어 옮긴 까닭이 그것이다.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    /// **글자로만 접는다** — 파일 시스템을 안 본다. 셋이 저마다 적고 있던 것을 여기로 모았으니
    /// (`hook::resolve`·`user_config::spellings`·`tz::name_under`), 재는 자도 여기 하나다.
    ///
    /// **재는 것은 뜻이지 고침이 아니다.** `/..` 이 `/` 로 서는 것은 `PathBuf::pop` 이 뿌리에서
    /// 이미 아무것도 안 해서이지, 모으면서 더한 막음 덕이 아니다 — 그 줄을 걷어도 이 네 줄은
    /// 그대로 푸르다. 그래도 세워 두는 까닭은 뜻이 바뀌면(std 가 아니라 이 함수가) 잡으라는 것이다.
    #[test]
    fn folding_dots_never_climbs_past_the_root() {
        assert_eq!(lexical(Path::new("/a/./b/../c")), PathBuf::from("/a/c"));
        assert_eq!(lexical(Path::new("/../a")), PathBuf::from("/a"));
        assert_eq!(lexical(Path::new("/..")), PathBuf::from("/"));
        // **절대 경로를 받는다.** 상대 철자에서 위로 넘치는 `..` 은 남지 않고 사라지므로
        // (`a/../../b` 는 `../b` 가 아니라 `b`), 부르는 쪽이 먼저 뿌리나 `cwd` 를 붙인다.
        assert_eq!(lexical(Path::new("a/../../b")), PathBuf::from("b"));
    }

    /// **아직 없는 파일도 있는 윗자리까지는 같게 풀린다.** 통째로 `canonicalize` 하면 없는
    /// 파일에서 실패해 준 철자가 그대로 나오고, 조상이 링크면 그 철자는 풀린 철자와 안 맞는다 —
    /// 훅의 규칙 2 가 **만드는 쪽에서만** 꺼지던 자리다.
    #[cfg(unix)]
    #[test]
    fn the_deepest_living_ancestor_is_the_one_that_resolves() {
        let s = Scratch::new("path-real-prefix");
        let real = s.join("real");
        std::fs::create_dir_all(real.join("src")).unwrap();
        std::os::unix::fs::symlink(&real, s.join("link")).unwrap();
        let here = std::fs::canonicalize(&real).unwrap();

        assert_eq!(real_prefix(&s.join("link/src")), here.join("src"));
        // 아직 없는 파일도 같은 자리다.
        assert_eq!(real_prefix(&s.join("link/src/새파일.rs")), here.join("src/새파일.rs"));
        // **통째로 풀린 자리에 가름선을 안 붙인다.** `real.join("")` 은 끝에 `/` 를 더하는데,
        // `PathBuf` 의 견주기는 조각으로 돌아 그것을 **못 잡는다** — 그래서 글자로 잰다.
        // [`crate::user_config::spellings`] 가 이 값을 목록에 실어 내보낸다.
        assert_eq!(real_prefix(&s.join("link")).as_os_str(), here.as_os_str(), "끝에 가름선이 붙었다");
        // 조상이 하나도 안 풀리면 받은 철자 그대로다 — 자리를 못 고르는 것보다 낫다(`real` 과
        // 같은 쪽). 절대 경로에서는 뿌리가 늘 풀리므로 그 판은 상대 철자에서만 선다.
        //
        // **머리는 이 시험의 제 자리 이름에서 딴다.** 아무 이름이나 적으면 그 이름의 디렉터리가
        // cwd 에 선 기계에서 조상이 풀려 답이 뒤집힌다 — 시험이 도는 자리는 아무도 안 정한다.
        let missing = PathBuf::from(s.path().file_name().expect("자리에 이름이 있다")).join("x");
        assert_eq!(real_prefix(&missing), missing);
    }

    /// **디렉터리 조각이 없으면 `.` 이지 빈 철자가 아니다.** 빈 철자를 내면 곁에 락을 거는 자리가
    /// (`store::lock_beside`) 뿌리로 미끄러진다.
    #[test]
    fn a_file_with_no_directory_part_sits_in_the_here() {
        assert_eq!(dir_of(Path::new("config.toml")), Path::new("."));
        assert_eq!(dir_of(Path::new("a/config.toml")), Path::new("a"));
        assert_eq!(dir_of(Path::new("/a/config.toml")), Path::new("/a"));
    }
}
