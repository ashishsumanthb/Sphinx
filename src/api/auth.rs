use async_trait::async_trait;
use axum::{extract::FromRequestParts, http::request::Parts};
use uuid::Uuid;

use crate::api::error::AppError;

/// Shared contract: Member 1's authentication middleware inserts this value.
#[derive(Debug, Clone, Copy)]
pub struct AuthenticatedUser {
    pub user_id: Uuid,
}

#[async_trait]
impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Self>()
            .copied()
            .ok_or_else(AppError::unauthenticated)
    }
}
