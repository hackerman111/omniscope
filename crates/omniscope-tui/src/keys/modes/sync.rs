use crate::app::{ActivePanel, App};
use crossterm::event::{KeyCode, KeyModifiers};

pub fn handle_sync_mode(app: &mut App, code: KeyCode, _modifiers: KeyModifiers) {
    match code {
        KeyCode::Esc => {
            app.active_panel = ActivePanel::BookList;
            app.status_message = "Exited Sync Panel".to_string();
        }
        KeyCode::Char('k') | KeyCode::Up => app.sync_move_up(),
        KeyCode::Char('j') | KeyCode::Down => app.sync_move_down(),
        KeyCode::Char('a') => {
            // Apply automatic sync (DiskWins)
            if let (Some(lr), Some(db), Some(report)) =
                (app.library_root.as_ref(), app.db.as_ref(), &app.sync_report)
            {
                let sync = omniscope_core::sync::folder_sync::FolderSync::new(lr, db);
                if let Err(e) = sync.apply_sync(
                    report,
                    omniscope_core::sync::folder_sync::SyncResolution::DiskWins,
                ) {
                    app.status_message = format!("Error applying sync: {}", e);
                } else {
                    app.status_message =
                        "Automatically applied disk state to database.".to_string();
                    app.generate_sync_report();
                }
            } else {
                app.status_message = "Cannot sync: Library or DB missing.".to_string();
            }
        }
        KeyCode::Char('i') => {
            // Import selected item
            app.import_selected_untracked();
        }
        KeyCode::Char('I') => {
            // Import all untracked files
            app.import_all_untracked();
        }
        KeyCode::Enter => {
            // Import selected item (same as 'i')
            app.import_selected_untracked();
        }
        KeyCode::Char('s') => {
            app.status_message = "Action: Apply selected (WIP)".to_string();
        }
        _ => {}
    }
}
