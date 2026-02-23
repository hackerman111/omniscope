use omniscope_science::references::resolver::{ExtractedReference, ResolutionMethod};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefsFilter {
    All,
    Resolved,
    Unresolved,
    InLibrary,
    NotInLibrary,
}

impl RefsFilter {
    pub fn next(&self) -> Self {
        match self {
            Self::All => Self::Resolved,
            Self::Resolved => Self::Unresolved,
            Self::Unresolved => Self::InLibrary,
            Self::InLibrary => Self::NotInLibrary,
            Self::NotInLibrary => Self::All,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Self::All => Self::NotInLibrary,
            Self::Resolved => Self::All,
            Self::Unresolved => Self::Resolved,
            Self::InLibrary => Self::Unresolved,
            Self::NotInLibrary => Self::InLibrary,
        }
    }
}

pub struct ReferencesPanelState {
    pub book_title: String,
    pub references: Vec<ExtractedReference>,
    pub filter: RefsFilter,
    pub selected: usize,
    pub search_query: String,
}

impl ReferencesPanelState {
    pub fn new(book_title: String, references: Vec<ExtractedReference>) -> Self {
        Self {
            book_title,
            references,
            filter: RefsFilter::All,
            selected: 0,
            search_query: String::new(),
        }
    }

    pub fn visible_references(&self) -> Vec<(usize, &ExtractedReference)> {
        self.references
            .iter()
            .enumerate()
            .filter(|(_, r)| self.matches_filter(r))
            .filter(|(_, r)| self.matches_search(r))
            .collect()
    }

    fn matches_filter(&self, r: &ExtractedReference) -> bool {
        match self.filter {
            RefsFilter::All => true,
            RefsFilter::Resolved => r.resolution_method != ResolutionMethod::Unresolved,
            RefsFilter::Unresolved => r.resolution_method == ResolutionMethod::Unresolved,
            RefsFilter::InLibrary => r.is_in_library.is_some(),
            RefsFilter::NotInLibrary => r.is_in_library.is_none(),
        }
    }

    fn matches_search(&self, r: &ExtractedReference) -> bool {
        if self.search_query.is_empty() {
            return true;
        }
        let query = self.search_query.to_lowercase();
        let text = r.raw_text.to_lowercase();
        let title = r.resolved_title.as_deref().unwrap_or("").to_lowercase();
        let authors = r.resolved_authors.join(" ").to_lowercase();
        
        text.contains(&query) || title.contains(&query) || authors.contains(&query)
    }

    pub fn move_down(&mut self) {
        let visible_count = self.visible_references().len();
        if visible_count == 0 {
            return;
        }
        if self.selected >= visible_count - 1 {
            self.selected = 0;
        } else {
            self.selected += 1;
        }
    }

    pub fn move_up(&mut self) {
        let visible_count = self.visible_references().len();
        if visible_count == 0 {
            return;
        }
        if self.selected == 0 {
            self.selected = visible_count - 1;
        } else {
            self.selected -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_ref(idx: usize, resolved: bool, in_library: bool) -> ExtractedReference {
        ExtractedReference {
            index: idx,
            raw_text: format!("Ref {}", idx),
            doi: None,
            arxiv_id: None,
            isbn: None,
            resolved_title: None,
            resolved_authors: Vec::new(),
            resolved_year: None,
            confidence: 0.8,
            resolution_method: if resolved { ResolutionMethod::DirectDoi } else { ResolutionMethod::Unresolved },
            is_in_library: if in_library { Some("abc".to_string()) } else { None },
        }
    }

    #[test]
    fn test_refs_filter() {
        let refs = vec![
            create_ref(1, false, false),
            create_ref(2, true, false),
            create_ref(3, true, true),
        ];

        let mut state = ReferencesPanelState::new("Book".to_string(), refs);
        
        state.filter = RefsFilter::All;
        assert_eq!(state.visible_references().len(), 3);
        
        state.filter = RefsFilter::Resolved;
        assert_eq!(state.visible_references().len(), 2);
        
        state.filter = RefsFilter::Unresolved;
        assert_eq!(state.visible_references().len(), 1);
        
        state.filter = RefsFilter::InLibrary;
        assert_eq!(state.visible_references().len(), 1);
        
        state.filter = RefsFilter::NotInLibrary;
        assert_eq!(state.visible_references().len(), 2);
    }
}
