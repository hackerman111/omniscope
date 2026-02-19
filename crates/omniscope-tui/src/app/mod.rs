mod books;
mod navigation;
mod sidebar;
mod vim;

use omniscope_core::{AppConfig, BookCard, BookSummaryView, Database, FuzzySearcher};
use crate::popup::Popup;

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
    RatingDesc,
    FrecencyDesc,
}

impl SortKey {
    pub fn next(self) -> Self {
        match self {
            Self::UpdatedDesc  => Self::UpdatedAsc,
            Self::UpdatedAsc   => Self::TitleAsc,
            Self::TitleAsc     => Self::YearDesc,
            Self::YearDesc     => Self::RatingDesc,
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
            Self::RatingDesc   => "rating ▼",
            Self::FrecencyDesc => "frecency ▼",
        }
    }
}

/// An undoable book-modification snapshot.
#[derive(Debug, Clone)]
pub struct UndoEntry {
    pub description: String,
    pub card: BookCard,
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
    pub vim_operator: Option<char>,

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
            vim_operator: None,
            pending_register_select: false,
            vim_register: None,
            registers: std::collections::HashMap::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            marks: std::collections::HashMap::new(),
            yank_register: None,
            sort_key: SortKey::default(),
        };

        app.refresh_sidebar();
        app
    }

    /// Currently selected book (if any).
    pub fn selected_book(&self) -> Option<&BookSummaryView> {
        self.books.get(self.selected_index)
    }
}
