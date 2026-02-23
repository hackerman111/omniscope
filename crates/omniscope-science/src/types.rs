use crate::identifiers::{arxiv::ArxivId, doi::Doi, isbn::Isbn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScientificIdentifiers {
    pub doi: Option<Doi>,
    pub arxiv_id: Option<ArxivId>,
    pub isbn: Vec<Isbn>,
    pub pmid: Option<String>,
    pub pmcid: Option<String>,
    pub s2_paper_id: Option<String>,
    pub openalex_id: Option<String>,
    pub mag_id: Option<String>,
    pub dblp_key: Option<String>,
    pub openlibrary_id: Option<String>,
    pub issn: Vec<String>,
    pub eissn: Option<String>,
    pub zbl_id: Option<String>,
    pub mr_id: Option<String>,
    pub oclc: Option<String>,
    pub wikidata_id: Option<String>,
    pub google_scholar_id: Option<String>,
    pub semantic_scholar_url: Option<String>,
    pub researchgate_url: Option<String>,
    pub academia_url: Option<String>,
    pub hal_id: Option<String>,
    pub ssrn_id: Option<String>,
    pub repec_id: Option<String>,
    pub nber_id: Option<String>,
    pub extra: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CitationGraph {
    pub citation_count: u32,
    pub reference_count: u32,
    pub influential_citation_count: u32,
    pub last_updated: Option<chrono::DateTime<chrono::Utc>>,
    pub references: Vec<String>,
    pub cited_by_sample: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OpenAccessInfo {
    pub is_open: bool,
    pub status: Option<String>,
    pub license: Option<String>,
    pub oa_url: Option<String>,
    pub pdf_urls: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DocumentType {
    Book,
    BookChapter,
    Textbook,
    JournalArticle,
    ReviewArticle,
    ConferencePaper,
    Preprint,
    WorkingPaper,
    TechnicalReport,
    PhdThesis,
    MasterThesis,
    BachelorThesis,
    Standard,
    Patent,
    Dataset,
    Software,
    Dissertation,
    Encyclopedia,
    Dictionary,
    Monograph,
    EditedVolume,
    Proceedings,
    Magazine,
    Newspaper,
    BlogPost,
    Webpage,
    Other(String),
}

impl DocumentType {
    pub fn from_crossref_type(s: &str) -> Self {
        match s {
            "journal-article" => Self::JournalArticle,
            "book" => Self::Book,
            "book-chapter" => Self::BookChapter,
            "proceedings-article" => Self::ConferencePaper,
            "proceedings" => Self::Proceedings,
            "posted-content" => Self::Preprint,
            "report" => Self::TechnicalReport,
            "dataset" => Self::Dataset,
            "dissertation" => Self::PhdThesis,
            "standard" => Self::Standard,
            "monograph" => Self::Monograph,
            "edited-book" => Self::EditedVolume,
            "reference-entry" => Self::Encyclopedia,
            other => Self::Other(other.to_string()),
        }
    }

    pub fn to_bibtex_type(&self) -> &'static str {
        match self {
            Self::JournalArticle | Self::ReviewArticle => "article",
            Self::Book | Self::Textbook | Self::Monograph => "book",
            Self::BookChapter => "incollection",
            Self::ConferencePaper => "inproceedings",
            Self::Preprint | Self::WorkingPaper => "misc",
            Self::PhdThesis | Self::Dissertation => "phdthesis",
            Self::MasterThesis => "mastersthesis",
            Self::TechnicalReport => "techreport",
            Self::Proceedings => "proceedings",
            _ => "misc",
        }
    }
}

impl Default for DocumentType {
    fn default() -> Self {
        Self::Other(String::new())
    }
}
