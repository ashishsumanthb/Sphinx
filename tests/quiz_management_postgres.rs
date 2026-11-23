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
use std::time::Duration;
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
async fn call_raw_json(
    app: Router,
    method: Method,
    uri: &str,
    token: &str,
    body: &str,
) -> (StatusCode, Value) {
    let response = app
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_owned()))
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();

    let response_body = serde_json::from_slice(&bytes).unwrap();

    (status, response_body)
}

fn assert_structured_error(response: &Value) {
    assert!(response["error"]["code"].is_string());
    assert!(response["error"]["message"].is_string());

    let request_id = response["error"]["request_id"].as_str().unwrap();
    assert!(request_id.starts_with("req_"));
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
    let (status, response) = call(
        app.clone(),
        Method::POST,
        "/api/v1/teams/not-a-uuid/quizzes",
        Some(&admin_token),
        Some(json!({
            "title": "Malformed path",
            "max_attempts": 1
        })),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_structured_error(&response);

    let (status, response) = call_raw_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/teams/{team_id}/quizzes"),
        &admin_token,
        r#"{"title":"Broken JSON","max_attempts":1"#,
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_structured_error(&response);

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

async fn wait_for_blocked_query(pool: &PgPool, blocker_pid: i32, query_fragment: &str) -> i32 {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let blocked_pid = sqlx::query_scalar::<_, i32>(
                "SELECT pid FROM pg_stat_activity
                 WHERE datname = current_database()
                   AND wait_event_type = 'Lock'
                   AND $1 = ANY(pg_blocking_pids(pid))
                   AND position($2 in query) > 0",
            )
            .bind(blocker_pid)
            .bind(query_fragment)
            .fetch_optional(pool)
            .await
            .unwrap();

            if let Some(pid) = blocked_pid {
                return pid;
            }

            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!("No query containing {query_fragment:?} blocked by backend {blocker_pid}")
    })
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL database; run with --test-threads=1"]
async fn quiz_management_cross_operation_lock_order_is_consistent() {
    let database_url = std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL is required");
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .unwrap();
    reset_schema(&pool).await;

    let app = app(AppState { pool: pool.clone() });
    let admin_id = register_user(
        app.clone(),
        "Concurrency Admin",
        "concurrency-admin@example.com",
        "ConcurrentQuizTest123!",
    )
    .await;
    let admin_token = login(
        app.clone(),
        "concurrency-admin@example.com",
        "ConcurrentQuizTest123!",
    )
    .await;
    let team_id = create_team(app.clone(), &admin_token).await;

    for (transition, edit) in [
        ("publish", false),
        ("publish", true),
        ("close", false),
        ("close", true),
    ] {
        let case = format!("{transition} versus {}", if edit { "edit" } else { "add" });
        let (status, created) = call(
            app.clone(),
            Method::POST,
            &format!("/api/v1/teams/{team_id}/quizzes"),
            Some(&admin_token),
            Some(json!({"title": case, "max_attempts": 1})),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{case}: {created}");
        let quiz_id = Uuid::parse_str(created["id"].as_str().unwrap()).unwrap();
        let question_uri = format!("/api/v1/teams/{team_id}/quizzes/{quiz_id}/questions");
        let (status, question) = call(
            app.clone(),
            Method::POST,
            &question_uri,
            Some(&admin_token),
            Some(json!({
                "question_text": "Original question",
                "options": ["First", "Second"],
                "correct_option_index": 0
            })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{case}: {question}");
        let question_id = question["stable_question_id"].as_str().unwrap();

        if transition == "close" {
            let (status, response) = call(
                app.clone(),
                Method::POST,
                &format!("/api/v1/quizzes/{quiz_id}/publish"),
                Some(&admin_token),
                Some(json!({})),
            )
            .await;
            assert_eq!(status, StatusCode::CREATED, "{case}: {response}");
        }

        // Stop add/edit after it owns the team row, at its membership check.
        // PostgreSQL's wait graph provides the barrier; scheduling delays do not.
        let mut membership_gate = pool.begin().await.unwrap();
        let gate_pid = sqlx::query_scalar::<_, i32>("SELECT pg_backend_pid()")
            .fetch_one(&mut *membership_gate)
            .await
            .unwrap();
        sqlx::query(
            "SELECT role FROM team_memberships
             WHERE team_id = $1 AND user_id = $2 FOR UPDATE",
        )
        .bind(team_id)
        .bind(admin_id)
        .fetch_one(&mut *membership_gate)
        .await
        .unwrap();

        let mutation_app = app.clone();
        let mutation_token = admin_token.clone();
        let mutation_uri = if edit {
            format!("{question_uri}/{question_id}")
        } else {
            question_uri
        };
        let changed_text = if edit {
            "Edited question"
        } else {
            "Added question"
        };
        let mutation = tokio::spawn(async move {
            call(
                mutation_app,
                if edit { Method::PUT } else { Method::POST },
                &mutation_uri,
                Some(&mutation_token),
                Some(json!({
                    "question_text": changed_text,
                    "options": ["First", "Second"],
                    "correct_option_index": 1,
                    "change_reason": "Concurrency regression coverage"
                })),
            )
            .await
        });
        let mutation_pid = wait_for_blocked_query(&pool, gate_pid, "FROM team_memberships").await;

        let transition_app = app.clone();
        let transition_token = admin_token.clone();
        let transition_request = tokio::spawn(async move {
            call(
                transition_app,
                Method::POST,
                &format!("/api/v1/quizzes/{quiz_id}/{transition}"),
                Some(&transition_token),
                (transition == "publish").then(|| json!({})),
            )
            .await
        });
        wait_for_blocked_query(&pool, mutation_pid, "FROM teams").await;

        // With quiz -> team ordering, publish/close now owns the quiz row while
        // add/edit owns the team row. Opening the gate deterministically creates
        // a deadlock. Team -> membership -> quiz lets both requests finish.
        membership_gate.commit().await.unwrap();
        let (mutation_result, transition_result) =
            tokio::time::timeout(Duration::from_secs(15), async {
                tokio::join!(mutation, transition_request)
            })
            .await
            .unwrap_or_else(|_| panic!("{case}: concurrent requests did not finish"));
        let (mutation_status, mutation_body) = mutation_result.unwrap();
        let (transition_status, transition_body) = transition_result.unwrap();
        let expected_mutation_status = if edit {
            StatusCode::OK
        } else if transition == "publish" {
            StatusCode::CREATED
        } else {
            StatusCode::CONFLICT
        };
        assert_eq!(
            mutation_status, expected_mutation_status,
            "{case}: mutation returned {mutation_body}"
        );
        assert_eq!(
            transition_status,
            if transition == "publish" {
                StatusCode::CREATED
            } else {
                StatusCode::OK
            },
            "{case}: transition returned {transition_body}"
        );

        let (stored_status, manually_closed) = sqlx::query_as::<_, (String, bool)>(
            "SELECT status, manually_closed_at IS NOT NULL FROM quizzes WHERE id = $1",
        )
        .bind(quiz_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            stored_status,
            if transition == "publish" {
                "open"
            } else {
                "closed"
            }
        );
        assert_eq!(manually_closed, transition == "close");

        let versions = sqlx::query_as::<_, (i32, String)>(
            "SELECT revisions.revision_no, versions.question_text
             FROM quiz_revisions AS revisions
             JOIN question_versions AS versions ON versions.quiz_revision_id = revisions.id
             WHERE revisions.quiz_id = $1
             ORDER BY revisions.revision_no, versions.position",
        )
        .bind(quiz_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        let expected_versions = match (transition, edit) {
            ("publish", false) => vec![(1, "Original question"), (1, "Added question")],
            ("publish", true) => vec![(1, "Edited question")],
            ("close", true) => vec![(1, "Original question"), (2, "Edited question")],
            _ => vec![(1, "Original question")],
        };
        assert_eq!(
            versions,
            expected_versions
                .into_iter()
                .map(|(revision, question)| (revision, question.to_owned()))
                .collect::<Vec<_>>(),
            "{case}: saved revision snapshots"
        );

        let drafts = sqlx::query_scalar::<_, String>(
            "SELECT question_text FROM draft_questions WHERE quiz_id = $1 ORDER BY position",
        )
        .bind(quiz_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        let expected_drafts = if edit {
            vec!["Edited question"]
        } else if transition == "publish" {
            vec!["Original question", "Added question"]
        } else {
            vec!["Original question"]
        };
        assert_eq!(drafts, expected_drafts, "{case}: saved draft questions");
    }

    pool.close().await;
}
