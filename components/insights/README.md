# Sphinx Insights component

Runnable member-results component for the Sphinx collaborative Rust backend. It provides attempt history, best percentage and competition rank per quiz revision through `GET /api/v1/quizzes/{quiz_id}/my-results`.

The working scope follows the Insights work developed in the conversation. The uploaded documents retain a different Member 4 UI/DevOps ownership proposal. Team confirmation of that wider ownership remains outstanding. Admin quiz/team analytics and other members' authentication, quiz creation and scoring are separate integration work.

## Run locally

Prerequisites are Docker Engine with Compose, Rust 1.100.0 (the version used for verification) with rustfmt and clippy, Python 3 and psql. Cargo.lock pins the tested dependencies. On NixOS you can obtain the client tools in your current shell with `nix shell nixpkgs#postgresql nixpkgs#python3`. Use your existing Rust installation. If needed, add its components with `rustup component add rustfmt clippy`. Start Docker using your existing NixOS Docker setup.

Extract the download into a separate folder first, then run from the extracted `sphinx-insights` directory:

```bash
docker compose up -d --wait db
export INSIGHTS_TEST_DATABASE_URL='postgres://sphinx_demo:local-demo-password@127.0.0.1:55432/sphinx'
bash scripts/verify.sh
cargo run --locked --example demo
```

The database password above is a published fictional local-demo credential. Compose publishes PostgreSQL only on loopback port 55432. The API example binds only to `127.0.0.1:8000`. The demo creates and prints a fresh isolated schema on every run and seeds fictional users using UUIDs. It uses the full DDL extracted from sphinx_design.md, rather than the earlier varchar scratch table. The schema fixtures live under tests/support and are never applied by the reusable component.

From another terminal, in the same extracted directory:

```bash
python3 scripts/verify_http.py
curl -i -H 'Authorization: Bearer demo-a' http://127.0.0.1:8000/api/v1/quizzes/00000000-0000-4000-8000-000000000100/my-results
```

Expected response includes A's two attempts (70%, 90%), best percentage 90 and rank 1 for revision 1. The live verification script prints PASS only after all its assertions succeed. Stop the API with Ctrl+C, then use `docker compose stop db` to stop the demo database. Demo/test schemas remain in the dedicated demo volume for inspection; reruns use unique names. The SQL-only verifier rolls its schema back automatically.

For an existing local test PostgreSQL server, set INSIGHTS_TEST_DATABASE_URL to that test database and omit Compose. Its user needs CREATE SCHEMA permission. If Compose reports port 55432 is busy, use your existing test database or coordinate a local port change and update the URL. An occupied API port 8000 must be freed before running this example.

## Files and ownership

| Path | Responsibility |
| --- | --- |
| src/repository/analytics.rs | SQLx parameter binding and quiz access lookup |
| src/services/analytics_service.rs | Read-only repeatable-read transaction, access check and result assembly |
| src/api/handlers/analytics.rs | Route inputs, trusted identity, request IDs and safe structured logs |
| src/api/dto.rs | JSON response types using snake_case |
| src/api/error.rs | Structured errors with request_id |
| sql/insights/ | History, ranking and access parameterized queries |
| tests/support/ | Exact design DDL and UUID fixtures for isolated tests/demo |
| tests/database_tests.rs | PostgreSQL-backed HTTP acceptance test |
| tests/api_tests.rs | Authentication and malformed UUID checks without a database |
| scripts/verify_sql.py | Executes actual SQL file contents and assertions in a rolled-back schema |
| examples/demo.rs | Local fictional-identity adapter and API executable |
| docs/ | API contract, peer walkthrough, integration guidance and test evidence |

The source layers and field casing follow design.md section 10 and sphinx_design.md sections 4, 5 and 7. The independent Cargo package, demo adapter, response envelope and verification scripts are implementation choices for running the component without the shared application. Review these choices during integration.

See docs/WALKTHROUGH.md for the presentation and docs/INTEGRATION.md before copying files into your branch. See docs/VERIFICATION.md for what was executed here and what needs a local socket-enabled run.
