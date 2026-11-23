use super::answer_validation::{AnswerInput, validate_answers};
use std::collections::HashMap;
use uuid::Uuid;

pub(crate) struct ScoringQuestion {
    pub(crate) option_count: usize,
    pub(crate) correct_option_index: usize,
}

#[derive(Debug, PartialEq)]
pub(crate) struct Score {
    pub(crate) score: usize,
    pub(crate) total_questions: usize,
    pub(crate) percentage: f64,
}
pub(crate) fn calculate_score(
    answer_key: &HashMap<Uuid, ScoringQuestion>,
    answers: &[AnswerInput],
) -> Result<Score, &'static str> {
    if answer_key.is_empty() {
        return Err("Cannot score an empty quiz");
    }
    let mut option_counts = HashMap::new();
    for (&question_id, question) in answer_key {
        option_counts.insert(question_id, question.option_count);
    }
    validate_answers(&option_counts, answers)?;
    let mut correct = 0;
    for answer in answers {
        let question = &answer_key[&answer.question_id];
        if answer.selected_option_index == question.correct_option_index {
            correct += 1;
        }
    }
    let total_questions = answer_key.len();
    Ok(Score {
        score: correct,
        total_questions,
        percentage: 100.0 * correct as f64 / total_questions as f64,
    })
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scores_answers_using_the_question_ids() {
        let a = Uuid::from_u128(1);
        let b = Uuid::from_u128(2);

        let answer_key = HashMap::from([
            (
                a,
                ScoringQuestion {
                    option_count: 2,
                    correct_option_index: 1,
                },
            ),
            (
                b,
                ScoringQuestion {
                    option_count: 2,
                    correct_option_index: 0,
                },
            ),
        ]);

        // Each row gives B's selection, A's selection, points, and percentage.
        for (pick_b, pick_a, points, percentage) in
            [(0, 1, 2, 100.0), (0, 0, 1, 50.0), (1, 0, 0, 0.0)]
        {
            let answers = [
                AnswerInput {
                    question_id: b,
                    selected_option_index: pick_b,
                },
                AnswerInput {
                    question_id: a,
                    selected_option_index: pick_a,
                },
            ];

            assert_eq!(
                calculate_score(&answer_key, &answers),
                Ok(Score {
                    score: points,
                    total_questions: 2,
                    percentage
                })
            );
        }
    }
}
