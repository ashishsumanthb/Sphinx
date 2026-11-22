use axum::{
    Router,
    extract::DefaultBodyLimit,
    middleware,
    response::{IntoResponse, Response},
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
        .route(
            "/api/v1/attempts/:attempt_id/review",
            get(handlers::attempt_review),
        )
        .route(
            "/api/v1/quizzes/:quiz_id/my-results",
            get(handlers::my_results),
        )
        .route(
            "/api/v1/quizzes/:quiz_id/analytics",
            get(handlers::quiz_analytics),
        )
        .layer(DefaultBodyLimit::max(1024 * 1024))
        .layer(middleware::map_response(normalize_error_response))
}

async fn normalize_error_response(response: Response) -> Response {
    if response.status() == axum::http::StatusCode::PAYLOAD_TOO_LARGE {
        crate::api::error::AppError::payload_too_large().into_response()
    } else {
        response
    }
}
