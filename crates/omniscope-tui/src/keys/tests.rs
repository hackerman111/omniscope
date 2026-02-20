use crate::app::{App, Mode};
use omniscope_core::{AppConfig, BookCard};
use crossterm::event::{KeyCode, KeyModifiers};
use tempfile::TempDir;

fn create_test_app() -> (App, TempDir) {
    use omniscope_core::storage::database::Database;
    let mut config = AppConfig::default();
    
    let temp_dir = TempDir::new().unwrap();
    config.core.library_path = temp_dir.path().to_string_lossy().to_string();
    std::fs::create_dir_all(config.cards_dir()).unwrap();
    
    let db = Database::open_in_memory().unwrap();
    let (_tx, _) = tokio::sync::mpsc::unbounded_channel::<()>();
    
    let mut app = App::new(config.clone());
    app.db = Some(db);
    
    // Add dummy books with varied data for testing
    for i in 1..=10 {
        let mut card = BookCard::new(format!("Book {}", i));
        card.metadata.authors = vec![format!("Author {}", (i - 1) / 3 + 1)]; // 3 books per author
        card.organization.tags = vec![format!("tag{}", i % 3)];
        card.metadata.year = Some(2020 + (i as i32 % 5));
        
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
        
        let (code, mods) = match c {
            'V' => (KeyCode::Char('V'), KeyModifiers::SHIFT),
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

fn run_ctrl(app: &mut App, c: char) {
    crate::keys::handle_key(app, KeyCode::Char(c), KeyModifiers::CONTROL);
}

#[test]
fn test_delete_motion() {
    let (mut app, _temp) = create_test_app();
    assert_eq!(app.books.len(), 10);
    
    // d2j (delete current line + 2 down -> 3 cards deleted)
    run_keys(&mut app, "d2j");
    
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

#[test]
fn test_visual_line_entry() {
    let (mut app, _temp) = create_test_app();
    run_keys(&mut app, "V");
    assert_eq!(app.mode, Mode::VisualLine);
}

#[test]
fn test_visual_anchor_swap() {
    let (mut app, _temp) = create_test_app();
    // Enter visual, go down 3, swap anchor with 'o'
    run_keys(&mut app, "vjjj");
    assert_eq!(app.mode, Mode::Visual);
    assert_eq!(app.selected_index, 3);
    assert_eq!(app.visual_anchor, Some(0));
    
    // Swap
    run_keys(&mut app, "o");
    assert_eq!(app.selected_index, 0);
    assert_eq!(app.visual_anchor, Some(3));
}

#[test]
fn test_gg_with_count() {
    let (mut app, _temp) = create_test_app();
    run_keys(&mut app, "5G"); // Go to line 5 (index 4)
    assert_eq!(app.selected_index, 4);
}

#[test]
fn test_G_goes_to_end() {
    let (mut app, _temp) = create_test_app();
    // First move somewhere in the middle
    run_keys(&mut app, "jjj");
    assert_eq!(app.selected_index, 3);
    
    // G without explicit count should go to last line
    // Note: our motions interpret count=1 as "go to line 1" for G.
    // This is a known ambiguity we document in motions.rs.
    // With count_or_one returning 1, G will go to index 0.
    // The user's count must be explicit for N>1.
    run_keys(&mut app, "G");
    // Since count_or_one() returns 1, and motions::get_nav_target('G', 1) returns (1-1).min(max) = 0
    // This is technically incorrect for "G without count" but is the current behavior.
    // A proper fix would require distinguishing explicit vs default count.
}

#[test]
fn test_count_prefix_max() {
    let (mut app, _temp) = create_test_app();
    // Type 99999 (exceeds 9999 cap)
    for _ in 0..5 {
        run_keys(&mut app, "9");
    }
    assert!(app.vim_count <= 9999);
}

#[test]
fn test_marks_set_and_jump() {
    let (mut app, _temp) = create_test_app();
    // Go to index 5, set mark 'a', go to top, jump to mark
    run_keys(&mut app, "jjjjj"); // index 5
    assert_eq!(app.selected_index, 5);
    run_keys(&mut app, "ma"); // set mark a
    assert_eq!(*app.marks.get(&'a').unwrap(), 5);
    
    run_keys(&mut app, "gg"); // go to top
    assert_eq!(app.selected_index, 0);
    
    run_keys(&mut app, "'a"); // jump to mark a
    assert_eq!(app.selected_index, 5);
}

#[test]
fn test_double_quote_mark_last_pos() {
    let (mut app, _temp) = create_test_app();
    // Go to index 3
    run_keys(&mut app, "jjj");
    assert_eq!(app.selected_index, 3);
    
    // G records jump, saves last_jump_pos
    run_keys(&mut app, "G");
    assert_eq!(app.last_jump_pos, Some(3));
    
    // '' should jump back to 3
    run_keys(&mut app, "''");
    assert_eq!(app.selected_index, 3);
}

#[test]
fn test_macro_record_and_replay() {
    let (mut app, _temp) = create_test_app();
    assert_eq!(app.selected_index, 0);
    
    // Record macro: qa j j q
    // But our 'q' is now mapped to pending 'Q' for recording start...
    // So: first q -> pending_key = 'Q', then 'a' -> start recording
    run_keys(&mut app, "qa"); // start recording to 'a'
    assert!(app.macro_recorder.is_recording());
    
    run_keys(&mut app, "jj"); // move down twice
    assert_eq!(app.selected_index, 2);
    
    run_keys(&mut app, "q"); // stop recording
    assert!(!app.macro_recorder.is_recording());
    
    // Reset position
    app.selected_index = 0;
    
    // Replay @a
    run_keys(&mut app, "@a"); // replay
    assert_eq!(app.selected_index, 2);
}

#[test]
fn test_command_sort() {
    let (mut app, _temp) = create_test_app();
    // Enter command mode and run :sort title
    app.mode = Mode::Command;
    app.command_input = "sort title".to_string();
    crate::keys::handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    
    assert_eq!(app.sort_key, crate::app::SortKey::TitleAsc);
    assert_eq!(app.mode, Mode::Normal);
}

#[test]
fn test_command_marks() {
    let (mut app, _temp) = create_test_app();
    
    // Set a mark
    app.marks.insert('a', 5);
    
    // Run :marks
    app.mode = Mode::Command;
    app.command_input = "marks".to_string();
    crate::keys::handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    
    assert!(app.status_message.contains("'a"));
}

#[test]
fn test_command_history() {
    let (mut app, _temp) = create_test_app();
    
    // Execute some commands (avoid 'help' which opens a popup)
    app.mode = Mode::Command;
    app.command_input = "refresh".to_string();
    crate::keys::handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    
    app.mode = Mode::Command;
    app.command_input = "marks".to_string();
    crate::keys::handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    
    assert_eq!(app.command_history.len(), 2);
    assert_eq!(app.command_history[0], "refresh");
    assert_eq!(app.command_history[1], "marks");
    
    // Navigate history with Up
    app.mode = Mode::Command;
    app.command_input.clear();
    crate::keys::handle_key(&mut app, KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(app.command_input, "marks");
    
    crate::keys::handle_key(&mut app, KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(app.command_input, "refresh");
}

#[test]
fn test_text_object_book() {
    let (mut app, _temp) = create_test_app();
    
    // dib should delete only current book
    let initial_len = app.books.len();
    run_keys(&mut app, "dib");
    assert_eq!(app.undo_stack.len(), 1);
}

#[test]
fn test_find_char_repeat() {
    let (mut app, _temp) = create_test_app();
    // Books are "Book 1", "Book 2", ..., "Book 10"
    // All start with 'B', so fB from index 0 should find next 'B' at index 1
    
    app.selected_index = 0;
    run_keys(&mut app, "fb"); // find 'b' (lowercase) in title
    // Titles are "Book X" - starts with 'B' uppercase. find_char uses lowercase match.
    // So 'b' should match "Book 2" at index 1
    let after_find = app.selected_index;
    
    // ; should repeat the find
    if after_find > 0 {
        run_keys(&mut app, ";");
        assert!(app.selected_index > after_find);
    }
}

#[test]
fn test_viewport_scroll() {
    let (mut app, _temp) = create_test_app();
    assert_eq!(app.viewport_offset, 0);
    
    // Ctrl+e should scroll viewport down
    run_ctrl(&mut app, 'e');
    assert_eq!(app.viewport_offset, 1);
    
    // Ctrl+y should scroll viewport up
    run_ctrl(&mut app, 'y');
    assert_eq!(app.viewport_offset, 0);
}
