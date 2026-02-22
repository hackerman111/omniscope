use crossterm::event::{KeyCode, KeyModifiers};
use crate::app::{App, SearchDirection};
use crate::popup::Popup;

pub(crate) fn handle_folder_mode(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    match code {
        KeyCode::Esc => {
            if matches!(app.mode, crate::app::Mode::Visual | crate::app::Mode::VisualLine | crate::app::Mode::VisualBlock) {
                app.exit_visual_mode();
            }
        }
        // ── Navigation ──
        KeyCode::Char('j') | KeyCode::Down => {
            app.reset_vim_count();
            app.move_down_n(1);
            if matches!(app.mode, crate::app::Mode::Visual | crate::app::Mode::VisualLine | crate::app::Mode::VisualBlock) {
                app.update_visual_selection();
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.reset_vim_count();
            app.move_up_n(1);
            if matches!(app.mode, crate::app::Mode::Visual | crate::app::Mode::VisualLine | crate::app::Mode::VisualBlock) {
                app.update_visual_selection();
            }
        }
        KeyCode::Char('h') | KeyCode::Left => {
            app.reset_vim_count();
            if let Some(crate::app::SidebarItem::FolderNode { is_expanded: true, .. }) = app.sidebar_items.get(app.sidebar_selected) {
                app.toggle_folder_expansion();
            } else {
                // If not expanded, move focus left out of sidebar (to books for example)
                // or just do nothing. Let's do nothing for Vim feel or go to parent folder.
            }
        }
        KeyCode::Char('l') | KeyCode::Right => {
            app.reset_vim_count();
            if let Some(crate::app::SidebarItem::FolderNode { is_expanded: false, .. }) = app.sidebar_items.get(app.sidebar_selected) {
                app.toggle_folder_expansion();
            } else {
                // If expanded or leaf, jump focus to BookList
                app.focus_right();
            }
        }
        KeyCode::Enter | KeyCode::Char(' ') => {
            app.reset_vim_count();
            app.select_sidebar_item();
        }

        // ── Complex Motions ──
        KeyCode::Char('G') => {
            app.reset_vim_count();
            app.move_to_bottom();
            if matches!(app.mode, crate::app::Mode::Visual | crate::app::Mode::VisualLine | crate::app::Mode::VisualBlock) {
                app.update_visual_selection();
            }
        }
        KeyCode::Char('g') => {
            if app.pending_key == Some('g') {
                app.pending_key = None;
                app.sidebar_selected = 0;
                if matches!(app.mode, crate::app::Mode::Visual | crate::app::Mode::VisualLine | crate::app::Mode::VisualBlock) {
                    app.update_visual_selection();
                }
            } else {
                app.pending_key = Some('g');
            }
        }

        // ── Folds (z commands) ──
        KeyCode::Char('z') => {
            app.pending_key = Some('z');
        }
        KeyCode::Char('a') if app.pending_key == Some('z') => {
            app.pending_key = None;
            app.toggle_folder_expansion();
        }
        KeyCode::Char('o') if app.pending_key == Some('z') => {
            app.pending_key = None;
            if let Some(crate::app::SidebarItem::FolderNode { is_expanded: false, .. }) = app.sidebar_items.get(app.sidebar_selected) {
                app.toggle_folder_expansion();
            }
        }
        KeyCode::Char('c') if app.pending_key == Some('z') => {
            app.pending_key = None;
            if let Some(crate::app::SidebarItem::FolderNode { is_expanded: true, .. }) = app.sidebar_items.get(app.sidebar_selected) {
                app.toggle_folder_expansion();
            }
        }
        KeyCode::Char('M') if app.pending_key == Some('z') => {
            app.pending_key = None;
            app.expanded_folders.clear();
            app.refresh_sidebar();
        }
        KeyCode::Char('R') if app.pending_key == Some('z') => {
            app.pending_key = None;
            if let Some(ref tree) = app.folder_tree {
                app.expanded_folders = tree.nodes.keys().cloned().collect();
                app.refresh_sidebar();
            }
        }

        // ── Folder Operations (a, r, d) ──
        KeyCode::Char('a') => {
            app.reset_vim_count();
            if let Some(crate::app::SidebarItem::FolderNode { id, .. }) = app.sidebar_items.get(app.sidebar_selected) {
                app.popup = Some(Popup::CreateFolder { parent_id: Some(id.clone()), input: String::new(), cursor: 0 });
            } else {
                app.popup = Some(Popup::CreateFolder { parent_id: None, input: String::new(), cursor: 0 });
            }
        }
        KeyCode::Char('A') => {
            app.reset_vim_count();
            app.popup = Some(Popup::CreateFolder { parent_id: None, input: String::new(), cursor: 0 });
        }
        KeyCode::Char('N') => {
            app.reset_vim_count();
            app.popup = Some(Popup::CreateVirtualFolder { input: String::new(), cursor: 0 });
        }
        KeyCode::Char('r') => {
            app.reset_vim_count();
            if let Some(crate::app::SidebarItem::FolderNode { id, name, .. }) = app.sidebar_items.get(app.sidebar_selected) {
                app.popup = Some(Popup::RenameFolder { folder_id: id.clone(), old_name: name.clone(), input: name.clone(), cursor: name.chars().count() });
            }
        }
        // Multiple d's (e.g. `dd` equivalent to `d` for simplicity)
        KeyCode::Char('d') if app.pending_key == Some('d') => {
            app.pending_key = None;
            if let Some(crate::app::SidebarItem::FolderNode { id, name, .. }) = app.sidebar_items.get(app.sidebar_selected) {
                app.popup = Some(Popup::ConfirmDeleteFolder { folder_id: id.clone(), folder_name: name.clone(), keep_files: false });
            }
        }
        KeyCode::Char('d') | KeyCode::Char('x') => {
            app.reset_vim_count();
            if matches!(app.mode, crate::app::Mode::Visual | crate::app::Mode::VisualLine | crate::app::Mode::VisualBlock) {
                if !app.visual_selections.is_empty() {
                    let mut folder_ids = Vec::new();
                    for &idx in &app.visual_selections {
                        if let Some(crate::app::SidebarItem::FolderNode { id, .. }) = app.sidebar_items.get(idx) {
                            folder_ids.push(id.clone());
                        }
                    }
                    if !folder_ids.is_empty() {
                        app.popup = Some(Popup::BulkDeleteFolders { folder_ids, keep_files: false });
                    }
                }
                app.exit_visual_mode();
            } else {
                if let Some(crate::app::SidebarItem::FolderNode { id, name, .. }) = app.sidebar_items.get(app.sidebar_selected) {
                    app.popup = Some(Popup::ConfirmDeleteFolder { folder_id: id.clone(), folder_name: name.clone(), keep_files: false });
                } else {
                     if app.pending_key == Some('d') {
                         app.pending_key = None;
                     } else {
                         app.pending_key = Some('d');
                     }
                }
            }
        }

        // ── Visual Mode Entry ──
        KeyCode::Char('v') => {
            app.enter_visual_mode(crate::app::Mode::VisualLine);
        }
        KeyCode::Char('V') => {
            app.enter_visual_mode(crate::app::Mode::VisualLine);
        }

        // ── Search ──
        KeyCode::Char('/') => {
            app.reset_vim_count();
            app.mode = crate::app::Mode::Search;
            app.search_input.clear();
            app.search_direction = SearchDirection::Forward;
        }

        // ── Panel Focus ──
        KeyCode::Tab => {
            app.focus_right();
        }

        _ => {}
    }
}
