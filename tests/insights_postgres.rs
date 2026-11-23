use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sphinx::{AppState, api::dto::MyResultsResponse, app};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tower::ServiceExt;
use uuid::Uuid;

async fn get_results(app: &Router, quiz: Uuid, token: Option<&str>) -> (StatusCode, Value) {
    let mut request = Request::builder().uri(format!("/api/v1/quizzes/{quiz}/my-results"));
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}

async fn create_user(pool: &PgPool) -> (Uuid, String) {
    let user = Uuid::new_v4();
    let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    sqlx::query(
        "INSERT INTO users(id,name,email,password_encoded) VALUES($1,'Test user',$2,'unused')",
    )
    .bind(user)
    .bind(format!("{user}@example.test"))
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO login_sessions(token_hash,user_id,expires_at) VALUES($1,$2,now()+interval '1 hour')")
        .bind(Sha256::digest(token.as_bytes()).to_vec()).bind(user).execute(pool).await.unwrap();
    (user, token)
}

async fn create_quiz(pool: &PgPool, team: Uuid, admin: Uuid) -> Uuid {
    let quiz = Uuid::new_v4();
    sqlx::query("INSERT INTO quizzes(id,team_id,created_by,title,status,starts_at,ends_at,max_attempts) VALUES($1,$2,$3,'Insights','closed',now()-interval '2 hours',now()-interval '1 hour',2)")
        .bind(quiz).bind(team).bind(admin).execute(pool).await.unwrap();
    quiz
}

async fn create_revision(pool: &PgPool, quiz: Uuid, admin: Uuid, number: i32) -> Uuid {
    let revision = Uuid::new_v4();
    // Deliberately reverse creation time: revision_no is the only latest-revision key.
    sqlx::query("INSERT INTO quiz_revisions(id,quiz_id,created_by,revision_no,change_reason,created_at) VALUES($1,$2,$3,$4,'Fixture',now()-make_interval(hours => $4))")
        .bind(revision).bind(quiz).bind(admin).bind(number).execute(pool).await.unwrap();
    revision
}

async fn save_attempt(
    pool: &PgPool,
    quiz: Uuid,
    revision: Uuid,
    user: Uuid,
    number: i16,
    score: i32,
) -> Uuid {
    let session = Uuid::new_v4();
    sqlx::query("INSERT INTO attempt_sessions(id,quiz_id,quiz_revision_id,user_id,status) VALUES($1,$2,$3,$4,'submitted')")
        .bind(session).bind(quiz).bind(revision).bind(user).execute(pool).await.unwrap();
    // The second attempt has an earlier timestamp than the first.
    sqlx::query_scalar("INSERT INTO quiz_attempts(session_id,quiz_id,quiz_revision_id,user_id,attempt_no,request_hash,score,total_questions,submitted_at) VALUES($1,$2,$3,$4,$5,$6,$7,10,now()-make_interval(hours => $5::int)) RETURNING id")
        .bind(session).bind(quiz).bind(revision).bind(user).bind(number).bind("a".repeat(64)).bind(score)
        .fetch_one(pool).await.unwrap()
}

fn assert_empty(body: &Value) {
    let typed: MyResultsResponse = serde_json::from_value(body.clone()).unwrap();
    assert!(typed.attempts.is_empty());
    assert!(typed.revision_results.is_empty());
    for field in ["best_score", "best_percentage", "revision_number", "rank"] {
        assert!(body.get(field).unwrap().is_null(), "{field}: {body}");
    }
}

fn assert_summary_matches_latest(body: &Value) {
    let results = body["revision_results"].as_array().unwrap();
    for field in ["best_score", "best_percentage", "revision_number", "rank"] {
        assert_eq!(
            body[field],
            results.last().unwrap()[field],
            "{field}: {body}"
        );
    }
    let typed: MyResultsResponse = serde_json::from_value(body.clone()).unwrap();
    for attempt in &typed.attempts {
        assert!(
            typed
                .revision_results
                .iter()
                .any(
                    |revision| revision.quiz_revision_id == attempt.quiz_revision_id
                        && revision.revision_number == attempt.revision_number
                )
        );
    }
    for private in [
        "correct_option_index",
        "explanation",
        "is_correct",
        "user_id",
        "participants",
    ] {
        assert!(!body.to_string().contains(private), "leaked {private}");
    }
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL database; resets public schema"]
async fn host_results_authorization_revisions_ties_and_summary_are_consistent() {
    let url = std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL is required");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .unwrap();
    sqlx::raw_sql("DROP SCHEMA public CASCADE; CREATE SCHEMA public;")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::migrate::Migrator::new(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations"
    )))
    .await
    .unwrap()
    .run(&pool)
    .await
    .unwrap();
    let app = app(AppState { pool: pool.clone() });
    let admin = create_user(&pool).await;
    let member = create_user(&pool).await;
    let peer = create_user(&pool).await;
    let third = create_user(&pool).await;
    let empty = create_user(&pool).await;
    let ineligible = create_user(&pool).await;
    let outsider = create_user(&pool).await;
    let team = Uuid::new_v4();
    sqlx::query("INSERT INTO teams(id,name,created_by) VALUES($1,'Insights team',$2)")
        .bind(team)
        .bind(admin.0)
        .execute(&pool)
        .await
        .unwrap();
    for (user, role) in [
        (admin.0, "admin"),
        (member.0, "member"),
        (peer.0, "member"),
        (third.0, "member"),
        (empty.0, "member"),
        (ineligible.0, "member"),
    ] {
        sqlx::query("INSERT INTO team_memberships(team_id,user_id,role) VALUES($1,$2,$3)")
            .bind(team)
            .bind(user)
            .bind(role)
            .execute(&pool)
            .await
            .unwrap();
    }
    let other_team = Uuid::new_v4();
    sqlx::query("INSERT INTO teams(id,name,created_by) VALUES($1,'Other team',$2)")
        .bind(other_team)
        .bind(outsider.0)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO team_memberships(team_id,user_id,role) VALUES($1,$2,'admin')")
        .bind(other_team)
        .bind(outsider.0)
        .execute(&pool)
        .await
        .unwrap();
    let quiz = create_quiz(&pool, team, admin.0).await;
    for user in [member.0, peer.0, third.0, empty.0] {
        sqlx::query("INSERT INTO quiz_eligibilities(quiz_id,user_id) VALUES($1,$2)")
            .bind(quiz)
            .bind(user)
            .execute(&pool)
            .await
            .unwrap();
    }

    for token in [
        None,
        Some("invalid"),
        Some("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"),
    ] {
        let (status, body) = get_results(&app, quiz, token).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
        assert_eq!(body["error"], "A valid login session is required");
    }
    for (denied_quiz, token) in [
        (quiz, &outsider.1),
        (quiz, &ineligible.1),
        (Uuid::new_v4(), &member.1),
    ] {
        let (status, body) = get_results(&app, denied_quiz, Some(token)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
        assert_eq!(body["error"]["code"], "FORBIDDEN");
        assert!(
            body["error"]["request_id"]
                .as_str()
                .unwrap()
                .starts_with("req_")
        );
    }
    // Admin does not need eligibility; both admin and eligible member see empty own results.
    for token in [&admin.1, &empty.1] {
        let (status, body) = get_results(&app, quiz, Some(token)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_empty(&body);
    }

    let first = create_revision(&pool, quiz, admin.0, 1).await;
    let second = create_revision(&pool, quiz, admin.0, 2).await;
    let _unattempted = create_revision(&pool, quiz, admin.0, 3).await;
    let member_first = save_attempt(&pool, quiz, first, member.0, 1, 10).await;
    let member_second = save_attempt(&pool, quiz, second, member.0, 2, 4).await;
    save_attempt(&pool, quiz, first, peer.0, 1, 10).await;
    save_attempt(&pool, quiz, second, peer.0, 2, 8).await;
    save_attempt(&pool, quiz, first, third.0, 1, 5).await;
    save_attempt(&pool, quiz, second, third.0, 2, 8).await;
    for (token, first_rank, second_rank) in [(&member.1, 1, 3), (&peer.1, 1, 1), (&third.1, 3, 1)] {
        let (status, body) = get_results(&app, quiz, Some(token)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_summary_matches_latest(&body);
        assert_eq!(body["revision_results"].as_array().unwrap().len(), 2);
        assert_eq!(body["revision_results"][0]["revision_number"], 1);
        assert_eq!(body["revision_results"][0]["rank"], first_rank);
        assert_eq!(body["revision_results"][1]["revision_number"], 2);
        assert_eq!(body["revision_results"][1]["rank"], second_rank);
    }
    let (_, body) = get_results(&app, quiz, Some(&member.1)).await;
    assert_eq!(body["best_score"], 4);
    assert_eq!(body["best_percentage"], 40.0);
    assert_eq!(body["revision_number"], 2);
    assert_eq!(body["rank"], 3);
    assert_eq!(body["revision_results"][0]["best_score"], 10);
    assert_eq!(body["attempts"].as_array().unwrap().len(), 2);
    assert_eq!(body["attempts"][0]["attempt_id"], member_second.to_string());
    assert_eq!(body["attempts"][1]["attempt_id"], member_first.to_string());
    for token in [&admin.1, &empty.1] {
        let (status, body) = get_results(&app, quiz, Some(token)).await;
        assert_eq!(status, StatusCode::OK);
        assert_empty(&body);
    }

    // Best per revision counts each competitor once, even with multiple attempts.
    let retry_quiz = create_quiz(&pool, team, admin.0).await;
    let retry_revision = create_revision(&pool, retry_quiz, admin.0, 1).await;
    for user in [member.0, peer.0] {
        sqlx::query("INSERT INTO quiz_eligibilities(quiz_id,user_id) VALUES($1,$2)")
            .bind(retry_quiz)
            .bind(user)
            .execute(&pool)
            .await
            .unwrap();
    }
    save_attempt(&pool, retry_quiz, retry_revision, member.0, 1, 8).await;
    save_attempt(&pool, retry_quiz, retry_revision, member.0, 2, 9).await;
    save_attempt(&pool, retry_quiz, retry_revision, peer.0, 1, 7).await;
    let (_, body) = get_results(&app, retry_quiz, Some(&member.1)).await;
    assert_summary_matches_latest(&body);
    assert_eq!(body["best_score"], 9);
    assert_eq!(body["best_percentage"], 90.0);
    let (_, body) = get_results(&app, retry_quiz, Some(&peer.1)).await;
    assert_eq!(body["rank"], 2);

    // Retained eligibility/attempts cannot grant access after team membership is removed.
    sqlx::query("DELETE FROM team_memberships WHERE team_id=$1 AND user_id=$2")
        .bind(team)
        .bind(member.0)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        get_results(&app, quiz, Some(&member.1)).await.0,
        StatusCode::FORBIDDEN
    );
    sqlx::query("UPDATE login_sessions SET created_at=now()-interval '2 hours', expires_at=now()-interval '1 hour' WHERE user_id=$1")
        .bind(peer.0).execute(&pool).await.unwrap();
    assert_eq!(
        get_results(&app, quiz, Some(&peer.1)).await.0,
        StatusCode::UNAUTHORIZED
    );
    pool.close().await;
}
