#!/usr/bin/env python3
"""Run design-schema tests against the exact distributed SQL files."""
import argparse
import os
from pathlib import Path
import subprocess
import uuid

root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument("--emit", action="store_true", help="print the generated SQL")
args = parser.parse_args()
schema = "insights_verify_" + uuid.uuid4().hex
rank_sql = (root / "sql/insights/member_revision_ranks.sql").read_text()
history_sql = (root / "sql/insights/member_attempt_history.sql").read_text()
sql = "BEGIN;\nCREATE SCHEMA " + schema + ";\nSET LOCAL search_path = " + schema + ";\n"
sql += (root / "tests/support/schema.sql").read_text()
sql += (root / "tests/support/fixtures.sql").read_text()
sql += """
CREATE FUNCTION pg_temp.member_revision_ranks(uuid, uuid)
RETURNS TABLE(quiz_revision_id uuid, quiz_revision_number int, best_percentage double precision, rank bigint)
LANGUAGE SQL AS $rank_query$
""" + rank_sql + "\n$rank_query$;\n"
sql += """
CREATE FUNCTION pg_temp.member_attempt_history(uuid, uuid)
RETURNS TABLE(attempt_id uuid, quiz_revision_id uuid, quiz_revision_number int, attempt_number smallint,
score int, total_questions int, percentage double precision, submitted_at timestamptz)
LANGUAGE SQL AS $history_query$
""" + history_sql + "\n$history_query$;\n"
sql += "CREATE FUNCTION pg_temp.member_quiz_access(uuid, uuid) RETURNS boolean LANGUAGE SQL AS $access_query$\n"
sql += (root / "sql/insights/member_quiz_access.sql").read_text()
sql += "\n$access_query$;\n"
sql += (root / "tests/sql/assertions.sql").read_text()
sql += "\nROLLBACK;\n"
if args.emit:
    print(sql)
else:
    url = os.environ.get("INSIGHTS_TEST_DATABASE_URL")
    if not url:
        parser.error("Set INSIGHTS_TEST_DATABASE_URL to your local PostgreSQL test database")
    result = subprocess.run(["psql", "-X", "-v", "ON_ERROR_STOP=1", "--dbname", url], input=sql, text=True)
    raise SystemExit(result.returncode)
