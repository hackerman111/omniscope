mod books;
mod navigation;
mod sidebar;
mod vim;

use crate::keys::core::operator::Operator;
use crate::keys::ext::jump_list::JumpList;
use crate::keys::ui::macro_recorder::MacroRecorder;
use crate::popup::Popup;
use crate::theme::NordTheme;
use omniscope_core::{
    models::{Folder, FolderTree},
    sync::{LibraryWatcher, WatcherEvent},
    undo::UndoEntry,
    AppConfig, BookCard, BookSummaryView, Database, FuzzySearcher, LibraryRoot,
};

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
            Self::Normal => write!(f, "NORMAL"),
            Self::Insert => write!(f, "INSERT"),
            Self::Search => write!(f, "SEARCH"),
            Self::Command => write!(f, "COMMAND"),
            Self::Visual => write!(f, "VISUAL"),
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
    Sync,
}

/// Left panel view mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeftPanelMode {
    LibraryView,
    FolderTree,
}

/// Center panel view mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CenterPanelMode {
    BookList,
    FolderView,
}

/// Sort parameter for FolderView mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FolderViewSort {
    #[default]
    FoldersFirst,
    Mixed,
    BooksFirst,
}

impl FolderViewSort {
    pub fn next(self) -> Self {
        match self {
            Self::FoldersFirst => Self::Mixed,
            Self::Mixed => Self::BooksFirst,
            Self::BooksFirst => Self::FoldersFirst,
        }
    }
}

/// Center panel mixed list items.
#[derive(Debug, Clone)]
pub enum CenterItem {
    Folder(Folder),
    Book(BookSummaryView),
}

/// What the sidebar is filtering by.
#[derive(Debug, Clone)]
pub enum SidebarFilter {
    All,
    Library(String),
    Tag(String),
    Folder(String),
    VirtualFolder(String),
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
            Self::UpdatedDesc => Self::UpdatedAsc,
            Self::UpdatedAsc => Self::TitleAsc,
            Self::TitleAsc => Self::YearDesc,
            Self::YearDesc => Self::YearAsc,
            Self::YearAsc => Self::RatingDesc,
            Self::RatingDesc => Self::FrecencyDesc,
            Self::FrecencyDesc => Self::UpdatedDesc,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::UpdatedDesc => "updated ▼",
            Self::UpdatedAsc => "updated ▲",
            Self::TitleAsc => "title A–Z",
            Self::YearDesc => "year ▼",
            Self::YearAsc => "year ▲",
            Self::RatingDesc => "rating ▼",
            Self::FrecencyDesc => "frecency ▼",
        }
    }
}

/// Items displayed in the sidebar.
#[derive(Debug, Clone)]
pub enum SidebarItem {
    AllBooks {
        count: u32,
    },
    Library {
        name: String,
        count: u32,
    },
    TagHeader,
    Tag {
        name: String,
        count: u32,
    },
    FolderHeader,
    VirtualFolder {
        id: String,
        name: String,
        count: u32,
    },
    FolderNode {
        id: String,
        name: String,
        depth: usize,
        is_expanded: bool,
        has_children: bool,
        ghost_count: u32,
        disk_path: String,
    },
}

/// Main application state.
pub struct App {
    pub should_quit: bool,
    pub mode: Mode,
    pub active_panel: ActivePanel,
    pub left_panel_mode: LeftPanelMode,
    pub center_panel_mode: CenterPanelMode,

    /// Books currently displayed in the list.
    pub books: Vec<BookSummaryView>,
    /// All books (unfiltered, for fuzzy search).
    pub all_books: Vec<BookSummaryView>,

    /// Items for FolderView
    pub center_items: Vec<CenterItem>,
    pub current_folder: Option<String>,
    pub folder_view_sort: FolderViewSort,

    pub sync_report: Option<omniscope_core::sync::folder_sync::SyncReport>,
    pub detached_books: Vec<omniscope_core::BookSummaryView>,
    pub sync_selected: usize,

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

    /// Discovered library root (`.libr/`-based), if any.
    pub library_root: Option<LibraryRoot>,

    /// Folder tree memory snapshot for hierarchical rendering in sidebar
    pub folder_tree: Option<FolderTree>,

    /// Track which folder IDs are expanded in the UI
    pub expanded_folders: std::collections::HashSet<String>,

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

    /// Whether the user typed an explicit digit count (to distinguish `G` from `1G`).
    pub has_explicit_count: bool,

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

    /// Command autocomplete suggestions.
    pub command_suggestions: Vec<String>,

    /// Currently selected suggestion index.
    pub command_suggestion_idx: Option<usize>,

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

    /// Path to open in $EDITOR (requires terminal suspension, handled by main loop).
    pub pending_editor_path: Option<String>,

    /// Filesystem watcher, keeps thread alive.
    pub library_watcher: Option<LibraryWatcher>,

    /// Channel receiver for filesystem events.
    pub watcher_rx: Option<std::sync::mpsc::Receiver<WatcherEvent>>,
}

impl App {
    /// Create a new App with default state.
    pub fn new(config: AppConfig, library_root: Option<LibraryRoot>) -> Self {
        // Derive DB and cards paths from library root if available, else use config
        let (db_path, cards_dir) = if let Some(ref lr) = library_root {
            (lr.database_path(), lr.cards_dir())
        } else {
            (config.database_path(), config.cards_dir())
        };

        // Try to open the database
        let db = if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok();
            Database::open(&db_path).ok()
        } else {
            None
        };

        // Sync JSON cards into SQLite
        if let Some(ref db) = db {
            if cards_dir.exists() {
                let _ = db.sync_from_cards(&cards_dir);
            }
        }

        let folder_tree = db
            .as_ref()
            .and_then(|db| db.list_folders(None).ok())
            .map(|folders| FolderTree::build(folders));

        // Load initial book list
        let all_books = db
            .as_ref()
            .and_then(|db| db.list_books(500, 0).ok())
            .unwrap_or_default();

        // Initialize Watcher if library exists and auto-import is on
        let mut library_watcher = None;
        let mut watcher_rx = None;
        if let Some(ref lr) = library_root {
            if let Ok(manifest) = lr.load_manifest() {
                if manifest.settings.watcher.auto_import {
                    if let Ok((watcher, rx)) =
                        LibraryWatcher::start(lr.root().to_path_buf(), manifest.settings.watcher)
                    {
                        library_watcher = Some(watcher);
                        watcher_rx = Some(rx);
                    }
                }
            }
        }

        let books = all_books.clone();

        let status_message = if library_root.is_some() {
            String::new()
        } else {
            "No library found. Run 'omniscope init' to create one.".to_string()
        };

        let mut app = Self {
            should_quit: false,
            mode: Mode::Normal,
            active_panel: ActivePanel::BookList,
            left_panel_mode: LeftPanelMode::LibraryView,
            center_panel_mode: CenterPanelMode::BookList,
            books,
            all_books,
            center_items: Vec::new(),
            current_folder: None,
            folder_view_sort: FolderViewSort::default(),
            sync_report: None,
            detached_books: Vec::new(),
            sync_selected: 0,
            selected_index: 0,
            sidebar_items: Vec::new(),
            sidebar_selected: 0,
            sidebar_filter: SidebarFilter::All,
            status_message,
            command_input: String::new(),
            search_input: String::new(),
            popup: None,
            pending_key: None,
            visual_selections: Vec::new(),
            visual_anchor: None,
            fuzzy_searcher: FuzzySearcher::new(),
            db,
            config,
            library_root,
            folder_tree,
            expanded_folders: std::collections::HashSet::new(),
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
            has_explicit_count: false,
            viewport_offset: 0,
            last_jump_pos: None,
            last_visual_range: None,
            last_search: None,
            search_direction: SearchDirection::Forward,
            command_history: Vec::new(),
            command_history_idx: None,
            command_suggestions: Vec::new(),
            command_suggestion_idx: None,
            macro_recorder: MacroRecorder::new(),
            ai_panel_active: false,
            ai_input: String::new(),
            theme: NordTheme::default(),
            clipboard: arboard::Clipboard::new().ok(),
            pending_editor_path: None,
            library_watcher,
            watcher_rx,
        };

        app.refresh_sidebar();
        app
    }

    /// Currently selected book (if any).
    pub fn selected_book(&self) -> Option<&BookSummaryView> {
        if self.center_panel_mode == CenterPanelMode::FolderView {
            if let Some(CenterItem::Book(b)) = self.center_items.get(self.selected_index) {
                return Some(b);
            }
            return None;
        }
        self.books.get(self.selected_index)
    }

    /// Get the cards directory, preferring LibraryRoot over legacy config.
    pub fn cards_dir(&self) -> std::path::PathBuf {
        if let Some(ref lr) = self.library_root {
            lr.cards_dir()
        } else {
            self.config.cards_dir()
        }
    }

    /// Generate a sync report and switch to Sync panel.
    pub fn generate_sync_report(&mut self) {
        if let Some(ref lr) = self.library_root {
            if let Some(ref db) = self.db {
                let sync = omniscope_core::sync::folder_sync::FolderSync::new(lr, db);
                if let Ok(report) = sync.full_scan() {
                    self.sync_report = Some(report);
                    // Dynamically gather ghost/detached books:
                    // Missing file presence or has_file = false
                    if let Ok(all_books) = db.list_books(10000, 0) {
                        self.detached_books = all_books
                            .into_iter()
                            .filter(|b| {
                                !b.has_file
                                    || matches!(
                                        b.file_presence,
                                        omniscope_core::models::folder::FilePresence::Missing { .. }
                                    )
                            })
                            .collect();
                    }

                    self.sync_selected = 0;
                    self.active_panel = ActivePanel::Sync;
                    self.status_message = "Sync report generated.".to_string();
                } else {
                    self.status_message = "Error generating sync report.".to_string();
                }
            } else {
                self.status_message = "Error: Database not available.".to_string();
            }
        } else {
            self.status_message = "Error: Library not available.".to_string();
        }
    }

    /// Get total count of items in sync panel (new folders + untracked files)
    pub fn sync_items_count(&self) -> usize {
        if let Some(ref report) = self.sync_report {
            report.new_on_disk.len() + report.untracked_files.len()
        } else {
            0
        }
    }

    /// Import selected untracked file
    pub fn import_selected_untracked(&mut self) {
        // Clone necessary data to avoid borrow issues
        let (folder_path_opt, file_path_opt) = if let Some(report) = &self.sync_report {
            let new_folders_count = report.new_on_disk.len();

            if self.sync_selected < new_folders_count {
                // Selected item is a new folder
                let folder_path = report.new_on_disk[self.sync_selected].clone();
                (Some(folder_path), None)
            } else {
                // Selected item is an untracked file
                let file_idx = self.sync_selected - new_folders_count;
                let file_path = report.untracked_files.get(file_idx).cloned();
                (None, file_path)
            }
        } else {
            return;
        };

        if let Some(folder_path) = folder_path_opt {
            // Sync folder
            if let Some(ref lr) = self.library_root {
                if let Some(ref db) = self.db {
                    let sync = omniscope_core::sync::folder_sync::FolderSync::new(lr, db);
                    if let Err(e) = sync.apply_sync(
                        &omniscope_core::sync::folder_sync::SyncReport {
                            new_on_disk: vec![folder_path.clone()],
                            missing_on_disk: vec![],
                            in_sync: 0,
                            untracked_files: vec![],
                        },
                        omniscope_core::sync::folder_sync::SyncResolution::DiskWins,
                    ) {
                        self.status_message = format!("Error syncing folder: {}", e);
                        return;
                    }
                    self.status_message = format!("Synced folder: {}", folder_path);
                    self.generate_sync_report();
                }
            }
        } else if let Some(file_path) = file_path_opt {
            self.import_untracked_file(&file_path);
        }
    }

    /// Import a single untracked file
    fn import_untracked_file(&mut self, file_path: &std::path::Path) {
        match omniscope_core::file_import::import_file(file_path) {
            Ok(mut card) => {
                // Attempt to assign the correct parent folder ID based on disk path
                if let (Some(db), Some(lr)) = (&self.db, &self.library_root) {
                    if let Some(parent_path) = file_path.parent() {
                        if let Ok(rel_parent) = parent_path.strip_prefix(lr.root()) {
                            let rel_str = rel_parent.to_string_lossy().replace('\\', "/");
                            if !rel_str.is_empty() {
                                if let Ok(Some(folder_id)) = db.find_folder_by_disk_path(&rel_str) {
                                    card.folder_id = Some(folder_id);
                                }
                            }
                        }
                    }
                }

                let cards_dir = self.cards_dir();
                if let Err(e) = omniscope_core::storage::json_cards::save_card(&cards_dir, &card) {
                    self.status_message = format!("Error saving card: {}", e);
                    return;
                }
                if let Some(ref db) = self.db {
                    if let Err(e) = db.upsert_book(&card) {
                        self.status_message = format!("Error saving to DB: {}", e);
                        return;
                    }
                }
                self.status_message = format!("Imported: {}", card.metadata.title);
                self.generate_sync_report();
            }
            Err(e) => {
                self.status_message = format!("Error importing file: {}", e);
            }
        }
    }

    /// Import all untracked files
    pub fn import_all_untracked(&mut self) {
        // Clone necessary data to avoid borrow issues
        let (new_folders, untracked_files) = if let Some(report) = &self.sync_report {
            (report.new_on_disk.clone(), report.untracked_files.clone())
        } else {
            return;
        };

        let mut imported = 0;
        let mut errors = 0;

        // First sync all folders
        if let Some(ref lr) = self.library_root {
            if let Some(ref db) = self.db {
                let sync = omniscope_core::sync::folder_sync::FolderSync::new(lr, db);
                if !new_folders.is_empty() {
                    if let Err(e) = sync.apply_sync(
                        &omniscope_core::sync::folder_sync::SyncReport {
                            new_on_disk: new_folders.clone(),
                            missing_on_disk: vec![],
                            in_sync: 0,
                            untracked_files: vec![],
                        },
                        omniscope_core::sync::folder_sync::SyncResolution::DiskWins,
                    ) {
                        self.status_message = format!("Error syncing folders: {}", e);
                        return;
                    }
                    imported += new_folders.len();
                }
            }
        }

        // Then import all files
        for file_path in &untracked_files {
            match omniscope_core::file_import::import_file(file_path) {
                Ok(mut card) => {
                    // Attempt to assign the correct parent folder ID based on disk path
                    if let (Some(db), Some(lr)) = (&self.db, &self.library_root) {
                        if let Some(parent_path) = std::path::Path::new(file_path).parent() {
                            if let Ok(rel_parent) = parent_path.strip_prefix(lr.root()) {
                                let rel_str = rel_parent.to_string_lossy().replace('\\', "/");
                                if !rel_str.is_empty() {
                                    if let Ok(Some(folder_id)) = db.find_folder_by_disk_path(&rel_str) {
                                        card.folder_id = Some(folder_id);
                                    }
                                }
                            }
                        }
                    }

                    let cards_dir = self.cards_dir();
                    if omniscope_core::storage::json_cards::save_card(&cards_dir, &card).is_ok() {
                        let mut db_ok = true;
                        if let Some(ref db) = self.db {
                            if db.upsert_book(&card).is_err() {
                                db_ok = false;
                            }
                        }
                        if db_ok {
                            imported += 1;
                        } else {
                            errors += 1;
                        }
                    } else {
                        errors += 1;
                    }
                }
                Err(_) => {
                    errors += 1;
                }
            }
        }

        if errors > 0 {
            self.status_message = format!("Imported {}, {} errors", imported, errors);
        } else {
            self.status_message = format!("Imported {} items", imported);
        }
        self.generate_sync_report();
    }

    /// Sync navigation - move up
    pub fn sync_move_up(&mut self) {
        if self.sync_selected > 0 {
            self.sync_selected -= 1;
        }
    }

    /// Sync navigation - move down
    pub fn sync_move_down(&mut self) {
        let max = self.sync_items_count().saturating_sub(1);
        if self.sync_selected < max {
            self.sync_selected += 1;
        }
    }
}
