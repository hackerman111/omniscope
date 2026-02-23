use crossterm::event::{KeyCode, KeyModifiers};
use crate::app::{App, Mode};

pub fn handle_references_mode(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    let mut exit_mode = false;

    if !matches!(app.active_overlay, Some(crate::app::OverlayState::References(_))) {
        app.mode = Mode::Normal;
        return;
    }

    // Unwrapping is safe because we check for is_none above, but we borrow app conditionally
    // However, it's easier to just do match below:

    match code {
        KeyCode::Esc | KeyCode::Char('q') => {
            exit_mode = true;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if let Some(crate::app::OverlayState::References(state)) = &mut app.active_overlay {
                state.move_down();
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if let Some(crate::app::OverlayState::References(state)) = &mut app.active_overlay {
                state.move_up();
            }
        }
        KeyCode::Tab => {
            if let Some(crate::app::OverlayState::References(state)) = &mut app.active_overlay {
                state.filter = if modifiers.contains(KeyModifiers::SHIFT) {
                    state.filter.prev()
                } else {
                    state.filter.next()
                };
            }
        }
        KeyCode::Char('a') => {
            // [A]dd selected reference to library
            app.status_message = "Add to library logic not implemented yet".to_string();
        }
        KeyCode::Char('A') => {
            // [A]dd all unresolved to library
            app.status_message = "Add all unresolved logic not implemented yet".to_string();
        }
        KeyCode::Char('f') => {
            // [F]ind PDF online
            app.status_message = "Find PDF online not implemented yet".to_string();
        }
        KeyCode::Char('e') => {
            // [E]xport references
            app.status_message = "Export references not implemented yet".to_string();
        }
        KeyCode::Enter => {
            app.status_message = "Open reference not implemented yet".to_string();
        }
        KeyCode::Char('/') => {
            app.mode = Mode::Search;
            app.search_input.clear();
        }
        _ => {}
    }

    if exit_mode {
        app.mode = Mode::Normal;
        app.active_overlay = None;
    }
}
