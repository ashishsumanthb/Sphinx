-- $1 quiz_id UUID; $2 authenticated user_id UUID.
-- Rank exact numeric percentages before converting for JSON output.
WITH best_results AS (
    SELECT quiz_revision_id, user_id,
           MAX(score * 100.0 / total_questions) AS best_percentage
    FROM quiz_attempts
    WHERE quiz_id = $1
    GROUP BY quiz_revision_id, user_id
), ranked_results AS (
    SELECT quiz_revision_id, user_id, best_percentage,
           RANK() OVER (
               PARTITION BY quiz_revision_id ORDER BY best_percentage DESC
           ) AS rank
    FROM best_results
)
SELECT r.quiz_revision_id, v.revision_no AS quiz_revision_number,
       r.best_percentage::double precision AS best_percentage, r.rank
FROM ranked_results r
JOIN quiz_revisions v ON v.id = r.quiz_revision_id AND v.quiz_id = $1
WHERE r.user_id = $2
ORDER BY v.revision_no;
