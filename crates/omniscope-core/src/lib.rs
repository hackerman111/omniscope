pub mod config;
pub mod error;
pub mod file_import;
pub mod frecency;
pub mod models;
pub mod search;
pub mod search_dsl;
pub mod storage;
pub mod undo;
pub mod viewer;

pub use config::{AppConfig, GlobalConfig, KnownLibrary};
pub use error::{OmniscopeError, Result};
pub use models::*;

pub use search::FuzzySearcher;
pub use search_dsl::SearchQuery;

pub use storage::database::{
    ConnectionPool, Database, DatabaseError, open_database, open_in_memory,
};
pub use storage::folders::{
    FolderTemplate, SyncReport, create_folder_on_disk, scaffold_template, sync_folders,
};
pub use storage::init::{InitOptions, init_library};
pub use storage::library_root::LibraryRoot;
pub use storage::scan::{ScanOptions, ScanResult, scan_library};

pub use storage::repositories::{
    BookRepository, FolderRepository, LibraryRepository, Repository, SqliteBookRepository,
    SqliteFolderRepository, SqliteLibraryRepository, SqliteTagRepository, TagRepository,
};

pub use storage::queries::{BookSearchQuery, FrecencyService, LibraryStatsQuery};

#[cfg(feature = "async")]
pub use storage::repositories::AsyncRepository;
