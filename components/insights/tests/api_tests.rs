use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use sphinx_insights::{api::AuthenticatedUser, app_state::AppState, build_router};
use tower::ServiceExt;

#[tokio::test]
async fn missing_identity_is_denied_before_database_access() {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://unused@127.0.0.1/unused")
        .unwrap();
    let response = build_router(AppState { pool })
        .oneshot(
            Request::builder()
                .uri("/api/v1/quizzes/00000000-0000-4000-8000-000000000100/my-results")
                .header("x-user-id", "00000000-0000-4000-8000-000000000001")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["error"]["code"], "UNAUTHENTICATED");
}

#[tokio::test]
async fn invalid_uuid_has_a_structured_error() {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://unused@127.0.0.1/unused")
        .unwrap();
    let mut request = Request::builder()
        .uri("/api/v1/quizzes/invalid/my-results")
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(AuthenticatedUser {
        user_id: uuid::Uuid::nil(),
    });
    let response = build_router(AppState { pool })
        .oneshot(request)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(response.headers().contains_key("x-request-id"));
}
