use crossterm::event::{KeyCode, KeyModifiers};
use crate::app::{App, Mode};

pub fn handle_find_download_mode(app: &mut App, code: KeyCode, _modifiers: KeyModifiers) {
    let mut exit_mode = false;

    if !matches!(app.active_overlay, Some(crate::app::OverlayState::FindDownload(_))) {
        app.mode = Mode::Normal;
        return;
    }

    match code {
        KeyCode::Esc | KeyCode::Char('q') => {
            exit_mode = true;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if let Some(crate::app::OverlayState::FindDownload(state)) = &mut app.active_overlay {
                state.move_down();
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if let Some(crate::app::OverlayState::FindDownload(state)) = &mut app.active_overlay {
                state.move_up();
            }
        }
        KeyCode::Tab => {
            if let Some(crate::app::OverlayState::FindDownload(state)) = &mut app.active_overlay {
                state.toggle_column();
            }
        }
        KeyCode::Char('D') | KeyCode::Char('d') => {
            app.status_message = "Download logic not implemented".to_string();
        }
        KeyCode::Char('M') | KeyCode::Char('m') => {
            app.status_message = "Import Meta logic not implemented".to_string();
        }
        KeyCode::Char('o') => {
            app.status_message = "Open in browser logic not implemented".to_string();
        }
        _ => {}
    }

    if exit_mode {
        app.mode = Mode::Normal;
        app.active_overlay = None;
    }
}
