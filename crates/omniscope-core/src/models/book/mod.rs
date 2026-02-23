mod ai;
mod file;
mod metadata;
mod organization;
mod web;

pub use ai::*;
pub use file::*;
pub use metadata::*;
pub use organization::*;
pub use web::*;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use super::{BookCitationGraph, BookOpenAccessInfo, BookPublication, ScientificIdentifiers};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookCard {
    pub id: Uuid,
    pub version: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    pub metadata: BookMetadata,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identifiers: Option<ScientificIdentifiers>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publication: Option<BookPublication>,

    #[serde(default)]
    pub citation_graph: BookCitationGraph,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_access: Option<BookOpenAccessInfo>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<BookFile>,

    #[serde(default)]
    pub organization: BookOrganization,

    #[serde(default)]
    pub ai: BookAi,

    #[serde(default)]
    pub web: BookWeb,

    #[serde(default)]
    pub notes: Vec<BookNote>,

    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata_sources: HashMap<String, String>,
}

impl BookCard {
    pub fn new(title: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::now_v7(),
            version: 1,
            created_at: now,
            updated_at: now,
            metadata: BookMetadata::new(title),
            identifiers: None,
            publication: None,
            citation_graph: BookCitationGraph::default(),
            open_access: None,
            file: None,
            organization: BookOrganization::default(),
            ai: BookAi::default(),
            web: BookWeb::default(),
            notes: Vec::new(),
            metadata_sources: HashMap::new(),
        }
    }

    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookSummaryView {
    pub id: Uuid,
    pub title: String,
    pub authors: Vec<String>,
    pub year: Option<i32>,
    pub format: Option<FileFormat>,
    pub rating: Option<u8>,
    pub read_status: ReadStatus,
    pub tags: Vec<String>,
    pub has_file: bool,
    pub frecency_score: f64,
}

impl From<&BookCard> for BookSummaryView {
    fn from(card: &BookCard) -> Self {
        Self {
            id: card.id,
            title: card.metadata.title.clone(),
            authors: card.metadata.authors.clone(),
            year: card.metadata.year,
            format: card.file.as_ref().map(|f| f.format),
            rating: card.organization.rating,
            read_status: card.organization.read_status,
            tags: card.organization.tags.clone(),
            has_file: card.file.is_some(),
            frecency_score: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_book_card_new() {
        let card = BookCard::new("The Rust Programming Language");
        assert_eq!(card.metadata.title, "The Rust Programming Language");
        assert_eq!(card.version, 1);
        assert!(card.metadata.authors.is_empty());
        assert!(card.file.is_none());
        assert_eq!(card.organization.read_status, ReadStatus::Unread);
    }

    #[test]
    fn test_book_card_json_roundtrip() {
        let mut card = BookCard::new("Test Book");
        card.metadata.authors = vec!["Author One".to_string(), "Author Two".to_string()];
        card.metadata.year = Some(2023);
        card.organization.tags = vec!["rust".to_string(), "programming".to_string()];
        card.organization.rating = Some(5);
        card.organization.read_status = ReadStatus::Read;

        let json = serde_json::to_string_pretty(&card).unwrap();
        let restored: BookCard = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.id, card.id);
        assert_eq!(restored.metadata.title, "Test Book");
        assert_eq!(restored.metadata.authors.len(), 2);
        assert_eq!(restored.metadata.year, Some(2023));
        assert_eq!(restored.organization.tags.len(), 2);
        assert_eq!(restored.organization.rating, Some(5));
        assert_eq!(restored.organization.read_status, ReadStatus::Read);
    }
}
