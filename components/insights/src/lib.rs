pub mod api;
pub mod app_state;
pub mod repository;
pub mod services;

use app_state::AppState;
use axum::{Router, routing::get};

/// Mount behind trusted authentication middleware which inserts api::AuthenticatedUser.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route(
            "/api/v1/quizzes/{quiz_id}/my-results",
            get(api::handlers::analytics::my_results),
        )
        .with_state(state)
}
