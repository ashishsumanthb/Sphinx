# Integration and Git conflict handling

## Verify the branch before introducing files

Your known repository is /home/technoidentity/sphinx-work and your branch is feat/insights. Run these individually:

```bash
cd /home/technoidentity/sphinx-work
git status -sb
git branch --show-current
git fetch origin
git diff --stat HEAD..origin/main
```

Check your teammate-approved PR base. The old roadmap says developer, while the observed repository used main. Keep feat/insights and use the actual agreed integration branch. If you have uncommitted work, make a focused commit of files you intend to keep before syncing. Inspect untracked files explicitly; git diff does not show their contents.

The lowest-conflict approach is to add this source as a standalone path dependency under components/insights, retaining its internal src/, sql/, tests/ and docs/ layout. Choose a previously unused destination and extract there. If components/insights exists, compare files before copying. Your earlier sql/insights/member_revision_ranks.sql uses the same mathematical policy; this package adds UUID integration, revision number and HTTP support. Compare any existing route before introducing a second registration.

## Shared app changes for peer review

Add one Cargo path dependency named sphinx-insights pointing to components/insights and regenerate the shared lockfile with Cargo. The package is a library. Obtain the pool from the shared state, construct sphinx_insights::app_state::AppState with a pool clone, call sphinx_insights::build_router, and merge the returned router into the host router. These are coordinated edits to the shared manifest, lockfile and route setup.

Have the host's verified auth middleware insert sphinx_insights::api::AuthenticatedUser from its authenticated UUID. This middleware must wrap the mounted results route. The demo adapter stays confined to examples/demo.rs. Startup for the reusable library never invokes the fixture helper, Compose stack or schema DDL.

Confirm the actual migration matches users, teams.deleted_at, team_members.role, quizzes.team_id, quiz_eligibilities, quiz_revisions.revision_no and quiz_attempts columns from the attached Sphinx DDL. Review the current membership/eligibility policy and global request IDs with the team. Run the host's test suite after merging and test real token expiry and team access through the normal login flow.

If the team requires direct source integration, coordinate the design's existing src/repository/analytics.rs, src/services/analytics_service.rs and src/api/handlers/analytics.rs with their owners. Merge functions and DTOs into the existing modules and state/error types. Existing module declarations, manifest and root router require deliberate reconciliation. Retain one canonical copy of each SQL query and update include_str paths if moved.

## Review and push

Stage explicit component paths only. Review the staged diff and run checks before committing. Example commands after you have copied the component to the new destination:

```bash
git add components/insights
git diff --cached --stat
git diff --cached --check
git commit -m "feat(insights): add member results and revision ranking"
git push origin feat/insights
```

Exclude target/, local credentials, downloaded archives and generated demo artifacts. The component's own .gitignore excludes its target folder.

After protecting local work, merge the agreed integration base into feat/insights. If Git reports conflicts, run git status and git diff --name-only --diff-filter=U. Resolve one file at a time, preserve both teammates' required behavior, and ask the owner to review shared router/state changes. Regenerate Cargo.lock through Cargo after reconciling manifest dependencies. Stage resolved files and finish the merge only after format, Clippy, SQL tests and the host's tests pass. Avoid blanket ours/theirs resolution. Stop for explicit confirmation before discarding work, resetting commits or rewriting shared history. An ordinary push should be sufficient.

The download was built independently from the supplied specifications and your pasted ranking query. Current private repository source and team auth changes were unavailable for review. This component is ready for local verification and peer review; compatibility with unseen changes must be checked in the host.
