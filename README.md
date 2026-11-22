# Sphinx

Team quiz and knowledge-sharing backend built with Rust, Axum,
SQLx and PostgreSQL.

## Current implementation

Identity and teams functionality includes:

- Registration and login.
- Database sessions with a one-hour expiry.
- Logout and current-user lookup.
- Creating and listing teams.
- Adding and listing team members.
- Shared membership and admin permission checks.

The team creator becomes an admin. Added users receive the member role.

## Local setup

Requirements: Rust, Cargo, Docker and Docker Compose.

Run commands from the project root.

1. Create your local configuration:

   cp -n .env.example .env

2. Edit `.env` to match your PostgreSQL credentials and exposed port.

3. Start PostgreSQL:

   docker compose up -d --wait db

4. Start the API:

   cargo run

The application applies database migrations during startup.
The local API runs at http://127.0.0.1:8000.

Check that it is running:

    curl http://127.0.0.1:8000/health/live

Keep `.env` out of Git.

## API endpoints

| Method | Endpoint | Access |
| --- | --- | --- |
| GET | /health/live | Public |
| POST | /api/v1/auth/register | Public |
| POST | /api/v1/auth/login | Public |
| GET | /api/v1/auth/me | Logged-in user |
| POST | /api/v1/auth/logout | Logged-in user |
| POST | /api/v1/teams | Logged-in user |
| GET | /api/v1/teams | Own teams |
| POST | /api/v1/teams/{team_id}/members | Team admin |
| GET | /api/v1/teams/{team_id}/members | Team member or admin |

Registration accepts `name`, `email` and `password`.
Login accepts `email` and `password`.
Team creation accepts `name`.
Adding a member accepts `user_id`.

Protected requests require this header:

    Authorization: Bearer YOUR_TOKEN

Logout invalidates the current session.

## Code organisation

| File | Responsibility |
| --- | --- |
| src/main.rs | Configuration, database connection and server startup |
| src/lib.rs | Application state and routes |
| src/auth.rs | Registration, login, logout and current-user endpoint |
| src/session.rs | Token creation, session verification and revocation |
| src/password.rs | Demo password encoding |
| src/teams.rs | Team and membership endpoints |
| src/permissions.rs | Shared member and admin access checks |
| migrations/ | Database schema changes |

Other modules can use `auth::AuthUser` to identify the caller.

Use `permissions::require_member` or `permissions::require_admin`
inside the same transaction as the protected database operation.
Admin checks acquire the team lock before membership changes.

## Verification

Run:

    cargo fmt --check
    cargo check
    cargo test
    git diff --check

Current automated tests cover registration validation and demo
password encoding. Login, logout and team permissions have also
been checked manually through the API.

## Known limitations

- Passwords currently use reversible demo encoding. The original
  Argon2id password hashing requirement remains outstanding.
- Session expiry and outsider access need final verification after
  the refactor.
- Role-changing and member-removal endpoints are outside the
  current smaller implementation.
- Full quiz functionality requires integration with the other
  project modules.