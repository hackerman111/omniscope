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
        self.all_books = self
            .db
            .as_ref()
            .and_then(|db| db.list_books(500, 0).ok())
            .unwrap_or_default();

        self.apply_filter();
        self.refresh_sidebar();
    }

    /// Apply the current sidebar filter to the book list.
    pub fn apply_filter(&mut self) {
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
        };
        self.selected_index = 0;
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
                _ => return,
            }
            self.apply_filter();
            self.active_panel = ActivePanel::BookList;
        }
    }
}
