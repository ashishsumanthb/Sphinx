pub use crate::session::AuthUser;
use crate::{AppState, password, session};
use axum::{
    Json, Router,
    extract::{State, rejection::JsonRejection},
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
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
    pub(crate) error: &'static str,
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
    .map_err(database_error("Unable to create account"))?;

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
        return Err(invalid_credentials());
    }

    let saved_user = sqlx::query_as::<_, (Uuid, String, String, String)>(
        "SELECT id, name, email, password_encoded
         FROM users
         WHERE email = $1",
    )
    .bind(&email)
    .fetch_optional(&state.pool)
    .await
    .map_err(database_error("Unable to log in"))?;

    let (id, name, email, password_encoded) = saved_user.ok_or_else(invalid_credentials)?;

    // Verify the reversible demo encoding without decoding it.
    if password::encode(&request.password) != password_encoded {
        return Err(invalid_credentials());
    }

    let (token, expires_in) = session::create(&state.pool, id).await?;

    Ok(Json(LoginResponse {
        token,
        token_type: "Bearer",
        expires_in,
        user: UserResponse { id, name, email },
    }))
}

async fn me(user: AuthUser) -> Json<UserResponse> {
    Json(UserResponse {
        id: user.id,
        name: user.name,
        email: user.email,
    })
}
async fn logout(State(state): State<AppState>, user: AuthUser) -> Result<StatusCode, ApiError> {
    session::revoke(&state.pool, &user).await?;

    Ok(StatusCode::NO_CONTENT)
}

fn invalid_credentials() -> ApiError {
    api_error(StatusCode::UNAUTHORIZED, "Invalid email or password")
}

fn database_error(message: &'static str) -> impl FnOnce(sqlx::Error) -> ApiError {
    move |_| api_error(StatusCode::INTERNAL_SERVER_ERROR, message)
}

#[cfg(test)]
mod tests {
    use super::{RegisterRequest, validate_registration};

    fn request(name: &str, email: &str, password: &str) -> RegisterRequest {
        RegisterRequest {
            name: name.to_owned(),
            email: email.to_owned(),
            password: password.to_owned(),
        }
    }

    #[test]
    fn validates_registration_details() {
        let cases = [
            ("Demo User", "demo@example.com", "DemoOnly123!", Ok(())),
            (
                "   ",
                "demo@example.com",
                "DemoOnly123!",
                Err("Name must contain 1 to 100 characters"),
            ),
            (
                "Demo\0User",
                "demo@example.com",
                "DemoOnly123!",
                Err("Name must not contain control characters"),
            ),
            ("Demo User", "", "DemoOnly123!", Err("Email is required")),
            (
                "Demo User",
                "demo@example.com",
                "abc",
                Err("Password must contain 8 to 128 characters"),
            ),
        ];

        for (name, email, password, expected) in cases {
            assert_eq!(
                validate_registration(&request(name, email, password)),
                expected
            );
        }
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
            assert_eq!(
                validate_registration(&request("Demo User", email, "DemoOnly123!")),
                Err("Enter a valid email address"),
                "Unexpected result for {email}"
            );
        }
    }

    #[test]
    fn checks_password_length_boundaries() {
        for (length, accepted) in [(7, false), (8, true), (128, true), (129, false)] {
            let input = request("Demo User", "demo@example.com", &"a".repeat(length));
            assert_eq!(validate_registration(&input).is_ok(), accepted);
        }
    }
}
