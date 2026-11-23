use crate::{
    api::{AuthenticatedUser, error::ApiError},
    app_state::AppState,
    services::analytics_service,
};
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use uuid::Uuid;

pub async fn my_results(
    State(state): State<AppState>,
    user: Option<Extension<AuthenticatedUser>>,
    Path(raw_quiz_id): Path<String>,
) -> Result<Response, ApiError> {
    let started = std::time::Instant::now();
    let request_id = Uuid::new_v4().to_string();
    let identity = user.ok_or_else(|| {
        ApiError::new(
            StatusCode::UNAUTHORIZED,
            "UNAUTHENTICATED",
            "Authentication required",
            &request_id,
        )
    })?;
    let quiz_id = Uuid::parse_str(&raw_quiz_id).map_err(|_| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "VALIDATION",
            "quiz_id must be a UUID",
            &request_id,
        )
    })?;
    let result =
        analytics_service::my_results(&state.pool, quiz_id, identity.user_id, &request_id).await;
    let status = match &result {
        Ok(_) => StatusCode::OK,
        Err(e) => e.status,
    };
    tracing::info!(request_id, quiz_id = %quiz_id, method = "GET", path = "/api/v1/quizzes/{quiz_id}/my-results", outcome = status.as_u16(), duration_ms = started.elapsed().as_millis() as u64, event = "member_results", "Results request completed");
    let mut response = Json(result?).into_response();
    response
        .headers_mut()
        .insert("x-request-id", request_id.parse().expect("generated UUID"));
    Ok(response)
}
