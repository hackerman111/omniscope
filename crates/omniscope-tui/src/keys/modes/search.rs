use crossterm::event::KeyCode;
use crate::app::{App, Mode, SearchDirection};

/// Search for the next/previous match of `last_search`.
pub fn search_next(app: &mut App, reverse: bool) {
    let query = match app.last_search.clone() {
        Some(q) if !q.is_empty() => q,
        _ => {
            app.status_message = "No previous search".to_string();
            return;
        }
    };

    let query_lower = query.to_lowercase();
    let forward = match (app.search_direction, reverse) {
        (SearchDirection::Forward, false) | (SearchDirection::Backward, true) => true,
        (SearchDirection::Forward, true) | (SearchDirection::Backward, false) => false,
    };

    if app.active_panel == crate::app::ActivePanel::Sidebar && app.left_panel_mode == crate::app::LeftPanelMode::FolderTree {
        let len = app.sidebar_items.len();
        if len == 0 { return; }
        let start = app.sidebar_selected;
        
        let iter: Vec<usize> = if forward {
            (1..=len).map(|o| (start + o) % len).collect()
        } else {
            (1..=len).map(|o| (start + len - o) % len).collect()
        };

        for idx in iter {
            if let crate::app::SidebarItem::FolderNode { name, .. } = &app.sidebar_items[idx] {
                if name.to_lowercase().contains(&query_lower) {
                    app.sidebar_selected = idx;
                    app.status_message = format!("/{query} [{}/{}]", idx + 1, len);
                    return;
                }
            }
        }
        app.status_message = format!("Pattern not found in folders: {query}");
        return;
    }

    let len = app.books.len();
    if len == 0 { return; }

    let start = app.selected_index;
    
    if forward {
        // Search forward from current+1, wrapping around
        for offset in 1..=len {
            let idx = (start + offset) % len;
            let book = &app.books[idx];
            let haystack = format!("{} {}", book.title, book.authors.join(" ")).to_lowercase();
            if haystack.contains(&query_lower) {
                app.selected_index = idx;
                app.status_message = format!("/{query} [{}/{}]", idx + 1, len);
                return;
            }
        }
    } else {
        // Search backward
        for offset in 1..=len {
            let idx = (start + len - offset) % len;
            let book = &app.books[idx];
            let haystack = format!("{} {}", book.title, book.authors.join(" ")).to_lowercase();
            if haystack.contains(&query_lower) {
                app.selected_index = idx;
                app.status_message = format!("?{query} [{}/{}]", idx + 1, len);
                return;
            }
        }
    }

    app.status_message = format!("Pattern not found: {query}");
}

pub(crate) fn handle_search_mode(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc => {
            app.mode = Mode::Normal;
            app.search_input.clear();
            app.apply_filter(); // restore unfiltered list
        }
        KeyCode::Enter => {
            app.mode = Mode::Normal;
            // Save search for n/N
            if !app.search_input.is_empty() {
                app.last_search = Some(app.search_input.clone());
            }
            // Keep results shown, clear input
            if app.search_input.is_empty() {
                app.apply_filter();
            }
            app.search_input.clear();
        }
        KeyCode::Backspace => {
            app.search_input.pop();
            app.fuzzy_search(&app.search_input.clone());
        }
        KeyCode::Char(c) => {
            app.search_input.push(c);
            app.fuzzy_search(&app.search_input.clone());
        }
        _ => {}
    }
}
