mod books;
mod navigation;
mod sidebar;
mod vim;

use omniscope_core::{AppConfig, BookCard, BookSummaryView, Database, FuzzySearcher, undo::UndoEntry};
use crate::popup::Popup;
use crate::keys::operator::Operator;
use crate::keys::jump_list::JumpList;
use crate::keys::macro_recorder::MacroRecorder;
use crate::theme::NordTheme;

/// Search direction for `/` and `?` searches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchDirection {
    Forward,
    Backward,
}

/// Vim-like modes for the TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Search,
    Command,
    Visual,
    VisualLine,
    VisualBlock,
    Pending,
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Normal  => write!(f, "NORMAL"),
            Self::Insert  => write!(f, "INSERT"),
            Self::Search  => write!(f, "SEARCH"),
            Self::Command => write!(f, "COMMAND"),
            Self::Visual  => write!(f, "VISUAL"),
            Self::VisualLine => write!(f, "VISUAL-LINE"),
            Self::VisualBlock => write!(f, "VISUAL-BLOCK"),
            Self::Pending => write!(f, "PENDING"),
        }
    }
}

/// Content stored in a register.
#[derive(Debug, Clone)]
pub enum RegisterContent {
    Card(BookCard),
    Path(String),
    Text(String),
    MultipleCards(Vec<BookCard>),
}

/// A vim-style register.
#[derive(Debug, Clone)]
pub struct Register {
    pub content: RegisterContent,
    pub is_append: bool,
}

/// Which panel currently has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivePanel {
    Sidebar,
    BookList,
    Preview,
}

/// What the sidebar is filtering by.
#[derive(Debug, Clone)]
pub enum SidebarFilter {
    All,
    Library(String),
    Tag(String),
}

/// Sort order for the book list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortKey {
    #[default]
    UpdatedDesc,
    UpdatedAsc,
    TitleAsc,
    YearDesc,
    YearAsc,
    RatingDesc,
    FrecencyDesc,
}

impl SortKey {
    pub fn next(self) -> Self {
        match self {
            Self::UpdatedDesc  => Self::UpdatedAsc,
            Self::UpdatedAsc   => Self::TitleAsc,
            Self::TitleAsc     => Self::YearDesc,
            Self::YearDesc     => Self::YearAsc,
            Self::YearAsc      => Self::RatingDesc,
            Self::RatingDesc   => Self::FrecencyDesc,
            Self::FrecencyDesc => Self::UpdatedDesc,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::UpdatedDesc  => "updated ▼",
            Self::UpdatedAsc   => "updated ▲",
            Self::TitleAsc     => "title A–Z",
            Self::YearDesc     => "year ▼",
            Self::YearAsc      => "year ▲",
            Self::RatingDesc   => "rating ▼",
            Self::FrecencyDesc => "frecency ▼",
        }
    }
}



/// Items displayed in the sidebar.
#[derive(Debug, Clone)]
pub enum SidebarItem {
    AllBooks { count: u32 },
    Library { name: String, count: u32 },
    TagHeader,
    Tag { name: String, count: u32 },
    FolderHeader,
    Folder { path: String },
}

/// Main application state.
pub struct App {
    pub should_quit: bool,
    pub mode: Mode,
    pub active_panel: ActivePanel,

    /// Books currently displayed in the list.
    pub books: Vec<BookSummaryView>,
    /// All books (unfiltered, for fuzzy search).
    pub all_books: Vec<BookSummaryView>,

    /// Currently selected book index in the list.
    pub selected_index: usize,

    /// Sidebar items (libraries, tags, folders).
    pub sidebar_items: Vec<SidebarItem>,
    pub sidebar_selected: usize,
    pub sidebar_filter: SidebarFilter,

    /// Status bar message.
    pub status_message: String,

    /// Command line input buffer (for `:` mode).
    pub command_input: String,

    /// Search input buffer (for `/` mode).
    pub search_input: String,

    /// Active popup (if any).
    pub popup: Option<Popup>,

    /// Pending key for multi-key sequences (e.g. `gg`).
    pub pending_key: Option<char>,

    /// Visual mode selection indices.
    pub visual_selections: Vec<usize>,
    /// Index where visual selection started.
    pub visual_anchor: Option<usize>,

    /// Fuzzy searcher instance.
    pub fuzzy_searcher: FuzzySearcher,

    /// Database handle.
    pub db: Option<Database>,

    /// App configuration.
    pub config: AppConfig,

    // ─── Phase 1: Vim extras ────────────────────────────────

    /// Accumulated numeric count prefix for motions (e.g. `5j`).
    pub vim_count: u32,

    /// Pending operator (e.g. `d`, `y`, `c`) waiting for a motion.
    pub pending_operator: Option<Operator>,

    /// Jump list for navigation history.
    pub jump_list: JumpList,

    /// Pending register selection (e.g. `"` waiting for char).
    pub pending_register_select: bool,
    /// Selected register for the next operation (default is unnamed `"`).
    pub vim_register: Option<char>,
    /// Registers storage.
    pub registers: std::collections::HashMap<char, Register>,

    /// Undo stack.
    pub undo_stack: Vec<UndoEntry>,

    /// Redo stack.
    pub redo_stack: Vec<UndoEntry>,

    /// Named marks: single-char key → book-list index.
    pub marks: std::collections::HashMap<char, usize>,

    /// Yank register: last-yanked BookSummaryView (legacy/default).
    pub yank_register: Option<BookSummaryView>,

    /// Current sort order for the book list.
    pub sort_key: SortKey,

    // ─── Phase 1: Quickfix ──────────────────────────────────

    /// Quickfix list of books for batch operations.
    pub quickfix_list: Vec<BookSummaryView>,
    /// Whether the quickfix panel is currently visible.
    pub quickfix_show: bool,
    /// Currently selected index in the quickfix list.
    pub quickfix_selected: usize,

    /// Last find-char motion (char we looked for, the motion key used f/F/t/T)
    pub last_find_char: Option<(char, char)>,

    // ─── Phase 2: Viewport & Navigation ─────────────────────

    /// Manual viewport scroll offset (for zz/zt/zb and Ctrl+e/y).
    pub viewport_offset: usize,

    /// Last position before a jump (for `''` mark).
    pub last_jump_pos: Option<usize>,

    /// Last visual selection range (start, end) for `gv` and `'<`/`'>`.
    pub last_visual_range: Option<(usize, usize)>,

    // ─── Phase 2: Search ────────────────────────────────────

    /// Last search query for `n`/`N` repeat.
    pub last_search: Option<String>,

    /// Current search direction.
    pub search_direction: SearchDirection,

    // ─── Phase 2: Command History ───────────────────────────

    /// History of executed commands.
    pub command_history: Vec<String>,

    /// Current position in command history navigation.
    pub command_history_idx: Option<usize>,

    // ─── Phase 2: Macros ────────────────────────────────────

    /// Macro recorder for q/@ commands.
    pub macro_recorder: MacroRecorder,

    // ─── Phase 1: AI Panel ──────────────────────────────────
    pub ai_panel_active: bool,
    pub ai_input: String,

    /// UI Theme
    pub theme: NordTheme,

    /// Persistent clipboard instance to avoid "dropped too fast" warnings.
    pub clipboard: Option<arboard::Clipboard>,
}

impl App {
    /// Create a new App with default state.
    pub fn new(config: AppConfig) -> Self {
        // Try to open the database
        let db_path = config.database_path();
        let db = if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok();
            Database::open(&db_path).ok()
        } else {
            None
        };

        // Sync JSON cards into SQLite
        if let Some(ref db) = db {
            let cards_dir = config.cards_dir();
            if cards_dir.exists() {
                let _ = db.sync_from_cards(&cards_dir);
            }
        }

        // Load initial book list
        let all_books = db
            .as_ref()
            .and_then(|db| db.list_books(500, 0).ok())
            .unwrap_or_default();

        let books = all_books.clone();

        let mut app = Self {
            should_quit: false,
            mode: Mode::Normal,
            active_panel: ActivePanel::BookList,
            books,
            all_books,
            selected_index: 0,
            sidebar_items: Vec::new(),
            sidebar_selected: 0,
            sidebar_filter: SidebarFilter::All,
            status_message: String::new(),
            command_input: String::new(),
            search_input: String::new(),
            popup: None,
            pending_key: None,
            visual_selections: Vec::new(),
            visual_anchor: None,
            fuzzy_searcher: FuzzySearcher::new(),
            db,
            config,
            // Phase 1 fields
            vim_count: 0,
            pending_operator: None,
            jump_list: JumpList::new(),
            pending_register_select: false,
            vim_register: None,
            registers: std::collections::HashMap::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            marks: std::collections::HashMap::new(),
            yank_register: None,
            sort_key: SortKey::default(),
            quickfix_list: Vec::new(),
            quickfix_show: false,
            quickfix_selected: 0,
            last_find_char: None,
            viewport_offset: 0,
            last_jump_pos: None,
            last_visual_range: None,
            last_search: None,
            search_direction: SearchDirection::Forward,
            command_history: Vec::new(),
            command_history_idx: None,
            macro_recorder: MacroRecorder::new(),
            ai_panel_active: false,
            ai_input: String::new(),
            theme: NordTheme::default(),
            clipboard: arboard::Clipboard::new().ok(),
        };

        app.refresh_sidebar();
        app
    }

    /// Currently selected book (if any).
    pub fn selected_book(&self) -> Option<&BookSummaryView> {
        self.books.get(self.selected_index)
    }
}
