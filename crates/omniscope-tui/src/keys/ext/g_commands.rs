use crate::app::{App, Mode};
use crate::keys::core::motions;
use crate::keys::ext::science_bindings;
use crate::popup::Popup;
use crossterm::event::KeyCode;

pub fn handle_g_command(app: &mut App, code: KeyCode) {
    if science_bindings::handle_g_science_command(app, code) {
        return;
    }

    match code {
        // gg — go to top
        KeyCode::Char('g') => {
            app.record_jump();
            if app.active_panel == crate::app::ActivePanel::Sidebar {
                app.move_to_top();
            } else {
                let count = app.count_or_one();
                app.reset_vim_count();
                if let Some(t) = motions::get_nav_target(app, 'g', count) {
                    app.selected_index = t;
                }
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
        // gr — Go Root (All Books)
        KeyCode::Char('r') => {
            app.record_jump();
            app.sidebar_filter = crate::app::SidebarFilter::All;
            app.refresh_books();
            app.sidebar_selected = 0;
            app.status_message = "Go Root (All Books)".to_string();
        }

        // gh — Go Home (All Books)
        KeyCode::Char('h') => {
            app.record_jump();
            app.sidebar_filter = crate::app::SidebarFilter::All;
            app.refresh_books();
            app.status_message = "Go Home (All Books)".to_string();
        }

        // gp — Go Parent (up one level in hierarchy)
        KeyCode::Char('p') => {
            app.record_jump();
            if let crate::app::SidebarFilter::All = app.sidebar_filter {
                // Already at top
                app.status_message = "Already at root".to_string();
            } else {
                app.sidebar_filter = crate::app::SidebarFilter::All;
                app.refresh_books();
                app.status_message = "Go Parent".to_string();
            }
        }

        // gS — cycle status
        KeyCode::Char('S') => {
            app.cycle_status();
        }

        // gl — go to last position (same as Ctrl+o)
        KeyCode::Char('l') => {
            app.jump_back();
        }

        // gf — open file in OS (open book's file)
        KeyCode::Char('f') => {
            app.open_selected_book();
        }

        // gI — open JSON card in $EDITOR
        KeyCode::Char('I') => {
            if let Some(book) = app.selected_book() {
                let id = book.id;
                let cards_dir = app.cards_dir();
                let card_path = cards_dir.join(format!("{}.json", id));
                if card_path.exists() {
                    app.pending_editor_path = Some(card_path.to_string_lossy().to_string());
                    app.status_message = "Opening JSON card in $EDITOR...".to_string();
                } else {
                    app.status_message = "JSON card file not found".to_string();
                }
            }
        }

        // gv — reselect last visual selection
        KeyCode::Char('v') => {
            if let Some((start, end)) = app.last_visual_range {
                let max_idx = app.books.len().saturating_sub(1);
                let start = start.min(max_idx);
                let end = end.min(max_idx);
                app.visual_anchor = Some(start);
                app.selected_index = end;
                app.mode = Mode::Visual;
                app.visual_selections = (start..=end).collect();
                app.status_message =
                    format!("-- VISUAL -- {} selected", app.visual_selections.len());
            } else {
                app.status_message = "No previous visual selection".to_string();
            }
        }

        // gz — center view (alias for zz)
        KeyCode::Char('z') => {
            let visible_height = 20_usize;
            app.viewport_offset = app.selected_index.saturating_sub(visible_height / 2);
            app.status_message = format!("Center view on {}", app.selected_index + 1);
        }

        // g* — search by current book's author
        KeyCode::Char('*') => {
            if let Some(book) = app.selected_book() {
                if let Some(author) = book.authors.first() {
                    let query = author.clone();
                    app.last_search = Some(query.clone());
                    app.search_direction = crate::app::SearchDirection::Forward;
                    app.fuzzy_search(&query);
                    app.status_message = format!("Search author: {query}");
                } else {
                    app.status_message = "No author for current book".to_string();
                }
            }
        }

        // gB — previous buffer (alias for Ctrl+o / jump back)
        KeyCode::Char('B') => {
            app.jump_back();
        }

        // gb — buffers (show telescope)
        KeyCode::Char('b') => {
            app.open_telescope();
        }

        // gF — goto Folder: filter by folder path
        KeyCode::Char('F') => {
            if app.active_panel == crate::app::ActivePanel::Sidebar {
                if let Some(crate::app::SidebarItem::Folder { path }) =
                    app.sidebar_items.get(app.sidebar_selected)
                {
                    let path = path.clone();
                    app.filter_by_folder(&path);
                }
            } else if let Some(book) = app.selected_book() {
                let id = book.id;
                let cards_dir = app.cards_dir();
                if let Ok(card) =
                    omniscope_core::storage::json_cards::load_card_by_id(&cards_dir, &id)
                {
                    if let Some(ref file) = card.file {
                        if let Some(parent) = std::path::Path::new(&file.path).parent() {
                            let folder_path = parent.to_string_lossy().to_string();
                            app.filter_by_folder(&folder_path);
                        }
                    } else {
                        app.status_message = "No file attached to current book".to_string();
                    }
                }
            }
        }

        _ => {}
    }
}
