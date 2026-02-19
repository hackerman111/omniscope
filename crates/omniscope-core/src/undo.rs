use crate::BookCard;
use chrono::{DateTime, Utc};

/// Action performed that can be undone.
#[derive(Debug, Clone)]
pub enum UndoAction {
    /// Save these cards (restore old state after a change or delete)
    UpsertCards(Vec<BookCard>),
    /// Delete these cards (revert an addition)
    DeleteCards(Vec<BookCard>),
}

/// An undoable book-modification snapshot.
#[derive(Debug, Clone)]
pub struct UndoEntry {
    pub description: String,
    pub action: UndoAction,
    pub timestamp: DateTime<Utc>,
}
