use chrono::{Duration, Utc};
use sphinx::{
    api::dto::{AnswerInput, AttemptSessionResponse, PublicQuestionResponse},
    domain::participation::{
        AnswerSetError, QuestionKey, canonical_request_hash, validate_and_score,
    },
};
use uuid::Uuid;

fn questions() -> Vec<QuestionKey> {
    vec![
        QuestionKey {
            question_id: Uuid::from_u128(1),
            option_count: 2,
            correct_option_index: 1,
        },
        QuestionKey {
            question_id: Uuid::from_u128(2),
            option_count: 3,
            correct_option_index: 0,
        },
    ]
}

#[test]
fn day_two_scoring_and_retry_fingerprint_are_deterministic() {
    let answers = vec![
        AnswerInput {
            question_id: Uuid::from_u128(1),
            selected_option_index: 1,
        },
        AnswerInput {
            question_id: Uuid::from_u128(2),
            selected_option_index: 2,
        },
    ];
    let scored = validate_and_score(&questions(), &answers).unwrap();
    assert_eq!(scored.iter().filter(|answer| answer.is_correct).count(), 1);

    let mut reversed = answers.clone();
    reversed.reverse();
    assert_eq!(
        canonical_request_hash(&answers),
        canonical_request_hash(&reversed)
    );
}

#[test]
fn day_two_rejects_every_invalid_answer_shape() {
    let keys = questions();
    let first = AnswerInput {
        question_id: Uuid::from_u128(1),
        selected_option_index: 1,
    };
    assert_eq!(
        validate_and_score(&keys, std::slice::from_ref(&first)),
        Err(AnswerSetError::MissingAnswers)
    );
    assert_eq!(
        validate_and_score(&keys, &[first.clone(), first]),
        Err(AnswerSetError::DuplicateQuestion)
    );
    assert_eq!(
        validate_and_score(
            &keys,
            &[
                AnswerInput {
                    question_id: Uuid::from_u128(1),
                    selected_option_index: 1,
                },
                AnswerInput {
                    question_id: Uuid::from_u128(99),
                    selected_option_index: 0,
                },
            ],
        ),
        Err(AnswerSetError::ForeignQuestion)
    );
    assert_eq!(
        validate_and_score(
            &keys,
            &[
                AnswerInput {
                    question_id: Uuid::from_u128(1),
                    selected_option_index: 2,
                },
                AnswerInput {
                    question_id: Uuid::from_u128(2),
                    selected_option_index: 0,
                },
            ],
        ),
        Err(AnswerSetError::OptionOutOfRange)
    );
}

#[test]
fn day_two_session_dto_never_exposes_answer_keys() {
    let now = Utc::now();
    let session = AttemptSessionResponse {
        session_id: Uuid::new_v4(),
        quiz_id: Uuid::new_v4(),
        quiz_revision_id: Uuid::new_v4(),
        started_at: now,
        deadline: now + Duration::minutes(15),
        attempt_number: 1,
        max_attempts: 2,
        remaining_attempts: 2,
        questions: vec![PublicQuestionResponse {
            question_id: Uuid::new_v4(),
            position: 0,
            question_text: "Safe question".to_owned(),
            options: vec!["A".to_owned(), "B".to_owned()],
        }],
    };
    let json = serde_json::to_value(session).unwrap().to_string();
    assert!(!json.contains("correct_option_index"));
    assert!(!json.contains("explanation"));
    assert!(!json.contains("is_correct"));
}
