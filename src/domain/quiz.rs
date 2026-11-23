#[cfg(test)]
#[derive(serde::Serialize)]
pub(crate) struct PublicQuestion {
    pub(crate) id: uuid::Uuid,
    pub(crate) position: usize,
    pub(crate) text: String,
    pub(crate) options: Vec<String>,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum QuizStatus {
    Draft,
    Open,
    Closed,
}
pub(crate) fn is_quiz_open(
    status: QuizStatus,
    now: i64,
    opens_at: i64,
    closes_at: i64,
    manually_closed_at: Option<i64>,
) -> bool {
    status == QuizStatus::Open
        && manually_closed_at.is_none()
        && super::timing::is_within_session_window(now, opens_at, closes_at)
}
#[cfg(test)]
pub(crate) fn has_attempt_remaining(completed_attempts: u8, max_attempts: u8) -> bool {
    completed_attempts < max_attempts
}
#[cfg(test)]
pub(crate) fn can_start(
    is_eligible: bool,
    quiz_is_open: bool,
    completed_attempts: u8,
    max_attempts: u8,
) -> bool {
    is_eligible && quiz_is_open && has_attempt_remaining(completed_attempts, max_attempts)
}
#[cfg(test)]
pub(crate) fn can_submit(
    is_eligible: bool,
    quiz_is_open: bool,
    completed_attempts: u8,
    max_attempts: u8,
    has_active_session: bool,
) -> bool {
    has_active_session && can_start(is_eligible, quiz_is_open, completed_attempts, max_attempts)
}
pub(crate) struct QuizRevision {
    pub(crate) id: uuid::Uuid,
    pub(crate) quiz_id: uuid::Uuid,
    pub(crate) revision_no: i32,
    pub(crate) created_by: uuid::Uuid,
    pub(crate) change_reason: String,
    #[allow(dead_code)]
    pub(crate) created_at: i64,
}
#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SessionStatus {
    InProgress,
    Submitted,
    #[allow(dead_code)]
    Expired,
}
#[cfg(test)]
pub(crate) struct AttemptSession {
    pub(crate) id: uuid::Uuid,
    pub(crate) user_id: uuid::Uuid,
    pub(crate) quiz_id: uuid::Uuid,
    pub(crate) quiz_revision_id: uuid::Uuid,
    pub(crate) status: SessionStatus,
    pub(crate) started_at: i64,
}
#[cfg(test)]
impl AttemptSession {
    pub(crate) fn new(
        id: uuid::Uuid,
        user_id: uuid::Uuid,
        revision: &QuizRevision,
        started_at: i64,
    ) -> Self {
        Self {
            id,
            user_id,
            quiz_id: revision.quiz_id,
            quiz_revision_id: revision.id,
            status: SessionStatus::InProgress,
            started_at,
        }
    }
}
pub(crate) struct Quiz {
    #[allow(dead_code)]
    pub(crate) id: uuid::Uuid,
    #[allow(dead_code)]
    pub(crate) team_id: uuid::Uuid,
    #[allow(dead_code)]
    pub(crate) title: String,
    pub(crate) status: QuizStatus,
    pub(crate) starts_at: Option<i64>,
    pub(crate) ends_at: Option<i64>,
    pub(crate) manually_closed_at: Option<i64>,
    pub(crate) max_attempts: u8,
    #[allow(dead_code)]
    pub(crate) created_by: uuid::Uuid,
    #[allow(dead_code)]
    pub(crate) description: Option<String>,
    #[allow(dead_code)]
    pub(crate) created_at: i64,
}

impl Quiz {
    pub(crate) fn is_open_at(&self, now: i64) -> bool {
        match (self.starts_at, self.ends_at) {
            (Some(start), Some(end)) => {
                is_quiz_open(self.status, now, start, end, self.manually_closed_at)
            }
            _ => false,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::{can_start, can_submit, has_attempt_remaining};
    #[test]
    fn enforces_one_attempt_limit() {
        assert!(has_attempt_remaining(0, 1));
        assert!(!has_attempt_remaining(1, 1));
    }
    #[test]
    fn enforces_two_attempt_limit() {
        assert!(has_attempt_remaining(1, 2));
        assert!(!has_attempt_remaining(2, 2));
    }
    #[test]
    fn starting_requires_eligibility_an_open_quiz_and_an_attempt_left() {
        assert!(can_start(true, true, 1, 2));
        assert!(!can_start(false, true, 1, 2));
        assert!(!can_start(true, false, 1, 2));
        assert!(!can_start(true, true, 2, 2));
    }
    #[test]
    fn submission_requires_an_active_session_and_an_open_quiz() {
        assert!(can_submit(true, true, 0, 1, true));
        assert!(!can_submit(true, false, 0, 1, true));
        assert!(!can_submit(true, true, 0, 1, false));
    }
    #[test]
    fn public_question_serializes_only_participant_fields() {
        let id = uuid::Uuid::from_u128(1);
        let question = super::PublicQuestion {
            id,
            position: 0,
            text: String::from("Which HTTP status means Created?"),
            options: vec![String::from("200 OK"), String::from("201 Created")],
        };
        let actual = serde_json::to_value(&question).unwrap();
        assert_eq!(
            actual,
            serde_json::json!({
                "id":id.to_string(),
                "position":0,
                "text": "Which HTTP status means Created?",
                "options":["200 OK","201 Created"]
            })
        );
    }
    #[test]
    fn quiz_must_be_open_and_within_its_window() {
        use super::{QuizStatus, is_quiz_open};

        // The window runs from second 0 up to second 900.
        assert!(is_quiz_open(QuizStatus::Open, 300, 0, 900, None));
        assert!(!is_quiz_open(QuizStatus::Draft, 300, 0, 900, None));
        assert!(!is_quiz_open(QuizStatus::Closed, 300, 0, 900, None));
        assert!(!is_quiz_open(QuizStatus::Open, 300, 0, 900, Some(240)));
        assert!(!is_quiz_open(QuizStatus::Open, 900, 0, 900, None));
    }
    #[test]
    fn draft_without_a_schedule_is_closed() {
        use super::{Quiz, QuizStatus};

        let quiz = Quiz {
            id: uuid::Uuid::from_u128(1),
            team_id: uuid::Uuid::from_u128(2),
            title: String::from("HTTP basics"),
            status: QuizStatus::Draft,
            starts_at: None,
            ends_at: None,
            manually_closed_at: None,
            max_attempts: 2,
            created_by: uuid::Uuid::from_u128(3),
            description: None,
            created_at: 0,
        };

        assert!(!quiz.is_open_at(300));
    }
    #[test]
    fn sessions_keep_the_revision_they_started_with() {
        use super::{AttemptSession, QuizRevision, SessionStatus};
        use uuid::Uuid;

        let quiz_id = Uuid::from_u128(1);
        let admin_id = Uuid::from_u128(2);
        let first_revision = QuizRevision {
            id: Uuid::from_u128(10),
            quiz_id,
            revision_no: 1,
            created_by: admin_id,
            change_reason: String::from("Initial publication"),
            created_at: 100,
        };

        let first_session = AttemptSession::new(
            Uuid::from_u128(20),
            Uuid::from_u128(30),
            &first_revision,
            110,
        );

        let second_revision = QuizRevision {
            id: Uuid::from_u128(11),
            quiz_id,
            revision_no: 2,
            created_by: admin_id,
            change_reason: String::from("Corrected the answer key"),
            created_at: 120,
        };

        let second_session = AttemptSession::new(
            Uuid::from_u128(21),
            Uuid::from_u128(31),
            &second_revision,
            130,
        );

        assert_eq!(first_session.status, SessionStatus::InProgress);
        assert_eq!(first_session.quiz_id, second_session.quiz_id);
        assert_eq!(first_session.quiz_revision_id, first_revision.id);
        assert_eq!(second_session.quiz_revision_id, second_revision.id);
    }
}
