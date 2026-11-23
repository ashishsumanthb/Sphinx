# Host integration

The repository root is the supported integration target. `sphinx-insights` is a
workspace member and a path dependency of `sphinx`. The existing host route in
`src/api/routes.rs` is the only production registration of GET
`/api/v1/quizzes/{quiz_id}/my-results`.

The existing handler obtains `session::AuthUser.id`, calls the participation
service/repository, then calls `sphinx_insights::results::my_results`. That shared
operation checks membership plus eligibility/admin access, reads history and ranks
within one read-only repeatable-read transaction. The host maps its typed errors
into the existing `AppError`; no component HTTP error envelope or Extension
identity is used by the host. Do not merge `build_router` into the host router.
The component router and fictional tokens are retained only for the standalone demo.

The host preserves the existing fields and adds `revision_results` (same revision
DTO as the component). All summary fields use the caller's highest attempted
`revision_no`, not timestamps and not the quiz's highest unattempted revision.
The best attempt within that revision is selected by exact score/total ratio;
equal ratios prefer the lowest attempt number, then UUID. An authorized caller
with no attempts receives null summary fields and empty arrays. Ranking uses all
participants' best percentages within each revision, with competition ranks
1, 1, 3. Member access requires current team membership and quiz eligibility;
current team admins can retrieve their own results without eligibility.
Inaccessible/missing quizzes return the host's 404 NOT_FOUND envelope.

`migrations/0020_quiz_schema.sql` supplies the documented quiz prerequisite tables
that migration 0030 already references. Identity/session/team definitions stay
owned by migrations 001/002. Coordinate this new migration with the quiz owner
before merging; if their equivalent migration lands, reconcile them rather than
creating the tables twice. No existing migration is modified.

Both component tests and host integration tests apply the real root migrations.
No private identity/team schema is maintained. Component Cargo.lock is historical;
the root workspace Cargo.lock is authoritative.

## Verification

From the repository root on NixOS:

    nix-shell -p python3 postgresql --run 'bash scripts/verify_insights_local.sh'

The launcher creates and stops only its temporary PostgreSQL cluster. The new
host test uses an additional UUID-named schema, real login requests and real
session extraction. Existing destructive ignored PostgreSQL tests are not invoked.
With an already isolated test database, set INSIGHTS_TEST_DATABASE_URL and run:

    bash scripts/verify_insights.sh

CI `.github/workflows/insights.yml` runs the host and component checks on PostgreSQL
17. The host DB test is explicitly run with `--ignored`; a normal successful
root test run alone is not evidence that this database test passed.
