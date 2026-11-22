# StandupPulse — Participation & Scoring

This branch contains Member 3's end-to-end participation feature: safe quiz
questions, attempt sessions, deterministic scoring, atomic persistence,
idempotent retries, and personal attempt retrieval.

See [`docs/PARTICIPATION_API.md`](docs/PARTICIPATION_API.md) for routes,
security rules, database dependencies, and integration notes.

[`fixtures/participation_flow.json`](fixtures/participation_flow.json) supplies
mock Identity, Quiz Management, request, response, and persisted-attempt data so
this branch can be developed and demonstrated before the upstream branches are
merged.

```bash
cargo test --all-targets
```
