pub mod merge;
pub mod pipeline;

pub use merge::{
    BookCardMergeExt, MergeStrategy, MetadataSource, PartialMetadata, source_priority,
};
pub use pipeline::{EnrichmentPipeline, EnrichmentReport, FileMetadataExtractor};
