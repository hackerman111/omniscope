use omniscope_core::{BookCard, ReadStatus};

use super::{App, SidebarFilter};
use crate::popup::{AddBookForm, EditTagsForm, Popup};

impl App {
    // ─── Book Operations ───────────────────────────────────

    /// Open the Add Book popup.
    pub fn open_add_popup(&mut self) {
        self.popup = Some(Popup::AddBook(AddBookForm::new()));
    }

    /// Submit the Add Book form.
    pub fn submit_add_book(&mut self) {
        if let Some(Popup::AddBook(ref form)) = self.popup {
            let title = form.fields[0].value.trim().to_string();
            if title.is_empty() {
                self.status_message = "Title cannot be empty".to_string();
                return;
            }

            let mut card = BookCard::new(&title);

            // Authors (comma-separated)
            let authors_str = &form.fields[1].value;
            if !authors_str.is_empty() {
                card.metadata.authors = authors_str
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }

            // Year
            if let Ok(year) = form.fields[2].value.trim().parse::<i32>() {
                card.metadata.year = Some(year);
            }

            // Tags (comma or space-separated)
            let tags_str = &form.fields[3].value;
            if !tags_str.is_empty() {
                card.organization.tags = tags_str
                    .split([',', ' '])
                    .map(|s| s.trim().trim_start_matches('#').to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }

            // Library
            let lib = form.fields[4].value.trim().to_string();
            if !lib.is_empty() {
                card.organization.libraries.push(lib);
            }

            // File path
            let file_path = form.fields[5].value.trim().to_string();
            if !file_path.is_empty() {
                if let Ok(imported) = omniscope_core::file_import::import_file(std::path::Path::new(&file_path)) {
                    card.file = imported.file;
                }
            }

            // Save to undo stack before creating
            self.push_undo(
                 format!("Added book: {}", card.metadata.title),
                 omniscope_core::undo::UndoAction::UpsertCards(vec![card.clone()])
            );

            // Save
            let cards_dir = self.config.cards_dir();
            if let Err(e) = omniscope_core::storage::json_cards::save_card(&cards_dir, &card) {
                self.status_message = format!("Save error: {e}");
                return;
            }
            if let Some(ref db) = self.db {
                if let Err(e) = db.upsert_book(&card) {
                    self.status_message = format!("DB error: {e}");
                    return;
                }
            }

            self.status_message = format!("Added: {}", card.metadata.title);
            self.popup = None;
            self.refresh_books();
        }
    }

    /// Open delete confirmation popup.
    pub fn open_delete_confirm(&mut self) {
        if let Some(book) = self.selected_book() {
            self.popup = Some(Popup::DeleteConfirm {
                title: book.title.clone(),
                id: book.id.to_string(),
            });
        }
    }

    /// Execute book deletion.
    pub fn confirm_delete(&mut self) {
        if let Some(Popup::DeleteConfirm { ref title, ref id }) = self.popup {
            let id = id.clone();
            let title = title.clone();

            // Check old state for undo
            if let Ok(uuid) = uuid::Uuid::parse_str(&id) {
                if let Ok(old_card) = omniscope_core::storage::json_cards::load_card_by_id(&self.config.cards_dir(), &uuid) {
                     self.push_undo(
                          format!("Deleted: {title}"),
                          omniscope_core::undo::UndoAction::DeleteCards(vec![old_card])
                     );
                }
            }

            // Delete from DB
            if let Some(ref db) = self.db {
                let _ = db.delete_book(&id);
            }

            // Delete JSON card
            if let Ok(uuid) = uuid::Uuid::parse_str(&id) {
                let _ = omniscope_core::storage::json_cards::delete_card(&self.config.cards_dir(), &uuid);
            }

            self.status_message = format!("Deleted: {title}");
            self.popup = None;
            self.refresh_books();
        }
    }

    /// Set rating for selected book.
    pub fn set_rating(&mut self, rating: u8) {
        if let Some(book) = self.selected_book() {
            let id = book.id;
            let cards_dir = self.config.cards_dir();

            // Load full card, update, save
            if let Ok(mut card) = omniscope_core::storage::json_cards::load_card_by_id(&cards_dir, &id) {
                self.push_undo(
                     format!("Set rating for: {}", card.metadata.title),
                     omniscope_core::undo::UndoAction::UpsertCards(vec![card.clone()])
                );
                
                card.organization.rating = Some(rating);
                card.updated_at = chrono::Utc::now();
                let _ = omniscope_core::storage::json_cards::save_card(&cards_dir, &card);
                if let Some(ref db) = self.db {
                    let _ = db.upsert_book(&card);
                }
                self.status_message = format!("Rating: {}", "★".repeat(rating as usize));
                self.refresh_books();
            }
        }
    }

    /// Toggle read status for selected book.
    pub fn cycle_status(&mut self) {
        if let Some(book) = self.selected_book() {
            let id = book.id;
            let new_status = match book.read_status {
                ReadStatus::Unread => ReadStatus::Reading,
                ReadStatus::Reading => ReadStatus::Read,
                ReadStatus::Read => ReadStatus::Dnf,
                ReadStatus::Dnf => ReadStatus::Unread,
            };

            let cards_dir = self.config.cards_dir();
            if let Ok(mut card) = omniscope_core::storage::json_cards::load_card_by_id(&cards_dir, &id) {
                self.push_undo(
                     format!("Cycled status for: {}", card.metadata.title),
                     omniscope_core::undo::UndoAction::UpsertCards(vec![card.clone()])
                );
                
                card.organization.read_status = new_status;
                card.updated_at = chrono::Utc::now();
                let _ = omniscope_core::storage::json_cards::save_card(&cards_dir, &card);
                if let Some(ref db) = self.db {
                    let _ = db.upsert_book(&card);
                }
                self.status_message = format!("Status: {new_status}");
                self.refresh_books();
            }
        }
    }

    /// Set read status to a specific value for the selected book.
    pub fn set_status(&mut self, status: ReadStatus) {
        if let Some(book) = self.selected_book() {
            let id = book.id;
            let cards_dir = self.config.cards_dir();
            if let Ok(mut card) = omniscope_core::storage::json_cards::load_card_by_id(&cards_dir, &id) {
                self.push_undo(
                    format!("Set status for: {}", card.metadata.title),
                    omniscope_core::undo::UndoAction::UpsertCards(vec![card.clone()]),
                );
                card.organization.read_status = status.clone();
                card.updated_at = chrono::Utc::now();
                let _ = omniscope_core::storage::json_cards::save_card(&cards_dir, &card);
                if let Some(ref db) = self.db {
                    let _ = db.upsert_book(&card);
                }
                self.status_message = format!("Status: {status}");
                self.refresh_books();
            }
        }
    }

    /// Open tags editor popup.
    pub fn open_edit_tags(&mut self) {
        if let Some(book) = self.selected_book() {
            self.popup = Some(Popup::EditTags(EditTagsForm::new(
                book.id.to_string(),
                book.tags.clone(),
            )));
        }
    }

    /// Submit edited tags.
    pub fn submit_edit_tags(&mut self) {
        if let Some(Popup::EditTags(ref form)) = self.popup {
            let id = form.book_id.clone();
            let tags = form.tags.clone();

            let cards_dir = self.config.cards_dir();
            if let Ok(uuid) = uuid::Uuid::parse_str(&id) {
                if let Ok(mut card) = omniscope_core::storage::json_cards::load_card_by_id(&cards_dir, &uuid) {
                    self.push_undo(
                         format!("Edited tags for: {}", card.metadata.title),
                         omniscope_core::undo::UndoAction::UpsertCards(vec![card.clone()])
                    );
                    
                    card.organization.tags = tags.clone();
                    card.updated_at = chrono::Utc::now();
                    let _ = omniscope_core::storage::json_cards::save_card(&cards_dir, &card);
                    if let Some(ref db) = self.db {
                        let _ = db.upsert_book(&card);
                    }
                }
            }

            self.status_message = format!("Tags: {}", tags.join(", "));
            self.popup = None;
            self.refresh_books();
        }
    }

    /// Open selected book in external viewer.
    pub fn open_selected_book(&mut self) {
        if let Some(book) = self.selected_book() {
            let id = book.id;
            let cards_dir = self.config.cards_dir();
            match omniscope_core::storage::json_cards::load_card_by_id(&cards_dir, &id) {
                Ok(card) => {
                    if card.file.is_none() {
                        self.status_message = "No file attached to this book".to_string();
                        return;
                    }
                    match omniscope_core::viewer::open_book(&card, &self.config) {
                        Ok(()) => self.status_message = format!("Opened: {}", card.metadata.title),
                        Err(e) => self.status_message = format!("Open error: {e}"),
                    }
                }
                Err(e) => {
                    self.status_message = format!("Failed to load card: {e}");
                }
            }
        }
    }

    /// Copy selected book's file path to status (clipboard on real system).
    pub fn yank_path(&mut self) {
        if let Some(book) = self.selected_book() {
            if book.has_file {
                let id = book.id;
                let cards_dir = self.config.cards_dir();
                if let Ok(card) = omniscope_core::storage::json_cards::load_card_by_id(&cards_dir, &id) {
                    if let Some(ref file) = card.file {
                        self.status_message = format!("Yanked: {}", file.path);
                        return;
                    }
                }
            }
            self.status_message = "No file path to yank".to_string();
        }
    }

    /// Toggle visual selection on current item.
    pub fn toggle_visual_select(&mut self) {
        if let Some(idx) = self.visual_selections.iter().position(|&i| i == self.selected_index) {
            self.visual_selections.remove(idx);
        } else {
            self.visual_selections.push(self.selected_index);
        }
    }

    /// Open help popup.
    pub fn show_help(&mut self) {
        self.popup = Some(Popup::Help);
    }

    /// Submit edited year from EditYear popup.
    pub fn submit_edit_year(&mut self, book_id: &str, year_str: &str) {
        let cards_dir = self.config.cards_dir();
        if let Ok(uuid) = uuid::Uuid::parse_str(book_id) {
            if let Ok(mut card) = omniscope_core::storage::json_cards::load_card_by_id(&cards_dir, &uuid) {
                self.push_undo(
                    format!("Edited year for: {}", card.metadata.title),
                    omniscope_core::undo::UndoAction::UpsertCards(vec![card.clone()]),
                );
                card.metadata.year = year_str.trim().parse::<i32>().ok();
                card.updated_at = chrono::Utc::now();
                let _ = omniscope_core::storage::json_cards::save_card(&cards_dir, &card);
                if let Some(ref db) = self.db {
                    let _ = db.upsert_book(&card);
                }
                self.status_message = format!("Year: {}", year_str.trim());
            }
        }
        self.popup = None;
        self.refresh_books();
    }

    /// Submit edited authors from EditAuthors popup.
    pub fn submit_edit_authors(&mut self, book_id: &str, authors_str: &str) {
        let cards_dir = self.config.cards_dir();
        if let Ok(uuid) = uuid::Uuid::parse_str(book_id) {
            if let Ok(mut card) = omniscope_core::storage::json_cards::load_card_by_id(&cards_dir, &uuid) {
                self.push_undo(
                    format!("Edited authors for: {}", card.metadata.title),
                    omniscope_core::undo::UndoAction::UpsertCards(vec![card.clone()]),
                );
                card.metadata.authors = authors_str
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                card.updated_at = chrono::Utc::now();
                let _ = omniscope_core::storage::json_cards::save_card(&cards_dir, &card);
                if let Some(ref db) = self.db {
                    let _ = db.upsert_book(&card);
                }
                self.status_message = format!("Authors: {}", authors_str.trim());
            }
        }
        self.popup = None;
        self.refresh_books();
    }

    /// Add a tag to multiple books by index.
    pub fn add_tag_to_indices(&mut self, indices: &[usize], tag: &str) {
        let tag = tag.trim().to_string();
        if tag.is_empty() { return; }
        let cards_dir = self.config.cards_dir();
        let mut count = 0;
        for &i in indices {
            if let Some(book) = self.books.get(i) {
                if let Ok(mut card) = omniscope_core::storage::json_cards::load_card_by_id(&cards_dir, &book.id) {
                    if !card.organization.tags.contains(&tag) {
                        self.push_undo(
                            format!("Added tag '{}' to: {}", tag, card.metadata.title),
                            omniscope_core::undo::UndoAction::UpsertCards(vec![card.clone()]),
                        );
                        card.organization.tags.push(tag.clone());
                        card.updated_at = chrono::Utc::now();
                        let _ = omniscope_core::storage::json_cards::save_card(&cards_dir, &card);
                        if let Some(ref db) = self.db {
                            let _ = db.upsert_book(&card);
                        }
                        count += 1;
                    }
                }
            }
        }
        self.status_message = format!("Added tag '{}' to {} books", tag, count);
        self.refresh_books();
    }

    /// Remove a tag from multiple books by index.
    pub fn remove_tag_from_indices(&mut self, indices: &[usize], tag: &str) {
        let tag = tag.trim().to_string();
        if tag.is_empty() { return; }
        let cards_dir = self.config.cards_dir();
        let mut count = 0;
        for &i in indices {
            if let Some(book) = self.books.get(i) {
                if let Ok(mut card) = omniscope_core::storage::json_cards::load_card_by_id(&cards_dir, &book.id) {
                    if let Some(pos) = card.organization.tags.iter().position(|t| t == &tag) {
                        self.push_undo(
                            format!("Removed tag '{}' from: {}", tag, card.metadata.title),
                            omniscope_core::undo::UndoAction::UpsertCards(vec![card.clone()]),
                        );
                        card.organization.tags.remove(pos);
                        card.updated_at = chrono::Utc::now();
                        let _ = omniscope_core::storage::json_cards::save_card(&cards_dir, &card);
                        if let Some(ref db) = self.db {
                            let _ = db.upsert_book(&card);
                        }
                        count += 1;
                    }
                }
            }
        }
        self.status_message = format!("Removed tag '{}' from {} books", tag, count);
        self.refresh_books();
    }

    /// Perform fuzzy search on current query.
    pub fn fuzzy_search(&mut self, query: &str) {
        if query.is_empty() {
            self.apply_filter();
            return;
        }

        let results = self.fuzzy_searcher.search(query, &self.all_books);
        self.books = results.into_iter().map(|r| r.book).collect();
        self.selected_index = 0;
    }

    // ─── Telescope ─────────────────────────────────────────

    /// Open the Telescope search overlay and preload all autocomplete candidates.
    pub fn open_telescope(&mut self) {
        use crate::popup::TelescopeState;
        let mut state = TelescopeState::new();
        state.results = self.all_books.clone();
        self.popup = Some(Popup::Telescope(state));
        // Preload all tags + authors as candidates
        self.telescope_reload_candidates("");
    }

    /// Preload autocomplete candidates based on current token prefix.
    pub fn telescope_reload_candidates(&mut self, token: &str) {
        let mut candidates: Vec<String> = Vec::new();

        // Always add DSL keywords if token is empty or very short
        if token.len() <= 1 {
            candidates.push("@author:".to_string());
            candidates.push("#".to_string());
            candidates.push("y:".to_string());
            candidates.push("r:>=".to_string());
            candidates.push("s:unread".to_string());
            candidates.push("s:reading".to_string());
            candidates.push("s:read".to_string());
            candidates.push("f:pdf".to_string());
            candidates.push("f:epub".to_string());
            candidates.push("has:file".to_string());
            candidates.push("has:tags".to_string());
            candidates.push("NOT".to_string());
        }

        // Authors: match on @author: OR just @ OR if token matches an author name
        if token.is_empty() || token.starts_with('@') || token.len() > 1 {
            let prefix = token.strip_prefix("@author:").or_else(|| token.strip_prefix('@')).unwrap_or(token);
            if let Some(ref db) = self.db {
                if let Ok(authors) = db.get_all_authors() {
                    for a in authors {
                        if prefix.is_empty() || a.to_lowercase().contains(&prefix.to_lowercase()) {
                            candidates.push(format!("@author:{a}"));
                        }
                    }
                }
            }
        }

        // Tags: match on # OR if token matches a tag name
        if token.is_empty() || token.starts_with('#') || token.len() > 1 {
            let prefix = token.strip_prefix('#').unwrap_or(token);
            if let Some(ref db) = self.db {
                if let Ok(tags) = db.list_tags() {
                    for (name, _count) in tags {
                        if prefix.is_empty() || name.to_lowercase().contains(&prefix.to_lowercase()) {
                            candidates.push(format!("#{name}"));
                        }
                    }
                }
            }
        }

        if let Some(Popup::Telescope(ref mut state)) = self.popup {
            state.autocomplete.set_candidates(candidates, token);
        }
    }

    /// Run telescope search: parse DSL, apply filters, then fuzzy on remaining terms.
    pub fn telescope_search(&mut self, query: &str) {
        use omniscope_core::search_dsl::SearchQuery;

        let parsed = SearchQuery::parse(query);

        let mut filtered: Vec<_> = self.all_books.iter()
            .filter(|b| parsed.matches(b))
            .cloned()
            .collect();

        let fuzzy_text = parsed.fuzzy_text();
        if !fuzzy_text.is_empty() {
            let results = self.fuzzy_searcher.search(&fuzzy_text, &filtered);
            filtered = results.into_iter().map(|r| r.book).collect();
        }

        let chips: Vec<String> = parsed.filters.iter().map(|f| {
            match f {
                omniscope_core::search_dsl::SearchFilter::Author(a) => format!("@{a}"),
                omniscope_core::search_dsl::SearchFilter::Tag(t) => format!("#{t}"),
                omniscope_core::search_dsl::SearchFilter::NotTag(t) => format!("!#{t}"),
                omniscope_core::search_dsl::SearchFilter::Year(y) => match y {
                    omniscope_core::search_dsl::YearFilter::Exact(v) => format!("y:{v}"),
                    omniscope_core::search_dsl::YearFilter::Range(a, b) => format!("y:{a}-{b}"),
                    omniscope_core::search_dsl::YearFilter::GreaterThan(v) => format!("y:≥{v}"),
                    omniscope_core::search_dsl::YearFilter::LessThan(v) => format!("y:≤{v}"),
                },
                omniscope_core::search_dsl::SearchFilter::Rating(op) => match op {
                    omniscope_core::search_dsl::CompareOp::Gte(v) => format!("r:≥{v}"),
                    omniscope_core::search_dsl::CompareOp::Gt(v) => format!("r:>{v}"),
                    _ => "r:?".to_string(),
                },
                omniscope_core::search_dsl::SearchFilter::Status(s) => format!("s:{s}"),
                omniscope_core::search_dsl::SearchFilter::Format(f) => format!("f:{f}"),
                omniscope_core::search_dsl::SearchFilter::Library(l) => format!("lib:{l}"),
                omniscope_core::search_dsl::SearchFilter::HasFile => "has:file".to_string(),
                omniscope_core::search_dsl::SearchFilter::HasSummary => "has:summary".to_string(),
                omniscope_core::search_dsl::SearchFilter::HasTags => "has:tags".to_string(),
                omniscope_core::search_dsl::SearchFilter::Not(_inner) => "NOT ...".to_string(),
            }
        }).collect();

        if let Some(Popup::Telescope(ref mut state)) = self.popup {
            state.results = filtered;
            state.selected = 0;
            state.scroll = 0;
            state.active_filters = chips;
        }
    }

    /// Open the selected result from Telescope.
    pub fn telescope_open_selected(&mut self) {
        let selected_book = if let Some(Popup::Telescope(ref state)) = self.popup {
            state.selected_result().cloned()
        } else {
            None
        };

        if let Some(book) = selected_book {
            if let Some(ref db) = self.db {
                let _ = db.record_access(&book.id.to_string());
            }
            self.apply_filter();
            if let Some(filtered_pos) = self.books.iter().position(|b| b.id == book.id) {
                self.selected_index = filtered_pos;
            } else {
                self.sidebar_filter = SidebarFilter::All;
                self.apply_filter();
                self.selected_index = self.books.iter().position(|b| b.id == book.id).unwrap_or(0);
            }
            self.popup = None;
            self.status_message = format!("→ {}", book.title);
        }
    }

    /// Update autocomplete suggestions based on current token.
    pub fn update_telescope_suggestions(&mut self) {
        let token = if let Some(Popup::Telescope(ref state)) = self.popup {
            state.current_token().to_string()
        } else {
            return;
        };
        self.telescope_reload_candidates(&token);
    }
}
