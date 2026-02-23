use crate::sources::Metadata;
use omniscope_core::models::book::BookCard;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MetadataSource {
    Unknown = 10,
    AnnasArchive = 30,
    AiInferred = 40,
    GoogleBooks = 55,
    OpenAlex = 60,
    SemanticScholar = 65,
    OpenLibrary = 70,
    EpubOpf = 75,
    PdfInternal = 80,
    ArxivApi = 85,
    CrossRef = 90,
    UserManual = 100,
}

impl MetadataSource {
    pub fn priority(&self) -> u8 {
        *self as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeStrategy {
    HighestPriority, 
    Concat,          
    Longest,         
    UserOverride,    
}

pub trait MergeMetadata {
    fn merge_metadata(&mut self, new_data: Metadata, source: MetadataSource, strategy: MergeStrategy);
}

impl MergeMetadata for BookCard {
    fn merge_metadata(&mut self, new_data: Metadata, source: MetadataSource, strategy: MergeStrategy) {
        // Title
        if (self.metadata.title.is_empty() || (strategy == MergeStrategy::HighestPriority && source == MetadataSource::UserManual))
            && !new_data.title.is_empty() {
                self.metadata.title = new_data.title.clone();
            }

        // Authors (List)
        if !new_data.authors.is_empty() {
            if self.metadata.authors.is_empty() {
                self.metadata.authors = new_data.authors.clone();
            } else if strategy == MergeStrategy::Concat {
                for author in new_data.authors {
                    if !self.metadata.authors.contains(&author) {
                        self.metadata.authors.push(author);
                    }
                }
            } else if strategy == MergeStrategy::HighestPriority && source == MetadataSource::UserManual {
                 self.metadata.authors = new_data.authors.clone();
            }
        }

        // Year
        if self.metadata.year.is_none() {
            self.metadata.year = new_data.year;
        } else if strategy == MergeStrategy::HighestPriority && source == MetadataSource::UserManual {
             self.metadata.year = new_data.year;
        }

        // Abstract - skipped for now as BookMetadata doesn't have it.

        // Identifiers
        if let Some(doi) = new_data.doi {
             if self.identifiers.is_none() {
                 self.identifiers = Some(Default::default());
             }
             if let Some(ids) = &mut self.identifiers
                 && ids.doi.is_none() {
                     ids.doi = Some(doi);
                 }
        }
        
        if let Some(isbn) = new_data.isbn
             && !self.metadata.isbn.contains(&isbn) {
                 self.metadata.isbn.push(isbn);
             }
        
        if self.metadata.publisher.is_none() && new_data.publisher.is_some() {
            self.metadata.publisher = new_data.publisher;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_title_empty() {
        let mut card = BookCard::new("");
        let meta = Metadata {
            title: "New Title".to_string(),
            ..default_meta()
        };
        card.merge_metadata(meta, MetadataSource::CrossRef, MergeStrategy::UserOverride);
        assert_eq!(card.metadata.title, "New Title");
    }

    #[test]
    fn test_merge_title_existing_user_override() {
        let mut card = BookCard::new("My Title");
        let meta = Metadata {
            title: "New Title".to_string(),
            ..default_meta()
        };
        card.merge_metadata(meta, MetadataSource::CrossRef, MergeStrategy::UserOverride);
        assert_eq!(card.metadata.title, "My Title");
    }

    #[test]
    fn test_merge_authors_concat() {
        let mut card = BookCard::new("Title");
        card.metadata.authors = vec!["A1".to_string()];
        let meta = Metadata {
            authors: vec!["A2".to_string()],
            ..default_meta()
        };
        card.merge_metadata(meta, MetadataSource::SemanticScholar, MergeStrategy::Concat);
        assert_eq!(card.metadata.authors, vec!["A1", "A2"]);
    }

    fn default_meta() -> Metadata {
        Metadata {
            title: "".to_string(),
            authors: vec![],
            year: None,
            abstract_text: None,
            doi: None,
            isbn: None,
            publisher: None,
            journal: None,
            volume: None,
            issue: None,
            pages: None,
        }
    }
}
