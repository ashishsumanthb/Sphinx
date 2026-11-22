use std::collections::HashMap;
use uuid::Uuid;
pub(crate) fn competition_rank(score: usize, best_scores: &[usize]) -> usize {
    let mut rank = 1;

    for &other_score in best_scores {
        if other_score > score {
            rank += 1;
        }
    }

    rank
}
pub(crate) fn best_scores_for_revision(attempts:&[(Uuid,usize)])->HashMap<Uuid,usize>{
    let mut best_scores = HashMap::new();

    
    for &(participant_id,score) in attempts{
        let best = best_scores.entry(participant_id).or_insert(score);
        if score > *best{
            *best = score;
        }
    }
    best_scores 
}
#[cfg(test)]
mod tests {
    use super::competition_rank;

    #[test]
    fn tied_scores_share_a_rank_and_skip_the_next_place() {
        let best_scores = [9, 9, 8];

        assert_eq!(competition_rank(9, &best_scores), 1);
        assert_eq!(competition_rank(8, &best_scores), 3);
    }
    #[test]
    fn keeps_each_participants_highest_score() {
        use super::best_scores_for_revision;
        use uuid::Uuid;

        let ashish = Uuid::from_u128(1);
        let teammate = Uuid::from_u128(2);

        let attempts = [(ashish, 9), (teammate, 5), (ashish, 6), (teammate, 8)];
        let best = best_scores_for_revision(&attempts);

        assert_eq!(best.len(), 2);
        assert_eq!(best.get(&ashish), Some(&9));
        assert_eq!(best.get(&teammate), Some(&8));
    }
}