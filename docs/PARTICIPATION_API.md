# Participation & Scoring API

Member 3 owns these routes and the `attempt_sessions`, `quiz_attempts`, and
`attempt_answers` tables. All routes require Member 1's authentication
middleware to insert `AuthenticatedUser` into the request extensions.

## Routes

| Method | Route | Behavior |
| --- | --- | --- |
| `GET` | `/api/v1/teams/{team_id}/quizzes/available` | Eligible, open quizzes where the user has an attempt remaining |
| `POST` | `/api/v1/quizzes/{quiz_id}/attempt-sessions` | Starts or returns the user's active session |
| `GET` | `/api/v1/attempt-sessions/{session_id}/questions` | Owner-only questions without answers or explanations |
| `POST` | `/api/v1/attempt-sessions/{session_id}/submit` | Validates, scores, and atomically saves one attempt |
| `GET` | `/api/v1/quizzes/{quiz_id}/my-attempts` | Returns only the authenticated user's attempt |
| `GET` | `/api/v1/attempts/{attempt_id}/review` | Owner or team-admin post-submit answer review |
| `GET` | `/api/v1/quizzes/{quiz_id}/my-results` | Own best result and revision-specific rank |
| `GET` | `/api/v1/quizzes/{quiz_id}/analytics` | Team-admin participation, averages, and rankings |

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

- Member 1: `users`, `team_members`, and authenticated-user middleware.
- Member 2: `quizzes`, `quiz_revisions`, `question_versions`, and the
  `quiz_eligibilities` snapshot.
- Member 4: reads committed attempts; it must not recompute saved scores.

Until those branches are integrated, `fixtures/participation_flow.json` provides
design-aligned records for all upstream tables, the submission request, safe API
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
