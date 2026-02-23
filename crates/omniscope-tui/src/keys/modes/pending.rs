use crate::app::{App, Mode};
use crate::keys::core::motions;
use crate::keys::core::operator::execute_operator;
use crate::keys::core::operator::Operator;
use crate::keys::core::text_objects::{self, TextObjectKind};
use crate::keys::ext::find_char;
use crate::keys::ext::science_bindings;
use crossterm::event::{KeyCode, KeyModifiers};

pub(crate) fn handle_pending_mode(app: &mut App, code: KeyCode, _modifiers: KeyModifiers) {
    let operator = match app.pending_operator {
        Some(op) => op,
        None => {
            app.mode = Mode::Normal;
            return;
        }
    };

    if let KeyCode::Char(c) = code {
        if operator == Operator::Yank && science_bindings::handle_yank_science_command(app, c) {
            app.reset_vim_count();
            app.mode = Mode::Normal;
            return;
        }

        // 1. Handle Double Operator (e.g. `dd`, `yy`) -> Linewise on current line
        let op_char = match operator {
            Operator::Delete => 'd',
            Operator::Yank => 'y',
            Operator::Change => 'c',
            Operator::AddTag => '>',
            Operator::RemoveTag => '<',
            _ => '\0',
        };

        if c == op_char {
            let count = app.count_or_one();
            let start = app.selected_index;
            let end = (start + count - 1).min(app.books.len().saturating_sub(1));
            let range: Vec<usize> = (start..=end).collect();

            execute_operator(app, operator, range);
            app.reset_vim_count();
            app.mode = Mode::Normal;
            return;
        }

        // 1.5 Handle Quick Edits for Change ('c')
        if operator == Operator::Change {
            if science_bindings::handle_change_science_command(app, c) {
                app.reset_vim_count();
                app.mode = Mode::Normal;
                return;
            }

            match c {
                'a' => {
                    // change authors
                    app.reset_vim_count();
                    app.mode = Mode::Normal;
                    if let Some(book) = app.selected_book() {
                        let id = book.id.to_string();
                        let current = book.authors.join(", ");
                        app.popup = Some(crate::popup::Popup::EditAuthors {
                            book_id: id,
                            input: current.clone(),
                            cursor: current.len(),
                        });
                    }
                    app.status_message = "Quick edit: Authors".to_string();
                    return;
                }
                't' | 'T' => {
                    // change tags / title
                    app.reset_vim_count();
                    app.mode = Mode::Normal;
                    app.open_edit_tags();
                    app.status_message = "Quick edit: Tags".to_string();
                    return;
                }
                'r' | 'R' => {
                    // change rating
                    app.reset_vim_count();
                    app.mode = Mode::Normal;
                    app.popup = Some(crate::popup::Popup::SetRating {
                        id: app
                            .selected_book()
                            .map(|b| b.id.to_string())
                            .unwrap_or_default(),
                        current: app.selected_book().and_then(|b| b.rating),
                    });
                    return;
                }
                's' => {
                    // change status (cycle)
                    app.reset_vim_count();
                    app.mode = Mode::Normal;
                    app.cycle_status();
                    app.status_message = "Quick edit: Status cycled".to_string();
                    return;
                }
                'y' => {
                    // change year
                    app.reset_vim_count();
                    app.mode = Mode::Normal;
                    if let Some(book) = app.selected_book() {
                        let id = book.id.to_string();
                        let year_str = book.year.map_or(String::new(), |y| y.to_string());
                        app.popup = Some(crate::popup::Popup::EditYear {
                            book_id: id,
                            input: year_str.clone(),
                            cursor: year_str.len(),
                        });
                    }
                    app.status_message = "Quick edit: Year".to_string();
                    return;
                }
                'n' => {
                    // change notes (not available yet, status message)
                    app.reset_vim_count();
                    app.mode = Mode::Normal;
                    app.status_message = "Quick edit: Notes (not yet implemented)".to_string();
                    return;
                }
                _ => {}
            }
        }

        // 2. Handle Text Objects (prefix `i` or `a`)
        if let Some(pending) = app.pending_key {
            if pending == 'i' || pending == 'a' {
                app.pending_key = None;
                let kind = if pending == 'i' {
                    TextObjectKind::Inner
                } else {
                    TextObjectKind::Around
                };

                if let Some(range) = text_objects::get_text_object_range(app, c, kind) {
                    execute_operator(app, operator, range);
                }
                app.reset_vim_count();
                app.mode = Mode::Normal;
                return;
            }

            // If we were waiting for 'g' (e.g. `dg...`)
            if pending == 'g' {
                app.pending_key = None;
                if c == 'g' {
                    let count = app.count_or_one();
                    if let Some(range) = motions::get_motion_range(app, 'g', count) {
                        execute_operator(app, operator, range);
                    }
                    app.reset_vim_count();
                    app.mode = Mode::Normal;
                    return;
                }
            }

            if pending == 'f' || pending == 'F' || pending == 't' || pending == 'T' {
                app.pending_key = None;
                let count = app.count_or_one();
                if let Some(range) = find_char::get_find_char_range(app, pending, c, count) {
                    execute_operator(app, operator, range);
                    app.last_find_char = Some((c, pending));
                }
                app.reset_vim_count();
                app.mode = Mode::Normal;
                return;
            }
        }

        // If no pending key, checking for start of text object or special motion prefix
        if (c == 'i' || c == 'a' || c == 'f' || c == 'F' || c == 't' || c == 'T')
            && app.pending_key.is_none()
        {
            app.pending_key = Some(c);
            return;
        }

        if c == 'g' && app.pending_key.is_none() {
            app.pending_key = Some('g');
            return;
        }

        // 3. Handle Digit (accumulate count)
        if c.is_ascii_digit() && c != '0' {
            app.push_vim_digit(c.to_digit(10).unwrap());
            return;
        }
        if c == '0' && app.vim_count > 0 {
            app.push_vim_digit(0);
            return;
        }

        // '0' as motion when no count accumulated — go to top
        if c == '0' && app.vim_count == 0 {
            if let Some(range) = motions::get_motion_range(app, '0', 1) {
                execute_operator(app, operator, range);
            }
            app.reset_vim_count();
            app.mode = Mode::Normal;
            return;
        }
    }

    // 4. Handle Motions
    let count = app.count_or_one();
    let motion_char = match code {
        KeyCode::Char(c) => c,
        KeyCode::Down => 'j',
        KeyCode::Up => 'k',
        KeyCode::Esc => {
            app.mode = Mode::Normal;
            app.reset_vim_count();
            return;
        }
        _ => return,
    };

    if let Some(range) = motions::get_motion_range(app, motion_char, count) {
        execute_operator(app, operator, range);
        app.reset_vim_count();
        app.mode = Mode::Normal;
    }
}
