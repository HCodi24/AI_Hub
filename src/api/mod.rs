use crate::app::AppState;
use axum::{
    extract::{DefaultBodyLimit, Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use serde_json::json;

pub mod addons;
pub mod attachments;
pub mod mcp;
pub mod pets;
pub mod chat;
pub mod config;
pub mod debate;
pub mod dev;
pub mod extensions;
pub mod folder;
pub mod memory;
pub mod mutis;
pub mod plans;
pub mod providers;
pub mod static_ui;
pub mod verify;

#[derive(Debug)]
pub(crate) struct ApiError {
    pub(crate) status: StatusCode,
    pub(crate) message: String,
}

impl ApiError {
    pub(crate) fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}

pub(crate) type ApiResult<T> = Result<T, ApiError>;

pub fn router(state: AppState) -> Router {
    let port = state.port;
    Router::new()
        // Static UI routes.
        .route("/app", get(crate::api::static_ui::web_index))
        .route("/app/", get(crate::api::static_ui::web_index))
        .route("/app/{*path}", get(crate::api::static_ui::web_asset))
        .route("/", get(crate::api::static_ui::index))
        // Development routes.
        .route("/api/dev/status", get(crate::api::dev::dev_status))
        .route("/api/dev/repo", get(crate::api::dev::dev_repo_info))
        .route("/api/dev/git/diff", get(crate::api::dev::dev_git_diff))
        .route(
            "/api/dev/git/checkpoint",
            post(crate::api::dev::dev_git_checkpoint),
        )
        .route("/api/dev/git/reset", post(crate::api::dev::dev_git_reset))
        .route("/api/dev/git/commit", post(crate::api::dev::dev_git_commit))
        .route("/api/dev/test", post(crate::api::dev::dev_test))
        .route("/api/dev/release", post(crate::api::dev::dev_release))
        .route(
            "/api/dev/settings",
            get(crate::api::dev::dev_settings).put(crate::api::dev::put_dev_settings),
        )
        .route("/api/dev/events", get(crate::api::dev::dev_events))
        .route("/api/dev/rebuild", post(crate::api::dev::dev_rebuild))
        .route("/api/dev/web-build", post(crate::api::dev::dev_web_build))
        // Plugin & Skill: add-ons loaded by Claude Code and Codex.
        .route(
            "/api/addons",
            get(crate::api::addons::overview).post(crate::api::addons::change),
        )
        // Configuration routes.
        .route(
            "/api/config",
            get(crate::api::config::get_config).put(crate::api::config::put_config),
        )
        .route("/api/memory", get(memory::list))
        .route("/api/memory/projects", get(memory::projects))
        .route("/api/memory/recall", get(memory::recall))
        .route(
            "/api/memory/{scope}",
            get(memory::get).put(memory::put).delete(memory::delete),
        )
        .route("/api/memory/{scope}/archive", post(memory::archive))
        .route("/api/memory/{scope}/restore", post(memory::restore))
        // MutiS routes.
        .route(
            "/api/mutis",
            get(crate::api::mutis::get_mutis).put(crate::api::mutis::put_mutis),
        )
        .route(
            "/api/mutis/roles/resolve",
            get(crate::api::mutis::resolve_mutis_roles),
        )
        .route("/api/mutis/stats", get(crate::api::mutis::mutis_stats))
        .route(
            "/api/mutis/diagnose",
            post(crate::api::mutis::mutis_diagnose),
        )
        .route(
            "/api/mutis/apply",
            post(crate::api::mutis::mutis_setting_change),
        )
        .route(
            "/api/mutis/setting-change",
            post(crate::api::mutis::mutis_setting_change),
        )
        .route(
            "/api/mutis/diagnoses",
            get(crate::api::mutis::mutis_diagnoses),
        )
        .route(
            "/api/mutis/events",
            get(crate::api::mutis::mutis_events).post(crate::api::mutis::record_mutis_event),
        )
        // Provider management and discovery routes.
        .route(
            "/api/providers/{id}/limits",
            get(crate::api::providers::provider_limits),
        )
        .route(
            "/api/providers",
            post(crate::api::providers::create_provider),
        )
        .route(
            "/api/providers/{id}",
            put(crate::api::providers::update_provider)
                .delete(crate::api::providers::remove_provider),
        )
        .route(
            "/api/providers/{id}/models",
            get(crate::api::providers::provider_models),
        )
        .route(
            "/api/providers/{id}/test",
            post(crate::api::providers::test_provider),
        )
        .route(
            "/api/providers/{id}/login/start",
            post(crate::api::providers::login_provider_start),
        )
        .route(
            "/api/providers/{id}/login/code",
            post(crate::api::providers::login_provider_code),
        )
        .route(
            "/api/providers/{id}/login/cancel",
            post(crate::api::providers::login_provider_cancel),
        )
        .route(
            "/api/cli-discover/{name}",
            get(crate::api::providers::discover_cli),
        )
        // Chat and saved conversation routes.
        .route("/api/chat", post(crate::api::chat::chat_handler))
        .route(
            "/api/attachments",
            post(crate::api::attachments::upload).layer(DefaultBodyLimit::max(14 * 1024 * 1024)),
        )
        .route(
            "/api/attachments/{chat}/{file}",
            get(crate::api::attachments::serve),
        )
        .route(
            "/api/chat/{chat_id}/stop",
            post(crate::api::chat::stop_chat),
        )
        .route("/api/folder-tree", get(crate::api::folder::list_dir))
        .route("/api/pick-folder", post(crate::api::folder::pick_folder))
        .route("/api/mcp/test", post(crate::api::mcp::test))
        .route("/api/pets", get(crate::api::pets::list))
        .route("/api/pets/import", post(crate::api::pets::import))
        .route("/api/pets/{name}", get(crate::api::pets::sprite).delete(crate::api::pets::remove))
        .route("/api/pets/{name}/install", post(crate::api::pets::install))
        .route("/api/chats", get(crate::api::chat::list_chats))
        .route(
            "/api/chats/{chat_id}",
            get(crate::api::chat::get_chat).delete(crate::api::chat::delete_chat),
        )
        // Codex vs Claude debate ("⚖ Tranh luận").
        .route(
            "/api/debates",
            get(crate::api::debate::list_debates).post(crate::api::debate::start_debate),
        )
        .route("/api/debates/{id}", get(crate::api::debate::get_debate))
        .route(
            "/api/debates/{id}/chat",
            post(crate::api::debate::attach_chat),
        )
        .route(
            "/api/debates/{id}/plan",
            post(crate::api::debate::link_plan),
        )
        // Plan management and generation routes.
        .route(
            "/api/plans",
            get(plans::list_plans).post(plans::generate_plan),
        )
        .route(
            "/api/plans/{id}",
            get(plans::get_plan).put(plans::save_plan),
        )
        // Verification and plan diff routes.
        .route("/api/verify", post(crate::api::verify::verify_handler))
        .route(
            "/api/plan-diff",
            post(crate::api::verify::plan_diff_handler),
        )
        .route("/api/cli-status", get(crate::api::providers::cli_status))
        .route("/api/providers/{id}/codex", get(crate::api::providers::codex_status)
            .post(crate::api::providers::codex_action).put(crate::api::providers::codex_settings))
        // Extension catalog and actions.
        .route(
            "/api/extensions",
            get(crate::api::extensions::extension_list),
        )
        .route(
            "/api/extensions/{id}/actions/{action}",
            post(crate::api::extensions::extension_action),
        )
        .route(
            "/api/extensions/{id}/stop",
            post(crate::api::extensions::extension_stop),
        )
        .layer(middleware::from_fn_with_state(port, enforce_local_host))
        .with_state(state)
}

async fn enforce_local_host(State(port): State<u16>, request: Request, next: Next) -> Response {
    let ipv4 = format!("127.0.0.1:{port}");
    let localhost = format!("localhost:{port}");
    let host = request
        .headers()
        .get("host")
        .and_then(|value| value.to_str().ok());
    if !host.is_some_and(|host| host == ipv4 || host.eq_ignore_ascii_case(&localhost)) {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    if request.method() != axum::http::Method::GET {
        if let Some(origin) = request.headers().get("origin") {
            let origin = origin.to_str().unwrap_or_default();
            let ipv4_origin = format!("http://{ipv4}");
            let localhost_origin = format!("http://localhost:{port}");
            if !origin.eq_ignore_ascii_case(&ipv4_origin)
                && !origin.eq_ignore_ascii_case(&localhost_origin)
            {
                return (StatusCode::FORBIDDEN, "Forbidden").into_response();
            }
        }
    }
    next.run(request).await
}
