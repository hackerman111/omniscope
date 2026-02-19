use crossterm::event::KeyCode;
use crate::app::App;

pub fn handle_z_command(app: &mut App, code: KeyCode) {
    match code {
        // zz — center view (just status message for now as we don't control viewport offset directly here easily yet)
        KeyCode::Char('z') => {
            app.status_message = format!("Center view on {}", app.selected_index + 1);
            // In a real implementation with manual viewport control, we would set viewport offset.
            // Virtualized list usually handles 'scroll_to_selection' automatically.
        }
        // zt — top
        KeyCode::Char('t') => {
            app.status_message = "Scroll Top".to_string();
        }
        // zb — bottom
        KeyCode::Char('b') => {
            app.status_message = "Scroll Bottom".to_string();
        }
        _ => {}
    }
}
