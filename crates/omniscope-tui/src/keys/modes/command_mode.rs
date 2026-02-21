use crate::app::{App, Mode};
use crossterm::event::KeyCode;

pub(crate) fn handle_command_mode(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc => {
            app.mode = Mode::Normal;
            app.command_input.clear();
            app.command_history_idx = None;
            app.command_suggestions.clear();
            app.command_suggestion_idx = None;
        }
        KeyCode::Enter => {
            let cmd = app.command_input.clone();
            app.mode = Mode::Normal;
            if !cmd.is_empty() {
                app.command_history.push(cmd.clone());
            }
            app.command_history_idx = None;
            app.command_suggestions.clear();
            app.command_suggestion_idx = None;
            crate::command::execute_command(app, &cmd);
            app.command_input.clear();
        }
        KeyCode::Backspace => {
            app.command_input.pop();
            if app.command_input.is_empty() {
                app.mode = Mode::Normal;
                app.command_suggestions.clear();
                app.command_suggestion_idx = None;
            } else {
                update_suggestions(app);
            }
        }
        KeyCode::Tab => {
            if !app.command_suggestions.is_empty() {
                let idx = app.command_suggestion_idx.unwrap_or(0);
                if idx + 1 < app.command_suggestions.len() {
                    app.command_suggestion_idx = Some(idx + 1);
                } else {
                    app.command_suggestion_idx = Some(0);
                }
                if let Some(suggestion) = app
                    .command_suggestions
                    .get(app.command_suggestion_idx.unwrap())
                {
                    app.command_input = suggestion.to_string();
                }
            }
        }
        KeyCode::BackTab => {
            if !app.command_suggestions.is_empty() {
                let idx = app.command_suggestion_idx.unwrap_or(0);
                if idx > 0 {
                    app.command_suggestion_idx = Some(idx - 1);
                } else {
                    app.command_suggestion_idx = Some(app.command_suggestions.len() - 1);
                }
                if let Some(suggestion) = app
                    .command_suggestions
                    .get(app.command_suggestion_idx.unwrap())
                {
                    app.command_input = suggestion.to_string();
                }
            }
        }
        KeyCode::Up => {
            if app.command_history.is_empty() {
                return;
            }
            let idx = match app.command_history_idx {
                None => app.command_history.len().saturating_sub(1),
                Some(i) => i.saturating_sub(1),
            };
            app.command_history_idx = Some(idx);
            if let Some(cmd) = app.command_history.get(idx) {
                app.command_input = cmd.clone();
            }
            app.command_suggestions.clear();
            app.command_suggestion_idx = None;
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
            app.command_suggestions.clear();
            app.command_suggestion_idx = None;
        }
        KeyCode::Char(c) => {
            app.command_input.push(c);
            app.command_history_idx = None;
            update_suggestions(app);
        }
        _ => {}
    }
}

fn update_suggestions(app: &mut App) {
    let suggestions = crate::command::parser::get_command_suggestions(&app.command_input);
    app.command_suggestions = suggestions.iter().map(|s| s.to_string()).collect();
    app.command_suggestion_idx = if app.command_suggestions.is_empty() {
        None
    } else {
        Some(0)
    };
}
