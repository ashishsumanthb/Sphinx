use axum::{
    Json, Router,
    extract::{
        Path, State,
        rejection::{JsonRejection, PathRejection},
    },
    http::StatusCode,
    routing::{patch, post},
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

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum TeamRole {
    Admin,
    Member,
}

impl TeamRole {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Member => "member",
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateRoleRequest {
    role: TeamRole,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", post(create_team).get(list_teams))
        .route("/{team_id}/members", post(add_member).get(list_members))
        .route(
            "/{team_id}/members/{user_id}",
            patch(update_member_role).delete(remove_member),
        )
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

    // Membership changes lock the team first to serialize changes within it.
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

    let membership = sqlx::query_as::<_, (String,)>(
        "SELECT role
         FROM team_memberships
         WHERE team_id = $1 AND user_id = $2
         FOR UPDATE",
    )
    .bind(team_id)
    .bind(user.id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(member_error)?;

    match membership {
        Some((role,)) if role == "admin" => {}
        Some(_) => {
            return Err(api_error(
                StatusCode::FORBIDDEN,
                "Only team admins can add members",
            ));
        }
        None => {
            return Err(api_error(StatusCode::NOT_FOUND, "Team not found"));
        }
    }

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

    // Keep the caller's membership valid while reading the member list.
    let membership = sqlx::query_as::<_, (Uuid,)>(
        "SELECT user_id
         FROM team_memberships
         WHERE team_id = $1 AND user_id = $2
         FOR SHARE",
    )
    .bind(team_id)
    .bind(user.id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(list_members_error)?;

    if membership.is_none() {
        return Err(api_error(StatusCode::NOT_FOUND, "Team not found"));
    }

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

async fn update_member_role(
    State(state): State<AppState>,
    user: AuthUser,
    path: Result<Path<(Uuid, Uuid)>, PathRejection>,
    payload: Result<Json<UpdateRoleRequest>, JsonRejection>,
) -> Result<Json<MemberResponse>, ApiError> {
    let Path((team_id, target_user_id)) = path.map_err(|_| {
        api_error(
            StatusCode::BAD_REQUEST,
            "Team ID and user ID must be valid UUIDs",
        )
    })?;

    let Json(request) = payload.map_err(|error| {
        api_error(
            error.status(),
            "Expected JSON with role set to admin or member",
        )
    })?;

    let mut transaction = state.pool.begin().await.map_err(role_error)?;

    let team = sqlx::query_as::<_, (Uuid,)>(
        "SELECT id
         FROM teams
         WHERE id = $1
         FOR UPDATE",
    )
    .bind(team_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(role_error)?;

    if team.is_none() {
        return Err(api_error(StatusCode::NOT_FOUND, "Team not found"));
    }

    let caller = sqlx::query_as::<_, (String,)>(
        "SELECT role
         FROM team_memberships
         WHERE team_id = $1 AND user_id = $2
         FOR UPDATE",
    )
    .bind(team_id)
    .bind(user.id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(role_error)?;

    match caller {
        Some((role,)) if role == "admin" => {}
        Some(_) => {
            return Err(api_error(
                StatusCode::FORBIDDEN,
                "Only team admins can change roles",
            ));
        }
        None => {
            return Err(api_error(StatusCode::NOT_FOUND, "Team not found"));
        }
    }

    let target = sqlx::query_as::<_, (String,)>(
        "SELECT role
         FROM team_memberships
         WHERE team_id = $1 AND user_id = $2
         FOR UPDATE",
    )
    .bind(team_id)
    .bind(target_user_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(role_error)?;

    let (current_role,) =
        target.ok_or_else(|| api_error(StatusCode::NOT_FOUND, "Team member not found"))?;

    let new_role = request.role.as_str();

    if current_role == "admin" && new_role == "member" {
        let (admin_count,) = sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*)
             FROM team_memberships
             WHERE team_id = $1 AND role = 'admin'",
        )
        .bind(team_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(role_error)?;

        if admin_count <= 1 {
            return Err(api_error(
                StatusCode::CONFLICT,
                "Cannot demote the last team admin",
            ));
        }
    }

    sqlx::query(
        "UPDATE team_memberships
         SET role = $1
         WHERE team_id = $2 AND user_id = $3",
    )
    .bind(new_role)
    .bind(team_id)
    .bind(target_user_id)
    .execute(&mut *transaction)
    .await
    .map_err(role_error)?;

    transaction.commit().await.map_err(role_error)?;

    Ok(Json(MemberResponse {
        team_id,
        user_id: target_user_id,
        role: new_role.to_owned(),
    }))
}

async fn remove_member(
    State(state): State<AppState>,
    user: AuthUser,
    path: Result<Path<(Uuid, Uuid)>, PathRejection>,
) -> Result<StatusCode, ApiError> {
    let Path((team_id, target_user_id)) = path.map_err(|_| {
        api_error(
            StatusCode::BAD_REQUEST,
            "Team ID and user ID must be valid UUIDs",
        )
    })?;

    let mut transaction = state.pool.begin().await.map_err(remove_member_error)?;

    let team = sqlx::query_as::<_, (Uuid,)>(
        "SELECT id
         FROM teams
         WHERE id = $1
         FOR UPDATE",
    )
    .bind(team_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(remove_member_error)?;

    if team.is_none() {
        return Err(api_error(StatusCode::NOT_FOUND, "Team not found"));
    }

    let caller = sqlx::query_as::<_, (String,)>(
        "SELECT role
         FROM team_memberships
         WHERE team_id = $1 AND user_id = $2
         FOR UPDATE",
    )
    .bind(team_id)
    .bind(user.id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(remove_member_error)?;

    match caller {
        Some((role,)) if role == "admin" => {}
        Some(_) => {
            return Err(api_error(
                StatusCode::FORBIDDEN,
                "Only team admins can remove members",
            ));
        }
        None => {
            return Err(api_error(StatusCode::NOT_FOUND, "Team not found"));
        }
    }

    let target = sqlx::query_as::<_, (String,)>(
        "SELECT role
         FROM team_memberships
         WHERE team_id = $1 AND user_id = $2
         FOR UPDATE",
    )
    .bind(team_id)
    .bind(target_user_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(remove_member_error)?;

    let (role,) =
        target.ok_or_else(|| api_error(StatusCode::NOT_FOUND, "Team member not found"))?;

    if role == "admin" {
        let (admin_count,) = sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*)
             FROM team_memberships
             WHERE team_id = $1 AND role = 'admin'",
        )
        .bind(team_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(remove_member_error)?;

        if admin_count <= 1 {
            return Err(api_error(
                StatusCode::CONFLICT,
                "Cannot remove the last team admin",
            ));
        }
    }

    sqlx::query(
        "DELETE FROM team_memberships
         WHERE team_id = $1 AND user_id = $2",
    )
    .bind(team_id)
    .bind(target_user_id)
    .execute(&mut *transaction)
    .await
    .map_err(remove_member_error)?;

    transaction.commit().await.map_err(remove_member_error)?;

    Ok(StatusCode::NO_CONTENT)
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

fn role_error(_error: sqlx::Error) -> ApiError {
    api_error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Unable to update member role",
    )
}

fn remove_member_error(_error: sqlx::Error) -> ApiError {
    api_error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Unable to remove team member",
    )
}
