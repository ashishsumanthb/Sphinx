use super::question_validation::{validate_question, QuestionDraft};

pub(crate) struct QuizDraft {
    questions: Vec<QuestionDraft>,
    max_attempts: u8,
}
impl QuizDraft {
    pub(crate) fn new(questions: Vec<QuestionDraft>, max_attempts: u8) -> Self {
        Self {
            questions,
            max_attempts,
        }
    }
}
pub(crate) fn validate_quiz(quiz: &QuizDraft) -> Result<(), &'static str> {
    let question_count = quiz.questions.len();

    if question_count < 1 || question_count > 50 {
        return Err("Quiz must have between 1 and 50 questions");
    }

    if quiz.max_attempts < 1 || quiz.max_attempts > 2 {
        return Err("Quiz must allow 1 or 2 attempts");
    }

    for question in &quiz.questions {
        validate_question(question)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::question_validation::{blank_question, sample_question};
    #[test]
    fn rejects_quiz_with_no_questions() {
        let quiz = QuizDraft {
            questions: vec![],
            max_attempts: 1,
        };

        assert_eq!(
            validate_quiz(&quiz),
            Err("Quiz must have between 1 and 50 questions")
        );
    }

    #[test]
    fn rejects_three_attempts() {
        let quiz = QuizDraft {
            questions: vec![sample_question()],
            max_attempts: 3,
        };

        assert_eq!(
            validate_quiz(&quiz),
            Err("Quiz must allow 1 or 2 attempts")
        );
    }
    #[test]
    fn accepts_quiz_with_two_attempts(){
        let quiz = QuizDraft{
            questions: vec![sample_question()],
            max_attempts: 2,
        };
        assert_eq!(validate_quiz(&quiz),Ok(()));
    }
    #[test]
    fn accepts_fifty_questions(){
        let quiz = QuizDraft{
            questions:(0..50).map(|_|sample_question()).collect(),
            max_attempts:1,
        };
        assert_eq!(validate_quiz(&quiz),Ok(()));

    }
    #[test]
    fn rejects_fifty_one_questions(){
        let quiz = QuizDraft{
            questions: (0..51).map(|_|sample_question()).collect(),
            max_attempts:1,
        };
        assert_eq!(
            validate_quiz(&quiz),
            Err("Quiz must have between 1 and 50 questions")
        );
    }
    #[test]
    fn rejects_quiz_when_second_question_is_blank() {
        let quiz = QuizDraft {
            questions: vec![sample_question(), blank_question()],
            max_attempts: 1,
        };

        assert_eq!(validate_quiz(&quiz), Err("Question text must not be blank"));
    }

    #[test]
    fn rejects_zero_attempts() {
        let quiz = QuizDraft {
            questions: vec![sample_question()],
            max_attempts: 0,
        };

        assert_eq!(validate_quiz(&quiz), Err("Quiz must allow 1 or 2 attempts"));
    }
}