pub(crate) fn is_within_session_window(now: i64, opens_at: i64, closes_at: i64) -> bool {
    opens_at <= now && now < closes_at
}

#[cfg(test)]
mod tests {
    use super::is_within_session_window;

    #[test]
    fn checks_both_ends_of_the_quiz_window() {
        let opens_at = 0;
        let closes_at = 15 * 60;

        assert!(!is_within_session_window(-1, opens_at, closes_at));
        assert!(is_within_session_window(0, opens_at, closes_at));
        assert!(is_within_session_window(899, opens_at, closes_at));
        assert!(!is_within_session_window(900, opens_at, closes_at));
    }
}