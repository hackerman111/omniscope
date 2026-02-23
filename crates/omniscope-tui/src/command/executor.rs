use super::{CommandAction, parse_command};
use crate::app::App;
use crate::panels::citation_graph::GraphMode;

pub fn execute_command(app: &mut App, cmd: &str) {
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
                    let search_text = format!(
                        "{} {} {}",
                        book.title,
                        book.authors.join(" "),
                        book.tags.join(" ")
                    );
                    if re.is_match(&search_text) {
                        matched_indices.push(i);
                    }
                }

                if matched_indices.is_empty() {
                    app.status_message = format!("No matches for pattern: {pattern}");
                } else {
                    if command == "d" {
                        app.delete_indices(&matched_indices);
                        app.status_message =
                            format!("Global delete executed on {} items", matched_indices.len());
                    } else if command.starts_with("tag ") {
                        let tag = command.trim_start_matches("tag ").trim();
                        let mut cards = Vec::new();
                        let cards_dir = app.cards_dir();
                        for &idx in &matched_indices {
                            if let Some(view) = app.books.get(idx) {
                                if let Ok(mut card) =
                                    omniscope_core::storage::json_cards::load_card_by_id(
                                        &cards_dir, &view.id,
                                    )
                                {
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
                                omniscope_core::undo::UndoAction::UpsertCards(cards.clone()),
                            );
                            for card in &cards {
                                let _ = omniscope_core::storage::json_cards::save_card(
                                    &cards_dir, card,
                                );
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
        CommandAction::Substitute {
            pattern,
            replacement,
            global,
        } => {
            app.status_message = format!(
                "Substitute `{pattern}` -> `{replacement}` (global: {global}) not yet fully implemented"
            );
        }
        CommandAction::UndoList => {
            app.status_message = format!(
                "Undo history: {} items, Redo: {} items",
                app.undo_stack.len(),
                app.redo_stack.len()
            );
        }
        CommandAction::QuickfixOpen => {
            if app.quickfix_list.is_empty() {
                app.status_message = "Quickfix list is empty.".to_string();
            } else {
                app.quickfix_show = true;
                app.status_message = format!(
                    "Opened quickfix list with {} items",
                    app.quickfix_list.len()
                );
            }
        }
        CommandAction::QuickfixClose => {
            app.quickfix_show = false;
        }
        CommandAction::QuickfixNext => {
            if !app.quickfix_list.is_empty() {
                app.quickfix_selected =
                    (app.quickfix_selected + 1).min(app.quickfix_list.len() - 1);
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
            app.status_message = format!(
                "Executing `{command}` on {} items (WIP)",
                app.quickfix_list.len()
            );
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
            let marks_display: Vec<String> = app
                .marks
                .iter()
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
                        crate::app::RegisterContent::MultipleCards(cards) => {
                            format!("{} cards", cards.len())
                        }
                        crate::app::RegisterContent::Text(t) => t.clone(),
                        crate::app::RegisterContent::Path(p) => p.clone(),
                    };
                    app.status_message = format!("\"{r}: {desc}");
                } else {
                    app.status_message = format!("Register \"{r} is empty");
                }
            } else {
                let regs: Vec<String> = app.registers.keys().map(|k| format!("\"{k}")).collect();
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
                let desc: Vec<String> = list
                    .iter()
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
        CommandAction::Cite(style) => {
            app.show_science_citation(style.as_deref());
        }
        CommandAction::Bibtex => {
            app.show_science_bibtex();
        }
        CommandAction::Refs => {
            app.open_science_references_panel();
        }
        CommandAction::CitedBy => {
            app.open_science_citation_graph_panel(GraphMode::CitedBy);
        }
        CommandAction::Unknown(unknown_cmd) => {
            app.status_message = format!("Unknown command: {unknown_cmd}");
        }
    }
}

fn parse_duration(s: &str) -> chrono::Duration {
    let s = s.trim();
    if s.ends_with('m') {
        let mins: i64 = s[..s.len() - 1].parse().unwrap_or(0);
        chrono::Duration::minutes(mins)
    } else if s.ends_with('h') {
        let hours: i64 = s[..s.len() - 1].parse().unwrap_or(0);
        chrono::Duration::hours(hours)
    } else if s.ends_with('s') {
        let secs: i64 = s[..s.len() - 1].parse().unwrap_or(0);
        chrono::Duration::seconds(secs)
    } else {
        chrono::Duration::zero()
    }
}
