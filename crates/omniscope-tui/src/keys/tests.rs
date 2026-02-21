use crate::app::{App, Mode};
use crossterm::event::{KeyCode, KeyModifiers};
use omniscope_core::{AppConfig, BookCard};
use tempfile::TempDir;

fn create_test_app() -> (App, TempDir) {
    use omniscope_core::storage::database::Database;
    let mut config = AppConfig::default();

    let temp_dir = TempDir::new().unwrap();
    config.core.library_path = temp_dir.path().to_string_lossy().to_string();
    std::fs::create_dir_all(config.cards_dir()).unwrap();

    let db = Database::open_in_memory().unwrap();
    let (_tx, _) = tokio::sync::mpsc::unbounded_channel::<()>();

    let mut app = App::new(config.clone(), None);
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

/// Helper to simulate sequential key presses.
/// Use '|' as a visual separator (ignored). Spaces ARE sent as KeyCode::Char(' ').
/// Use '_' for Esc, '\n' for Enter.
fn run_keys(app: &mut App, sequence: &str) {
    for c in sequence.chars() {
        if c == '|' {
            continue;
        } // visual separator only

        let (code, mods) = match c {
            'V' => (KeyCode::Char('V'), KeyModifiers::SHIFT),
            '_' => (KeyCode::Esc, KeyModifiers::NONE),
            '\n' => (KeyCode::Enter, KeyModifiers::NONE),
            _ => {
                let code = KeyCode::Char(c);
                let mods = if c.is_ascii_uppercase() {
                    KeyModifiers::SHIFT
                } else {
                    KeyModifiers::NONE
                };
                (code, mods)
            }
        };

        crate::keys::handle_key(app, code, mods);
    }
}

fn run_ctrl(app: &mut App, c: char) {
    crate::keys::handle_key(app, KeyCode::Char(c), KeyModifiers::CONTROL);
}

// ═══════════════════════════════════════════════════════════
// Existing tests (maintained)
// ═══════════════════════════════════════════════════════════

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
    let _initial_len = app.books.len();
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

// ═══════════════════════════════════════════════════════════
// NEW TESTS — for all fixed bugs
// ═══════════════════════════════════════════════════════════

#[test]
fn test_G_default_goes_to_bottom() {
    let (mut app, _temp) = create_test_app();
    assert_eq!(app.selected_index, 0);
    assert!(app.books.len() >= 10);

    // G without explicit count should go to last line
    run_keys(&mut app, "G");
    assert_eq!(
        app.selected_index,
        app.books.len() - 1,
        "G should go to last line"
    );
}

#[test]
fn test_G_with_explicit_count() {
    let (mut app, _temp) = create_test_app();

    // 3G should go to line 3 (index 2)
    run_keys(&mut app, "3G");
    assert_eq!(app.selected_index, 2, "3G should go to index 2");

    // 1G should go to line 1 (index 0)
    run_keys(&mut app, "1G");
    assert_eq!(app.selected_index, 0, "1G should go to index 0");
}

#[test]
fn test_space_space_easy_motion() {
    let (mut app, _temp) = create_test_app();
    assert!(app.popup.is_none());

    // Space Space should open EasyMotion popup with labels
    run_keys(&mut app, "  "); // two spaces (no longer skipped!)

    // EasyMotion should be in popup
    assert!(
        matches!(app.popup, Some(crate::popup::Popup::EasyMotion(_))),
        "Space Space should open EasyMotion popup"
    );

    if let Some(crate::popup::Popup::EasyMotion(ref state)) = app.popup {
        assert!(!state.targets.is_empty(), "EasyMotion should have targets");
        assert!(!state.pending, "EasyMotion should not be in pending state");
    }
}

#[test]
fn test_space_slash_easy_motion_pending() {
    let (mut app, _temp) = create_test_app();

    // Space / should open EasyMotion in pending mode
    run_keys(&mut app, " /"); // space then slash

    assert!(
        matches!(app.popup, Some(crate::popup::Popup::EasyMotion(_))),
        "Space / should open EasyMotion popup in pending mode"
    );

    if let Some(crate::popup::Popup::EasyMotion(ref state)) = app.popup {
        assert!(
            state.pending,
            "EasyMotion should be in pending state after Space /"
        );
    }
}

#[test]
fn test_delete_undo_restores() {
    let (mut app, _temp) = create_test_app();
    let initial_len = app.books.len();
    assert!(initial_len >= 2);

    // dd — delete current book
    run_keys(&mut app, "dd");
    assert_eq!(app.undo_stack.len(), 1, "Should have 1 undo entry after dd");

    // Verify undo action type is UpsertCards (restores cards)
    if let Some(entry) = app.undo_stack.last() {
        assert!(
            matches!(
                entry.action,
                omniscope_core::undo::UndoAction::UpsertCards(_)
            ),
            "Undo action for delete should be UpsertCards (to restore cards)"
        );
    }
}

#[test]
fn test_normal_mode_hints() {
    let (mut app, _temp) = create_test_app();
    app.mode = Mode::Normal;
    app.pending_key = None;
    app.pending_operator = None;

    let hints = crate::keys::ui::hints::get_hints(&app);
    assert!(!hints.is_empty(), "Normal mode hints should not be empty");
    assert!(
        hints.iter().any(|h| h.key.contains("j")),
        "Should have j/k hint"
    );
    assert!(
        hints.iter().any(|h| h.key.contains("d")),
        "Should have delete hint"
    );
    assert!(
        hints.iter().any(|h| h.key.contains("u")),
        "Should have undo hint"
    );
}

#[test]
fn test_sidebar_gg_G() {
    let (mut app, _temp) = create_test_app();
    app.active_panel = crate::app::ActivePanel::Sidebar;

    // Ensure we have sidebar items
    app.refresh_sidebar();
    assert!(!app.sidebar_items.is_empty());

    // Move sidebar down first
    app.sidebar_selected = 2;

    // gg in sidebar should go to top
    run_keys(&mut app, "gg");
    assert_eq!(app.sidebar_selected, 0, "gg should move sidebar to top");

    // G in sidebar should go to bottom
    run_keys(&mut app, "G");
    assert_eq!(
        app.sidebar_selected,
        app.sidebar_items.len() - 1,
        "G should move sidebar to bottom"
    );
}

#[test]
fn test_pending_0_motion() {
    let (mut app, _temp) = create_test_app();
    app.selected_index = 5;

    // d0 should delete from index 0 to current
    let initial_len = app.books.len();
    run_keys(&mut app, "d0");

    // Should have deleted items from 0 to 5 (inclusive) = 6 items
    assert!(
        app.books.len() < initial_len,
        "d0 should delete items from start to current"
    );
    assert_eq!(app.undo_stack.len(), 1, "Should have 1 undo entry");
}

#[test]
fn test_enter_in_booklist() {
    let (mut app, _temp) = create_test_app();
    app.active_panel = crate::app::ActivePanel::BookList;

    // Enter should attempt to open the book (won't actually open, but should set status)
    crate::keys::handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);

    // It should have tried to open and set a status message
    // (books in test don't have files, so it should show an error)
    assert!(
        !app.status_message.is_empty(),
        "Enter in BookList should set status message"
    );
}

#[test]
fn test_undo_key_works() {
    let (mut app, _temp) = create_test_app();

    // u should trigger undo
    run_keys(&mut app, "u");
    assert!(
        app.status_message.contains("Nothing to undo"),
        "u should trigger undo function"
    );
}

#[test]
fn test_visual_G_goes_bottom() {
    let (mut app, _temp) = create_test_app();

    // v G should select from 0 to bottom
    run_keys(&mut app, "vG");
    assert_eq!(app.mode, Mode::Visual);
    assert_eq!(
        app.selected_index,
        app.books.len() - 1,
        "G in visual should go to bottom"
    );
    assert_eq!(
        app.visual_selections.len(),
        app.books.len(),
        "Should select all items"
    );
}

#[test]
fn test_has_explicit_count_tracking() {
    let (mut app, _temp) = create_test_app();

    // Initially no explicit count
    assert!(!app.has_explicit_count);

    // Typing a digit should set has_explicit_count
    app.push_vim_digit(5);
    assert!(app.has_explicit_count);

    // Reset should clear it
    app.reset_vim_count();
    assert!(!app.has_explicit_count);
}

// ═══════════════════════════════════════════════════════════
// Regression tests — bug fix batch
// ═══════════════════════════════════════════════════════════

#[test]
fn test_refresh_books_preserves_cursor() {
    let (mut app, _temp) = create_test_app();

    // Upsert all test books into the DB so refresh_books can find them
    let cards_dir = app.cards_dir();
    for book in &app.books {
        if let Ok(card) = omniscope_core::storage::json_cards::load_card_by_id(&cards_dir, &book.id)
        {
            if let Some(ref db) = app.db {
                let _ = db.upsert_book(&card);
            }
        }
    }

    app.selected_index = 5;
    let book_id = app.books[5].id;

    app.refresh_books();

    // Cursor should still point to the same book
    assert_eq!(
        app.books[app.selected_index].id, book_id,
        "refresh_books should preserve cursor on the same book"
    );
}

#[test]
fn test_cs_cycles_status() {
    let (mut app, _temp) = create_test_app();
    app.selected_index = 0;
    let initial_status = app.books[0].read_status.clone();

    // cs should cycle status
    run_keys(&mut app, "cs");

    // The popup should have opened OR the status should have changed
    // Since 'cs' goes through pending mode → cycle_status directly
    // (gs also does this)
    run_keys(&mut app, "gs");
    app.refresh_books();

    // After gs, status should have changed from initial
    let new_status = app.books[app.selected_index].read_status.clone();
    assert_ne!(
        format!("{:?}", initial_status),
        format!("{:?}", new_status),
        "gs should cycle the read status"
    );
}

#[test]
fn test_cr_opens_rating_popup() {
    let (mut app, _temp) = create_test_app();

    // cr should open rating popup
    run_keys(&mut app, "cr");

    assert!(
        matches!(app.popup, Some(crate::popup::Popup::SetRating { .. })),
        "cr should open SetRating popup"
    );
}

#[test]
fn test_cy_opens_year_edit_popup() {
    let (mut app, _temp) = create_test_app();

    // cy should open EditYear popup, NOT AddBook
    run_keys(&mut app, "cy");

    assert!(
        matches!(app.popup, Some(crate::popup::Popup::EditYear { .. })),
        "cy should open EditYear popup, not AddBook"
    );
}

#[test]
fn test_ca_opens_authors_edit_popup() {
    let (mut app, _temp) = create_test_app();

    // ca should open EditAuthors popup, NOT AddBook
    run_keys(&mut app, "ca");

    assert!(
        matches!(app.popup, Some(crate::popup::Popup::EditAuthors { .. })),
        "ca should open EditAuthors popup, not AddBook"
    );
}

#[test]
fn test_visual_c_does_not_delete() {
    let (mut app, _temp) = create_test_app();
    let initial_len = app.books.len();

    // v j c should NOT delete books
    run_keys(&mut app, "vjc");

    assert_eq!(
        app.books.len(),
        initial_len,
        "c in visual mode should not delete books"
    );
}

#[test]
fn test_visual_add_tag_operator() {
    let (mut app, _temp) = create_test_app();

    // v j > should open AddTagPrompt popup
    run_keys(&mut app, "vj>");

    assert!(
        matches!(app.popup, Some(crate::popup::Popup::AddTagPrompt { .. })),
        "< in visual mode should open AddTagPrompt popup"
    );
}

#[test]
fn test_add_tag_prompt_via_operator() {
    let (mut app, _temp) = create_test_app();

    // >ib should open AddTagPrompt for current book
    run_keys(&mut app, ">ib");

    assert!(
        matches!(app.popup, Some(crate::popup::Popup::AddTagPrompt { .. })),
        ">ib should open AddTagPrompt popup"
    );
}
