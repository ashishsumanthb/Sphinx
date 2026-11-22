use axum::{
    Router,
    routing::{get, post},
};

use crate::{AppState, api::handlers};

pub fn participation_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/teams/:team_id/quizzes/available",
            get(handlers::list_available_quizzes),
        )
        .route(
            "/api/v1/quizzes/:quiz_id/attempt-sessions",
            post(handlers::start_attempt_session),
        )
        .route(
            "/api/v1/attempt-sessions/:session_id/questions",
            get(handlers::get_session_questions),
        )
        .route(
            "/api/v1/attempt-sessions/:session_id/submit",
            post(handlers::submit_attempt),
        )
        .route(
            "/api/v1/quizzes/:quiz_id/my-attempts",
            get(handlers::my_attempts),
        )
}
