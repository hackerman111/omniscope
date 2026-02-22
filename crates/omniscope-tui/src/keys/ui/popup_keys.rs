use crate::app::App;
use crate::popup::{Popup, TelescopeMode};
use crossterm::event::{KeyCode, KeyModifiers};

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
                        if let Some(selection) = form.autocomplete.current().map(|s| s.to_string())
                        {
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
                                path.parent()
                                    .unwrap_or(std::path::Path::new("."))
                                    .to_path_buf()
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
                    let cards_dir = app.cards_dir();
                    // Load and modify card directly — logic could be moved to app struct method
                    // but keeping it here as in original code for now
                    if let Ok(mut card) =
                        omniscope_core::storage::json_cards::load_card_by_id(&cards_dir, &id)
                    {
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
            KeyCode::Esc => {
                app.popup = None;
            }
            KeyCode::Char(' ') => {
                // Cycle through statuses
                app.popup = None;
                app.cycle_status();
            }
            KeyCode::Char('1') => {
                app.popup = None;
                app.set_status(omniscope_core::ReadStatus::Unread);
            }
            KeyCode::Char('2') => {
                app.popup = None;
                app.set_status(omniscope_core::ReadStatus::Reading);
            }
            KeyCode::Char('3') => {
                app.popup = None;
                app.set_status(omniscope_core::ReadStatus::Read);
            }
            KeyCode::Char('4') => {
                app.popup = None;
                app.set_status(omniscope_core::ReadStatus::Dnf);
            }
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
                            if let Some(candidate) =
                                state.autocomplete.current().map(|s| s.to_string())
                            {
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
                    KeyCode::Char('q') if modifiers.contains(KeyModifiers::CONTROL) => {
                        app.quickfix_list = state.results.clone();
                        app.quickfix_show = true;
                        app.popup = None; // close telescope
                        app.quickfix_selected = 0;
                        app.status_message =
                            format!("Sent {} items to quickfix", app.quickfix_list.len());
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
                    KeyCode::Esc => {
                        app.popup = None;
                    }
                    KeyCode::Char('q') if modifiers.is_empty() => {
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
                    KeyCode::Char('j') | KeyCode::Down => {
                        state.result_down(20);
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        state.result_up();
                    }
                    KeyCode::Char('G') => {
                        state.result_bottom();
                    }
                    KeyCode::Char('g') => {
                        if state.pending_g {
                            state.result_top();
                            state.pending_g = false;
                        } else {
                            state.pending_g = true;
                        }
                    }
                    KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
                        state.result_down(20);
                        state.half_page_down(20);
                    }
                    KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
                        state.result_up();
                        state.half_page_up(20);
                    }
                    KeyCode::Char('q') if modifiers.contains(KeyModifiers::CONTROL) => {
                        app.quickfix_list = state.results.clone();
                        app.quickfix_show = true;
                        app.popup = None;
                        app.quickfix_selected = 0;
                        app.status_message =
                            format!("Sent {} items to quickfix", app.quickfix_list.len());
                    }
                    _ => {
                        state.pending_g = false;
                    }
                },
            }
        }

        Some(Popup::Help) => {
            app.popup = None;
        }

        Some(Popup::EasyMotion(state)) => {
            let is_pending = state.pending;
            let state_clone = state.clone(); // Clone to avoid borrow checker issues
            match code {
                KeyCode::Esc => {
                    app.popup = None;
                    app.status_message = "EasyMotion cancelled".to_string();
                }
                KeyCode::Char(c) => {
                    if is_pending {
                        // Space / <char> — filter by first letter, build targets
                        let targets =
                            crate::keys::ext::easy_motion::build_easy_motion_targets_by_char(
                                app, c,
                            );
                        if targets.is_empty() {
                            app.popup = None;
                            app.status_message = format!("No books starting with '{c}'");
                        } else {
                            app.popup = Some(Popup::EasyMotion(crate::popup::EasyMotionState {
                                pending: false,
                                targets,
                            }));
                            app.status_message = format!("EasyMotion ('{c}'): type target label");
                        }
                    } else if let Some(&(_, idx)) =
                        state_clone.targets.iter().find(|&&(tc, _)| tc == c)
                    {
                        app.selected_index = idx;
                        app.popup = None;
                        app.status_message = "EasyMotion jump success".to_string();
                    } else {
                        app.popup = None;
                        app.status_message = "EasyMotion target not found".to_string();
                    }
                }
                _ => {}
            }
        }

        Some(Popup::Marks) | Some(Popup::Registers) => match code {
            KeyCode::Esc | KeyCode::Char('q') => {
                app.popup = None;
            }
            _ => {}
        },

        Some(Popup::EditYear { .. }) => match code {
            KeyCode::Esc => {
                app.popup = None;
            }
            KeyCode::Enter => {
                if let Some(Popup::EditYear { book_id, input, .. }) = app.popup.take() {
                    app.submit_edit_year(&book_id, &input);
                }
            }
            KeyCode::Backspace => {
                if let Some(Popup::EditYear {
                    ref mut input,
                    ref mut cursor,
                    ..
                }) = app.popup
                {
                    if *cursor > 0 {
                        input.remove(*cursor - 1);
                        *cursor -= 1;
                    }
                }
            }
            KeyCode::Char(c) if c.is_ascii_digit() || c == '-' => {
                if let Some(Popup::EditYear {
                    ref mut input,
                    ref mut cursor,
                    ..
                }) = app.popup
                {
                    input.insert(*cursor, c);
                    *cursor += 1;
                }
            }
            _ => {}
        },

        Some(Popup::EditAuthors { .. }) => match code {
            KeyCode::Esc => {
                app.popup = None;
            }
            KeyCode::Enter => {
                if let Some(Popup::EditAuthors { book_id, input, .. }) = app.popup.take() {
                    app.submit_edit_authors(&book_id, &input);
                }
            }
            KeyCode::Backspace => {
                if let Some(Popup::EditAuthors {
                    ref mut input,
                    ref mut cursor,
                    ..
                }) = app.popup
                {
                    if *cursor > 0 {
                        let prev = input[..*cursor]
                            .char_indices()
                            .last()
                            .map(|(i, _)| i)
                            .unwrap_or(0);
                        input.remove(prev);
                        *cursor = prev;
                    }
                }
            }
            KeyCode::Left => {
                if let Some(Popup::EditAuthors {
                    ref mut cursor,
                    ref input,
                    ..
                }) = app.popup
                {
                    if *cursor > 0 {
                        *cursor = input[..*cursor]
                            .char_indices()
                            .last()
                            .map(|(i, _)| i)
                            .unwrap_or(0);
                    }
                }
            }
            KeyCode::Right => {
                if let Some(Popup::EditAuthors {
                    ref mut cursor,
                    ref input,
                    ..
                }) = app.popup
                {
                    if *cursor < input.len() {
                        *cursor = input[*cursor..]
                            .char_indices()
                            .nth(1)
                            .map(|(i, _)| *cursor + i)
                            .unwrap_or(input.len());
                    }
                }
            }
            KeyCode::Char(c) => {
                if let Some(Popup::EditAuthors {
                    ref mut input,
                    ref mut cursor,
                    ..
                }) = app.popup
                {
                    input.insert(*cursor, c);
                    *cursor += c.len_utf8();
                }
            }
            _ => {}
        },

        Some(Popup::AddTagPrompt { .. }) => match code {
            KeyCode::Esc => {
                app.popup = None;
            }
            KeyCode::Enter => {
                if let Some(Popup::AddTagPrompt { indices, input, .. }) = app.popup.take() {
                    if !input.trim().is_empty() {
                        app.add_tag_to_indices(&indices, &input);
                    }
                }
            }
            KeyCode::Backspace => {
                if let Some(Popup::AddTagPrompt {
                    ref mut input,
                    ref mut cursor,
                    ..
                }) = app.popup
                {
                    if *cursor > 0 {
                        let prev = input[..*cursor]
                            .char_indices()
                            .last()
                            .map(|(i, _)| i)
                            .unwrap_or(0);
                        input.remove(prev);
                        *cursor = prev;
                    }
                }
            }
            KeyCode::Char(c) => {
                if let Some(Popup::AddTagPrompt {
                    ref mut input,
                    ref mut cursor,
                    ..
                }) = app.popup
                {
                    input.insert(*cursor, c);
                    *cursor += c.len_utf8();
                }
            }
            _ => {}
        },

        Some(Popup::RemoveTagPrompt { .. }) => match code {
            KeyCode::Esc => {
                app.popup = None;
            }
            KeyCode::Enter => {
                if let Some(Popup::RemoveTagPrompt {
                    indices,
                    available_tags,
                    selected,
                    ..
                }) = app.popup.take()
                {
                    if let Some(tag) = available_tags.get(selected) {
                        let tag = tag.clone();
                        app.remove_tag_from_indices(&indices, &tag);
                    }
                }
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(Popup::RemoveTagPrompt {
                    ref mut selected,
                    ref available_tags,
                    ..
                }) = app.popup
                {
                    if *selected + 1 < available_tags.len() {
                        *selected += 1;
                    }
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(Popup::RemoveTagPrompt {
                    ref mut selected, ..
                }) = app.popup
                {
                    if *selected > 0 {
                        *selected -= 1;
                    }
                }
            }
            _ => {}
        },

        Some(Popup::CreateFolder { .. }) => match code {
            KeyCode::Esc => {
                app.popup = None;
            }
            KeyCode::Enter => {
                if let Some(Popup::CreateFolder { parent_id, input, .. }) = app.popup.take() {
                    app.submit_create_folder(parent_id, &input);
                }
            }
            KeyCode::Backspace => {
                if let Some(Popup::CreateFolder { ref mut input, ref mut cursor, .. }) = app.popup {
                    if *cursor > 0 {
                        input.remove(*cursor - 1);
                        *cursor -= 1;
                    }
                }
            }
            KeyCode::Char(c) => {
                if let Some(Popup::CreateFolder { ref mut input, ref mut cursor, .. }) = app.popup {
                    input.insert(*cursor, c);
                    *cursor += c.len_utf8();
                }
            }
            _ => {}
        },

        Some(Popup::RenameFolder { .. }) => match code {
            KeyCode::Esc => {
                app.popup = None;
            }
            KeyCode::Enter => {
                if let Some(Popup::RenameFolder { folder_id, input, .. }) = app.popup.take() {
                    app.submit_rename_folder(&folder_id, &input);
                }
            }
            KeyCode::Backspace => {
                if let Some(Popup::RenameFolder { ref mut input, ref mut cursor, .. }) = app.popup {
                    if *cursor > 0 {
                        input.remove(*cursor - 1);
                        *cursor -= 1;
                    }
                }
            }
            KeyCode::Char(c) => {
                if let Some(Popup::RenameFolder { ref mut input, ref mut cursor, .. }) = app.popup {
                    input.insert(*cursor, c);
                    *cursor += c.len_utf8();
                }
            }
            _ => {}
        },

        Some(Popup::ConfirmDeleteFolder { .. }) => match code {
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                app.popup = None;
            }
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Some(Popup::ConfirmDeleteFolder { folder_id, keep_files, .. }) = app.popup.take() {
                    app.submit_delete_folder(&folder_id, keep_files);
                }
            }
            KeyCode::Tab => {
                if let Some(Popup::ConfirmDeleteFolder { ref mut keep_files, .. }) = app.popup {
                    *keep_files = !*keep_files;
                }
            }
            _ => {}
        },

        Some(Popup::BulkDeleteFolders { .. }) => match code {
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                app.popup = None;
            }
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Some(Popup::BulkDeleteFolders { folder_ids, keep_files }) = app.popup.take() {
                    app.submit_bulk_delete_folders(&folder_ids, keep_files);
                }
            }
            KeyCode::Tab => {
                if let Some(Popup::BulkDeleteFolders { ref mut keep_files, .. }) = app.popup {
                    *keep_files = !*keep_files;
                }
            }
            _ => {}
        },

        Some(Popup::AttachGhostFile { .. }) => match code {
            KeyCode::Esc => {
                if let Some(Popup::AttachGhostFile { ref mut autocomplete, .. }) = app.popup {
                    if autocomplete.active {
                        autocomplete.clear();
                    } else {
                        app.popup = None;
                    }
                }
            }
            KeyCode::Enter => {
                let mut should_submit = false;
                let mut attach_id = String::new();
                let mut attach_path = String::new();

                if let Some(Popup::AttachGhostFile { ref mut autocomplete, ref mut input, ref book_id, .. }) = app.popup {
                    if autocomplete.active {
                        if let Some(selection) = autocomplete.current().map(|s| s.to_string()) {
                            let current = input.clone();
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

                            *input = final_val;

                            let check_path = if input.starts_with("~/") {
                                if let Some(home) = dirs::home_dir() {
                                    input.replacen("~", &home.to_string_lossy(), 1)
                                } else {
                                    input.clone()
                                }
                            } else {
                                input.clone()
                            };

                            if let Ok(m) = std::fs::metadata(&check_path) {
                                if m.is_dir() && !input.ends_with('/') {
                                    input.push('/');
                                }
                            }
                            autocomplete.clear();
                        } else {
                            autocomplete.clear();
                        }
                        
                        if input.ends_with('/') {
                            update_ghost_filepath_suggestions(input, autocomplete);
                        }
                        
                    } else {
                        should_submit = true;
                        attach_id = book_id.clone();
                        attach_path = input.clone();
                    }
                }

                if should_submit {
                    app.popup = None;
                    if !attach_path.is_empty() {
                        if let Some(ref db) = app.db {
                            if let Ok(uuid) = uuid::Uuid::parse_str(&attach_id) {
                                if let Ok(mut card) = omniscope_core::storage::json_cards::load_card_by_id(&app.cards_dir(), &uuid) {
                                    card.file = Some(omniscope_core::models::book::BookFile {
                                        path: attach_path.clone(),
                                        format: omniscope_core::models::book::FileFormat::Pdf, // Default assumption
                                        size_bytes: 0,
                                        hash_sha256: None,
                                        added_at: chrono::Utc::now(),
                                    });
                                    let _ = omniscope_core::storage::json_cards::save_card(&app.cards_dir(), &card);
                                    let _ = db.upsert_book(&card);
                                    app.refresh_books();
                                    app.status_message = format!("Attached {} to {}", attach_path, attach_id);
                                }
                            }
                        }
                    }
                }
            }
            KeyCode::Tab => {
                if let Some(Popup::AttachGhostFile { ref mut autocomplete, .. }) = app.popup {
                    if autocomplete.active {
                        autocomplete.move_down();
                    }
                }
            }
            KeyCode::BackTab => {
                if let Some(Popup::AttachGhostFile { ref mut autocomplete, .. }) = app.popup {
                    if autocomplete.active {
                        autocomplete.move_up();
                    }
                }
            }
            KeyCode::Backspace => {
                if let Some(Popup::AttachGhostFile { ref mut input, ref mut cursor, ref mut autocomplete, .. }) = app.popup {
                    if *cursor > 0 {
                        input.remove(*cursor - 1);
                        *cursor -= 1;
                        if input.is_empty() {
                            autocomplete.clear();
                        } else {
                            update_ghost_filepath_suggestions(input, autocomplete);
                        }
                    }
                }
            }
            KeyCode::Char(c) => {
                if let Some(Popup::AttachGhostFile { ref mut input, ref mut cursor, ref mut autocomplete, .. }) = app.popup {
                    input.insert(*cursor, c);
                    *cursor += c.len_utf8();
                    update_ghost_filepath_suggestions(input, autocomplete);
                }
            }
            _ => {}
        },

        Some(Popup::FindGhostFilePlaceholder { .. }) => {
            app.popup = None;
        },

        Some(Popup::CreateVirtualFolder { .. }) => match code {
            KeyCode::Esc => {
                app.popup = None;
            }
            KeyCode::Enter => {
                if let Some(Popup::CreateVirtualFolder { input, .. }) = app.popup.take() {
                    app.submit_create_virtual_folder(&input);
                }
            }
            KeyCode::Backspace => {
                if let Some(Popup::CreateVirtualFolder { ref mut input, ref mut cursor, .. }) = app.popup {
                    if *cursor > 0 {
                        input.remove(*cursor - 1);
                        *cursor -= 1;
                    }
                }
            }
            KeyCode::Char(c) => {
                if let Some(Popup::CreateVirtualFolder { ref mut input, ref mut cursor, .. }) = app.popup {
                    input.insert(*cursor, c);
                    *cursor += c.len_utf8();
                }
            }
            _ => {}
        },

        Some(Popup::AddToVirtualFolder { .. }) => match code {
            KeyCode::Esc => {
                app.popup = None;
            }
            KeyCode::Enter => {
                if let Some(Popup::AddToVirtualFolder { book_idx, selected_folder_idx, folders }) = app.popup.take() {
                    if let Some(folder) = folders.get(selected_folder_idx) {
                        app.submit_add_to_virtual_folder(book_idx, &folder.id);
                    }
                }
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(Popup::AddToVirtualFolder { ref mut selected_folder_idx, ref folders, .. }) = app.popup {
                    if !folders.is_empty() && *selected_folder_idx + 1 < folders.len() {
                        *selected_folder_idx += 1;
                    }
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(Popup::AddToVirtualFolder { ref mut selected_folder_idx, .. }) = app.popup {
                    if *selected_folder_idx > 0 {
                        *selected_folder_idx -= 1;
                    }
                }
            }
            _ => {}
        },

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

fn update_ghost_filepath_suggestions(input: &str, autocomplete: &mut crate::popup::AutocompleteState) {
    let expanded_val = if input.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            input.replacen("~", &home.to_string_lossy(), 1)
        } else {
            input.to_string()
        }
    } else {
        input.to_string()
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
            0 => autocomplete.clear(),
            _ => {
                matches.sort();
                autocomplete.activate(matches);
            }
        }
    } else {
        autocomplete.clear();
    }
}
