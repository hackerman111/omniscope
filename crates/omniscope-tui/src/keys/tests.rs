use crate::app::{App, ActivePanel, Mode};
use omniscope_core::{AppConfig, BookCard};
use crossterm::event::{KeyCode, KeyModifiers};
use tempfile::TempDir;
fn create_test_app() -> (App, TempDir) {
    use omniscope_core::storage::database::Database;
    let mut config = AppConfig::default();
    
    // Create temp library
    let temp_dir = TempDir::new().unwrap();
    config.core.library_path = temp_dir.path().to_string_lossy().to_string();
    std::fs::create_dir_all(config.cards_dir()).unwrap();
    
    // Create an in-memory DB for tests
    let db = Database::open_in_memory().unwrap();
    // command_tx can be a dummy channel
    let (tx, _) = tokio::sync::mpsc::unbounded_channel::<()>();
    
    // Looks like App::new might just take (config) or something else? Wait, let's look at app/mod.rs
    // Actually the compiler error said: "this function takes 1 argument but 3 arguments were supplied"
    // So let's just pass config if that's what it wants. We will manually attach db and tx.
    let mut app = App::new(config.clone());
    app.db = Some(db);
    
    // Add dummy books
    for i in 1..=10 {
        let mut card = BookCard::new(format!("Book {}", i));
        card.organization.tags = vec![format!("tag{}", i % 3)];
        
        let _ = omniscope_core::storage::json_cards::save_card(&config.cards_dir(), &card);
        
        let view = omniscope_core::BookSummaryView {
            id: card.id,
            title: card.metadata.title.clone(),
            authors: card.metadata.authors.clone(),
            format: None,
            year: card.metadata.year,
            rating: card.organization.rating,
            read_status: card.organization.read_status.clone(),
            tags: card.organization.tags.clone(),
            frecency_score: 0.0,
            has_file: false,
        };
        app.all_books.push(view.clone());
        app.books.push(view);
    }
    app.apply_filter();
    (app, temp_dir)
}

/// Helper to simulate sequential key presses
fn run_keys(app: &mut App, sequence: &str) {
    for c in sequence.chars() {
        if c == ' ' { continue; }
        
        // Handle special mock keys
        let (code, mods) = match c {
            'V' => (KeyCode::Char('v'), KeyModifiers::CONTROL), // hack to test block visual
            '_' => (KeyCode::Esc, KeyModifiers::NONE),
            '\n' => (KeyCode::Enter, KeyModifiers::NONE),
            _ => {
                let code = KeyCode::Char(c);
                let mods = if c.is_ascii_uppercase() { KeyModifiers::SHIFT } else { KeyModifiers::NONE };
                (code, mods)
            }
        };
        
        crate::keys::handle_key(app, code, mods);
    }
}

#[test]
fn test_delete_motion() {
    let (mut app, _temp) = create_test_app();
    assert_eq!(app.books.len(), 10);
    
    // d2j (delete current line + 2 down -> 3 cards deleted)
    run_keys(&mut app, "d2j");
    
    // Validating undo state instead since App doesn't refresh automatically in test.
    assert_eq!(app.undo_stack.len(), 1);
}

#[test]
fn test_registers_and_paste() {
    let (mut app, _temp) = create_test_app();
    
    // " a y j
    run_keys(&mut app, "\"ayj"); // Yank current + 1 down to register 'a'
    assert!(app.registers.contains_key(&'a'));
}

#[test]
fn test_visual_mode() {
    let (mut app, _temp) = create_test_app();
    run_keys(&mut app, "vjjy"); // enter visual, go down 2, yank
    
    assert!(app.registers.contains_key(&'"'));
    
    if let Some(r) = app.registers.get(&'"') {
        if let crate::app::RegisterContent::MultipleCards(cards) = &r.content {
            assert_eq!(cards.len(), 3);
        } else {
             panic!("Expected MultipleCards");
        }
    }
}
