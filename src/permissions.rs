use axum::http::StatusCode;
use sqlx::PgConnection;
use uuid::Uuid;

use crate::auth::{ApiError, api_error};

// Call inside a transaction to hold the membership lock during the operation.
pub(crate) async fn require_member(
    connection: &mut PgConnection,
    team_id: Uuid,
    user_id: Uuid,
) -> Result<String, ApiError> {
    let membership = sqlx::query_as::<_, (String,)>(
        "SELECT role FROM team_memberships
         WHERE team_id = $1 AND user_id = $2
         FOR SHARE",
    )
    .bind(team_id)
    .bind(user_id)
    .fetch_optional(connection)
    .await
    .map_err(permission_error)?;

    membership
        .map(|(role,)| role)
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "Team not found"))
}

// Lock order for team-scoped writes: team -> membership -> dependent rows
// (including quizzes). Callers must not hold a quiz lock before this check.
pub(crate) async fn require_admin(
    connection: &mut PgConnection,
    team_id: Uuid,
    user_id: Uuid,
    message: &'static str,
) -> Result<(), ApiError> {
    let team = sqlx::query_as::<_, (Uuid,)>("SELECT id FROM teams WHERE id = $1 FOR UPDATE")
        .bind(team_id)
        .fetch_optional(&mut *connection)
        .await
        .map_err(permission_error)?;

    if team.is_none() {
        return Err(api_error(StatusCode::NOT_FOUND, "Team not found"));
    }

    let role = require_member(connection, team_id, user_id).await?;

    if role != "admin" {
        return Err(api_error(StatusCode::FORBIDDEN, message));
    }

    Ok(())
}

fn permission_error(_error: sqlx::Error) -> ApiError {
    api_error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Unable to verify team permissions",
    )
}
