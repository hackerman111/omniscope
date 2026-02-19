#[cfg(test)]
mod tests {
    use crate::app::{App, SidebarItem, SidebarFilter};
    use omniscope_core::{AppConfig, BookSummaryView};
    use crate::keys::{motions, text_objects};
    use crate::keys::text_objects::TextObjectKind;

    fn create_mock_app(count: usize) -> App {
        let config = AppConfig::default();
        
        let mut books = Vec::new();
        for i in 0..count {
            books.push(BookSummaryView {
                id: uuid::Uuid::nil(), 
                title: format!("Book {}", i),
                authors: vec![if i % 2 == 0 { "AuthorA".to_string() } else { "AuthorB".to_string() }],
                year: Some(2020 + (i as i32 % 5)),
                format: None,
                rating: None,
                read_status: omniscope_core::ReadStatus::Unread,
                tags: Vec::new(),
                has_file: false,
                frecency_score: 0.0,
            });
        }

        let mut app = App::new(config);
        app.books = books;
        app.selected_index = 0;
        app
    }

    #[test]
    fn test_motion_j() {
        let mut app = create_mock_app(10);
        app.selected_index = 0;
        
        // j (count 1)
        let range = motions::get_motion_range(&app, 'j', 1).unwrap();
        assert_eq!(range, vec![0, 1]);
        
        // 2j from 0 => 0, 1, 2
        let range = motions::get_motion_range(&app, 'j', 2).unwrap();
        assert_eq!(range, vec![0, 1, 2]);
        
        // j from bottom
        app.selected_index = 9;
        let range = motions::get_motion_range(&app, 'j', 1).unwrap();
        assert_eq!(range, vec![9]); // clamped to max
    }

    #[test]
    fn test_motion_k() {
        let mut app = create_mock_app(10);
        app.selected_index = 5;
        
        // k (count 1) => 5, 4 (range 4..=5)
        let range = motions::get_motion_range(&app, 'k', 1).unwrap();
        assert_eq!(range, vec![4, 5]);
        
        // 2k from 5 => 5, 4, 3 (range 3..=5)
        let range = motions::get_motion_range(&app, 'k', 2).unwrap();
        assert_eq!(range, vec![3, 4, 5]);
    }

    #[test]
    fn test_motion_G() {
        let mut app = create_mock_app(10);
        app.selected_index = 0;
        
        // G (default count 1 -> processed as explicit if > 1, but we pass raw count)
        // logic in simple motions: count=1 -> max_idx?
        // Wait, logic was: if count <= 1 { max_idx } else { count - 1 }
        
        // G (count 1) -> go to bottom (9)
        let range = motions::get_motion_range(&app, 'G', 1).unwrap();
        assert_eq!(range, (0..=9).collect::<Vec<_>>());
        
        // 5G -> go to index 4 (line 5)
        let range = motions::get_motion_range(&app, 'G', 5).unwrap();
        assert_eq!(range, (0..=4).collect::<Vec<_>>());
    }
    


    #[test]
    fn test_text_object_a() {
        let mut app = create_mock_app(10);
        // Even: AuthorA, Odd: AuthorB
        app.selected_index = 0; // AuthorA
        
        let range = text_objects::get_text_object_range(&app, 'a', TextObjectKind::Inner).unwrap();
        // Should be 0, 2, 4, 6, 8
        assert_eq!(range, vec![0, 2, 4, 6, 8]);
    }
}
