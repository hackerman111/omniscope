pub mod config;
pub mod error;
pub mod file_import;
pub mod frecency;
pub mod models;
pub mod search;
pub mod search_dsl;
pub mod storage;
<<<<<<< gemini
=======
pub mod sync;
pub mod undo;
>>>>>>> local
pub mod viewer;

pub use config::{AppConfig, GlobalConfig, KnownLibrary};
pub use error::{OmniscopeError, Result};
pub use models::*;

pub use search::FuzzySearcher;
pub use search_dsl::SearchQuery;
<<<<<<< gemini
pub use storage::database::Database;
=======

pub use storage::database::{ConnectionPool, Database, DatabaseError, open_database, open_in_memory};
pub use storage::library_root::LibraryRoot;
pub use storage::init::{init_library, InitOptions};
pub use storage::folders::{FolderTemplate, scaffold_template, create_folder_on_disk};
pub use storage::scan::{scan_library, ScanOptions, ScanResult};

pub use storage::repositories::{
    Repository, BookRepository, SqliteBookRepository,
    FolderRepository, SqliteFolderRepository,
    LibraryRepository, SqliteLibraryRepository,
    TagRepository, SqliteTagRepository,
};

pub use storage::queries::{BookSearchQuery, FrecencyService, LibraryStatsQuery};

#[cfg(feature = "async")]
pub use storage::repositories::AsyncRepository;
>>>>>>> local
