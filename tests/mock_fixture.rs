use serde::Deserialize;
use standuppulse::{
    api::dto::{
        AttemptSessionResponse, AvailableQuizListResponse, MyAttemptsResponse,
        SubmitAttemptRequest, SubmitAttemptResponse,
    },
    domain::participation::{QuestionKey, validate_and_score},
};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
struct Fixture {
    upstream_records: UpstreamRecords,
    submission_request: SubmitAttemptRequest,
    mock_api_responses: MockApiResponses,
    persisted_participation_records: PersistedRecords,
}

#[derive(Debug, Deserialize)]
struct UpstreamRecords {
    users: Vec<UserRecord>,
    teams: Vec<TeamRecord>,
    team_members: Vec<TeamMemberRecord>,
    quizzes: Vec<QuizRecord>,
    quiz_revisions: Vec<RevisionRecord>,
    question_versions: Vec<QuestionRecord>,
    quiz_eligibilities: Vec<EligibilityRecord>,
}

#[derive(Debug, Deserialize)]
struct UserRecord {
    id: Uuid,
}
#[derive(Debug, Deserialize)]
struct TeamRecord {
    id: Uuid,
}
#[derive(Debug, Deserialize)]
struct TeamMemberRecord {
    team_id: Uuid,
    user_id: Uuid,
}
#[derive(Debug, Deserialize)]
struct QuizRecord {
    id: Uuid,
    team_id: Uuid,
    max_attempts: u8,
}
#[derive(Debug, Deserialize)]
struct RevisionRecord {
    id: Uuid,
    quiz_id: Uuid,
}
#[derive(Debug, Deserialize)]
struct QuestionRecord {
    id: Uuid,
    quiz_revision_id: Uuid,
    options: Vec<String>,
    correct_option_index: u16,
}
#[derive(Debug, Deserialize)]
struct EligibilityRecord {
    quiz_id: Uuid,
    user_id: Uuid,
}

#[derive(Debug, Deserialize)]
struct MockApiResponses {
    available_quizzes: AvailableQuizListResponse,
    attempt_session: AttemptSessionResponse,
    submission: SubmitAttemptResponse,
    identical_retry: SubmitAttemptResponse,
    my_attempts: MyAttemptsResponse,
}

#[derive(Debug, Deserialize)]
struct PersistedRecords {
    attempt_sessions: Vec<serde_json::Value>,
    quiz_attempts: Vec<serde_json::Value>,
    attempt_answers: Vec<serde_json::Value>,
}

fn fixture() -> Fixture {
    serde_json::from_str(include_str!("../fixtures/participation_flow.json"))
        .expect("participation fixture must follow the shared JSON contract")
}

#[test]
fn fixture_preserves_cross_service_relationships() {
    let fixture = fixture();
    let user = &fixture.upstream_records.users[0];
    let team = &fixture.upstream_records.teams[0];
    let membership = &fixture.upstream_records.team_members[0];
    let quiz = &fixture.upstream_records.quizzes[0];
    let revision = &fixture.upstream_records.quiz_revisions[0];
    let eligibility = &fixture.upstream_records.quiz_eligibilities[0];

    assert_eq!(membership.user_id, user.id);
    assert_eq!(membership.team_id, team.id);
    assert_eq!(quiz.team_id, team.id);
    assert_eq!(quiz.max_attempts, 1);
    assert_eq!(revision.quiz_id, quiz.id);
    assert_eq!(eligibility.quiz_id, quiz.id);
    assert_eq!(eligibility.user_id, user.id);
    assert!(
        fixture
            .upstream_records
            .question_versions
            .iter()
            .all(|question| question.quiz_revision_id == revision.id)
    );
}

#[test]
fn mock_submission_matches_deterministic_domain_score() {
    let fixture = fixture();
    let questions: Vec<QuestionKey> = fixture
        .upstream_records
        .question_versions
        .iter()
        .map(|question| QuestionKey {
            question_id: question.id,
            option_count: question.options.len(),
            correct_option_index: question.correct_option_index,
        })
        .collect();
    let scored = validate_and_score(&questions, &fixture.submission_request.answers).unwrap();
    let score = scored.iter().filter(|answer| answer.is_correct).count() as u32;

    assert_eq!(score, fixture.mock_api_responses.submission.score);
    assert_eq!(
        questions.len() as u32,
        fixture.mock_api_responses.submission.total_questions
    );
    assert_eq!(fixture.mock_api_responses.submission.percentage, 50.0);
    assert!(!fixture.mock_api_responses.submission.idempotent_replay);
    assert!(fixture.mock_api_responses.identical_retry.idempotent_replay);
    assert_eq!(
        fixture.mock_api_responses.identical_retry.attempt_id,
        fixture.mock_api_responses.submission.attempt_id
    );
}

#[test]
fn mock_api_results_deserialize_and_never_leak_answer_keys() {
    let fixture = fixture();
    assert_eq!(
        fixture.mock_api_responses.available_quizzes.quizzes.len(),
        1
    );
    assert_eq!(
        fixture.mock_api_responses.attempt_session.questions.len(),
        2
    );
    assert_eq!(fixture.mock_api_responses.my_attempts.attempts.len(), 1);

    let public_json = serde_json::to_value(&fixture.mock_api_responses.attempt_session.questions)
        .unwrap()
        .to_string();
    assert!(!public_json.contains("correct_option_index"));
    assert!(!public_json.contains("explanation"));
    assert!(!public_json.contains("is_correct"));

    assert_eq!(
        fixture
            .persisted_participation_records
            .attempt_sessions
            .len(),
        1
    );
    assert_eq!(
        fixture.persisted_participation_records.quiz_attempts.len(),
        1
    );
    assert_eq!(
        fixture
            .persisted_participation_records
            .attempt_answers
            .len(),
        2
    );
}
