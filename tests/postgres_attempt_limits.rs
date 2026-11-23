use chrono::{Duration, Utc};
use sphinx::{api::dto::AnswerInput, repository::participation::ParticipationRepository};
use sqlx::{PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

async fn reset_schema(pool: &PgPool) {
    sqlx::raw_sql(
        r#"DROP SCHEMA public CASCADE; CREATE SCHEMA public;
        CREATE EXTENSION IF NOT EXISTS pgcrypto;
        CREATE TABLE users (id UUID PRIMARY KEY);
        CREATE TABLE teams (id UUID PRIMARY KEY);
        CREATE TABLE team_members (
            team_id UUID REFERENCES teams(id), user_id UUID REFERENCES users(id),
            role TEXT NOT NULL, PRIMARY KEY(team_id,user_id));
        CREATE TABLE quizzes (
            id UUID PRIMARY KEY, team_id UUID REFERENCES teams(id), title TEXT NOT NULL,
            status TEXT NOT NULL, starts_at TIMESTAMPTZ NOT NULL, ends_at TIMESTAMPTZ NOT NULL,
            manually_closed_at TIMESTAMPTZ, max_attempts SMALLINT NOT NULL);
        CREATE TABLE quiz_revisions (
            id UUID PRIMARY KEY, quiz_id UUID REFERENCES quizzes(id), revision_no INT NOT NULL,
            UNIQUE(quiz_id,id));
        CREATE TABLE question_versions (
            id UUID PRIMARY KEY, quiz_revision_id UUID REFERENCES quiz_revisions(id),
            position INT NOT NULL, question_text TEXT NOT NULL, options JSONB NOT NULL,
            correct_option_index INT NOT NULL, explanation TEXT NOT NULL,
            UNIQUE(quiz_revision_id,id));
        CREATE TABLE quiz_eligibilities (
            quiz_id UUID REFERENCES quizzes(id), user_id UUID REFERENCES users(id),
            PRIMARY KEY(quiz_id,user_id));"#,
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!("../migrations/0030_participation.sql"))
        .execute(pool)
        .await
        .unwrap();
}

async fn seed_quiz(pool: &PgPool, max_attempts: i16) -> (Uuid, Uuid, Uuid) {
    let user = Uuid::new_v4();
    let team = Uuid::new_v4();
    let quiz = Uuid::new_v4();
    let revision = Uuid::new_v4();
    let question = Uuid::new_v4();
    sqlx::query("INSERT INTO users VALUES($1)")
        .bind(user)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO teams VALUES($1)")
        .bind(team)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO quizzes VALUES($1,$2,'Attempts','open',$3,$4,NULL,$5)")
        .bind(quiz)
        .bind(team)
        .bind(Utc::now() - Duration::minutes(1))
        .bind(Utc::now() + Duration::minutes(10))
        .bind(max_attempts)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO quiz_revisions VALUES($1,$2,1)")
        .bind(revision)
        .bind(quiz)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO question_versions VALUES($1,$2,0,'2+2?','[\"3\",\"4\"]',1,'Four')")
        .bind(question)
        .bind(revision)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO quiz_eligibilities VALUES($1,$2)")
        .bind(quiz)
        .bind(user)
        .execute(pool)
        .await
        .unwrap();
    (quiz, user, question)
}

async fn submit_attempt(
    repository: &ParticipationRepository,
    quiz: Uuid,
    user: Uuid,
    question: Uuid,
) -> sphinx::api::dto::SubmitAttemptResponse {
    let session = repository.start_session(quiz, user).await.unwrap();
    repository
        .submit(
            session.session_id,
            user,
            &[AnswerInput {
                question_id: question,
                selected_option_index: 1,
            }],
        )
        .await
        .unwrap()
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL database"]
async fn honors_one_and_two_attempt_quiz_limits() {
    let database_url = std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL is required");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .unwrap();
    reset_schema(&pool).await;
    let repository = ParticipationRepository::new(pool.clone());

    let (one_quiz, one_user, one_question) = seed_quiz(&pool, 1).await;
    let first = submit_attempt(&repository, one_quiz, one_user, one_question).await;
    assert_eq!(first.attempt_number, 1);
    assert_eq!(first.remaining_attempts, 0);
    assert_eq!(
        repository
            .start_session(one_quiz, one_user)
            .await
            .unwrap_err()
            .code(),
        "ATTEMPT_LIMIT_REACHED"
    );

    let (two_quiz, two_user, two_question) = seed_quiz(&pool, 2).await;
    let first = submit_attempt(&repository, two_quiz, two_user, two_question).await;
    assert_eq!(first.attempt_number, 1);
    assert_eq!(first.remaining_attempts, 1);
    let second = submit_attempt(&repository, two_quiz, two_user, two_question).await;
    assert_eq!(second.attempt_number, 2);
    assert_eq!(second.remaining_attempts, 0);
    assert_eq!(
        repository
            .start_session(two_quiz, two_user)
            .await
            .unwrap_err()
            .code(),
        "ATTEMPT_LIMIT_REACHED"
    );

    let attempts = repository.my_attempts(two_quiz, two_user).await.unwrap();
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[0].attempt_number, 1);
    assert_eq!(attempts[1].attempt_number, 2);
}
