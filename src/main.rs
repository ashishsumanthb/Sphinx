use std::{env, error::Error, time::Duration};

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    if let Err(error) = dotenvy::dotenv() {
        if !error.not_found() {
            return Err(error.into());
        }
    }

    let database_options = PgConnectOptions::new()
        .host(&env::var("POSTGRES_HOST")?)
        .port(env::var("POSTGRES_PORT")?.parse()?)
        .username(&env::var("POSTGRES_USER")?)
        .password(&env::var("POSTGRES_PASSWORD")?)
        .database(&env::var("POSTGRES_DB")?);

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .connect_with(database_options)
        .await?;

    println!("Connected to PostgreSQL");

    let state = sphinx::AppState { pool };
    let listener = TcpListener::bind("127.0.0.1:8000").await?;

    println!("Sphinx is listening on http://127.0.0.1:8000");

    axum::serve(listener, sphinx::app(state)).await?;

    Ok(())
}
