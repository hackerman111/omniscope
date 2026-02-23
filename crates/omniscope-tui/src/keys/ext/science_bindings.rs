use crate::app::App;
use crate::panels::citation_graph::GraphMode;
use crossterm::event::KeyCode;

pub fn handle_g_science_command(app: &mut App, code: KeyCode) -> bool {
    match code {
        KeyCode::Char('r') => {
            if app.has_science_context() {
                app.open_science_references_panel();
                true
            } else {
                false
            }
        }
        KeyCode::Char('R') => {
            app.open_science_citation_graph_panel(GraphMode::CitedBy);
            true
        }
        KeyCode::Char('s') => {
            if app.has_science_context() {
                app.open_science_related_panel();
                true
            } else {
                false
            }
        }
        KeyCode::Char('d') => {
            app.open_science_doi_in_browser();
            true
        }
        KeyCode::Char('a') => {
            app.open_science_arxiv_abs();
            true
        }
        KeyCode::Char('A') => {
            app.open_science_arxiv_pdf();
            true
        }
        KeyCode::Char('o') => {
            app.find_science_open_pdf();
            true
        }
        _ => false,
    }
}

pub fn handle_yank_science_command(app: &mut App, code: char) -> bool {
    match code {
        'D' => {
            app.yank_science_doi();
            true
        }
        'A' => {
            app.yank_science_arxiv();
            true
        }
        'B' => {
            app.yank_science_bibtex();
            true
        }
        'C' => {
            app.yank_science_citation(None);
            true
        }
        _ => false,
    }
}

pub fn handle_change_science_command(app: &mut App, code: char) -> bool {
    match code {
        'D' => {
            app.start_edit_science_doi();
            true
        }
        'A' => {
            app.start_edit_science_arxiv_id();
            true
        }
        _ => false,
    }
}

pub fn handle_at_science_command(app: &mut App, code: char) -> bool {
    match code {
        'e' => {
            app.trigger_ai_enrich_metadata();
            true
        }
        'm' => {
            app.trigger_metadata_enrich();
            true
        }
        'r' => {
            app.trigger_ai_extract_references();
            true
        }
        _ => false,
    }
}
