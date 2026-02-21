use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LibraryMap {
    pub generated_at: String,
    pub stats: LibraryStats,
    pub libraries: std::collections::HashMap<String, LibraryBrief>,
    pub tag_cloud: std::collections::HashMap<String, u32>,
    pub books: Vec<BookSummaryCompact>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LibraryStats {
    pub total: usize,
    pub unread: usize,
    pub reading: usize,
    pub read: usize,
    pub dnf: usize,
    pub with_file: usize,
    pub with_summary: usize,
    pub pdf: usize,
    pub epub: usize,
    pub other_format: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryBrief {
    pub book_count: u32,
    pub unread_count: u32,
    pub top_tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookSummaryCompact {
    pub id: Uuid,
    pub title: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub authors: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tags: Vec<String>,
    pub status: String,
    pub frecency: f64,
}
