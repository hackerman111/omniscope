use crate::app::App;
use crossterm::event::KeyCode;

pub fn handle_z_command(app: &mut App, code: KeyCode) {
    match code {
        // zz — center current item in viewport
        KeyCode::Char('z') => {
            let visible_height = 20_usize; // approximate visible rows
            app.viewport_offset = app.selected_index.saturating_sub(visible_height / 2);
            app.status_message = format!("Center view on {}", app.selected_index + 1);
        }
        // zt — scroll so current item is at top of viewport
        KeyCode::Char('t') => {
            app.viewport_offset = app.selected_index;
            app.status_message = format!("Scroll top: {}", app.selected_index + 1);
        }
        // zb — scroll so current item is at bottom of viewport
        KeyCode::Char('b') => {
            let visible_height = 20_usize;
            app.viewport_offset = app
                .selected_index
                .saturating_sub(visible_height.saturating_sub(1));
            app.status_message = format!("Scroll bottom: {}", app.selected_index + 1);
        }
        // za — toggle fold (group/folder)
        KeyCode::Char('a') => {
            app.status_message = "Toggle fold (za)".to_string();
        }
        // zo — open fold
        KeyCode::Char('o') => {
            app.status_message = "Open fold (zo)".to_string();
        }
        // zc — close fold
        KeyCode::Char('c') => {
            app.status_message = "Close fold (zc)".to_string();
        }
        // zR — open all folds
        KeyCode::Char('R') => {
            app.status_message = "Open all folds (zR)".to_string();
        }
        // zM — close all folds
        KeyCode::Char('M') => {
            app.status_message = "Close all folds (zM)".to_string();
        }
        // zi — toggle folding entirely
        KeyCode::Char('i') => {
            app.status_message = "Toggle folding (zi)".to_string();
        }
        _ => {}
    }
}
