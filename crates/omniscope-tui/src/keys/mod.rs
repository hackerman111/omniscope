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
#[cfg(test)]
mod tests;

use crossterm::event::{KeyCode, KeyModifiers};
use crate::app::{App, Mode};
use crate::popup::Popup;
use crate::keys::operator::Operator;

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
            app.push_vim_digit(c.to_digit(10).unwrap());
            return;
        }
        if c == '0' && app.vim_count > 0 {
             app.push_vim_digit(0);
             return;
        }
    }

    // ── Handle pending key sequences (g, z, m, ') ─────────────────────
    if let Some(pending) = app.pending_key {
        handle_pending_sequence(app, pending, code);
        return;
    }

    match code {
        // ─── Quit / Quickfix ───────────────────────────────────────
        KeyCode::Char('q') => {
            if modifiers.contains(KeyModifiers::CONTROL) {
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
            } else {
                app.should_quit = true;
            }
        }

        // ─── Visual Mode Entry ─────────────────────────────────────
        KeyCode::Char('v') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.enter_visual_mode(Mode::VisualBlock);
        }
        KeyCode::Char('v') => {
            app.enter_visual_mode(Mode::Visual);
        }
        KeyCode::Char('/') => {
            app.open_telescope(); // / enters search via telescope popup
        }
        KeyCode::Char('?') => {
            app.open_telescope(); // ? could be reverse search, but mapped to telescope for now
        }
        KeyCode::Char('n') => {
            // Jump to next match of last search.
            // Since Telescope does filtering and updates `app.books`,
            // "next match" in a filtered list just means moving down, but if we aren't filtering,
            // we'd need a real "search forward" logic.
            // For now, if we have a search query, applying it filters the list. So 'n' usually
            // just means "go down" in the filtered list.
            // A true vim search would jump the cursor matching a regex in the unfiltered list.
            app.status_message = "Search next ('n') requires full regex search buffer implementation.".to_string();
        }
        KeyCode::Char('N') => {
            app.status_message = "Search prev ('N') requires full regex search buffer implementation.".to_string();
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
            app.record_jump(); // Push current pos to jump list before moving
            let n = app.count_or_one(); 
            // ...
            if let Some(t) = motions::get_nav_target(app, 'G', n) {
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
            app.move_to_top();
        }
        KeyCode::Char('M') => {
            app.reset_vim_count();
            // Middle: approximate
            let mid = app.books.len() / 2;
            if mid < app.books.len() {
                app.selected_index = mid;
            }
        }
        KeyCode::Char('L') => {
            app.reset_vim_count();
            app.move_to_bottom();
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

        // ─── Operators ──────────────────────────────────────────────
        KeyCode::Char('d') if !modifiers.contains(KeyModifiers::CONTROL) => {
             app.pending_operator = Some(Operator::Delete);
             app.mode = Mode::Pending;
             // We do NOT use Pending mode for `dd`, we used to check it here,
             // but `Mode::Pending` will handle the next key.
        }
        KeyCode::Char('y') => {
             app.pending_operator = Some(Operator::Yank);
             app.mode = Mode::Pending;
        }
        KeyCode::Char('c') => {
             app.pending_operator = Some(Operator::Change);
             app.mode = Mode::Pending;
        }
        KeyCode::Char('>') => {
             app.pending_operator = Some(Operator::AddTag); // Or Indent
             app.mode = Mode::Pending;
        }
        KeyCode::Char('<') => {
             app.pending_operator = Some(Operator::RemoveTag); // or Outdent
             app.mode = Mode::Pending;
        }
        
        KeyCode::Char('p') => {
             app.reset_vim_count();
             app.paste_from_register();
        }
        
        KeyCode::Char('"') => { app.pending_register_select = true; }

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

        // ─── Find Character Motions ────────────────────────────────
        KeyCode::Char('f') | KeyCode::Char('F') | KeyCode::Char('t') | KeyCode::Char('T') => {
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
        KeyCode::Char('S') => { 
            app.reset_vim_count(); 
            app.pending_key = Some('S'); 
        }

        // ─── Modes & Search ─────────────────────────────────────────
        KeyCode::Char('i') if !modifiers.contains(KeyModifiers::CONTROL) => {
             // Basic "insert" mode in this TUI currently maps to opening the add/edit form
             app.reset_vim_count();
             app.open_add_popup();
             app.status_message = "-- INSERT --".to_string();
        }
        KeyCode::Char(':') => {
            app.reset_vim_count();
            app.mode = Mode::Command;
            app.command_input.clear();
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
            app.jump_to_mark(c);
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
        _ => {}
    }
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
             // Invalid key or ESC cancels
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
                // Find matching books
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
                     // For now, support simple commands like 'd' (delete) or 'tag foo'
                     // A real implementation might feed `command` recursively to parse_command
                     if command == "d" {
                          app.delete_indices(&matched_indices);
                          app.status_message = format!("Global delete executed on {} items", matched_indices.len());
                     } else if command.starts_with("tag ") {
                          let tag = command.trim_start_matches("tag ").trim();
                          // Load all cards, add tag, save
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
             // Basic implementation placeholder for %s/foo/bar/g
             // Full renaming logic requires loading all cards, regex replace title/tags, and saving.
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
            // For later, we just redo
            app.status_message = format!(":later {time_str} is WIP. Use Ctrl+r to redo.");
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
