-- Member 3: Participation & Scoring.
-- Requires the shared users, quizzes, quiz_revisions, question_versions and
-- quiz_eligibilities tables to exist first.

CREATE TABLE attempt_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    quiz_id UUID NOT NULL REFERENCES quizzes(id),
    user_id UUID NOT NULL REFERENCES users(id),
    quiz_revision_id UUID NOT NULL,
    status TEXT NOT NULL DEFAULT 'in_progress'
        CHECK (status IN ('in_progress', 'submitted', 'expired')),
    started_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    FOREIGN KEY (quiz_id, user_id)
        REFERENCES quiz_eligibilities(quiz_id, user_id),
    FOREIGN KEY (quiz_id, quiz_revision_id)
        REFERENCES quiz_revisions(quiz_id, id),
    UNIQUE (id, quiz_id, quiz_revision_id)
);

CREATE UNIQUE INDEX uq_participation_active_session
    ON attempt_sessions(quiz_id, user_id)
    WHERE status = 'in_progress';

CREATE TABLE quiz_attempts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL UNIQUE REFERENCES attempt_sessions(id),
    quiz_id UUID NOT NULL REFERENCES quizzes(id),
    user_id UUID NOT NULL REFERENCES users(id),
    quiz_revision_id UUID NOT NULL,
    attempt_no SMALLINT NOT NULL CHECK (attempt_no IN (1, 2)),
    request_hash CHAR(64) NOT NULL,
    score INT NOT NULL CHECK (score >= 0),
    total_questions INT NOT NULL CHECK (total_questions BETWEEN 1 AND 50),
    submitted_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    UNIQUE (quiz_id, user_id, attempt_no),
    UNIQUE (id, quiz_revision_id),
    FOREIGN KEY (quiz_id, quiz_revision_id)
        REFERENCES quiz_revisions(quiz_id, id),
    FOREIGN KEY (quiz_id, user_id)
        REFERENCES quiz_eligibilities(quiz_id, user_id),
    CHECK (score <= total_questions)
);

CREATE TABLE attempt_answers (
    attempt_id UUID NOT NULL,
    quiz_revision_id UUID NOT NULL,
    question_version_id UUID NOT NULL,
    selected_option_index INT NOT NULL CHECK (selected_option_index >= 0),
    is_correct BOOLEAN NOT NULL,
    PRIMARY KEY (attempt_id, question_version_id),
    FOREIGN KEY (attempt_id, quiz_revision_id)
        REFERENCES quiz_attempts(id, quiz_revision_id),
    FOREIGN KEY (quiz_revision_id, question_version_id)
        REFERENCES question_versions(quiz_revision_id, id)
);

CREATE INDEX idx_participation_attempts_user
    ON quiz_attempts(user_id, quiz_id);
