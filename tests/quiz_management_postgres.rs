use axum::{
    Router,
    body::Body,
    http::{
        Method, Request, StatusCode,
        header::{AUTHORIZATION, CONTENT_TYPE},
    },
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sphinx::{AppState, app};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tower::ServiceExt;
use uuid::Uuid;

async fn reset_schema(pool: &PgPool) {
    sqlx::raw_sql(
        "DROP SCHEMA public CASCADE;
         CREATE SCHEMA public;
         CREATE EXTENSION IF NOT EXISTS pgcrypto;",
    )
    .execute(pool)
    .await
    .unwrap();

    for migration in [
        include_str!("../migrations/001_identity_and_teams.sql"),
        include_str!("../migrations/002_rename_password_column.sql"),
        include_str!("../migrations/0020_quiz_management.sql"),
        include_str!("../migrations/0030_participation.sql"),
    ] {
        sqlx::raw_sql(migration).execute(pool).await.unwrap();
    }
}

async fn call(
    app: Router,
    method: Method,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder().method(method).uri(uri);

    if let Some(token) = token {
        request = request.header(AUTHORIZATION, format!("Bearer {token}"));
    }

    let request_body = match body {
        Some(value) => {
            request = request.header(CONTENT_TYPE, "application/json");
            Body::from(value.to_string())
        }
        None => Body::empty(),
    };

    let response = app
        .oneshot(request.body(request_body).unwrap())
        .await
        .unwrap();

    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();

    let response_body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };

    (status, response_body)
}

async fn register_user(app: Router, name: &str, email: &str, password: &str) -> Uuid {
    let (status, response) = call(
        app,
        Method::POST,
        "/api/v1/auth/register",
        None,
        Some(json!({
            "name": name,
            "email": email,
            "password": password
        })),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
    Uuid::parse_str(response["id"].as_str().unwrap()).unwrap()
}

async fn login(app: Router, email: &str, password: &str) -> String {
    let (status, response) = call(
        app,
        Method::POST,
        "/api/v1/auth/login",
        None,
        Some(json!({
            "email": email,
            "password": password
        })),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    response["token"].as_str().unwrap().to_owned()
}

async fn create_team(app: Router, token: &str) -> Uuid {
    let (status, response) = call(
        app,
        Method::POST,
        "/api/v1/teams",
        Some(token),
        Some(json!({
            "name": "Quiz Management PostgreSQL Test Team"
        })),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
    Uuid::parse_str(response["id"].as_str().unwrap()).unwrap()
}
#[tokio::test]
#[ignore = "requires an isolated PostgreSQL database"]
async fn quiz_management_api_is_transactional_and_revision_safe() {
    let database_url = std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL is required");

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .unwrap();

    reset_schema(&pool).await;

    let app = app(AppState { pool: pool.clone() });

    let _admin_id = register_user(
        app.clone(),
        "Quiz Admin",
        "quiz-admin-postgres@example.com",
        "LocalQuizTest123!",
    )
    .await;

    let member_id = register_user(
        app.clone(),
        "Quiz Member",
        "quiz-member-postgres@example.com",
        "LocalMemberTest123!",
    )
    .await;

    let admin_token = login(
        app.clone(),
        "quiz-admin-postgres@example.com",
        "LocalQuizTest123!",
    )
    .await;

    let member_token = login(
        app.clone(),
        "quiz-member-postgres@example.com",
        "LocalMemberTest123!",
    )
    .await;

    let team_id = create_team(app.clone(), &admin_token).await;

    let (status, _) = call(
        app.clone(),
        Method::POST,
        &format!("/api/v1/teams/{team_id}/members"),
        Some(&admin_token),
        Some(json!({
            "user_id": member_id
        })),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);

    let quiz_count_before = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM quizzes")
        .fetch_one(&pool)
        .await
        .unwrap();

    let (status, _) = call(
        app.clone(),
        Method::POST,
        &format!("/api/v1/teams/{team_id}/quizzes"),
        Some(&admin_token),
        Some(json!({
            "title": "Invalid attempt limit",
            "max_attempts": 3
        })),
    )
    .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let quiz_count_after = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM quizzes")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(quiz_count_before, quiz_count_after);

    let (status, _) = call(
        app.clone(),
        Method::POST,
        &format!("/api/v1/teams/{team_id}/quizzes"),
        Some(&member_token),
        Some(json!({
            "title": "Unauthorized quiz",
            "max_attempts": 1
        })),
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, response) = call(
        app.clone(),
        Method::POST,
        &format!("/api/v1/teams/{team_id}/quizzes"),
        Some(&admin_token),
        Some(json!({
            "title": "PostgreSQL Quiz",
            "description": "Tests the complete quiz-management workflow",
            "max_attempts": 2
        })),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);

    let quiz_id = Uuid::parse_str(response["id"].as_str().unwrap()).unwrap();

    let (status, _) = call(
        app.clone(),
        Method::POST,
        &format!("/api/v1/teams/{team_id}/quizzes/{quiz_id}/questions"),
        Some(&admin_token),
        Some(json!({
            "question_text": "Invalid question",
            "options": ["Only one option"],
            "correct_option_index": 0
        })),
    )
    .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let question_count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM draft_questions WHERE quiz_id = $1")
            .bind(quiz_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(question_count, 0);

    let (status, response) = call(
        app.clone(),
        Method::POST,
        &format!("/api/v1/teams/{team_id}/quizzes/{quiz_id}/questions"),
        Some(&admin_token),
        Some(json!({
            "question_text": "Which HTTP status means Created?",
            "options": ["200 OK", "201 Created"],
            "correct_option_index": 1,
            "explanation": "201 Created indicates resource creation."
        })),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);

    let question_id = Uuid::parse_str(response["stable_question_id"].as_str().unwrap()).unwrap();

    let (status, response) = call(
        app.clone(),
        Method::PUT,
        &format!("/api/v1/teams/{team_id}/quizzes/{quiz_id}/questions/{question_id}"),
        Some(&admin_token),
        Some(json!({
            "question_text": "Which status represents Created?",
            "options": ["200 OK", "201 Created"],
            "correct_option_index": 1,
            "explanation": "201 Created represents successful creation."
        })),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(response.get("revision_no").is_none());

    let stored_draft_text = sqlx::query_scalar::<_, String>(
        "SELECT question_text
         FROM draft_questions
         WHERE quiz_id = $1 AND stable_question_id = $2",
    )
    .bind(quiz_id)
    .bind(question_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(stored_draft_text, "Which status represents Created?");

    let (status, response) = call(
        app.clone(),
        Method::POST,
        &format!("/api/v1/teams/{team_id}/quizzes"),
        Some(&admin_token),
        Some(json!({
            "title": "Empty quiz",
            "max_attempts": 1
        })),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);

    let empty_quiz_id = Uuid::parse_str(response["id"].as_str().unwrap()).unwrap();

    let (status, _) = call(
        app.clone(),
        Method::POST,
        &format!("/api/v1/quizzes/{empty_quiz_id}/publish"),
        Some(&admin_token),
        Some(json!({})),
    )
    .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (empty_status, empty_revision_count) = sqlx::query_as::<_, (String, i64)>(
        "SELECT status,
                    (SELECT COUNT(*)
                     FROM quiz_revisions
                     WHERE quiz_id = quizzes.id)
             FROM quizzes
             WHERE id = $1",
    )
    .bind(empty_quiz_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(empty_status, "draft");
    assert_eq!(empty_revision_count, 0);

    let publish_uri = format!("/api/v1/quizzes/{quiz_id}/publish");

    let first_publish = call(
        app.clone(),
        Method::POST,
        &publish_uri,
        Some(&admin_token),
        Some(json!({})),
    );

    let second_publish = call(
        app.clone(),
        Method::POST,
        &publish_uri,
        Some(&admin_token),
        Some(json!({})),
    );

    let (first_publish, second_publish) = tokio::join!(first_publish, second_publish);

    let publish_statuses = [first_publish.0, second_publish.0];

    assert!(publish_statuses.contains(&StatusCode::CREATED));
    assert!(publish_statuses.contains(&StatusCode::CONFLICT));

    let (quiz_status, revision_count, version_count, eligibility_count, duration_seconds) =
        sqlx::query_as::<_, (String, i64, i64, i64, f64)>(
            "SELECT status,
                (SELECT COUNT(*)
                 FROM quiz_revisions
                 WHERE quiz_id = quizzes.id),
                (SELECT COUNT(*)
                 FROM question_versions
                 JOIN quiz_revisions
                   ON quiz_revisions.id =
                      question_versions.quiz_revision_id
                 WHERE quiz_revisions.quiz_id = quizzes.id),
                (SELECT COUNT(*)
                 FROM quiz_eligibilities
                 WHERE quiz_id = quizzes.id),
                EXTRACT(EPOCH FROM (ends_at - starts_at))::DOUBLE PRECISION
         FROM quizzes
         WHERE id = $1",
        )
        .bind(quiz_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(quiz_status, "open");
    assert_eq!(revision_count, 1);
    assert_eq!(version_count, 1);
    assert_eq!(eligibility_count, 2);
    assert_eq!(duration_seconds, 900.0);

    let (status, first_close) = call(
        app.clone(),
        Method::POST,
        &format!("/api/v1/quizzes/{quiz_id}/close"),
        Some(&admin_token),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::OK);

    let (status, second_close) = call(
        app.clone(),
        Method::POST,
        &format!("/api/v1/quizzes/{quiz_id}/close"),
        Some(&admin_token),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        first_close["manually_closed_at"],
        second_close["manually_closed_at"]
    );

    let (status, response) = call(
        app.clone(),
        Method::PUT,
        &format!("/api/v1/teams/{team_id}/quizzes/{quiz_id}/questions/{question_id}"),
        Some(&admin_token),
        Some(json!({
            "question_text": "Which HTTP status means Accepted?",
            "options": ["201 Created", "202 Accepted"],
            "correct_option_index": 1,
            "explanation": "202 Accepted indicates accepted processing.",
            "change_reason": "Correct the question to cover Accepted"
        })),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(response["revision_no"].as_i64(), Some(2));

    let (status, _) = call(
        app.clone(),
        Method::PUT,
        &format!("/api/v1/teams/{team_id}/quizzes/{quiz_id}/questions/{question_id}"),
        Some(&admin_token),
        Some(json!({
            "question_text": "This must roll back",
            "options": ["Option A", "Option B"],
            "correct_option_index": 0,
            "explanation": "This must not be saved"
        })),
    )
    .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let revision_rows = sqlx::query_as::<_, (i32, Uuid, String)>(
        "SELECT quiz_revisions.revision_no,
                question_versions.id,
                question_versions.question_text
         FROM quiz_revisions
         JOIN question_versions
           ON question_versions.quiz_revision_id =
              quiz_revisions.id
         WHERE quiz_revisions.quiz_id = $1
         ORDER BY quiz_revisions.revision_no",
    )
    .bind(quiz_id)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(revision_rows.len(), 2);
    assert_eq!(revision_rows[0].2, "Which status represents Created?");
    assert_eq!(revision_rows[1].2, "Which HTTP status means Accepted?");
    assert_ne!(revision_rows[0].1, revision_rows[1].1);

    let current_draft_text = sqlx::query_scalar::<_, String>(
        "SELECT question_text
         FROM draft_questions
         WHERE quiz_id = $1 AND stable_question_id = $2",
    )
    .bind(quiz_id)
    .bind(question_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(current_draft_text, "Which HTTP status means Accepted?");

    let (status, _) = call(
        app,
        Method::PUT,
        &format!("/api/v1/teams/{team_id}/quizzes/{quiz_id}/questions/{question_id}"),
        Some(&member_token),
        Some(json!({
            "question_text": "Unauthorized revision",
            "options": ["Option A", "Option B"],
            "correct_option_index": 0,
            "change_reason": "A member must not revise this quiz"
        })),
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}
