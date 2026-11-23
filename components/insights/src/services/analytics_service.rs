use crate::api::{dto::MyResultsResponse, error::ApiError};
use axum::http::StatusCode;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn my_results(
    pool: &PgPool,
    quiz_id: Uuid,
    user_id: Uuid,
    request_id: &str,
) -> Result<MyResultsResponse, ApiError> {
    crate::results::my_results(pool, quiz_id, user_id)
        .await
        .map_err(|error| match error {
            crate::results::ResultsError::NotFound => ApiError::new(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "Quiz unavailable",
                request_id,
            ),
            crate::results::ResultsError::Database(_) => unavailable(request_id),
        })
}

fn unavailable(request_id: &str) -> ApiError {
    tracing::error!(
        request_id,
        event = "insights_database_failure",
        "Results query failed"
    );
    ApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "DEPENDENCY_UNAVAILABLE",
        "Results temporarily unavailable",
        request_id,
    )
}
