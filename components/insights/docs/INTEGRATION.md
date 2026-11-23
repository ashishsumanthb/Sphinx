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

The host preserves the existing fields and adds `revision_results` using the same
revision DTO as the component. All summary fields use the caller's highest
attempted `revision_no`, rather than submission timestamps or the quiz's highest
unattempted revision.

The best attempt within that revision is selected by exact score/total ratio.
Equal ratios prefer the lowest attempt number, then UUID. An authorized caller
with no attempts receives null summary fields and empty arrays.

Ranking uses all participants' best percentages within each revision, with
competition ranks such as 1, 1, 3. Member access requires current team membership
and quiz eligibility. Current team admins can retrieve their own results without
eligibility. Inaccessible or missing quizzes return the host's 404 `NOT_FOUND`
envelope.

## Shared migrations

The host and component tests reuse the existing shared
`migrations/0020_quiz_management.sql` from `developer`. The duplicate Insights
migration has been removed. Identity/session/team definitions remain owned by
migrations 001/002. No existing shared migration is changed.

Both component tests and host integration tests apply the real root migrations.
No private identity/team schema is maintained. The root workspace `Cargo.lock`
is authoritative; the component's historical lockfile is not used by workspace
commands.

The SQL verification script discovers migration files from the repository-root
`migrations/` directory. The component test helper explicitly includes the shared
quiz-management migration.

## Verification

Run verification after merging the latest `developer` into `feat/insights` so
the checks cover the integrated application and shared migrations.

From the repository root on NixOS:

```bash
nix-shell -p python3 postgresql --run 'bash scripts/verify_insights_local.sh'
```

The launcher creates and stops only its temporary PostgreSQL cluster. The host
acceptance test uses an additional UUID-named schema, real login requests and
the shared session extractor. Existing destructive ignored PostgreSQL tests are
not invoked by this launcher.

With an already isolated test database, set `INSIGHTS_TEST_DATABASE_URL` and run:

```bash
bash scripts/verify_insights.sh
```

Run the root build and lint checks from the repository root:

```bash
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
```

The host database acceptance test is explicitly run with `--ignored` by
`scripts/verify_insights.sh`. A successful default root test run alone is not
evidence that this database test passed.

CI `.github/workflows/insights.yml` runs the host and component checks on
PostgreSQL 17. Confirm that CI results correspond to the latest PR head after
the shared-migration correction is pushed.

## Final PR review

After committing the migration correction, inspect the final PR diff:

```bash
git diff --stat origin/developer...HEAD
git diff --name-status origin/developer...HEAD
git diff --check origin/developer...HEAD
```

Confirm that the shared application files remain intact, the shared
quiz-management migration is unchanged, and no duplicate quiz-table migration
is added by this PR.
