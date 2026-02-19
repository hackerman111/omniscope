use crossterm::event::KeyCode;
use crate::app::App;
use crate::popup::Popup;
use super::motions;

pub fn handle_g_command(app: &mut App, code: KeyCode) {
    match code {
        // gg — go to top
        KeyCode::Char('g') => {
             app.record_jump();
             if let Some(t) = motions::get_nav_target(app, 'g', 1) {
                 app.selected_index = t;
             }
        }
        // gt — quick-edit title
        KeyCode::Char('t') => {
            if let Some(book) = app.selected_book() {
                let title = book.title.clone();
                app.open_add_popup();
                if let Some(Popup::AddBook(ref mut form)) = app.popup {
                    form.fields[0].value = title;
                    form.fields[0].cursor = form.fields[0].value.len();
                }
            }
        }
        // gr — set rating (alias for R) or Go Root
        // Plan says: gr -> goto root. R -> rating.
        // But previously it was mapped to rating.
        // Let's implement spec: gr -> goto root (All Books)
        KeyCode::Char('r') => {
            // "Go Root" means switching to "All Books" view?
            // Assuming we have a way to set filter to All.
            app.sidebar_filter = crate::app::SidebarFilter::All;
            app.refresh_books();
            app.sidebar_selected = 0; // approximate root in sidebar
            app.status_message = "Go Root (All Books)".to_string();
        }
        
        // gh — Go Home (All Books) - similar to gr?
        // Spec: gh -> "All Books"
        // gr -> "root of current library"
        KeyCode::Char('h') => {
            app.sidebar_filter = crate::app::SidebarFilter::All;
            app.refresh_books();
            app.status_message = "Go Home (All Books)".to_string();
        }
        
        // gp — Go Parent
        KeyCode::Char('p') => {
             // Go up one level in hierarchy.
             // If in a folder, go to parent folder.
             // If in a library, go to All.
             // Currently we have flat structure mostly, sidebar handles hierarchy?
             // If we are filtering by Library, go to All.
             if let crate::app::SidebarFilter::All = app.sidebar_filter {
                 // Already at top
             } else {
                 app.sidebar_filter = crate::app::SidebarFilter::All;
                 app.refresh_books();
                 app.status_message = "Go Parent".to_string();
             }
        }

        // gs — cycle status (alias for s)
        KeyCode::Char('s') => { app.cycle_status(); }
        
        // gl — go to last position
        KeyCode::Char('l') => {
             app.jump_back();
        }
        
        _ => {}
    }
}
