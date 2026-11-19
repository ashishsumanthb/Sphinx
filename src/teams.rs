use axum::{
    Json, Router,
    extract::{State, rejection::JsonRejection},
    http::StatusCode,
    routing::post,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    AppState,
    auth::{ApiError, AuthUser, api_error},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateTeamRequest {
    name: String,
}

#[derive(Serialize)]
struct TeamResponse {
    id: Uuid,
    name: String,
    role: String,
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/", post(create_team).get(list_teams))
}

async fn create_team(
    State(state): State<AppState>,
    user: AuthUser,
    payload: Result<Json<CreateTeamRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<TeamResponse>), ApiError> {
    let Json(request) =
        payload.map_err(|error| api_error(error.status(), "Expected JSON with a name field"))?;

    let name = request.name.trim();

    if !(1..=100).contains(&name.chars().count()) {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "Team name must contain 1 to 100 characters",
        ));
    }

    if name.chars().any(char::is_control) {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "Team name must not contain control characters",
        ));
    }

    let mut transaction = state.pool.begin().await.map_err(team_error)?;

    let (id, name) = sqlx::query_as::<_, (Uuid, String)>(
        "INSERT INTO teams (name, created_by)
         VALUES ($1, $2)
         RETURNING id, name",
    )
    .bind(name)
    .bind(user.id)
    .fetch_one(&mut *transaction)
    .await
    .map_err(team_error)?;

    sqlx::query(
        "INSERT INTO team_memberships (team_id, user_id, role)
         VALUES ($1, $2, 'admin')",
    )
    .bind(id)
    .bind(user.id)
    .execute(&mut *transaction)
    .await
    .map_err(team_error)?;

    transaction.commit().await.map_err(team_error)?;

    Ok((
        StatusCode::CREATED,
        Json(TeamResponse {
            id,
            name,
            role: "admin".to_owned(),
        }),
    ))
}
async fn list_teams(
    State(state): State<AppState>,
    user: AuthUser,
) -> Result<Json<Vec<TeamResponse>>, ApiError> {
    let rows = sqlx::query_as::<_, (Uuid, String, String)>(
        "SELECT teams.id, teams.name, team_memberships.role
         FROM teams
         JOIN team_memberships
           ON team_memberships.team_id = teams.id
         WHERE team_memberships.user_id = $1
         ORDER BY teams.created_at, teams.id",
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await
    .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "Unable to list teams"))?;

    let teams = rows
        .into_iter()
        .map(|(id, name, role)| TeamResponse { id, name, role })
        .collect();

    Ok(Json(teams))
}

fn team_error(_error: sqlx::Error) -> ApiError {
    api_error(StatusCode::INTERNAL_SERVER_ERROR, "Unable to create team")
}
