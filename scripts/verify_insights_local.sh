#!/usr/bin/env bash
# Run from a Nix shell containing Python 3, PostgreSQL and your existing Rust tools.
set -euo pipefail

project_dir="$(cd "$(dirname "$0")/.." && pwd)"
for required_command in python3 initdb pg_ctl createdb psql cargo; do
    if ! command -v "$required_command" >/dev/null 2>&1; then
        printf 'Missing tool: %s. Run this launcher inside nix-shell -p python3 postgresql.\n' "$required_command" >&2
        exit 1
    fi
done
if [[ ! -f "$project_dir/Cargo.toml" || ! -f "$project_dir/scripts/verify_insights.sh" ]]; then
    printf 'Repository files missing: %s\nPass its path as the first argument.\n' "$project_dir" >&2
    exit 1
fi
cd "$project_dir"
umask 077
runtime_dir=$(mktemp -d /tmp/sphinx-demo.XXXXXX)
data_dir="$runtime_dir/data"
password_file="$runtime_dir/database-password"
postgres_started=0
cleanup() {
    exit_status=$?
    trap - EXIT INT TERM
    if [[ "$postgres_started" == 1 ]]; then
        if ! pg_ctl -D "$data_dir" -m fast -w stop; then
            printf 'Database stop failed. Inspect %s/server.log\n' "$runtime_dir" >&2
        fi
    fi
    printf '\nDemo files retained at %s\n' "$runtime_dir"
    exit "$exit_status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

python3 - "$password_file" <<'PY'
import secrets
import sys
from pathlib import Path
Path(sys.argv[1]).write_text(secrets.token_urlsafe(32) + '\n')
PY
initdb -D "$data_dir" -U sphinx_demo --auth=scram-sha-256 \
    --pwfile="$password_file" --no-locale --encoding=UTF8 > "$runtime_dir/initdb.log" 2>&1 || {
    cat "$runtime_dir/initdb.log" >&2
    exit 1
}
# Select an available loopback port. If another process takes it before startup,
# PostgreSQL fails startup and the launcher stops without interrupting that process.
database_port=$(python3 - <<'PY'
import socket
with socket.socket() as listener:
    listener.bind(('127.0.0.1', 0))
    print(listener.getsockname()[1])
PY
)
if ! pg_ctl -D "$data_dir" -l "$runtime_dir/server.log" \
    -o "-h 127.0.0.1 -p $database_port -k $runtime_dir" -w start; then
    # Handle a partially completed startup using only this launcher's data directory.
    if pg_ctl -D "$data_dir" status >/dev/null 2>&1; then
        postgres_started=1
    fi
    cat "$runtime_dir/server.log" >&2
    exit 1
fi
postgres_started=1
PGPASSWORD="$(< "$password_file")" createdb -h 127.0.0.1 -p "$database_port" -U sphinx_demo sphinx
TEST_DATABASE_URL=$(python3 - "$password_file" "$database_port" <<'PY'
from pathlib import Path
from urllib.parse import quote
import sys
password = Path(sys.argv[1]).read_text().strip()
print(f'postgres://sphinx_demo:{quote(password, safe="")}@127.0.0.1:{sys.argv[2]}/sphinx')
PY
)
export TEST_DATABASE_URL
printf '\nRunning host verification against this isolated database...\n'
bash scripts/verify_insights.sh
printf '\nHost verification passed.\n'
