-- $1 quiz_id UUID; $2 authenticated user_id UUID.
SELECT EXISTS (
    SELECT 1 FROM quizzes q
    JOIN teams t ON t.id = q.team_id
    JOIN team_memberships m ON m.team_id = q.team_id AND m.user_id = $2
    WHERE q.id = $1
      AND (m.role = 'admin' OR EXISTS (
          SELECT 1 FROM quiz_eligibilities e
          WHERE e.quiz_id = q.id AND e.user_id = $2
      ))
);
