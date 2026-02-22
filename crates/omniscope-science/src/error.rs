use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScienceError {
    #[error("invalid DOI: {0}")]
    InvalidDoi(String),

    #[error("invalid arXiv ID: {0}")]
    InvalidArxivId(String),

    #[error("invalid ISBN: {0}")]
    InvalidIsbn(String),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("API error from {0}: {1}")]
    ApiError(String, String),

    #[error("rate limit from {0}, retry after {1}s")]
    RateLimit(String, u64),

    #[error("no mirror available for {0}")]
    NoMirror(String),

    #[error("parse error: {0}")]
    Parse(String),

    #[error("PDF extraction error: {0}")]
    PdfExtraction(String),

    #[error("source unavailable: {0}")]
    SourceUnavailable(String),

    #[error("cache error: {0}")]
    Cache(String),

    #[error("identifier not found: {0}")]
    IdentifierNotFound(String),
}

pub type Result<T> = std::result::Result<T, ScienceError>;
