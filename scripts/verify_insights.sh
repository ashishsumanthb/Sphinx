#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
: "${TEST_DATABASE_URL:?Set TEST_DATABASE_URL to an isolated PostgreSQL database; suites reset public}"
cargo test --locked --all-targets
cargo test --locked --test insights_host --test insights_postgres \
  --test postgres_attempt_limits --test postgres_edge_cases \
  -- --ignored --test-threads=1 --nocapture
