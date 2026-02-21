use thiserror::Error;

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("Connection error: {0}")]
    Connection(#[source] rusqlite::Error),

    #[error("Migration error at version {version}: {message}")]
    Migration { version: u32, message: String },

    #[error("Query error: {0}")]
    Query(String),

    #[error("Book not found: {0}")]
    BookNotFound(String),

    #[error("Folder not found: {0}")]
    FolderNotFound(String),
}

impl From<rusqlite::Error> for DatabaseError {
    fn from(e: rusqlite::Error) -> Self {
        match e {
            rusqlite::Error::QueryReturnedNoRows => {
                DatabaseError::Query("No rows returned".to_string())
            }
            other => DatabaseError::Connection(other),
        }
    }
}
