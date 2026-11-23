use chrono::Utc;
use standuppulse::api::dto::{
    AttemptQuestionReviewResponse, AttemptReviewResponse, MyAttemptResponse, MyResultsResponse,
    ParticipantAnalyticsResponse, QuizAnalyticsResponse,
};
use uuid::Uuid;

#[test]
fn member_results_are_personal_and_revision_aware() {
    let results = MyResultsResponse {
        quiz_id: Uuid::new_v4(),
        best_score: Some(2),
        best_percentage: Some(100.0),
        revision_number: Some(2),
        rank: Some(1),
        attempts: vec![MyAttemptResponse {
            attempt_id: Uuid::new_v4(),
            quiz_id: Uuid::new_v4(),
            score: 2,
            total_questions: 2,
            percentage: 100.0,
            submitted_at: Utc::now(),
        }],
    };
    let json = serde_json::to_value(results).unwrap();
    assert_eq!(json["revision_number"], 2);
    assert_eq!(json["rank"], 1);
    assert!(json.get("participants").is_none());
    assert!(json.get("correct_option_index").is_none());
}

#[test]
fn admin_analytics_keep_ranks_separate_by_revision() {
    let analytics = QuizAnalyticsResponse {
        quiz_id: Uuid::new_v4(),
        eligible_members: 3,
        unique_participants: 3,
        total_submissions: 3,
        participation_rate: Some(100.0),
        average_score_percentage: Some(83.3333),
        participants: vec![
            ParticipantAnalyticsResponse {
                user_id: Uuid::new_v4(),
                revision_number: 1,
                score: 2,
                total_questions: 2,
                percentage: 100.0,
                rank: 1,
            },
            ParticipantAnalyticsResponse {
                user_id: Uuid::new_v4(),
                revision_number: 2,
                score: 1,
                total_questions: 2,
                percentage: 50.0,
                rank: 1,
            },
        ],
    };
    let json = serde_json::to_value(analytics).unwrap();
    assert_eq!(json["participants"][0]["revision_number"], 1);
    assert_eq!(json["participants"][1]["revision_number"], 2);
    assert_eq!(json["participants"][0]["rank"], 1);
    assert_eq!(json["participants"][1]["rank"], 1);
}

#[test]
fn answer_key_is_only_present_in_post_submission_review() {
    let review = AttemptReviewResponse {
        attempt_id: Uuid::new_v4(),
        quiz_id: Uuid::new_v4(),
        score: 1,
        total_questions: 1,
        percentage: 100.0,
        submitted_at: Utc::now(),
        questions: vec![AttemptQuestionReviewResponse {
            question_id: Uuid::new_v4(),
            position: 0,
            question_text: "Reviewed question".to_owned(),
            options: vec!["A".to_owned(), "B".to_owned()],
            selected_option_index: 1,
            correct_option_index: 1,
            is_correct: true,
            explanation: "B is correct".to_owned(),
        }],
    };
    let json = serde_json::to_value(review).unwrap();
    assert_eq!(json["questions"][0]["correct_option_index"], 1);
    assert_eq!(json["questions"][0]["is_correct"], true);
    assert_eq!(json["questions"][0]["explanation"], "B is correct");
}
