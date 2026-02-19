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

        // Navigation
        KeyCode::Char('j') | KeyCode::Down => {
            let n = app.count_or_one();
            app.reset_vim_count();
            app.move_down_n(n);
            app.update_visual_selection();
        }
        KeyCode::Char('k') | KeyCode::Up => {
            let n = app.count_or_one();
            app.reset_vim_count();
            app.move_up_n(n);
            app.update_visual_selection();
        }
        KeyCode::Char('h') | KeyCode::Left  => { 
            app.reset_vim_count(); 
            app.exit_visual_mode();
            app.focus_left();
        }
        KeyCode::Char('l') | KeyCode::Right => { 
            app.reset_vim_count(); 
            app.exit_visual_mode();
            app.focus_right(); 
        }

        KeyCode::Char('G') => {
             app.reset_vim_count(); 
             app.move_to_bottom();
             app.update_visual_selection();
        }
        KeyCode::Char('g') => { app.pending_key = Some('g'); }
        KeyCode::Char('0') => {
             app.reset_vim_count(); 
             app.move_to_top();
             app.update_visual_selection();
        }
        KeyCode::Char('$') => {
             app.reset_vim_count();
             app.move_to_bottom(); // List doesn't have horizontal scroll really
             app.update_visual_selection();
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
