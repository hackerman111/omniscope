use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,

    #[serde(default)]
    pub interests: Vec<String>,

    #[serde(default = "default_daily_budget")]
    pub daily_token_budget: u64,

    #[serde(default = "default_monthly_budget")]
    pub monthly_token_budget: u64,

    #[serde(default)]
    pub reading_history: Vec<Uuid>,

    #[serde(default)]
    pub tokens_used_today: u64,

    #[serde(default)]
    pub tokens_used_month: u64,
}

fn default_daily_budget() -> u64 {
    100_000
}

fn default_monthly_budget() -> u64 {
    2_000_000
}

impl Default for UserProfile {
    fn default() -> Self {
        Self {
            name: None,
            email: None,
            interests: Vec::new(),
            daily_token_budget: default_daily_budget(),
            monthly_token_budget: default_monthly_budget(),
            reading_history: Vec::new(),
            tokens_used_today: 0,
            tokens_used_month: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_profile_defaults() {
        let profile = UserProfile::default();
        assert_eq!(profile.daily_token_budget, 100_000);
        assert_eq!(profile.monthly_token_budget, 2_000_000);
    }
}
