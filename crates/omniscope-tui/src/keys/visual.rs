use crossterm::event::{KeyCode, KeyModifiers};
use crate::app::{App, Mode};

pub(crate) fn handle_visual_mode(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    // ─── Handle pending key sequences (same as Normal) ──────────────────────────
    if let Some(pending) = app.pending_key.take() {
        // let count = app.count_or_one(); // unused for now in visual specific logic
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
        KeyCode::Esc => app.exit_visual_mode(),
        KeyCode::Char('v') if modifiers.contains(KeyModifiers::CONTROL) => {
            if app.mode == Mode::VisualBlock {
                app.exit_visual_mode();
            } else {
                app.enter_visual_mode(Mode::VisualBlock);
            }
        }
        KeyCode::Char('V') => {
            if app.mode == Mode::VisualLine {
                app.exit_visual_mode();
            } else {
                app.enter_visual_mode(Mode::VisualLine);
            }
        }
        KeyCode::Char('v') => {
            if app.mode == Mode::Visual {
                app.exit_visual_mode();
            } else {
                app.enter_visual_mode(Mode::Visual);
            }
        }

        // Navigation through standard motions
        // We will catch known motion keys and pass them to Motions.
        KeyCode::Char(c) if "jkhlGg0$".contains(c) => {
            let n = app.count_or_one();
            app.reset_vim_count();
            
            // h/l change focus, which exits visual mode
            if c == 'h' {
                 app.exit_visual_mode();
                 app.focus_left();
                 return;
            } else if c == 'l' {
                 app.exit_visual_mode();
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
             app.exit_visual_mode();
             app.focus_left();
        }
        KeyCode::Right => {
             app.reset_vim_count(); 
             app.exit_visual_mode();
             app.focus_right(); 
        }

        // Half-page scroll
        // Kept separate as it uses `app.move_down_n` internally, but could be unified if desired.
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
            // Yank selection
            let selections = app.visual_selections.clone();
            app.yank_indices(&selections); 
            app.exit_visual_mode();
        }
        KeyCode::Char('d') | KeyCode::Char('x') => {
             // Delete selection
             let selections = app.visual_selections.clone();
             app.delete_indices(&selections);
             app.exit_visual_mode();
        }

        // Registers
        KeyCode::Char('"') => {
            app.pending_register_select = true;
        }

        _ => {}
    }
}
