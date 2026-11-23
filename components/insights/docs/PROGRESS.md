# Insights integration checklist

- [x] Use shared team_memberships and current eligibility/admin policy.
- [x] Replace private schema definitions with real migration loading.
- [x] Keep component as a workspace member and host path dependency.
- [x] Reuse component core logic through the existing host route and session extractor.
- [x] Keep host AppError and avoid mounting another router.
- [x] Align all summary fields to highest attempted revision_no; add revision_results.
- [x] Add host test for actual login, authorization, empty results, revisions and ties.
- [x] Add explicit PostgreSQL CI/local verification commands.
- [ ] Run the socket-backed host and component database tests locally/CI.
- [ ] Confirm additive response contract and prerequisite quiz migration with teammates.
- [ ] Review, commit and push integration to feat/insights.
