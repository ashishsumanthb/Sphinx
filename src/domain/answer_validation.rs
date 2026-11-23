use std::collections::{HashMap, HashSet};
use uuid::Uuid;
#[derive(serde::Deserialize)]
pub(crate) struct AnswerInput {
    pub(crate) question_id: Uuid,
    pub(crate) selected_option_index: usize,
}
pub(crate) fn validate_answers(
    option_counts: &HashMap<Uuid, usize>,
    answers: &[AnswerInput],
) -> Result<(), &'static str> {
    let mut answered_ids = HashSet::new();
    for answer in answers {
        let Some(&option_count) = option_counts.get(&answer.question_id) else {
            return Err("Question does not belong to this quiz session");
        };
        if !answered_ids.insert(answer.question_id) {
            return Err("Question was answered more than once");
        }
        validate_selected_option(answer.selected_option_index, option_count)?;
    }
    if answered_ids.len() != option_counts.len() {
        return Err("Every question must have an answer");
    }
    Ok(())
}
pub(crate) fn validate_selected_option(
    selected_option_index: usize,
    option_count: usize,
) -> Result<(), &'static str> {
    if selected_option_index >= option_count {
        return Err("Selected option index is out of range");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_first_option() {
        assert_eq!(validate_selected_option(0, 2), Ok(()));
    }

    #[test]
    fn accepts_last_option() {
        assert_eq!(validate_selected_option(1, 2), Ok(()));
    }

    #[test]
    fn rejects_index_equal_to_option_count() {
        assert_eq!(
            validate_selected_option(2, 2),
            Err("Selected option index is out of range")
        );
    }
    #[test]
    fn rejects_duplicate_even_when_answer_count_matches() {
        let question_a = Uuid::from_u128(1);
        let question_b = Uuid::from_u128(2);
        let option_counts = HashMap::from([(question_a, 2), (question_b, 2)]);

        let answers = [
            AnswerInput {
                question_id: question_a,
                selected_option_index: 0,
            },
            AnswerInput {
                question_id: question_a,
                selected_option_index: 1,
            },
        ];

        assert_eq!(
            validate_answers(&option_counts, &answers),
            Err("Question was answered more than once")
        );
    }

    fn answer(question_id: Uuid, selected_option_index: usize) -> AnswerInput {
        AnswerInput {
            question_id,
            selected_option_index,
        }
    }

    #[test]
    fn accepts_all_answers_in_any_order() {
        let a = Uuid::from_u128(1);
        let b = Uuid::from_u128(2);
        let option_counts = HashMap::from([(a, 2), (b, 3)]);

        let answers = [answer(b, 2), answer(a, 0)];

        assert_eq!(validate_answers(&option_counts, &answers), Ok(()));
    }

    #[test]
    fn rejects_missing_answer() {
        let a = Uuid::from_u128(1);
        let b = Uuid::from_u128(2);
        let option_counts = HashMap::from([(a, 2), (b, 3)]);

        assert_eq!(
            validate_answers(&option_counts, &[answer(a, 0)]),
            Err("Every question must have an answer")
        );
    }

    #[test]
    fn rejects_question_from_another_session() {
        let a = Uuid::from_u128(1);
        let unrelated = Uuid::from_u128(99);
        let option_counts = HashMap::from([(a, 2)]);

        assert_eq!(
            validate_answers(&option_counts, &[answer(unrelated, 0)]),
            Err("Question does not belong to this quiz session")
        );
    }

    #[test]
    fn rejects_option_out_of_range_in_submission() {
        let a = Uuid::from_u128(1);
        let option_counts = HashMap::from([(a, 2)]);

        assert_eq!(
            validate_answers(&option_counts, &[answer(a, 2)]),
            Err("Selected option index is out of range")
        );
    }
}
