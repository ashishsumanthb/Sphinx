use std::{env, error::Error, time::Duration};

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use uuid::Uuid;

const DEMO_PASSWORD: &str = "SphinxDemo123!";
const TEAM_ID: &str = "51f4d5f0-84cf-4d97-90f3-0fd47fb9d001";

const DEMO_USERS: [(&str, &str, &str); 11] = [
    ("Srikanth", "srikanth@sphinx.local", "admin"),
    ("Arshad", "arshad@sphinx.local", "member"),
    ("Ashish", "ashish@sphinx.local", "member"),
    ("Sushma", "sushma@sphinx.local", "member"),
    ("Gangadhar", "gangadhar@sphinx.local", "member"),
    ("Abhishek", "abhishek@sphinx.local", "member"),
    ("Aditya", "aditya@sphinx.local", "member"),
    ("Sanchey", "sanchey@sphinx.local", "member"),
    ("Sowjanya", "sowjanya@sphinx.local", "member"),
    ("Sheryas", "sheryas@sphinx.local", "member"),
    ("Sathvik", "sathvik@sphinx.local", "member"),
];

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    if let Err(error) = dotenvy::dotenv()
        && !error.not_found()
    {
        return Err(error.into());
    }

    let options = PgConnectOptions::new()
        .host(&env::var("POSTGRES_HOST")?)
        .port(env::var("POSTGRES_PORT")?.parse()?)
        .username(&env::var("POSTGRES_USER")?)
        .password(&env::var("POSTGRES_PASSWORD")?)
        .database(&env::var("POSTGRES_DB")?);
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(Duration::from_secs(5))
        .connect_with(options)
        .await?;

    sqlx::migrate::Migrator::new(std::path::Path::new("./migrations"))
        .await?
        .run(&pool)
        .await?;

    let mut tx = pool.begin().await?;
    let encoded = sphinx::password::encode(DEMO_PASSWORD);
    let mut users = Vec::with_capacity(DEMO_USERS.len());

    for (name, email, role) in DEMO_USERS {
        let user_id = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO users (name, email, password_encoded)
             VALUES ($1, $2, $3)
             ON CONFLICT (email) DO UPDATE
             SET name = EXCLUDED.name,
                 password_encoded = EXCLUDED.password_encoded
             RETURNING id",
        )
        .bind(name)
        .bind(email)
        .bind(&encoded)
        .fetch_one(&mut *tx)
        .await?;
        users.push((user_id, name, email, role));
    }

    let team_id = Uuid::parse_str(TEAM_ID)?;
    let admin_id = users[0].0;
    sqlx::query(
        "INSERT INTO teams (id, name, created_by)
         VALUES ($1, 'Sphinx Presentation Team', $2)
         ON CONFLICT (id) DO UPDATE
         SET name = EXCLUDED.name,
             created_by = EXCLUDED.created_by",
    )
    .bind(team_id)
    .bind(admin_id)
    .execute(&mut *tx)
    .await?;

    for (user_id, _, _, role) in &users {
        sqlx::query(
            "INSERT INTO team_memberships (team_id, user_id, role)
             VALUES ($1, $2, $3)
             ON CONFLICT (team_id, user_id) DO UPDATE SET role = EXCLUDED.role",
        )
        .bind(team_id)
        .bind(user_id)
        .bind(role)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    println!("Sphinx demo accounts are ready.");
    println!("Team: Sphinx Presentation Team ({team_id})");
    println!("Password for every demo account: {DEMO_PASSWORD}");
    for (_, name, email, role) in users {
        println!("{role:>6}  {name:<10}  {email}");
    }

    Ok(())
}
