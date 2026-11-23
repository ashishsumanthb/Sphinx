use std::time::Duration as StdDuration;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::{Duration, Utc};
use http_body_util::BodyExt;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, postgres::PgPoolOptions};
use sphinx::{
    AppState,
    api::{dto::AnswerInput, routes::participation_routes},
    repository::participation::ParticipationRepository,
};
use tower::ServiceExt;
use uuid::Uuid;

const DEFAULT_DATABASE_URL: &str = "postgres://postgres:edgepass@127.0.0.1:55432/sphinx_edge";

async fn reset(pool: &PgPool) {
    sqlx::raw_sql(
        r#"DROP SCHEMA public CASCADE; CREATE SCHEMA public;
        CREATE EXTENSION IF NOT EXISTS pgcrypto;"#,
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!("../migrations/001_identity_and_teams.sql"))
        .execute(pool)
        .await
        .unwrap();
    sqlx::raw_sql(
        r#"CREATE TABLE quizzes (
            id UUID PRIMARY KEY, team_id UUID REFERENCES teams(id), title TEXT NOT NULL,
            status TEXT NOT NULL, starts_at TIMESTAMPTZ NOT NULL, ends_at TIMESTAMPTZ NOT NULL,
            manually_closed_at TIMESTAMPTZ, max_attempts SMALLINT NOT NULL DEFAULT 1);
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

struct Seed {
    team: Uuid,
    admin: Uuid,
    member: Uuid,
    other: Uuid,
    quiz: Uuid,
    revision: Uuid,
    question: Uuid,
}

async fn seed(pool: &PgPool, starts: chrono::DateTime<Utc>, ends: chrono::DateTime<Utc>) -> Seed {
    let seed = Seed {
        team: Uuid::new_v4(), admin: Uuid::new_v4(), member: Uuid::new_v4(),
        other: Uuid::new_v4(), quiz: Uuid::new_v4(), revision: Uuid::new_v4(),
        question: Uuid::new_v4(),
    };
    sqlx::query(
        "INSERT INTO users(id,name,email,password_hash) VALUES
         ($1,'Admin',$4,'hash'),($2,'Member',$5,'hash'),($3,'Other',$6,'hash')",
    )
        .bind(seed.admin).bind(seed.member).bind(seed.other)
        .bind(format!("{}@example.com", seed.admin))
        .bind(format!("{}@example.com", seed.member))
        .bind(format!("{}@example.com", seed.other))
        .execute(pool).await.unwrap();
    sqlx::query("INSERT INTO teams(id,name,created_by) VALUES ($1,'Edge team',$2)")
        .bind(seed.team).bind(seed.admin).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO team_memberships(team_id,user_id,role) VALUES ($1,$2,'admin'),($1,$3,'member'),($1,$4,'member')")
        .bind(seed.team).bind(seed.admin).bind(seed.member).bind(seed.other).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO quizzes(id,team_id,title,status,starts_at,ends_at,max_attempts) VALUES($1,$2,'Edge quiz','open',$3,$4,1)")
        .bind(seed.quiz).bind(seed.team).bind(starts).bind(ends).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO quiz_revisions(id,quiz_id,revision_no) VALUES($1,$2,1)")
        .bind(seed.revision).bind(seed.quiz).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO question_versions VALUES($1,$2,0,'2+2?', '[\"3\",\"4\"]',1,'Four')")
        .bind(seed.question).bind(seed.revision).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO quiz_eligibilities VALUES($1,$2),($1,$3)")
        .bind(seed.quiz).bind(seed.member).bind(seed.other).execute(pool).await.unwrap();
    seed
}

async fn assert_logged_in_oversized_request_returns_413(pool: &PgPool, user_id: Uuid) {
    let token = "ab".repeat(32);
    let token_hash = Sha256::digest(token.as_bytes()).to_vec();
    sqlx::query(
        "INSERT INTO login_sessions(token_hash,user_id,expires_at)
         VALUES($1,$2,clock_timestamp()+interval '1 hour')",
    )
    .bind(token_hash)
    .bind(user_id)
    .execute(pool)
    .await
    .unwrap();

    let request = Request::builder()
        .method("POST")
        .uri(format!(
            "/api/v1/attempt-sessions/{}/submit",
            Uuid::new_v4()
        ))
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from("x".repeat(1024 * 1024 + 1)))
        .unwrap();
    let response = participation_routes()
        .with_state(AppState { pool: pool.clone() })
        .oneshot(request)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["error"]["code"], "PAYLOAD_TOO_LARGE");
    assert!(json["error"]["request_id"]
        .as_str()
        .unwrap()
        .starts_with("req_"));
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL database; see docs/PARTICIPATION_API.md"]
async fn postgres_edge_case_matrix() {
    let database_url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_owned());
    let pool = PgPoolOptions::new().max_connections(10).connect(&database_url).await.unwrap();
    reset(&pool).await;
    let repo = ParticipationRepository::new(pool.clone());

    // Start is inclusive; before start and at/after the exclusive end are rejected.
    let base = seed(&pool, Utc::now() + Duration::minutes(1), Utc::now() + Duration::minutes(2)).await;
    assert_logged_in_oversized_request_returns_413(&pool, base.member).await;
    assert_eq!(repo.start_session(base.quiz, base.member).await.unwrap_err().code(), "QUIZ_NOT_AVAILABLE");
    sqlx::query("UPDATE quizzes SET starts_at=clock_timestamp(), ends_at=clock_timestamp()+interval '5 minutes' WHERE id=$1")
        .bind(base.quiz).execute(&pool).await.unwrap();

    let late_session = repo.start_session(base.quiz, base.other).await.unwrap();
    sqlx::query("UPDATE quizzes SET ends_at=clock_timestamp() WHERE id=$1")
        .bind(base.quiz).execute(&pool).await.unwrap();
    let late_answers = vec![AnswerInput { question_id: base.question, selected_option_index: 1 }];
    assert_eq!(
        repo.submit(late_session.session_id, base.other, &late_answers)
            .await.unwrap_err().code(),
        "QUIZ_NOT_AVAILABLE"
    );
    sqlx::query("UPDATE quizzes SET ends_at=clock_timestamp()+interval '5 minutes' WHERE id=$1")
        .bind(base.quiz).execute(&pool).await.unwrap();

    // Concurrent starts resolve to one active session.
    let (left, right) = tokio::join!(repo.start_session(base.quiz, base.member), repo.start_session(base.quiz, base.member));
    let left = left.unwrap();
    let right = right.unwrap();
    assert_eq!(left.session_id, right.session_id);
    assert_eq!(sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM attempt_sessions WHERE user_id=$1")
        .bind(base.member).fetch_one(&pool).await.unwrap(), 1);

    let answers = vec![AnswerInput { question_id: base.question, selected_option_index: 1 }];
    // Concurrent identical submits create one attempt and return one replay.
    let (first, second) = tokio::join!(
        repo.submit(left.session_id, base.member, &answers),
        repo.submit(left.session_id, base.member, &answers)
    );
    let first = first.unwrap();
    let second = second.unwrap();
    assert_eq!(first.attempt_id, second.attempt_id);
    assert_ne!(first.idempotent_replay, second.idempotent_replay);
    assert_eq!(sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM quiz_attempts").fetch_one(&pool).await.unwrap(), 1);

    // Retry remains idempotent after constructing a fresh repository; changed retry conflicts.
    let restarted = ParticipationRepository::new(pool.clone());
    assert!(restarted.submit(left.session_id, base.member, &answers).await.unwrap().idempotent_replay);
    let changed = vec![AnswerInput { question_id: base.question, selected_option_index: 0 }];
    assert_eq!(restarted.submit(left.session_id, base.member, &changed).await.unwrap_err().code(), "SESSION_ALREADY_SUBMITTED");

    // Review is owner/admin-only.
    assert!(repo.attempt_review(first.attempt_id, base.member).await.is_ok());
    assert!(repo.attempt_review(first.attempt_id, base.admin).await.is_ok());
    let stranger = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,name,email,password_hash) VALUES($1,'Stranger',$2,'hash')")
        .bind(stranger)
        .bind(format!("{stranger}@example.com"))
        .execute(&pool).await.unwrap();
    assert_eq!(repo.attempt_review(first.attempt_id, stranger).await.unwrap_err().status(), axum::http::StatusCode::FORBIDDEN);

    // Foreign keys and uniqueness are enforced by PostgreSQL.
    let invalid = sqlx::query("INSERT INTO attempt_sessions(quiz_id,user_id,quiz_revision_id) VALUES($1,$2,$3)")
        .bind(base.quiz).bind(stranger).bind(base.revision).execute(&pool).await;
    assert!(invalid.is_err());
    let duplicate = sqlx::query("INSERT INTO quiz_attempts(session_id,quiz_id,user_id,quiz_revision_id,attempt_no,request_hash,score,total_questions) SELECT session_id,quiz_id,user_id,quiz_revision_id,attempt_no,request_hash,score,total_questions FROM quiz_attempts LIMIT 1")
        .execute(&pool).await;
    let duplicate_error = duplicate.unwrap_err();
    assert_eq!(
        duplicate_error
            .as_database_error()
            .and_then(|error| error.constraint()),
        Some("quiz_attempts_session_id_key")
    );

    // Exact end is exclusive.
    sqlx::query("UPDATE quizzes SET ends_at=clock_timestamp() WHERE id=$1").bind(base.quiz).execute(&pool).await.unwrap();
    assert_eq!(repo.start_session(base.quiz, base.other).await.unwrap_err().code(), "QUIZ_NOT_AVAILABLE");

    // Empty analytics cohort returns null participation.
    let empty_quiz = Uuid::new_v4();
    sqlx::query("INSERT INTO quizzes(id,team_id,title,status,starts_at,ends_at,max_attempts) VALUES($1,$2,'Empty','closed',clock_timestamp()-interval '1 hour',clock_timestamp(),1)")
        .bind(empty_quiz).bind(base.team).execute(&pool).await.unwrap();
    let empty = repo.quiz_analytics(empty_quiz, base.admin).await.unwrap();
    assert_eq!(empty.participation_rate, None);

    // A close that obtains the quiz lock first wins over submission admission.
    let close_seed = seed(&pool, Utc::now() - Duration::minutes(1), Utc::now() + Duration::minutes(5)).await;
    let close_session = repo.start_session(close_seed.quiz, close_seed.member).await.unwrap();
    let mut close_tx = pool.begin().await.unwrap();
    sqlx::query("UPDATE quizzes SET manually_closed_at=clock_timestamp() WHERE id=$1")
        .bind(close_seed.quiz).execute(&mut *close_tx).await.unwrap();
    let close_repo = repo.clone();
    let close_answers = vec![AnswerInput { question_id: close_seed.question, selected_option_index: 1 }];
    let submit = tokio::spawn(async move { close_repo.submit(close_session.session_id, close_seed.member, &close_answers).await });
    tokio::task::yield_now().await;
    close_tx.commit().await.unwrap();
    assert_eq!(submit.await.unwrap().unwrap_err().code(), "QUIZ_NOT_AVAILABLE");

    // A forced failure during answer insertion rolls back attempt and session state.
    let rollback_seed = seed(&pool, Utc::now() - Duration::minutes(1), Utc::now() + Duration::minutes(5)).await;
    let rollback_session = repo.start_session(rollback_seed.quiz, rollback_seed.member).await.unwrap();
    sqlx::raw_sql("CREATE FUNCTION fail_answer() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'forced'; END $$; CREATE TRIGGER force_answer_failure BEFORE INSERT ON attempt_answers FOR EACH ROW EXECUTE FUNCTION fail_answer();")
        .execute(&pool).await.unwrap();
    let rollback_answers = vec![AnswerInput { question_id: rollback_seed.question, selected_option_index: 1 }];
    assert!(repo.submit(rollback_session.session_id, rollback_seed.member, &rollback_answers).await.is_err());
    assert_eq!(sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM quiz_attempts WHERE quiz_id=$1").bind(rollback_seed.quiz).fetch_one(&pool).await.unwrap(), 0);
    assert_eq!(sqlx::query_scalar::<_, String>("SELECT status FROM attempt_sessions WHERE id=$1").bind(rollback_session.session_id).fetch_one(&pool).await.unwrap(), "in_progress");
    sqlx::raw_sql("DROP TRIGGER force_answer_failure ON attempt_answers; DROP FUNCTION fail_answer();").execute(&pool).await.unwrap();

    // Analytics ranks each participant's best attempt per revision. Submission
    // counts and averages still include every stored attempt.
    let ranked_quiz = Uuid::new_v4();
    let revision_one = Uuid::new_v4();
    let revision_two = Uuid::new_v4();
    sqlx::query("INSERT INTO quizzes(id,team_id,title,status,starts_at,ends_at,max_attempts) VALUES($1,$2,'Ranks','closed',clock_timestamp()-interval '1 hour',clock_timestamp(),2)")
        .bind(ranked_quiz).bind(base.team).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO quiz_revisions VALUES($1,$3,1),($2,$3,2)")
        .bind(revision_one).bind(revision_two).bind(ranked_quiz).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO quiz_eligibilities VALUES($1,$2),($1,$3),($1,$4)")
        .bind(ranked_quiz).bind(base.member).bind(base.other).bind(stranger).execute(&pool).await.unwrap();
    let session_a = Uuid::new_v4();
    let session_b = Uuid::new_v4();
    let session_c = Uuid::new_v4();
    let session_d = Uuid::new_v4();
    sqlx::query("INSERT INTO attempt_sessions(id,quiz_id,user_id,quiz_revision_id,status) VALUES($1,$5,$6,$8,'submitted'),($2,$5,$6,$8,'submitted'),($3,$5,$7,$8,'submitted'),($4,$5,$9,$10,'submitted')")
        .bind(session_a).bind(session_b).bind(session_c).bind(session_d).bind(ranked_quiz)
        .bind(base.member).bind(base.other).bind(revision_one).bind(stranger).bind(revision_two)
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO quiz_attempts(session_id,quiz_id,user_id,quiz_revision_id,attempt_no,request_hash,score,total_questions) VALUES($1,$5,$6,$8,1,$11,10,10),($2,$5,$6,$8,2,$11,9,10),($3,$5,$7,$8,1,$11,8,10),($4,$5,$9,$10,1,$11,5,10)")
        .bind(session_a).bind(session_b).bind(session_c).bind(session_d).bind(ranked_quiz)
        .bind(base.member).bind(base.other).bind(revision_one).bind(stranger).bind(revision_two)
        .bind("a".repeat(64)).execute(&pool).await.unwrap();
    let analytics = repo.quiz_analytics(ranked_quiz, base.admin).await.unwrap();
    assert_eq!(analytics.participation_rate, Some(100.0));
    assert_eq!(analytics.total_submissions, 4);
    assert_eq!(analytics.unique_participants, 3);
    assert_eq!(analytics.average_score_percentage, Some(80.0));
    assert_eq!(analytics.participants.len(), 3);
    let member_rank = analytics.participants.iter()
        .find(|row| row.user_id == base.member && row.revision_number == 1).unwrap();
    assert_eq!((member_rank.score, member_rank.rank), (10, 1));
    let other_rank = analytics.participants.iter()
        .find(|row| row.user_id == base.other && row.revision_number == 1).unwrap();
    assert_eq!((other_rank.score, other_rank.rank), (8, 2));
    let revision_two_rank = analytics.participants.iter()
        .find(|row| row.user_id == stranger && row.revision_number == 2).unwrap();
    assert_eq!(revision_two_rank.rank, 1);

    // Unavailable database maps to a safe 503 error.
    let dead = PgPoolOptions::new().acquire_timeout(StdDuration::from_millis(100))
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused").unwrap();
    let error = ParticipationRepository::new(dead).list_available(Uuid::new_v4(), Uuid::new_v4()).await.unwrap_err();
    assert_eq!(error.status(), axum::http::StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(error.code(), "DB_UNAVAILABLE");
}
