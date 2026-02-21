use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BookMetadata {
    pub title: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,

    #[serde(default)]
    pub authors: Vec<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,

    #[serde(default)]
    pub isbn: Vec<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pages: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edition: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series_index: Option<f32>,
}

impl BookMetadata {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookNote {
    pub id: uuid::Uuid,
    pub text: String,
    pub created_at: DateTime<Utc>,

    #[serde(default = "default_note_author")]
    pub author: String,
}

fn default_note_author() -> String {
    "human".to_string()
}

impl BookNote {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::now_v7(),
            text: text.into(),
            created_at: Utc::now(),
            author: "human".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_new() {
        let m = BookMetadata::new("Test Book");
        assert_eq!(m.title, "Test Book");
        assert!(m.authors.is_empty());
    }
}
