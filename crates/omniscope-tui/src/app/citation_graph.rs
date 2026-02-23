use omniscope_core::BookCard;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphMode {
    References,
    CitedBy,
    Related,
}

impl GraphMode {
    pub fn next(&self) -> Self {
        match self {
            Self::References => Self::CitedBy,
            Self::CitedBy => Self::Related,
            Self::Related => Self::References,
        }
    }
    
    pub fn prev(&self) -> Self {
        match self {
            Self::References => Self::Related,
            Self::CitedBy => Self::References,
            Self::Related => Self::CitedBy,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CitationEdge {
    pub title: String,
    pub year: Option<i32>,
    pub id: Option<String>,
    pub id_type: Option<String>,
    pub in_library: bool,
}

pub struct CitationGraphPanelState {
    pub book: BookCard,
    pub mode: GraphMode,
    pub references: Vec<CitationEdge>,
    pub cited_by: Vec<CitationEdge>,
    pub related: Vec<CitationEdge>,
    pub cursor: usize,
    pub is_loading: bool,
}

impl CitationGraphPanelState {
    pub fn new(book: BookCard) -> Self {
        Self {
            book,
            references: Vec::new(),
            cited_by: Vec::new(),
            related: Vec::new(),
            mode: GraphMode::References,
            cursor: 0,
            is_loading: true, // Start in loading state
        }
    }

    pub fn handle_loaded(&mut self, res: Result<Vec<CitationEdge>, String>) {
        self.is_loading = false;
        match res {
            Ok(edges) => {
                // For now, depending on the actual API we might sort these into the right buckets.
                // Since this is a generic load, let's just push everything to references for the mockup.
                self.references = edges;
                // If the real API returns them pre-split, we'd assign them to references, cited_by, related.
            }
            Err(e) => {
                // Set some error state, or append a dummy edge for the error message
                self.references = vec![CitationEdge {
                    title: format!("Error: {}", e),
                    year: None,
                    id: None,
                    id_type: None,
                    in_library: false,
                }];
            }
        }
        self.cursor = 0;
    }

    pub fn visible_items(&self) -> &[CitationEdge] {
        match self.mode {
            GraphMode::References => &self.references,
            GraphMode::CitedBy => &self.cited_by,
            GraphMode::Related => &self.related,
        }
    }
    
    pub fn visible_len(&self) -> usize {
        self.visible_items().len()
    }
    
    pub fn move_down(&mut self) {
        let len = self.visible_len();
        if len == 0 { return; }
        if self.cursor >= len - 1 {
            self.cursor = 0;
        } else {
            self.cursor += 1;
        }
    }
    
    pub fn move_up(&mut self) {
        let len = self.visible_len();
        if len == 0 { return; }
        if self.cursor == 0 {
            self.cursor = len - 1;
        } else {
            self.cursor -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omniscope_core::BookCard;
    
    fn create_mock_book() -> BookCard {
        BookCard::new("Test Book")
    }

    #[test]
    fn test_citation_graph_state() {
        let mut state = CitationGraphPanelState::new(create_mock_book());
        state.references.push(CitationEdge {
            title: "Ref 1".into(),
            year: None,
            id: None,
            id_type: None,
            in_library: false,
        });

        assert_eq!(state.mode, GraphMode::References);
        assert_eq!(state.visible_len(), 1);

        state.move_down();
        assert_eq!(state.cursor, 0); // Wraps around or stays at 0

        state.mode = state.mode.next();
        assert_eq!(state.mode, GraphMode::CitedBy);
        assert_eq!(state.visible_len(), 0);

        state.mode = state.mode.next();
        assert_eq!(state.mode, GraphMode::Related);

        state.mode = state.mode.next();
        assert_eq!(state.mode, GraphMode::References);
    }
}
