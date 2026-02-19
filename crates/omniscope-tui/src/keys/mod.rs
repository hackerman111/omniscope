mod popup_keys;

mod visual;
mod motions;
mod text_objects;
mod pending;
#[cfg(test)]
mod tests;

use crossterm::event::{KeyCode, KeyModifiers};
use crate::app::{App, Mode};
use crate::popup::Popup;

pub(crate) fn handle_key(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    // Popup takes priority
    if app.popup.is_some() {
        popup_keys::handle_popup_key(app, code, modifiers);
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

    match app.mode {
        Mode::Normal => handle_normal_mode(app, code, modifiers),
        Mode::Visual | Mode::VisualLine | Mode::VisualBlock => visual::handle_visual_mode(app, code, modifiers),
        Mode::Pending => pending::handle_pending_mode(app, code, modifiers),
        Mode::Command => handle_command_mode(app, code),
        Mode::Search => handle_search_mode(app, code),
        _ => {
            if code == KeyCode::Esc {
                app.mode = Mode::Normal;
            }
        }
    }
}

fn handle_normal_mode(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    // ── Digit accumulation (vim count prefix) ─────────────────────────
    if let KeyCode::Char(c) = code {
        if c.is_ascii_digit() && c != '0' {
            // '0' alone means "go to beginning of line", not a digit prefix
            app.push_vim_digit(c.to_digit(10).unwrap());
            return;
        }
    }

    // ── Handle pending key sequences ──────────────────────────────────
    if let Some(pending) = app.pending_key.take() {
        let _count = app.count_or_one();
        app.reset_vim_count();

        match (pending, code) {
            // gg — go to top
            ('g', KeyCode::Char('g')) => { app.move_to_top(); return; }
            // gt — quick-edit title
            ('g', KeyCode::Char('t')) => {
                if let Some(book) = app.selected_book() {
                    let title = book.title.clone();
                    app.open_add_popup();
                    if let Some(Popup::AddBook(ref mut form)) = app.popup {
                        form.fields[0].value = title;
                        form.fields[0].cursor = form.fields[0].value.len();
                    }
                }
                return;
            }
            // gr — set rating (alias for R)
            ('g', KeyCode::Char('r')) => {
                app.popup = Some(Popup::SetRating {
                    id: app.selected_book().map(|b| b.id.to_string()).unwrap_or_default(),
                    current: app.selected_book().and_then(|b| b.rating),
                });
                return;
            }
            // gs — cycle status (alias for s)
            ('g', KeyCode::Char('s')) => { app.cycle_status(); return; }
            // zz — center (just status bar feedback)
            ('z', KeyCode::Char('z')) => {
                app.status_message = format!("Line {}", app.selected_index + 1);
                return;
            }
            // m<char> — set mark
            ('m', KeyCode::Char(c)) if c.is_ascii_alphabetic() => {
                app.set_mark(c);
                return;
            }
            // '<char> — jump to mark
            ('\'', KeyCode::Char(c)) if c.is_ascii_alphabetic() => {
                app.jump_to_mark(c);
                return;
            }
            // d<motion> — operator pending: delete
            ('d', KeyCode::Char('d')) => {
                // If we get 'dd' here, it's a double-tap.
                // BUT wait, 'd' enters Pending mode now.
                // This 'pending_key' logic in normal mode handles the FIRST 'd'.
                // NO, this is for `g`, `z`, etc.
                // Wait, existing logic uses `app.pending_key` which is distinct from `Mode::Pending`.
                // Existing `d` handling:
                /*
                ('d', KeyCode::Char('d')) => {
                    for _ in 0..count { app.open_delete_confirm(); }
                    return;
                }
                */
                // We want to REPLACE this with proper Operator Pending.
                // So when user types 'd', we switch to Mode::Pending.
                // We shouldn't use `app.pending_key` for 'd' anymore in the same way?
                // Actually `app.pending_key` is a simple single-char buffer.
                // Let's rely on Mode::Pending instead.
                
                // So I will REMOVE the `d` and `y` cases from here if they are handled by Main `handle_normal_mode`.
                // BUT `handle_normal_mode` calls this "pending key" block first.
                // Let's remove them from there or update them.
            }
            // y<motion> — yank
            ('y', KeyCode::Char('y')) => {
                 // Remove this too, handle via Mode::Pending 'yy' logic.
            }
            _ => {} // unknown sequence
        }
        return;
    }

    match code {
        // ─── Quit ──────────────────────────────────────────────────
        KeyCode::Char('q') => app.should_quit = true,

        // ─── Visual Mode Entry ─────────────────────────────────────
        KeyCode::Char('v') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.enter_visual_mode(Mode::VisualBlock);
        }
        KeyCode::Char('v') => {
            app.enter_visual_mode(Mode::Visual);
        }
        KeyCode::Char('V') => {
            app.enter_visual_mode(Mode::VisualLine);
        }

        // ─── Navigation (count-aware) ───────────────────────────────
        KeyCode::Char('j') | KeyCode::Down => {
            let n = app.count_or_one();
            app.reset_vim_count();
            app.move_down_n(n);
        }
        KeyCode::Char('k') | KeyCode::Up => {
            let n = app.count_or_one();
            app.reset_vim_count();
            app.move_up_n(n);
        }
        KeyCode::Char('h') | KeyCode::Left  => { app.reset_vim_count(); app.focus_left(); }
        KeyCode::Char('l') | KeyCode::Right => { app.reset_vim_count(); app.focus_right(); }

        // ─── Pending leaders: g / z / m / ' ────────────────────────
        KeyCode::Char('g') => { app.pending_key = Some('g'); }
        KeyCode::Char('z') => { app.pending_key = Some('z'); }
        KeyCode::Char('m') => { app.pending_key = Some('m'); }
        KeyCode::Char('\'') => { app.pending_key = Some('\''); }
        
        // Operators -> Enter Pending Mode
        KeyCode::Char('d') if !modifiers.contains(KeyModifiers::CONTROL) => {
            app.vim_operator = Some('d');
            app.mode = Mode::Pending;
            // Don't use pending_key for this, use Mode!
        }
        KeyCode::Char('y') => {
             app.vim_operator = Some('y');
             app.mode = Mode::Pending;
        }
        KeyCode::Char('c') => {
             app.vim_operator = Some('c');
             app.mode = Mode::Pending;
        }
        KeyCode::Char('>') => {
             app.vim_operator = Some('>');
             app.mode = Mode::Pending;
        }
        KeyCode::Char('<') => {
             app.vim_operator = Some('<');
             app.mode = Mode::Pending;
        }

        KeyCode::Char('"') => { app.pending_register_select = true; }

        // ─── G / 0 / $ ─────────────────────────────────────────────
        KeyCode::Char('G') => { app.reset_vim_count(); app.move_to_bottom(); }
        KeyCode::Char('0') => { app.reset_vim_count(); app.move_to_top(); }

        // ─── Half-page scroll ───────────────────────────────────────
        KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
            let n = 10 * app.count_or_one();
            app.reset_vim_count();
            app.move_down_n(n);
        }
        KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
            let n = 10 * app.count_or_one();
            app.reset_vim_count();
            app.move_up_n(n);
        }

        // ─── Undo / Redo ────────────────────────────────────────────
        KeyCode::Char('u') if !modifiers.contains(KeyModifiers::CONTROL) => {
            app.reset_vim_count();
            app.undo();
        }
        KeyCode::Char('r') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.reset_vim_count();
            app.redo();
        }

        // ─── Book Operations ────────────────────────────────────────
        KeyCode::Char('a') => { app.reset_vim_count(); app.open_add_popup(); }
        KeyCode::Char('o') => { app.reset_vim_count(); app.open_selected_book(); }
        KeyCode::Char('R') => {
            app.reset_vim_count();
            app.popup = Some(Popup::SetRating {
                id: app.selected_book().map(|b| b.id.to_string()).unwrap_or_default(),
                current: app.selected_book().and_then(|b| b.rating),
            });
        }
        KeyCode::Char('s') => { app.reset_vim_count(); app.cycle_status(); }
        KeyCode::Char('t') => { app.reset_vim_count(); app.open_edit_tags(); }

        // ─── Sorting ────────────────────────────────────────────────
        KeyCode::Char('S') => { app.reset_vim_count(); app.cycle_sort(); }

        // ─── Modes ──────────────────────────────────────────────────
        KeyCode::Char(':') => {
            app.reset_vim_count();
            app.mode = Mode::Command;
            app.command_input.clear();
        }
        KeyCode::Char('/') => {
            app.reset_vim_count();
            app.open_telescope();
        }

        // ─── Enter ──────────────────────────────────────────────────
        KeyCode::Enter => {
            app.reset_vim_count();
            if app.active_panel == crate::app::ActivePanel::Sidebar {
                app.select_sidebar_item();
            }
        }

        // ─── Help ────────────────────────────────────────────────────
        KeyCode::Char('?') => { app.reset_vim_count(); app.show_help(); }

        // Tab / BackTab
        KeyCode::Tab    => { app.reset_vim_count(); app.focus_right(); }
        KeyCode::BackTab => { app.reset_vim_count(); app.focus_left(); }

        _ => { app.reset_vim_count(); }
    }
}

fn handle_command_mode(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc => {
            app.mode = Mode::Normal;
            app.command_input.clear();
        }
        KeyCode::Enter => {
            let cmd = app.command_input.clone();
            app.mode = Mode::Normal;
            execute_command(app, &cmd);
            app.command_input.clear();
        }
        KeyCode::Backspace => {
            app.command_input.pop();
            if app.command_input.is_empty() {
                app.mode = Mode::Normal;
            }
        }
        KeyCode::Char(c) => {
            app.command_input.push(c);
        }
        _ => {}
    }
}

fn handle_search_mode(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc => {
            app.mode = Mode::Normal;
            app.search_input.clear();
            app.apply_filter(); // restore unfiltered list
        }
        KeyCode::Enter => {
            app.mode = Mode::Normal;
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

fn execute_command(app: &mut App, cmd: &str) {
    let parts: Vec<&str> = cmd.trim().split_whitespace().collect();
    match parts.as_slice() {
        ["q" | "quit"] => app.should_quit = true,
        ["w" | "write"] => {
            app.status_message = "Saved.".to_string();
        }
        ["wq"] => {
            app.status_message = "Saved.".to_string();
            app.should_quit = true;
        }
        ["add"] => app.open_add_popup(),
        ["open"] => app.open_selected_book(),
        ["tags"] => app.open_edit_tags(),
        ["help"] => app.show_help(),
        ["search" | "find", rest @ ..] => {
            app.open_telescope();
            if !rest.is_empty() {
                let query = rest.join(" ");
                if let Some(crate::popup::Popup::Telescope(ref mut state)) = app.popup {
                    state.query = query.clone();
                    state.cursor = query.len();
                }
                app.telescope_search(&query);
            }
        }
        ["refresh"] => {
            app.refresh_books();
            app.status_message = "Refreshed.".to_string();
        }
        _ => {
            app.status_message = format!("Unknown command: {cmd}");
        }
    }
}
