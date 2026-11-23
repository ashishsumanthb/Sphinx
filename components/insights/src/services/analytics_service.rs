use crate::{
    api::{dto::MyResultsResponse, error::ApiError},
    repository::analytics,
};
use axum::http::StatusCode;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn my_results(
    pool: &PgPool,
    quiz_id: Uuid,
    user_id: Uuid,
    request_id: &str,
) -> Result<MyResultsResponse, ApiError> {
    // Authorization, history and ranking must see one consistent database snapshot.
    let mut tx = pool.begin().await.map_err(|_| unavailable(request_id))?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await
        .map_err(|_| unavailable(request_id))?;
    let allowed = analytics::can_view_quiz(&mut tx, quiz_id, user_id)
        .await
        .map_err(|_| unavailable(request_id))?;
    if !allowed {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            "Quiz unavailable",
            request_id,
        ));
    }
    let attempts = analytics::attempt_history(&mut tx, quiz_id, user_id)
        .await
        .map_err(|_| unavailable(request_id))?;
    let revision_results = analytics::revision_ranks(&mut tx, quiz_id, user_id)
        .await
        .map_err(|_| unavailable(request_id))?;
    tx.commit().await.map_err(|_| unavailable(request_id))?;
    Ok(MyResultsResponse {
        quiz_id,
        attempts,
        revision_results,
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
