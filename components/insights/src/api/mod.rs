pub mod dto;
pub mod error;
pub mod handlers;

/// Trusted identity inserted by the host's verified authentication middleware.
/// Never populate this from a caller-supplied user ID header or query parameter.
#[derive(Clone, Copy)]
pub struct AuthenticatedUser {
    pub user_id: uuid::Uuid,
}
