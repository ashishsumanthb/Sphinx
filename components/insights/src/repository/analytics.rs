use crate::api::dto::{AttemptResult, RevisionResult};
use sqlx::PgConnection;
use uuid::Uuid;

pub const MEMBER_REVISION_RANKS: &str =
    include_str!("../../sql/insights/member_revision_ranks.sql");
pub const MEMBER_ATTEMPT_HISTORY: &str =
    include_str!("../../sql/insights/member_attempt_history.sql");

pub async fn can_view_quiz(
    conn: &mut PgConnection,
    quiz_id: Uuid,
    user_id: Uuid,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(include_str!("../../sql/insights/member_quiz_access.sql"))
        .bind(quiz_id)
        .bind(user_id)
        .fetch_one(conn)
        .await
}

pub async fn attempt_history(
    conn: &mut PgConnection,
    quiz_id: Uuid,
    user_id: Uuid,
) -> Result<Vec<AttemptResult>, sqlx::Error> {
    sqlx::query_as(MEMBER_ATTEMPT_HISTORY)
        .bind(quiz_id)
        .bind(user_id)
        .fetch_all(conn)
        .await
}

pub async fn revision_ranks(
    conn: &mut PgConnection,
    quiz_id: Uuid,
    user_id: Uuid,
) -> Result<Vec<RevisionResult>, sqlx::Error> {
    sqlx::query_as(MEMBER_REVISION_RANKS)
        .bind(quiz_id)
        .bind(user_id)
        .fetch_all(conn)
        .await
}
