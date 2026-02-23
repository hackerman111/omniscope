use crossterm::event::{KeyCode, KeyModifiers};
use crate::app::{App, Mode};

pub fn handle_citation_graph_mode(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    let mut exit_mode = false;

    if !matches!(app.active_overlay, Some(crate::app::OverlayState::CitationGraph(_))) {
        app.mode = Mode::Normal;
        return;
    }

    match code {
        KeyCode::Esc | KeyCode::Char('q') => {
            exit_mode = true;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if let Some(crate::app::OverlayState::CitationGraph(state)) = &mut app.active_overlay {
                state.move_down();
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if let Some(crate::app::OverlayState::CitationGraph(state)) = &mut app.active_overlay {
                state.move_up();
            }
        }
        KeyCode::Tab => {
            if let Some(crate::app::OverlayState::CitationGraph(state)) = &mut app.active_overlay {
                state.mode = if modifiers.contains(KeyModifiers::SHIFT) {
                    state.mode.prev()
                } else {
                    state.mode.next()
                };
            }
        }
        KeyCode::Enter => {
            // open book
            app.status_message = "Open book logic not implemented".to_string();
        }
        KeyCode::Char('a') => {
            // [a]dd to library
            app.status_message = "Add to library logic not implemented".to_string();
        }
        KeyCode::Char('f') => {
            // [f]ind PDF online
            app.status_message = "Find PDF online not implemented".to_string();
        }
        _ => {}
    }

    if exit_mode {
        app.mode = Mode::Normal;
        app.active_overlay = None;
    }
}
