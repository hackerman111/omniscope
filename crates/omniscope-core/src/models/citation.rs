use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BookCitationGraph {
    #[serde(default)]
    pub citation_count: u32,

    #[serde(default)]
    pub reference_count: u32,

    #[serde(default)]
    pub influential_citation_count: u32,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<DateTime<Utc>>,

    #[serde(default)]
    pub cited_by_ids: Vec<Uuid>,

    #[serde(default)]
    pub references_ids: Vec<Uuid>,

    #[serde(default)]
    pub references: Vec<String>,

    #[serde(default)]
    pub cited_by_sample: Vec<String>,
}
