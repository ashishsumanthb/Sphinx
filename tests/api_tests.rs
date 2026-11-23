use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use sphinx::{AppState, api::routes::participation_routes, app};
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;
use uuid::Uuid;

fn test_state() -> AppState {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1/unused")
        .unwrap();
    AppState { pool }
}

fn test_app() -> axum::Router {
    participation_routes().with_state(test_state())
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
        assert_eq!(
            json,
            serde_json::json!({"error": "A valid login session is required"})
        );
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
    assert_eq!(
        json,
        serde_json::json!({"error": "A valid login session is required"})
    );
}

#[tokio::test]
async fn host_routes_preserve_authentication_errors_before_parsing_requests() {
    let id = Uuid::new_v4();
    let routes = [
        ("GET", "/api/v1/auth/me".to_owned()),
        ("POST", "/api/v1/auth/logout".to_owned()),
        ("GET", "/api/v1/teams".to_owned()),
        ("POST", "/api/v1/teams".to_owned()),
        ("GET", format!("/api/v1/teams/{id}/members")),
        ("POST", "/api/v1/teams/not-a-uuid/members".to_owned()),
        ("POST", "/api/v1/teams/not-a-uuid/quizzes".to_owned()),
        ("POST", format!("/api/v1/teams/{id}/quizzes/{id}/questions")),
        (
            "PUT",
            format!("/api/v1/teams/{id}/quizzes/{id}/questions/{id}"),
        ),
        ("POST", format!("/api/v1/quizzes/{id}/publish")),
        ("POST", format!("/api/v1/quizzes/{id}/close")),
    ];
    let app = app(test_state());

    for (method, uri) in routes {
        for authorization in [None, Some("Bearer invalid"), Some("Basic invalid")] {
            let mut request = Request::builder()
                .method(method)
                .uri(&uri)
                .header("content-type", "application/json");
            if let Some(authorization) = authorization {
                request = request.header("authorization", authorization);
            }
            let response = app
                .clone()
                .oneshot(request.body(Body::from("{")).unwrap())
                .await
                .unwrap();

            assert_eq!(
                response.status(),
                StatusCode::UNAUTHORIZED,
                "{method} {uri}, authorization: {authorization:?}"
            );
            let bytes = response.into_body().collect().await.unwrap().to_bytes();
            let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(
                json,
                serde_json::json!({"error": "A valid login session is required"}),
                "{method} {uri}, authorization: {authorization:?}"
            );
        }
    }
}

#[tokio::test]
async fn login_and_registration_preserve_string_error_contract() {
    let cases = [
        (
            "/api/v1/auth/login",
            "{",
            StatusCode::BAD_REQUEST,
            "Expected JSON with email and password fields",
        ),
        (
            "/api/v1/auth/login",
            "{}",
            StatusCode::UNPROCESSABLE_ENTITY,
            "Expected JSON with email and password fields",
        ),
        (
            "/api/v1/auth/login",
            r#"{"email":"invalid","password":"short"}"#,
            StatusCode::UNAUTHORIZED,
            "Invalid email or password",
        ),
        (
            "/api/v1/auth/register",
            "{",
            StatusCode::BAD_REQUEST,
            "Expected JSON with name, email and password fields",
        ),
        (
            "/api/v1/auth/register",
            "{}",
            StatusCode::UNPROCESSABLE_ENTITY,
            "Expected JSON with name, email and password fields",
        ),
        (
            "/api/v1/auth/register",
            r#"{"name":"","email":"demo@example.com","password":"DemoOnly123!"}"#,
            StatusCode::BAD_REQUEST,
            "Name must contain 1 to 100 characters",
        ),
    ];
    let app = app(test_state());

    for (uri, body, status, message) in cases {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(uri)
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), status, "{uri}, body: {body}");
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"error": message}),
            "{uri}, body: {body}"
        );
    }
}

#[tokio::test]
async fn local_frontend_origin_can_call_the_api_with_a_bearer_token() {
    let response = app(test_state())
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/api/v1/quizzes/00000000-0000-4000-8000-000000000001/my-results")
                .header("origin", "http://127.0.0.1:3000")
                .header("access-control-request-method", "GET")
                .header("access-control-request-headers", "authorization")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .unwrap(),
        "http://127.0.0.1:3000"
    );
    assert!(
        response
            .headers()
            .get("access-control-allow-headers")
            .unwrap()
            .to_str()
            .unwrap()
            .contains("authorization")
    );

    let rejected_origin = app(test_state())
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/api/v1/auth/login")
                .header("origin", "https://untrusted.example")
                .header("access-control-request-method", "POST")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        rejected_origin
            .headers()
            .get("access-control-allow-origin")
            .is_none()
    );
}
