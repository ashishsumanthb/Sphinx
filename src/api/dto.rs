use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableQuizResponse {
    pub id: Uuid,
    pub team_id: Uuid,
    pub title: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableQuizListResponse {
    pub quizzes: Vec<AvailableQuizResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicQuestionResponse {
    pub question_id: Uuid,
    pub position: u32,
    pub question_text: String,
    pub options: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttemptSessionResponse {
    pub session_id: Uuid,
    pub quiz_id: Uuid,
    pub quiz_revision_id: Uuid,
    pub started_at: DateTime<Utc>,
    pub deadline: DateTime<Utc>,
    pub attempt_number: u8,
    pub max_attempts: u8,
    pub remaining_attempts: u8,
    pub questions: Vec<PublicQuestionResponse>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitAttemptRequest {
    pub answers: Vec<AnswerInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AnswerInput {
    pub question_id: Uuid,
    pub selected_option_index: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitAttemptResponse {
    pub attempt_id: Uuid,
    pub quiz_id: Uuid,
    pub attempt_number: u8,
    pub max_attempts: u8,
    pub remaining_attempts: u8,
    pub score: u32,
    pub total_questions: u32,
    pub percentage: f64,
    pub submitted_at: DateTime<Utc>,
    pub idempotent_replay: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyAttemptResponse {
    pub attempt_id: Uuid,
    pub quiz_id: Uuid,
    pub quiz_revision_id: Uuid,
    pub revision_number: u32,
    pub attempt_number: u8,
    pub score: u32,
    pub total_questions: u32,
    pub percentage: f64,
    pub submitted_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyAttemptsResponse {
    pub attempts: Vec<MyAttemptResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttemptQuestionReviewResponse {
    pub question_id: Uuid,
    pub position: u32,
    pub question_text: String,
    pub options: Vec<String>,
    pub selected_option_index: u16,
    pub correct_option_index: u16,
    pub is_correct: bool,
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttemptReviewResponse {
    pub attempt_id: Uuid,
    pub quiz_id: Uuid,
    pub score: u32,
    pub total_questions: u32,
    pub percentage: f64,
    pub submitted_at: DateTime<Utc>,
    pub questions: Vec<AttemptQuestionReviewResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyResultsResponse {
    pub quiz_id: Uuid,
    /// Summary of the highest revision_no attempted by this user.
    pub best_score: Option<u32>,
    pub best_percentage: Option<f64>,
    pub revision_number: Option<u32>,
    pub rank: Option<u64>,
    pub attempts: Vec<MyAttemptResponse>,
    pub revision_results: Vec<RevisionResultResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevisionResultResponse {
    pub quiz_revision_id: Uuid,
    pub revision_number: u32,
    pub best_score: u32,
    pub best_percentage: f64,
    pub rank: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantAnalyticsResponse {
    pub user_id: Uuid,
    pub revision_number: u32,
    pub score: u32,
    pub total_questions: u32,
    pub percentage: f64,
    pub rank: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizAnalyticsResponse {
    pub quiz_id: Uuid,
    pub eligible_members: u64,
    pub unique_participants: u64,
    pub total_submissions: u64,
    pub participation_rate: Option<f64>,
    pub average_score_percentage: Option<f64>,
    pub participants: Vec<ParticipantAnalyticsResponse>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorResponse {
    pub error: ErrorDetail,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
    pub request_id: String,
}
