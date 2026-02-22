use crate::app::{App, Mode};
use crossterm::event::{KeyCode, KeyModifiers};

pub(crate) fn handle_visual_mode(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    // ── Digit accumulation for count prefix ──────────────────────────────
    if let KeyCode::Char(c) = code {
        if c.is_ascii_digit() && c != '0' {
            app.push_vim_digit(c.to_digit(10).unwrap());
            return;
        }
        if c == '0' && app.vim_count > 0 {
            app.push_vim_digit(0);
            return;
        }
    }

    // ── Handle pending key sequences (same as Normal) ──────────────────────────
    if let Some(pending) = app.pending_key.take() {
        app.reset_vim_count();

        match (pending, code) {
            // gg — go to top
            ('g', KeyCode::Char('g')) => {
                app.move_to_top();
                app.update_visual_selection();
            }
            // other pending keys ...
            _ => {}
        }
        return;
    }

    match code {
        KeyCode::Esc => {
            // Save visual range before exiting
            if let Some(anchor) = app.visual_anchor {
                let start = anchor.min(app.selected_index);
                let end = anchor.max(app.selected_index);
                app.last_visual_range = Some((start, end));
            }
            app.exit_visual_mode();
        }
        KeyCode::Char('v') if modifiers.contains(KeyModifiers::CONTROL) => {
            if app.mode == Mode::VisualBlock {
                save_and_exit_visual(app);
            } else {
                app.enter_visual_mode(Mode::VisualBlock);
            }
        }
        KeyCode::Char('V') => {
            if app.mode == Mode::VisualLine {
                save_and_exit_visual(app);
            } else {
                app.enter_visual_mode(Mode::VisualLine);
            }
        }
        KeyCode::Char('v') => {
            if app.mode == Mode::Visual {
                save_and_exit_visual(app);
            } else {
                app.enter_visual_mode(Mode::Visual);
            }
        }

        // o — swap anchor and cursor
        KeyCode::Char('o') => {
            if let Some(anchor) = app.visual_anchor {
                let current = app.selected_index;
                app.visual_anchor = Some(current);
                app.selected_index = anchor;
                app.update_visual_selection();
            }
        }

        // Space — toggle individual item selection and move down
        KeyCode::Char(' ') => {
            app.toggle_visual_select();
            if app.selected_index < app.books.len().saturating_sub(1) {
                app.selected_index += 1;
            }
            app.update_visual_selection();
        }

        // Ctrl+a — select all
        KeyCode::Char('a') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.visual_anchor = Some(0);
            app.selected_index = app.books.len().saturating_sub(1);
            app.visual_selections = (0..app.books.len()).collect();
            app.status_message = format!(
                "-- VISUAL -- {} selected (all)",
                app.visual_selections.len()
            );
        }

        // g prefix
        KeyCode::Char('g') => {
            app.pending_key = Some('g');
        }

        // Navigation through standard motions
        KeyCode::Char(c) if "jkhlG0$".contains(c) => {
            let n = app.count_or_one();
            app.reset_vim_count();

            // h/l change focus, which exits visual mode
            if c == 'h' {
                save_and_exit_visual(app);
                app.focus_left();
                return;
            } else if c == 'l' {
                save_and_exit_visual(app);
                app.focus_right();
                return;
            }

            if c == 'G' {
                // G in visual: go to bottom (no explicit count in visual)
                if let Some(target) = crate::keys::core::motions::get_nav_target(app, 'G', 0) {
                    app.selected_index = target;
                    app.update_visual_selection();
                }
            } else if let Some(target) = crate::keys::core::motions::get_nav_target(app, c, n) {
                app.selected_index = target;
                app.update_visual_selection();
            }
        }

        // Screen-relative motions
        KeyCode::Char('H') => {
            app.reset_vim_count();
            app.selected_index = app.viewport_offset;
            if app.selected_index >= app.books.len() {
                app.selected_index = 0;
            }
            app.update_visual_selection();
        }
        KeyCode::Char('M') => {
            app.reset_vim_count();
            let visible_height = 20_usize;
            let mid = app.viewport_offset + visible_height / 2;
            app.selected_index = mid.min(app.books.len().saturating_sub(1));
            app.update_visual_selection();
        }
        KeyCode::Char('L') => {
            app.reset_vim_count();
            let visible_height = 20_usize;
            let bottom = app.viewport_offset + visible_height.saturating_sub(1);
            app.selected_index = bottom.min(app.books.len().saturating_sub(1));
            app.update_visual_selection();
        }

        // Group navigation
        KeyCode::Char('{') => {
            app.reset_vim_count();
            app.move_prev_group();
            app.update_visual_selection();
        }
        KeyCode::Char('}') => {
            app.reset_vim_count();
            app.move_next_group();
            app.update_visual_selection();
        }
        KeyCode::Down => {
            let n = app.count_or_one();
            app.reset_vim_count();
            if let Some(target) = crate::keys::core::motions::get_nav_target(app, 'j', n) {
                app.selected_index = target;
                app.update_visual_selection();
            }
        }
        KeyCode::Up => {
            let n = app.count_or_one();
            app.reset_vim_count();
            if let Some(target) = crate::keys::core::motions::get_nav_target(app, 'k', n) {
                app.selected_index = target;
                app.update_visual_selection();
            }
        }
        KeyCode::Left => {
            app.reset_vim_count();
            save_and_exit_visual(app);
            app.focus_left();
        }
        KeyCode::Right => {
            app.reset_vim_count();
            save_and_exit_visual(app);
            app.focus_right();
        }

        // Half-page scroll
        KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
            let n = 10 * app.count_or_one();
            app.reset_vim_count();
            app.move_down_n(n);
            app.update_visual_selection();
        }
        KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
            let n = 10 * app.count_or_one();
            app.reset_vim_count();
            app.move_up_n(n);
            app.update_visual_selection();
        }

        // Operators
        KeyCode::Char('y') => {
            let selections = app.visual_selections.clone();
            app.yank_indices(&selections);
            save_and_exit_visual(app);
        }
        KeyCode::Char('d') | KeyCode::Char('x') => {
            let selections = app.visual_selections.clone();
            app.status_message = format!("Delete debug: {:?}", selections);
            if app.active_panel == crate::app::ActivePanel::Sidebar && app.left_panel_mode == crate::app::LeftPanelMode::FolderTree {
                let mut folder_ids = Vec::new();
                for &idx in &selections {
                    if let Some(crate::app::SidebarItem::FolderNode { id, .. }) = app.sidebar_items.get(idx) {
                        folder_ids.push(id.clone());
                    }
                }
                if !folder_ids.is_empty() {
                    app.popup = Some(crate::popup::Popup::BulkDeleteFolders { folder_ids, keep_files: true });
                }
            } else if app.active_panel == crate::app::ActivePanel::BookList && app.center_panel_mode == crate::app::CenterPanelMode::FolderView {
                 let mut folder_ids = Vec::new();
                 let mut book_ids = Vec::new();
                 for &idx in &selections {
                     if let Some(item) = app.center_items.get(idx).cloned() {
                         match item {
                             crate::app::CenterItem::Folder(f) => folder_ids.push(f.id.clone()),
                             crate::app::CenterItem::Book(b) => book_ids.push(b.id),
                         }
                     }
                 }
                 if !folder_ids.is_empty() {
                      app.popup = Some(crate::popup::Popup::BulkDeleteFolders { folder_ids, keep_files: true });
                 }
                 if !book_ids.is_empty() {
                      app.delete_books_by_id(&book_ids);
                 }
            } else if app.active_panel == crate::app::ActivePanel::BookList {
                app.delete_indices(&selections);
            }
            save_and_exit_visual(app);
        }
        KeyCode::Char('c') => {
            // Change: open tags editor for the first selected item (don't delete!)
            let selections = app.visual_selections.clone();
            if !selections.is_empty() {
                app.selected_index = selections[0];
                app.open_edit_tags();
            }
            save_and_exit_visual(app);
            app.status_message = format!("Change {} items", selections.len());
        }

        // Tag operators in visual mode
        KeyCode::Char('>') => {
            let selections = app.visual_selections.clone();
            if !selections.is_empty() {
                app.popup = Some(crate::popup::Popup::AddTagPrompt {
                    indices: selections,
                    input: String::new(),
                    cursor: 0,
                });
            }
            save_and_exit_visual(app);
        }
        KeyCode::Char('<') => {
            let selections = app.visual_selections.clone();
            if !selections.is_empty() {
                let mut tags: Vec<String> = Vec::new();
                for &i in &selections {
                    if let Some(book) = app.books.get(i) {
                        for tag in &book.tags {
                            if !tags.contains(tag) {
                                tags.push(tag.clone());
                            }
                        }
                    }
                }
                if !tags.is_empty() {
                    app.popup = Some(crate::popup::Popup::RemoveTagPrompt {
                        indices: selections,
                        available_tags: tags,
                        selected: 0,
                    });
                } else {
                    app.status_message = "No tags to remove".to_string();
                }
            }
            save_and_exit_visual(app);
        }

        // Registers
        KeyCode::Char('"') => {
            app.pending_register_select = true;
        }

        // Quickfix send
        KeyCode::Char('q') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.quickfix_list = app
                .visual_selections
                .iter()
                .filter_map(|&i| app.books.get(i).cloned())
                .collect();
            app.quickfix_show = true;
            app.quickfix_selected = 0;
            let count = app.quickfix_list.len();
            save_and_exit_visual(app);
            app.status_message = format!("Sent {} items to quickfix", count);
        }

        _ => {}
    }
}

/// Save visual range and exit visual mode.
fn save_and_exit_visual(app: &mut App) {
    if let Some(anchor) = app.visual_anchor {
        let start = anchor.min(app.selected_index);
        let end = anchor.max(app.selected_index);
        app.last_visual_range = Some((start, end));
    }
    app.exit_visual_mode();
}
