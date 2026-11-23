pub mod api;
pub mod auth;
pub mod domain;
pub mod password;
pub(crate) mod permissions;
mod quiz_admin;
pub mod repository;
pub mod services;
pub mod session;
pub mod teams;

use axum::{Json, Router, routing::get};
use serde::Serialize;
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "sphinx",
    })
}
async fn check_question(
    Json(draft): Json<domain::question_validation::QuestionDraft>,
) -> (axum::http::StatusCode, &'static str) {
    use axum::http::StatusCode;

    match domain::question_validation::validate_question(&draft) {
        Ok(()) => (StatusCode::OK, "Question is valid"),
        Err(message) => (StatusCode::UNPROCESSABLE_ENTITY, message),
    }
}
pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/health/live", get(health))
        .route("/questions/validate", axum::routing::post(check_question))
        .nest("/api/v1/auth", auth::routes())
        .nest("/api/v1/teams", teams::routes())
        .merge(api::routes::participation_routes())
        .merge(quiz_admin::routes())
        .with_state(state)
}
