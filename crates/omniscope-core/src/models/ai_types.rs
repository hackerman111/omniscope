use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ─── ScientificIdentifiers ─────────────────────────────────

/// Typed scientific identifiers attached to a book card.
/// All fields optional — fill what is known.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ScientificIdentifiers {
    /// Digital Object Identifier, e.g. "10.1145/1327452.1327492"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doi: Option<String>,

    /// arXiv identifier, e.g. "1706.03762"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arxiv_id: Option<String>,

    /// ISBN-13 (13-digit), e.g. "9780132350884"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isbn13: Option<String>,

    /// ISBN-10 (legacy 10-digit)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isbn10: Option<String>,

    /// PubMed ID (PMID), e.g. "12345678"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pmid: Option<String>,

    /// PubMed Central ID (PMCID), e.g. "PMC1234567"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pmcid: Option<String>,

    /// OpenAlex work ID, e.g. "W2741809807"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openalex_id: Option<String>,

    /// Semantic Scholar corpus ID
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_scholar_id: Option<String>,

    /// MAG (Microsoft Academic Graph) ID
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mag_id: Option<String>,
}

impl ScientificIdentifiers {
    pub fn is_empty(&self) -> bool {
        self.doi.is_none()
            && self.arxiv_id.is_none()
            && self.isbn13.is_none()
            && self.isbn10.is_none()
            && self.pmid.is_none()
            && self.pmcid.is_none()
            && self.openalex_id.is_none()
            && self.semantic_scholar_id.is_none()
            && self.mag_id.is_none()
    }
}

// ─── DocumentType ──────────────────────────────────────────

/// Full taxonomy of document types in Omniscope.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentType {
    /// General book (default)
    #[default]
    Book,
    /// Journal article
    Article,
    /// Conference paper (proceedings)
    ConferencePaper,
    /// Preprint (e.g. arXiv)
    Preprint,
    /// PhD / master's thesis or dissertation
    Thesis,
    /// Technical report
    Report,
    /// Dataset
    Dataset,
    /// Software / code
    Software,
    /// Patent
    Patent,
    /// Standard (ISO, RFC, etc.)
    Standard,
    /// Book chapter
    Chapter,
    /// Magazine article
    MagazineArticle,
    /// Web page / blog post
    WebPage,
    /// Other / unknown
    Other,
}

impl std::fmt::Display for DocumentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use DocumentType::*;
        let s = match self {
            Book => "book",
            Article => "article",
            ConferencePaper => "conference_paper",
            Preprint => "preprint",
            Thesis => "thesis",
            Report => "report",
            Dataset => "dataset",
            Software => "software",
            Patent => "patent",
            Standard => "standard",
            Chapter => "chapter",
            MagazineArticle => "magazine_article",
            WebPage => "web_page",
            Other => "other",
        };
        write!(f, "{s}")
    }
}

// ─── BookPublication ───────────────────────────────────────

/// Publication metadata — journal, conference, venue.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BookPublication {
    #[serde(default)]
    pub doc_type: DocumentType,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub journal: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conference: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub venue: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pages: Option<String>,
}

// ─── BookCitationGraph ─────────────────────────────────────

/// Citation metrics and reference lists (stub — populated by Phase 3).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BookCitationGraph {
    #[serde(default)]
    pub citation_count: u32,

    /// UUIDs of books in library that reference this one (cited_by)
    #[serde(default)]
    pub cited_by_ids: Vec<Uuid>,

    /// UUIDs of books in library that this one references
    #[serde(default)]
    pub references_ids: Vec<Uuid>,
}

// ─── LibraryMap ────────────────────────────────────────────

/// L1 AI memory: compact library representation sent to AI on each query.
/// Target token budget: 4-8K tokens.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LibraryMap {
    /// Generation timestamp (RFC3339)
    pub generated_at: String,

    pub stats: LibraryStats,

    /// Per-library breakdown
    pub libraries: std::collections::HashMap<String, LibraryBrief>,

    /// Tag → book count
    pub tag_cloud: std::collections::HashMap<String, u32>,

    /// Compact book summaries
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

/// Ultra-compact summary for use in LibraryMap (minimal token footprint).
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

// ─── OmniscopeAction ───────────────────────────────────────

/// All possible actions that the Omniscope AI can emit.
/// Corresponds to AI_SYSTEM §2.1 — the Action Protocol.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OmniscopeAction {
    // ── Informational ──────────────────────────────────────
    ShowMessage { message: String, level: MessageLevel },
    ShowBookList { ids: Vec<Uuid>, title: Option<String> },

    // ── Navigational ───────────────────────────────────────
    NavigateTo { target: NavigationTarget },
    OpenSearch { query: String },
    OpenFile { book_id: Uuid },

    // ── Mutating ───────────────────────────────────────────
    CreateCard { card: serde_json::Value },
    UpdateCard { id: Uuid, fields: serde_json::Value },
    BatchUpdate { ids: Vec<Uuid>, fields: serde_json::Value },
    DeleteCard { id: Uuid },
    AddTag { book_id: Uuid, tag: String },
    RemoveTag { book_id: Uuid, tag: String },
    MoveBooks { ids: Vec<Uuid>, to_library: Option<String>, to_folder: Option<String> },

    // ── External ───────────────────────────────────────────
    EnrichMetadata { book_id: Uuid, sources: Vec<String> },
    FetchAndAdd { query: String, source: String },
    ExtractReferences { book_id: Uuid },

    // ── Composite ──────────────────────────────────────────
    Transaction { actions: Vec<OmniscopeAction> },
    ConfirmThen { message: String, action: Box<OmniscopeAction> },
    AskUser { question: String, context: Option<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageLevel { Info, Warning, Error, Success }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NavigationTarget {
    AllBooks,
    Library(String),
    Tag(String),
    Folder(String),
    BookCard(Uuid),
    Search(String),
}

// ─── ActionLogEntry ────────────────────────────────────────

/// Audit log entry for every AI action. Stored in action_log table and action_log.jsonl.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionLogEntry {
    pub id: Uuid,
    pub action_type: String,
    /// Serialized OmniscopeAction
    pub payload: serde_json::Value,
    /// Serialized state before action (for undo). None for read-only actions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_before: Option<serde_json::Value>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub reversed: bool,
}

impl ActionLogEntry {
    pub fn new(action: &OmniscopeAction) -> Self {
        Self {
            id: Uuid::now_v7(),
            action_type: action.type_name().to_string(),
            payload: serde_json::to_value(action).unwrap_or(serde_json::Value::Null),
            snapshot_before: None,
            created_at: chrono::Utc::now(),
            reversed: false,
        }
    }
}

impl OmniscopeAction {
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::ShowMessage { .. } => "show_message",
            Self::ShowBookList { .. } => "show_book_list",
            Self::NavigateTo { .. } => "navigate_to",
            Self::OpenSearch { .. } => "open_search",
            Self::OpenFile { .. } => "open_file",
            Self::CreateCard { .. } => "create_card",
            Self::UpdateCard { .. } => "update_card",
            Self::BatchUpdate { .. } => "batch_update",
            Self::DeleteCard { .. } => "delete_card",
            Self::AddTag { .. } => "add_tag",
            Self::RemoveTag { .. } => "remove_tag",
            Self::MoveBooks { .. } => "move_books",
            Self::EnrichMetadata { .. } => "enrich_metadata",
            Self::FetchAndAdd { .. } => "fetch_and_add",
            Self::ExtractReferences { .. } => "extract_references",
            Self::Transaction { .. } => "transaction",
            Self::ConfirmThen { .. } => "confirm_then",
            Self::AskUser { .. } => "ask_user",
        }
    }
}

// ─── UserProfile ───────────────────────────────────────────

/// User profile for AI personalization. Stored in config dir.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,

    /// Topics of interest (for recommendations)
    #[serde(default)]
    pub interests: Vec<String>,

    /// Daily AI token budget (tokens)
    #[serde(default = "default_daily_budget")]
    pub daily_token_budget: u64,

    /// Monthly AI token budget (tokens)
    #[serde(default = "default_monthly_budget")]
    pub monthly_token_budget: u64,

    /// Recent book IDs (last 100 opened)
    #[serde(default)]
    pub reading_history: Vec<Uuid>,

    /// AI tokens used today
    #[serde(default)]
    pub tokens_used_today: u64,

    /// AI tokens used this month
    #[serde(default)]
    pub tokens_used_month: u64,
}

fn default_daily_budget() -> u64 { 100_000 }
fn default_monthly_budget() -> u64 { 2_000_000 }

// ─── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scientific_identifiers_roundtrip() {
        let id = ScientificIdentifiers {
            doi: Some("10.1145/1327452.1327492".to_string()),
            arxiv_id: Some("1706.03762".to_string()),
            isbn13: Some("9780132350884".to_string()),
            ..Default::default()
        };
        let json = serde_json::to_string(&id).unwrap();
        let restored: ScientificIdentifiers = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, id);
        assert_eq!(restored.doi.as_deref(), Some("10.1145/1327452.1327492"));
        assert_eq!(restored.arxiv_id.as_deref(), Some("1706.03762"));
    }

    #[test]
    fn test_document_type_roundtrip() {
        for dt in [
            DocumentType::Book, DocumentType::Article, DocumentType::Preprint,
            DocumentType::Thesis, DocumentType::ConferencePaper,
        ] {
            let json = serde_json::to_string(&dt).unwrap();
            let restored: DocumentType = serde_json::from_str(&json).unwrap();
            assert_eq!(restored, dt);
        }
    }

    #[test]
    fn test_library_map_roundtrip() {
        let map = LibraryMap {
            generated_at: "2026-01-01T00:00:00Z".to_string(),
            stats: LibraryStats { total: 42, unread: 10, ..Default::default() },
            ..Default::default()
        };
        let json = serde_json::to_string(&map).unwrap();
        let restored: LibraryMap = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.stats.total, 42);
    }

    #[test]
    fn test_omniscope_action_roundtrip() {
        let action = OmniscopeAction::UpdateCard {
            id: Uuid::now_v7(),
            fields: serde_json::json!({"rating": 5}),
        };
        let json = serde_json::to_string(&action).unwrap();
        let restored: OmniscopeAction = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.type_name(), "update_card");
    }

    #[test]
    fn test_user_profile_roundtrip() {
        let mut profile = UserProfile::default();
        profile.name = Some("Alice".to_string());
        profile.interests = vec!["rust".to_string(), "ai".to_string()];
        profile.tokens_used_today = 1234;
        let json = serde_json::to_string(&profile).unwrap();
        let restored: UserProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.name, Some("Alice".to_string()));
        assert_eq!(restored.tokens_used_today, 1234);
    }

    #[test]
    fn test_action_log_entry_new() {
        let action = OmniscopeAction::ShowMessage {
            message: "Hello".to_string(),
            level: MessageLevel::Info,
        };
        let entry = ActionLogEntry::new(&action);
        assert_eq!(entry.action_type, "show_message");
        assert!(!entry.reversed);
    }
}
