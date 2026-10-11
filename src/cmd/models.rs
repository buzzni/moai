//! 사람의 모델 표를 읽는다. 저장하거나 트래커를 열지 않는다.

use super::{Ctx, R};

pub fn run(ctx: &Ctx) -> R<Vec<String>> {
    let reg = ctx.registry();
    let lang = ctx.lang();
    // 파일 자체의 탈과 키의 탈은 따로 든다. 성한 키는 살리고 종료 0으로 알린다.
    for line in crate::view::settings_problems(reg, lang).into_iter().chain(crate::view::models_problems(reg, lang)) {
        super::tell(&line);
    }
    if ctx.json {
        #[derive(serde::Serialize)]
        struct Out<'a> {
            #[serde(flatten)]
            models: &'a crate::user_config::Models,
            path: Option<&'a std::path::Path>,
        }
        super::json_line(&Out { models: &reg.models, path: reg.path.as_deref() })
    } else {
        Ok(crate::view::models(reg, lang))
    }
}
