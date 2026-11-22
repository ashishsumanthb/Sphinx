use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use uuid::Uuid;

use crate::{
    AppState,
    api::{
        auth::AuthenticatedUser,
        dto::{
            AttemptSessionResponse, AvailableQuizListResponse, MyAttemptsResponse,
            AttemptReviewResponse, MyResultsResponse, PublicQuestionResponse,
            QuizAnalyticsResponse, SubmitAttemptRequest, SubmitAttemptResponse,
        },
        error::ApiResult,
    },
    repository::participation::ParticipationRepository,
    services::participation::ParticipationService,
};

fn service(state: &AppState) -> ParticipationService {
    ParticipationService::new(ParticipationRepository::new(state.pool.clone()))
}

pub async fn list_available_quizzes(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(team_id): Path<Uuid>,
) -> ApiResult<Json<AvailableQuizListResponse>> {
    let quizzes = service(&state)
        .list_available(team_id, auth.user_id)
        .await?;
    Ok(Json(AvailableQuizListResponse { quizzes }))
}

pub async fn start_attempt_session(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(quiz_id): Path<Uuid>,
) -> ApiResult<(StatusCode, Json<AttemptSessionResponse>)> {
    let session = service(&state).start_session(quiz_id, auth.user_id).await?;
    Ok((StatusCode::CREATED, Json(session)))
}

pub async fn get_session_questions(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(session_id): Path<Uuid>,
) -> ApiResult<Json<Vec<PublicQuestionResponse>>> {
    let questions = service(&state)
        .session_questions(session_id, auth.user_id)
        .await?;
    Ok(Json(questions))
}

pub async fn submit_attempt(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(session_id): Path<Uuid>,
    Json(request): Json<SubmitAttemptRequest>,
) -> ApiResult<Json<SubmitAttemptResponse>> {
    let result = service(&state)
        .submit(session_id, auth.user_id, &request.answers)
        .await?;
    Ok(Json(result))
}

pub async fn my_attempts(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(quiz_id): Path<Uuid>,
) -> ApiResult<Json<MyAttemptsResponse>> {
    let attempts = service(&state).my_attempts(quiz_id, auth.user_id).await?;
    Ok(Json(MyAttemptsResponse { attempts }))
}

pub async fn attempt_review(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(attempt_id): Path<Uuid>,
) -> ApiResult<Json<AttemptReviewResponse>> {
    Ok(Json(service(&state).attempt_review(attempt_id, auth.user_id).await?))
}

pub async fn my_results(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(quiz_id): Path<Uuid>,
) -> ApiResult<Json<MyResultsResponse>> {
    Ok(Json(service(&state).my_results(quiz_id, auth.user_id).await?))
}

pub async fn quiz_analytics(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(quiz_id): Path<Uuid>,
) -> ApiResult<Json<QuizAnalyticsResponse>> {
    Ok(Json(service(&state).quiz_analytics(quiz_id, auth.user_id).await?))
}
