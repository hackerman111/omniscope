use crossterm::event::{KeyCode, KeyModifiers};
use crate::app::{App, Mode};
use super::motions;
// use super::text_objects; // will use later

pub(crate) fn handle_pending_mode(app: &mut App, code: KeyCode, _modifiers: KeyModifiers) {
    let operator = match app.vim_operator {
        Some(op) => op,
        None => {
            // Should not happen in Pending mode without operator, 
            // unless we are in some other pending state?
            // "g" or "z" pending state is also possible.
            // But usually we set Mode::Pending only for operators OR we can use it for "g".
            // App struct has `pending_key`.
            // Let's assume Mode::Pending is specifically for Operator Pending for now, 
            // or we unify `pending_key` logic here.
            app.mode = Mode::Normal;
            return;
        }
    };

    // If we have a pending operator 'd', 'y', 'c', '>', '<', '='
    
    // 1. Handle double operator (dd, yy, cc, etc.) -> linewise operation on current line
    if let KeyCode::Char(ch) = code {
        if ch == operator {
            // Execute linewise on current line (or count lines)
            let count = app.count_or_one();
            let start = app.selected_index;
            let end = (start + count - 1).min(app.books.len().saturating_sub(1));
            let range: Vec<usize> = (start..=end).collect();
            
            execute_operator(app, operator, range);
            app.reset_vim_count();
            app.mode = Mode::Normal;
            return;
        }
    }

    // 2. Handle motions
    
    // Check for pending motion keys (like 'g' waiting for second 'g')
    if let Some(pmt) = app.pending_key {
        if pmt == 'g' {
             app.pending_key = None; // consume it
             if code == KeyCode::Char('g') {
                 // matched 'gg'
                 // execute motion 'g' (which we mapped to top in motions.rs)
                 let count = app.count_or_one();
                 if let Some(range) = motions::get_motion_range(app, 'g', count) {
                     execute_operator(app, operator, range);
                 }
                 app.reset_vim_count();
                 app.mode = Mode::Normal;
                 return;
             } else {
                 // handle other g-commands in pending mode? e.g. 'ge' end of word backward?
                 // or just ignore/reset if invalid sequence
                 app.reset_vim_count();
                 app.mode = Mode::Normal;
                 return; 
             }
        }
        // handle other pending keys if necessary
    }
    
    // Check for text object prefix 'i' or 'a' in pending_key is tricky because they are also normal keys.
    // But in Pending Mode, 'i' and 'a' usually start a text object.
    // UNLESS the operator is 'c' and we type 'c' (cc).
    // OR operator is 'd' and we type 'd' (dd).
    // If we type 'i', it's likely "inner ...".
    
    if let KeyCode::Char(c) = code {
         // Handle 'g' as start of sequence
         if c == 'g' {
             app.pending_key = Some('g');
             return;
         }
         
         // Text object prefixes
         if (c == 'i' || c == 'a') && app.pending_key.is_none() {
             // We need to store that we are waiting for a text object.
             // We can use pending_key for this too?
             // 'i' or 'a'
             app.pending_key = Some(c);
             return;
         }
         
         // If we have a pending text object prefix
         if let Some(prefix) = app.pending_key {
             if prefix == 'i' || prefix == 'a' {
                 // matched text object: prefix + c
                 app.pending_key = None;
                 // Resolve text object range
                 let kind = if prefix == 'i' { crate::keys::text_objects::TextObjectKind::Inner } else { crate::keys::text_objects::TextObjectKind::Around };
                 
                 if let Some(range) = crate::keys::text_objects::get_text_object_range(app, c, kind) {
                     execute_operator(app, operator, range);
                 }
                 app.reset_vim_count();
                 app.mode = Mode::Normal;
                 return;
             }
         }

         if c.is_ascii_digit() && c != '0' {
             // Multiply existing count? or start new count?
             // Vim parsing: [count] op [count] motion. Total = count1 * count2.
             // We already consumed count1 into `app.vim_count` before entering pending.
             // But wait, `app.reset_vim_count` might have been called?
             // Actually, usually we store "operator count".
             
             // Simplification: just accumulate digit into vim_count.
             app.push_vim_digit(c.to_digit(10).unwrap());
             return;
         }
    }
    
    // Check if it's a motion
    let count = app.count_or_one();
    let motion_char = match code {
        KeyCode::Char(c) => c,
        // map other keys like Down -> 'j'
        KeyCode::Down => 'j',
        KeyCode::Up => 'k',
        KeyCode::Esc => {
            app.mode = Mode::Normal;
            app.reset_vim_count();
            return;
        }
        _ => return, // ignore unknown keys or handle better?
    };
    
    if let Some(range) = motions::get_motion_range(app, motion_char, count) {
        execute_operator(app, operator, range);
        app.reset_vim_count();
        app.mode = Mode::Normal;
    } else {
        // Maybe it's a text object?
        // Check for text object (e.g. 'i', 'a' prefix)
        // Need to handle 'pending_key' for text objects ('i' or 'a') too?
        // Or handle here directly if it's 'i'/'a'
        // For now complex text objects (like `diw`) would require more state.
        // Let's just reset if unknown
        app.mode = Mode::Normal;
        app.reset_vim_count();
    }
}

fn execute_operator(app: &mut App, op: char, range: Vec<usize>) {
    match op {
        'd' => {
            // Delete range
            // We need to handle this in app/vim.rs or here.
            // Ideally call app.delete_indices(range)
            app.delete_indices(&range); // Assume this method exists or we add it
        }
        'y' => {
            app.yank_indices(&range);
        }
        'c' => {
            // Change = delete + insert (or special action)
            // For now, let's just implement 'delete' part and maybe enter insert?
            // "change 3 books" -> delete them? No, that's not right.
            // "change tag" (ct) -> special logic.
            // If it's a known special combo like `ct`, we might handle it differently.
            // But here we are executing generic operator on a range.
            // If range is valid, maybe we open edit for the first one?
            // Spec: 2cc -> open form for 2 books sequentially.
            // So we should trigger "edit multiple".
            // app.edit_indices(range) ?
            app.status_message = format!("Change on {} items not impl", range.len());
        }
        '>' => {
            // Add tag
            app.status_message = "Add tag to range".to_string();
        }
        '<' => {
            // Remove tag
             app.status_message = "Remove tag from range".to_string();
        }
        _ => {}
    }
}
