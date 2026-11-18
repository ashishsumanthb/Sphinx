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

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/health/live", get(health))
        .with_state(state)
}
