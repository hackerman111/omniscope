use std::collections::HashSet;

use omniscope_core::models::{BookCard, DocumentType};

use crate::identifiers::{arxiv::ArxivId, doi::Doi, isbn::Isbn};

const FIELD_TITLE: &str = "metadata.title";
const FIELD_SUBTITLE: &str = "metadata.subtitle";
const FIELD_AUTHORS: &str = "metadata.authors";
const FIELD_YEAR: &str = "metadata.year";
const FIELD_PUBLISHER: &str = "metadata.publisher";
const FIELD_LANGUAGE: &str = "metadata.language";
const FIELD_PAGES: &str = "metadata.pages";
const FIELD_EDITION: &str = "metadata.edition";
const FIELD_SERIES: &str = "metadata.series";
const FIELD_SERIES_INDEX: &str = "metadata.series_index";
const FIELD_TAGS: &str = "organization.tags";
const FIELD_ABSTRACT: &str = "ai.summary";
const FIELD_TLDR: &str = "ai.tldr";
const FIELD_DOI: &str = "identifiers.doi";
const FIELD_ARXIV: &str = "identifiers.arxiv_id";
const FIELD_ISBN: &str = "metadata.isbn";
const FIELD_ISBN13: &str = "identifiers.isbn13";
const FIELD_ISBN10: &str = "identifiers.isbn10";
const FIELD_PMID: &str = "identifiers.pmid";
const FIELD_PMCID: &str = "identifiers.pmcid";
const FIELD_S2: &str = "identifiers.semantic_scholar_id";
const FIELD_OPENALEX: &str = "identifiers.openalex_id";
const FIELD_MAG: &str = "identifiers.mag_id";
const FIELD_DBLP: &str = "identifiers.dblp_key";
const FIELD_OPENLIBRARY: &str = "web.openlibrary_id";
const FIELD_DOC_TYPE: &str = "publication.doc_type";
const FIELD_JOURNAL: &str = "publication.journal";
const FIELD_CONFERENCE: &str = "publication.conference";
const FIELD_VENUE: &str = "publication.venue";
const FIELD_VOLUME: &str = "publication.volume";
const FIELD_ISSUE: &str = "publication.issue";
const FIELD_PUBLICATION_PAGES: &str = "publication.pages";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MetadataSource {
    UserManual,
    CrossRef,
    ArxivApi,
    PdfInternal,
    EpubOpf,
    OpenLibrary,
    SemanticScholar,
    OpenAlex,
    GoogleBooks,
    AiInferred,
    AnnasArchive,
    Unknown,
}

impl MetadataSource {
    pub fn as_tag(self) -> &'static str {
        match self {
            Self::UserManual => "user_manual",
            Self::CrossRef => "crossref",
            Self::ArxivApi => "arxiv_api",
            Self::PdfInternal => "pdf_internal",
            Self::EpubOpf => "epub_opf",
            Self::OpenLibrary => "openlibrary",
            Self::SemanticScholar => "semantic_scholar",
            Self::OpenAlex => "openalex",
            Self::GoogleBooks => "google_books",
            Self::AiInferred => "ai_inferred",
            Self::AnnasArchive => "annas_archive",
            Self::Unknown => "unknown",
        }
    }

    fn from_tag(value: &str) -> Self {
        match value {
            "user_manual" => Self::UserManual,
            "crossref" => Self::CrossRef,
            "arxiv_api" => Self::ArxivApi,
            "pdf_internal" => Self::PdfInternal,
            "epub_opf" => Self::EpubOpf,
            "openlibrary" => Self::OpenLibrary,
            "semantic_scholar" => Self::SemanticScholar,
            "openalex" => Self::OpenAlex,
            "google_books" => Self::GoogleBooks,
            "ai_inferred" => Self::AiInferred,
            "annas_archive" => Self::AnnasArchive,
            _ => Self::Unknown,
        }
    }
}

pub fn source_priority(source: MetadataSource) -> u8 {
    match source {
        MetadataSource::UserManual => 100,
        MetadataSource::CrossRef => 90,
        MetadataSource::ArxivApi => 85,
        MetadataSource::PdfInternal => 80,
        MetadataSource::EpubOpf => 75,
        MetadataSource::OpenLibrary => 70,
        MetadataSource::SemanticScholar => 65,
        MetadataSource::OpenAlex => 60,
        MetadataSource::GoogleBooks => 55,
        MetadataSource::AiInferred => 40,
        MetadataSource::AnnasArchive => 30,
        MetadataSource::Unknown => 10,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeStrategy {
    HighestPriority,
    Concat,
    Longest,
    UserOverride,
}

#[derive(Debug, Clone, Default)]
pub struct PartialMetadata {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub authors: Vec<String>,
    pub year: Option<i32>,
    pub publisher: Option<String>,
    pub language: Option<String>,
    pub pages: Option<u32>,
    pub edition: Option<u32>,
    pub series: Option<String>,
    pub series_index: Option<f32>,
    pub tags: Vec<String>,
    pub abstract_text: Option<String>,
    pub tldr: Option<String>,
    pub doi: Option<Doi>,
    pub arxiv_id: Option<ArxivId>,
    pub isbn: Vec<Isbn>,
    pub pmid: Option<String>,
    pub pmcid: Option<String>,
    pub semantic_scholar_id: Option<String>,
    pub openalex_id: Option<String>,
    pub mag_id: Option<String>,
    pub dblp_key: Option<String>,
    pub openlibrary_id: Option<String>,
    pub doc_type: Option<DocumentType>,
    pub journal: Option<String>,
    pub conference: Option<String>,
    pub venue: Option<String>,
    pub volume: Option<String>,
    pub issue: Option<String>,
    pub publication_pages: Option<String>,
}

pub trait BookCardMergeExt {
    fn merge_metadata(&mut self, new_data: PartialMetadata, source: MetadataSource);
    fn merge_metadata_with_trace(
        &mut self,
        new_data: PartialMetadata,
        source: MetadataSource,
    ) -> Vec<String>;
}

impl BookCardMergeExt for BookCard {
    fn merge_metadata(&mut self, new_data: PartialMetadata, source: MetadataSource) {
        let _ = self.merge_metadata_with_trace(new_data, source);
    }

    fn merge_metadata_with_trace(
        &mut self,
        new_data: PartialMetadata,
        source: MetadataSource,
    ) -> Vec<String> {
        let mut updated = Vec::new();

        if let Some(value) = clean_text_option(new_data.title) {
            let existing_source = field_source(self, FIELD_TITLE);
            let should_set = self.metadata.title.trim().is_empty()
                || can_update(existing_source, source, MergeStrategy::HighestPriority);
            if should_set {
                if self.metadata.title != value {
                    self.metadata.title = value;
                    push_unique(&mut updated, FIELD_TITLE);
                }
                set_field_source(self, FIELD_TITLE, source);
            }
        }

        if let Some(value) = clean_text_option(new_data.subtitle) {
            let existing_source = field_source(self, FIELD_SUBTITLE);
            let should_set = self.metadata.subtitle.is_none()
                || can_update(existing_source, source, MergeStrategy::HighestPriority);
            if should_set {
                if self.metadata.subtitle.as_deref() != Some(value.as_str()) {
                    self.metadata.subtitle = Some(value);
                    push_unique(&mut updated, FIELD_SUBTITLE);
                }
                set_field_source(self, FIELD_SUBTITLE, source);
            }
        }

        if !new_data.authors.is_empty() {
            let added = concat_unique(&mut self.metadata.authors, new_data.authors);
            if added {
                push_unique(&mut updated, FIELD_AUTHORS);
                set_field_source(self, FIELD_AUTHORS, source);
            }
        }

        if let Some(value) = new_data.year {
            let existing_source = field_source(self, FIELD_YEAR);
            let should_set = self.metadata.year.is_none()
                || can_update(existing_source, source, MergeStrategy::HighestPriority);
            if should_set {
                if self.metadata.year != Some(value) {
                    self.metadata.year = Some(value);
                    push_unique(&mut updated, FIELD_YEAR);
                }
                set_field_source(self, FIELD_YEAR, source);
            }
        }

        if let Some(value) = clean_text_option(new_data.publisher) {
            let existing_source = field_source(self, FIELD_PUBLISHER);
            let should_set = self.metadata.publisher.is_none()
                || can_update(existing_source, source, MergeStrategy::HighestPriority);
            if should_set {
                if self.metadata.publisher.as_deref() != Some(value.as_str()) {
                    self.metadata.publisher = Some(value);
                    push_unique(&mut updated, FIELD_PUBLISHER);
                }
                set_field_source(self, FIELD_PUBLISHER, source);
            }
        }

        if let Some(value) = clean_text_option(new_data.language) {
            let existing_source = field_source(self, FIELD_LANGUAGE);
            let should_set = self.metadata.language.is_none()
                || can_update(existing_source, source, MergeStrategy::HighestPriority);
            if should_set {
                if self.metadata.language.as_deref() != Some(value.as_str()) {
                    self.metadata.language = Some(value);
                    push_unique(&mut updated, FIELD_LANGUAGE);
                }
                set_field_source(self, FIELD_LANGUAGE, source);
            }
        }

        if let Some(value) = new_data.pages {
            let existing_source = field_source(self, FIELD_PAGES);
            let should_set = self.metadata.pages.is_none()
                || can_update(existing_source, source, MergeStrategy::HighestPriority);
            if should_set {
                if self.metadata.pages != Some(value) {
                    self.metadata.pages = Some(value);
                    push_unique(&mut updated, FIELD_PAGES);
                }
                set_field_source(self, FIELD_PAGES, source);
            }
        }

        if let Some(value) = new_data.edition {
            let existing_source = field_source(self, FIELD_EDITION);
            let should_set = self.metadata.edition.is_none()
                || can_update(existing_source, source, MergeStrategy::HighestPriority);
            if should_set {
                if self.metadata.edition != Some(value) {
                    self.metadata.edition = Some(value);
                    push_unique(&mut updated, FIELD_EDITION);
                }
                set_field_source(self, FIELD_EDITION, source);
            }
        }

        if let Some(value) = clean_text_option(new_data.series) {
            let existing_source = field_source(self, FIELD_SERIES);
            let should_set = self.metadata.series.is_none()
                || can_update(existing_source, source, MergeStrategy::HighestPriority);
            if should_set {
                if self.metadata.series.as_deref() != Some(value.as_str()) {
                    self.metadata.series = Some(value);
                    push_unique(&mut updated, FIELD_SERIES);
                }
                set_field_source(self, FIELD_SERIES, source);
            }
        }

        if let Some(value) = new_data.series_index {
            let existing_source = field_source(self, FIELD_SERIES_INDEX);
            let should_set = self.metadata.series_index.is_none()
                || can_update(existing_source, source, MergeStrategy::HighestPriority);
            if should_set {
                if self.metadata.series_index != Some(value) {
                    self.metadata.series_index = Some(value);
                    push_unique(&mut updated, FIELD_SERIES_INDEX);
                }
                set_field_source(self, FIELD_SERIES_INDEX, source);
            }
        }

        if !new_data.tags.is_empty() {
            let added = concat_unique(&mut self.organization.tags, new_data.tags);
            if added {
                push_unique(&mut updated, FIELD_TAGS);
                set_field_source(self, FIELD_TAGS, source);
            }
        }

        if let Some(value) = clean_text_option(new_data.abstract_text) {
            let existing_source = field_source(self, FIELD_ABSTRACT);
            let current_len = self
                .ai
                .summary
                .as_ref()
                .map(|s| s.chars().count())
                .unwrap_or(0);
            let candidate_len = value.chars().count();
            let should_set = self.ai.summary.is_none()
                || (candidate_len > current_len
                    && can_update(existing_source, source, MergeStrategy::Longest));
            if should_set {
                if self.ai.summary.as_deref() != Some(value.as_str()) {
                    self.ai.summary = Some(value);
                    push_unique(&mut updated, FIELD_ABSTRACT);
                }
                set_field_source(self, FIELD_ABSTRACT, source);
            }
        }

        if let Some(value) = clean_text_option(new_data.tldr) {
            let existing_source = field_source(self, FIELD_TLDR);
            let should_set = self.ai.tldr.is_none()
                || can_update(existing_source, source, MergeStrategy::HighestPriority);
            if should_set {
                if self.ai.tldr.as_deref() != Some(value.as_str()) {
                    self.ai.tldr = Some(value);
                    push_unique(&mut updated, FIELD_TLDR);
                }
                set_field_source(self, FIELD_TLDR, source);
            }
        }

        if let Some(value) = new_data.doi {
            let existing_source = field_source(self, FIELD_DOI);
            let identifiers = self.identifiers.get_or_insert_with(Default::default);
            let should_set = identifiers.doi.is_none()
                || can_update(existing_source, source, MergeStrategy::HighestPriority);
            if should_set {
                if identifiers.doi.as_deref() != Some(value.normalized.as_str()) {
                    identifiers.doi = Some(value.normalized);
                    push_unique(&mut updated, FIELD_DOI);
                }
                set_field_source(self, FIELD_DOI, source);
            }
        }

        if let Some(value) = new_data.arxiv_id {
            let existing_source = field_source(self, FIELD_ARXIV);
            let normalized = format_arxiv_id(&value);
            let identifiers = self.identifiers.get_or_insert_with(Default::default);
            let should_set = identifiers.arxiv_id.is_none()
                || can_update(existing_source, source, MergeStrategy::HighestPriority);
            if should_set {
                if identifiers.arxiv_id.as_deref() != Some(normalized.as_str()) {
                    identifiers.arxiv_id = Some(normalized);
                    push_unique(&mut updated, FIELD_ARXIV);
                }
                set_field_source(self, FIELD_ARXIV, source);
            }
        }

        if !new_data.isbn.is_empty() {
            let mut isbn_added = false;
            let mut isbn13_updated = false;
            let mut isbn10_updated = false;
            let existing_isbn_source = field_source(self, FIELD_ISBN);
            let identifiers = self.identifiers.get_or_insert_with(Default::default);
            for isbn in new_data.isbn {
                if !self.metadata.isbn.iter().any(|value| value == &isbn.isbn13) {
                    self.metadata.isbn.push(isbn.isbn13.clone());
                    isbn_added = true;
                }
                let can_update_isbn13 = identifiers.isbn13.is_none()
                    || can_update(existing_isbn_source, source, MergeStrategy::HighestPriority);
                if can_update_isbn13 && identifiers.isbn13.as_deref() != Some(isbn.isbn13.as_str())
                {
                    identifiers.isbn13 = Some(isbn.isbn13.clone());
                    isbn13_updated = true;
                }
                if let Some(isbn10) = isbn.isbn10
                    && (identifiers.isbn10.is_none()
                        || can_update(existing_isbn_source, source, MergeStrategy::HighestPriority))
                    && identifiers.isbn10.as_deref() != Some(isbn10.as_str())
                {
                    identifiers.isbn10 = Some(isbn10);
                    isbn10_updated = true;
                }
            }

            if isbn_added {
                push_unique(&mut updated, FIELD_ISBN);
                set_field_source(self, FIELD_ISBN, source);
            }
            if isbn13_updated {
                push_unique(&mut updated, FIELD_ISBN13);
                set_field_source(self, FIELD_ISBN13, source);
            }
            if isbn10_updated {
                push_unique(&mut updated, FIELD_ISBN10);
                set_field_source(self, FIELD_ISBN10, source);
            }
        }

        merge_identifier_field(
            self,
            FIELD_PMID,
            source,
            new_data.pmid,
            |identifiers| &mut identifiers.pmid,
            &mut updated,
        );
        merge_identifier_field(
            self,
            FIELD_PMCID,
            source,
            new_data.pmcid,
            |identifiers| &mut identifiers.pmcid,
            &mut updated,
        );
        merge_identifier_field(
            self,
            FIELD_S2,
            source,
            new_data.semantic_scholar_id,
            |identifiers| &mut identifiers.semantic_scholar_id,
            &mut updated,
        );
        merge_identifier_field(
            self,
            FIELD_OPENALEX,
            source,
            new_data.openalex_id,
            |identifiers| &mut identifiers.openalex_id,
            &mut updated,
        );
        merge_identifier_field(
            self,
            FIELD_MAG,
            source,
            new_data.mag_id,
            |identifiers| &mut identifiers.mag_id,
            &mut updated,
        );
        merge_identifier_field(
            self,
            FIELD_DBLP,
            source,
            new_data.dblp_key,
            |identifiers| &mut identifiers.dblp_key,
            &mut updated,
        );

        if let Some(value) = clean_text_option(new_data.openlibrary_id) {
            let existing_source = field_source(self, FIELD_OPENLIBRARY);
            let should_set = self.web.openlibrary_id.is_none()
                || can_update(existing_source, source, MergeStrategy::HighestPriority);
            if should_set {
                if self.web.openlibrary_id.as_deref() != Some(value.as_str()) {
                    self.web.openlibrary_id = Some(value);
                    push_unique(&mut updated, FIELD_OPENLIBRARY);
                }
                set_field_source(self, FIELD_OPENLIBRARY, source);
            }
        }

        if let Some(value) = new_data.doc_type {
            let existing_source = field_source(self, FIELD_DOC_TYPE);
            let publication = self.publication.get_or_insert_with(Default::default);
            let should_set = publication.doc_type == DocumentType::default()
                || can_update(existing_source, source, MergeStrategy::HighestPriority);
            if should_set {
                if publication.doc_type != value {
                    publication.doc_type = value;
                    push_unique(&mut updated, FIELD_DOC_TYPE);
                }
                set_field_source(self, FIELD_DOC_TYPE, source);
            }
        }

        merge_publication_field(
            self,
            FIELD_JOURNAL,
            source,
            new_data.journal,
            |publication| &mut publication.journal,
            &mut updated,
        );
        merge_publication_field(
            self,
            FIELD_CONFERENCE,
            source,
            new_data.conference,
            |publication| &mut publication.conference,
            &mut updated,
        );
        merge_publication_field(
            self,
            FIELD_VENUE,
            source,
            new_data.venue,
            |publication| &mut publication.venue,
            &mut updated,
        );
        merge_publication_field(
            self,
            FIELD_VOLUME,
            source,
            new_data.volume,
            |publication| &mut publication.volume,
            &mut updated,
        );
        merge_publication_field(
            self,
            FIELD_ISSUE,
            source,
            new_data.issue,
            |publication| &mut publication.issue,
            &mut updated,
        );
        merge_publication_field(
            self,
            FIELD_PUBLICATION_PAGES,
            source,
            new_data.publication_pages,
            |publication| &mut publication.pages,
            &mut updated,
        );

        if !updated.is_empty() {
            self.touch();
        }

        updated
    }
}

fn merge_identifier_field(
    card: &mut BookCard,
    field_name: &str,
    source: MetadataSource,
    incoming: Option<String>,
    slot: impl FnOnce(&mut omniscope_core::models::ScientificIdentifiers) -> &mut Option<String>,
    updated: &mut Vec<String>,
) {
    let Some(value) = clean_text_option(incoming) else {
        return;
    };

    let existing_source = field_source(card, field_name);
    let identifiers = card.identifiers.get_or_insert_with(Default::default);
    let target = slot(identifiers);
    let should_set =
        target.is_none() || can_update(existing_source, source, MergeStrategy::HighestPriority);
    if should_set {
        if target.as_deref() != Some(value.as_str()) {
            *target = Some(value);
            push_unique(updated, field_name);
        }
        set_field_source(card, field_name, source);
    }
}

fn merge_publication_field(
    card: &mut BookCard,
    field_name: &str,
    source: MetadataSource,
    incoming: Option<String>,
    slot: impl FnOnce(&mut omniscope_core::models::BookPublication) -> &mut Option<String>,
    updated: &mut Vec<String>,
) {
    let Some(value) = clean_text_option(incoming) else {
        return;
    };

    let existing_source = field_source(card, field_name);
    let publication = card.publication.get_or_insert_with(Default::default);
    let target = slot(publication);
    let should_set =
        target.is_none() || can_update(existing_source, source, MergeStrategy::HighestPriority);
    if should_set {
        if target.as_deref() != Some(value.as_str()) {
            *target = Some(value);
            push_unique(updated, field_name);
        }
        set_field_source(card, field_name, source);
    }
}

fn field_source(card: &BookCard, field: &str) -> MetadataSource {
    card.metadata_sources
        .get(field)
        .map(String::as_str)
        .map(MetadataSource::from_tag)
        .unwrap_or(MetadataSource::Unknown)
}

fn set_field_source(card: &mut BookCard, field: &str, source: MetadataSource) {
    card.metadata_sources
        .insert(field.to_string(), source.as_tag().to_string());
}

fn can_update(
    existing_source: MetadataSource,
    new_source: MetadataSource,
    strategy: MergeStrategy,
) -> bool {
    if matches!(strategy, MergeStrategy::UserOverride)
        && existing_source == MetadataSource::UserManual
        && new_source != MetadataSource::UserManual
    {
        return false;
    }

    source_priority(new_source) >= source_priority(existing_source)
}

fn clean_text_option(input: Option<String>) -> Option<String> {
    input
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn normalize_identity(input: &str) -> String {
    input.trim().to_ascii_lowercase()
}

fn concat_unique(target: &mut Vec<String>, incoming: Vec<String>) -> bool {
    let mut seen = target
        .iter()
        .map(|value| normalize_identity(value))
        .collect::<HashSet<_>>();
    let mut changed = false;

    for value in incoming {
        let cleaned = value.trim();
        if cleaned.is_empty() {
            continue;
        }
        let key = normalize_identity(cleaned);
        if seen.insert(key) {
            target.push(cleaned.to_string());
            changed = true;
        }
    }

    changed
}

fn push_unique(target: &mut Vec<String>, value: &str) {
    if target.iter().any(|existing| existing == value) {
        return;
    }
    target.push(value.to_string());
}

fn format_arxiv_id(value: &ArxivId) -> String {
    match value.version {
        Some(version) => format!("{}v{version}", value.id),
        None => value.id.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_priority_matches_spec() {
        assert_eq!(source_priority(MetadataSource::UserManual), 100);
        assert_eq!(source_priority(MetadataSource::CrossRef), 90);
        assert_eq!(source_priority(MetadataSource::ArxivApi), 85);
        assert_eq!(source_priority(MetadataSource::PdfInternal), 80);
        assert_eq!(source_priority(MetadataSource::EpubOpf), 75);
        assert_eq!(source_priority(MetadataSource::OpenLibrary), 70);
        assert_eq!(source_priority(MetadataSource::SemanticScholar), 65);
        assert_eq!(source_priority(MetadataSource::OpenAlex), 60);
        assert_eq!(source_priority(MetadataSource::GoogleBooks), 55);
        assert_eq!(source_priority(MetadataSource::AiInferred), 40);
        assert_eq!(source_priority(MetadataSource::AnnasArchive), 30);
        assert_eq!(source_priority(MetadataSource::Unknown), 10);
    }

    #[test]
    fn higher_priority_source_overrides_field() {
        let mut card = BookCard::new("Initial title");

        let low_priority = PartialMetadata {
            title: Some("Open Library title".to_string()),
            ..Default::default()
        };
        card.merge_metadata(low_priority, MetadataSource::OpenLibrary);
        assert_eq!(card.metadata.title, "Open Library title");

        let high_priority = PartialMetadata {
            title: Some("CrossRef title".to_string()),
            ..Default::default()
        };
        card.merge_metadata(high_priority, MetadataSource::CrossRef);
        assert_eq!(card.metadata.title, "CrossRef title");

        let lower_again = PartialMetadata {
            title: Some("AI guessed title".to_string()),
            ..Default::default()
        };
        card.merge_metadata(lower_again, MetadataSource::AiInferred);
        assert_eq!(card.metadata.title, "CrossRef title");
    }

    #[test]
    fn concat_merges_authors_and_tags_without_duplicates() {
        let mut card = BookCard::new("Paper");
        card.metadata.authors = vec!["Alice".to_string()];
        card.organization.tags = vec!["transformer".to_string()];

        let incoming = PartialMetadata {
            authors: vec![
                "Alice".to_string(),
                "Bob".to_string(),
                "  bob  ".to_string(),
            ],
            tags: vec!["transformer".to_string(), "nlp".to_string()],
            ..Default::default()
        };
        card.merge_metadata(incoming, MetadataSource::CrossRef);

        assert_eq!(card.metadata.authors, vec!["Alice", "Bob"]);
        assert_eq!(card.organization.tags, vec!["transformer", "nlp"]);
    }

    #[test]
    fn longest_strategy_prefers_longer_abstract() {
        let mut card = BookCard::new("Paper");
        card.merge_metadata(
            PartialMetadata {
                abstract_text: Some("Short abstract.".to_string()),
                ..Default::default()
            },
            MetadataSource::AiInferred,
        );

        card.merge_metadata(
            PartialMetadata {
                abstract_text: Some(
                    "A longer and substantially more informative abstract for the same paper."
                        .to_string(),
                ),
                ..Default::default()
            },
            MetadataSource::SemanticScholar,
        );

        card.merge_metadata(
            PartialMetadata {
                abstract_text: Some("Tiny".to_string()),
                ..Default::default()
            },
            MetadataSource::AnnasArchive,
        );

        assert_eq!(
            card.ai.summary.as_deref(),
            Some("A longer and substantially more informative abstract for the same paper.")
        );
    }

    #[test]
    fn user_manual_data_is_preserved() {
        let mut card = BookCard::new("Paper");
        card.merge_metadata(
            PartialMetadata {
                title: Some("Manual title".to_string()),
                ..Default::default()
            },
            MetadataSource::UserManual,
        );

        card.merge_metadata(
            PartialMetadata {
                title: Some("CrossRef title".to_string()),
                ..Default::default()
            },
            MetadataSource::CrossRef,
        );

        assert_eq!(card.metadata.title, "Manual title");
        assert_eq!(
            card.metadata_sources.get(FIELD_TITLE).map(String::as_str),
            Some("user_manual")
        );
    }
}
