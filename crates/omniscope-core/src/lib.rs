pub mod config;
pub mod error;
pub mod file_import;
pub mod frecency;
pub mod models;
pub mod search;
pub mod search_dsl;
pub mod storage;
pub mod viewer;

pub use config::AppConfig;
pub use error::{OmniscopeError, Result};
pub use models::*;
pub use search::FuzzySearcher;
pub use search_dsl::SearchQuery;
pub use storage::database::Database;
