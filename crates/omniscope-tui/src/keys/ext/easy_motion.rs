use crossterm::event::KeyCode;
use crate::app::App;
use crate::popup::{Popup, EasyMotionState};

pub fn handle_easy_motion_start(app: &mut App, code: KeyCode) {
    if app.books.is_empty() {
        app.status_message = "No books to jump to".to_string();
        return;
    }

    match code {
        KeyCode::Char(' ') => {
            // Space Space - Assign labels to nearby books
            let mut targets = Vec::new();
            let start = app.selected_index.saturating_sub(26);
            let end = (app.selected_index + 26).min(app.books.len());
            
            let chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
            for (i, idx) in (start..end).enumerate() {
                if i < chars.len() {
                    targets.push((chars.chars().nth(i).unwrap(), idx));
                }
            }
            
            app.popup = Some(Popup::EasyMotion(EasyMotionState {
                pending: false,
                targets,
            }));
            app.status_message = "EasyMotion: type target label".to_string();
        }
        KeyCode::Char('j') | KeyCode::Down => {
            // Space j - EasyMotion below cursor
            let mut targets = Vec::new();
            let start = app.selected_index;
            let end = (app.selected_index + 52).min(app.books.len());
            
            let chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
            for (i, idx) in (start..end).enumerate() {
                if i < chars.len() {
                    targets.push((chars.chars().nth(i).unwrap(), idx));
                }
            }
            
            app.popup = Some(Popup::EasyMotion(EasyMotionState {
                pending: false,
                targets,
            }));
            app.status_message = "EasyMotion (Down): type target label".to_string();
        }
        KeyCode::Char('k') | KeyCode::Up => {
            // Space k - EasyMotion above cursor
            let mut targets = Vec::new();
            let start = app.selected_index.saturating_sub(52);
            let end = app.selected_index;
            
            let chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
            for (i, idx) in (start..end).rev().enumerate() {
                if i < chars.len() {
                    targets.push((chars.chars().nth(i).unwrap(), idx));
                }
            }
            
            app.popup = Some(Popup::EasyMotion(EasyMotionState {
                pending: false,
                targets,
            }));
            app.status_message = "EasyMotion (Up): type target label".to_string();
        }
        KeyCode::Char('/') => {
            // Space / - EasyMotion by first letter: enter pending mode to prompt for a char
            app.popup = Some(Popup::EasyMotion(EasyMotionState {
                pending: true,  // Waiting for a character to filter by
                targets: Vec::new(),
            }));
            app.status_message = "EasyMotion: type first letter of title".to_string();
        }
        _ => {
            app.status_message = "Unknown EasyMotion/Leader command".to_string();
        }
    }
}

/// Handle the second key press in EasyMotion pending mode (Space / <char>).
/// This is called from popup_keys when the EasyMotion popup is in pending state.
pub fn build_easy_motion_targets_by_char(app: &App, filter_char: char) -> Vec<(char, usize)> {
    let filter_lower = filter_char.to_lowercase().next().unwrap_or(filter_char);
    let chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    
    let mut targets = Vec::new();
    let mut label_idx = 0;
    
    for (idx, book) in app.books.iter().enumerate() {
        if book.title.to_lowercase().starts_with(filter_lower) {
            if label_idx < chars.len() {
                targets.push((chars.chars().nth(label_idx).unwrap(), idx));
                label_idx += 1;
            }
        }
    }
    
    targets
}
