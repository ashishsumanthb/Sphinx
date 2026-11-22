mod domain;
mod services;
use axum::{
    http::StatusCode,
    routing::{get,post},
    Json,Router,
};
use domain::question_validation::{validate_question,QuestionDraft};
async fn health()->&'static str{
    "Hello Ashish"
}
async fn check_question(Json(draft): Json<QuestionDraft>) -> (StatusCode, &'static str) {
    match validate_question(&draft) {
        Ok(()) => (StatusCode::OK, "Question is valid"),
        Err(message) => (StatusCode::UNPROCESSABLE_ENTITY, message),
    }
}
#[tokio::main]
async fn main() -> std::io::Result<()> {
    let app = Router::new()
        .route("/health/live", get(health))
        .route("/questions/validate", post(check_question));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    println!("Sphinx is listening on http://127.0.0.1:3000");
    axum::serve(listener, app).await
}

