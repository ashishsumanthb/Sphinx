//! Uses real migrations and the host login/session extractor in an isolated schema.
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use sphinx::{AppState, app};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tower::ServiceExt;
use uuid::Uuid;

fn id(n: u32) -> Uuid {
    Uuid::parse_str(&format!("00000000-0000-4000-8000-{n:012}")).unwrap()
}
async fn request(
    router: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let response = router
        .clone()
        .oneshot(builder.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (
        status,
        if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        },
    )
}
async fn login(router: &Router, email: &str) -> String {
    let (status, value) = request(
        router,
        "POST",
        "/api/v1/auth/login",
        None,
        json!({"email":email, "password":"InsightsDemo123!"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
    value["token"].as_str().unwrap().to_owned()
}
async fn results(router: &Router, token: &str, quiz: u32) -> (StatusCode, Value) {
    request(
        router,
        "GET",
        &format!("/api/v1/quizzes/{}/my-results", id(quiz)),
        Some(token),
        Value::Null,
    )
    .await
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL; scripts/verify_insights.sh runs this explicitly"]
async fn shared_schema_login_and_results_matrix() {
    let url = std::env::var("TEST_DATABASE_URL").expect("set an isolated test database URL");
    let admin_pool = PgPool::connect(&url).await.unwrap();
    let schema = format!("host_insights_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&admin_pool)
        .await
        .unwrap();
    let search_path = schema.clone();
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .after_connect(move |conn, _| {
            let search_path = search_path.clone();
            Box::pin(async move {
                sqlx::query("SELECT set_config('search_path', $1, false)")
                    .bind(search_path)
                    .execute(conn)
                    .await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .unwrap();
    // Exactly the migrator used by main.rs, scoped to this unique schema.
    sqlx::migrate::Migrator::new(std::path::Path::new("./migrations"))
        .await
        .unwrap()
        .run(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!("fixtures/insights.sql"))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE users SET password_encoded = $1")
        .bind(sphinx::password::encode("InsightsDemo123!"))
        .execute(&pool)
        .await
        .unwrap();
    let router = app(AppState { pool: pool.clone() });
    let a = login(&router, "a@example.test").await;
    let b = login(&router, "b@example.test").await;
    let c = login(&router, "c@example.test").await;
    let empty = login(&router, "z@example.test").await;
    let outsider = login(&router, "outsider@example.test").await;
    let new_member = login(&router, "new_member@example.test").await;

    let path = format!("/api/v1/quizzes/{}/my-results", id(100));
    assert_eq!(
        request(&router, "GET", &path, None, Value::Null).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (status, value) = results(&router, &a, 100).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert_eq!(value["attempts"].as_array().unwrap().len(), 2);
    assert_eq!(value["best_score"], 9);
    assert_eq!(value["best_percentage"], 90.0);
    assert_eq!(value["rank"], 1);
    assert_eq!(results(&router, &b, 100).await.1["rank"], 1);
    assert_eq!(results(&router, &c, 100).await.1["rank"], 3);
    let (status, denied) = results(&router, &outsider, 100).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(denied["error"]["code"], "FORBIDDEN");
    assert!(
        denied["error"]["request_id"]
            .as_str()
            .unwrap()
            .starts_with("req_")
    );
    assert_eq!(
        results(&router, &new_member, 100).await.0,
        StatusCode::FORBIDDEN
    );
    // An eligible member with no submissions has an empty, nullable result.
    let (status, value) = results(&router, &empty, 100).await;
    assert_eq!(status, StatusCode::OK);
    for key in ["best_score", "best_percentage", "revision_number", "rank"] {
        assert!(value[key].is_null());
    }
    assert_eq!(value["attempts"], json!([]));
    assert_eq!(value["revision_results"], json!([]));
    // Admin policy allows own empty results without quiz eligibility.
    sqlx::query("UPDATE team_memberships SET role='admin' WHERE user_id=$1")
        .bind(id(7))
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(results(&router, &new_member, 100).await.0, StatusCode::OK);
    sqlx::query("UPDATE team_memberships SET role='member' WHERE user_id=$1")
        .bind(id(7))
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        results(&router, &new_member, 100).await.0,
        StatusCode::FORBIDDEN
    );
    // Query input never overrides the authenticated caller's identity.
    let (_, value) = request(
        &router,
        "GET",
        &format!("{path}?user_id={}", id(1)),
        Some(&c),
        Value::Null,
    )
    .await;
    assert_eq!(value["attempts"].as_array().unwrap().len(), 1);
    assert_eq!(value["best_score"], 8);

    // Older revision has the better score. Latest is revision_no, not timestamp.
    sqlx::query("UPDATE quiz_attempts SET quiz_revision_id=$1, score=6, submitted_at='2020-01-01' WHERE id=$2")
        .bind(id(201)).bind(id(401)).execute(&pool).await.unwrap();
    sqlx::query("UPDATE attempt_sessions SET quiz_revision_id=$1 WHERE id=$2")
        .bind(id(201))
        .bind(id(301))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO quiz_revisions(id,quiz_id,revision_no,created_by,change_reason) VALUES($1,$2,3,$3,'Unattempted')")
        .bind(id(204)).bind(id(100)).bind(id(1)).execute(&pool).await.unwrap();
    let (status, value) = results(&router, &a, 100).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert_eq!(value["revision_number"], 2);
    assert_eq!(value["best_score"], 6);
    assert_eq!(value["best_percentage"], 60.0);
    assert_eq!(value["rank"], 1);
    let revisions = value["revision_results"].as_array().unwrap();
    assert_eq!(revisions.len(), 2);
    assert_eq!(revisions[0]["best_percentage"], 70.0);
    assert_eq!(revisions[1]["revision_number"], value["revision_number"]);
    assert_eq!(revisions[1]["best_percentage"], value["best_percentage"]);
    assert_eq!(revisions[1]["rank"], value["rank"]);
    // Missing quiz is indistinguishable from denied access.
    assert_eq!(results(&router, &a, 999).await.0, StatusCode::FORBIDDEN);
    // Fractions and a legitimate zero score remain numeric.
    let (_, value) = results(&router, &a, 101).await;
    assert!((value["best_percentage"].as_f64().unwrap() - 200.0 / 3.0).abs() < 1e-10);
    assert_eq!(results(&router, &c, 101).await.1["best_percentage"], 0.0);
    sqlx::query("DELETE FROM team_memberships WHERE team_id=$1 AND user_id=$2")
        .bind(id(10))
        .bind(id(3))
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(results(&router, &c, 100).await.0, StatusCode::FORBIDDEN);
    // Logout invalidates the same session used by the host results extractor.
    assert_eq!(
        request(
            &router,
            "POST",
            "/api/v1/auth/logout",
            Some(&a),
            Value::Null
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(results(&router, &a, 100).await.0, StatusCode::UNAUTHORIZED);
    pool.close().await;
    // Drop only the schema created by this test, never public or shared tables.
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin_pool)
        .await
        .unwrap();
    admin_pool.close().await;
}
