pub mod api;
pub mod auth;
pub mod domain;
pub mod password;
pub(crate) mod permissions;
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

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/health/live", get(health))
        .nest("/api/v1/auth", auth::routes())
        .nest("/api/v1/teams", teams::routes())
        .merge(api::routes::participation_routes())
        .with_state(state)
}
