use crate::domain::quiz_validation::{validate_quiz, QuizDraft};

#[derive(Clone)]
pub(crate) struct DraftItem {
    pub(crate) stable_id: uuid::Uuid,
    pub(crate) question: crate::domain::question_validation::QuestionDraft,
    pub(crate) explanation: Option<String>,
}
#[derive(Clone)]
pub(crate) struct VersionedItem {
    pub(crate) id: uuid::Uuid,
    pub(crate) position: usize,
    pub(crate) item: DraftItem,
}
pub(crate) struct Snapshot {
    pub(crate) revision: crate::domain::quiz::QuizRevision,
    pub(crate) questions: Vec<VersionedItem>,
}
pub(crate) fn build_snapshot(
    revision: crate::domain::quiz::QuizRevision,
    draft_items: &[DraftItem],
) -> Snapshot {
    let mut questions = Vec::with_capacity(draft_items.len());

    for (position, item) in draft_items.iter().enumerate() {
        questions.push(VersionedItem {
            id: uuid::Uuid::new_v4(),
            position,
            item: item.clone(),
        });
    }

    Snapshot { revision, questions }
}
pub(crate) fn prepare_quiz_snapshot(
    revision: crate::domain::quiz::QuizRevision,
    draft_items: &[DraftItem],
    max_attempts: u8,
    starts_at: i64,
    duration_minutes: Option<u32>,
) -> Result<(Snapshot, i64), &'static str> {
    let questions = draft_items
        .iter()
        .map(|item| item.question.clone())
        .collect();

    let draft = crate::domain::quiz_validation::QuizDraft::new(questions, max_attempts);
    let closes_at = prepare_publication(&draft, starts_at, duration_minutes)?;
    let snapshot = build_snapshot(revision, draft_items);

    Ok((snapshot, closes_at))
}
pub(crate) fn revise_question(
    previous: &Snapshot,
    stable_id: uuid::Uuid,
    replacement: crate::domain::question_validation::QuestionDraft,
    explanation: Option<String>,
    changed_by: uuid::Uuid,
    changed_at: i64,
    reason: &str,
) -> Result<Snapshot, &'static str> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err("A revision requires a change reason");
    }

    let mut items: Vec<DraftItem> = previous
        .questions
        .iter()
        .map(|version| version.item.clone())
        .collect();

    let item = items
        .iter_mut()
        .find(|item| item.stable_id == stable_id)
        .ok_or("Question does not belong to this revision")?;

    crate::domain::question_validation::validate_question(&replacement)?;
    item.question = replacement;
    item.explanation = explanation;

    let revision_no = previous.revision.revision_no
        .checked_add(1)
        .ok_or("Quiz revision number is out of range")?;

    let revision = crate::domain::quiz::QuizRevision {
        id: uuid::Uuid::new_v4(),
        quiz_id: previous.revision.quiz_id,
        revision_no,
        created_by: changed_by,
        change_reason: reason.to_owned().into(),
        created_at: changed_at,
    };

    Ok(build_snapshot(revision, &items))
}
pub(crate) fn close_quiz(
    quiz: &mut crate::domain::quiz::Quiz,
    now: i64,
) -> Result<(), &'static str> {
    use crate::domain::quiz::QuizStatus;

    if matches!(quiz.status, QuizStatus::Closed) {
        return Ok(());
    }

    if !quiz.is_open_at(now) {
        return Err("Only an open quiz can be closed manually");
    }

    quiz.manually_closed_at = Some(now);
    quiz.status = QuizStatus::Closed;

    Ok(())
}
pub(crate) fn open_quiz(
    quiz: &mut crate::domain::quiz::Quiz,
    revision: crate::domain::quiz::QuizRevision,
    draft_items: &[DraftItem],
    starts_at: i64,
    duration_minutes: Option<u32>,
) -> Result<Snapshot, &'static str> {
    use crate::domain::quiz::QuizStatus;

    if !matches!(quiz.status, QuizStatus::Draft) {
        return Err("Only a draft quiz can be opened");
    }

    if revision.quiz_id != quiz.id || revision.revision_no != 1 {
        return Err("Opening a quiz requires its first revision");
    }

    let (snapshot, closes_at) = prepare_quiz_snapshot(
        revision,
        draft_items,
        quiz.max_attempts,
        starts_at,
        duration_minutes,
    )?;

    quiz.starts_at = starts_at.into();
    quiz.ends_at = closes_at.into();
    quiz.status = QuizStatus::Open;

    Ok(snapshot)
}
pub(crate) fn edit_draft_question(
    status: &crate::domain::quiz::QuizStatus,
    draft_items: &mut [DraftItem],
    stable_id: uuid::Uuid,
    question: crate::domain::question_validation::QuestionDraft,
    explanation: Option<String>,
) -> Result<(), &'static str> {
    if !matches!(status, crate::domain::quiz::QuizStatus::Draft) {
        return Err("Only draft quizzes can be edited directly");
    }

    let item = draft_items
        .iter_mut()
        .find(|item| item.stable_id == stable_id)
        .ok_or("Question does not belong to this draft")?;

    crate::domain::question_validation::validate_question(&question)?;

    item.question = question;
    item.explanation = explanation;
    Ok(())
}
pub(crate) fn add_draft_question(
    status: &crate::domain::quiz::QuizStatus,
    draft_items: &mut Vec<DraftItem>,
    question: crate::domain::question_validation::QuestionDraft,
    explanation: Option<String>,
) -> Result<uuid::Uuid, &'static str> {
    if !matches!(status, crate::domain::quiz::QuizStatus::Draft) {
        return Err("Only draft quizzes can be edited directly");
    }

    if draft_items.len() >= 50 {
        return Err("Quiz must have between 1 and 50 questions");
    }

    crate::domain::question_validation::validate_question(&question)?;

    let stable_id = uuid::Uuid::new_v4();
    draft_items.push(DraftItem {
        stable_id,
        question,
        explanation,
    });

    Ok(stable_id)
}
pub(crate) fn prepare_publication(
    draft: &QuizDraft,
    starts_at: i64,
    duration_minutes: Option<u32>,
) -> Result<i64, &'static str> {
    validate_quiz(draft)?;

    let minutes = duration_minutes.unwrap_or(15);
    if minutes == 0 {
        return Err("Quiz duration must be greater than zero");
    }

    let duration_seconds = i64::from(minutes) * 60;
    starts_at
        .checked_add(duration_seconds)
        .ok_or("Quiz closing time is out of range")
}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::question_validation::QuestionDraft;

    fn valid_draft() -> QuizDraft {
        let question: QuestionDraft = serde_json::from_value(serde_json::json!({
            "question_text": "Which HTTP status means Created?",
            "options": ["200 OK", "201 Created"],
            "correct_option_index": 1
        }))
        .unwrap();

        QuizDraft::new(vec![question], 2)
    }

    #[test]
    fn uses_default_or_requested_duration() {
        let draft = valid_draft();

        assert_eq!(prepare_publication(&draft, 100, None), Ok(1000));
        assert_eq!(prepare_publication(&draft, 100, Some(5)), Ok(400));
    }

    #[test]
    fn rejects_zero_duration() {
        assert_eq!(
            prepare_publication(&valid_draft(), 100, Some(0)),
            Err("Quiz duration must be greater than zero")
        );
    }

    #[test]
    fn rejects_an_empty_quiz_before_publishing() {
        let draft = QuizDraft::new(vec![], 1);

        assert_eq!(
            prepare_publication(&draft, 100, None),
            Err("Quiz must have between 1 and 50 questions")
        );
    }
}
#[cfg(test)]
mod snapshot_tests {
    use super::{build_snapshot, DraftItem};
    use crate::domain::question_validation::QuestionDraft;
    use crate::domain::quiz::QuizRevision;
    use uuid::Uuid;

    #[test]
    fn editing_a_draft_preserves_the_original_snapshot() {
        let revision = QuizRevision {
            id: Uuid::from_u128(1),
            quiz_id: Uuid::from_u128(2),
            revision_no: 1,
            created_by: Uuid::from_u128(3),
            change_reason: Default::default(),
            created_at: 0,
        };

        let mut draft_items = vec![DraftItem {
            stable_id: Uuid::from_u128(4),
            question: QuestionDraft {
                question_text: String::from("Which status means Created?"),
                options: vec![String::from("200 OK"), String::from("201 Created")],
                correct_option_index: 1,
            },
            explanation: None,
        }];

        let snapshot = build_snapshot(revision, &draft_items);

        draft_items[0].question.question_text = String::from("Which status means OK?");
        draft_items[0].question.correct_option_index = 0;

        let saved = &snapshot.questions[0].item;
        assert_eq!(saved.stable_id, draft_items[0].stable_id);
        assert_eq!(saved.question.question_text, "Which status means Created?");
        assert_eq!(saved.question.correct_option_index, 1);

    }
    #[test]
    fn invalid_edit_preserves_the_draft_question() {
        let stable_id = Uuid::from_u128(4);
        let mut items = vec![DraftItem {
            stable_id,
            question: QuestionDraft {
                question_text: String::from("Which status means Created?"),
                options: vec![String::from("200 OK"), String::from("201 Created")],
                correct_option_index: 1,
            },
            explanation: Some(String::from("201 means Created")),
        }];

        let mut replacement = items[0].question.clone();
        replacement.question_text = String::from("Which status means OK?");
        replacement.correct_option_index = 2;

        let result = super::edit_draft_question(
            &crate::domain::quiz::QuizStatus::Draft,
            &mut items,
            stable_id,
            replacement,
            None,
        );

        assert_eq!(result, Err("Correct answer index is out of range"));
        assert_eq!(items[0].question.question_text, "Which status means Created?");
        assert_eq!(items[0].question.correct_option_index, 1);
        assert_eq!(items[0].explanation.as_deref(), Some("201 means Created"));
    }
    #[test]
    fn edits_the_added_question_using_its_returned_id() {
        let status = crate::domain::quiz::QuizStatus::Draft;
        let mut items = Vec::new();

        let question = QuestionDraft {
            question_text: String::from("Which status means Created?"),
            options: vec![String::from("200 OK"), String::from("201 Created")],
            correct_option_index: 1,
        };

        let mut replacement = question.clone();
        replacement.question_text = String::from("Which status means OK?");
        replacement.correct_option_index = 0;

        let id = super::add_draft_question(&status, &mut items, question, None)
            .expect("adding a valid question should succeed");

        super::edit_draft_question(&status, &mut items, id, replacement, None)
            .expect("editing the question using its returned ID should succeed");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].stable_id, id);
        assert_eq!(items[0].question.question_text, "Which status means OK?");
        assert_eq!(items[0].question.correct_option_index, 0);
    }
    #[test]
    fn prepares_a_snapshot_with_its_questions_and_closing_time() {
        let revision_id = Uuid::from_u128(1);
        let revision = QuizRevision {
            id: revision_id,
            quiz_id: Uuid::from_u128(2),
            revision_no: 1,
            created_by: Uuid::from_u128(3),
            change_reason: Default::default(),
            created_at: 0,
        };

        let mut items = Vec::new();
        let question = QuestionDraft {
            question_text: String::from("Which status means Created?"),
            options: vec![String::from("200 OK"), String::from("201 Created")],
            correct_option_index: 1,
        };
        let question_id = super::add_draft_question(
            &crate::domain::quiz::QuizStatus::Draft,
            &mut items,
            question,
            None,
        )
        .expect("adding the question should succeed");

        let (snapshot, closes_at) =
            super::prepare_quiz_snapshot(revision, &items, 1, 1_000, Some(10))
                .expect("a valid quiz should produce a snapshot");

        assert_eq!(closes_at, 1_600);
        assert_eq!(snapshot.revision.id, revision_id);
        assert_eq!(snapshot.questions.len(), 1);
        assert_eq!(snapshot.questions[0].item.stable_id, question_id);
    }
    #[test]
    fn correcting_an_answer_preserves_the_previous_revision() {
        let quiz_id = Uuid::from_u128(1);
        let admin_id = Uuid::from_u128(2);
        let question_id = Uuid::from_u128(3);

        let revision = QuizRevision {
            id: Uuid::from_u128(4),
            quiz_id,
            revision_no: 1,
            created_by: admin_id,
            change_reason: Default::default(),
            created_at: 1_000,
        };

        let item = DraftItem {
            stable_id: question_id,
            question: QuestionDraft {
                question_text: String::from("Which status means Created?"),
                options: vec![String::from("200 OK"), String::from("201 Created")],
                correct_option_index: 0,
            },
            explanation: None,
        };

        let original = super::build_snapshot(revision, &[item]);
        let mut correction = original.questions[0].item.question.clone();
        correction.correct_option_index = 1;

        let revised = super::revise_question(
            &original,
            question_id,
            correction,
            None,
            admin_id,
            1_100,
            "Corrected the answer key",
        )
        .expect("a valid correction should create revision 2");

        let before = &original.questions[0];
        let after = &revised.questions[0];

        assert_eq!(before.item.question.correct_option_index, 0);
        assert_eq!(after.item.question.correct_option_index, 1);
        assert_eq!(after.item.stable_id, before.item.stable_id);
        assert_ne!(after.id, before.id);

        assert_eq!(revised.revision.quiz_id, quiz_id);
        assert_eq!(revised.revision.revision_no, 2);
        assert_ne!(revised.revision.id, original.revision.id);
    }
    #[test]
    fn closing_a_quiz_early_keeps_the_first_closing_time() {
        use crate::domain::quiz::{Quiz, QuizStatus};

        let mut quiz = Quiz {
            id: Uuid::from_u128(1),
            team_id: Uuid::from_u128(2),
            created_by: Uuid::from_u128(3),
            title: String::from("HTTP basics"),
            description: Default::default(),
            status: QuizStatus::Open,
            starts_at: 1_000_i64.into(),
            ends_at: 2_000_i64.into(),
            manually_closed_at: None,
            max_attempts: 2,
            created_at: 900,
        };

        assert!(quiz.is_open_at(1_200));

        super::close_quiz(&mut quiz, 1_200).expect("an open quiz can be closed");
        assert!(matches!(quiz.status, QuizStatus::Closed));
        assert!(!quiz.is_open_at(1_200));
        assert_eq!(quiz.manually_closed_at, Some(1_200));

        super::close_quiz(&mut quiz, 1_205).expect("closing again should succeed");
        assert_eq!(quiz.manually_closed_at, Some(1_200));
    }
    #[test]
    fn opening_an_empty_quiz_preserves_its_draft_status_and_schedule() {
        use crate::domain::quiz::{Quiz, QuizStatus};

        let admin_id = Uuid::from_u128(1);
        let mut quiz = Quiz {
            id: Uuid::from_u128(2),
            team_id: Uuid::from_u128(3),
            created_by: admin_id,
            title: String::from("HTTP basics"),
            description: Default::default(),
            status: QuizStatus::Draft,
            starts_at: Default::default(),
            ends_at: Default::default(),
            manually_closed_at: None,
            max_attempts: 1,
            created_at: 900,
        };

        let revision = QuizRevision {
            id: Uuid::from_u128(4),
            quiz_id: quiz.id,
            revision_no: 1,
            created_by: admin_id,
            change_reason: Default::default(),
            created_at: 900,
        };

        let original_start = quiz.starts_at;
        let original_end = quiz.ends_at;

        let result = super::open_quiz(&mut quiz, revision, &[], 1_000, Some(10));

        assert_eq!(result.err(), Some("Quiz must have between 1 and 50 questions"));
        assert!(matches!(quiz.status, QuizStatus::Draft));
        assert_eq!(quiz.starts_at, original_start);
        assert_eq!(quiz.ends_at, original_end);
    }
    #[test]
    fn opening_a_valid_draft_sets_the_window_and_returns_its_first_snapshot() {
        use crate::domain::quiz::{Quiz, QuizStatus};

        let admin_id = Uuid::from_u128(1);
        let mut quiz = Quiz {
            id: Uuid::from_u128(2),
            team_id: Uuid::from_u128(3),
            created_by: admin_id,
            title: String::from("HTTP basics"),
            description: Default::default(),
            status: QuizStatus::Draft,
            starts_at: Default::default(),
            ends_at: Default::default(),
            manually_closed_at: None,
            max_attempts: 1,
            created_at: 900,
        };

        let revision = QuizRevision {
            id: Uuid::from_u128(4),
            quiz_id: quiz.id,
            revision_no: 1,
            created_by: admin_id,
            change_reason: Default::default(),
            created_at: 900,
        };

        let mut items = Vec::new();
        let question = QuestionDraft {
            question_text: String::from("Which status means Created?"),
            options: vec![String::from("200 OK"), String::from("201 Created")],
            correct_option_index: 1,
        };
        let question_id = super::add_draft_question(&quiz.status, &mut items, question, None)
            .expect("a valid question should be added");

        let snapshot = super::open_quiz(&mut quiz, revision, &items, 1_000, Some(10))
            .expect("a valid draft should open");

        assert!(matches!(quiz.status, QuizStatus::Open));
        assert!(!quiz.is_open_at(999));
        assert!(quiz.is_open_at(1_000));
        assert!(quiz.is_open_at(1_599));
        assert!(!quiz.is_open_at(1_600));

        assert_eq!(snapshot.revision.quiz_id, quiz.id);
        assert_eq!(snapshot.revision.revision_no, 1);
        assert_eq!(snapshot.questions.len(), 1);
        assert_eq!(snapshot.questions[0].item.stable_id, question_id);
    }
}