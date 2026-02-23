pub mod extractor;
pub mod parser;
pub mod resolver;

pub use extractor::{LibraryLookup, PdfTextExtractor, ReferenceExtractor};
pub use resolver::{ExtractedReference, ResolutionMethod, resolve_unidentified};
