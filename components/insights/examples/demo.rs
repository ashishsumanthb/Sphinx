#[path = "../tests/support/mod.rs"]
mod support;

use axum::{
    Router,
    extract::Request,
    middleware::{self, Next},
    response::Response,
};
use sphinx_insights::{api::AuthenticatedUser, app_state::AppState, build_router};

// Fictional bearer tokens for this loopback example only.
// The reusable component accepts identities from the host's verified auth middleware.
async fn demo_identity(mut request: Request, next: Next) -> Response {
    let user_id = match request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
    {
        Some("Bearer demo-a") => Some(1),
        Some("Bearer demo-b") => Some(2),
        Some("Bearer demo-c") => Some(3),
        Some("Bearer demo-d") => Some(4),
        Some("Bearer demo-z") => Some(5),
        Some("Bearer demo-outsider") => Some(6),
        Some("Bearer demo-new-member") => Some(7),
        _ => None,
    };
    if let Some(n) = user_id {
        request.extensions_mut().insert(AuthenticatedUser {
            user_id: support::id(n),
        });
    }
    next.run(request).await
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter("sphinx_insights=info")
        .init();
    let (pool, schema) = support::fixture_pool().await;
    println!("Created demo schema {schema}. See docs/WALKTHROUGH.md for requests.");
    let app: Router = build_router(AppState { pool }).layer(middleware::from_fn(demo_identity));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8000").await?;
    println!("Demo listening at http://127.0.0.1:8000");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
