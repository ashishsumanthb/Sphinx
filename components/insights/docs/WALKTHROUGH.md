# Peer walkthrough

Allow about 8 minutes. Run commands from the extracted component folder. Start the database and API using README.md first.

1. Explain the data path. Open sql/insights/member_attempt_history.sql, then member_revision_ranks.sql. Show that the two input parameters are quiz_id and authenticated user_id. Explain grouping, MAX and RANK, then the caller filter. A has 70% and 90%, B has 90%, C has 80%. Their ranks are 1,1,3.

2. Demonstrate A's result.

```bash
curl -sS -H 'Authorization: Bearer demo-a' http://127.0.0.1:8000/api/v1/quizzes/00000000-0000-4000-8000-000000000100/my-results | python3 -m json.tool
```

Point out both attempt IDs, timestamp, percentages and the single revision summary. Explain that previous recorded scores survive question revisions.

3. Demonstrate tie, revision separation and empty history.

```bash
curl -sS -H 'Authorization: Bearer demo-c' http://127.0.0.1:8000/api/v1/quizzes/00000000-0000-4000-8000-000000000100/my-results | python3 -m json.tool
curl -sS -H 'Authorization: Bearer demo-d' http://127.0.0.1:8000/api/v1/quizzes/00000000-0000-4000-8000-000000000100/my-results | python3 -m json.tool
curl -sS -H 'Authorization: Bearer demo-z' http://127.0.0.1:8000/api/v1/quizzes/00000000-0000-4000-8000-000000000100/my-results | python3 -m json.tool
```

C ranks 3. D ranks 1 on revision 2. Z receives empty lists. Responses expose only the caller's attempts and rank, with no other member's private score list.

4. Demonstrate denied access.

```bash
curl -i http://127.0.0.1:8000/api/v1/quizzes/00000000-0000-4000-8000-000000000100/my-results
curl -i -H 'Authorization: Bearer demo-outsider' http://127.0.0.1:8000/api/v1/quizzes/00000000-0000-4000-8000-000000000100/my-results
```

Expect 401 and 404 respectively. Explain that real authentication belongs to the host. Show the fictional mapping in examples/demo.rs and the trusted extension boundary in src/api/mod.rs.

5. Demonstrate verification.

```bash
python3 scripts/verify_http.py
export INSIGHTS_TEST_DATABASE_URL='postgres://sphinx_demo:local-demo-password@127.0.0.1:55432/sphinx'
bash scripts/verify.sh
```

Explain that the SQL runner reads the actual query files and creates/rolls back a test schema. The Rust test exercises the HTTP router, SQLx and access control together against real PostgreSQL. Tests also cover fractions (2/3 outranks 33/50), removed membership, deleted teams and safe dependency failure.

6. Explain integration handoffs. Participation owns valid submitted quiz_attempts, sessions and immutable revision IDs. Auth owns verified UUID identity. Database owner reviews schema compatibility. The shared API owner mounts the router once. Show docs/INTEGRATION.md and clearly report any local check still failing before calling integration complete.
