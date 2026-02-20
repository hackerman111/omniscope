mod popup_keys;

mod visual;
mod motions;
mod text_objects;
mod pending;
pub mod operator;
pub mod jump_list;
mod g_commands;
mod z_commands;
pub mod find_char;
pub mod easy_motion;
pub mod hints;
pub mod macro_recorder;
#[cfg(test)]
mod tests;

use crossterm::event::{KeyCode, KeyModifiers};
use crate::app::{App, Mode, SearchDirection};
use crate::popup::Popup;
use crate::keys::operator::Operator;

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
            let d = c.to_digit(10).unwrap();
            let new_count = app.vim_count.saturating_mul(10).saturating_add(d);
            if new_count <= 9999 {
                app.push_vim_digit(d);
            }
            return;
        }
        if c == '0' && app.vim_count > 0 {
             let new_count = app.vim_count.saturating_mul(10);
             if new_count <= 9999 {
                 app.push_vim_digit(0);
             }
             return;
        }
    }

    // ── Handle pending key sequences (g, z, m, ', [, ], Space, S, f/F/t/T, @) ──
    if let Some(pending) = app.pending_key {
        handle_pending_sequence(app, pending, code);
        return;
    }

    match code {
        // ─── Macro Recording ──────────────────────────────────────
        KeyCode::Char('q') if !modifiers.contains(KeyModifiers::CONTROL) => {
            if app.macro_recorder.is_recording() {
                app.macro_recorder.stop_recording();
                app.status_message = "Macro recording stopped".to_string();
            } else {
                // Start recording: next char is register
                app.pending_key = Some('Q'); // Use 'Q' to distinguish from regular 'q' quit
            }
        }

        // ─── Macro Replay ─────────────────────────────────────────
        KeyCode::Char('@') => {
            app.pending_key = Some('@');
        }

        // ─── Quit / Quickfix ───────────────────────────────────────
        KeyCode::Char('Q') => {
            app.should_quit = true;
        }

        // ─── Ctrl+q — quickfix from visual/search ──────────────────
        KeyCode::Char('q') if modifiers.contains(KeyModifiers::CONTROL) => {
             app.reset_vim_count();
             if !app.visual_selections.is_empty() {
                 app.quickfix_list = app.visual_selections.iter()
                    .filter_map(|&i| app.books.get(i).cloned())
                    .collect();
             } else {
                 app.quickfix_list = app.books.clone();
             }
             app.quickfix_show = true;
             app.quickfix_selected = 0;
             app.exit_visual_mode();
             app.status_message = format!("Sent {} items to quickfix", app.quickfix_list.len());
        }

        // ─── Visual Mode Entry ─────────────────────────────────────
        KeyCode::Char('v') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.enter_visual_mode(Mode::VisualBlock);
        }
        KeyCode::Char('V') => {
            app.enter_visual_mode(Mode::VisualLine);
        }
        KeyCode::Char('v') => {
            app.enter_visual_mode(Mode::Visual);
        }

        // ─── Search ─────────────────────────────────────────────────
        KeyCode::Char('/') => {
            app.reset_vim_count();
            app.mode = Mode::Search;
            app.search_input.clear();
            app.search_direction = SearchDirection::Forward;
        }
        KeyCode::Char('Z') => {
            app.reset_vim_count();
            app.open_telescope();
        }
        KeyCode::Char('?') => {
            app.reset_vim_count();
            app.mode = Mode::Search;
            app.search_input.clear();
            app.search_direction = SearchDirection::Backward;
        }

        // ─── Search Next/Prev ───────────────────────────────────────
        KeyCode::Char('n') => {
            search_next(app, false);
        }
        KeyCode::Char('N') => {
            search_next(app, true);
        }

        // ─── Word-under-cursor Search ───────────────────────────────
        KeyCode::Char('*') => {
            // Search for current book's author (forward)
            if let Some(book) = app.selected_book() {
                let query = book.authors.first()
                    .cloned()
                    .unwrap_or_else(|| book.title.clone());
                app.last_search = Some(query.clone());
                app.search_direction = SearchDirection::Forward;
                search_next(app, false);
            }
        }
        KeyCode::Char('#') => {
            // Search for current book's author (backward)
            if let Some(book) = app.selected_book() {
                let query = book.authors.first()
                    .cloned()
                    .unwrap_or_else(|| book.title.clone());
                app.last_search = Some(query.clone());
                app.search_direction = SearchDirection::Backward;
                search_next(app, false);
            }
        }

        // ─── Navigation ─────────────────────────────────────────────
        KeyCode::Char('j') | KeyCode::Down => {
            let n = app.count_or_one();
            app.reset_vim_count();
            if app.active_panel == crate::app::ActivePanel::Sidebar {
                 app.move_down_n(n);
            } else {
                 if let Some(target) = motions::get_nav_target(app, 'j', n) {
                     app.selected_index = target;
                 }
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            let n = app.count_or_one();
            app.reset_vim_count();
            if app.active_panel == crate::app::ActivePanel::Sidebar {
                 app.move_up_n(n);
            } else {
                 if let Some(target) = motions::get_nav_target(app, 'k', n) {
                     app.selected_index = target;
                 }
            }
        }
        KeyCode::Char('h') | KeyCode::Left  => { app.reset_vim_count(); app.focus_left(); }
        KeyCode::Char('l') | KeyCode::Right => { app.reset_vim_count(); app.focus_right(); }

        // ─── Complex Motions (G, gg, 0, $) ──────────────────────────
        KeyCode::Char('G') => {
            app.record_jump();
            let count = app.count_or_one();
            if let Some(t) = motions::get_nav_target(app, 'G', count) {
                 app.selected_index = t;
            }
            app.reset_vim_count();
        }
        KeyCode::Char('0') => {
            app.reset_vim_count();
            if let Some(t) = motions::get_nav_target(app, '0', 1) {
                app.selected_index = t;
            }
        }
        KeyCode::Char('$') => {
            app.reset_vim_count();
            if let Some(t) = motions::get_nav_target(app, '$', 1) {
                 app.selected_index = t;
            }
        }

        // ─── Screen Motions (H, M, L) ──────────────────────────────
        KeyCode::Char('H') => {
            app.reset_vim_count();
            // Top of visible area
            app.selected_index = app.viewport_offset;
            if app.selected_index >= app.books.len() {
                app.selected_index = 0;
            }
        }
        KeyCode::Char('M') => {
            app.reset_vim_count();
            let visible_height = 20_usize;
            let mid = app.viewport_offset + visible_height / 2;
            app.selected_index = mid.min(app.books.len().saturating_sub(1));
        }
        KeyCode::Char('L') => {
            app.reset_vim_count();
            let visible_height = 20_usize;
            let bottom = app.viewport_offset + visible_height.saturating_sub(1);
            app.selected_index = bottom.min(app.books.len().saturating_sub(1));
        }

        // ─── Hierarchy ──────────────────────────────────────────────
        KeyCode::Char('-') | KeyCode::Backspace => {
             app.reset_vim_count();
             app.go_up();
        }

        // ─── Group Navigation ──────────────────────────────────────
        KeyCode::Char('{') => {
             app.reset_vim_count();
             app.move_prev_group();
        }
        KeyCode::Char('}') => {
             app.reset_vim_count();
             app.move_next_group();
        }

        // ─── Jump List ──────────────────────────────────────────────
        KeyCode::Char('o') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.reset_vim_count();
            app.jump_back();
        }
        KeyCode::Char('i') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.reset_vim_count();
            app.jump_forward();
        }

        // ─── Pending leaders ────────────────────────────────────────
        KeyCode::Char('g') => { app.pending_key = Some('g'); }
        KeyCode::Char('z') => { app.pending_key = Some('z'); }
        KeyCode::Char('m') => { app.pending_key = Some('m'); }
        KeyCode::Char('\'') => { app.pending_key = Some('\''); }
        KeyCode::Char('[') => { app.pending_key = Some('['); }
        KeyCode::Char(']') => { app.pending_key = Some(']'); }
        KeyCode::Char(' ') => { app.pending_key = Some(' '); }

        // ─── Scroll (Ctrl variants MUST precede plain char matches) ──
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
        KeyCode::Char('f') if modifiers.contains(KeyModifiers::CONTROL) => {
            let n = 20 * app.count_or_one();
            app.reset_vim_count();
            app.move_down_n(n);
        }
        KeyCode::Char('b') if modifiers.contains(KeyModifiers::CONTROL) => {
            let n = 20 * app.count_or_one();
            app.reset_vim_count();
            app.move_up_n(n);
        }
        KeyCode::Char('e') if modifiers.contains(KeyModifiers::CONTROL) => {
            let n = app.count_or_one();
            app.reset_vim_count();
            app.viewport_offset = app.viewport_offset.saturating_add(n);
            let max_offset = app.books.len().saturating_sub(1);
            if app.viewport_offset > max_offset {
                app.viewport_offset = max_offset;
            }
        }
        KeyCode::Char('y') if modifiers.contains(KeyModifiers::CONTROL) => {
            let n = app.count_or_one();
            app.reset_vim_count();
            app.viewport_offset = app.viewport_offset.saturating_sub(n);
        }

        // ─── Operators ──────────────────────────────────────────────
        KeyCode::Char('d') if !modifiers.contains(KeyModifiers::CONTROL) => {
             app.pending_operator = Some(Operator::Delete);
             app.mode = Mode::Pending;
        }
        KeyCode::Char('y') if !modifiers.contains(KeyModifiers::CONTROL) => {
             app.pending_operator = Some(Operator::Yank);
             app.mode = Mode::Pending;
        }
        KeyCode::Char('c') => {
             app.pending_operator = Some(Operator::Change);
             app.mode = Mode::Pending;
        }
        KeyCode::Char('>') => {
             app.pending_operator = Some(Operator::AddTag);
             app.mode = Mode::Pending;
        }
        KeyCode::Char('<') => {
             app.pending_operator = Some(Operator::RemoveTag);
             app.mode = Mode::Pending;
        }
        
        KeyCode::Char('p') => {
             app.reset_vim_count();
             app.paste_from_register();
        }
        
        KeyCode::Char('"') => { app.pending_register_select = true; }

        // ─── Undo / Redo ────────────────────────────────────────────
        KeyCode::Char('u') if !modifiers.contains(KeyModifiers::CONTROL) => {
            app.reset_vim_count();
            app.undo();
        }
        KeyCode::Char('r') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.reset_vim_count();
            app.redo();
        }

        // ─── Find Character Motions ────────────────────────────────
        KeyCode::Char('f') | KeyCode::Char('F') | KeyCode::Char('t') | KeyCode::Char('T')
            if !modifiers.contains(KeyModifiers::CONTROL) =>
        {
             if let KeyCode::Char(c) = code {
                  app.pending_key = Some(c);
             }
        }
        KeyCode::Char(';') => {
             if let Some((target_char, motion)) = app.last_find_char {
                  let n = app.count_or_one();
                  app.reset_vim_count();
                  if let Some(target) = find_char::get_find_char_target(app, motion, target_char, n) {
                      app.selected_index = target;
                  }
             }
        }
        KeyCode::Char(',') => {
             if let Some((target_char, motion)) = app.last_find_char {
                  let opp_motion = match motion {
                      'f' => 'F', 'F' => 'f', 't' => 'T', 'T' => 't', _ => motion
                  };
                  let n = app.count_or_one();
                  app.reset_vim_count();
                  if let Some(target) = find_char::get_find_char_target(app, opp_motion, target_char, n) {
                      app.selected_index = target;
                  }
             }
        }

        // ─── Book Operations ────────────────────────────────────────
        KeyCode::Char('a') if !modifiers.contains(KeyModifiers::CONTROL) => {
            app.reset_vim_count(); app.open_add_popup();
        }
        KeyCode::Char('o') if !modifiers.contains(KeyModifiers::CONTROL) => {
            app.reset_vim_count(); app.open_selected_book();
        }
        KeyCode::Char('R') => {
            app.reset_vim_count();
            app.popup = Some(Popup::SetRating {
                id: app.selected_book().map(|b| b.id.to_string()).unwrap_or_default(),
                current: app.selected_book().and_then(|b| b.rating),
            });
        }
        KeyCode::Char('s') => { app.reset_vim_count(); app.cycle_status(); }
        KeyCode::Char('S') => { 
            app.reset_vim_count(); 
            app.pending_key = Some('S'); 
        }

        // ─── Modes & Search ─────────────────────────────────────────
        KeyCode::Char('i') if !modifiers.contains(KeyModifiers::CONTROL) => {
             app.reset_vim_count();
             app.open_add_popup();
             app.status_message = "-- INSERT --".to_string();
        }
        KeyCode::Char(':') => {
            app.reset_vim_count();
            app.mode = Mode::Command;
            app.command_input.clear();
            app.command_history_idx = None;
        }

        // ─── Misc ───────────────────────────────────────────────────
        KeyCode::Enter => {
            app.reset_vim_count();
            if app.active_panel == crate::app::ActivePanel::Sidebar {
                app.select_sidebar_item();
            }
        }
        KeyCode::Tab    => { app.reset_vim_count(); app.focus_right(); }
        KeyCode::BackTab => { app.reset_vim_count(); app.focus_left(); }

        _ => { app.reset_vim_count(); }
    }
}

fn handle_pending_sequence(app: &mut App, pending: char, code: KeyCode) {
    app.pending_key = None; // consume pending
    match (pending, code) {
        ('g', c) => g_commands::handle_g_command(app, c),
        ('z', c) => z_commands::handle_z_command(app, c),
        // m<char> — set mark
        ('m', KeyCode::Char(c)) if c.is_ascii_alphabetic() => {
            app.set_mark(c);
        }
        // '<char> — jump to mark
        ('\'', KeyCode::Char(c)) if c.is_ascii_alphabetic() => {
            app.record_jump();
            app.jump_to_mark(c);
        }
        // '' — jump to last position before jump
        ('\'', KeyCode::Char('\'')) => {
            if let Some(last_pos) = app.last_jump_pos {
                let current = app.selected_index;
                app.last_jump_pos = Some(current);
                if last_pos < app.books.len() {
                    app.selected_index = last_pos;
                    app.status_message = format!("Jumped to last position: {}", last_pos + 1);
                }
            } else {
                app.status_message = "No previous jump position".to_string();
            }
        }
        // '< — jump to start of last visual selection
        ('\'', KeyCode::Char('<')) => {
            if let Some((start, _)) = app.last_visual_range {
                if start < app.books.len() {
                    app.selected_index = start;
                    app.status_message = format!("Jump to '< ({})", start + 1);
                }
            }
        }
        // '> — jump to end of last visual selection  
        ('\'', KeyCode::Char('>')) => {
            if let Some((_, end)) = app.last_visual_range {
                if end < app.books.len() {
                    app.selected_index = end;
                    app.status_message = format!("Jump to '> ({})", end + 1);
                }
            }
        }
        // [[ or ]]
        ('[', KeyCode::Char('[')) => {
            app.move_prev_group();
        }
        (']', KeyCode::Char(']')) => {
            app.move_next_group();
        }
        // S - Sort menu
        ('S', code) => handle_sort_command(app, code),
        // <Space> - EasyMotion or Leader
        (' ', code) => easy_motion::handle_easy_motion_start(app, code),
        // f/F/t/T - find char
        (motion @ ('f' | 'F' | 't' | 'T'), KeyCode::Char(target_char)) => {
            let n = app.count_or_one();
            app.reset_vim_count();
            if let Some(target) = find_char::get_find_char_target(app, motion, target_char, n) {
                app.selected_index = target;
                app.last_find_char = Some((target_char, motion));
            }
        }
        // Q<char> - start macro recording (our internal mapping for 'q')
        ('Q', KeyCode::Char(c)) if c.is_ascii_lowercase() => {
            app.macro_recorder.start_recording(c);
            app.status_message = format!("Recording @{c}...");
        }
        ('Q', _) => {
            app.status_message = "Invalid register for macro (use a-z)".to_string();
        }
        // @<char> - replay macro
        ('@', KeyCode::Char('@')) => {
            // @@ — replay last macro
            if let Some(last) = app.macro_recorder.last_played {
                let count = app.count_or_one();
                app.reset_vim_count();
                replay_macro(app, last, count);
            } else {
                app.status_message = "No last macro to replay".to_string();
            }
        }
        ('@', KeyCode::Char(c)) if c.is_ascii_lowercase() => {
            let count = app.count_or_one();
            app.reset_vim_count();
            replay_macro(app, c, count);
        }
        ('@', _) => {
            app.status_message = "Invalid register for macro replay".to_string();
        }
        _ => {}
    }
}

/// Replay a macro `count` times.
fn replay_macro(app: &mut App, reg: char, count: usize) {
    if let Some(keys) = app.macro_recorder.get_macro(reg).cloned() {
        app.macro_recorder.last_played = Some(reg);
        for _ in 0..count {
            for (code, mods) in &keys {
                handle_key(app, *code, *mods);
            }
        }
        app.status_message = format!("Replayed @{reg} ×{count}");
    } else {
        app.status_message = format!("Macro @{reg} is empty");
    }
}

/// Search for the next/previous match of `last_search`.
fn search_next(app: &mut App, reverse: bool) {
    let query = match app.last_search.clone() {
        Some(q) if !q.is_empty() => q,
        _ => {
            app.status_message = "No previous search".to_string();
            return;
        }
    };

    let query_lower = query.to_lowercase();
    let len = app.books.len();
    if len == 0 { return; }

    let forward = match (app.search_direction, reverse) {
        (SearchDirection::Forward, false) | (SearchDirection::Backward, true) => true,
        (SearchDirection::Forward, true) | (SearchDirection::Backward, false) => false,
    };

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

fn handle_sort_command(app: &mut App, code: KeyCode) {
    use crate::app::SortKey;
    match code {
        KeyCode::Char('y') => {
            app.sort_key = SortKey::YearDesc;
            app.apply_sort();
            app.status_message = "Sort: Year Desc".to_string();
        }
        KeyCode::Char('Y') => {
            app.sort_key = SortKey::YearAsc;
            app.apply_sort();
            app.status_message = "Sort: Year Asc".to_string();
        }
        KeyCode::Char('t') => {
            app.sort_key = SortKey::TitleAsc;
            app.apply_sort();
            app.status_message = "Sort: Title Asc".to_string();
        }
        KeyCode::Char('r') => {
            app.sort_key = SortKey::RatingDesc;
            app.apply_sort();
            app.status_message = "Sort: Rating Desc".to_string();
        }
        KeyCode::Char('f') => {
            app.sort_key = SortKey::FrecencyDesc;
            app.apply_sort();
            app.status_message = "Sort: Frecency".to_string();
        }
        KeyCode::Char('u') => {
            app.sort_key = SortKey::UpdatedDesc;
            app.apply_sort();
            app.status_message = "Sort: Updated (Default)".to_string();
        }
        _ => {
             if code == KeyCode::Esc {
                 app.status_message.clear();
             } else {
                 app.status_message = "Sort cancelled".to_string();
             }
        }
    }
}

fn handle_command_mode(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc => {
            app.mode = Mode::Normal;
            app.command_input.clear();
            app.command_history_idx = None;
        }
        KeyCode::Enter => {
            let cmd = app.command_input.clone();
            app.mode = Mode::Normal;
            if !cmd.is_empty() {
                app.command_history.push(cmd.clone());
            }
            app.command_history_idx = None;
            execute_command(app, &cmd);
            app.command_input.clear();
        }
        KeyCode::Backspace => {
            app.command_input.pop();
            if app.command_input.is_empty() {
                app.mode = Mode::Normal;
            }
        }
        KeyCode::Up => {
            // Navigate command history backward
            if app.command_history.is_empty() { return; }
            let idx = match app.command_history_idx {
                None => app.command_history.len().saturating_sub(1),
                Some(i) => i.saturating_sub(1),
            };
            app.command_history_idx = Some(idx);
            if let Some(cmd) = app.command_history.get(idx) {
                app.command_input = cmd.clone();
            }
        }
        KeyCode::Down => {
            if let Some(idx) = app.command_history_idx {
                if idx + 1 < app.command_history.len() {
                    app.command_history_idx = Some(idx + 1);
                    app.command_input = app.command_history[idx + 1].clone();
                } else {
                    app.command_history_idx = None;
                    app.command_input.clear();
                }
            }
        }
        KeyCode::Char(c) => {
            app.command_input.push(c);
            app.command_history_idx = None;
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

fn execute_command(app: &mut App, cmd: &str) {
    use crate::command::{parse_command, CommandAction};
    
    match parse_command(cmd) {
        CommandAction::Quit => app.should_quit = true,
        CommandAction::Write => {
            app.status_message = "Saved.".to_string();
        }
        CommandAction::WriteQuit => {
            app.status_message = "Saved.".to_string();
            app.should_quit = true;
        }
        CommandAction::Add => app.open_add_popup(),
        CommandAction::Open => app.open_selected_book(),
        CommandAction::Tags => app.open_edit_tags(),
        CommandAction::Help => app.show_help(),
        CommandAction::Search(query) => {
            app.open_telescope();
            if !query.is_empty() {
                if let Some(crate::popup::Popup::Telescope(ref mut state)) = app.popup {
                    state.query = query.clone();
                    state.cursor = query.len();
                }
                app.telescope_search(&query);
            }
        }
        CommandAction::Refresh => {
            app.refresh_books();
            app.status_message = "Refreshed.".to_string();
        }
        CommandAction::Global { pattern, command } => {
            if let Ok(re) = regex::Regex::new(&pattern) {
                let mut matched_indices = Vec::new();
                for (i, book) in app.books.iter().enumerate() {
                     let search_text = format!("{} {} {}", book.title, book.authors.join(" "), book.tags.join(" "));
                     if re.is_match(&search_text) {
                          matched_indices.push(i);
                     }
                }
                
                if matched_indices.is_empty() {
                     app.status_message = format!("No matches for pattern: {pattern}");
                } else {
                     if command == "d" {
                          app.delete_indices(&matched_indices);
                          app.status_message = format!("Global delete executed on {} items", matched_indices.len());
                     } else if command.starts_with("tag ") {
                          let tag = command.trim_start_matches("tag ").trim();
                          let mut cards = Vec::new();
                          let cards_dir = app.config.cards_dir();
                          for &idx in &matched_indices {
                               if let Some(view) = app.books.get(idx) {
                                    if let Ok(mut card) = omniscope_core::storage::json_cards::load_card_by_id(&cards_dir, &view.id) {
                                         if !card.organization.tags.contains(&tag.to_string()) {
                                             card.organization.tags.push(tag.to_string());
                                         }
                                         cards.push(card);
                                    }
                               }
                          }
                          
                          if !cards.is_empty() {
                               app.push_undo(
                                   format!("Global tag '{tag}' applied to {} items", cards.len()),
                                   omniscope_core::undo::UndoAction::UpsertCards(cards.clone())
                               );
                               for card in &cards {
                                    let _ = omniscope_core::storage::json_cards::save_card(&cards_dir, card);
                                    if let Some(ref db) = app.db {
                                         let _ = db.upsert_book(card);
                                    }
                               }
                               app.refresh_books();
                          }
                          app.status_message = format!("Global tag applied to {} items", cards.len());
                     } else {
                          app.status_message = format!("Global command not supported: {command}");
                     }
                }
            } else {
                 app.status_message = format!("Invalid regex pattern: {pattern}");
            }
        }
        CommandAction::Substitute { pattern, replacement, global } => {
             app.status_message = format!("Substitute `{pattern}` -> `{replacement}` (global: {global}) not yet fully implemented");
        }
        CommandAction::UndoList => {
            app.status_message = format!("Undo history: {} items, Redo: {} items", app.undo_stack.len(), app.redo_stack.len());
        }
        CommandAction::QuickfixOpen => {
            if app.quickfix_list.is_empty() {
                app.status_message = "Quickfix list is empty.".to_string();
            } else {
                app.quickfix_show = true;
                app.status_message = format!("Opened quickfix list with {} items", app.quickfix_list.len());
            }
        }
        CommandAction::QuickfixClose => {
            app.quickfix_show = false;
        }
        CommandAction::QuickfixNext => {
            if !app.quickfix_list.is_empty() {
                app.quickfix_selected = (app.quickfix_selected + 1).min(app.quickfix_list.len() - 1);
            }
        }
        CommandAction::QuickfixPrev => {
            if !app.quickfix_list.is_empty() {
                app.quickfix_selected = app.quickfix_selected.saturating_sub(1);
            }
        }
        CommandAction::QuickfixDo(command) => {
            if app.quickfix_list.is_empty() {
                app.status_message = "Quickfix list is empty.".to_string();
                return;
            }
            app.status_message = format!("Executing `{command}` on {} items (WIP)", app.quickfix_list.len());
        }
        CommandAction::Earlier(time_str) => {
            let duration = parse_duration(&time_str);
            let target_time = chrono::Utc::now() - duration;
            let mut count = 0;
            while let Some(entry) = app.undo_stack.last() {
                if entry.timestamp < target_time {
                    break;
                }
                app.undo();
                count += 1;
            }
            app.status_message = format!("Undid {} changes (back {})", count, time_str);
        }
        CommandAction::Later(time_str) => {
            app.status_message = format!(":later {time_str} is WIP. Use Ctrl+r to redo.");
        }
        CommandAction::Sort(field) => {
            use crate::app::SortKey;
            let key = match field.as_str() {
                "title" => SortKey::TitleAsc,
                "year" | "year_desc" => SortKey::YearDesc,
                "year_asc" => SortKey::YearAsc,
                "rating" => SortKey::RatingDesc,
                "frecency" => SortKey::FrecencyDesc,
                "updated" => SortKey::UpdatedDesc,
                _ => {
                    app.status_message = format!("Unknown sort field: {field}");
                    return;
                }
            };
            app.sort_key = key;
            app.apply_sort();
            app.status_message = format!("Sort: {}", key.label());
        }
        CommandAction::Library(name) => {
            app.sidebar_filter = crate::app::SidebarFilter::Library(name.clone());
            app.refresh_books();
            app.status_message = format!("Library: {name}");
        }
        CommandAction::FilterTag(tag) => {
            app.sidebar_filter = crate::app::SidebarFilter::Tag(tag.clone());
            app.refresh_books();
            app.status_message = format!("Tag filter: {tag}");
        }
        CommandAction::Marks => {
            let marks_display: Vec<String> = app.marks.iter()
                .map(|(&k, &v)| format!("'{k} -> {}", v + 1))
                .collect();
            if marks_display.is_empty() {
                app.status_message = "No marks set".to_string();
            } else {
                app.status_message = format!("Marks: {}", marks_display.join(" | "));
            }
        }
        CommandAction::Registers(reg) => {
            if let Some(r) = reg {
                if let Some(register) = app.registers.get(&r) {
                    let desc = match &register.content {
                        crate::app::RegisterContent::Card(c) => c.metadata.title.clone(),
                        crate::app::RegisterContent::MultipleCards(cards) => format!("{} cards", cards.len()),
                        crate::app::RegisterContent::Text(t) => t.clone(),
                        crate::app::RegisterContent::Path(p) => p.clone(),
                    };
                    app.status_message = format!("\"{r}: {desc}");
                } else {
                    app.status_message = format!("Register \"{r} is empty");
                }
            } else {
                let regs: Vec<String> = app.registers.keys()
                    .map(|k| format!("\"{k}"))
                    .collect();
                if regs.is_empty() {
                    app.status_message = "No registers".to_string();
                } else {
                    app.status_message = format!("Registers: {}", regs.join(" "));
                }
            }
        }
        CommandAction::DeleteMarks(marks_str) => {
            for c in marks_str.chars() {
                app.marks.remove(&c);
            }
            app.status_message = format!("Deleted marks: {marks_str}");
        }
        CommandAction::Macros => {
            let list = app.macro_recorder.list_macros();
            if list.is_empty() {
                app.status_message = "No macros recorded".to_string();
            } else {
                let desc: Vec<String> = list.iter()
                    .map(|(reg, len)| format!("@{reg} ({len} keys)"))
                    .collect();
                app.status_message = format!("Macros: {}", desc.join(" | "));
            }
        }
        CommandAction::Doctor => {
            let book_count = app.books.len();
            let all_count = app.all_books.len();
            let undo_count = app.undo_stack.len();
            let marks_count = app.marks.len();
            let reg_count = app.registers.len();
            let macro_count = app.macro_recorder.list_macros().len();
            app.status_message = format!(
                "Doctor: books={book_count}/{all_count} undo={undo_count} marks={marks_count} regs={reg_count} macros={macro_count}"
            );
        }
        CommandAction::Unknown(unknown_cmd) => {
            app.status_message = format!("Unknown command: {unknown_cmd}");
        }
    }
}

fn parse_duration(s: &str) -> chrono::Duration {
    let s = s.trim();
    if s.ends_with('m') {
        let mins: i64 = s[..s.len()-1].parse().unwrap_or(0);
        chrono::Duration::minutes(mins)
    } else if s.ends_with('h') {
        let hours: i64 = s[..s.len()-1].parse().unwrap_or(0);
        chrono::Duration::hours(hours)
    } else if s.ends_with('s') {
        let secs: i64 = s[..s.len()-1].parse().unwrap_or(0);
        chrono::Duration::seconds(secs)
    } else {
        chrono::Duration::zero()
    }
}
