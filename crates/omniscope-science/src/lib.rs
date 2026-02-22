//! Omniscope Science — arXiv, DOI, CrossRef, metadata enrichment.

pub mod error;
pub mod http;
pub mod config;
pub mod identifiers;
pub mod types;
pub mod arxiv;
pub mod sources;
pub mod references;
pub mod enrichment;
pub mod formats;
pub mod dedup;

pub use error::{Result, ScienceError};
pub use config::ScienceConfig;
pub use types::{ScientificIdentifiers, CitationGraph, OpenAccessInfo, DocumentType};
