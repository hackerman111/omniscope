/// Zoxide-like frecency scoring for books.
///
/// frecency = frequency × recency_weight
/// This ensures recently and frequently accessed books rank higher.
use chrono::{DateTime, Utc};

/// Frecency data for a book.
#[derive(Debug, Clone)]
pub struct FrecencyData {
    /// Number of times this book was accessed (opened, viewed, searched for).
    pub access_count: u32,
    /// Last time the book was accessed.
    pub last_accessed: DateTime<Utc>,
}

impl Default for FrecencyData {
    fn default() -> Self {
        Self {
            access_count: 0,
            last_accessed: Utc::now(),
        }
    }
}

/// Calculate frecency score for a book.
///
/// Algorithm follows zoxide:
/// - recency weight is based on how many days ago the last access was
/// - the final score combines access count and recency
pub fn calculate_frecency(access_count: u32, last_accessed: DateTime<Utc>) -> f64 {
    let now = Utc::now();
    let days_ago = (now - last_accessed).num_hours() as f64 / 24.0;

    let recency_weight = if days_ago < 1.0 {
        8.0
    } else if days_ago < 4.0 {
        6.0
    } else if days_ago < 14.0 {
        4.0
    } else if days_ago < 31.0 {
        2.0
    } else if days_ago < 90.0 {
        1.0
    } else {
        0.5
    };

    let freq = (access_count as f64).max(1.0);
    (freq * recency_weight).sqrt()
}

/// Boost a search score with frecency.
pub fn boost_with_frecency(base_score: u32, frecency_score: f64) -> f64 {
    let base = base_score as f64;
    // Frecency contributes up to 20% of the final score
    base + (base * 0.2 * frecency_score / 10.0).min(base * 0.5)
}

/// Calculate new frecency score after a new access.
pub fn record_access(_current_score: f64, access_count: u32) -> (f64, u32) {
    let new_count = access_count + 1;
    let new_score = calculate_frecency(new_count, Utc::now());
    (new_score, new_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_recent_scores_higher() {
        let now = Utc::now();
        let yesterday = now - Duration::hours(12);
        let last_week = now - Duration::days(7);
        let last_month = now - Duration::days(40);

        let score_recent = calculate_frecency(5, yesterday);
        let score_week = calculate_frecency(5, last_week);
        let score_month = calculate_frecency(5, last_month);

        assert!(score_recent > score_week, "recent > week");
        assert!(score_week > score_month, "week > month");
    }

    #[test]
    fn test_frequent_scores_higher() {
        let now = Utc::now();
        let high_freq = calculate_frecency(20, now);
        let low_freq = calculate_frecency(2, now);

        assert!(high_freq > low_freq, "high freq > low freq");
    }

    #[test]
    fn test_boost_with_frecency() {
        let boosted = boost_with_frecency(100, 5.0);
        assert!(boosted > 100.0, "frecency should boost base score");
        assert!(boosted < 200.0, "boost shouldn't double the score");
    }

    #[test]
    fn test_record_access() {
        let (new_score, new_count) = record_access(0.0, 0);
        assert!(new_score > 0.0);
        assert_eq!(new_count, 1);
    }
}
