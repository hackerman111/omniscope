use crossterm::event::KeyCode;
use crate::app::{App, Mode};

pub(crate) fn handle_command_mode(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc => {
            app.mode = Mode::Normal;
            app.command_input.clear();
            app.command_history_idx = None;
        }
        KeyCode::Enter => {
            let cmd = app.command_input.clone();
            app.mode = Mode::Normal;
            if !cmd.is_empty() {
                app.command_history.push(cmd.clone());
            }
            app.command_history_idx = None;
            crate::command::execute_command(app, &cmd);
            app.command_input.clear();
        }
        KeyCode::Backspace => {
            app.command_input.pop();
            if app.command_input.is_empty() {
                app.mode = Mode::Normal;
            }
        }
        KeyCode::Up => {
            // Navigate command history backward
            if app.command_history.is_empty() { return; }
            let idx = match app.command_history_idx {
                None => app.command_history.len().saturating_sub(1),
                Some(i) => i.saturating_sub(1),
            };
            app.command_history_idx = Some(idx);
            if let Some(cmd) = app.command_history.get(idx) {
                app.command_input = cmd.clone();
            }
        }
        KeyCode::Down => {
            if let Some(idx) = app.command_history_idx {
                if idx + 1 < app.command_history.len() {
                    app.command_history_idx = Some(idx + 1);
                    app.command_input = app.command_history[idx + 1].clone();
                } else {
                    app.command_history_idx = None;
                    app.command_input.clear();
                }
            }
        }
        KeyCode::Char(c) => {
            app.command_input.push(c);
            app.command_history_idx = None;
        }
        _ => {}
    }
}
