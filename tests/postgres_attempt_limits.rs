use chrono::{Duration, Utc};
use sphinx::{api::dto::AnswerInput, repository::participation::ParticipationRepository};
use sqlx::{PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

async fn reset_schema(pool: &PgPool) {
    sqlx::raw_sql("DROP SCHEMA public CASCADE; CREATE SCHEMA public;")
        .execute(pool)
        .await
        .unwrap();
    sqlx::migrate::Migrator::new(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations"
    )))
    .await
    .unwrap()
    .run(pool)
    .await
    .unwrap();
}

async fn seed_quiz(pool: &PgPool, max_attempts: i16) -> (Uuid, Uuid, Uuid) {
    let user = Uuid::new_v4();
    let team = Uuid::new_v4();
    let quiz = Uuid::new_v4();
    let revision = Uuid::new_v4();
    let question = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,name,email,password_encoded) VALUES($1,'Member',$2,'hash')")
        .bind(user)
        .bind(format!("{user}@example.com"))
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO teams(id,name,created_by) VALUES($1,'Attempts team',$2)")
        .bind(team)
        .bind(user)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO team_memberships(team_id,user_id,role) VALUES($1,$2,'member')")
        .bind(team)
        .bind(user)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO quizzes(id,team_id,created_by,title,status,starts_at,ends_at,max_attempts)
         VALUES($1,$2,$6,'Attempts','open',$3,$4,$5)",
    )
    .bind(quiz)
    .bind(team)
    .bind(Utc::now() - Duration::minutes(1))
    .bind(Utc::now() + Duration::minutes(10))
    .bind(max_attempts)
    .bind(user)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO quiz_revisions(id,quiz_id,revision_no,created_by,change_reason) VALUES($1,$2,1,$3,'Initial revision')")
        .bind(revision)
        .bind(quiz)
        .bind(user)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO question_versions(id,quiz_revision_id,stable_question_id,position,question_text,options,correct_option_index,explanation)
         VALUES($1,$2,$1,0,'2+2?','[\"3\",\"4\"]',1,'Four')",
    )
    .bind(question)
    .bind(revision)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO quiz_eligibilities(quiz_id,user_id) VALUES($1,$2)")
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
