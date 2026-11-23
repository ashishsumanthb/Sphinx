use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    api::{
        dto::{
            AnswerInput, AttemptSessionResponse, AvailableQuizResponse, MyAttemptResponse,
            PublicQuestionResponse, SubmitAttemptResponse,
        },
        error::AppError,
    },
    domain::participation::{
        AnswerSetError, QuestionKey, canonical_request_hash, validate_and_score,
    },
};

#[derive(Clone)]
pub struct ParticipationRepository {
    pool: PgPool,
}

impl ParticipationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list_available(
        &self,
        team_id: Uuid,
        user_id: Uuid,
    ) -> Result<Vec<AvailableQuizResponse>, AppError> {
        let rows = sqlx::query_as::<_, (Uuid, Uuid, String, DateTime<Utc>, DateTime<Utc>)>(
            r#"SELECT q.id, q.team_id, q.title, q.starts_at, q.ends_at
               FROM quizzes q
               JOIN team_members tm ON tm.team_id = q.team_id AND tm.user_id = $2
               JOIN quiz_eligibilities qe ON qe.quiz_id = q.id AND qe.user_id = $2
               WHERE q.team_id = $1
                 AND q.status = 'open'
                 AND q.manually_closed_at IS NULL
                 AND q.starts_at <= clock_timestamp()
                 AND clock_timestamp() < q.ends_at
                 AND (SELECT COUNT(*) FROM quiz_attempts qa
                      WHERE qa.quiz_id = q.id AND qa.user_id = $2) < q.max_attempts
               ORDER BY q.ends_at, q.title"#,
        )
        .bind(team_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| AvailableQuizResponse {
                id: row.0,
                team_id: row.1,
                title: row.2,
                starts_at: row.3,
                ends_at: row.4,
            })
            .collect())
    }

    pub async fn start_session(
        &self,
        quiz_id: Uuid,
        user_id: Uuid,
    ) -> Result<AttemptSessionResponse, AppError> {
        let mut transaction = self.pool.begin().await?;
        let quiz = sqlx::query_as::<
            _,
            (
                String,
                DateTime<Utc>,
                DateTime<Utc>,
                Option<DateTime<Utc>>,
                i16,
                DateTime<Utc>,
            ),
        >(
            r#"SELECT status, starts_at, ends_at, manually_closed_at, max_attempts, clock_timestamp()
               FROM quizzes WHERE id = $1 FOR UPDATE"#,
        )
        .bind(quiz_id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or_else(|| AppError::not_found("Quiz"))?;
        ensure_open(&quiz.0, quiz.1, quiz.2, quiz.3, quiz.5)?;

        let eligible = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM quiz_eligibilities WHERE quiz_id = $1 AND user_id = $2)",
        )
        .bind(quiz_id)
        .bind(user_id)
        .fetch_one(&mut *transaction)
        .await?;
        if !eligible {
            return Err(AppError::forbidden("You are not eligible for this quiz"));
        }

        let completed_attempts = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM quiz_attempts WHERE quiz_id = $1 AND user_id = $2",
        )
        .bind(quiz_id)
        .bind(user_id)
        .fetch_one(&mut *transaction)
        .await?;
        if completed_attempts >= i64::from(quiz.4) {
            return Err(attempt_limit_reached());
        }

        if let Some(session) = sqlx::query_as::<_, (Uuid, Uuid, DateTime<Utc>)>(
            r#"SELECT id, quiz_revision_id, started_at FROM attempt_sessions
               WHERE quiz_id = $1 AND user_id = $2 AND status = 'in_progress'"#,
        )
        .bind(quiz_id)
        .bind(user_id)
        .fetch_optional(&mut *transaction)
        .await?
        {
            let questions = load_public_questions(&mut transaction, session.1).await?;
            transaction.commit().await?;
            return Ok(AttemptSessionResponse {
                session_id: session.0,
                quiz_id,
                quiz_revision_id: session.1,
                started_at: session.2,
                deadline: quiz.2,
                attempt_number: completed_attempts as u8 + 1,
                max_attempts: quiz.4 as u8,
                remaining_attempts: (quiz.4 as u8).saturating_sub(completed_attempts as u8),
                questions,
            });
        }

        let revision_id = sqlx::query_scalar::<_, Uuid>(
            r#"SELECT id FROM quiz_revisions
               WHERE quiz_id = $1 ORDER BY revision_no DESC LIMIT 1"#,
        )
        .bind(quiz_id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or_else(|| AppError::conflict("QUIZ_NOT_READY", "Quiz has no published revision"))?;
        let questions = load_public_questions(&mut transaction, revision_id).await?;
        if questions.is_empty() {
            return Err(AppError::conflict(
                "QUIZ_NOT_READY",
                "Quiz has no published questions",
            ));
        }

        let session = sqlx::query_as::<_, (Uuid, DateTime<Utc>)>(
            r#"INSERT INTO attempt_sessions (quiz_id, user_id, quiz_revision_id)
               VALUES ($1, $2, $3) RETURNING id, started_at"#,
        )
        .bind(quiz_id)
        .bind(user_id)
        .bind(revision_id)
        .fetch_one(&mut *transaction)
        .await?;
        transaction.commit().await?;

        Ok(AttemptSessionResponse {
            session_id: session.0,
            quiz_id,
            quiz_revision_id: revision_id,
            started_at: session.1,
            deadline: quiz.2,
            attempt_number: completed_attempts as u8 + 1,
            max_attempts: quiz.4 as u8,
            remaining_attempts: (quiz.4 as u8).saturating_sub(completed_attempts as u8),
            questions,
        })
    }

    pub async fn session_questions(
        &self,
        session_id: Uuid,
        user_id: Uuid,
    ) -> Result<Vec<PublicQuestionResponse>, AppError> {
        let row = sqlx::query_as::<
            _,
            (
                Uuid,
                String,
                DateTime<Utc>,
                DateTime<Utc>,
                Option<DateTime<Utc>>,
                DateTime<Utc>,
            ),
        >(
            r#"SELECT s.quiz_revision_id, q.status, q.starts_at, q.ends_at,
                      q.manually_closed_at, clock_timestamp()
               FROM attempt_sessions s JOIN quizzes q ON q.id = s.quiz_id
               WHERE s.id = $1 AND s.user_id = $2 AND s.status = 'in_progress'"#,
        )
        .bind(session_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::not_found("Attempt session"))?;
        ensure_open(&row.1, row.2, row.3, row.4, row.5)?;

        let mut transaction = self.pool.begin().await?;
        let questions = load_public_questions(&mut transaction, row.0).await?;
        transaction.commit().await?;
        Ok(questions)
    }

    pub async fn submit(
        &self,
        session_id: Uuid,
        user_id: Uuid,
        answers: &[AnswerInput],
    ) -> Result<SubmitAttemptResponse, AppError> {
        let request_hash = canonical_request_hash(answers);
        let mut transaction = self.pool.begin().await?;

        if let Some(existing) =
            find_attempt_by_session(&mut transaction, session_id, user_id).await?
        {
            transaction.commit().await?;
            if existing.6 == request_hash {
                return Ok(to_submit_response(existing, true));
            }
            return Err(AppError::conflict(
                "SESSION_ALREADY_SUBMITTED",
                "This session was already submitted with different answers",
            ));
        }

        let session = sqlx::query_as::<
            _,
            (
                Uuid,
                Uuid,
                String,
                DateTime<Utc>,
                DateTime<Utc>,
                Option<DateTime<Utc>>,
                i16,
                DateTime<Utc>,
            ),
        >(
            r#"SELECT s.quiz_id, s.quiz_revision_id, q.status, q.starts_at, q.ends_at,
                      q.manually_closed_at, q.max_attempts, clock_timestamp()
               FROM attempt_sessions s JOIN quizzes q ON q.id = s.quiz_id
               WHERE s.id = $1 AND s.user_id = $2 AND s.status = 'in_progress'
               FOR UPDATE OF s, q"#,
        )
        .bind(session_id)
        .bind(user_id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or_else(|| AppError::not_found("Attempt session"))?;
        ensure_open(&session.2, session.3, session.4, session.5, session.7)?;

        let completed_attempts = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM quiz_attempts WHERE quiz_id = $1 AND user_id = $2",
        )
        .bind(session.0)
        .bind(user_id)
        .fetch_one(&mut *transaction)
        .await?;
        if completed_attempts >= i64::from(session.6) {
            return Err(attempt_limit_reached());
        }
        let attempt_no = completed_attempts as i16 + 1;

        let question_rows = sqlx::query_as::<_, (Uuid, i32, i32)>(
            r#"SELECT id, jsonb_array_length(options), correct_option_index
               FROM question_versions WHERE quiz_revision_id = $1 ORDER BY position"#,
        )
        .bind(session.1)
        .fetch_all(&mut *transaction)
        .await?;
        let questions: Vec<QuestionKey> = question_rows
            .into_iter()
            .map(|row| QuestionKey {
                question_id: row.0,
                option_count: row.1 as usize,
                correct_option_index: row.2 as u16,
            })
            .collect();
        let scored = validate_and_score(&questions, answers).map_err(map_answer_error)?;
        let score = scored.iter().filter(|answer| answer.is_correct).count() as u32;
        let total = scored.len() as u32;

        let attempt = sqlx::query_as::<_, (Uuid, DateTime<Utc>)>(
            r#"INSERT INTO quiz_attempts
               (session_id, quiz_id, user_id, quiz_revision_id, attempt_no, request_hash, score, total_questions)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
               RETURNING id, submitted_at"#,
        )
        .bind(session_id)
        .bind(session.0)
        .bind(user_id)
        .bind(session.1)
        .bind(attempt_no)
        .bind(&request_hash)
        .bind(score as i32)
        .bind(total as i32)
        .fetch_one(&mut *transaction)
        .await?;

        for answer in scored {
            sqlx::query(
                r#"INSERT INTO attempt_answers
                   (attempt_id, quiz_revision_id, question_version_id, selected_option_index, is_correct)
                   VALUES ($1, $2, $3, $4, $5)"#,
            )
            .bind(attempt.0)
            .bind(session.1)
            .bind(answer.question_id)
            .bind(i32::from(answer.selected_option_index))
            .bind(answer.is_correct)
            .execute(&mut *transaction)
            .await?;
        }
        sqlx::query("UPDATE attempt_sessions SET status = 'submitted' WHERE id = $1")
            .bind(session_id)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;

        Ok(SubmitAttemptResponse {
            attempt_id: attempt.0,
            quiz_id: session.0,
            attempt_number: attempt_no as u8,
            max_attempts: session.6 as u8,
            remaining_attempts: (session.6 - attempt_no) as u8,
            score,
            total_questions: total,
            percentage: percentage(score, total),
            submitted_at: attempt.1,
            idempotent_replay: false,
        })
    }

    pub async fn my_attempts(
        &self,
        quiz_id: Uuid,
        user_id: Uuid,
    ) -> Result<Vec<MyAttemptResponse>, AppError> {
        let rows = sqlx::query_as::<_, (Uuid, Uuid, i16, i32, i32, DateTime<Utc>)>(
            r#"SELECT id, quiz_id, attempt_no, score, total_questions, submitted_at
               FROM quiz_attempts WHERE quiz_id = $1 AND user_id = $2
               ORDER BY submitted_at"#,
        )
        .bind(quiz_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| MyAttemptResponse {
                attempt_id: row.0,
                quiz_id: row.1,
                attempt_number: row.2 as u8,
                score: row.3 as u32,
                total_questions: row.4 as u32,
                percentage: percentage(row.3 as u32, row.4 as u32),
                submitted_at: row.5,
            })
            .collect())
    }
}

type ExistingAttempt = (Uuid, Uuid, i32, i32, DateTime<Utc>, Uuid, String, i16, i16);

async fn find_attempt_by_session(
    transaction: &mut Transaction<'_, Postgres>,
    session_id: Uuid,
    user_id: Uuid,
) -> Result<Option<ExistingAttempt>, AppError> {
    Ok(sqlx::query_as::<_, ExistingAttempt>(
        r#"SELECT qa.id, qa.quiz_id, qa.score, qa.total_questions, qa.submitted_at,
                  qa.user_id, qa.request_hash, qa.attempt_no, q.max_attempts
           FROM quiz_attempts qa JOIN quizzes q ON q.id = qa.quiz_id
           WHERE qa.session_id = $1 AND qa.user_id = $2"#,
    )
    .bind(session_id)
    .bind(user_id)
    .fetch_optional(&mut **transaction)
    .await?)
}

async fn load_public_questions(
    transaction: &mut Transaction<'_, Postgres>,
    revision_id: Uuid,
) -> Result<Vec<PublicQuestionResponse>, AppError> {
    let rows = sqlx::query_as::<_, (Uuid, i32, String, serde_json::Value)>(
        r#"SELECT id, position, question_text, options
           FROM question_versions WHERE quiz_revision_id = $1 ORDER BY position"#,
    )
    .bind(revision_id)
    .fetch_all(&mut **transaction)
    .await?;
    rows.into_iter()
        .map(|row| {
            let options =
                serde_json::from_value::<Vec<String>>(row.3).map_err(|_| AppError::internal())?;
            Ok(PublicQuestionResponse {
                question_id: row.0,
                position: row.1 as u32,
                question_text: row.2,
                options,
            })
        })
        .collect()
}

fn ensure_open(
    status: &str,
    starts_at: DateTime<Utc>,
    ends_at: DateTime<Utc>,
    manually_closed_at: Option<DateTime<Utc>>,
    database_time: DateTime<Utc>,
) -> Result<(), AppError> {
    if status == "open"
        && manually_closed_at.is_none()
        && starts_at <= database_time
        && database_time < ends_at
    {
        Ok(())
    } else {
        Err(AppError::conflict(
            "QUIZ_NOT_AVAILABLE",
            "Quiz is not currently available",
        ))
    }
}

fn map_answer_error(error: AnswerSetError) -> AppError {
    let message = match error {
        AnswerSetError::MissingAnswers => "Every published question must be answered exactly once",
        AnswerSetError::DuplicateQuestion => "A question was answered more than once",
        AnswerSetError::ForeignQuestion => {
            "An answer references a question outside this quiz revision"
        }
        AnswerSetError::OptionOutOfRange => {
            "A selected option index is outside the question options"
        }
    };
    AppError::invalid_answers(message)
}

fn attempt_limit_reached() -> AppError {
    AppError::conflict(
        "ATTEMPT_LIMIT_REACHED",
        "No attempts remaining for this quiz",
    )
}

fn percentage(score: u32, total: u32) -> f64 {
    if total == 0 {
        0.0
    } else {
        f64::from(score) * 100.0 / f64::from(total)
    }
}

fn to_submit_response(existing: ExistingAttempt, idempotent_replay: bool) -> SubmitAttemptResponse {
    SubmitAttemptResponse {
        attempt_id: existing.0,
        quiz_id: existing.1,
        attempt_number: existing.7 as u8,
        max_attempts: existing.8 as u8,
        remaining_attempts: (existing.8 - existing.7) as u8,
        score: existing.2 as u32,
        total_questions: existing.3 as u32,
        percentage: percentage(existing.2 as u32, existing.3 as u32),
        submitted_at: existing.4,
        idempotent_replay,
    }
}
