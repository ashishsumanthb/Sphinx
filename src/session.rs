use axum::{
    extract::FromRequestParts,
    http::{StatusCode, header::AUTHORIZATION, request::Parts},
    response::{IntoResponse, Response},
};
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    AppState,
    auth::{ApiError, api_error},
};

pub struct AuthUser {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    token_hash: Vec<u8>,
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let unauthorized = || {
            api_error(
                StatusCode::UNAUTHORIZED,
                "A valid login session is required",
            )
            .into_response()
        };

        let mut headers = parts.headers.get_all(AUTHORIZATION).iter();
        let header = headers.next().ok_or_else(unauthorized)?;

        if headers.next().is_some() {
            return Err(unauthorized());
        }

        let header = header.to_str().map_err(|_| unauthorized())?;
        let (scheme, token) = header.split_once(' ').ok_or_else(unauthorized)?;

        if !scheme.eq_ignore_ascii_case("Bearer")
            || token.len() != 64
            || !token.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(unauthorized());
        }

        let token_hash = Sha256::digest(token.as_bytes()).to_vec();

        let user = sqlx::query_as::<_, (Uuid, String, String)>(
            "SELECT users.id, users.name, users.email
             FROM login_sessions
             JOIN users ON users.id = login_sessions.user_id
             WHERE login_sessions.token_hash = $1
               AND login_sessions.expires_at > now()",
        )
        .bind(&token_hash)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| {
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Unable to verify login session",
            )
            .into_response()
        })?;

        let (id, name, email) = user.ok_or_else(unauthorized)?;

        Ok(Self {
            id,
            name,
            email,
            token_hash,
        })
    }
}

fn generate_session_token() -> Result<(String, Vec<u8>), ApiError> {
    let mut random_bytes = [0u8; 32];

    OsRng.try_fill_bytes(&mut random_bytes).map_err(|_| {
        api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Unable to create login session",
        )
    })?;

    let token = hex::encode(random_bytes);
    let token_hash = Sha256::digest(token.as_bytes()).to_vec();

    Ok((token, token_hash))
}

pub(crate) async fn create(pool: &PgPool, user_id: Uuid) -> Result<(String, i64), ApiError> {
    let (token, token_hash) = generate_session_token()?;
    let expires_in: i64 = 3600;

    sqlx::query(
        "INSERT INTO login_sessions (token_hash, user_id, expires_at)
         VALUES ($1, $2, now() + make_interval(secs => $3))",
    )
    .bind(&token_hash)
    .bind(user_id)
    .bind(expires_in as f64)
    .execute(pool)
    .await
    .map_err(|_| {
        api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Unable to create login session",
        )
    })?;

    Ok((token, expires_in))
}

pub(crate) async fn revoke(pool: &PgPool, user: &AuthUser) -> Result<(), ApiError> {
    sqlx::query("DELETE FROM login_sessions WHERE token_hash = $1")
        .bind(&user.token_hash)
        .execute(pool)
        .await
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "Unable to log out"))?;

    Ok(())
}
