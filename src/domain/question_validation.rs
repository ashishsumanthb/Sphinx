#[derive(Clone, serde::Deserialize)]
pub(crate) struct QuestionDraft {
    pub(crate) question_text: String,
    pub(crate) options: Vec<String>,
    pub(crate) correct_option_index: usize,
}
pub(crate) fn validate_question(question: &QuestionDraft) -> Result<(), &'static str> {
    if question.question_text.trim().is_empty() {
        return Err("Question text must not be blank");
    }

    let option_count = question.options.len();

    if option_count < 2 || option_count > 6 {
        return Err("Question must have between 2 and 6 options");
    }

    for option in &question.options {
        if option.trim().is_empty() {
            return Err("Option text must not be blank");
        }
    }

    if question.correct_option_index >= option_count {
        return Err("Correct answer index is out of range");
    }

    Ok(())
}
#[cfg(test)]
pub(super) fn blank_question() -> QuestionDraft {
    QuestionDraft {
        question_text: String::new(),
        ..sample_question()
    }
}
#[cfg(test)]
pub(super) fn sample_question() -> QuestionDraft {
    QuestionDraft {
        question_text: String::from("Which HTTP status means Created?"),
        options: vec![String::from("200 OK"), String::from("201 Created")],
        correct_option_index: 1,
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_blank_question_text() {
        let draft = QuestionDraft {
            question_text: String::from("   "),
            options: vec![String::from("200 OK"), String::from("201 Created")],
            correct_option_index: 1,
        };

        assert_eq!(
            validate_question(&draft),
            Err("Question text must not be blank")
        );
    }

    #[test]
    fn accepts_valid_question() {
        let draft = QuestionDraft {
            question_text: String::from("Which HTTP status means Created?"),
            options: vec![String::from("200 OK"), String::from("201 Created")],
            correct_option_index: 1,
        };

        assert_eq!(validate_question(&draft), Ok(()));
    }

    #[test]
    fn accepts_six_options() {
        let draft = QuestionDraft {
            question_text: String::from("Which HTTP status means Created?"),
            options: vec![
                String::from("200 OK"),
                String::from("202 Accepted"),
                String::from("204 No Content"),
                String::from("400 Bad Request"),
                String::from("404 Not Found"),
                String::from("201 Created"),
            ],
            correct_option_index: 5,
        };

        assert_eq!(validate_question(&draft), Ok(()));
    }

    #[test]
    fn rejects_too_few_options() {
        let draft = QuestionDraft {
            question_text: String::from("Which HTTP status means Created?"),
            options: vec![String::from("201 Created")],
            correct_option_index: 0,
        };

        assert_eq!(
            validate_question(&draft),
            Err("Question must have between 2 and 6 options")
        );
    }

    #[test]
    fn rejects_too_many_options() {
        let draft = QuestionDraft {
            question_text: String::from("Which HTTP status means Created?"),
            options: vec![
                String::from("200 OK"),
                String::from("201 Created"),
                String::from("202 Accepted"),
                String::from("204 No Content"),
                String::from("400 Bad Request"),
                String::from("404 Not Found"),
                String::from("500 Internal Server Error"),
            ],
            correct_option_index: 1,
        };

        assert_eq!(
            validate_question(&draft),
            Err("Question must have between 2 and 6 options")
        );
    }

    #[test]
    fn rejects_blank_option() {
        let draft = QuestionDraft {
            question_text: String::from("Which HTTP status means Created?"),
            options: vec![String::from("201 Created"), String::from("   ")],
            correct_option_index: 0,
        };

        assert_eq!(
            validate_question(&draft),
            Err("Option text must not be blank")
        );
    }

    #[test]
    fn rejects_invalid_answer_index() {
        let draft = QuestionDraft {
            question_text: String::from("Which HTTP status means Created?"),
            options: vec![String::from("200 OK"), String::from("201 Created")],
            correct_option_index: 2,
        };

        assert_eq!(
            validate_question(&draft),
            Err("Correct answer index is out of range")
        );
    }
}
