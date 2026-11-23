use std::collections::{HashMap, HashSet};

use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::api::dto::AnswerInput;

#[derive(Debug, Clone)]
pub struct QuestionKey {
    pub question_id: Uuid,
    pub option_count: usize,
    pub correct_option_index: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoredAnswer {
    pub question_id: Uuid,
    pub selected_option_index: u16,
    pub is_correct: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnswerSetError {
    MissingAnswers,
    DuplicateQuestion,
    ForeignQuestion,
    OptionOutOfRange,
}

pub fn validate_and_score(
    questions: &[QuestionKey],
    answers: &[AnswerInput],
) -> Result<Vec<ScoredAnswer>, AnswerSetError> {
    if answers.len() != questions.len() {
        return Err(AnswerSetError::MissingAnswers);
    }

    let keys: HashMap<Uuid, &QuestionKey> = questions
        .iter()
        .map(|question| (question.question_id, question))
        .collect();
    let mut seen = HashSet::with_capacity(answers.len());
    let mut scored = Vec::with_capacity(answers.len());

    for answer in answers {
        if !seen.insert(answer.question_id) {
            return Err(AnswerSetError::DuplicateQuestion);
        }
        let question = keys
            .get(&answer.question_id)
            .ok_or(AnswerSetError::ForeignQuestion)?;
        if usize::from(answer.selected_option_index) >= question.option_count {
            return Err(AnswerSetError::OptionOutOfRange);
        }
        scored.push(ScoredAnswer {
            question_id: answer.question_id,
            selected_option_index: answer.selected_option_index,
            is_correct: answer.selected_option_index == question.correct_option_index,
        });
    }

    scored.sort_by_key(|answer| answer.question_id);
    Ok(scored)
}

pub fn canonical_request_hash(answers: &[AnswerInput]) -> String {
    let mut canonical = answers.to_vec();
    canonical.sort_by_key(|answer| answer.question_id);
    let mut hasher = Sha256::new();
    for answer in canonical {
        hasher.update(answer.question_id.as_bytes());
        hasher.update(answer.selected_option_index.to_be_bytes());
    }
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (Vec<QuestionKey>, Vec<AnswerInput>) {
        let first = Uuid::from_u128(1);
        let second = Uuid::from_u128(2);
        (
            vec![
                QuestionKey {
                    question_id: first,
                    option_count: 3,
                    correct_option_index: 1,
                },
                QuestionKey {
                    question_id: second,
                    option_count: 2,
                    correct_option_index: 0,
                },
            ],
            vec![
                AnswerInput {
                    question_id: first,
                    selected_option_index: 1,
                },
                AnswerInput {
                    question_id: second,
                    selected_option_index: 1,
                },
            ],
        )
    }

    #[test]
    fn scores_complete_answer_set() {
        let (questions, answers) = fixture();
        let scored = validate_and_score(&questions, &answers).unwrap();
        assert_eq!(scored.iter().filter(|answer| answer.is_correct).count(), 1);
    }

    #[test]
    fn rejects_missing_duplicate_foreign_and_out_of_range_answers() {
        let (questions, answers) = fixture();
        assert_eq!(
            validate_and_score(&questions, &answers[..1]),
            Err(AnswerSetError::MissingAnswers)
        );

        let duplicate = vec![answers[0].clone(), answers[0].clone()];
        assert_eq!(
            validate_and_score(&questions, &duplicate),
            Err(AnswerSetError::DuplicateQuestion)
        );

        let foreign = vec![
            answers[0].clone(),
            AnswerInput {
                question_id: Uuid::from_u128(3),
                selected_option_index: 0,
            },
        ];
        assert_eq!(
            validate_and_score(&questions, &foreign),
            Err(AnswerSetError::ForeignQuestion)
        );

        let out_of_range = vec![
            answers[0].clone(),
            AnswerInput {
                question_id: answers[1].question_id,
                selected_option_index: 2,
            },
        ];
        assert_eq!(
            validate_and_score(&questions, &out_of_range),
            Err(AnswerSetError::OptionOutOfRange)
        );
    }

    #[test]
    fn canonical_hash_is_independent_of_answer_order() {
        let (_, mut answers) = fixture();
        let first = canonical_request_hash(&answers);
        answers.reverse();
        assert_eq!(first, canonical_request_hash(&answers));
    }
}
