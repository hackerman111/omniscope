use crate::app::App;
use crate::panels::citation_graph::{CitationGraphPanel, CitationGraphPanelAction, GraphMode};
use crate::panels::find_download::{
    FindDownloadPanel, FindDownloadPanelAction, FindResult, FindSource,
};
use crate::panels::references::{ReferenceAddTarget, ReferencesPanelAction};
use crate::popup::{Popup, TelescopeMode};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use omniscope_science::identifiers::extract::{
    extract_arxiv_ids_from_text, extract_dois_from_text,
};
use omniscope_science::identifiers::{arxiv::ArxivId, doi::Doi};

pub(crate) fn handle_popup_key(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    if handle_science_popup_key(app, code, modifiers) {
        return;
    }

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

        Some(_) => {}

        None => {}
    }
}

fn handle_science_popup_key(app: &mut App, code: KeyCode, modifiers: KeyModifiers) -> bool {
    let Some(current_popup) = app.popup.take() else {
        return false;
    };

    match current_popup {
        Popup::ScienceReferences {
            mut panel,
            book_title,
        } => {
            let mut keep_open = true;
            let key = KeyEvent::new(code, modifiers);

            match code {
                KeyCode::Esc | KeyCode::Char('q') if modifiers == KeyModifiers::NONE => {
                    keep_open = false;
                }
                _ => {
                    if let Some(action) = panel.handle_key(key, 16) {
                        match action {
                            ReferencesPanelAction::OpenBook(book_id) => {
                                if app.select_book_by_id(book_id) {
                                    app.status_message = "Opened linked book".to_string();
                                } else {
                                    app.status_message =
                                        format!("Linked book not found: {book_id}");
                                }
                                keep_open = false;
                            }
                            ReferencesPanelAction::ShowDetails { reference_index } => {
                                if let Some(reference) = panel.references.get(reference_index) {
                                    let details = format_reference_details(reference);
                                    app.open_science_text_viewer(
                                        format!(" Reference #{} ", reference.index),
                                        details,
                                    );
                                    keep_open = false;
                                }
                            }
                            ReferencesPanelAction::AddReference {
                                reference_index,
                                target,
                            } => {
                                if let Some(reference) = panel.references.get(reference_index) {
                                    let title = reference
                                        .resolved_title
                                        .as_deref()
                                        .unwrap_or(reference.raw_text.as_str())
                                        .to_string();
                                    let authors = reference.resolved_authors.clone();
                                    let year = reference.resolved_year;
                                    let doi = match target.as_ref() {
                                        Some(ReferenceAddTarget::Doi(doi)) => Some(doi.as_str()),
                                        _ => reference
                                            .doi
                                            .as_ref()
                                            .map(|value| value.normalized.as_str()),
                                    };
                                    let arxiv_value = match target.as_ref() {
                                        Some(ReferenceAddTarget::Arxiv(arxiv)) => {
                                            Some(arxiv.to_string())
                                        }
                                        _ => reference.arxiv_id.as_ref().map(|value| {
                                            if let Some(version) = value.version {
                                                format!("{}v{version}", value.id)
                                            } else {
                                                value.id.clone()
                                            }
                                        }),
                                    };

                                    if let Some(book_id) = app.add_science_entry_to_library(
                                        &title,
                                        &authors,
                                        year,
                                        doi,
                                        arxiv_value.as_deref(),
                                        "Add reference",
                                    ) {
                                        if let Some(reference) =
                                            panel.references.get_mut(reference_index)
                                        {
                                            reference.is_in_library = Some(book_id);
                                        }
                                    }
                                } else {
                                    app.status_message = "Selected reference not found".to_string();
                                }
                            }
                            ReferencesPanelAction::FindOnline { reference_index } => {
                                let query = panel
                                    .references
                                    .get(reference_index)
                                    .map(reference_query)
                                    .unwrap_or_default();
                                if query.trim().is_empty() {
                                    app.status_message =
                                        "Selected reference has no searchable text".to_string();
                                } else {
                                    app.open_science_find_download_panel(Some(query));
                                    keep_open = false;
                                }
                            }
                            ReferencesPanelAction::AddAllUnresolved { reference_indices } => {
                                app.status_message = format!(
                                    "Add unresolved references: {} item(s)",
                                    reference_indices.len()
                                );
                            }
                            ReferencesPanelAction::Export { reference_indices } => {
                                app.status_message = format!(
                                    "Export references: {} item(s)",
                                    reference_indices.len()
                                );
                            }
                            ReferencesPanelAction::StartSearch => {
                                app.status_message =
                                    "Inline search in references is not implemented yet"
                                        .to_string();
                            }
                        }
                    }
                }
            }

            if keep_open && app.popup.is_none() {
                app.popup = Some(Popup::ScienceReferences { panel, book_title });
            }
            true
        }
        Popup::ScienceCitationGraph(mut panel) => {
            let mut keep_open = true;
            let key = KeyEvent::new(code, modifiers);

            match code {
                KeyCode::Esc | KeyCode::Char('q') if modifiers == KeyModifiers::NONE => {
                    keep_open = false;
                }
                _ => {
                    if let Some(action) = panel.handle_key(key) {
                        match action {
                            CitationGraphPanelAction::OpenBook(book_id) => {
                                if app.select_book_by_id(book_id) {
                                    app.status_message = "Opened linked citation".to_string();
                                } else {
                                    app.status_message =
                                        format!("Linked citation not found: {book_id}");
                                }
                                keep_open = false;
                            }
                            CitationGraphPanelAction::AddToLibrary {
                                mode,
                                edge_index,
                                target,
                            } => {
                                if let Some(edge) = citation_edge(&panel, mode, edge_index) {
                                    let title = edge.title.clone();
                                    let authors = edge.authors.clone();
                                    let year = edge.year;
                                    let doi = match target.as_ref() {
                                        Some(
                                            crate::panels::citation_graph::CitationAddTarget::Doi(
                                                doi,
                                            ),
                                        ) => Some(doi.as_str()),
                                        _ => {
                                            edge.doi.as_ref().map(|value| value.normalized.as_str())
                                        }
                                    };
                                    let arxiv_value = match target.as_ref() {
                                        Some(
                                            crate::panels::citation_graph::CitationAddTarget::Arxiv(
                                                arxiv,
                                            ),
                                        ) => Some(arxiv.to_string()),
                                        _ => edge.arxiv_id.as_ref().map(|value| {
                                            if let Some(version) = value.version {
                                                format!("{}v{version}", value.id)
                                            } else {
                                                value.id.clone()
                                            }
                                        }),
                                    };

                                    if let Some(book_id) = app.add_science_entry_to_library(
                                        &title,
                                        &authors,
                                        year,
                                        doi,
                                        arxiv_value.as_deref(),
                                        &format!("Add citation ({})", mode.label()),
                                    ) {
                                        if let Some(edge) =
                                            citation_edge_mut(&mut panel, mode, edge_index)
                                        {
                                            edge.source_id = Some(book_id);
                                        }
                                    }
                                } else {
                                    app.status_message = "Selected citation not found".to_string();
                                }
                            }
                            CitationGraphPanelAction::FindOnline { mode, edge_index } => {
                                if let Some(query) = citation_query(&panel, mode, edge_index) {
                                    app.open_science_find_download_panel(Some(query));
                                    keep_open = false;
                                } else {
                                    app.status_message =
                                        "No DOI/arXiv in selected citation".to_string();
                                }
                            }
                        }
                    }
                }
            }

            if keep_open && app.popup.is_none() {
                app.popup = Some(Popup::ScienceCitationGraph(panel));
            }
            true
        }
        Popup::ScienceFindDownload(mut panel) => {
            let mut keep_open = true;
            let key = KeyEvent::new(code, modifiers);

            if let Some(action) = panel.handle_key(key) {
                match action {
                    FindDownloadPanelAction::Download {
                        source,
                        result_index,
                    } => {
                        if let Some(url) = download_url_for_result(&panel, source, result_index) {
                            app.open_external_url(&url, "download");
                        } else {
                            app.status_message = format!(
                                "No download URL for {} #{}",
                                source_name(source),
                                result_index + 1
                            );
                        }
                    }
                    FindDownloadPanelAction::ImportMetadata {
                        source,
                        result_index,
                    } => {
                        app.import_science_metadata_from_find_result(&panel, source, result_index);
                    }
                    FindDownloadPanelAction::OpenInBrowser {
                        source,
                        result_index,
                    } => {
                        if let Some(url) = find_result(&panel, source, result_index)
                            .and_then(|item| item.open_url.as_deref())
                        {
                            app.open_external_url(url, "result");
                        } else {
                            app.status_message =
                                format!("No URL for {} #{}", source_name(source), result_index + 1);
                        }
                    }
                    FindDownloadPanelAction::Close => {
                        keep_open = false;
                    }
                }
            }

            if keep_open && app.popup.is_none() {
                app.popup = Some(Popup::ScienceFindDownload(panel));
            }
            true
        }
        Popup::EditDoi {
            book_id,
            mut input,
            mut cursor,
        } => {
            let mut keep_open = true;
            match code {
                KeyCode::Esc => keep_open = false,
                KeyCode::Enter => {
                    app.submit_edit_science_doi(&book_id, &input);
                    keep_open = false;
                }
                KeyCode::Backspace => {
                    delete_prev_char(&mut input, &mut cursor);
                }
                KeyCode::Left => {
                    cursor = prev_char_boundary(&input, cursor);
                }
                KeyCode::Right => {
                    cursor = next_char_boundary(&input, cursor);
                }
                KeyCode::Char(c) => {
                    if !c.is_control() {
                        input.insert(cursor, c);
                        cursor += c.len_utf8();
                    }
                }
                _ => {}
            }

            if keep_open && app.popup.is_none() {
                app.popup = Some(Popup::EditDoi {
                    book_id,
                    input,
                    cursor,
                });
            }
            true
        }
        Popup::EditArxivId {
            book_id,
            mut input,
            mut cursor,
        } => {
            let mut keep_open = true;
            match code {
                KeyCode::Esc => keep_open = false,
                KeyCode::Enter => {
                    app.submit_edit_science_arxiv_id(&book_id, &input);
                    keep_open = false;
                }
                KeyCode::Backspace => {
                    delete_prev_char(&mut input, &mut cursor);
                }
                KeyCode::Left => {
                    cursor = prev_char_boundary(&input, cursor);
                }
                KeyCode::Right => {
                    cursor = next_char_boundary(&input, cursor);
                }
                KeyCode::Char(c) => {
                    if is_valid_arxiv_char(c) {
                        input.insert(cursor, c);
                        cursor += c.len_utf8();
                    }
                }
                _ => {}
            }

            if keep_open && app.popup.is_none() {
                app.popup = Some(Popup::EditArxivId {
                    book_id,
                    input,
                    cursor,
                });
            }
            true
        }
        Popup::TextViewer {
            title,
            body,
            mut scroll,
        } => {
            let mut keep_open = true;
            let line_count = body.lines().count().max(1);

            match code {
                KeyCode::Esc | KeyCode::Char('q') if modifiers == KeyModifiers::NONE => {
                    keep_open = false;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    scroll = scroll.saturating_add(1).min(line_count.saturating_sub(1));
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    scroll = scroll.saturating_sub(1);
                }
                KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
                    scroll = scroll.saturating_add(10).min(line_count.saturating_sub(1));
                }
                KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
                    scroll = scroll.saturating_sub(10);
                }
                KeyCode::Char('g') => {
                    scroll = 0;
                }
                KeyCode::Char('G') => {
                    scroll = line_count.saturating_sub(1);
                }
                _ => {}
            }

            if keep_open && app.popup.is_none() {
                app.popup = Some(Popup::TextViewer {
                    title,
                    body,
                    scroll,
                });
            }
            true
        }
        popup => {
            app.popup = Some(popup);
            false
        }
    }
}

fn source_name(source: FindSource) -> &'static str {
    match source {
        FindSource::AnnaArchive => "Anna's Archive",
        FindSource::SciHub => "Sci-Hub",
        FindSource::OpenAlex => "OpenAlex",
        FindSource::SemanticGraph => "Semantic Scholar",
    }
}

fn find_result(
    panel: &FindDownloadPanel,
    source: FindSource,
    result_index: usize,
) -> Option<&FindResult> {
    match source {
        FindSource::AnnaArchive => panel.anna_results.get(result_index),
        FindSource::SciHub => panel.sci_hub_results.get(result_index),
        FindSource::OpenAlex => panel.open_alex_results.get(result_index),
        FindSource::SemanticGraph => panel.semantic_scholar_results.get(result_index),
    }
}

fn citation_edge(
    panel: &CitationGraphPanel,
    mode: GraphMode,
    edge_index: usize,
) -> Option<&crate::panels::citation_graph::CitationEdge> {
    match mode {
        GraphMode::References => panel.references.get(edge_index),
        GraphMode::CitedBy => panel.cited_by.get(edge_index),
        GraphMode::Related => panel.related.get(edge_index),
    }
}

fn citation_edge_mut(
    panel: &mut CitationGraphPanel,
    mode: GraphMode,
    edge_index: usize,
) -> Option<&mut crate::panels::citation_graph::CitationEdge> {
    match mode {
        GraphMode::References => panel.references.get_mut(edge_index),
        GraphMode::CitedBy => panel.cited_by.get_mut(edge_index),
        GraphMode::Related => panel.related.get_mut(edge_index),
    }
}

fn download_url_for_result(
    panel: &FindDownloadPanel,
    source: FindSource,
    result_index: usize,
) -> Option<String> {
    let result = find_result(panel, source, result_index)?;
    if let Some(url) = result
        .open_url
        .as_ref()
        .map(|value| value.trim().to_string())
        && !url.is_empty()
    {
        return Some(url);
    }

    if let Some(primary_id) = result.primary_id.as_deref() {
        if let Some(doi) = parse_doi_from_any(primary_id) {
            return Some(doi.url);
        }
        if let Some(arxiv_id) = parse_arxiv_from_any(primary_id) {
            return Some(arxiv_id.pdf_url);
        }
    }

    if let Some(arxiv_id) = parse_arxiv_from_any(&result.title) {
        return Some(arxiv_id.pdf_url);
    }
    if let Some(doi) = parse_doi_from_any(&result.title) {
        return Some(doi.url);
    }

    let query = panel.query.trim();
    if query.is_empty() {
        return None;
    }
    if let Some(doi) = parse_doi_from_any(query) {
        return Some(doi.url);
    }
    if let Some(arxiv_id) = parse_arxiv_from_any(query) {
        return Some(arxiv_id.pdf_url);
    }

    let encoded_query = query.split_whitespace().collect::<Vec<_>>().join("+");
    Some(format!("https://duckduckgo.com/?q={encoded_query}"))
}

fn citation_query(
    panel: &CitationGraphPanel,
    mode: GraphMode,
    edge_index: usize,
) -> Option<String> {
    let edge = match mode {
        GraphMode::References => panel.references.get(edge_index),
        GraphMode::CitedBy => panel.cited_by.get(edge_index),
        GraphMode::Related => panel.related.get(edge_index),
    }?;

    if let Some(doi) = &edge.doi {
        return Some(doi.normalized.clone());
    }
    if let Some(arxiv_id) = &edge.arxiv_id {
        return Some(if let Some(version) = arxiv_id.version {
            format!("{}v{version}", arxiv_id.id)
        } else {
            arxiv_id.id.clone()
        });
    }
    if !edge.title.trim().is_empty() {
        return Some(edge.title.trim().to_string());
    }
    None
}

fn reference_query(reference: &omniscope_science::references::ExtractedReference) -> String {
    if let Some(doi) = &reference.doi {
        return doi.normalized.clone();
    }
    if let Some(arxiv_id) = &reference.arxiv_id {
        return if let Some(version) = arxiv_id.version {
            format!("{}v{version}", arxiv_id.id)
        } else {
            arxiv_id.id.clone()
        };
    }
    reference.raw_text.clone()
}

fn format_reference_details(
    reference: &omniscope_science::references::ExtractedReference,
) -> String {
    let mut lines = vec![
        format!("index: {}", reference.index),
        format!(
            "resolution: {:?} (confidence {:.2})",
            reference.resolution_method, reference.confidence
        ),
    ];

    if let Some(doi) = &reference.doi {
        lines.push(format!("doi: {}", doi.normalized));
    }
    if let Some(arxiv_id) = &reference.arxiv_id {
        lines.push(format!(
            "arxiv: {}",
            if let Some(version) = arxiv_id.version {
                format!("{}v{version}", arxiv_id.id)
            } else {
                arxiv_id.id.clone()
            }
        ));
    }
    if let Some(isbn) = &reference.isbn {
        lines.push(format!("isbn13: {}", isbn.isbn13));
    }
    if let Some(title) = &reference.resolved_title {
        lines.push(format!("title: {title}"));
    }
    if !reference.resolved_authors.is_empty() {
        lines.push(format!(
            "authors: {}",
            reference.resolved_authors.join(", ")
        ));
    }
    if let Some(year) = reference.resolved_year {
        lines.push(format!("year: {year}"));
    }
    if let Some(book_id) = reference.is_in_library {
        lines.push(format!("in_library: {book_id}"));
    }

    lines.push(String::new());
    lines.push("raw:".to_string());
    lines.push(reference.raw_text.clone());
    lines.join("\n")
}

fn prev_char_boundary(input: &str, cursor: usize) -> usize {
    if cursor == 0 {
        return 0;
    }
    input[..cursor]
        .char_indices()
        .last()
        .map(|(idx, _)| idx)
        .unwrap_or(0)
}

fn next_char_boundary(input: &str, cursor: usize) -> usize {
    if cursor >= input.len() {
        return input.len();
    }
    input[cursor..]
        .char_indices()
        .nth(1)
        .map(|(offset, _)| cursor + offset)
        .unwrap_or(input.len())
}

fn delete_prev_char(input: &mut String, cursor: &mut usize) {
    if *cursor == 0 {
        return;
    }
    let prev = prev_char_boundary(input, *cursor);
    input.remove(prev);
    *cursor = prev;
}

fn is_valid_arxiv_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '/' | '-' | ':' | 'v')
}

fn parse_doi_from_any(value: &str) -> Option<Doi> {
    Doi::parse(value)
        .ok()
        .or_else(|| extract_dois_from_text(value).into_iter().next())
}

fn parse_arxiv_from_any(value: &str) -> Option<ArxivId> {
    ArxivId::parse(value)
        .ok()
        .or_else(|| extract_arxiv_ids_from_text(value).into_iter().next())
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
                if e.path().is_dir() { name + "/" } else { name }
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
