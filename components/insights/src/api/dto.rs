use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AttemptResult {
    pub attempt_id: Uuid,
    pub quiz_revision_id: Uuid,
    pub quiz_revision_number: i32,
    pub attempt_number: i16,
    pub score: i32,
    pub total_questions: i32,
    pub percentage: f64,
    pub submitted_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct RevisionResult {
    pub quiz_revision_id: Uuid,
    pub quiz_revision_number: i32,
    pub best_percentage: f64,
    pub rank: i64,
}

#[derive(Debug, Serialize)]
pub struct MyResultsResponse {
    pub quiz_id: Uuid,
    pub attempts: Vec<AttemptResult>,
    pub revision_results: Vec<RevisionResult>,
}
