mod support;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::Value;
use sphinx_insights::{api::AuthenticatedUser, app_state::AppState, build_router};
use tower::ServiceExt;

async fn call(app: &Router, quiz: &str, user: Option<u32>) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .uri(format!("/api/v1/quizzes/{quiz}/my-results"))
        .body(Body::empty())
        .unwrap();
    if let Some(n) = user {
        request.extensions_mut().insert(AuthenticatedUser {
            user_id: support::id(n),
        });
    }
    let response = app.clone().oneshot(request).await.unwrap();
    assert!(response.headers().contains_key("x-request-id"));
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn postgres_and_http_acceptance() {
    let (pool, _) = support::fixture_pool().await;
    let app = build_router(AppState { pool: pool.clone() });
    let quiz = support::id(100).to_string();
    let (status, a) = call(&app, &quiz, Some(1)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(a["attempts"].as_array().unwrap().len(), 2);
    assert_eq!(a["attempts"][0]["attempt_number"], 1);
    assert_eq!(a["attempts"][0]["percentage"], 70.0);
    assert_eq!(a["attempts"][1]["attempt_number"], 2);
    assert_eq!(a["revision_results"][0]["best_percentage"], 90.0);
    assert_eq!(a["revision_results"][0]["rank"], 1);
    assert!(a["attempts"][0].get("submitted_at").is_some());
    assert!(a["attempts"][0].get("user_id").is_none());
    let (_, b) = call(&app, &quiz, Some(2)).await;
    assert_eq!(b["revision_results"][0]["rank"], 1);
    let (_, c) = call(&app, &quiz, Some(3)).await;
    assert_eq!(c["revision_results"][0]["rank"], 3);
    assert_eq!(c["attempts"].as_array().unwrap().len(), 1);
    let (_, d) = call(&app, &quiz, Some(4)).await;
    assert_eq!(d["revision_results"][0]["quiz_revision_number"], 2);
    assert_eq!(d["revision_results"][0]["rank"], 1);
    let (_, z) = call(&app, &quiz, Some(5)).await;
    assert_eq!(z["attempts"], serde_json::json!([]));
    assert_eq!(z["revision_results"], serde_json::json!([]));
    assert_eq!(call(&app, &quiz, None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(call(&app, &quiz, Some(6)).await.0, StatusCode::NOT_FOUND);
    assert_eq!(call(&app, &quiz, Some(7)).await.0, StatusCode::NOT_FOUND);
    assert_eq!(
        call(&app, &support::id(999).to_string(), Some(1)).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&app, "invalid", Some(1)).await.0,
        StatusCode::BAD_REQUEST
    );
    // Decimal scores and quiz isolation. 2/3 must outrank 33/50.
    let (_, fractions) = call(&app, &support::id(101).to_string(), Some(1)).await;
    let percentage = fractions["revision_results"][0]["best_percentage"]
        .as_f64()
        .unwrap();
    assert!((percentage - 66.6666666667).abs() < 1e-8);
    assert_eq!(fractions["revision_results"][0]["rank"], 1);
    let (_, zero) = call(&app, &support::id(101).to_string(), Some(3)).await;
    assert_eq!(zero["revision_results"][0]["best_percentage"], 0.0);
    assert_eq!(zero["revision_results"][0]["rank"], 3);
    let (_, empty_quiz) = call(&app, &support::id(102).to_string(), Some(1)).await;
    assert_eq!(empty_quiz["attempts"], serde_json::json!([]));
    // Public query inputs cannot choose another user.
    let mut request = Request::builder()
        .uri(format!(
            "/api/v1/quizzes/{quiz}/my-results?user_id={}",
            support::id(1)
        ))
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(AuthenticatedUser {
        user_id: support::id(3),
    });
    let response = app.clone().oneshot(request).await.unwrap();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let result: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(result["attempts"].as_array().unwrap().len(), 1);
    assert_eq!(result["revision_results"][0]["rank"], 3);
    // Removed members lose read access even when eligibility remains.
    sqlx::query("DELETE FROM team_memberships WHERE user_id = $1")
        .bind(support::id(3))
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(call(&app, &quiz, Some(3)).await.0, StatusCode::NOT_FOUND);
    sqlx::query("DELETE FROM team_memberships WHERE team_id = $1 AND user_id = $2")
        .bind(support::id(10))
        .bind(support::id(1))
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(call(&app, &quiz, Some(1)).await.0, StatusCode::NOT_FOUND);
    pool.close().await;
    let (status, error) = call(&app, &quiz, Some(1)).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(error["error"]["code"], "DEPENDENCY_UNAVAILABLE");
    println!(
        "PASS: ties, best-of-two, revision and quiz isolation, decimals, zero, empty history, identity, permissions, validation, dependency failure"
    );
}
