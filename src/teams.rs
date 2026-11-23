use axum::{
    Json, Router,
    extract::{
        Path, State,
        rejection::{JsonRejection, PathRejection},
    },
    http::StatusCode,
    routing::post,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    AppState,
    auth::{ApiError, AuthUser, api_error},
    permissions::{require_admin, require_member},
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AddMemberRequest {
    user_id: Uuid,
}

#[derive(Serialize)]
struct MemberResponse {
    team_id: Uuid,
    user_id: Uuid,
    role: String,
}

#[derive(Serialize)]
struct TeamMemberResponse {
    user_id: Uuid,
    name: String,
    role: String,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", post(create_team).get(list_teams))
        .route("/{team_id}/members", post(add_member).get(list_members))
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

async fn add_member(
    State(state): State<AppState>,
    user: AuthUser,
    path: Result<Path<Uuid>, PathRejection>,
    payload: Result<Json<AddMemberRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<MemberResponse>), ApiError> {
    let Path(team_id) =
        path.map_err(|_| api_error(StatusCode::BAD_REQUEST, "Team ID must be a valid UUID"))?;

    let Json(request) = payload
        .map_err(|error| api_error(error.status(), "Expected JSON with a valid user_id UUID"))?;

    let mut transaction = state.pool.begin().await.map_err(member_error)?;

    // Serialize membership changes before checking the caller's permissions.
    let team = sqlx::query_as::<_, (Uuid,)>(
        "SELECT id
         FROM teams
         WHERE id = $1
         FOR UPDATE",
    )
    .bind(team_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(member_error)?;

    if team.is_none() {
        return Err(api_error(StatusCode::NOT_FOUND, "Team not found"));
    }

    require_admin(
        &mut transaction,
        team_id,
        user.id,
        "Only team admins can add members",
    )
    .await?;

    let target_user = sqlx::query_as::<_, (Uuid,)>(
        "SELECT id
         FROM users
         WHERE id = $1
         FOR KEY SHARE",
    )
    .bind(request.user_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(member_error)?;

    if target_user.is_none() {
        return Err(api_error(StatusCode::NOT_FOUND, "User not found"));
    }

    let inserted = sqlx::query(
        "INSERT INTO team_memberships (team_id, user_id, role)
         VALUES ($1, $2, 'member')
         ON CONFLICT (team_id, user_id) DO NOTHING",
    )
    .bind(team_id)
    .bind(request.user_id)
    .execute(&mut *transaction)
    .await
    .map_err(member_error)?;

    if inserted.rows_affected() == 0 {
        return Err(api_error(
            StatusCode::CONFLICT,
            "User is already a team member",
        ));
    }

    transaction.commit().await.map_err(member_error)?;

    Ok((
        StatusCode::CREATED,
        Json(MemberResponse {
            team_id,
            user_id: request.user_id,
            role: "member".to_owned(),
        }),
    ))
}

async fn list_members(
    State(state): State<AppState>,
    user: AuthUser,
    path: Result<Path<Uuid>, PathRejection>,
) -> Result<Json<Vec<TeamMemberResponse>>, ApiError> {
    let Path(team_id) =
        path.map_err(|_| api_error(StatusCode::BAD_REQUEST, "Team ID must be a valid UUID"))?;

    let mut transaction = state.pool.begin().await.map_err(list_members_error)?;

    require_member(&mut transaction, team_id, user.id).await?;

    let rows = sqlx::query_as::<_, (Uuid, String, String)>(
        "SELECT users.id, users.name, team_memberships.role
         FROM team_memberships
         JOIN users ON users.id = team_memberships.user_id
         WHERE team_memberships.team_id = $1
         ORDER BY team_memberships.joined_at, users.id",
    )
    .bind(team_id)
    .fetch_all(&mut *transaction)
    .await
    .map_err(list_members_error)?;

    transaction.commit().await.map_err(list_members_error)?;

    let members = rows
        .into_iter()
        .map(|(user_id, name, role)| TeamMemberResponse {
            user_id,
            name,
            role,
        })
        .collect();

    Ok(Json(members))
}

fn team_error(_error: sqlx::Error) -> ApiError {
    api_error(StatusCode::INTERNAL_SERVER_ERROR, "Unable to create team")
}

fn member_error(_error: sqlx::Error) -> ApiError {
    api_error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Unable to add team member",
    )
}

fn list_members_error(_error: sqlx::Error) -> ApiError {
    api_error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Unable to list team members",
    )
}
