use sqlx::{PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

pub fn schema_sql() -> String {
    [
        include_str!("../../../../migrations/001_identity_and_teams.sql"),
        include_str!("../../../../migrations/002_rename_password_column.sql"),
        include_str!("../../../../migrations/0020_quiz_schema.sql"),
        include_str!("../../../../migrations/0030_participation.sql"),
    ]
    .join("\n")
}

pub fn id(n: u32) -> Uuid {
    Uuid::parse_str(&format!("00000000-0000-4000-8000-{n:012}")).unwrap()
}

/// Creates a uniquely named schema in the caller-selected local test database.
/// Shared application tables are never migrated or seeded by this helper.
pub async fn fixture_pool() -> (PgPool, String) {
    let url = std::env::var("INSIGHTS_TEST_DATABASE_URL")
        .expect("Set INSIGHTS_TEST_DATABASE_URL to a local PostgreSQL test database");
    let schema = format!("insights_test_{}", Uuid::new_v4().simple());
    let admin = PgPool::connect(&url)
        .await
        .expect("connect to test PostgreSQL");
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&admin)
        .await
        .expect("create isolated test schema");
    admin.close().await;
    let search_path = schema.clone();
    let pool = PgPoolOptions::new()
        .max_connections(3)
        .after_connect(move |conn, _| {
            let search_path = search_path.clone();
            Box::pin(async move {
                sqlx::query("SELECT set_config('search_path', $1, false)")
                    .bind(search_path)
                    .execute(conn)
                    .await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .unwrap();
    sqlx::raw_sql(&schema_sql())
        .execute(&pool)
        .await
        .expect("apply design schema");
    sqlx::raw_sql(include_str!("fixtures.sql"))
        .execute(&pool)
        .await
        .expect("seed valid UUID fixtures");
    (pool, schema)
}
