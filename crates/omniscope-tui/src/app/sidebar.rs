use super::{ActivePanel, App, SidebarFilter, SidebarItem};

impl App {
    // ─── Sidebar ───────────────────────────────────────────

    /// Refresh sidebar items from the database.
    pub fn refresh_sidebar(&mut self) {
        let mut items = Vec::new();

        match self.left_panel_mode {
            crate::app::LeftPanelMode::LibraryView => {
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
            }
            crate::app::LeftPanelMode::FolderTree => {
                // Virtual Folders
                if let Some(ref db) = self.db {
                    if let Ok(v_folders) = db.list_virtual_folders() {
                        for folder in v_folders {
                            let count = db.count_books_in_virtual_folder(&folder.id).unwrap_or(0);
                            items.push(SidebarItem::VirtualFolder {
                                id: folder.id,
                                name: folder.name,
                                count: count as u32,
                            });
                        }
                    }
                }

                items.push(SidebarItem::FolderHeader);

                // Folders
                if let Some(ref tree) = self.folder_tree {
                    let mut stack: Vec<(&String, usize)> =
                        tree.root_ids.iter().rev().map(|id| (id, 0)).collect();

                    while let Some((id, depth)) = stack.pop() {
                        if let Some(node) = tree.nodes.get(id) {
                            let has_children = !node.children.is_empty();
                            let is_expanded = self.expanded_folders.contains(id);
                            let disk_path = node.folder.disk_path.clone().unwrap_or_default();

                            items.push(SidebarItem::FolderNode {
                                id: id.clone(),
                                name: node.folder.name.clone(),
                                depth,
                                is_expanded,
                                has_children,
                                ghost_count: 0, // TODO: calculate dynamically
                                disk_path,
                            });

                            if is_expanded && has_children {
                                // Push children in reverse order so they pop in alphabetical order (as sorted in tree)
                                for child_id in node.children.iter().rev() {
                                    stack.push((child_id, depth + 1));
                                }
                            }
                        }
                    }
                }
            }
        }

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
            SidebarFilter::VirtualFolder(folder_id) => {
                if let Some(ref db) = self.db {
                    db.list_books_by_virtual_folder(folder_id, 1000).unwrap_or_default()
                } else {
                    Vec::new()
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
                SidebarItem::FolderNode { disk_path, .. } => {
                    self.sidebar_filter = SidebarFilter::Folder(disk_path.clone());
                }
                // Placeholder for virtual folders, acting as AllBooks for now until F-4 filtering implemented
                SidebarItem::VirtualFolder { id, .. } => {
                    self.sidebar_filter = SidebarFilter::VirtualFolder(id.clone());
                }
                _ => return,
            }
            self.refresh_books();
            self.active_panel = ActivePanel::BookList;
        }
    }

    /// Refresh the center panel items when in FolderView mode
    pub fn refresh_center_panel(&mut self) {
        if self.center_panel_mode == crate::app::CenterPanelMode::BookList {
            self.center_items.clear();
            return;
        }

        let mut items = Vec::new();
        let current_folder_id = self.current_folder.clone();

        // 1. Get nested folders
        let mut folders = Vec::new();
        if let Some(tree) = &self.folder_tree {
            for (_, node) in &tree.nodes {
                if node.folder.parent_id == current_folder_id {
                    folders.push(crate::app::CenterItem::Folder(node.folder.clone()));
                }
            }
            folders.sort_by(|a, b| {
                if let (crate::app::CenterItem::Folder(fa), crate::app::CenterItem::Folder(fb)) =
                    (a, b)
                {
                    fa.name.cmp(&fb.name)
                } else {
                    std::cmp::Ordering::Equal
                }
            });
        }

        // 2. Get folder's books
        let mut books = Vec::new();
        if let Some(ref db) = self.db {
            if let Ok(folder_books) = db.list_books_by_folder_id(current_folder_id.as_deref(), 1000)
            {
                for book in folder_books {
                    books.push(crate::app::CenterItem::Book(book));
                }
            }
        }

        // 3. Sort accordingly
        match self.folder_view_sort {
            crate::app::FolderViewSort::FoldersFirst => {
                items.extend(folders);
                items.extend(books);
            }
            crate::app::FolderViewSort::BooksFirst => {
                items.extend(books);
                items.extend(folders);
            }
            crate::app::FolderViewSort::Mixed => {
                items.extend(folders);
                items.extend(books);
                items.sort_by(|a, b| {
                    let name_a = match a {
                        crate::app::CenterItem::Folder(f) => f.name.to_lowercase(),
                        crate::app::CenterItem::Book(bk) => bk.title.to_lowercase(),
                    };
                    let name_b = match b {
                        crate::app::CenterItem::Folder(f) => f.name.to_lowercase(),
                        crate::app::CenterItem::Book(bk) => bk.title.to_lowercase(),
                    };
                    name_a.cmp(&name_b)
                });
            }
        }

        self.center_items = items;
        if self.selected_index >= self.center_items.len() {
            self.selected_index = self.center_items.len().saturating_sub(1);
        }
    }

    /// Filter books by folder path (for gF command).
    pub fn filter_by_folder(&mut self, path: &str) {
        self.sidebar_filter = SidebarFilter::Folder(path.to_string());
        self.apply_filter();
        self.active_panel = ActivePanel::BookList;
        self.status_message = format!("Folder: {}", path);
    }

    /// Toggle expansion of the currently selected folder in the sidebar
    pub fn toggle_folder_expansion(&mut self) {
        if let Some(SidebarItem::FolderNode {
            id, has_children, ..
        }) = self.sidebar_items.get(self.sidebar_selected)
        {
            if *has_children {
                let id_clone = id.clone();
                if self.expanded_folders.contains(&id_clone) {
                    self.expanded_folders.remove(&id_clone);
                } else {
                    self.expanded_folders.insert(id_clone);
                }
                self.refresh_sidebar();
            }
        }
    }
}
