//! Omniscope Science — arXiv, DOI, CrossRef, metadata enrichment.

pub mod arxiv;
pub mod config;
pub mod dedup;
pub mod enrichment;
pub mod error;
pub mod formats;
pub mod http;
pub mod identifiers;
pub mod references;
pub mod sources;
pub mod types;

pub use config::ScienceConfig;
pub use error::{Result, ScienceError};
pub use types::{CitationGraph, DocumentType, OpenAccessInfo, ScientificIdentifiers};
