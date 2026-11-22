# StandupPulse — Participation & Scoring

This branch contains Member 3's end-to-end participation feature: safe quiz
questions, attempt sessions, deterministic scoring, atomic persistence,
idempotent retries, and personal attempt retrieval.

See [`docs/PARTICIPATION_API.md`](docs/PARTICIPATION_API.md) for routes,
security rules, database dependencies, and integration notes.

```bash
cargo test --all-targets
```
