use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::ai_types::{BookCitationGraph, BookPublication, ScientificIdentifiers};

// ─── BookCard ───────────────────────────────────────────────

/// Full book card — the canonical representation of a book in Omniscope.
/// Stored as `{uuid}.json` on disk, denormalized into SQLite for queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookCard {
    pub id: Uuid,
    pub version: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    pub metadata: BookMetadata,

    /// Scientific identifiers: DOI, arXiv, ISBN-13, PMID, etc.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identifiers: Option<ScientificIdentifiers>,

    /// Publication metadata: journal, conference, doc_type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publication: Option<BookPublication>,

    /// Citation graph (populated in Phase 3).
    #[serde(default)]
    pub citation_graph: BookCitationGraph,

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
}

impl BookCard {
    /// Create a new BookCard with minimal required fields.
    pub fn new(title: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::now_v7(),
            version: 1,
            created_at: now,
            updated_at: now,
            metadata: BookMetadata {
                title: title.into(),
                ..Default::default()
            },
            identifiers: None,
            publication: None,
            citation_graph: BookCitationGraph::default(),
            file: None,
            organization: BookOrganization::default(),
            ai: BookAi::default(),
            web: BookWeb::default(),
            notes: Vec::new(),
        }
    }
}

// ─── Metadata ───────────────────────────────────────────────

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

// ─── File ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookFile {
    pub path: String,
    pub format: FileFormat,
    pub size_bytes: u64,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash_sha256: Option<String>,

    pub added_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileFormat {
    Pdf,
    Epub,
    Djvu,
    Mobi,
    Fb2,
    Txt,
    Html,
    Azw3,
    Cbz,
    Cbr,
    Other,
}

impl std::fmt::Display for FileFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Pdf => "pdf",
            Self::Epub => "epub",
            Self::Djvu => "djvu",
            Self::Mobi => "mobi",
            Self::Fb2 => "fb2",
            Self::Txt => "txt",
            Self::Html => "html",
            Self::Azw3 => "azw3",
            Self::Cbz => "cbz",
            Self::Cbr => "cbr",
            Self::Other => "other",
        };
        write!(f, "{s}")
    }
}

impl FileFormat {
    /// Detect format from file extension.
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "pdf" => Self::Pdf,
            "epub" => Self::Epub,
            "djvu" => Self::Djvu,
            "mobi" => Self::Mobi,
            "fb2" => Self::Fb2,
            "txt" | "text" => Self::Txt,
            "html" | "htm" => Self::Html,
            "azw3" => Self::Azw3,
            "cbz" => Self::Cbz,
            "cbr" => Self::Cbr,
            _ => Self::Other,
        }
    }
}

// ─── Organization ──────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BookOrganization {
    #[serde(default)]
    pub libraries: Vec<String>,

    #[serde(default)]
    pub folders: Vec<String>,

    #[serde(default)]
    pub tags: Vec<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating: Option<u8>,

    #[serde(default)]
    pub read_status: ReadStatus,

    #[serde(default)]
    pub priority: Priority,

    #[serde(default)]
    pub custom_fields: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReadStatus {
    #[default]
    Unread,
    Reading,
    Read,
    /// Did Not Finish
    Dnf,
}

impl std::fmt::Display for ReadStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unread => write!(f, "unread"),
            Self::Reading => write!(f, "reading"),
            Self::Read => write!(f, "read"),
            Self::Dnf => write!(f, "dnf"),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    #[default]
    None,
    Low,
    Medium,
    High,
}

// ─── AI ────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BookAi {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,

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

// ─── Web ───────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BookWeb {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openlibrary_id: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goodreads_id: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_url: Option<String>,

    #[serde(default)]
    pub sources: Vec<WebSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSource {
    pub name: String,
    pub url: String,
}

// ─── Notes ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookNote {
    pub id: Uuid,
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
            id: Uuid::now_v7(),
            text: text.into(),
            created_at: Utc::now(),
            author: "human".to_string(),
        }
    }
}

// ─── BookSummary (lightweight for list display) ────────────

/// Lightweight struct used for list rendering — not the full card.
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

// ─── Tests ─────────────────────────────────────────────────

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

    #[test]
    fn test_file_format_from_extension() {
        assert_eq!(FileFormat::from_extension("pdf"), FileFormat::Pdf);
        assert_eq!(FileFormat::from_extension("PDF"), FileFormat::Pdf);
        assert_eq!(FileFormat::from_extension("epub"), FileFormat::Epub);
        assert_eq!(FileFormat::from_extension("djvu"), FileFormat::Djvu);
        assert_eq!(FileFormat::from_extension("xyz"), FileFormat::Other);
    }

    #[test]
    fn test_book_summary_view_from_card() {
        let mut card = BookCard::new("Test");
        card.metadata.authors = vec!["A".to_string()];
        card.metadata.year = Some(2024);
        card.organization.rating = Some(4);
        card.organization.tags = vec!["tag1".to_string()];

        let summary = BookSummaryView::from(&card);
        assert_eq!(summary.title, "Test");
        assert_eq!(summary.year, Some(2024));
        assert!(!summary.has_file);
    }
}
