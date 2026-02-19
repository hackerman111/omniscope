use omniscope_core::{BookSummaryView, ReadStatus};

/// Popup dialog types.
#[derive(Debug)]
pub enum Popup {
    /// Add a new book — form with title, author, year, tags.
    AddBook(AddBookForm),
    /// Confirm deletion of a book.
    DeleteConfirm { title: String, id: String },
    /// Set rating (1-5).
    SetRating { id: String, current: Option<u8> },
    /// Set read status.
    SetStatus { id: String, current: ReadStatus },
    /// Edit tags.
    EditTags(EditTagsForm),
    /// Telescope-like search overlay (full-screen).
    Telescope(TelescopeState),
    /// Help screen.
    Help,
    /// EasyMotion overlay.
    EasyMotion(EasyMotionState),
}

/// Telescope internal mode (vim-like).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TelescopeMode {
    /// Typing a query: characters are inserted, autocomplete is live.
    Insert,
    /// Navigating results with j/k/gg/G.
    Normal,
}

/// EasyMotion state matching visible characters to book indices.
#[derive(Debug, Clone)]
pub struct EasyMotionState {
    pub pending: bool,
    pub targets: Vec<(char, usize)>, // letter -> index in app.books
}

/// Smart autocomplete state — filters candidates as-you-type,
/// supports Tab cycling, arrow selection, and Enter confirmation.
#[derive(Debug, Clone, Default)]
pub struct AutocompleteState {
    /// All available candidates (tags, authors, etc.)
    pub all_candidates: Vec<String>,
    /// Filtered subset (prefix matches current token).
    pub visible: Vec<String>,
    /// Index into `visible` currently highlighted.
    pub selected: Option<usize>,
    /// Whether the dropdown is shown.
    pub active: bool,
}

impl AutocompleteState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load candidates and filter immediately.
    pub fn set_candidates(&mut self, candidates: Vec<String>, prefix: &str) {
        self.all_candidates = candidates;
        self.filter(prefix);
    }

    /// Refilter `visible` based on current prefix.
    pub fn filter(&mut self, prefix: &str) {
        let prefix_lower = prefix.to_lowercase();
        self.visible = self.all_candidates.iter()
            .filter(|c| c.to_lowercase().contains(&prefix_lower))
            .cloned()
            .take(8)
            .collect();

        self.active = !self.visible.is_empty();
        // Reset selection
        self.selected = if self.active { Some(0) } else { None };
    }

    /// Move selection down (wraps).
    pub fn move_down(&mut self) {
        if !self.visible.is_empty() {
            self.selected = Some(match self.selected {
                Some(i) => (i + 1) % self.visible.len(),
                None => 0,
            });
        }
    }

    /// Move selection up (wraps).
    pub fn move_up(&mut self) {
        if !self.visible.is_empty() {
            self.selected = Some(match self.selected {
                Some(0) | None => self.visible.len() - 1,
                Some(i) => i - 1,
            });
        }
    }

    /// Tab: cycle to next item.
    pub fn tab_next(&mut self) {
        self.move_down();
    }

    /// Return currently highlighted candidate (if any).
    pub fn current(&self) -> Option<&str> {
        self.selected.and_then(|i| self.visible.get(i)).map(|s| s.as_str())
    }

    pub fn clear(&mut self) {
        self.visible.clear();
        self.selected = None;
        self.active = false;
    }

    pub fn activate(&mut self, candidates: Vec<String>) {
        self.visible = candidates;
        self.active = !self.visible.is_empty();
        self.selected = if self.active { Some(0) } else { None };
    }
}

/// State for the Telescope search overlay.
#[derive(Debug)]
pub struct TelescopeState {
    // ── Mode ─────────────────────────────────────
    pub mode: TelescopeMode,

    // ── Query input ──────────────────────────────
    /// Full query string (may include DSL tokens).
    pub query: String,
    /// Byte-level cursor position in `query`.
    pub cursor: usize,

    // ── Results ──────────────────────────────────
    /// Filtered results (fuzzy + DSL).
    pub results: Vec<BookSummaryView>,
    /// Selected result index.
    pub selected: usize,
    /// Scroll offset for virtual list.
    pub scroll: usize,

    // ── Filter chips ─────────────────────────────
    /// DSL tokens extracted from query, shown as chips.
    pub active_filters: Vec<String>,

    // ── Autocomplete ─────────────────────────────
    pub autocomplete: AutocompleteState,

    // ── Pending gg ───────────────────────────────
    pub pending_g: bool,
}

impl TelescopeState {
    pub fn new() -> Self {
        Self {
            mode: TelescopeMode::Insert,
            query: String::new(),
            cursor: 0,
            results: Vec::new(),
            selected: 0,
            scroll: 0,
            active_filters: Vec::new(),
            autocomplete: AutocompleteState::new(),
            pending_g: false,
        }
    }

    // ── Result navigation ──────────────────────────

    pub fn result_down(&mut self, visible_height: usize) {
        if !self.results.is_empty() && self.selected < self.results.len() - 1 {
            self.selected += 1;
            // Scroll if necessary
            if self.selected >= self.scroll + visible_height {
                self.scroll = self.selected.saturating_sub(visible_height - 1);
            }
        }
    }

    pub fn result_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            if self.selected < self.scroll {
                self.scroll = self.selected;
            }
        }
    }

    pub fn result_top(&mut self) {
        self.selected = 0;
        self.scroll = 0;
    }

    pub fn result_bottom(&mut self) {
        if !self.results.is_empty() {
            self.selected = self.results.len() - 1;
        }
    }

    pub fn half_page_down(&mut self, visible_height: usize) {
        let step = (visible_height / 2).max(1);
        for _ in 0..step {
            if self.selected + 1 < self.results.len() {
                self.selected += 1;
            }
        }
        if self.selected >= self.scroll + visible_height {
            self.scroll = self.selected.saturating_sub(visible_height - 1);
        }
    }

    pub fn half_page_up(&mut self, visible_height: usize) {
        let step = (visible_height / 2).max(1);
        self.selected = self.selected.saturating_sub(step);
        if self.selected < self.scroll {
            self.scroll = self.selected;
        }
    }

    pub fn selected_result(&self) -> Option<&BookSummaryView> {
        self.results.get(self.selected)
    }

    // ── Query editing (Insert mode) ────────────────

    pub fn insert_char(&mut self, c: char) {
        self.query.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }

    pub fn delete_back(&mut self) {
        if self.cursor > 0 {
            let prev = self.query[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.query.remove(prev);
            self.cursor = prev;
        }
    }

    pub fn delete_word_back(&mut self) {
        // Delete back to previous whitespace
        while self.cursor > 0 {
            let prev_char = self.query[..self.cursor]
                .chars().last().unwrap_or(' ');
            self.delete_back();
            if prev_char == ' ' {
                break;
            }
        }
    }

    pub fn cursor_left(&mut self) {
        if self.cursor > 0 {
            self.cursor = self.query[..self.cursor]
                .char_indices().last().map(|(i, _)| i).unwrap_or(0);
        }
    }

    pub fn cursor_right(&mut self) {
        if self.cursor < self.query.len() {
            self.cursor = self.query[self.cursor..]
                .char_indices().nth(1).map(|(i, _)| self.cursor + i)
                .unwrap_or(self.query.len());
        }
    }

    pub fn cursor_word_forward(&mut self) {
        // Skip spaces, then skip word
        let s = &self.query[self.cursor..];
        let skip_space: usize = s.chars().take_while(|c| *c == ' ').map(|c| c.len_utf8()).sum();
        let after_space = self.cursor + skip_space;
        let word_end: usize = self.query[after_space..].chars()
            .take_while(|c| *c != ' ').map(|c| c.len_utf8()).sum();
        self.cursor = (after_space + word_end).min(self.query.len());
    }

    pub fn cursor_word_back(&mut self) {
        if self.cursor == 0 { return; }
        let before = &self.query[..self.cursor];
        let skip_space: usize = before.chars().rev().take_while(|c| *c == ' ').map(|c| c.len_utf8()).sum();
        let at = self.cursor - skip_space;
        let word_len: usize = self.query[..at].chars().rev()
            .take_while(|c| *c != ' ').map(|c| c.len_utf8()).sum();
        self.cursor = at - word_len;
    }

    pub fn cursor_home(&mut self) { self.cursor = 0; }
    pub fn cursor_end(&mut self) { self.cursor = self.query.len(); }

    /// Current DSL token being typed (last whitespace-delimited word).
    pub fn current_token(&self) -> &str {
        self.query[..self.cursor].rsplit_once(' ')
            .map(|(_, w)| w).unwrap_or(&self.query[..self.cursor])
    }

    /// Accept the current autocomplete candidate — replaces the current token.
    pub fn accept_autocomplete(&mut self, candidate: &str) {
        // Extract DSL key prefix from candidate (e.g. "@author:Steve Klabnik" → "@author:Steve Klabnik")
        // Just replace the current incomplete token with the candidate
        let token_start = self.query[..self.cursor].rfind(' ')
            .map(|i| i + 1).unwrap_or(0);
        self.query.replace_range(token_start..self.cursor, candidate);
        self.cursor = token_start + candidate.len();
        // Add trailing space so user can type next token
        if !self.query[self.cursor..].starts_with(' ') {
            self.query.insert(self.cursor, ' ');
        }
        self.autocomplete.clear();
    }
}

/// Form for adding a new book.
#[derive(Debug)]
pub struct AddBookForm {
    pub fields: Vec<FormField>,
    pub active_field: usize,
    pub autocomplete: AutocompleteState,
}

/// Form for editing tags.
#[derive(Debug)]
pub struct EditTagsForm {
    pub book_id: String,
    pub tags: Vec<String>,
    pub input: String,
    pub cursor: usize,
}

/// A single form field with label and text input.
#[derive(Debug)]
pub struct FormField {
    pub label: String,
    pub value: String,
    pub cursor: usize,
}

impl FormField {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: String::new(),
            cursor: 0,
        }
    }

    pub fn with_value(label: impl Into<String>, value: impl Into<String>) -> Self {
        let value = value.into();
        let cursor = value.len();
        Self {
            label: label.into(),
            value,
            cursor,
        }
    }

    pub fn insert_char(&mut self, c: char) {
        self.value.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }

    pub fn delete_back(&mut self) {
        if self.cursor > 0 {
            let prev = self.value[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.value.remove(prev);
            self.cursor = prev;
        }
    }

    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor = self.value[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    pub fn move_right(&mut self) {
        if self.cursor < self.value.len() {
            self.cursor = self.value[self.cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor + i)
                .unwrap_or(self.value.len());
        }
    }
}

impl AddBookForm {
    pub fn new() -> Self {
        Self {
            fields: vec![
                FormField::new("Title"),
                FormField::new("Author(s)"),
                FormField::new("Year"),
                FormField::new("Tags"),
                FormField::new("Library"),
                FormField::new("File path"),
            ],
            active_field: 0,
            autocomplete: AutocompleteState::new(),
        }
    }

    pub fn active_field_mut(&mut self) -> &mut FormField {
        &mut self.fields[self.active_field]
    }

    pub fn next_field(&mut self) {
        if self.active_field < self.fields.len() - 1 {
            self.active_field += 1;
        }
    }

    pub fn prev_field(&mut self) {
        if self.active_field > 0 {
            self.active_field -= 1;
        }
    }
}

impl EditTagsForm {
    pub fn new(book_id: String, tags: Vec<String>) -> Self {
        Self {
            book_id,
            tags,
            input: String::new(),
            cursor: 0,
        }
    }

    pub fn add_tag(&mut self) {
        let tag = self.input.trim().to_string();
        if !tag.is_empty() && !self.tags.contains(&tag) {
            self.tags.push(tag);
        }
        self.input.clear();
        self.cursor = 0;
    }

    pub fn remove_last_tag(&mut self) {
        if self.input.is_empty() {
            self.tags.pop();
        }
    }
}
