use crate::domain::question_validation::{QuestionDraft, validate_question};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{post, put},
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
    Router::new()
        .route("/api/v1/teams/{team_id}/quizzes", post(create_quiz))
        .route(
            "/api/v1/teams/{team_id}/quizzes/{quiz_id}/questions",
            post(add_question),
        )
        .route(
            "/api/v1/teams/{team_id}/quizzes/{quiz_id}/questions/{question_id}",
            put(edit_question),
        )
}
#[derive(Deserialize)]
struct AddQuestionRequest {
    #[serde(flatten)]
    question: QuestionDraft,
    #[serde(default)]
    explanation: String,
}

#[derive(Serialize)]
struct AddedQuestion {
    stable_question_id: Uuid,
    position: i32,
}
#[derive(Serialize)]
struct UpdatedQuestion {
    stable_question_id: Uuid,
    position: i32,
}

async fn add_question(
    State(state): State<AppState>,
    user: AuthUser,
    Path((team_id, quiz_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<AddQuestionRequest>,
) -> Result<(StatusCode, Json<AddedQuestion>), ApiError> {
    let mut tx = state.pool.begin().await.map_err(question_db_error)?;

    require_admin(
        &mut tx,
        team_id,
        user.id,
        "Only a team admin can add a question",
    )
    .await?;

    let status = sqlx::query_scalar::<_, String>(
        "SELECT status FROM quizzes
         WHERE id = $1 AND team_id = $2
         FOR UPDATE",
    )
    .bind(quiz_id)
    .bind(team_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(question_db_error)?
    .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "Quiz not found"))?;

    if status != "draft" {
        return Err(api_error(
            StatusCode::CONFLICT,
            "Only a draft quiz can accept new questions",
        ));
    }

    let question = request.question;
    validate_question(&question)
        .map_err(|message| api_error(StatusCode::UNPROCESSABLE_ENTITY, message))?;

    let (count, position) = sqlx::query_as::<_, (i64, i32)>(
        "SELECT COUNT(*), COALESCE(MAX(position), -1) + 1
         FROM draft_questions WHERE quiz_id = $1",
    )
    .bind(quiz_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(question_db_error)?;

    if count >= 50 {
        return Err(api_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Quiz must have between 1 and 50 questions",
        ));
    }

    let stable_question_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO draft_questions
            (quiz_id, position, question_text, options, correct_option_index, explanation)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING stable_question_id",
    )
    .bind(quiz_id)
    .bind(position)
    .bind(question.question_text)
    .bind(sqlx::types::Json(question.options))
    .bind(question.correct_option_index as i32)
    .bind(request.explanation)
    .fetch_one(&mut *tx)
    .await
    .map_err(question_db_error)?;

    tx.commit().await.map_err(question_db_error)?;

    Ok((
        StatusCode::CREATED,
        Json(AddedQuestion {
            stable_question_id,
            position,
        }),
    ))
}
async fn edit_question(
    State(state): State<AppState>,
    user: AuthUser,
    Path((team_id, quiz_id, question_id)): Path<(Uuid, Uuid, Uuid)>,
    Json(request): Json<AddQuestionRequest>,
) -> Result<Json<UpdatedQuestion>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(edit_question_db_error)?;

    require_admin(
        &mut tx,
        team_id,
        user.id,
        "Only a team admin can edit a question",
    )
    .await?;

    let status = sqlx::query_scalar::<_, String>(
        "SELECT status FROM quizzes
         WHERE id = $1 AND team_id = $2
         FOR UPDATE",
    )
    .bind(quiz_id)
    .bind(team_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(edit_question_db_error)?
    .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "Quiz not found"))?;

    if status != "draft" {
        return Err(api_error(
            StatusCode::CONFLICT,
            "Only questions in a draft quiz can be edited",
        ));
    }

    let question = request.question;
    validate_question(&question)
        .map_err(|message| api_error(StatusCode::UNPROCESSABLE_ENTITY, message))?;

    let position = sqlx::query_scalar::<_, i32>(
        "UPDATE draft_questions
         SET question_text = $1,
             options = $2,
             correct_option_index = $3,
             explanation = $4
         WHERE quiz_id = $5
           AND stable_question_id = $6
         RETURNING position",
    )
    .bind(question.question_text)
    .bind(sqlx::types::Json(question.options))
    .bind(question.correct_option_index as i32)
    .bind(request.explanation)
    .bind(quiz_id)
    .bind(question_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(edit_question_db_error)?
    .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "Question not found"))?;

    tx.commit().await.map_err(edit_question_db_error)?;

    Ok(Json(UpdatedQuestion {
        stable_question_id: question_id,
        position,
    }))
}

fn edit_question_db_error(_: sqlx::Error) -> ApiError {
    api_error(StatusCode::INTERNAL_SERVER_ERROR, "Unable to edit question")
}
fn question_db_error(_: sqlx::Error) -> ApiError {
    api_error(StatusCode::INTERNAL_SERVER_ERROR, "Unable to add question")
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
