use super::{App, SortKey, UndoEntry};
use omniscope_core::BookCard;

impl App {
    // ─── Phase 1: Sorting ───────────────────────────────────

    /// Cycle to the next sort order and re-apply.
    pub fn cycle_sort(&mut self) {
        self.sort_key = self.sort_key.next();
        self.apply_sort();
        self.status_message = format!("Sort: {}", self.sort_key.label());
    }

    /// Sort `self.books` according to `self.sort_key`.
    pub fn apply_sort(&mut self) {
        match self.sort_key {
            SortKey::UpdatedDesc  => {} // default DB order
            SortKey::UpdatedAsc   => self.books.reverse(),
            SortKey::TitleAsc     => self.books.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase())),
            SortKey::YearDesc     => self.books.sort_by(|a, b| b.year.cmp(&a.year)),
            SortKey::RatingDesc   => self.books.sort_by(|a, b| b.rating.cmp(&a.rating)),
            SortKey::FrecencyDesc => self.books.sort_by(|a, b| b.frecency_score.partial_cmp(&a.frecency_score).unwrap_or(std::cmp::Ordering::Equal)),
        }
        self.selected_index = 0;
    }

    // ─── Phase 1: Undo / Redo ───────────────────────────────

    /// Push a snapshot onto the undo stack before modifying a card.
    pub fn push_undo(&mut self, description: impl Into<String>, card: BookCard) {
        self.undo_stack.push(UndoEntry { description: description.into(), card });
        // Clear redo when a new action is taken
        self.redo_stack.clear();
    }

    /// Undo the last modification.
    pub fn undo(&mut self) {
        if let Some(entry) = self.undo_stack.pop() {
            let desc = entry.description.clone();
            // Capture current card state for redo
            let cards_dir = self.config.cards_dir();
            if let Ok(current) = omniscope_core::storage::json_cards::load_card_by_id(&cards_dir, &entry.card.id) {
                self.redo_stack.push(UndoEntry { description: desc.clone(), card: current });
            }
            // Restore old card
            let _ = omniscope_core::storage::json_cards::save_card(&cards_dir, &entry.card);
            if let Some(ref db) = self.db {
                let _ = db.upsert_book(&entry.card);
            }
            self.status_message = format!("Undo: {desc}");
            self.refresh_books();
        } else {
            self.status_message = "Nothing to undo".to_string();
        }
    }

    /// Redo the last undone modification.
    pub fn redo(&mut self) {
        if let Some(entry) = self.redo_stack.pop() {
            let desc = entry.description.clone();
            let cards_dir = self.config.cards_dir();
            if let Ok(current) = omniscope_core::storage::json_cards::load_card_by_id(&cards_dir, &entry.card.id) {
                self.undo_stack.push(UndoEntry { description: desc.clone(), card: current });
            }
            let _ = omniscope_core::storage::json_cards::save_card(&cards_dir, &entry.card);
            if let Some(ref db) = self.db {
                let _ = db.upsert_book(&entry.card);
            }
            self.status_message = format!("Redo: {desc}");
            self.refresh_books();
        } else {
            self.status_message = "Nothing to redo".to_string();
        }
    }

    // ─── Phase 1: Marks ────────────────────────────────────

    /// Set a named mark at the current position.
    pub fn set_mark(&mut self, key: char) {
        self.marks.insert(key, self.selected_index);
        self.status_message = format!("Mark '{key}' set");
    }

    /// Jump to a named mark.
    pub fn jump_to_mark(&mut self, key: char) {
        if let Some(&idx) = self.marks.get(&key) {
            if idx < self.books.len() {
                self.selected_index = idx;
                self.status_message = format!("Jumped to mark '{key}'");
            } else {
                self.status_message = format!("Mark '{key}' is out of range");
            }
        } else {
            self.status_message = format!("No mark '{key}'");
        }
    }

    // ─── Phase 1: Yank register ────────────────────────────

    /// Yank the currently selected book into the register.
    pub fn yank_selected(&mut self) {
        if let Some(book) = self.selected_book().cloned() {
            let title = book.title.clone();
            self.yank_register = Some(book);
            self.status_message = format!("Yanked: {title}");
        }
    }

    // ─── Phase 1: Count helpers ────────────────────────────

    /// Returns the accumulated vim count, or 1 if none was typed.
    pub fn count_or_one(&self) -> usize {
        if self.vim_count == 0 { 1 } else { self.vim_count as usize }
    }

    /// Reset vim count and operator.
    pub fn reset_vim_count(&mut self) {
        self.vim_count = 0;
        self.vim_operator = None;
    }

    /// Accumulate a digit into vim_count.
    pub fn push_vim_digit(&mut self, d: u32) {
        self.vim_count = self.vim_count.saturating_mul(10).saturating_add(d);
    }
}
