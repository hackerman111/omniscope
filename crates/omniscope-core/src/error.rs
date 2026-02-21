use thiserror::Error;

/// All errors that can occur in omniscope-core.
#[derive(Debug, Error)]
pub enum OmniscopeError {
    #[error("Book not found: {0}")]
    BookNotFound(String),

    #[error("Tag not found: {0}")]
    TagNotFound(String),

    #[error("Library not found: {0}")]
    LibraryNotFound(String),

    #[error("Library not initialized. Run 'omniscope init' in your books directory.")]
    LibraryNotInitialized,

    #[error("Library already exists at: {0}")]
    LibraryAlreadyExists(String),

    #[error("Directory does not exist: {0}")]
    DirectoryNotFound(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Duplicate book: {0}")]
    DuplicateBook(String),

    #[error("Config error: {0}")]
    ConfigError(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("TOML serialize error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
}

/// Exit codes matching the CLI specification.
#[repr(i32)]
pub enum ExitCode {
    Success = 0,
    GeneralError = 1,
    NotFound = 2,
    InvalidArgs = 3,
    FileSystemError = 4,
    AiError = 5,
    NetworkError = 6,
    Conflict = 7,
    ConfirmRequired = 8,
}

pub type Result<T> = std::result::Result<T, OmniscopeError>;
