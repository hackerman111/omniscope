use super::{ActivePanel, App};

impl App {
    // ─── Navigation ────────────────────────────────────────

    pub fn move_down(&mut self) {
        match self.active_panel {
            ActivePanel::BookList => {
                if !self.books.is_empty() && self.selected_index < self.books.len() - 1 {
                    self.selected_index += 1;
                }
            }
            ActivePanel::Sidebar => {
                if !self.sidebar_items.is_empty()
                    && self.sidebar_selected < self.sidebar_items.len() - 1
                {
                    self.sidebar_selected += 1;
                }
            }
            ActivePanel::Preview => {}
        }
    }

    pub fn move_up(&mut self) {
        match self.active_panel {
            ActivePanel::BookList => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
            }
            ActivePanel::Sidebar => {
                if self.sidebar_selected > 0 {
                    self.sidebar_selected -= 1;
                }
            }
            ActivePanel::Preview => {}
        }
    }

    pub fn move_to_top(&mut self) {
        match self.active_panel {
            ActivePanel::BookList => self.selected_index = 0,
            ActivePanel::Sidebar => self.sidebar_selected = 0,
            ActivePanel::Preview => {}
        }
    }

    pub fn move_to_bottom(&mut self) {
        match self.active_panel {
            ActivePanel::BookList => {
                if !self.books.is_empty() {
                    self.selected_index = self.books.len() - 1;
                }
            }
            ActivePanel::Sidebar => {
                if !self.sidebar_items.is_empty() {
                    self.sidebar_selected = self.sidebar_items.len() - 1;
                }
            }
            ActivePanel::Preview => {}
        }
    }

    pub fn focus_left(&mut self) {
        self.active_panel = match self.active_panel {
            ActivePanel::BookList => ActivePanel::Sidebar,
            ActivePanel::Preview => ActivePanel::BookList,
            ActivePanel::Sidebar => ActivePanel::Sidebar,
        };
    }

    pub fn focus_right(&mut self) {
        self.active_panel = match self.active_panel {
            ActivePanel::Sidebar => ActivePanel::BookList,
            ActivePanel::BookList => ActivePanel::Preview,
            ActivePanel::Preview => ActivePanel::Preview,
        };
    }

    /// Move down n times (count-aware).
    pub fn move_down_n(&mut self, n: usize) {
        for _ in 0..n {
            self.move_down();
        }
    }

    /// Move up n times (count-aware).
    pub fn move_up_n(&mut self, n: usize) {
        for _ in 0..n {
            self.move_up();
        }
    }

    // ─── Phase 1: Jump List ────────────────────────────────

    pub fn record_jump(&mut self) {
        // Save current position for '' mark
        self.last_jump_pos = Some(self.selected_index);
        if let Some(book) = self.selected_book() {
            self.jump_list
                .push(self.selected_index, book.id.to_string());
        }
    }

    pub fn jump_back(&mut self) {
        // If we are currently "live" (not traversing history already), we might need to push current pos?
        // Standard vim: if you jump back, you are traversing.
        // But if you were not traversing, your current pos is saved in jump list (as the "newest").
        // Our jump list implementation manages `current` index.
        // We need to:
        // 1. Get previous location from jump list
        // 2. Move selected_index to there
        if let Some(loc) = self.jump_list.back() {
            // Restore position
            // Ideally check book_id compatibility, but index is fallback
            // In a virtualized list, index might be unstable if sort changed.
            // But Phase 1: simple index.
            if loc.index < self.books.len() {
                self.selected_index = loc.index;
                self.status_message = format!("Jump back to {}", loc.index + 1);
            } else {
                self.status_message = "Jump target out of range".to_string();
            }
        } else {
            self.status_message = "At bottom of jump list".to_string();
        }
    }

    pub fn jump_forward(&mut self) {
        if let Some(loc) = self.jump_list.forward() {
            if loc.index < self.books.len() {
                self.selected_index = loc.index;
                self.status_message = format!("Jump forward to {}", loc.index + 1);
            }
        } else {
            self.status_message = "At top of jump list".to_string();
        }
    }

    // ─── Phase 1: Hierarchy & Groups ───────────────────────

    pub fn go_up(&mut self) {
        // Equivalent to '-' or Backspace
        match self.active_panel {
            ActivePanel::BookList => {
                // If filtered, clear filter (go to All)
                // If in folder, go to parent folder (Phase 2)
                match self.sidebar_filter {
                    super::SidebarFilter::All => {
                        self.status_message = "Already at root".to_string();
                    }
                    _ => {
                        self.sidebar_filter = super::SidebarFilter::All;
                        self.refresh_books();
                        self.status_message = "Up to All Books".to_string();
                        // Also selection in sidebar should reflect this?
                        self.sidebar_selected = 0; // simplified
                    }
                }
            }
            ActivePanel::Sidebar => {
                // Move selection to parent item?
                // For simplified sidebar, just select top?
                self.sidebar_selected = 0;
            }
            _ => {}
        }
    }

    pub fn move_next_group(&mut self) {
        // Jump to next header in sidebar
        // Iter sidebar items, find next Header
        let current = self.sidebar_selected;
        if let Some(_next_idx) = self
            .sidebar_items
            .iter()
            .enumerate()
            .skip(current + 1)
            .position(|(_, item)| {
                matches!(
                    item,
                    super::SidebarItem::Library { .. }
                        | super::SidebarItem::TagHeader
                        | super::SidebarItem::FolderHeader
                )
            })
        {
            // position is relative to skip, need to add current + 1
            // Actually .position returns index in the iterator.
            // .enumerate().skip() ... yields (index, item).
            // .find step?
            // Let's loop manually for clarity
            for i in current + 1..self.sidebar_items.len() {
                match self.sidebar_items[i] {
                    super::SidebarItem::TagHeader | super::SidebarItem::FolderHeader => {
                        self.sidebar_selected = i;
                        return;
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn move_prev_group(&mut self) {
        if self.sidebar_selected == 0 {
            return;
        }
        for i in (0..self.sidebar_selected).rev() {
            match self.sidebar_items[i] {
                super::SidebarItem::TagHeader | super::SidebarItem::FolderHeader => {
                    self.sidebar_selected = i;
                    return;
                }
                _ => {}
            }
        }
    }
}
