use super::{App, SortKey};
use omniscope_core::undo::{UndoAction, UndoEntry};
use omniscope_core::BookCard;
use crate::app::Mode;

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
            SortKey::YearAsc      => self.books.sort_by(|a, b| a.year.cmp(&b.year)),
            SortKey::RatingDesc   => self.books.sort_by(|a, b| b.rating.cmp(&a.rating)),
            SortKey::FrecencyDesc => self.books.sort_by(|a, b| b.frecency_score.partial_cmp(&a.frecency_score).unwrap_or(std::cmp::Ordering::Equal)),
        }
        self.selected_index = 0;
    }

    // ─── Phase 1: Undo / Redo ───────────────────────────────

    pub fn push_undo(&mut self, description: impl Into<String>, action: UndoAction) {
        self.undo_stack.push(UndoEntry { description: description.into(), action, timestamp: chrono::Utc::now() });
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) {
        if let Some(entry) = self.undo_stack.pop() {
            let desc = entry.description.clone();
            let redo_action = self.apply_undo_action(&entry.action);
            if let Some(action) = redo_action {
                 self.redo_stack.push(UndoEntry { description: desc.clone(), action, timestamp: chrono::Utc::now() });
            }
            self.status_message = format!("Undo: {desc}");
            self.refresh_books();
        } else {
            self.status_message = "Nothing to undo".to_string();
        }
    }

    pub fn redo(&mut self) {
        if let Some(entry) = self.redo_stack.pop() {
            let desc = entry.description.clone();
            let undo_action = self.apply_undo_action(&entry.action);
            if let Some(action) = undo_action {
                 self.undo_stack.push(UndoEntry { description: desc.clone(), action, timestamp: chrono::Utc::now() });
            }
            self.status_message = format!("Redo: {desc}");
            self.refresh_books();
        } else {
            self.status_message = "Nothing to redo".to_string();
        }
    }

    fn apply_undo_action(&mut self, action: &UndoAction) -> Option<UndoAction> {
        let cards_dir = self.config.cards_dir();
        match action {
            UndoAction::UpsertCards(cards) => {
                 let mut prev_state = Vec::new();
                 for card in cards {
                     if let Ok(current) = omniscope_core::storage::json_cards::load_card_by_id(&cards_dir, &card.id) {
                          prev_state.push(current);
                     }
                     
                     let _ = omniscope_core::storage::json_cards::save_card(&cards_dir, card);
                     if let Some(ref db) = self.db {
                          let _ = db.upsert_book(card);
                     }
                 }
                 if prev_state.is_empty() {
                      Some(UndoAction::DeleteCards(cards.clone()))
                 } else {
                      Some(UndoAction::UpsertCards(prev_state))
                 }
            }
            UndoAction::DeleteCards(cards) => {
                 for card in cards {
                      let _ = omniscope_core::storage::json_cards::delete_card(&cards_dir, &card.id);
                      if let Some(ref db) = self.db {
                           let _ = db.delete_book(&card.id.to_string());
                      }
                 }
                 Some(UndoAction::UpsertCards(cards.clone()))
            }
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
        let indices = if !self.visual_selections.is_empty() {
            self.visual_selections.clone()
        } else {
            vec![self.selected_index]
        };
        self.yank_indices(&indices);
    }

    /// Yank specific indices into the register.
    pub fn yank_indices(&mut self, indices: &[usize]) {
        let cards_dir = self.config.cards_dir();

        let books_to_yank: Vec<BookCard> = indices.iter()
             .filter_map(|&i| {
                 self.books.get(i).and_then(|view| {
                     omniscope_core::storage::json_cards::load_card_by_id(&cards_dir, &view.id).ok()
                 })
             })
             .collect();

        if books_to_yank.is_empty() { return; }

        let title_feedback = if books_to_yank.len() == 1 {
            books_to_yank[0].metadata.title.clone()
        } else {
            format!("{} items", books_to_yank.len())
        };

        // Determine target register
        let reg_char = self.vim_register.unwrap_or('"');
        
        let content = if books_to_yank.len() == 1 {
            crate::app::RegisterContent::Card(books_to_yank[0].clone())
        } else {
            crate::app::RegisterContent::MultipleCards(books_to_yank.clone())
        };

        self.registers.insert(reg_char, crate::app::Register {
            content: content.clone(),
            is_append: false,
        });

        // Also push to unnamed register ""
        if reg_char != '"' {
             self.registers.insert('"', crate::app::Register {
                 content: content.clone(),
                 is_append: false,
             });
        }
        
        // System clipboard integration
        if reg_char == '+' || reg_char == '*' {
             let mut text_to_yank = String::new();
             for card in &books_to_yank {
                  text_to_yank.push_str(&card.metadata.title);
                  text_to_yank.push('\n');
             }
             if let Some(ref mut clipboard) = self.clipboard {
                  let _ = clipboard.set_text(text_to_yank.trim());
             }
        }

        self.status_message = format!("Yanked: {title_feedback} to \"{reg_char}");
        self.vim_register = None;
    }

    /// Paste from the active register.
    pub fn paste_from_register(&mut self) {
        let reg_char = self.vim_register.unwrap_or('"');
        self.vim_register = None;
        let mut text_from_clip = None;

        // Try to pull from system clipboard if pasting from + or *
        if reg_char == '+' || reg_char == '*' {
             if let Some(ref mut clipboard) = self.clipboard {
                  if let Ok(text) = clipboard.get_text() {
                       text_from_clip = Some(text);
                  }
             }
        }
        
        if let Some(text) = text_from_clip {
             self.status_message = format!("Pasted from clipboard: {}...", &text.chars().take(20).collect::<String>());
             return;
        }

        if let Some(reg) = self.registers.get(&reg_char) {
            match &reg.content {
                crate::app::RegisterContent::Card(card) => {
                     // For now, "pasting" a card might mean duplicating it or just showing success.
                     // A full implementation would create a new UUID and insert it.
                     // Let's implement actual duplication for UI response.
                     let mut new_card = card.clone();
                     new_card.id = uuid::Uuid::new_v4();
                     new_card.metadata.title = format!("{} (copy)", new_card.metadata.title);
                     
                     let cards_dir = self.config.cards_dir();
                     let _ = omniscope_core::storage::json_cards::save_card(&cards_dir, &new_card);
                     if let Some(ref db) = self.db {
                          let _ = db.upsert_book(&new_card);
                     }
                     
                     self.push_undo(
                          format!("Pasted 1 item from \"{reg_char}"),
                          UndoAction::DeleteCards(vec![new_card]) // To undo pasting, delete it
                     );
                     self.status_message = format!("Pasted 1 item from \"{reg_char}");
                     self.refresh_books();
                }
                crate::app::RegisterContent::MultipleCards(cards) => {
                     let mut new_cards = Vec::new();
                     for card in cards {
                          let mut new_card = card.clone();
                          new_card.id = uuid::Uuid::new_v4();
                          // No rename loop to keep it clean, but vim duplicates it.
                          new_cards.push(new_card);
                     }
                     
                     let cards_dir = self.config.cards_dir();
                     for new_card in &new_cards {
                          let _ = omniscope_core::storage::json_cards::save_card(&cards_dir, new_card);
                          if let Some(ref db) = self.db {
                               let _ = db.upsert_book(new_card);
                          }
                     }
                     
                     let cards_len = cards.len();
                     self.push_undo(
                          format!("Pasted {} items from \"{reg_char}", new_cards.len()),
                          UndoAction::DeleteCards(new_cards)
                     );
                     self.status_message = format!("Pasted {} items from \"{reg_char}", cards_len);
                     self.refresh_books();
                }
                crate::app::RegisterContent::Path(path) => {
                     self.status_message = format!("Pasted path: {path}");
                }
                crate::app::RegisterContent::Text(text) => {
                     self.status_message = format!("Pasted text: {text}");
                }
            }
        } else {
             self.status_message = format!("Register \"{reg_char} is empty");
        }
    }

    // ─── Phase 1: Delete Operations ────────────────────────
    
    /// Delete specific indices (cards only).
    pub fn delete_indices(&mut self, indices: &[usize]) {
        if indices.is_empty() { return; }
        
        let mut sorted_indices = indices.to_vec();
        sorted_indices.sort_unstable();
        sorted_indices.dedup(); // just in case
        
        // We delete from end to start to avoid index shifting problems?
        // Actually best to collect IDs first.
        let ids_to_delete: Vec<uuid::Uuid> = sorted_indices.iter()
            .filter_map(|&i| self.books.get(i).map(|b| b.id))
            .collect();
            
        let count = ids_to_delete.len();
        
        // Save to undo stack before deleting        
        let mut cards_to_delete = Vec::new();
        for id in &ids_to_delete {
             let cards_dir = self.config.cards_dir();
             if let Ok(card) = omniscope_core::storage::json_cards::load_card_by_id(&cards_dir, id) {
                 cards_to_delete.push(card);
             }
        }
        
        if !cards_to_delete.is_empty() {
             self.push_undo(
                 format!("Deleted {} items", cards_to_delete.len()),
                 UndoAction::DeleteCards(cards_to_delete)
             );
        }
        
        for id in ids_to_delete {
            let cards_dir = self.config.cards_dir();
            
            // Delete the json card
             let _ = omniscope_core::storage::json_cards::delete_card(&cards_dir, &id);
             
             // Remove from DB (requires String ID for now? check DB sig)
             if let Some(ref db) = self.db {
                 let _ = db.delete_book(&id.to_string());
             }
        }
        
        self.search_input.clear(); // clear search to refresh list properly
        self.refresh_books();
        self.status_message = format!("Deleted {} cards", count);
        
        // Reset selection to something safe
        if self.selected_index >= self.books.len() {
            self.selected_index = self.books.len().saturating_sub(1);
        }
    }

    // ─── Phase 1: Visual Mode ──────────────────────────────

    pub fn enter_visual_mode(&mut self, mode: Mode) {
        self.mode = mode;
        self.visual_anchor = Some(self.selected_index);
        self.update_visual_selection();
        self.status_message = "-- VISUAL --".to_string();
    }

    pub fn exit_visual_mode(&mut self) {
        self.mode = Mode::Normal;
        self.visual_anchor = None;
        self.visual_selections.clear();
        self.status_message.clear();
    }

    pub fn update_visual_selection(&mut self) {
        if let Some(anchor) = self.visual_anchor {
            let start = anchor.min(self.selected_index);
            let end = anchor.max(self.selected_index);
            self.visual_selections = (start..=end).collect();
            self.status_message = format!("-- VISUAL -- {} selected", self.visual_selections.len());
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
        self.pending_operator = None;
        // Don't reset register here as it might be set before operator
    }

    /// Accumulate a digit into vim_count.
    pub fn push_vim_digit(&mut self, d: u32) {
        self.vim_count = self.vim_count.saturating_mul(10).saturating_add(d);
    }
}
