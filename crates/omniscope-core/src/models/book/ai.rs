use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BookAi {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tldr: Option<String>,

    #[serde(default)]
    pub table_of_contents: Vec<TocEntry>,

    #[serde(default)]
    pub key_topics: Vec<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub difficulty: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_notes: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indexed_at: Option<DateTime<Utc>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index_version: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding_model: Option<String>,

    #[serde(default)]
    pub embedding_stored: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TocEntry {
    pub chapter: u32,
    pub title: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
}
