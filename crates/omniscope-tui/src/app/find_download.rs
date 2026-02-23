#[derive(Debug, Clone)]
pub struct SearchResultItem {
    pub title: String,
    pub authors: String,
    pub year: Option<i32>,
    pub source: String,
    pub format_or_metrics: String,
    pub in_library: bool,
    pub download_available: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchColumn {
    Left,  // Anna's Archive + Sci-Hub
    Right, // Semantic Scholar + OpenAlex
}

pub struct FindDownloadPanelState {
    pub query: String,
    pub active_column: SearchColumn,
    
    pub left_results: Vec<SearchResultItem>,
    pub right_results: Vec<SearchResultItem>,
    
    pub left_cursor: usize,
    pub right_cursor: usize,

    pub left_loading: bool,
    pub right_loading: bool,
}

impl FindDownloadPanelState {
    pub fn new(query: String) -> Self {
        Self {
            query,
            active_column: SearchColumn::Left,
            left_results: Vec::new(),
            right_results: Vec::new(),
            left_cursor: 0,
            right_cursor: 0,
            left_loading: true, // Will be set to false when async fetch completes
            right_loading: true,
        }
    }

    pub fn handle_loaded(&mut self, column: SearchColumn, res: Result<Vec<SearchResultItem>, String>) {
        match column {
            SearchColumn::Left => {
                self.left_loading = false;
                match res {
                    Ok(items) => self.left_results = items,
                    Err(e) => self.left_results = vec![SearchResultItem {
                        title: format!("Error: {}", e),
                        authors: "".into(),
                        year: None,
                        source: "Error".into(),
                        format_or_metrics: "".into(),
                        in_library: false,
                        download_available: false,
                    }],
                }
                self.left_cursor = 0;
            }
            SearchColumn::Right => {
                self.right_loading = false;
                match res {
                    Ok(items) => self.right_results = items,
                    Err(e) => self.right_results = vec![SearchResultItem {
                        title: format!("Error: {}", e),
                        authors: "".into(),
                        year: None,
                        source: "Error".into(),
                        format_or_metrics: "".into(),
                        in_library: false,
                        download_available: false,
                    }],
                }
                self.right_cursor = 0;
            }
        }
    }
    
    pub fn toggle_column(&mut self) {
        self.active_column = match self.active_column {
            SearchColumn::Left => SearchColumn::Right,
            SearchColumn::Right => SearchColumn::Left,
        };
    }
    
    pub fn move_down(&mut self) {
        match self.active_column {
            SearchColumn::Left => {
                if !self.left_results.is_empty() {
                    self.left_cursor = (self.left_cursor + 1) % self.left_results.len();
                }
            }
            SearchColumn::Right => {
                if !self.right_results.is_empty() {
                    self.right_cursor = (self.right_cursor + 1) % self.right_results.len();
                }
            }
        }
    }
    
    pub fn move_up(&mut self) {
        match self.active_column {
            SearchColumn::Left => {
                if !self.left_results.is_empty() {
                    if self.left_cursor == 0 {
                        self.left_cursor = self.left_results.len() - 1;
                    } else {
                        self.left_cursor -= 1;
                    }
                }
            }
            SearchColumn::Right => {
                if !self.right_results.is_empty() {
                    if self.right_cursor == 0 {
                        self.right_cursor = self.right_results.len() - 1;
                    } else {
                        self.right_cursor -= 1;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_download_state() {
        let mut state = FindDownloadPanelState::new("query".into());
        state.left_results.push(SearchResultItem {
            title: "Left 1".into(),
            authors: "Auth".into(),
            year: None,
            source: "src".into(),
            format_or_metrics: "fmt".into(),
            in_library: false,
            download_available: true,
        });

        assert_eq!(state.active_column, SearchColumn::Left);
        
        state.move_down();
        assert_eq!(state.left_cursor, 0);

        state.toggle_column();
        assert_eq!(state.active_column, SearchColumn::Right);

        state.move_down(); // Shouldn't change anything as right is empty
        assert_eq!(state.right_cursor, 0);
    }
}
