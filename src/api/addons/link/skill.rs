use super::super::bad;
use super::checkout::{checkout, Checkout};
use crate::{api::ApiResult, app::AppState, features::addons};
use axum::{extract::State, Json};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
pub(crate) struct LinkSkillInstall {
    link: String,
    skill_id: String,
    confirmation: Option<String>,
}

pub(crate) async fn install_link_skill(
    State(state): State<AppState>,
    Json(input): Json<LinkSkillInstall>,
) -> ApiResult<Json<Value>> {
    if input.skill_id.len() != 64 || !input.skill_id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(bad("Mã skill không hợp lệ. Hãy kiểm tra lại liên kết."));
    }
    let Checkout {
        dir, found, tree, ..
    } = checkout(&input.link).await?;
    let skill = found
        .skills
        .into_iter()
        .find(|skill| skill.id == input.skill_id)
        .ok_or_else(|| bad("Không tìm thấy skill đã chọn. Hãy kiểm tra lại liên kết."))?;
    let name = skill.name.clone();
    let confirmation = input.confirmation;
    let skill_id = skill.id.clone();
    let config_path = state.config_path.clone();
    let result = tokio::task::spawn_blocking(move || {
        addons::install_checkout_skill(&dir.0, &tree, &skill, &config_path, confirmation.as_deref())
    })
    .await
    .map_err(|_| bad("Không xử lý được skill"))?
    .map_err(|error| bad(format!("{error:#}")))?;
    Ok(Json(match result {
        addons::SkillInstall::Installed => json!({
            "ok": true,
            "output": format!("Đã cài skill «{name}» vào kho chung của MutiS. Gõ /{name} ở đầu tin nhắn để dùng."),
        }),
        addons::SkillInstall::Conflict(confirmation) => json!({
            "ok": false,
            "output": format!("Kho chung của MutiS đã có skill «{name}». Cần xác nhận để thay thế."),
            "conflict": { "skill_id": skill_id, "confirmation": confirmation },
        }),
    }))
}
