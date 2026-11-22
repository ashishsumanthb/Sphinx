DO $checks$
DECLARE
    q uuid := '00000000-0000-4000-8000-000000000100';
    a uuid := '00000000-0000-4000-8000-000000000001';
    b uuid := '00000000-0000-4000-8000-000000000002';
    c uuid := '00000000-0000-4000-8000-000000000003';
    d uuid := '00000000-0000-4000-8000-000000000004';
    z uuid := '00000000-0000-4000-8000-000000000005';
BEGIN
    IF (SELECT rank FROM pg_temp.member_revision_ranks(q,a)) IS DISTINCT FROM 1::bigint
       OR (SELECT best_percentage FROM pg_temp.member_revision_ranks(q,a)) IS DISTINCT FROM 90::double precision
       OR (SELECT rank FROM pg_temp.member_revision_ranks(q,b)) IS DISTINCT FROM 1::bigint
       OR (SELECT rank FROM pg_temp.member_revision_ranks(q,c)) IS DISTINCT FROM 3::bigint THEN
        RAISE EXCEPTION 'Best-of-two or competition ranking failed';
    END IF;
    IF (SELECT count(*) FROM pg_temp.member_attempt_history(q,a)) <> 2
       OR (SELECT array_agg(attempt_number ORDER BY attempt_number) FROM pg_temp.member_attempt_history(q,a)) <> ARRAY[1,2]::smallint[] THEN
        RAISE EXCEPTION 'Attempt history failed';
    END IF;
    IF (SELECT count(*) FROM pg_temp.member_attempt_history(q,z)) <> 0
       OR (SELECT count(*) FROM pg_temp.member_revision_ranks(q,z)) <> 0 THEN
        RAISE EXCEPTION 'Empty history failed';
    END IF;
    IF (SELECT quiz_revision_number FROM pg_temp.member_revision_ranks(q,d)) IS DISTINCT FROM 2
       OR (SELECT rank FROM pg_temp.member_revision_ranks(q,d)) IS DISTINCT FROM 1::bigint THEN
        RAISE EXCEPTION 'Revision isolation failed';
    END IF;
    IF ABS((SELECT best_percentage FROM pg_temp.member_revision_ranks('00000000-0000-4000-8000-000000000101',a)) - 66.66666666666667) > 0.00000001
       OR (SELECT rank FROM pg_temp.member_revision_ranks('00000000-0000-4000-8000-000000000101',a)) IS DISTINCT FROM 1::bigint
       OR (SELECT best_percentage FROM pg_temp.member_revision_ranks('00000000-0000-4000-8000-000000000101',c)) IS DISTINCT FROM 0::double precision THEN
        RAISE EXCEPTION 'Fraction, quiz isolation or zero score failed';
    END IF;
    IF (SELECT count(*) FROM pg_temp.member_revision_ranks('00000000-0000-4000-8000-000000000102',a)) <> 0 THEN
        RAISE EXCEPTION 'Empty quiz failed';
    END IF;
    IF NOT pg_temp.member_quiz_access(q,a)
       OR pg_temp.member_quiz_access(q,'00000000-0000-4000-8000-000000000006')
       OR pg_temp.member_quiz_access(q,'00000000-0000-4000-8000-000000000007')
       OR pg_temp.member_quiz_access('00000000-0000-4000-8000-000000000999',a) THEN
        RAISE EXCEPTION 'Member access policy failed';
    END IF;
    UPDATE team_members SET role = 'admin' WHERE user_id = '00000000-0000-4000-8000-000000000007';
    IF NOT pg_temp.member_quiz_access(q,'00000000-0000-4000-8000-000000000007') THEN
        RAISE EXCEPTION 'Current team admin should be allowed';
    END IF;
    DELETE FROM team_members WHERE user_id = c;
    IF pg_temp.member_quiz_access(q,c) THEN
        RAISE EXCEPTION 'Removed membership should be denied';
    END IF;
    UPDATE teams SET deleted_at = clock_timestamp();
    IF pg_temp.member_quiz_access(q,a) THEN
        RAISE EXCEPTION 'Deleted team should be denied';
    END IF;
    RAISE NOTICE 'PASS: actual query files, UUID design schema, best-of-two, ties, revision/quiz isolation, fractions, zero scores, empty results';
END;
$checks$;

SELECT 'PASS: SQL assertions completed' AS verification;
