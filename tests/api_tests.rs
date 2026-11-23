use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use sqlx::postgres::PgPoolOptions;
use sphinx::{
    AppState,
    api::routes::participation_routes,
};
use tower::ServiceExt;
use uuid::Uuid;

fn test_app() -> axum::Router {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1/unused")
        .unwrap();
    participation_routes().with_state(AppState { pool })
}

#[tokio::test]
async fn every_member_three_route_requires_authentication() {
    let id = Uuid::new_v4();
    let routes = [
        ("GET", format!("/api/v1/teams/{id}/quizzes/available")),
        ("POST", format!("/api/v1/quizzes/{id}/attempt-sessions")),
        ("GET", format!("/api/v1/attempt-sessions/{id}/questions")),
        ("POST", format!("/api/v1/attempt-sessions/{id}/submit")),
        ("GET", format!("/api/v1/quizzes/{id}/my-attempts")),
        ("GET", format!("/api/v1/attempts/{id}/review")),
        ("GET", format!("/api/v1/quizzes/{id}/my-results")),
        ("GET", format!("/api/v1/quizzes/{id}/analytics")),
    ];

    for (method, uri) in routes {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["error"], "A valid login session is required");
    }
}

#[tokio::test]
async fn oversized_unauthenticated_submission_is_rejected_before_body_parsing() {
    let request = Request::builder()
        .method("POST")
        .uri(format!(
            "/api/v1/attempt-sessions/{}/submit",
            Uuid::new_v4()
        ))
        .header("content-type", "application/json")
        .body(Body::from("x".repeat(1024 * 1024 + 1)))
        .unwrap();
    let response = test_app().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["error"], "A valid login session is required");
}
