use crossterm::event::KeyCode;
use crate::app::App;

pub fn handle_sort_command(app: &mut App, code: KeyCode) {
    use crate::app::SortKey;
    match code {
        KeyCode::Char('y') => {
            app.sort_key = SortKey::YearDesc;
            app.apply_sort();
            app.status_message = "Sort: Year Desc".to_string();
        }
        KeyCode::Char('Y') => {
            app.sort_key = SortKey::YearAsc;
            app.apply_sort();
            app.status_message = "Sort: Year Asc".to_string();
        }
        KeyCode::Char('t') => {
            app.sort_key = SortKey::TitleAsc;
            app.apply_sort();
            app.status_message = "Sort: Title Asc".to_string();
        }
        KeyCode::Char('r') => {
            app.sort_key = SortKey::RatingDesc;
            app.apply_sort();
            app.status_message = "Sort: Rating Desc".to_string();
        }
        KeyCode::Char('f') => {
            app.sort_key = SortKey::FrecencyDesc;
            app.apply_sort();
            app.status_message = "Sort: Frecency".to_string();
        }
        KeyCode::Char('u') => {
            app.sort_key = SortKey::UpdatedDesc;
            app.apply_sort();
            app.status_message = "Sort: Updated (Default)".to_string();
        }
        _ => {
             if code == KeyCode::Esc {
                 app.status_message.clear();
             } else {
                 app.status_message = "Sort cancelled".to_string();
             }
        }
    }
}
