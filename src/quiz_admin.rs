use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::post,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    AppState,
    auth::{ApiError, AuthUser, api_error},
    permissions::require_admin,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateQuizRequest {
    title: String,
    description: Option<String>,
    max_attempts: i16,
}

#[derive(Serialize)]
struct CreatedQuiz {
    id: Uuid,
    status: String,
}

pub(crate) fn routes() -> Router<AppState> {
    Router::new().route("/api/v1/teams/{team_id}/quizzes", post(create_quiz))
}

async fn create_quiz(
    State(state): State<AppState>,
    user: AuthUser,
    Path(team_id): Path<Uuid>,
    Json(request): Json<CreateQuizRequest>,
) -> Result<(StatusCode, Json<CreatedQuiz>), ApiError> {
    let title = request.title.trim();
    if !(1..=150).contains(&title.chars().count()) {
        return Err(api_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Quiz title must contain 1 to 150 characters",
        ));
    }
    if !(1..=2).contains(&request.max_attempts) {
        return Err(api_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Quiz must allow 1 or 2 attempts",
        ));
    }

    let mut tx = state.pool.begin().await.map_err(create_error)?;

    require_admin(
        &mut tx,
        team_id,
        user.id,
        "Only a team admin can create a quiz",
    )
    .await?;

    let (id, status) = sqlx::query_as::<_, (Uuid, String)>(
        "INSERT INTO quizzes (team_id, created_by, title, description, max_attempts)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id, status",
    )
    .bind(team_id)
    .bind(user.id)
    .bind(title)
    .bind(request.description)
    .bind(request.max_attempts)
    .fetch_one(&mut *tx)
    .await
    .map_err(create_error)?;

    tx.commit().await.map_err(create_error)?;
    Ok((StatusCode::CREATED, Json(CreatedQuiz { id, status })))
}

fn create_error(_: sqlx::Error) -> ApiError {
    api_error(StatusCode::INTERNAL_SERVER_ERROR, "Unable to create quiz")
}
