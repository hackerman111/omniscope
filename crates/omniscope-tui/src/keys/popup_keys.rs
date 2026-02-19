use crossterm::event::{KeyCode, KeyModifiers};
use crate::app::App;
use crate::popup::{Popup, TelescopeMode};

pub(crate) fn handle_popup_key(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    match &mut app.popup {
        Some(Popup::AddBook(form)) => {
            // Check if we are editing the "File path" field (index 5)
            let is_path_field = form.active_field == 5;
            
            match code {
                KeyCode::Esc => {
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
                            let field = &mut form.fields[5];
                            let current = &field.value;
                            
                            // Simple logic: expand ~ if needed, resolve parent dir
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
                            
                            // Restore ~ prefix if it was there
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
                            
                            // If resulting path is a directory, append /
                            let check_path = if field.value.starts_with("~/") {
                                 if let Some(home) = dirs::home_dir() {
                                     field.value.replacen("~", &home.to_string_lossy(), 1)
                                 } else {
                                     field.value.clone()
                                 }
                            } else {
                                field.value.clone()
                            };
                            
                            if let Ok(m) = std::fs::metadata(&check_path) {
                                if m.is_dir() && !field.value.ends_with('/') {
                                    field.value.push('/');
                                }
                            }
                            
                            field.cursor = field.value.len();
                            
                            // If directory, keep completing inside it
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
                        form.autocomplete.tab_next();
                    } else if is_path_field {
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
                    // Load and modify card directly — logic could be moved to app struct method 
                    // but keeping it here as in original code for now
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
            match state.mode {
                TelescopeMode::Insert => match code {
                    KeyCode::Esc => {
                        if state.autocomplete.active {
                            state.autocomplete.clear();
                        } else {
                            app.popup = None;
                        }
                    }
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
                            state.result_down(20);
                        }
                    }
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
                    KeyCode::Char('w') if modifiers.contains(KeyModifiers::CONTROL) => {
                        state.delete_word_back();
                        let q = state.query.clone();
                        app.telescope_search(&q);
                        app.update_telescope_suggestions();
                    }
                    KeyCode::Backspace => {
                        state.delete_back();
                        let q = state.query.clone();
                        app.telescope_search(&q);
                        app.update_telescope_suggestions();
                    }
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
                    KeyCode::Char('n') if modifiers.contains(KeyModifiers::CONTROL) => {
                        state.mode = TelescopeMode::Normal;
                        state.autocomplete.clear();
                    }
                    KeyCode::Char(c) => {
                        state.insert_char(c);
                        let q = state.query.clone();
                        app.telescope_search(&q);
                        app.update_telescope_suggestions();
                    }
                    _ => {}
                },

                TelescopeMode::Normal => match code {
                    KeyCode::Esc | KeyCode::Char('q') => {
                        app.popup = None;
                    }
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
                    KeyCode::Char('/') => {
                        state.query.clear();
                        state.cursor = 0;
                        state.mode = TelescopeMode::Insert;
                        app.telescope_search("");
                        app.update_telescope_suggestions();
                    }
                    KeyCode::Enter => {
                        app.telescope_open_selected();
                    }
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
            app.popup = None;
        }

        None => {}
    }
}

fn update_filepath_suggestions(form: &mut crate::popup::AddBookForm) {
    let field = &form.fields[5];
    let raw_val = &field.value;
    
    let expanded_val = if raw_val.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
             raw_val.replacen("~", &home.to_string_lossy(), 1)
        } else {
            raw_val.clone()
        }
    } else {
        raw_val.clone()
    };

    let (search_dir, prefix) = if expanded_val.ends_with('/') {
        (std::path::PathBuf::from(&expanded_val), "".to_string())
    } else {
        let p = std::path::Path::new(&expanded_val);
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
        match matches.len() {
             0 => form.autocomplete.clear(),
             _ => {
                 matches.sort();
                 form.autocomplete.activate(matches);
             }
        }
    } else {
        form.autocomplete.clear();
    }
}
