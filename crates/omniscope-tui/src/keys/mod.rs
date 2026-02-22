pub mod modes;
pub mod core;
pub mod ext;
pub mod ui;
#[cfg(test)]
pub mod tests;

use crossterm::event::{KeyCode, KeyModifiers};
use crate::app::{App, Mode};

pub(crate) fn handle_key(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    // Record macro events (before any handling, except for the 'q' that stops recording)
    if app.macro_recorder.is_recording() {
        // Don't record the 'q' that stops recording
        let is_stop_recording = code == KeyCode::Char('q')
            && !modifiers.contains(KeyModifiers::CONTROL)
            && app.mode == Mode::Normal
            && app.popup.is_none()
            && app.pending_key.is_none();
        if !is_stop_recording {
            app.macro_recorder.record_key(code, modifiers);
        }
    }

    // Popup takes priority
    if app.popup.is_some() {
        ui::popup_keys::handle_popup_key(app, code, modifiers);
        return;
    }

    // Handle register selection
    if app.mode != Mode::Insert && app.mode != Mode::Command && app.mode != Mode::Search && app.pending_register_select {
        if let KeyCode::Char(c) = code {
            app.vim_register = Some(c);
            app.pending_register_select = false;
        } else if code == KeyCode::Esc {
            app.pending_register_select = false;
        }
        return;
    }

    // Handle Tab to toggle Left Panel Mode globally if we're in Normal or Visual mode
    if code == KeyCode::Tab && modifiers.is_empty() && (app.mode == Mode::Normal || matches!(app.mode, Mode::Visual | Mode::VisualLine | Mode::VisualBlock)) {
        if app.left_panel_mode == crate::app::LeftPanelMode::LibraryView {
            app.left_panel_mode = crate::app::LeftPanelMode::FolderTree;
        } else {
            app.left_panel_mode = crate::app::LeftPanelMode::LibraryView;
        }
        app.refresh_sidebar();
        // Keep focus on sidebar
        app.active_panel = crate::app::ActivePanel::Sidebar;
        return;
    }

    match app.mode {
        Mode::Normal => {
            if app.active_panel == crate::app::ActivePanel::Sync {
                modes::sync::handle_sync_mode(app, code, modifiers);
            } else if app.active_panel == crate::app::ActivePanel::Sidebar && app.left_panel_mode == crate::app::LeftPanelMode::FolderTree {
                modes::folder::handle_folder_mode(app, code, modifiers);
            } else {
                modes::normal::handle_normal_mode(app, code, modifiers);
            }
        },
        Mode::Visual | Mode::VisualLine | Mode::VisualBlock => {
            if app.active_panel == crate::app::ActivePanel::Sync {
                modes::sync::handle_sync_mode(app, code, modifiers);
            } else if app.active_panel == crate::app::ActivePanel::Sidebar && app.left_panel_mode == crate::app::LeftPanelMode::FolderTree {
                modes::folder::handle_folder_mode(app, code, modifiers);
            } else {
                modes::visual::handle_visual_mode(app, code, modifiers);
            }
        },
        Mode::Pending => modes::pending::handle_pending_mode(app, code, modifiers),
        Mode::Command => modes::command_mode::handle_command_mode(app, code),
        Mode::Search => modes::search::handle_search_mode(app, code),
        _ => {
            if code == KeyCode::Esc {
                app.mode = Mode::Normal;
            }
        }
    }
}
