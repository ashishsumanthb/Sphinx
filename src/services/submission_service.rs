use crate::domain::quiz::{can_start, AttemptSession, Quiz, QuizRevision, SessionStatus};
use uuid::Uuid;

pub(crate) fn start_or_resume(
    quiz: &Quiz,
    current_revision: &QuizRevision,
    user_id: Uuid,
    is_eligible: bool,
    completed_attempts: u8,
    now: i64,
    sessions: &mut Vec<AttemptSession>,
) -> Result<Uuid, &'static str> {
    let quiz_is_open = quiz.is_open_at(now);

    let existing = sessions.iter().find(|session| {
        session.user_id == user_id
            && session.quiz_id == quiz.id
            && matches!(session.status, SessionStatus::InProgress)
    });

    if let Some(session) = existing {
        if !is_eligible || !quiz_is_open {
            return Err("Cannot resume this quiz session");
        }
        return Ok(session.id);
    }

    if !can_start(is_eligible, quiz_is_open, completed_attempts, quiz.max_attempts) {
        return Err("Cannot start this quiz session");
    }

    if current_revision.quiz_id != quiz.id {
        return Err("Revision does not belong to this quiz");
    }

    let id = Uuid::new_v4();
    sessions.push(AttemptSession::new(id, user_id, current_revision, now));
    Ok(id)
}

use crate::domain::answer_validation::AnswerInput;
use crate::domain::quiz::can_submit;
use crate::domain::scoring::{calculate_score, Score, ScoringQuestion};
use crate::services::quiz_service::Snapshot;
use std::collections::HashMap;
use std::collections::hash_map::Entry;

pub(crate) struct SavedSubmission {
    answers: Vec<(Uuid, usize)>,
    pub(crate) score: Score,
}

pub(crate) fn submit_answers<'a>(
    quiz: &Quiz,
    snapshot: &Snapshot,
    session: &mut AttemptSession,
    user_id: Uuid,
    is_eligible: bool,
    completed_attempts: u8,
    now: i64,
    answers: &[AnswerInput],
    submissions: &'a mut HashMap<Uuid, SavedSubmission>,
) -> Result<&'a Score, &'static str> {
    if session.user_id != user_id || session.quiz_id != quiz.id {
        return Err("Session does not belong to this participant and quiz");
    }

    if snapshot.revision.id != session.quiz_revision_id
        || snapshot.revision.quiz_id != quiz.id
    {
        return Err("Snapshot does not match this session");
    }

    // Answer order can change when a participant retries the same submission.
    let mut selections: Vec<_> = answers
        .iter()
        .map(|answer| (answer.question_id, answer.selected_option_index))
        .collect();
    selections.sort_unstable();

    match submissions.entry(session.id) {
        Entry::Occupied(entry) => {
            let saved = entry.into_mut();
            if saved.answers != selections {
                return Err("Session was already submitted with different answers");
            }
            Ok(&saved.score)
        }
        Entry::Vacant(entry) => {
            let active = matches!(session.status, SessionStatus::InProgress);
            if !can_submit(
                is_eligible,
                quiz.is_open_at(now),
                completed_attempts,
                quiz.max_attempts,
                active,
            ) {
                return Err("Cannot submit this quiz session");
            }

            let answer_key = snapshot.questions.iter().map(|version| {
                let question = &version.item.question;
                let scoring_question = ScoringQuestion {
                    option_count: question.options.len(),
                    correct_option_index: question.correct_option_index,
                };
                (version.id, scoring_question)
            }).collect();

            let score = calculate_score(&answer_key, answers)?;
            let saved = entry.insert(SavedSubmission {
                answers: selections,
                score,
            });
            session.status = SessionStatus::Submitted;
            Ok(&saved.score)
        }
    }
}
pub(crate) fn questions_for_session(
    session: &AttemptSession,
    user_id: Uuid,
    snapshot: &Snapshot,
) -> Result<Vec<crate::domain::quiz::PublicQuestion>, &'static str> {
    use crate::domain::quiz::PublicQuestion;

    if session.user_id != user_id {
        return Err("Session does not belong to this participant");
    }

    if session.quiz_id != snapshot.revision.quiz_id
        || session.quiz_revision_id != snapshot.revision.id
    {
        return Err("Snapshot does not match this session");
    }

    let mut questions = Vec::with_capacity(snapshot.questions.len());

    for version in &snapshot.questions {
        let question = &version.item.question;
        questions.push(PublicQuestion {
            id: version.id,
            position: version.position.try_into().map_err(|_| "Invalid question position")?,
            text: question.question_text.clone(),
            options: question.options.clone(),
        });
    }

    Ok(questions)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::quiz::QuizStatus;

    #[test]
    fn starting_again_resumes_the_session_on_its_original_revision() {
        let admin_id = Uuid::from_u128(1);
        let user_id = Uuid::from_u128(2);
        let quiz = Quiz {
            id: Uuid::from_u128(3),
            team_id: Uuid::from_u128(4),
            created_by: admin_id,
            title: String::from("HTTP basics"),
            description: Default::default(),
            status: QuizStatus::Open,
            starts_at: 1_000_i64.into(),
            ends_at: 2_000_i64.into(),
            manually_closed_at: None,
            max_attempts: 1,
            created_at: 900,
        };

        let first_revision = QuizRevision {
            id: Uuid::from_u128(5),
            quiz_id: quiz.id,
            revision_no: 1,
            created_by: admin_id,
            change_reason: Default::default(),
            created_at: 900,
        };
        let mut sessions = Vec::new();

        let first_id = start_or_resume(&quiz, &first_revision, user_id, true, 0, 1_100, &mut sessions)
            .expect("the first click should start a session");

        let second_revision = QuizRevision {
            id: Uuid::from_u128(6),
            quiz_id: quiz.id,
            revision_no: 2,
            created_by: admin_id,
            change_reason: String::from("Corrected the answer key").into(),
            created_at: 1_150,
        };

        let resumed_id = start_or_resume(&quiz, &second_revision, user_id, true, 0, 1_200, &mut sessions)
            .expect("the second click should resume the session");

        assert_eq!(resumed_id, first_id);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].quiz_revision_id, first_revision.id);
        assert_eq!(sessions[0].started_at, 1_100);
    }
    #[test]
    fn submission_is_scored_once_and_only_identical_retries_succeed() {
        use crate::domain::question_validation::QuestionDraft;
        use crate::services::quiz_service::{build_snapshot, DraftItem};

        let admin_id = Uuid::from_u128(1);
        let user_id = Uuid::from_u128(2);
        let quiz = Quiz {
            id: Uuid::from_u128(3),
            team_id: Uuid::from_u128(4),
            created_by: admin_id,
            title: String::from("HTTP basics"),
            description: Default::default(),
            status: QuizStatus::Open,
            starts_at: 1_000_i64.into(),
            ends_at: 2_000_i64.into(),
            manually_closed_at: None,
            max_attempts: 1,
            created_at: 900,
        };

        let revision = QuizRevision {
            id: Uuid::from_u128(5),
            quiz_id: quiz.id,
            revision_no: 1,
            created_by: admin_id,
            change_reason: Default::default(),
            created_at: 900,
        };

        let item = DraftItem {
            stable_id: Uuid::from_u128(6),
            question: QuestionDraft {
                question_text: String::from("Which status means Created?"),
                options: vec![String::from("200 OK"), String::from("201 Created")],
                correct_option_index: 1,
            },
            explanation: None,
        };

        let snapshot = build_snapshot(revision, &[item]);
        let question_id = snapshot.questions[0].id;
        let mut session =
            AttemptSession::new(Uuid::from_u128(7), user_id, &snapshot.revision, 1_100);
        let mut submissions = HashMap::new();

        // Each call represents one click on Submit.
        let mut submit = |selected_option_index, completed_attempts, now| {
            let answers = [AnswerInput {
                question_id,
                selected_option_index,
            }];
            submit_answers(
                &quiz, &snapshot, &mut session, user_id, true,
                completed_attempts, now, &answers, &mut submissions,
            )
            .map(|score| score.score)
        };

        assert_eq!(submit(1, 0, 1_200), Ok(1));
        assert_eq!(submit(1, 1, 2_100), Ok(1));
        assert_eq!(
            submit(0, 1, 2_100),
            Err("Session was already submitted with different answers")
        );

        assert_eq!(submissions.len(), 1);
        assert!(matches!(session.status, SessionStatus::Submitted));
    }
    #[test]
    fn standings_only_count_scores_from_the_requested_revision() {
        use crate::services::analytics_service::{participant_standing, Standing};

        let quiz_id = Uuid::from_u128(1);
        let make_revision = |id, revision_no| QuizRevision {
            id: Uuid::from_u128(id),
            quiz_id,
            revision_no,
            created_by: Uuid::from_u128(2),
            change_reason: Default::default(),
            created_at: 900,
        };

        let first = make_revision(10, 1);
        let second = make_revision(20, 2);
        let user_id = Uuid::from_u128(100);
        let peer_id = Uuid::from_u128(101);
        let other_revision_user = Uuid::from_u128(102);

        let mut sessions = Vec::new();
        let mut submissions = HashMap::new();

        for (participant, revision, selected_index) in [
            (user_id, &first, 0),
            (peer_id, &first, 1),
            (other_revision_user, &second, 1),
        ] {
            let question_id = Uuid::new_v4();
            let key = HashMap::from([(
                question_id,
                ScoringQuestion {
                    option_count: 2,
                    correct_option_index: 1,
                },
            )]);
            let answer = AnswerInput {
                question_id,
                selected_option_index: selected_index,
            };
            let score = calculate_score(&key, &[answer]).expect("valid answer");

            let mut session = AttemptSession::new(Uuid::new_v4(), participant, revision, 1_000);
            session.status = SessionStatus::Submitted;
            submissions.insert(session.id, SavedSubmission {
                answers: vec![(question_id, selected_index)],
                score,
            });
            sessions.push(session);
        }

        assert_eq!(
            participant_standing(&first, user_id, &sessions, &submissions),
            Some(Standing { best_score: 0, rank: 2 })
        );
        assert_eq!(
            participant_standing(&first, other_revision_user, &sessions, &submissions),
            None
        );
    }
    #[test]
    fn participant_is_graded_on_the_revision_they_started() -> Result<(), &'static str> {
        use crate::domain::question_validation::QuestionDraft;
        use crate::services::quiz_service::{add_draft_question, open_quiz, revise_question};

        let admin = Uuid::from_u128(1);
        let participant = Uuid::from_u128(2);
        let mut quiz = Quiz {
            id: Uuid::from_u128(3),
            team_id: Uuid::from_u128(4),
            created_by: admin,
            title: String::from("HTTP basics"),
            description: Default::default(),
            status: QuizStatus::Draft,
            starts_at: Default::default(),
            ends_at: Default::default(),
            manually_closed_at: None,
            max_attempts: 1,
            created_at: 900,
        };

        // The admin accidentally marks "200 OK" as correct in revision 1.
        let mut items = Vec::new();
        let question = QuestionDraft {
            question_text: String::from("Which status means Created?"),
            options: vec![String::from("200 OK"), String::from("201 Created")],
            correct_option_index: 0,
        };
        let stable_id = add_draft_question(&quiz.status, &mut items, question, None)?;
        let revision = QuizRevision {
            id: Uuid::from_u128(5),
            quiz_id: quiz.id,
            revision_no: 1,
            created_by: admin,
            change_reason: Default::default(),
            created_at: 1_000,
        };
        let first = open_quiz(&mut quiz, revision, &items, 1_000, Some(10))?;

        // The participant starts and receives revision 1's question ID.
        let mut sessions = Vec::new();
        let session_id = start_or_resume(
            &quiz, &first.revision, participant, true, 0, 1_100, &mut sessions,
        )?;
        let session = sessions.iter_mut().find(|s| s.id == session_id)
            .expect("starting the quiz should store the session");

        let shown = questions_for_session(session, participant, &first)?;
        assert_eq!(shown.len(), 1);
        assert_eq!(shown[0].id, first.questions[0].id);

        // The admin corrects the answer while the participant is answering.
        let mut correction = first.questions[0].item.question.clone();
        correction.correct_option_index = 1;
        let second = revise_question(
            &first, stable_id, correction, None, admin, 1_150, "Fixed the answer key",
        )?;

        assert!(questions_for_session(session, admin, &first).is_err());
        assert!(questions_for_session(session, participant, &second).is_err());

        // Submit the ID the participant received and the option revision 1 accepted.
        let answers = [AnswerInput {
            question_id: shown[0].id,
            selected_option_index: 0,
        }];
        let mut submissions = HashMap::new();

        assert!(submit_answers(
            &quiz, &second, session, participant, true, 0, 1_200, &answers, &mut submissions,
        ).is_err());
        assert!(submissions.is_empty());

        let score = submit_answers(
            &quiz, &first, session, participant, true, 0, 1_200, &answers, &mut submissions,
        )?;
        assert_eq!(score.score, 1);
        assert_eq!(submissions.len(), 1);
        Ok(())
    }
}