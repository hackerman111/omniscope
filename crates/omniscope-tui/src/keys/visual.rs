use crossterm::event::{KeyCode, KeyModifiers};
use crate::app::{App, Mode};

pub(crate) fn handle_visual_mode(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    // ── Digit accumulation for count prefix ──────────────────────────────
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

    // ── Handle pending key sequences (same as Normal) ──────────────────────────
    if let Some(pending) = app.pending_key.take() {
        app.reset_vim_count();

        match (pending, code) {
            // gg — go to top
            ('g', KeyCode::Char('g')) => {
                app.move_to_top();
                app.update_visual_selection();
            }
            // other pending keys ...
            _ => {}
        }
        return;
    }

    match code {
        KeyCode::Esc => {
            // Save visual range before exiting
            if let Some(anchor) = app.visual_anchor {
                let start = anchor.min(app.selected_index);
                let end = anchor.max(app.selected_index);
                app.last_visual_range = Some((start, end));
            }
            app.exit_visual_mode();
        }
        KeyCode::Char('v') if modifiers.contains(KeyModifiers::CONTROL) => {
            if app.mode == Mode::VisualBlock {
                save_and_exit_visual(app);
            } else {
                app.enter_visual_mode(Mode::VisualBlock);
            }
        }
        KeyCode::Char('V') => {
            if app.mode == Mode::VisualLine {
                save_and_exit_visual(app);
            } else {
                app.enter_visual_mode(Mode::VisualLine);
            }
        }
        KeyCode::Char('v') => {
            if app.mode == Mode::Visual {
                save_and_exit_visual(app);
            } else {
                app.enter_visual_mode(Mode::Visual);
            }
        }

        // o — swap anchor and cursor
        KeyCode::Char('o') => {
            if let Some(anchor) = app.visual_anchor {
                let current = app.selected_index;
                app.visual_anchor = Some(current);
                app.selected_index = anchor;
                app.update_visual_selection();
            }
        }

        // Space — toggle individual item selection
        KeyCode::Char(' ') => {
            app.toggle_visual_select();
            // Move down after toggling (like Vim)
            if app.selected_index < app.books.len().saturating_sub(1) {
                app.selected_index += 1;
            }
            app.status_message = format!("-- VISUAL -- {} selected", app.visual_selections.len());
        }

        // Ctrl+a — select all
        KeyCode::Char('a') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.visual_anchor = Some(0);
            app.selected_index = app.books.len().saturating_sub(1);
            app.visual_selections = (0..app.books.len()).collect();
            app.status_message = format!("-- VISUAL -- {} selected (all)", app.visual_selections.len());
        }

        // g prefix
        KeyCode::Char('g') => {
            app.pending_key = Some('g');
        }

        // Navigation through standard motions
        KeyCode::Char(c) if "jkhlG0$".contains(c) => {
            let n = app.count_or_one();
            app.reset_vim_count();
            
            // h/l change focus, which exits visual mode
            if c == 'h' {
                 save_and_exit_visual(app);
                 app.focus_left();
                 return;
            } else if c == 'l' {
                 save_and_exit_visual(app);
                 app.focus_right();
                 return;
            }
            
            if let Some(target) = super::motions::get_nav_target(app, c, n) {
                 app.selected_index = target;
                 app.update_visual_selection();
            }
        }
        KeyCode::Down => {
            let n = app.count_or_one();
            app.reset_vim_count();
            if let Some(target) = super::motions::get_nav_target(app, 'j', n) {
                 app.selected_index = target;
                 app.update_visual_selection();
            }
        }
        KeyCode::Up => {
            let n = app.count_or_one();
            app.reset_vim_count();
            if let Some(target) = super::motions::get_nav_target(app, 'k', n) {
                 app.selected_index = target;
                 app.update_visual_selection();
            }
        }
        KeyCode::Left => {
             app.reset_vim_count(); 
             save_and_exit_visual(app);
             app.focus_left();
        }
        KeyCode::Right => {
             app.reset_vim_count(); 
             save_and_exit_visual(app);
             app.focus_right(); 
        }

        // Half-page scroll
        KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
            let n = 10 * app.count_or_one();
            app.reset_vim_count();
            app.move_down_n(n);
            app.update_visual_selection();
        }
        KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
            let n = 10 * app.count_or_one();
            app.reset_vim_count();
            app.move_up_n(n);
            app.update_visual_selection();
        }

        // Operators
        KeyCode::Char('y') => {
            let selections = app.visual_selections.clone();
            app.yank_indices(&selections); 
            save_and_exit_visual(app);
        }
        KeyCode::Char('d') | KeyCode::Char('x') => {
             let selections = app.visual_selections.clone();
             app.delete_indices(&selections);
             save_and_exit_visual(app);
        }
        KeyCode::Char('c') => {
             // Change: delete selection
             let selections = app.visual_selections.clone();
             app.delete_indices(&selections);
             save_and_exit_visual(app);
             app.status_message = format!("Changed {} items", selections.len());
        }

        // Registers
        KeyCode::Char('"') => {
            app.pending_register_select = true;
        }

        // Quickfix send
        KeyCode::Char('q') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.quickfix_list = app.visual_selections.iter()
                .filter_map(|&i| app.books.get(i).cloned())
                .collect();
            app.quickfix_show = true;
            app.quickfix_selected = 0;
            let count = app.quickfix_list.len();
            save_and_exit_visual(app);
            app.status_message = format!("Sent {} items to quickfix", count);
        }

        _ => {}
    }
}

/// Save visual range and exit visual mode.
fn save_and_exit_visual(app: &mut App) {
    if let Some(anchor) = app.visual_anchor {
        let start = anchor.min(app.selected_index);
        let end = anchor.max(app.selected_index);
        app.last_visual_range = Some((start, end));
    }
    app.exit_visual_mode();
}
