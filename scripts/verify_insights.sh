#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
: "${INSIGHTS_TEST_DATABASE_URL:?Set INSIGHTS_TEST_DATABASE_URL to a local test PostgreSQL database}"
cargo test --locked --all-targets
cargo test --locked --test insights_host -- --ignored --nocapture
cargo test --locked -p sphinx-insights --all-targets
python3 components/insights/scripts/verify_sql.py
