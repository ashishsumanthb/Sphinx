use crate::domain::quiz::{AttemptSession, QuizRevision, SessionStatus};
use crate::domain::ranking::{best_scores_for_revision, competition_rank};
use crate::services::submission_service::SavedSubmission;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Standing {
    pub(crate) best_score: usize,
    pub(crate) rank: usize,
}

pub(crate) fn participant_standing(
    revision: &QuizRevision,
    user_id: Uuid,
    sessions: &[AttemptSession],
    submissions: &HashMap<Uuid, SavedSubmission>,
) -> Option<Standing> {
    let mut attempts = Vec::new();

    for session in sessions {
        if session.quiz_id == revision.quiz_id
            && session.quiz_revision_id == revision.id
            && matches!(session.status, SessionStatus::Submitted)
        {
            if let Some(saved) = submissions.get(&session.id) {
                attempts.push((session.user_id, saved.score.score));
            }
        }
    }

    let best_by_user = best_scores_for_revision(&attempts);
    let best_score = *best_by_user.get(&user_id)?;
    let scores: Vec<usize> = best_by_user.values().copied().collect();

    Some(Standing {
        best_score,
        rank: competition_rank(best_score, &scores),
    })
}