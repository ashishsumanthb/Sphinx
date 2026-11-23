use uuid::Uuid;

use crate::{
    api::{
        dto::{
            AnswerInput, AttemptReviewResponse, AttemptSessionResponse, AvailableQuizResponse,
            MyAttemptResponse, MyResultsResponse, PublicQuestionResponse, QuizAnalyticsResponse,
            SubmitAttemptResponse,
        },
        error::AppError,
    },
    repository::participation::ParticipationRepository,
};

#[derive(Clone)]
pub struct ParticipationService {
    repository: ParticipationRepository,
}

impl ParticipationService {
    pub fn new(repository: ParticipationRepository) -> Self {
        Self { repository }
    }

    pub async fn list_available(
        &self,
        team_id: Uuid,
        user_id: Uuid,
    ) -> Result<Vec<AvailableQuizResponse>, AppError> {
        self.repository.list_available(team_id, user_id).await
    }

    pub async fn start_session(
        &self,
        quiz_id: Uuid,
        user_id: Uuid,
    ) -> Result<AttemptSessionResponse, AppError> {
        self.repository.start_session(quiz_id, user_id).await
    }

    pub async fn session_questions(
        &self,
        session_id: Uuid,
        user_id: Uuid,
    ) -> Result<Vec<PublicQuestionResponse>, AppError> {
        self.repository.session_questions(session_id, user_id).await
    }

    pub async fn submit(
        &self,
        session_id: Uuid,
        user_id: Uuid,
        answers: &[AnswerInput],
    ) -> Result<SubmitAttemptResponse, AppError> {
        self.repository.submit(session_id, user_id, answers).await
    }

    pub async fn my_attempts(
        &self,
        quiz_id: Uuid,
        user_id: Uuid,
    ) -> Result<Vec<MyAttemptResponse>, AppError> {
        self.repository.my_attempts(quiz_id, user_id).await
    }

    pub async fn attempt_review(
        &self,
        attempt_id: Uuid,
        user_id: Uuid,
    ) -> Result<AttemptReviewResponse, AppError> {
        self.repository.attempt_review(attempt_id, user_id).await
    }

    pub async fn my_results(
        &self,
        quiz_id: Uuid,
        user_id: Uuid,
    ) -> Result<MyResultsResponse, AppError> {
        self.repository.my_results(quiz_id, user_id).await
    }

    pub async fn quiz_analytics(
        &self,
        quiz_id: Uuid,
        user_id: Uuid,
    ) -> Result<QuizAnalyticsResponse, AppError> {
        self.repository.quiz_analytics(quiz_id, user_id).await
    }
}
