//! Participation handlers share the application's database-backed session
//! extractor instead of defining a second authentication mechanism.

pub use crate::session::AuthUser as AuthenticatedUser;
