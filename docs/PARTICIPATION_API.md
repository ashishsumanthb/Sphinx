# Participation & Scoring API

Member 3 owns these routes and the `attempt_sessions`, `quiz_attempts`, and
`attempt_answers` tables. All routes use the shared `session::AuthUser`
extractor (aliased as `AuthenticatedUser`) and its `id` field. Callers send a
Bearer login-session token; no extension adapter or second results router is required.

## Routes

| Method | Route | Behavior |
| --- | --- | --- |
| `GET` | `/api/v1/teams/{team_id}/quizzes/available` | Eligible, open quizzes where the user has an attempt remaining |
| `POST` | `/api/v1/quizzes/{quiz_id}/attempt-sessions` | Starts or returns the user's active session |
| `GET` | `/api/v1/attempt-sessions/{session_id}/questions` | Owner-only questions without answers or explanations |
| `POST` | `/api/v1/attempt-sessions/{session_id}/submit` | Validates, scores, and atomically saves one attempt |
| `GET` | `/api/v1/quizzes/{quiz_id}/my-attempts` | Returns only the authenticated user's attempt |
| `GET` | `/api/v1/attempts/{attempt_id}/review` | Owner or team-admin post-submit answer review |
| `GET` | `/api/v1/quizzes/{quiz_id}/my-results` | Latest attempted revision summary, personal attempts, and per-revision results |
| `GET` | `/api/v1/quizzes/{quiz_id}/analytics` | Team-admin participation, averages, and rankings |

## Member results

The existing route, handler, service, repository, and shared DTO implement
insights in the root crate. The standalone `components/insights` implementation
has been removed. Access requires current `team_memberships` membership and
either quiz eligibility or the team's admin role. Admins see their own results;
the analytics endpoint remains the team-wide view. Denied or unknown quizzes
return the host `AppError` (`403 FORBIDDEN`); authentication failures retain the
shared session extractor's `401` response.

`best_score`, `best_percentage`, `revision_number`, and `rank` all describe the
highest `revision_no` the caller has attempted, regardless of revision creation
or submission timestamps. An earlier revision's better score never replaces
the summary. Unattempted revisions do not affect it.

`revision_results` is ordered by ascending `revision_number`. Each entry contains
`quiz_revision_id`, `revision_number`, `best_score`, `best_percentage`, and `rank`.
Ranks compare each participant's best saved percentage within that revision,
using competition ranking (`1, 1, 3`). The selected best attempt is deterministic:
percentage descending, then submission time and attempt ID ascending. Rank ties
use percentage alone. `attempts` retains the caller's full history, ordered by
submission time then attempt ID, and now includes revision ID and number.

For an authorized caller with no attempts, all four summary fields are null and
both arrays are empty. Authorization, attempts, and ranking use one read-only
repeatable-read PostgreSQL transaction so the response uses a consistent snapshot.

## Submission

```json
{
  "answers": [
    {
      "question_id": "11111111-1111-4111-8111-111111111111",
      "selected_option_index": 1
    }
  ]
}
```

Every published question must be answered exactly once. Unknown questions,
duplicates, missing answers, and invalid option indices return
`422 INVALID_ANSWER_SET`.

The submission transaction locks the quiz row, reads PostgreSQL
`clock_timestamp()`, checks status, closing time, manual closure, eligibility,
and the quiz's `max_attempts` limit, then inserts the numbered attempt and all answers before
committing. No partial attempt is visible.

Quizzes permit one or two completed attempts according to `max_attempts`.
Attempts are stored with `attempt_no`, and `UNIQUE (quiz_id, user_id,
attempt_no)` allows each permitted attempt while preventing duplicate attempt
numbers. The independent unique `session_id` constraint still prevents the
same session from being counted twice.

Identical retries use an order-independent SHA-256 request hash and return the
stored result with `idempotent_replay: true`, even after the quiz closes. A
retry for the same session with different answers returns
`409 SESSION_ALREADY_SUBMITTED`.

## Data exposure

Pre-submission responses contain only `question_id`, `position`,
`question_text`, and `options`. They never serialize `correct_option_index`,
`is_correct`, or `explanation`. Correct answers and explanations appear only in
the submitted attempt owner's review or to an administrator of that quiz's
team. Member result endpoints take identity exclusively from authentication and
never accept a user ID.

Requests are limited to 1 MiB. Oversized requests return the standard
`PAYLOAD_TOO_LARGE` error envelope.

## Verification

```bash
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
```

CI also runs all three PostgreSQL suites against a disposable PostgreSQL 17
service. They apply the real root migrations, including the unchanged
`0020_quiz_management.sql` from `feat/quiz-management` (`c7a69ce`); no test defines
alternative tables or columns. Run the same database checks locally with:

```bash
TEST_DATABASE_URL=postgres://postgres:edgepass@127.0.0.1:55432/sphinx_edge \
  cargo test --locked --test insights_postgres --test postgres_attempt_limits \
  --test postgres_edge_cases -- --ignored --test-threads=1
```

The insights suite invokes the complete host router and real login-session
extractor. It covers eligible and ineligible members, admins without eligibility,
outsiders including another team's admin, removed membership, missing/expired
sessions, no attempts, multiple revisions with inverted timestamps, an
unattempted newer revision, ties, repeat attempts, and summary/history consistency.
These suites reset `public`; use an isolated test database and run them serially.

Run the PostgreSQL timing, concurrency, rollback, retry, authorization,
constraint, analytics, and dependency-failure matrix against an isolated test
database:

```bash
docker run --name sphinx-edge-postgres \
  -e POSTGRES_PASSWORD=edgepass -e POSTGRES_DB=sphinx_edge \
  -p 127.0.0.1:55432:5432 -d postgres:17-alpine

TEST_DATABASE_URL=postgres://postgres:edgepass@127.0.0.1:55432/sphinx_edge \
  cargo test --test postgres_edge_cases -- --ignored --nocapture

docker rm -f sphinx-edge-postgres
```

The test resets the `public` schema and must never target a development or
production database.

## Integration dependencies

- Member 1: `users`, `team_memberships`, and the shared session extractor.
- Member 2: `quizzes`, `quiz_revisions`, `question_versions`, and the
  `quiz_eligibilities` snapshot.
- Member 4: reads committed attempts; it must not recompute saved scores.

`fixtures/participation_flow.json` provides shared-schema records for upstream
tables, the submission request, safe API
responses, and the expected persisted attempt rows. The fixture is validated by
`tests/mock_fixture.rs`, so DTO or relationship drift fails the test suite.

## PostgreSQL attempt-limit test

The integration test uses an isolated database because it resets the `public`
schema. It proves that a one-attempt quiz rejects attempt 2 and a two-attempt
quiz accepts attempt 2 but rejects attempt 3.

```bash
TEST_DATABASE_URL=postgres://postgres:edgepass@127.0.0.1:55433/sphinx_attempts \
  cargo test --test postgres_attempt_limits -- --ignored --nocapture
```
