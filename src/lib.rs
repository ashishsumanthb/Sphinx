pub mod api;
pub mod domain;
pub mod repository;
pub mod services;

use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
}
