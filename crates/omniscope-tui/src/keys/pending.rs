use crossterm::event::{KeyCode, KeyModifiers};
use crate::app::{App, Mode};
use super::motions;
use super::find_char;
use super::text_objects::{self, TextObjectKind};
use crate::keys::operator::Operator;

pub(crate) fn handle_pending_mode(app: &mut App, code: KeyCode, _modifiers: KeyModifiers) {
    let operator = match app.pending_operator {
        Some(op) => op,
        None => {
            app.mode = Mode::Normal;
            return;
        }
    };

    if let KeyCode::Char(c) = code {
        // 1. Handle Double Operator (e.g. `dd`, `yy`) -> Linewise on current line
        // We need to map key char to Operator to check if it matches
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
             match c {
                  'a' => { // change authors
                       app.reset_vim_count();
                       app.mode = Mode::Normal;
                       // We don't have an isolated authors popup yet, but we can open BookForm 
                       // or we could add a dedicated popup. For now, open full form.
                       app.open_add_popup();
                       app.status_message = "Quick edit: Authors (opening full form)".to_string();
                       return;
                  }
                  't' | 'T' => { // change tags / title
                       app.reset_vim_count();
                       app.mode = Mode::Normal;
                       app.open_edit_tags();
                       app.status_message = "Quick edit: Tags".to_string();
                       return;
                  }
                  'r' | 'R' => { // change rating
                       app.reset_vim_count();
                       app.mode = Mode::Normal;
                       app.popup = Some(crate::popup::Popup::SetRating {
                            id: app.selected_book().map(|b| b.id.to_string()).unwrap_or_default(),
                            current: app.selected_book().and_then(|b| b.rating),
                       });
                       return;
                  }
                  _ => {}
             }
        }

        // 2. Handle Text Objects (prefix `i` or `a`)
        // If we are already waiting for a text object target (e.g. we typed `di`)
        if let Some(pending) = app.pending_key {
            if pending == 'i' || pending == 'a' {
                // Completed text object sequence: `diw`, `dap`
                app.pending_key = None;
                let kind = if pending == 'i' { TextObjectKind::Inner } else { TextObjectKind::Around };
                
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
                    // `dgg` -> delete to top
                    // This is a motion `gg`.
                    let count = app.count_or_one();
                    // `gg` in motions is mapped to `g`
                    if let Some(range) = motions::get_motion_range(app, 'g', count) {
                        execute_operator(app, operator, range);
                    }
                    app.reset_vim_count();
                    app.mode = Mode::Normal;
                    return;
                }
                // Handle other g-motions if needed
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
        if (c == 'i' || c == 'a' || c == 'f' || c == 'F' || c == 't' || c == 'T') && app.pending_key.is_none() {
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
        // Note: '0' is treated as start-of-line motion `0` below, unless it's part of a count (which we don't fully disambiguate here perfectly yet, but `push_vim_digit` handles non-zero).
        // If we want `d10j`, the `1` is handled, then `0`... currently `0` might be interpreted as motion `0` if we aren't careful.
        // `app.vim_count` > 0 means we are building a number?
        // standard vim: `0` is digit if count > 0.
        if c == '0' && app.vim_count > 0 {
            app.push_vim_digit(0);
            return;
        }
    }

    // 4. Handle Motions
    // Map keys to motion characters
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
        _ => return, // ignore unknown keys
    };
    
    if let Some(range) = motions::get_motion_range(app, motion_char, count) {
        execute_operator(app, operator, range);
        app.reset_vim_count();
        app.mode = Mode::Normal;
    } else {
        // Unknown motion or invalid key
        // app.mode = Mode::Normal; 
        // app.reset_vim_count();
    }
}

fn execute_operator(app: &mut App, op: Operator, range: Vec<usize>) {
    match op {
        Operator::Delete => app.delete_indices(&range),
        Operator::Yank => app.yank_indices(&range),
        Operator::Change => {
             // Change: delete + enter insert mode?
             // For now, just delete and notify.
             // Ideally: app.delete_indices(&range); app.mode = Mode::Insert;
             // But "Insert" in this TUI usually means "Popup".
             // We don't have inline editing yet.
             app.status_message = format!("Change on {} items (not impl fully)", range.len());
        }
        Operator::AddTag => {
             // Indent/Add tag?
             app.status_message = "Indent > (not impl)".to_string();
        }
        Operator::RemoveTag => {
             // Outdent/Remove tag?
             app.status_message = "Outdent < (not impl)".to_string();
        }
        _ => {}
    }
}
