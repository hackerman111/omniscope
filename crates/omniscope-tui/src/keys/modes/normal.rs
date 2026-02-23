use crate::app::{App, Mode, SearchDirection};
use crate::keys::core::motions;
use crate::keys::core::operator::Operator;
use crate::keys::ext::easy_motion;
use crate::keys::ext::find_char;
use crate::keys::ext::g_commands;
use crate::keys::ext::science_bindings;
use crate::keys::ext::sort;
use crate::keys::ext::z_commands;
use crate::keys::modes::search;
use crate::panels::citation_graph::GraphMode;
use crate::popup::Popup;
use crossterm::event::{KeyCode, KeyModifiers};

pub(crate) fn handle_normal_mode(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
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

    if modifiers == KeyModifiers::NONE
        && app.active_panel == crate::app::ActivePanel::Preview
        && app.has_science_context()
        && handle_preview_science_shortcut(app, code)
    {
        app.reset_vim_count();
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
                app.quickfix_list = app
                    .visual_selections
                    .iter()
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
            search::search_next(app, false);
        }
        KeyCode::Char('N') => {
            search::search_next(app, true);
        }

        // ─── Word-under-cursor Search ───────────────────────────────
        KeyCode::Char('*') => {
            // Search for current book's author (forward)
            if let Some(book) = app.selected_book() {
                let query = book
                    .authors
                    .first()
                    .cloned()
                    .unwrap_or_else(|| book.title.clone());
                app.last_search = Some(query.clone());
                app.search_direction = SearchDirection::Forward;
                search::search_next(app, false);
            }
        }
        KeyCode::Char('#') => {
            // Search for current book's author (backward)
            if let Some(book) = app.selected_book() {
                let query = book
                    .authors
                    .first()
                    .cloned()
                    .unwrap_or_else(|| book.title.clone());
                app.last_search = Some(query.clone());
                app.search_direction = SearchDirection::Backward;
                search::search_next(app, false);
            }
        }

        // ─── Navigation ─────────────────────────────────────────────
        KeyCode::Char('j') | KeyCode::Down => {
            let n = app.count_or_one();
            app.reset_vim_count();
            if app.active_panel == crate::app::ActivePanel::Sidebar {
                app.move_down_n(n);
            } else if app.active_panel == crate::app::ActivePanel::Preview {
                app.preview_scroll_down(n);
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
            } else if app.active_panel == crate::app::ActivePanel::Preview {
                app.preview_scroll_up(n);
            } else {
                if let Some(target) = motions::get_nav_target(app, 'k', n) {
                    app.selected_index = target;
                }
            }
        }
        KeyCode::Char('h') | KeyCode::Left => {
            app.reset_vim_count();
            app.focus_left();
        }
        KeyCode::Char('l') | KeyCode::Right => {
            app.reset_vim_count();
            app.focus_right();
        }

        // ─── Complex Motions (G, gg, 0, $) ──────────────────────────
        KeyCode::Char('G') => {
            app.record_jump();
            if app.active_panel == crate::app::ActivePanel::Sidebar {
                app.move_to_bottom();
            } else {
                let count = if app.has_explicit_count {
                    app.vim_count as usize
                } else {
                    0
                };
                if let Some(t) = motions::get_nav_target(app, 'G', count) {
                    app.selected_index = t;
                }
            }
            app.reset_vim_count();
        }
        KeyCode::Char('0') => {
            app.reset_vim_count();
            if app.active_panel == crate::app::ActivePanel::Sidebar {
                app.sidebar_selected = 0;
            } else if let Some(t) = motions::get_nav_target(app, '0', 1) {
                app.selected_index = t;
            }
        }
        KeyCode::Char('$') => {
            app.reset_vim_count();
            if app.active_panel == crate::app::ActivePanel::Sidebar {
                app.move_to_bottom();
            } else if let Some(t) = motions::get_nav_target(app, '$', 1) {
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
        KeyCode::Char('g') => {
            app.pending_key = Some('g');
        }
        KeyCode::Char('z') => {
            app.pending_key = Some('z');
        }
        KeyCode::Char('m') => {
            app.pending_key = Some('m');
        }
        KeyCode::Char('\'') => {
            app.pending_key = Some('\'');
        }
        KeyCode::Char('[') => {
            app.pending_key = Some('[');
        }
        KeyCode::Char(']') => {
            app.pending_key = Some(']');
        }
        KeyCode::Char(' ') => {
            app.pending_key = Some(' ');
        }

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

        KeyCode::Char('"') => {
            app.pending_register_select = true;
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
                    'f' => 'F',
                    'F' => 'f',
                    't' => 'T',
                    'T' => 't',
                    _ => motion,
                };
                let n = app.count_or_one();
                app.reset_vim_count();
                if let Some(target) =
                    find_char::get_find_char_target(app, opp_motion, target_char, n)
                {
                    app.selected_index = target;
                }
            }
        }

        // ─── Book Operations ────────────────────────────────────────
        KeyCode::Char('a') if !modifiers.contains(KeyModifiers::CONTROL) => {
            app.reset_vim_count();
            app.open_add_popup();
        }
        KeyCode::Char('o') if !modifiers.contains(KeyModifiers::CONTROL) => {
            app.reset_vim_count();
            app.open_selected_book();
        }
        KeyCode::Char('R') => {
            app.reset_vim_count();
            app.popup = Some(Popup::SetRating {
                id: app
                    .selected_book()
                    .map(|b| b.id.to_string())
                    .unwrap_or_default(),
                current: app.selected_book().and_then(|b| b.rating),
            });
        }
        KeyCode::Char('s') => {
            app.reset_vim_count();
            app.cycle_status();
        }
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
            match app.active_panel {
                crate::app::ActivePanel::Sidebar => app.select_sidebar_item(),
                crate::app::ActivePanel::BookList => app.open_selected_book(),
                _ => {}
            }
        }
        KeyCode::Tab => {
            app.reset_vim_count();
            app.focus_right();
        }
        KeyCode::BackTab => {
            app.reset_vim_count();
            app.focus_left();
        }

        _ => {
            app.reset_vim_count();
        }
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
        ('S', code) => sort::handle_sort_command(app, code),
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
        // @<char> - science actions or macro replay
        ('@', KeyCode::Char(c)) => {
            if c == '@' {
                if let Some(last) = app.macro_recorder.last_played {
                    let count = app.count_or_one();
                    app.reset_vim_count();
                    replay_macro(app, last, count);
                } else {
                    app.status_message = "No last macro to replay".to_string();
                }
                return;
            }

            if science_bindings::handle_at_science_command(app, c) {
                app.reset_vim_count();
                return;
            }

            if c.is_ascii_lowercase() {
                let count = app.count_or_one();
                app.reset_vim_count();
                replay_macro(app, c, count);
            } else {
                app.status_message = "Invalid register for macro replay".to_string();
            }
        }
        ('@', _) => app.status_message = "Invalid register for macro replay".to_string(),
        _ => {}
    }
}

/// Replay a macro `count` times.
fn replay_macro(app: &mut App, reg: char, count: usize) {
    if let Some(keys) = app.macro_recorder.get_macro(reg).cloned() {
        app.macro_recorder.last_played = Some(reg);
        for _ in 0..count {
            for (code, mods) in &keys {
                crate::keys::handle_key(app, *code, *mods);
            }
        }
        app.status_message = format!("Replayed @{reg} ×{count}");
    } else {
        app.status_message = format!("Macro @{reg} is empty");
    }
}

fn handle_preview_science_shortcut(app: &mut App, code: KeyCode) -> bool {
    match code {
        KeyCode::Char('r') => {
            app.open_science_references_panel();
            true
        }
        KeyCode::Char('c') => {
            app.open_science_citation_graph_panel(GraphMode::CitedBy);
            true
        }
        KeyCode::Char('o') => {
            app.find_science_open_pdf();
            true
        }
        KeyCode::Char('f') => {
            app.open_science_find_download_panel(None);
            true
        }
        KeyCode::Char('e') => {
            app.show_science_bibtex();
            true
        }
        _ => false,
    }
}
