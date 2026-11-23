# Host results contract

Canonical production route: `GET /api/v1/quizzes/{quiz_id}/my-results`.
The existing host handler uses the real login Bearer token through `session::AuthUser`.
No public user_id parameter can select another member's results.

The response preserves `quiz_id`, `best_score`, `best_percentage`, `revision_number`,
`rank`, and `attempts`. It adds `revision_results`, ordered by revision number,
with `quiz_revision_id`, `quiz_revision_number`, `best_percentage`, and `rank`.
Summary fields refer to the highest revision_no attempted by the caller. All four
summary fields are null if the authorized caller has no attempts; both arrays
are empty. A later unattempted quiz revision does not change the summary.
Ranks use exact numeric best percentages per participant/revision and competition
ranking (1, 1, 3). All attempts are retained in history. The summary's best score
is the raw score of the selected best-percentage attempt in that same revision.

Membership plus eligibility is required, except that current team admins may
retrieve their own results without eligibility. Missing and inaccessible quizzes
both return the existing host AppError 404 NOT_FOUND envelope. Authentication
failures retain the existing host session extractor's response.

This contract is implemented atomically in the bundled DTO and repository change;
it still needs teammate review before merging.

## Standalone demo contract (not mounted in host)

# Member results API contract

## Route and identity

`GET /api/v1/quizzes/{quiz_id}/my-results`. quiz_id is a UUID. The host's authentication middleware verifies its real token/session and inserts `sphinx_insights::api::AuthenticatedUser { user_id }` into request extensions. The component never accepts identity from URL/query/header user IDs. Missing trusted identity returns 401. The standalone example maps fictional bearer strings to fixtures solely to demonstrate integration.

The selected access policy requires current team membership plus publish-time quiz eligibility, or current team-admin membership. Soft-deleted teams are hidden. Inaccessible and missing quizzes both return 404. This operationalizes the design's own-results, eligible-member and admin rules and should be reconciled with the team's central access policy before merging.

## Success response

200 JSON fields are `quiz_id`, `attempts` and `revision_results`. An accessible quiz without the caller's submissions returns two empty arrays. There is no fabricated rank 0 or score 0 for an absent submission.

Each attempt has `attempt_id`, `quiz_revision_id`, `quiz_revision_number`, `attempt_number`, `score`, `total_questions`, `percentage`, `submitted_at`. History is sorted by attempt number, timestamp, ID. Each revision result has `quiz_revision_id`, `quiz_revision_number`, `best_percentage`, `rank`, sorted by revision number. `quiz_revision_number` follows the newer Sphinx response example. The envelope and revision-result field names are this component's explicit proposal because the design does not supply a complete my-results response DTO.

The API selects the maximum unrounded numeric percentage for each user and revision of the selected quiz. RANK produces 1,1,3 for a tie. The query filters the requesting user after computing rank. Percentages are converted to JSON-compatible double precision only after ranking. Display rounding belongs to the UI. The schema enforces total_questions > 0 and score between 0 and total_questions. Completed rows in quiz_attempts represent submitted attempts. The component reads immutable recorded scores and never recalculates correctness from an edited question.

SQL parameter order is $1 UUID quiz_id and $2 UUID user_id for both queries. Raw query functions are internal building blocks and rely on the service's authorization. The service uses one read-only REPEATABLE READ transaction for authorization, history and rank so a concurrent submission cannot produce mismatched counts and ranks within one response.

## Error behavior

| Status | Code | Trigger |
| --- | --- | --- |
| 400 | VALIDATION | Malformed quiz UUID after authentication |
| 401 | UNAUTHENTICATED | Missing verified identity |
| 404 | NOT_FOUND | Missing quiz, wrong team, ineligible member or deleted team |
| 503 | DEPENDENCY_UNAVAILABLE | Database acquisition/query failure |

Errors use `{ "error": { "code": "...", "message": "...", "request_id": "..." } }`. Component responses include x-request-id. This package generates UUID request IDs. During integration reconcile with the team's global request-ID middleware to use one correlation ID throughout. Authorization credentials, request bodies, answer keys and private answers are excluded from tracing. Upstream auth owns token expiration, logout/revocation, credential checks and any 403 policy.
