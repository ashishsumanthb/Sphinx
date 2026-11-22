-- $1 quiz_id UUID; $2 authenticated user_id UUID.
SELECT a.id AS attempt_id, a.quiz_revision_id,
       v.revision_no AS quiz_revision_number,
       a.attempt_no AS attempt_number, a.score, a.total_questions,
       (a.score * 100.0 / a.total_questions)::double precision AS percentage,
       a.submitted_at
FROM quiz_attempts a
JOIN quiz_revisions v ON v.id = a.quiz_revision_id AND v.quiz_id = a.quiz_id
WHERE a.quiz_id = $1 AND a.user_id = $2
ORDER BY a.attempt_no, a.submitted_at, a.id;
