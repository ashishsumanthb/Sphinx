use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use sphinx::{
    AppState,
    api::{dto::PublicQuestionResponse, routes::participation_routes},
};
use sqlx::postgres::PgPoolOptions;
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
