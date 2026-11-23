CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL CHECK (char_length(trim(name)) BETWEEN 1 AND 100),
    email TEXT NOT NULL CHECK (
        email = lower(trim(email))
        AND char_length(email) BETWEEN 3 AND 254
    ),
    password_encoded TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT users_email_unique UNIQUE (email)
);

CREATE TABLE login_sessions (
    token_hash BYTEA PRIMARY KEY CHECK (octet_length(token_hash) = 32),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL,

    CONSTRAINT login_sessions_valid_expiry CHECK (expires_at > created_at)
);

CREATE INDEX login_sessions_user_id_idx
    ON login_sessions (user_id);

CREATE INDEX login_sessions_expires_at_idx
    ON login_sessions (expires_at);

CREATE TABLE teams (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL CHECK (char_length(trim(name)) BETWEEN 1 AND 100),
    created_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE team_memberships (
    team_id UUID NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role TEXT NOT NULL DEFAULT 'member'
        CHECK (role IN ('admin', 'member')),
    joined_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    PRIMARY KEY (team_id, user_id)
);

CREATE INDEX team_memberships_user_id_idx
    ON team_memberships (user_id);
CREATE TABLE quizzes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    team_id UUID NOT NULL REFERENCES teams(id),
    created_by UUID NOT NULL REFERENCES users(id),
    title VARCHAR(150) NOT NULL,
    description TEXT,
    status TEXT NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft', 'open', 'closed')),
    starts_at TIMESTAMPTZ,
    ends_at TIMESTAMPTZ,
    manually_closed_at TIMESTAMPTZ,
    max_attempts SMALLINT NOT NULL DEFAULT 1 CHECK (max_attempts IN (1, 2)),
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    CHECK (starts_at IS NULL OR ends_at IS NULL OR ends_at > starts_at),
    CHECK (status = 'draft' OR (starts_at IS NOT NULL AND ends_at IS NOT NULL))
);

CREATE TABLE draft_questions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    quiz_id UUID NOT NULL REFERENCES quizzes(id),
    stable_question_id UUID NOT NULL DEFAULT gen_random_uuid(),
    position INT NOT NULL CHECK (position >= 0),
    question_text TEXT NOT NULL CHECK (length(trim(question_text)) > 0),
    options JSONB NOT NULL,
    correct_option_index INT NOT NULL,
    explanation TEXT NOT NULL DEFAULT '',
    UNIQUE (quiz_id, stable_question_id),
    UNIQUE (quiz_id, position),
    CHECK (jsonb_typeof(options) = 'array'),
    CHECK (jsonb_array_length(options) BETWEEN 2 AND 6),
    CHECK (correct_option_index >= 0
        AND correct_option_index < jsonb_array_length(options))
);

CREATE TABLE quiz_revisions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    quiz_id UUID NOT NULL REFERENCES quizzes(id),
    revision_no INT NOT NULL CHECK (revision_no > 0),
    created_by UUID NOT NULL REFERENCES users(id),
    change_reason TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    UNIQUE (quiz_id, revision_no),
    UNIQUE (quiz_id, id)
);

CREATE TABLE question_versions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    quiz_revision_id UUID NOT NULL REFERENCES quiz_revisions(id),
    stable_question_id UUID NOT NULL,
    position INT NOT NULL CHECK (position >= 0),
    question_text TEXT NOT NULL CHECK (length(trim(question_text)) > 0),
    options JSONB NOT NULL,
    correct_option_index INT NOT NULL,
    explanation TEXT NOT NULL DEFAULT '',
    UNIQUE (quiz_revision_id, position),
    UNIQUE (quiz_revision_id, stable_question_id),
    UNIQUE (quiz_revision_id, id),
    CHECK (jsonb_typeof(options) = 'array'),
    CHECK (jsonb_array_length(options) BETWEEN 2 AND 6),
    CHECK (correct_option_index >= 0
        AND correct_option_index < jsonb_array_length(options))
);

CREATE TABLE quiz_eligibilities (
    quiz_id UUID NOT NULL REFERENCES quizzes(id),
    user_id UUID NOT NULL REFERENCES users(id),
    PRIMARY KEY (quiz_id, user_id)
);

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

CREATE UNIQUE INDEX uq_active_session_per_user_quiz
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

CREATE TABLE tags (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    team_id UUID NOT NULL REFERENCES teams(id),
    name VARCHAR(50) NOT NULL,
    UNIQUE (team_id, name)
);

CREATE TABLE question_tags (
    question_version_id UUID NOT NULL REFERENCES question_versions(id),
    tag_id UUID NOT NULL REFERENCES tags(id),
    PRIMARY KEY (question_version_id, tag_id)
);

CREATE TABLE quiz_revision_audit (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    quiz_id UUID NOT NULL REFERENCES quizzes(id),
    revision_id UUID NOT NULL REFERENCES quiz_revisions(id),
    editor_id UUID NOT NULL REFERENCES users(id),
    action TEXT NOT NULL,
    reason TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);

CREATE INDEX idx_quizzes_team_status ON quizzes(team_id, status);
CREATE INDEX idx_quizzes_window ON quizzes(starts_at, ends_at);
CREATE INDEX idx_questions_revision ON question_versions(quiz_revision_id, position);
CREATE INDEX idx_sessions_quiz_user ON attempt_sessions(quiz_id, user_id);
CREATE INDEX idx_attempts_quiz_user ON quiz_attempts(quiz_id, user_id);
CREATE INDEX idx_attempts_revision ON quiz_attempts(quiz_revision_id);
CREATE INDEX idx_attempts_submitted_at ON quiz_attempts(submitted_at);
