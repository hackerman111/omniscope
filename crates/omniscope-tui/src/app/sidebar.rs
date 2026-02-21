use super::{ActivePanel, App, SidebarFilter, SidebarItem};

impl App {
    // ─── Sidebar ───────────────────────────────────────────

    /// Refresh sidebar items from the database.
    pub fn refresh_sidebar(&mut self) {
        let mut items = Vec::new();

        let total = self.all_books.len() as u32;
        items.push(SidebarItem::AllBooks { count: total });

        // Libraries
        if let Some(ref db) = self.db {
            if let Ok(libs) = db.list_libraries() {
                for (name, count) in libs {
                    items.push(SidebarItem::Library { name, count });
                }
            }
        }

        // Tags
        items.push(SidebarItem::TagHeader);
        if let Some(ref db) = self.db {
            if let Ok(tags) = db.list_tags() {
                for (name, count) in tags {
                    items.push(SidebarItem::Tag { name, count });
                }
            }
        }

        items.push(SidebarItem::FolderHeader);
        self.sidebar_items = items;
    }

    /// Refresh the book list based on current sidebar filter.
    pub fn refresh_books(&mut self) {
        // Remember current book ID to restore cursor position
        let current_id = self.books.get(self.selected_index).map(|b| b.id);

        self.all_books = self
            .db
            .as_ref()
            .and_then(|db| db.list_books(500, 0).ok())
            .unwrap_or_default();

        self.apply_filter_preserve_cursor(current_id);
        self.refresh_sidebar();
    }

    /// Apply the current sidebar filter to the book list.
    pub fn apply_filter(&mut self) {
        self.apply_filter_preserve_cursor(None);
    }

    /// Apply the current sidebar filter, optionally preserving cursor on a specific book ID.
    fn apply_filter_preserve_cursor(&mut self, preserve_id: Option<uuid::Uuid>) {
        let current_id = preserve_id.or_else(|| self.books.get(self.selected_index).map(|b| b.id));

        self.books = match &self.sidebar_filter {
            SidebarFilter::All => self.all_books.clone(),
            SidebarFilter::Library(lib) => {
                if let Some(ref db) = self.db {
                    db.list_books_by_library(lib, 500).unwrap_or_default()
                } else {
                    self.all_books.clone()
                }
            }
            SidebarFilter::Tag(tag) => {
                if let Some(ref db) = self.db {
                    db.list_books_by_tag(tag, 500).unwrap_or_default()
                } else {
                    self.all_books.clone()
                }
            }
            SidebarFilter::Folder(folder_path) => self
                .all_books
                .iter()
                .filter(|b| b.has_file)
                .filter(|b| {
                    if let Some(ref _db) = self.db {
                        if let Ok(card) = omniscope_core::storage::json_cards::load_card_by_id(
                            &self.cards_dir(),
                            &b.id,
                        ) {
                            if let Some(ref file) = card.file {
                                return file.path.starts_with(folder_path);
                            }
                        }
                    }
                    false
                })
                .cloned()
                .collect(),
        };

        // Restore cursor: find the same book by ID, else clamp to valid range
        if let Some(id) = current_id {
            if let Some(pos) = self.books.iter().position(|b| b.id == id) {
                self.selected_index = pos;
            } else {
                // Book was deleted or filtered out — clamp
                if self.selected_index >= self.books.len() {
                    self.selected_index = self.books.len().saturating_sub(1);
                }
            }
        } else {
            self.selected_index = 0;
        }
    }

    /// Handle sidebar Enter — apply filter.
    pub fn select_sidebar_item(&mut self) {
        if let Some(item) = self.sidebar_items.get(self.sidebar_selected) {
            match item {
                SidebarItem::AllBooks { .. } => {
                    self.sidebar_filter = SidebarFilter::All;
                }
                SidebarItem::Library { name, .. } => {
                    self.sidebar_filter = SidebarFilter::Library(name.clone());
                }
                SidebarItem::Tag { name, .. } => {
                    self.sidebar_filter = SidebarFilter::Tag(name.clone());
                }
                SidebarItem::Folder { path } => {
                    self.sidebar_filter = SidebarFilter::Folder(path.clone());
                }
                _ => return,
            }
            self.apply_filter();
            self.active_panel = ActivePanel::BookList;
        }
    }

    /// Filter books by folder path (for gF command).
    pub fn filter_by_folder(&mut self, path: &str) {
        self.sidebar_filter = SidebarFilter::Folder(path.to_string());
        self.apply_filter();
        self.active_panel = ActivePanel::BookList;
        self.status_message = format!("Folder: {}", path);
    }
}
