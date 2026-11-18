use axum::{
    Json, Router,
    extract::{State, rejection::JsonRejection},
    http::StatusCode,
    routing::post,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{AppState, password};

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
struct ErrorResponse {
    error: &'static str,
}

type ApiError = (StatusCode, Json<ErrorResponse>);

fn api_error(status: StatusCode, message: &'static str) -> ApiError {
    (status, Json(ErrorResponse { error: message }))
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/register", post(register))
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
