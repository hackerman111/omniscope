pub mod app;
pub mod event;
pub mod popup;
pub mod ui;

use std::io;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use app::{App, Mode};
use event::{AppEvent, EventHandler};
use popup::Popup;

/// Run the full TUI application.
pub fn run_tui(app: &mut App) -> Result<()> {
    // Install panic hook: if a panic occurs, restore the terminal before
    // printing the panic message, so the user can read it in their shell.
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = std::io::stdout().execute(crossterm::terminal::LeaveAlternateScreen);
        original_hook(info);
    }));

    // Setup terminal
    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let event_handler = EventHandler::new(Duration::from_millis(250));

    // Main loop
    loop {
        // Render
        terminal.draw(|frame| ui::render(frame, app))?;

        // Handle events
        match event_handler.next()? {
            AppEvent::Key(key) => handle_key(app, key.code, key.modifiers),
            AppEvent::Resize(_, _) => {}
            AppEvent::Tick => {}
        }

        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

/// Central key handler — dispatches based on popup or mode.
fn handle_key(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    // Popup takes priority
    if app.popup.is_some() {
        handle_popup_key(app, code, modifiers);
        return;
    }

    match app.mode {
        Mode::Normal => handle_normal_mode(app, code, modifiers),
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
        let count = app.count_or_one();
        app.reset_vim_count();

        match (pending, code) {
            // gg — go to top
            ('g', KeyCode::Char('g')) => { app.move_to_top(); return; }
            // gt — quick-edit title (open add-book form with this book's title pre-filled)
            ('g', KeyCode::Char('t')) => {
                if let Some(book) = app.selected_book() {
                    let title = book.title.clone();
                    app.open_add_popup();
                    if let Some(crate::popup::Popup::AddBook(ref mut form)) = app.popup {
                        form.fields[0].value = title;
                        form.fields[0].cursor = form.fields[0].value.len();
                    }
                }
                return;
            }
            // gr — set rating (alias for R)
            ('g', KeyCode::Char('r')) => {
                app.popup = Some(crate::popup::Popup::SetRating {
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
                // dd = delete current book
                for _ in 0..count { app.open_delete_confirm(); }
                return;
            }
            // y<motion> — yank
            ('y', KeyCode::Char('y')) => {
                for _ in 0..count { app.yank_selected(); }
                return;
            }
            _ => {} // unknown sequence
        }
        return;
    }

    match code {
        // ─── Quit ──────────────────────────────────────────────────
        KeyCode::Char('q') => app.should_quit = true,

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

        // ─── Pending leaders: g / z / m / ' / d / y ───────────────
        KeyCode::Char('g') => { app.pending_key = Some('g'); }
        KeyCode::Char('z') => { app.pending_key = Some('z'); }
        KeyCode::Char('m') => { app.pending_key = Some('m'); }
        KeyCode::Char('\'') => { app.pending_key = Some('\''); }
        KeyCode::Char('d') if !modifiers.contains(KeyModifiers::CONTROL) => {
            app.pending_key = Some('d');
        }
        KeyCode::Char('y') => { app.pending_key = Some('y'); }

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
            if app.active_panel == app::ActivePanel::Sidebar {
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
            // Live fuzzy search as you type
            app.fuzzy_search(&app.search_input.clone());
        }
        KeyCode::Char(c) => {
            app.search_input.push(c);
            // Live fuzzy search as you type
            app.fuzzy_search(&app.search_input.clone());
        }
        _ => {}
    }
}

fn handle_popup_key(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {


    match &mut app.popup {
        Some(Popup::AddBook(form)) => {
            // Check if we are editing the "File path" field (index 5)
            let is_path_field = form.active_field == 5;
            
            match code {
                KeyCode::Esc => {
                    // If autocomplete is active, close it first
                    if form.autocomplete.active {
                        form.autocomplete.clear();
                    } else {
                        app.popup = None;
                    }
                }
                KeyCode::Enter => {
                    if form.autocomplete.active {
                        if let Some(selection) = form.autocomplete.current().map(|s| s.to_string()) {
                            // Apply selection
                            // If selection ends with /, it's a directory -> keep it open for more typing
                            // If it's a file, we might be done
                            
                            // We need to resolve the full path based on what was typed
                            // The autocomplete logic below (update_filepath_suggestions) handles the resolution
                            // Here we just want to replace the current input with the selection
                            
                            // Reconstruction logic:
                            // We need to know the directory context of the selection. 
                            // Easier approach: the selection in dropdown should probably be the FULL path or relative path 
                            // that we can just drop in.
                            // In our previous logic, we listed filenames.
                            // Let's adapt `update_filepath_suggestions` to store full valid paths in candidates?
                            // OR, we recompose it here.
                            
                            let field = &mut form.fields[5];
                            let current = &field.value;
                            
                            // Resolve header directory
                            let expanded = if current.starts_with("~/") {
                                if let Some(home) = dirs::home_dir() {
                                    current.replacen("~", &home.to_string_lossy(), 1)
                                } else {
                                    current.clone()
                                }
                            } else {
                                current.clone()
                            };
                            
                            let path = std::path::Path::new(&expanded);
                            let dir = if expanded.ends_with('/') {
                                path.to_path_buf()
                            } else {
                                path.parent().unwrap_or(std::path::Path::new(".")).to_path_buf()
                            };
                            
                            let new_full_path = dir.join(&selection).to_string_lossy().to_string();
                            
                            // Restore ~ if needed
                            let final_val = if current.starts_with("~/") {
                                if let Some(home) = dirs::home_dir() {
                                    let h = home.to_string_lossy();
                                    if new_full_path.starts_with(h.as_ref()) {
                                        new_full_path.replacen(h.as_ref(), "~", 1)
                                    } else {
                                        new_full_path
                                    }
                                } else {
                                    new_full_path
                                }
                            } else {
                                new_full_path
                            };
                            
                            field.value = final_val;
                            
                            // Check if dir to append /
                            // We need to expand again to check metadata safely
                            let check_path = if field.value.starts_with("~/") {
                                 if let Some(home) = dirs::home_dir() {
                                     field.value.replacen("~", &home.to_string_lossy(), 1)
                                 } else {
                                     field.value.clone()
                                 }
                            } else {
                                field.value.clone()
                            };
                            
                            if let Ok(m) = std::fs::metadata(check_path) {
                                if m.is_dir() && !field.value.ends_with('/') {
                                    field.value.push('/');
                                }
                            }
                            
                            field.cursor = field.value.len();
                            
                            // If it's a directory, trigger suggestions again for the new dir
                            if field.value.ends_with('/') {
                                update_filepath_suggestions(form);
                            } else {
                                form.autocomplete.clear();
                            }
                        } else {
                             form.autocomplete.clear();
                        }
                    } else {
                        app.submit_add_book();
                    }
                }
                KeyCode::Tab => {
                    if form.autocomplete.active {
                        // Cycle selection
                        form.autocomplete.tab_next();
                    } else if is_path_field {
                         // Trigger autocomplete manually
                         update_filepath_suggestions(form);
                         if !form.autocomplete.active {
                             form.next_field();
                         }
                    } else {
                        form.next_field();
                    }
                }
                KeyCode::BackTab => {
                    if form.autocomplete.active {
                        form.autocomplete.move_up();
                    } else {
                        form.prev_field();
                    }
                }
                KeyCode::Up => {
                    if form.autocomplete.active {
                        form.autocomplete.move_up();
                    }
                }
                KeyCode::Down => {
                    if form.autocomplete.active {
                        form.autocomplete.move_down();
                    }
                }
                KeyCode::Backspace => {
                    form.active_field_mut().delete_back();
                    if is_path_field {
                        update_filepath_suggestions(form);
                    }
                }
                KeyCode::Left => form.active_field_mut().move_left(),
                KeyCode::Right => form.active_field_mut().move_right(),
                KeyCode::Char(c) => {
                    form.active_field_mut().insert_char(c);
                    if is_path_field {
                        update_filepath_suggestions(form);
                    }
                }
                _ => {}
            }
        }

        Some(Popup::DeleteConfirm { .. }) => match code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                app.confirm_delete();
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                app.popup = None;
            }
            _ => {}
        },

        Some(Popup::SetRating { .. }) => match code {
            KeyCode::Char(c) if ('1'..='5').contains(&c) => {
                let rating = c.to_digit(10).unwrap() as u8;
                app.popup = None;
                app.set_rating(rating);
            }
            KeyCode::Char('0') => {
                // Clear rating
                if let Some(book) = app.selected_book() {
                    let id = book.id;
                    let cards_dir = app.config.cards_dir();
                    if let Ok(mut card) = omniscope_core::storage::json_cards::load_card_by_id(&cards_dir, &id) {
                        card.organization.rating = None;
                        card.updated_at = chrono::Utc::now();
                        let _ = omniscope_core::storage::json_cards::save_card(&cards_dir, &card);
                        if let Some(ref db) = app.db {
                            let _ = db.upsert_book(&card);
                        }
                    }
                }
                app.popup = None;
                app.status_message = "Rating cleared".to_string();
                app.refresh_books();
            }
            KeyCode::Esc => {
                app.popup = None;
            }
            _ => {}
        },

        Some(Popup::SetStatus { .. }) => match code {
            KeyCode::Esc => { app.popup = None; }
            _ => {}
        },

        Some(Popup::EditTags(form)) => match code {
            KeyCode::Esc => {
                app.popup = None;
            }
            KeyCode::Enter => {
                if !form.input.is_empty() {
                    form.add_tag();
                } else {
                    app.submit_edit_tags();
                }
            }
            KeyCode::Backspace => {
                if form.input.is_empty() {
                    form.remove_last_tag();
                } else {
                    form.input.pop();
                    if form.cursor > 0 {
                        form.cursor -= 1;
                    }
                }
            }
            KeyCode::Char(c) => {
                form.input.push(c);
                form.cursor += 1;
            }
            _ => {}
        },

        Some(Popup::Telescope(state)) => {
            use crate::popup::TelescopeMode;

            match state.mode {
                TelescopeMode::Insert => match code {
                    // ── Exit overlay ──────────────────────────────
                    KeyCode::Esc => {
                        if state.autocomplete.active {
                            // First Esc: dismiss autocomplete
                            state.autocomplete.clear();
                        } else {
                            app.popup = None;
                        }
                    }

                    // ── Autocomplete: ↑ / ↓ navigate dropdown ────
                    KeyCode::Up => {
                        if state.autocomplete.active {
                            state.autocomplete.move_up();
                        } else {
                            state.result_up();
                        }
                    }
                    KeyCode::Down => {
                        if state.autocomplete.active {
                            state.autocomplete.move_down();
                        } else {
                            // APPROX visible height — we use a constant since we
                            // cannot query frame size here; renderer clamps anyway
                            state.result_down(20);
                        }
                    }

                    // ── Autocomplete: Tab cycles ──────────────────
                    KeyCode::Tab => {
                        if state.autocomplete.active {
                            state.autocomplete.tab_next();
                        }
                    }
                    KeyCode::BackTab => {
                        if state.autocomplete.active {
                            state.autocomplete.move_up();
                        }
                    }

                    // ── Enter: confirm autocomplete OR open result ─
                    KeyCode::Enter => {
                        if state.autocomplete.active {
                            if let Some(candidate) = state.autocomplete.current().map(|s| s.to_string()) {
                                state.accept_autocomplete(&candidate);
                                let q = state.query.clone();
                                app.telescope_search(&q);
                                app.update_telescope_suggestions();
                            }
                        } else {
                            app.telescope_open_selected();
                        }
                    }

                    // ── Ctrl-w: delete word back ──────────────────
                    KeyCode::Char('w') if modifiers.contains(KeyModifiers::CONTROL) => {
                        state.delete_word_back();
                        let q = state.query.clone();
                        app.telescope_search(&q);
                        app.update_telescope_suggestions();
                    }

                    // ── Backspace ─────────────────────────────────
                    KeyCode::Backspace => {
                        state.delete_back();
                        let q = state.query.clone();
                        app.telescope_search(&q);
                        app.update_telescope_suggestions();
                    }

                    // ── Cursor movement in input ──────────────────
                    KeyCode::Left => {
                        if modifiers.contains(KeyModifiers::CONTROL) {
                            state.cursor_word_back();
                        } else {
                            state.cursor_left();
                        }
                    }
                    KeyCode::Right => {
                        if modifiers.contains(KeyModifiers::CONTROL) {
                            state.cursor_word_forward();
                        } else {
                            state.cursor_right();
                        }
                    }
                    KeyCode::Home => state.cursor_home(),
                    KeyCode::End => state.cursor_end(),

                    // ── Switch to Normal mode ─────────────────────
                    KeyCode::Char('n') if modifiers.contains(KeyModifiers::CONTROL) => {
                        state.mode = TelescopeMode::Normal;
                        state.autocomplete.clear();
                    }

                    // ── Normal character input ────────────────────
                    KeyCode::Char(c) => {
                        state.insert_char(c);
                        let q = state.query.clone();
                        app.telescope_search(&q);
                        app.update_telescope_suggestions();
                    }

                    _ => {}
                },

                TelescopeMode::Normal => match code {
                    // ── Exit overlay ──────────────────────────────
                    KeyCode::Esc | KeyCode::Char('q') => {
                        app.popup = None;
                    }

                    // ── Enter Insert mode ─────────────────────────
                    KeyCode::Char('i') => {
                        state.mode = TelescopeMode::Insert;
                    }
                    KeyCode::Char('A') => {
                        state.cursor_end();
                        state.mode = TelescopeMode::Insert;
                    }
                    KeyCode::Char('I') => {
                        state.cursor_home();
                        state.mode = TelescopeMode::Insert;
                    }
                    // '/' in normal mode: clear and start new search
                    KeyCode::Char('/') => {
                        state.query.clear();
                        state.cursor = 0;
                        state.mode = TelescopeMode::Insert;
                        app.telescope_search("");
                        app.update_telescope_suggestions();
                    }

                    // ── Enter: open selected ──────────────────────
                    KeyCode::Enter => {
                        app.telescope_open_selected();
                    }

                    // ── Vim navigation ────────────────────────────
                    KeyCode::Char('j') | KeyCode::Down => { state.result_down(20); }
                    KeyCode::Char('k') | KeyCode::Up => { state.result_up(); }
                    KeyCode::Char('G') => { state.result_bottom(); }
                    KeyCode::Char('g') => {
                        if state.pending_g {
                            state.result_top();
                            state.pending_g = false;
                        } else {
                            state.pending_g = true;
                        }
                    }
                    KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
                        state.result_down(20); state.half_page_down(20);
                    }
                    KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
                        state.result_up(); state.half_page_up(20);
                    }

                    _ => { state.pending_g = false; }
                },
            }
        }

        Some(Popup::Help) => {
            // Any key closes help
            app.popup = None;
        }

        None => {}
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
                if let Some(Popup::Telescope(ref mut state)) = app.popup {
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

fn update_filepath_suggestions(form: &mut crate::popup::AddBookForm) {
    let field = &form.fields[5];
    let raw_val = &field.value;
    
    // 1. Expand `~`
    let expanded_val = if raw_val.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
             raw_val.replacen("~", &home.to_string_lossy(), 1)
        } else {
            raw_val.clone()
        }
    } else {
        raw_val.clone()
    };

    // 2. Identify dir + prefix
    // If it ends with /, show contents of that dir.
    // Otherwise, show contents of parent matching prefix.
    let (search_dir, prefix) = if expanded_val.ends_with('/') {
        (std::path::PathBuf::from(&expanded_val), "".to_string())
    } else {
        let p = std::path::Path::new(&expanded_val);
        // If path has no parent (e.g. just "foo"), parent is "."
        let parent = if let Some(p) = p.parent() {
            if p.as_os_str().is_empty() {
                std::path::PathBuf::from(".")
            } else {
                p.to_path_buf()
            }
        } else {
            std::path::PathBuf::from(".")
        };
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        (parent, name.to_string())
    };

    // 3. Read dir
    if let Ok(entries) = std::fs::read_dir(&search_dir) {
        let mut matches: Vec<String> = entries
            .filter_map(|e| e.ok())
            .map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                if e.path().is_dir() {
                    name + "/"
                } else {
                    name
                }
            })
            .filter(|name| name.starts_with(&prefix))
            .collect();

        // Sort for stability
        matches.sort();
        
        form.autocomplete.all_candidates = matches.clone();
        form.autocomplete.visible = matches;
        form.autocomplete.active = !form.autocomplete.visible.is_empty();
        form.autocomplete.selected = if form.autocomplete.active { Some(0) } else { None };
    } else {
        form.autocomplete.clear();
    }
}
