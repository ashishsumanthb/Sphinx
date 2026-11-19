use crate::{AppState, password};
use axum::{
    Json, Router,
    extract::{FromRequestParts, State, rejection::JsonRejection},
    http::{StatusCode, header::AUTHORIZATION, request::Parts},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegisterRequest {
    name: String,
    email: String,
    password: String,
}

#[derive(Serialize)]
struct UserResponse {
    id: Uuid,
    name: String,
    email: String,
}

#[derive(Serialize)]
pub(crate) struct ErrorResponse {
    error: &'static str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Serialize)]
struct LoginResponse {
    token: String,
    token_type: &'static str,
    expires_in: i64,
    user: UserResponse,
}

pub(crate) type ApiError = (StatusCode, Json<ErrorResponse>);

pub(crate) fn api_error(status: StatusCode, message: &'static str) -> ApiError {
    (status, Json(ErrorResponse { error: message }))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/me", get(me))
        .route("/logout", post(logout))
}

async fn register(
    State(state): State<AppState>,
    payload: Result<Json<RegisterRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<UserResponse>), ApiError> {
    let Json(request) = payload.map_err(|error| {
        api_error(
            error.status(),
            "Expected JSON with name, email and password fields",
        )
    })?;

    validate_registration(&request)
        .map_err(|message| api_error(StatusCode::BAD_REQUEST, message))?;

    let name = request.name.trim().to_owned();
    let email = request.email.trim().to_lowercase();

    // Demo encoding only; this does not securely protect passwords.
    let password_encoded = password::encode(&request.password);

    let saved_user = sqlx::query_as::<_, (Uuid, String, String)>(
        "INSERT INTO users (name, email, password_encoded)
         VALUES ($1, $2, $3)
         ON CONFLICT ON CONSTRAINT users_email_unique DO NOTHING
         RETURNING id, name, email",
    )
    .bind(&name)
    .bind(&email)
    .bind(&password_encoded)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| {
        api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Unable to create account",
        )
    })?;

    let (id, name, email) = saved_user.ok_or_else(|| {
        api_error(
            StatusCode::CONFLICT,
            "An account with this email already exists",
        )
    })?;

    Ok((StatusCode::CREATED, Json(UserResponse { id, name, email })))
}

fn validate_registration(request: &RegisterRequest) -> Result<(), &'static str> {
    let name = request.name.trim();

    if !(1..=100).contains(&name.chars().count()) {
        return Err("Name must contain 1 to 100 characters");
    }

    if name.chars().any(char::is_control) {
        return Err("Name must not contain control characters");
    }

    let email = request.email.trim();

    if email.is_empty() {
        return Err("Email is required");
    }

    if !valid_email(email) {
        return Err("Enter a valid email address");
    }

    if !(8..=128).contains(&request.password.chars().count()) {
        return Err("Password must contain 8 to 128 characters");
    }

    Ok(())
}

// Supports common ASCII email addresses, not every RFC email format.
// This checks the format, not whether the mailbox exists.
fn valid_email(email: &str) -> bool {
    if email.len() > 254 || !email.is_ascii() {
        return false;
    }

    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };

    if local.is_empty()
        || local.len() > 64
        || local.starts_with('.')
        || local.ends_with('.')
        || local.contains("..")
    {
        return false;
    }

    let valid_local = local
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || b".!#$%&'*+-/=?^_`{|}~".contains(&byte));

    let valid_domain = domain.contains('.')
        && domain.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        });

    valid_local && valid_domain
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

async fn login(
    State(state): State<AppState>,
    payload: Result<Json<LoginRequest>, JsonRejection>,
) -> Result<Json<LoginResponse>, ApiError> {
    let Json(request) = payload.map_err(|error| {
        api_error(
            error.status(),
            "Expected JSON with email and password fields",
        )
    })?;

    let email = request.email.trim().to_lowercase();

    if !valid_email(&email) || !(8..=128).contains(&request.password.chars().count()) {
        return Err(api_error(
            StatusCode::UNAUTHORIZED,
            "Invalid email or password",
        ));
    }

    let saved_user = sqlx::query_as::<_, (Uuid, String, String, String)>(
        "SELECT id, name, email, password_encoded
         FROM users
         WHERE email = $1",
    )
    .bind(&email)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "Unable to log in"))?;

    let (id, name, email, password_encoded) = saved_user
        .ok_or_else(|| api_error(StatusCode::UNAUTHORIZED, "Invalid email or password"))?;

    // Verify the reversible demo encoding without decoding it.
    if password::encode(&request.password) != password_encoded {
        return Err(api_error(
            StatusCode::UNAUTHORIZED,
            "Invalid email or password",
        ));
    }

    let (token, token_hash) = generate_session_token()?;
    let expires_in: i64 = 3600;

    sqlx::query(
        "INSERT INTO login_sessions (token_hash, user_id, expires_at)
         VALUES ($1, $2, now() + make_interval(secs => $3))",
    )
    .bind(&token_hash)
    .bind(id)
    .bind(expires_in as f64)
    .execute(&state.pool)
    .await
    .map_err(|_| {
        api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Unable to create login session",
        )
    })?;

    Ok(Json(LoginResponse {
        token,
        token_type: "Bearer",
        expires_in,
        user: UserResponse { id, name, email },
    }))
}

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

async fn me(user: AuthUser) -> Json<UserResponse> {
    Json(UserResponse {
        id: user.id,
        name: user.name,
        email: user.email,
    })
}
async fn logout(State(state): State<AppState>, user: AuthUser) -> Result<StatusCode, ApiError> {
    sqlx::query("DELETE FROM login_sessions WHERE token_hash = $1")
        .bind(&user.token_hash)
        .execute(&state.pool)
        .await
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "Unable to log out"))?;

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]

mod tests {
    use super::{RegisterRequest, validate_registration};

    fn valid_request() -> RegisterRequest {
        RegisterRequest {
            name: String::from("Demo User"),
            email: String::from("demo@example.com"),
            password: String::from("DemoOnly123!"),
        }
    }

    #[test]
    fn accepts_valid_details() {
        let request = valid_request();

        assert_eq!(validate_registration(&request), Ok(()));
    }

    #[test]
    fn rejects_blank_name() {
        let mut request = valid_request();
        request.name = String::from("   ");

        assert_eq!(
            validate_registration(&request),
            Err("Name must contain 1 to 100 characters")
        );
    }

    #[test]
    fn rejects_empty_email() {
        let mut request = valid_request();
        request.email = String::new();

        assert_eq!(validate_registration(&request), Err("Email is required"));
    }

    #[test]
    fn rejects_short_password() {
        let mut request = valid_request();
        request.password = String::from("abc");

        assert_eq!(
            validate_registration(&request),
            Err("Password must contain 8 to 128 characters")
        );
    }

    #[test]
    fn rejects_invalid_email_formats() {
        for email in [
            "invalid",
            "@example.com",
            "demo@",
            "demo@@example.com",
            "demo user@example.com",
            "demo@example..com",
            ".demo@example.com",
        ] {
            let mut request = valid_request();
            request.email = email.to_owned();

            assert_eq!(
                validate_registration(&request),
                Err("Enter a valid email address")
            );
        }
    }

    #[test]
    fn rejects_null_character_in_name() {
        let mut request = valid_request();
        request.name = String::from("Demo\0User");

        assert_eq!(
            validate_registration(&request),
            Err("Name must not contain control characters")
        );
    }

    #[test]
    fn checks_password_length_boundaries() {
        for (length, accepted) in [(7, false), (8, true), (128, true), (129, false)] {
            let mut request = valid_request();
            request.password = "a".repeat(length);

            assert_eq!(validate_registration(&request).is_ok(), accepted);
        }
    }
}
