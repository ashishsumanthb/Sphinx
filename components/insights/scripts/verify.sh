#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
: "${INSIGHTS_TEST_DATABASE_URL:?Set this to a local PostgreSQL test database}"
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
python3 scripts/verify_sql.py
cargo test --locked -- --nocapture
