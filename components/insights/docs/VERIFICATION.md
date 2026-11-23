# Integration verification

Prepared against developer d7445f9c7b78016b376a09204b01b075c756f699 and
feat/insights b535a0d322d097a4d7695d11865b73ae1e8992f5.

- Root cargo test --all-targets: 23 passed, 3 database tests ignored.
- Component API tests: 2 passed.
- Canonical SQL assertions on PostgreSQL 16.2 single-user backend: passed with
  real migrations. Single-user execution was necessary because this build
  environment forbids TCP and Unix sockets.
- New shared-schema host integration test: compiled, not executed here because
  PostgreSQL sockets are unavailable. It must pass locally or in PostgreSQL 17 CI.

The new host test exercises real login/session extraction, eligible member,
admin without eligibility, outsider, ineligible member, missing quiz, no attempts,
competition ties, fractions/zero, revision-number ordering, summary consistency,
caller identity isolation, membership removal and logout.

The local launcher runs this test explicitly and never runs the older ignored
tests that reset the public schema. No remote branch has been pushed by this build.

- Workspace Clippy: passed (`cargo clippy --locked --workspace --all-targets -- -D warnings`); existing unused async-trait manifest warning remains.
