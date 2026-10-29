//! 사람의 `[writing]` 표(얼마나 자세히 쓸지)를 읽는다. 저장하거나 트래커를 열지 않는다(moai-d1zn).

use super::{Ctx, R};

pub fn run(ctx: &Ctx) -> R<Vec<String>> {
    let reg = ctx.registry();
    let lang = ctx.lang();
    // **저장소의 표는 권하는 값이다**(moai-w25x.jud) — 기본값을 덮고 사용자 설정에 진다. 설정을 읽는 것뿐이라
    // 트래커 밖에서도, 저장소 설정이 깨진 곳에서도 사람의 표는 그대로 선다 — 깨진 설정은 다른 명령이 댄다.
    let repo = crate::store::Repo::find(|| lang).ok().flatten();
    let (writing, repo_problems) = match &repo {
        Some(r) => reg.writing.with_repo(&r.config.writing),
        None => (reg.writing.clone(), Vec::new()),
    };
    // **조각마다 잇는다**(moai-lag7.951) — `.moai/config.toml` 을 통째로 이으면 Windows 에서 가름선이 섞인 철자가 선다.
    let repo_file = repo.as_ref().map(|r| r.dir().join("config.toml"));
    let repo_path = repo_file.as_deref().filter(|_| !writing.repo.is_empty());
    // 파일 자체의 탈과 키의 탈은 따로 든다. 성한 키는 살리고 종료 0으로 알린다.
    for line in crate::view::settings_problems(reg, lang).into_iter().chain(crate::view::writing_problems(reg, lang)) {
        super::tell(&line);
    }
    if let Some(file) = &repo_file {
        let at = crate::text::one_line(&file.display().to_string());
        for why in &repo_problems {
            super::tell(&format!("{at}: {}", crate::view::writing_trouble(lang, why)));
        }
    }
    if ctx.json {
        #[derive(serde::Serialize)]
        struct Out<'a> {
            #[serde(flatten)]
            writing: &'a crate::user_config::Writing,
            path: Option<&'a std::path::Path>,
            /// 저장소의 표가 값을 하나라도 댔을 때 그 파일 — 아니면 `null` 이다.
            repo_path: Option<&'a std::path::Path>,
        }
        super::json_line(&Out { writing: &writing, path: reg.path.as_deref(), repo_path })
    } else {
        Ok(crate::view::writing(&writing, reg.path.as_deref(), repo_path, lang))
    }
}
