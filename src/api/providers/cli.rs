use crate::{
    api::ApiResult,
    app::AppState,
    features::providers::cli::{
        discovery::CliCandidate,
        status::{self, CliStatus},
    },
};
use axum::{
    extract::{Path, State},
    Json,
};

pub(crate) async fn cli_status(State(state): State<AppState>) -> Json<CliStatus> {
    Json(status::cli_status(&state).await)
}

pub(crate) async fn discover_cli(Path(name): Path<String>) -> ApiResult<Json<Vec<CliCandidate>>> {
    Ok(Json(status::discover_cli(&name).await?))
}
