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
}
