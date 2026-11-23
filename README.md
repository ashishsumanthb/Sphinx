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
- Eligible quiz discovery and revision-pinned attempt sessions.
- Deterministic scoring, atomic persistence and idempotent retries.
- Safe public questions and personal attempt retrieval.
- PostgreSQL-backed quiz creation, question edits, publication, and manual closure.
- Immutable published revisions and membership eligibility snapshots.

See [`docs/PARTICIPATION_API.md`](docs/PARTICIPATION_API.md) for participation
routes, security rules, database dependencies and integration notes.

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
| POST | /api/v1/teams/{team_id}/quizzes | Team admin |
| POST | /api/v1/teams/{team_id}/quizzes/{quiz_id}/questions | Team admin; draft only |
| PUT | /api/v1/teams/{team_id}/quizzes/{quiz_id}/questions/{question_id} | Team admin; creates a revision after publication |
| POST | /api/v1/quizzes/{quiz_id}/publish | Team admin |
| POST | /api/v1/quizzes/{quiz_id}/close | Team admin |

Registration accepts `name`, `email` and `password`.
Login accepts `email` and `password`.
Team creation accepts `name`.
Adding a member accepts `user_id`.

Protected requests require this header:

    Authorization: Bearer YOUR_TOKEN

Logout invalidates the current session.

Authentication, session extraction, and team endpoints retain their established
error contract: `{"error":"message"}`. Quiz-management handler validation,
authorization, and database failures use the shared structured `AppError`
envelope with `error.code`, `error.message`, and `error.request_id`. A missing or
invalid session is rejected by the shared extractor before quiz-handler input
parsing, using the established authentication response.

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
All quiz-management writes use the same lock order: team, membership, then quiz.
Publish and close first discover the quiz's team without a row lock, authorize
and lock that team, then lock the quiz while rechecking the team ID. This avoids
the lock cycle with concurrent add/edit operations.

## Verification

Run:

    cargo fmt --all -- --check
    cargo clippy --locked --all-targets -- -D warnings
    cargo test --locked --all-targets
    git diff --check

The GitHub workflow also runs PostgreSQL workflow, lock-order, attempt-limit,
and participation regressions against a disposable PostgreSQL 17 service.
To run them locally, set `TEST_DATABASE_URL` to an isolated test database:

```bash
cargo test --locked --test quiz_management_postgres \
  --test postgres_attempt_limits --test postgres_edge_cases \
  -- --ignored --test-threads=1
```

These tests reset the database's `public` schema and must run serially. The
quiz-management tests apply real migrations and exercise the full router and
login/session extractor. The concurrency regression uses PostgreSQL's wait
graph to synchronize publish/close versus add/edit; it checks responses and
persisted quiz/revision state. API tests also preserve the existing authentication
error contract across auth, team, participation, and quiz-management routes.

## Known limitations

- Passwords currently use reversible demo encoding. The original
  Argon2id password hashing requirement remains outstanding.
- Session expiry and outsider access need final verification after
  the refactor.
- Role-changing and member-removal endpoints are outside the
  current smaller implementation.
- Quiz participation requires the upstream quiz, revision, question and
  eligibility migrations described in the participation API documentation.
