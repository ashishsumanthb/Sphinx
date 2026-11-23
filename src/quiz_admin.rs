use crate::domain::question_validation::{QuestionDraft, validate_question};
use crate::{
    AppState,
    auth::{ApiError, AuthUser, api_error},
    domain::quiz::{Quiz, QuizRevision, QuizStatus},
    permissions::require_admin,
    services::quiz_service::{
        DraftItem, Snapshot, VersionedItem, close_quiz, open_quiz, revise_question,
    },
};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{post, put},
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PublishQuizRequest {
    starts_at: Option<DateTime<Utc>>,
    duration_minutes: Option<u32>,
}

#[derive(Serialize)]
struct PublishedQuiz {
    quiz_id: Uuid,
    quiz_revision_id: Uuid,
    revision_no: i32,
    status: String,
    starts_at: DateTime<Utc>,
    ends_at: DateTime<Utc>,
    eligible_users: u64,
}
#[derive(Serialize)]
struct ClosedQuiz {
    quiz_id: Uuid,
    status: String,
    manually_closed_at: DateTime<Utc>,
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
        .route("/api/v1/quizzes/{quiz_id}/publish", post(publish_quiz))
        .route(
            "/api/v1/quizzes/{quiz_id}/close",
            post(close_published_quiz),
        )
}
#[derive(Deserialize)]
struct AddQuestionRequest {
    #[serde(flatten)]
    question: QuestionDraft,
    #[serde(default)]
    explanation: String,
}

#[derive(Deserialize)]
struct EditQuestionRequest {
    #[serde(flatten)]
    question: QuestionDraft,
    #[serde(default)]
    explanation: String,
    change_reason: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    quiz_revision_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    revision_no: Option<i32>,
}
async fn publish_quiz(
    State(state): State<AppState>,
    user: AuthUser,
    Path(quiz_id): Path<Uuid>,
    Json(request): Json<PublishQuizRequest>,
) -> Result<(StatusCode, Json<PublishedQuiz>), ApiError> {
    let mut tx = state.pool.begin().await.map_err(publish_db_error)?;

    let (
        team_id,
        title,
        status,
        existing_starts_at,
        existing_ends_at,
        manually_closed_at,
        max_attempts,
        created_by,
        description,
        created_at,
        database_time,
    ) = sqlx::query_as::<
        _,
        (
            Uuid,
            String,
            String,
            Option<DateTime<Utc>>,
            Option<DateTime<Utc>>,
            Option<DateTime<Utc>>,
            i16,
            Uuid,
            Option<String>,
            DateTime<Utc>,
            DateTime<Utc>,
        ),
    >(
        "SELECT team_id,
                title,
                status,
                starts_at,
                ends_at,
                manually_closed_at,
                max_attempts,
                created_by,
                description,
                created_at,
                clock_timestamp()
         FROM quizzes
         WHERE id = $1
         FOR UPDATE",
    )
    .bind(quiz_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(publish_db_error)?
    .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "Quiz not found"))?;

    require_admin(
        &mut tx,
        team_id,
        user.id,
        "Only a team admin can publish a quiz",
    )
    .await?;

    if status != "draft" {
        return Err(api_error(
            StatusCode::CONFLICT,
            "Only a draft quiz can be published",
        ));
    }

    let max_attempts = u8::try_from(max_attempts).map_err(|_| {
        api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Quiz has an invalid attempt limit",
        )
    })?;

    let mut quiz = Quiz {
        id: quiz_id,
        team_id,
        title,
        status: QuizStatus::Draft,
        starts_at: existing_starts_at.map(|value| value.timestamp()),
        ends_at: existing_ends_at.map(|value| value.timestamp()),
        manually_closed_at: manually_closed_at.map(|value| value.timestamp()),
        max_attempts,
        created_by,
        description,
        created_at: created_at.timestamp(),
    };

    let question_rows = sqlx::query_as::<
        _,
        (
            Uuid,
            i32,
            String,
            sqlx::types::Json<Vec<String>>,
            i32,
            String,
        ),
    >(
        "SELECT stable_question_id,
                position,
                question_text,
                options,
                correct_option_index,
                explanation
         FROM draft_questions
         WHERE quiz_id = $1
         ORDER BY position",
    )
    .bind(quiz_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(publish_db_error)?;

    let mut draft_items = Vec::with_capacity(question_rows.len());

    for (
        stable_question_id,
        _position,
        question_text,
        options,
        correct_option_index,
        explanation,
    ) in question_rows
    {
        let correct_option_index = usize::try_from(correct_option_index).map_err(|_| {
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Question has an invalid correct answer index",
            )
        })?;

        draft_items.push(DraftItem {
            stable_id: stable_question_id,
            question: QuestionDraft {
                question_text,
                options: options.0,
                correct_option_index,
            },
            explanation: Some(explanation),
        });
    }

    let starts_at = request.starts_at.unwrap_or(database_time);

    if starts_at < database_time {
        return Err(api_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Quiz start time must not be in the past",
        ));
    }

    let revision = QuizRevision {
        id: Uuid::new_v4(),
        quiz_id,
        revision_no: 1,
        created_by: user.id,
        change_reason: String::from("Initial publication"),
        created_at: database_time.timestamp(),
    };

    let snapshot = open_quiz(
        &mut quiz,
        revision,
        &draft_items,
        starts_at.timestamp(),
        request.duration_minutes,
    )
    .map_err(|message| api_error(StatusCode::UNPROCESSABLE_ENTITY, message))?;

    let ends_at_seconds = quiz.ends_at.ok_or_else(|| {
        api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Quiz closing time was not created",
        )
    })?;

    let duration_seconds = ends_at_seconds
        .checked_sub(starts_at.timestamp())
        .ok_or_else(|| {
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Quiz duration is out of range",
            )
        })?;

    let ends_at = starts_at
        .checked_add_signed(Duration::seconds(duration_seconds))
        .ok_or_else(|| {
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Quiz closing time is out of range",
            )
        })?;

    sqlx::query(
        "INSERT INTO quiz_revisions
            (id, quiz_id, revision_no, created_by, change_reason, created_at)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(snapshot.revision.id)
    .bind(snapshot.revision.quiz_id)
    .bind(snapshot.revision.revision_no)
    .bind(snapshot.revision.created_by)
    .bind(snapshot.revision.change_reason.as_str())
    .bind(database_time)
    .execute(&mut *tx)
    .await
    .map_err(publish_db_error)?;

    for version in &snapshot.questions {
        let position = i32::try_from(version.position).map_err(|_| {
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Question position is out of range",
            )
        })?;

        let correct_option_index = i32::try_from(version.item.question.correct_option_index)
            .map_err(|_| {
                api_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Correct answer index is out of range",
                )
            })?;

        sqlx::query(
            "INSERT INTO question_versions
                (id,
                 quiz_revision_id,
                 stable_question_id,
                 position,
                 question_text,
                 options,
                 correct_option_index,
                 explanation)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(version.id)
        .bind(snapshot.revision.id)
        .bind(version.item.stable_id)
        .bind(position)
        .bind(version.item.question.question_text.as_str())
        .bind(sqlx::types::Json(version.item.question.options.clone()))
        .bind(correct_option_index)
        .bind(version.item.explanation.clone().unwrap_or_default())
        .execute(&mut *tx)
        .await
        .map_err(publish_db_error)?;
    }

    let eligible_users = sqlx::query(
        "INSERT INTO quiz_eligibilities (quiz_id, user_id)
         SELECT $1, user_id
         FROM team_memberships
         WHERE team_id = $2
         ON CONFLICT (quiz_id, user_id) DO NOTHING",
    )
    .bind(quiz_id)
    .bind(team_id)
    .execute(&mut *tx)
    .await
    .map_err(publish_db_error)?
    .rows_affected();

    sqlx::query(
        "UPDATE quizzes
         SET status = 'open',
             starts_at = $2,
             ends_at = $3,
             manually_closed_at = NULL
         WHERE id = $1",
    )
    .bind(quiz_id)
    .bind(starts_at)
    .bind(ends_at)
    .execute(&mut *tx)
    .await
    .map_err(publish_db_error)?;

    tx.commit().await.map_err(publish_db_error)?;

    Ok((
        StatusCode::CREATED,
        Json(PublishedQuiz {
            quiz_id,
            quiz_revision_id: snapshot.revision.id,
            revision_no: snapshot.revision.revision_no,
            status: String::from("open"),
            starts_at,
            ends_at,
            eligible_users,
        }),
    ))
}

fn publish_db_error(_: sqlx::Error) -> ApiError {
    api_error(StatusCode::INTERNAL_SERVER_ERROR, "Unable to publish quiz")
}
async fn close_published_quiz(
    State(state): State<AppState>,
    user: AuthUser,
    Path(quiz_id): Path<Uuid>,
) -> Result<Json<ClosedQuiz>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(close_db_error)?;

    let (
        team_id,
        title,
        status,
        starts_at,
        ends_at,
        existing_manually_closed_at,
        max_attempts,
        created_by,
        description,
        created_at,
        database_time,
    ) = sqlx::query_as::<
        _,
        (
            Uuid,
            String,
            String,
            Option<DateTime<Utc>>,
            Option<DateTime<Utc>>,
            Option<DateTime<Utc>>,
            i16,
            Uuid,
            Option<String>,
            DateTime<Utc>,
            DateTime<Utc>,
        ),
    >(
        "SELECT team_id,
                title,
                status,
                starts_at,
                ends_at,
                manually_closed_at,
                max_attempts,
                created_by,
                description,
                created_at,
                clock_timestamp()
         FROM quizzes
         WHERE id = $1
         FOR UPDATE",
    )
    .bind(quiz_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(close_db_error)?
    .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "Quiz not found"))?;

    require_admin(
        &mut tx,
        team_id,
        user.id,
        "Only a team admin can close a quiz",
    )
    .await?;

    let domain_status = match status.as_str() {
        "draft" => QuizStatus::Draft,
        "open" => QuizStatus::Open,
        "closed" => QuizStatus::Closed,
        _ => {
            return Err(api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Quiz has an invalid status",
            ));
        }
    };

    let max_attempts = u8::try_from(max_attempts).map_err(|_| {
        api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Quiz has an invalid attempt limit",
        )
    })?;

    let mut quiz = Quiz {
        id: quiz_id,
        team_id,
        title,
        status: domain_status,
        starts_at: starts_at.map(|value| value.timestamp()),
        ends_at: ends_at.map(|value| value.timestamp()),
        manually_closed_at: existing_manually_closed_at.map(|value| value.timestamp()),
        max_attempts,
        created_by,
        description,
        created_at: created_at.timestamp(),
    };

    close_quiz(&mut quiz, database_time.timestamp())
        .map_err(|message| api_error(StatusCode::CONFLICT, message))?;

    let manually_closed_at = existing_manually_closed_at.unwrap_or(database_time);

    sqlx::query(
        "UPDATE quizzes
         SET status = 'closed',
             manually_closed_at = COALESCE(manually_closed_at, $2)
         WHERE id = $1",
    )
    .bind(quiz_id)
    .bind(manually_closed_at)
    .execute(&mut *tx)
    .await
    .map_err(close_db_error)?;

    tx.commit().await.map_err(close_db_error)?;

    Ok(Json(ClosedQuiz {
        quiz_id,
        status: String::from("closed"),
        manually_closed_at,
    }))
}

fn close_db_error(_: sqlx::Error) -> ApiError {
    api_error(StatusCode::INTERNAL_SERVER_ERROR, "Unable to close quiz")
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
    Json(request): Json<EditQuestionRequest>,
) -> Result<Json<UpdatedQuestion>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(edit_question_db_error)?;

    require_admin(
        &mut tx,
        team_id,
        user.id,
        "Only a team admin can edit a question",
    )
    .await?;

    let (status, database_time) = sqlx::query_as::<_, (String, DateTime<Utc>)>(
        "SELECT status, clock_timestamp()
         FROM quizzes
         WHERE id = $1 AND team_id = $2
         FOR UPDATE",
    )
    .bind(quiz_id)
    .bind(team_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(edit_question_db_error)?
    .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "Quiz not found"))?;

    let EditQuestionRequest {
        question,
        explanation,
        change_reason,
    } = request;

    validate_question(&question)
        .map_err(|message| api_error(StatusCode::UNPROCESSABLE_ENTITY, message))?;

    if status == "draft" {
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
        .bind(explanation)
        .bind(quiz_id)
        .bind(question_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(edit_question_db_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "Question not found"))?;

        tx.commit().await.map_err(edit_question_db_error)?;

        return Ok(Json(UpdatedQuestion {
            stable_question_id: question_id,
            position,
            quiz_revision_id: None,
            revision_no: None,
        }));
    }

    if status != "open" && status != "closed" {
        return Err(api_error(
            StatusCode::CONFLICT,
            "Quiz cannot be revised in its current state",
        ));
    }

    let change_reason = change_reason.as_deref().unwrap_or("");

    let (
        previous_revision_id,
        previous_revision_no,
        previous_created_by,
        previous_change_reason,
        previous_created_at,
    ) = sqlx::query_as::<_, (Uuid, i32, Uuid, String, DateTime<Utc>)>(
        "SELECT id, revision_no, created_by, change_reason, created_at
         FROM quiz_revisions
         WHERE quiz_id = $1
         ORDER BY revision_no DESC
         LIMIT 1",
    )
    .bind(quiz_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(edit_question_db_error)?
    .ok_or_else(|| api_error(StatusCode::CONFLICT, "Quiz has no published revision"))?;

    let version_rows = sqlx::query_as::<
        _,
        (
            Uuid,
            Uuid,
            i32,
            String,
            sqlx::types::Json<Vec<String>>,
            i32,
            String,
        ),
    >(
        "SELECT id,
                stable_question_id,
                position,
                question_text,
                options,
                correct_option_index,
                explanation
         FROM question_versions
         WHERE quiz_revision_id = $1
         ORDER BY position",
    )
    .bind(previous_revision_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(edit_question_db_error)?;

    let mut previous_questions = Vec::with_capacity(version_rows.len());

    for (
        version_id,
        stable_question_id,
        position,
        question_text,
        options,
        correct_option_index,
        explanation,
    ) in version_rows
    {
        let position = usize::try_from(position).map_err(|_| {
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Question position is out of range",
            )
        })?;

        let correct_option_index = usize::try_from(correct_option_index).map_err(|_| {
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Correct answer index is out of range",
            )
        })?;

        previous_questions.push(VersionedItem {
            id: version_id,
            position,
            item: DraftItem {
                stable_id: stable_question_id,
                question: QuestionDraft {
                    question_text,
                    options: options.0,
                    correct_option_index,
                },
                explanation: Some(explanation),
            },
        });
    }

    let previous = Snapshot {
        revision: QuizRevision {
            id: previous_revision_id,
            quiz_id,
            revision_no: previous_revision_no,
            created_by: previous_created_by,
            change_reason: previous_change_reason,
            created_at: previous_created_at.timestamp(),
        },
        questions: previous_questions,
    };

    let revised = revise_question(
        &previous,
        question_id,
        question,
        Some(explanation),
        user.id,
        database_time.timestamp(),
        change_reason,
    )
    .map_err(|message| api_error(StatusCode::UNPROCESSABLE_ENTITY, message))?;

    sqlx::query(
        "INSERT INTO quiz_revisions
            (id, quiz_id, revision_no, created_by, change_reason, created_at)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(revised.revision.id)
    .bind(revised.revision.quiz_id)
    .bind(revised.revision.revision_no)
    .bind(revised.revision.created_by)
    .bind(revised.revision.change_reason.as_str())
    .bind(database_time)
    .execute(&mut *tx)
    .await
    .map_err(edit_question_db_error)?;

    for version in &revised.questions {
        let position = i32::try_from(version.position).map_err(|_| {
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Question position is out of range",
            )
        })?;

        let correct_option_index = i32::try_from(version.item.question.correct_option_index)
            .map_err(|_| {
                api_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Correct answer index is out of range",
                )
            })?;

        sqlx::query(
            "INSERT INTO question_versions
                (id,
                 quiz_revision_id,
                 stable_question_id,
                 position,
                 question_text,
                 options,
                 correct_option_index,
                 explanation)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(version.id)
        .bind(revised.revision.id)
        .bind(version.item.stable_id)
        .bind(position)
        .bind(version.item.question.question_text.as_str())
        .bind(sqlx::types::Json(version.item.question.options.clone()))
        .bind(correct_option_index)
        .bind(version.item.explanation.clone().unwrap_or_default())
        .execute(&mut *tx)
        .await
        .map_err(edit_question_db_error)?;
    }

    let revised_question = revised
        .questions
        .iter()
        .find(|version| version.item.stable_id == question_id)
        .ok_or_else(|| {
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Revised question was not created",
            )
        })?;

    let position = i32::try_from(revised_question.position).map_err(|_| {
        api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Question position is out of range",
        )
    })?;

    let correct_option_index = i32::try_from(revised_question.item.question.correct_option_index)
        .map_err(|_| {
        api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Correct answer index is out of range",
        )
    })?;

    let updated = sqlx::query(
        "UPDATE draft_questions
         SET question_text = $1,
             options = $2,
             correct_option_index = $3,
             explanation = $4
         WHERE quiz_id = $5
           AND stable_question_id = $6",
    )
    .bind(revised_question.item.question.question_text.as_str())
    .bind(sqlx::types::Json(
        revised_question.item.question.options.clone(),
    ))
    .bind(correct_option_index)
    .bind(
        revised_question
            .item
            .explanation
            .clone()
            .unwrap_or_default(),
    )
    .bind(quiz_id)
    .bind(question_id)
    .execute(&mut *tx)
    .await
    .map_err(edit_question_db_error)?;

    if updated.rows_affected() != 1 {
        return Err(api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Published question is missing from the draft",
        ));
    }

    tx.commit().await.map_err(edit_question_db_error)?;

    Ok(Json(UpdatedQuestion {
        stable_question_id: question_id,
        position,
        quiz_revision_id: Some(revised.revision.id),
        revision_no: Some(revised.revision.revision_no),
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
