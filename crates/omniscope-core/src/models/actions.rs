use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OmniscopeAction {
    ShowMessage {
        message: String,
        level: MessageLevel,
    },
    ShowBookList {
        ids: Vec<Uuid>,
        title: Option<String>,
    },

    NavigateTo {
        target: NavigationTarget,
    },
    OpenSearch {
        query: String,
    },
    OpenFile {
        book_id: Uuid,
    },

    CreateCard {
        card: serde_json::Value,
    },
    UpdateCard {
        id: Uuid,
        fields: serde_json::Value,
    },
    BatchUpdate {
        ids: Vec<Uuid>,
        fields: serde_json::Value,
    },
    DeleteCard {
        id: Uuid,
    },
    AddTag {
        book_id: Uuid,
        tag: String,
    },
    RemoveTag {
        book_id: Uuid,
        tag: String,
    },
    MoveBooks {
        ids: Vec<Uuid>,
        to_library: Option<String>,
        to_folder: Option<String>,
    },

    EnrichMetadata {
        book_id: Uuid,
        sources: Vec<String>,
    },
    FetchAndAdd {
        query: String,
        source: String,
    },
    ExtractReferences {
        book_id: Uuid,
    },

    Transaction {
        actions: Vec<OmniscopeAction>,
    },
    ConfirmThen {
        message: String,
        action: Box<OmniscopeAction>,
    },
    AskUser {
        question: String,
        context: Option<String>,
    },
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageLevel {
    Info,
    Warning,
    Error,
    Success,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionLogEntry {
    pub id: Uuid,
    pub action_type: String,
    pub payload: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_before: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub reversed: bool,
}

impl ActionLogEntry {
    pub fn new(action: &OmniscopeAction) -> Self {
        Self {
            id: Uuid::now_v7(),
            action_type: action.type_name().to_string(),
            payload: serde_json::to_value(action).unwrap_or(serde_json::Value::Null),
            snapshot_before: None,
            created_at: Utc::now(),
            reversed: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_type_name() {
        let action = OmniscopeAction::ShowMessage {
            message: "Hello".to_string(),
            level: MessageLevel::Info,
        };
        assert_eq!(action.type_name(), "show_message");
    }
}
