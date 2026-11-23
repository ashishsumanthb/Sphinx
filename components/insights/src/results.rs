//! Reusable results operation. The caller supplies an already authenticated user ID.
use crate::{api::dto::MyResultsResponse, repository::analytics};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug)]
pub enum ResultsError {
    NotFound,
    Database(sqlx::Error),
}
impl From<sqlx::Error> for ResultsError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

pub async fn my_results(
    pool: &PgPool,
    quiz_id: Uuid,
    user_id: Uuid,
) -> Result<MyResultsResponse, ResultsError> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    if !analytics::can_view_quiz(&mut tx, quiz_id, user_id).await? {
        return Err(ResultsError::NotFound);
    }
    let attempts = analytics::attempt_history(&mut tx, quiz_id, user_id).await?;
    let revision_results = analytics::revision_ranks(&mut tx, quiz_id, user_id).await?;
    tx.commit().await?;
    Ok(MyResultsResponse {
        quiz_id,
        attempts,
        revision_results,
    })
}
