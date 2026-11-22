use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use sqlx::postgres::PgPoolOptions;
use standuppulse::{
    AppState,
    api::{
        auth::AuthenticatedUser, dto::PublicQuestionResponse, routes::participation_routes,
    },
};
use tower::ServiceExt;
use uuid::Uuid;

#[test]
fn public_question_never_serializes_protected_answer_fields() {
    let question = PublicQuestionResponse {
        question_id: Uuid::new_v4(),
        position: 0,
        question_text: "Which keyword creates an immutable binding?".to_owned(),
        options: vec!["let".to_owned(), "mut".to_owned()],
    };
    let json = serde_json::to_value(question).unwrap();
    assert!(json.get("correct_option_index").is_none());
    assert!(json.get("explanation").is_none());
    assert!(json.get("is_correct").is_none());
}

#[tokio::test]
async fn participation_routes_require_authentication() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1/unused")
        .unwrap();
    let app = participation_routes().with_state(AppState { pool });
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/teams/{}/quizzes/available",
                    Uuid::new_v4()
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn review_results_and_analytics_require_authentication() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1/unused")
        .unwrap();
    let id = Uuid::new_v4();
    for uri in [
        format!("/api/v1/attempts/{id}/review"),
        format!("/api/v1/quizzes/{id}/my-results"),
        format!("/api/v1/quizzes/{id}/analytics"),
    ] {
        let app = participation_routes().with_state(AppState { pool: pool.clone() });
        let response = app
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}

#[tokio::test]
async fn oversized_submission_uses_standard_error_envelope() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1/unused")
        .unwrap();
    let app = participation_routes().with_state(AppState { pool });
    let mut request = Request::builder()
        .method("POST")
        .uri(format!(
            "/api/v1/attempt-sessions/{}/submit",
            Uuid::new_v4()
        ))
        .header("content-type", "application/json")
        .body(Body::from("x".repeat(1024 * 1024 + 1)))
        .unwrap();
    request.extensions_mut().insert(AuthenticatedUser {
        user_id: Uuid::new_v4(),
    });
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["error"]["code"], "PAYLOAD_TOO_LARGE");
}
