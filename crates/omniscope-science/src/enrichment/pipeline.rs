use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use chrono::{Datelike, Utc};
use lopdf::Document;
use omniscope_core::models::{BookCard, BookOpenAccessInfo, DocumentType, FileFormat};
use once_cell::sync::Lazy;
use regex::Regex;
use uuid::Uuid;
use zip::ZipArchive;

use crate::arxiv::client::ArxivClient;
use crate::arxiv::types::ArxivMetadata;
use crate::enrichment::merge::{BookCardMergeExt, MetadataSource, PartialMetadata};
use crate::error::{Result, ScienceError};
use crate::identifiers::arxiv::ArxivId;
use crate::identifiers::doi::Doi;
use crate::identifiers::extract::{
    extract_isbn_from_text, find_arxiv_id_in_pdf, find_doi_in_first_page,
};
use crate::identifiers::isbn::Isbn;
use crate::references::{ExtractedReference, ReferenceExtractor};
use crate::sources::crossref::{CrossRefAuthor, CrossRefSource, CrossRefWork};
use crate::sources::openalex::OpenAlexSource;
use crate::sources::openlibrary::{OpenLibrarySource, OpenLibraryWork};
use crate::sources::semantic_scholar::{S2Paper, S2PaperId, S2Reference, SemanticScholarSource};
use crate::sources::unpaywall::{UnpaywallResult, UnpaywallSource};
use crate::types::DocumentType as ScienceDocumentType;

static YEAR_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b(19|20)\d{2}\b").expect("valid regex"));
static XML_TAG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?is)<[^>]+>").expect("valid regex"));
static REF_MARKER_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\[\d+\]$").expect("valid regex"));
static JOURNAL_HEADER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)^journal of .+\(\d{4}\)\s+\d").expect("valid regex"));
static REF_PREFIX_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\[\d+\][\s,.:;\-]*").expect("valid regex"));

#[derive(Debug, Clone, Default)]
pub struct EnrichmentReport {
    pub steps: Vec<String>,
    pub fields_updated: Vec<String>,
    pub sources_used: Vec<String>,
    pub errors: Vec<String>,
}

impl EnrichmentReport {
    fn add_step(&mut self, step: impl Into<String>) {
        self.steps.push(step.into());
    }

    fn add_source(&mut self, source: impl Into<String>) {
        push_unique(&mut self.sources_used, source.into());
    }

    fn add_error(&mut self, error: impl Into<String>) {
        self.errors.push(error.into());
    }

    fn add_fields<I>(&mut self, fields: I)
    where
        I: IntoIterator<Item = String>,
    {
        for field in fields {
            push_unique(&mut self.fields_updated, field);
        }
    }
}

pub trait FileMetadataExtractor: Send + Sync {
    fn extract_pdf_metadata(&self, pdf_path: &Path) -> Result<PartialMetadata>;
    fn extract_epub_metadata(&self, epub_path: &Path) -> Result<PartialMetadata>;
}

struct DefaultFileMetadataExtractor;

impl FileMetadataExtractor for DefaultFileMetadataExtractor {
    fn extract_pdf_metadata(&self, pdf_path: &Path) -> Result<PartialMetadata> {
        let mut page_count_error = None;
        let page_count = match extract_pdf_page_count(pdf_path) {
            Ok(value) => value,
            Err(err) => {
                page_count_error = Some(err.to_string());
                None
            }
        };

        let mut metadata_error = None;
        let metadata = match Document::load_metadata(pdf_path) {
            Ok(value) => Some(value),
            Err(err) => {
                metadata_error = Some(err.to_string());
                None
            }
        };

        let mut first_page_error = None;
        let first_page_text = match extract_pdf_first_page_text_for_title(pdf_path) {
            Ok(value) => Some(value),
            Err(err) => {
                first_page_error = Some(err.to_string());
                None
            }
        };

        if metadata.is_none() && first_page_text.is_none() && page_count.is_none() {
            let pages_msg =
                page_count_error.unwrap_or_else(|| "failed to read PDF page count".to_string());
            let metadata_msg =
                metadata_error.unwrap_or_else(|| "failed to read PDF metadata".to_string());
            let text_msg =
                first_page_error.unwrap_or_else(|| "failed to extract first-page text".to_string());
            return Err(ScienceError::PdfExtraction(format!(
                "{pages_msg}; {metadata_msg}; {text_msg}"
            )));
        }

        let metadata_title = metadata
            .as_ref()
            .and_then(|value| value.title.as_deref())
            .and_then(clean_pdf_metadata_field)
            .filter(|value| is_plausible_pdf_title(value));

        let text_title = first_page_text
            .as_deref()
            .and_then(extract_title_from_pdf_text);

        let year = first_page_text
            .as_deref()
            .and_then(parse_year_from_text)
            .or_else(|| {
                metadata
                    .as_ref()
                    .and_then(|value| value.creation_date.as_deref())
                    .and_then(parse_year_from_text)
            })
            .or_else(|| {
                metadata
                    .as_ref()
                    .and_then(|value| value.modification_date.as_deref())
                    .and_then(parse_year_from_text)
            });

        let authors = metadata
            .as_ref()
            .and_then(|value| value.author.as_deref())
            .and_then(clean_pdf_metadata_field)
            .map(|value| split_pdf_authors(&value))
            .unwrap_or_default();

        let filename_title = extract_title_from_filename(pdf_path, &authors);
        let title = pick_best_title_candidate(metadata_title, text_title, filename_title);

        Ok(PartialMetadata {
            title,
            authors,
            year,
            pages: page_count,
            ..Default::default()
        })
    }

    fn extract_epub_metadata(&self, epub_path: &Path) -> Result<PartialMetadata> {
        let file = File::open(epub_path).map_err(|e| {
            ScienceError::Parse(format!("failed to open EPUB {}: {e}", epub_path.display()))
        })?;
        let mut archive = ZipArchive::new(file)
            .map_err(|e| ScienceError::Parse(format!("invalid EPUB ZIP: {e}")))?;

        let container_xml = read_zip_entry_to_string(&mut archive, "META-INF/container.xml")?;
        let opf_path = parse_container_full_path(&container_xml).or_else(|| {
            archive
                .file_names()
                .find(|name| name.to_ascii_lowercase().ends_with(".opf"))
                .map(ToOwned::to_owned)
        });
        let Some(opf_path) = opf_path else {
            return Err(ScienceError::Parse(
                "EPUB does not contain OPF package path".to_string(),
            ));
        };

        let opf_xml = read_zip_entry_to_string(&mut archive, &opf_path)?;
        Ok(parse_epub_opf(&opf_xml))
    }
}

#[derive(Clone)]
pub struct EnrichmentPipeline {
    pub crossref: Arc<CrossRefSource>,
    pub s2: Arc<SemanticScholarSource>,
    pub openalex: Arc<OpenAlexSource>,
    pub unpaywall: Arc<UnpaywallSource>,
    pub openlibrary: Arc<OpenLibrarySource>,
    pub arxiv_client: Arc<ArxivClient>,
    file_metadata_extractor: Arc<dyn FileMetadataExtractor>,
}

impl EnrichmentPipeline {
    pub fn new(
        crossref: Arc<CrossRefSource>,
        s2: Arc<SemanticScholarSource>,
        openalex: Arc<OpenAlexSource>,
        unpaywall: Arc<UnpaywallSource>,
        openlibrary: Arc<OpenLibrarySource>,
        arxiv_client: Arc<ArxivClient>,
    ) -> Self {
        Self {
            crossref,
            s2,
            openalex,
            unpaywall,
            openlibrary,
            arxiv_client,
            file_metadata_extractor: Arc::new(DefaultFileMetadataExtractor),
        }
    }

    pub fn with_file_extractor(mut self, extractor: Arc<dyn FileMetadataExtractor>) -> Self {
        self.file_metadata_extractor = extractor;
        self
    }

    /// Run only local file metadata extraction (PDF/EPUB parsing + DOI/arXiv probes).
    ///
    /// This mode avoids network calls and is intended for fast import-time enrichment.
    pub fn enrich_local_file_metadata(card: &mut BookCard) -> EnrichmentReport {
        let mut report = EnrichmentReport::default();
        let extractor = DefaultFileMetadataExtractor;
        run_file_stage_with_extractor(&extractor, card, &mut report);
        report
    }

    /// Create a production pipeline using defaults and optional env credentials.
    pub fn from_env() -> Self {
        let polite_email = env_first([
            "OMNISCOPE_POLITE_EMAIL",
            "POLITE_POOL_EMAIL",
            "OMNISCOPE_CROSSREF_EMAIL",
        ]);
        let semantic_scholar_key = env_first([
            "OMNISCOPE_SEMANTIC_SCHOLAR_API_KEY",
            "SEMANTIC_SCHOLAR_API_KEY",
        ]);
        let unpaywall_email = env_first(["OMNISCOPE_UNPAYWALL_EMAIL", "UNPAYWALL_EMAIL"])
            .or_else(|| polite_email.clone())
            .unwrap_or_else(|| "noreply@example.com".to_string());

        Self::new(
            Arc::new(CrossRefSource::new(polite_email)),
            Arc::new(SemanticScholarSource::new(semantic_scholar_key)),
            Arc::new(OpenAlexSource::new()),
            Arc::new(UnpaywallSource::new(unpaywall_email)),
            Arc::new(OpenLibrarySource::new()),
            Arc::new(ArxivClient::new()),
        )
    }

    /// Blocking helper for sync callers (TUI/CLI): runs full file + online enrichment.
    pub fn enrich_full_metadata_blocking(card: &mut BookCard) -> EnrichmentReport {
        let pipeline = Self::from_env();
        match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime.block_on(async { pipeline.enrich(card).await }),
            Err(err) => {
                let mut report = Self::enrich_local_file_metadata(card);
                report.add_error(format!("failed to start async runtime: {err}"));
                report
            }
        }
    }

    pub async fn enrich(&self, card: &mut BookCard) -> EnrichmentReport {
        let mut report = EnrichmentReport::default();

        self.run_file_stage(card, &mut report);
        self.run_identifier_stage(card, &mut report).await;
        self.run_semantic_scholar_stage(card, &mut report).await;
        self.run_references_stage(card, &mut report).await;
        self.run_open_access_stage(card, &mut report).await;

        report
    }

    fn run_file_stage(&self, card: &mut BookCard, report: &mut EnrichmentReport) {
        run_file_stage_with_extractor(self.file_metadata_extractor.as_ref(), card, report);
    }

    async fn run_identifier_stage(&self, card: &mut BookCard, report: &mut EnrichmentReport) {
        if let Some(doi) = card
            .identifiers
            .as_ref()
            .and_then(|ids| ids.doi.as_deref())
            .and_then(parse_doi)
        {
            match self.crossref.fetch_by_doi(&doi).await {
                Ok(work) => {
                    let fields = card.merge_metadata_with_trace(
                        partial_from_crossref(work),
                        MetadataSource::CrossRef,
                    );
                    if !fields.is_empty() {
                        report.add_fields(fields);
                    }
                    report.add_step("Enriched from CrossRef via DOI");
                    report.add_source("crossref");
                }
                Err(err) => report.add_error(format!("crossref enrichment failed: {err}")),
            }
        }

        if let Some(arxiv_id) = card
            .identifiers
            .as_ref()
            .and_then(|ids| ids.arxiv_id.as_deref())
            .and_then(parse_arxiv)
        {
            match self.arxiv_client.fetch_metadata(&arxiv_id).await {
                Ok(metadata) => {
                    let fields = card.merge_metadata_with_trace(
                        partial_from_arxiv(metadata),
                        MetadataSource::ArxivApi,
                    );
                    if !fields.is_empty() {
                        report.add_fields(fields);
                    }
                    report.add_step("Enriched from arXiv API");
                    report.add_source("arxiv_api");
                }
                Err(err) => report.add_error(format!("arxiv enrichment failed: {err}")),
            }
        }

        if let Some(isbn) = first_isbn(card) {
            match self.openlibrary.fetch_by_isbn(&isbn).await {
                Ok(work) => {
                    let fields = card.merge_metadata_with_trace(
                        partial_from_openlibrary(work, isbn),
                        MetadataSource::OpenLibrary,
                    );
                    if !fields.is_empty() {
                        report.add_fields(fields);
                    }
                    report.add_step("Enriched from Open Library via ISBN");
                    report.add_source("openlibrary");
                }
                Err(err) => report.add_error(format!("openlibrary enrichment failed: {err}")),
            }
        }
    }

    async fn run_semantic_scholar_stage(&self, card: &mut BookCard, report: &mut EnrichmentReport) {
        let Some(s2_id) = semantic_scholar_paper_id(card) else {
            return;
        };

        match self.s2.fetch_paper(&s2_id).await {
            Ok(paper) => {
                let paper_reference_count = paper.reference_count;
                let paper_citation_count = paper.citation_count;
                let now = Utc::now();
                if card.citation_graph.citation_count != paper.citation_count {
                    card.citation_graph.citation_count = paper.citation_count;
                    report.add_fields(vec!["citation_graph.citation_count".to_string()]);
                }
                if card.citation_graph.reference_count != paper.reference_count {
                    card.citation_graph.reference_count = paper.reference_count;
                    report.add_fields(vec!["citation_graph.reference_count".to_string()]);
                }
                if card.citation_graph.influential_citation_count
                    != paper.influential_citation_count
                {
                    card.citation_graph.influential_citation_count =
                        paper.influential_citation_count;
                    report.add_fields(vec![
                        "citation_graph.influential_citation_count".to_string(),
                    ]);
                }
                card.citation_graph.last_updated = Some(now);
                report.add_fields(vec!["citation_graph.last_updated".to_string()]);

                let fields = card.merge_metadata_with_trace(
                    partial_from_semantic_scholar(paper),
                    MetadataSource::SemanticScholar,
                );
                if !fields.is_empty() {
                    report.add_fields(fields);
                }

                if paper_reference_count > 0 {
                    match self.s2.fetch_references(&s2_id).await {
                        Ok(references) => {
                            let sampled = s2_references_to_graph_text(&references);
                            if !sampled.is_empty() {
                                if card.citation_graph.references != sampled {
                                    card.citation_graph.references = sampled;
                                    report
                                        .add_fields(vec!["citation_graph.references".to_string()]);
                                }

                                let sampled_len =
                                    u32::try_from(card.citation_graph.references.len())
                                        .unwrap_or(u32::MAX);
                                if card.citation_graph.reference_count < sampled_len {
                                    card.citation_graph.reference_count = sampled_len;
                                    report.add_fields(vec![
                                        "citation_graph.reference_count".to_string(),
                                    ]);
                                }
                            }
                        }
                        Err(err) => report
                            .add_error(format!("semantic scholar references fetch failed: {err}")),
                    }
                }

                if paper_citation_count > 0 {
                    match self.s2.fetch_citations(&s2_id).await {
                        Ok(citations) => {
                            let sampled = s2_references_to_graph_text(&citations);
                            if !sampled.is_empty() {
                                if card.citation_graph.cited_by_sample != sampled {
                                    card.citation_graph.cited_by_sample = sampled;
                                    report.add_fields(vec![
                                        "citation_graph.cited_by_sample".to_string(),
                                    ]);
                                }

                                let sampled_len =
                                    u32::try_from(card.citation_graph.cited_by_sample.len())
                                        .unwrap_or(u32::MAX);
                                if card.citation_graph.citation_count < sampled_len {
                                    card.citation_graph.citation_count = sampled_len;
                                    report.add_fields(vec![
                                        "citation_graph.citation_count".to_string(),
                                    ]);
                                }
                            }
                        }
                        Err(err) => report
                            .add_error(format!("semantic scholar citations fetch failed: {err}")),
                    }
                }

                report.add_step("Enriched from Semantic Scholar");
                report.add_source("semantic_scholar");
            }
            Err(err) => report.add_error(format!("semantic scholar enrichment failed: {err}")),
        }
    }

    async fn run_references_stage(&self, card: &mut BookCard, report: &mut EnrichmentReport) {
        if !card.citation_graph.references.is_empty() {
            return;
        }

        let extractor = ReferenceExtractor::new(self.crossref.clone(), self.s2.clone());
        match extractor.extract(card).await {
            Ok(references) => {
                if references.is_empty() {
                    return;
                }

                let raw_references = references
                    .iter()
                    .map(reference_to_graph_text)
                    .filter(|value| !value.trim().is_empty())
                    .collect::<Vec<_>>();
                if raw_references.is_empty() {
                    return;
                }

                let mut references_ids = references
                    .iter()
                    .filter_map(|reference| reference.is_in_library)
                    .collect::<Vec<_>>();
                references_ids.sort_unstable();
                references_ids.dedup();

                card.citation_graph.references = raw_references;
                card.citation_graph.references_ids = references_ids;

                let extracted_count =
                    u32::try_from(card.citation_graph.references.len()).unwrap_or(u32::MAX);
                if card.citation_graph.reference_count < extracted_count {
                    card.citation_graph.reference_count = extracted_count;
                }
                card.citation_graph.last_updated = Some(Utc::now());

                report.add_fields(vec![
                    "citation_graph.references".to_string(),
                    "citation_graph.references_ids".to_string(),
                    "citation_graph.reference_count".to_string(),
                    "citation_graph.last_updated".to_string(),
                ]);
                report.add_step("References extracted from available sources");
                report.add_source("reference_extractor");
            }
            Err(err) => report.add_error(format!("reference extraction failed: {err}")),
        }
    }

    async fn run_open_access_stage(&self, card: &mut BookCard, report: &mut EnrichmentReport) {
        let Some(doi) = card
            .identifiers
            .as_ref()
            .and_then(|ids| ids.doi.as_deref())
            .and_then(parse_doi)
        else {
            return;
        };

        match self.unpaywall.check_oa(&doi).await {
            Ok(oa) => {
                let mapped = map_unpaywall_to_open_access(&oa);
                let changed = card.open_access.as_ref() != Some(&mapped);
                if changed {
                    card.open_access = Some(mapped);
                    card.touch();
                    report.add_fields(vec!["open_access".to_string()]);
                }
                report.add_step("Open Access status checked");
                report.add_source("unpaywall");
            }
            Err(err) => report.add_error(format!("unpaywall check failed: {err}")),
        }
    }
}

fn partial_from_crossref(work: CrossRefWork) -> PartialMetadata {
    let authors = work
        .author
        .iter()
        .filter_map(CrossRefAuthor::display_name)
        .collect::<Vec<_>>();

    let isbn = work
        .isbn
        .into_iter()
        .filter_map(|raw| Isbn::parse(raw.as_str()).ok())
        .collect::<Vec<_>>();

    PartialMetadata {
        title: work.title.first().cloned(),
        authors,
        year: work.published_year,
        publisher: work.publisher,
        abstract_text: work.abstract_text,
        doi: Some(work.doi),
        isbn,
        doc_type: Some(map_document_type(work.work_type)),
        journal: work.container_title.first().cloned(),
        ..Default::default()
    }
}

fn partial_from_arxiv(metadata: ArxivMetadata) -> PartialMetadata {
    PartialMetadata {
        title: Some(metadata.title),
        authors: metadata
            .authors
            .into_iter()
            .map(|author| author.name)
            .collect(),
        year: Some(metadata.published.year()),
        abstract_text: Some(metadata.abstract_text),
        doi: metadata.doi,
        arxiv_id: Some(metadata.arxiv_id),
        doc_type: Some(DocumentType::Preprint),
        journal: metadata.journal_ref,
        ..Default::default()
    }
}

fn partial_from_openlibrary(work: OpenLibraryWork, isbn: Isbn) -> PartialMetadata {
    PartialMetadata {
        title: Some(work.title),
        authors: work.authors,
        year: work.publish_date.as_deref().and_then(parse_year_from_text),
        publisher: work.publishers.first().cloned(),
        tags: work.subjects,
        isbn: vec![isbn],
        openlibrary_id: work.openlibrary_id,
        ..Default::default()
    }
}

fn partial_from_semantic_scholar(paper: S2Paper) -> PartialMetadata {
    let doi = lookup_external_id(&paper.external_ids, "DOI").and_then(parse_doi);
    let arxiv_id = lookup_external_id(&paper.external_ids, "ArXiv").and_then(parse_arxiv);
    let pmid = lookup_external_id(&paper.external_ids, "PubMed").map(ToOwned::to_owned);
    let pmcid = lookup_external_id(&paper.external_ids, "PubMedCentral")
        .or_else(|| lookup_external_id(&paper.external_ids, "PMCID"))
        .map(ToOwned::to_owned);
    let mag_id = lookup_external_id(&paper.external_ids, "MAG").map(ToOwned::to_owned);
    let dblp_key = lookup_external_id(&paper.external_ids, "DBLP").map(ToOwned::to_owned);
    let openalex_id = lookup_external_id(&paper.external_ids, "OpenAlex").map(ToOwned::to_owned);

    PartialMetadata {
        title: Some(paper.title),
        authors: paper
            .authors
            .into_iter()
            .map(|author| author.name)
            .collect(),
        year: paper.year,
        abstract_text: paper.abstract_text,
        tldr: paper.tldr.map(|tldr| tldr.text),
        doi,
        arxiv_id,
        pmid,
        pmcid,
        semantic_scholar_id: Some(paper.paper_id),
        openalex_id,
        mag_id,
        dblp_key,
        ..Default::default()
    }
}

fn map_document_type(value: ScienceDocumentType) -> DocumentType {
    match value {
        ScienceDocumentType::Book
        | ScienceDocumentType::Textbook
        | ScienceDocumentType::Monograph
        | ScienceDocumentType::EditedVolume => DocumentType::Book,
        ScienceDocumentType::BookChapter => DocumentType::Chapter,
        ScienceDocumentType::JournalArticle | ScienceDocumentType::ReviewArticle => {
            DocumentType::Article
        }
        ScienceDocumentType::ConferencePaper | ScienceDocumentType::Proceedings => {
            DocumentType::ConferencePaper
        }
        ScienceDocumentType::Preprint | ScienceDocumentType::WorkingPaper => DocumentType::Preprint,
        ScienceDocumentType::TechnicalReport => DocumentType::Report,
        ScienceDocumentType::PhdThesis
        | ScienceDocumentType::MasterThesis
        | ScienceDocumentType::BachelorThesis
        | ScienceDocumentType::Dissertation => DocumentType::Thesis,
        ScienceDocumentType::Standard => DocumentType::Standard,
        ScienceDocumentType::Patent => DocumentType::Patent,
        ScienceDocumentType::Dataset => DocumentType::Dataset,
        ScienceDocumentType::Software => DocumentType::Software,
        ScienceDocumentType::Magazine => DocumentType::MagazineArticle,
        ScienceDocumentType::Webpage | ScienceDocumentType::BlogPost => DocumentType::WebPage,
        _ => DocumentType::Other,
    }
}

fn semantic_scholar_paper_id(card: &BookCard) -> Option<S2PaperId> {
    let identifiers = card.identifiers.as_ref()?;

    if let Some(id) = identifiers
        .semantic_scholar_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(S2PaperId::new(id.to_string()));
    }

    if let Some(doi) = identifiers.doi.as_deref().and_then(parse_doi) {
        return Some(S2PaperId::new(format!("DOI:{}", doi.normalized)));
    }

    identifiers
        .arxiv_id
        .as_deref()
        .and_then(parse_arxiv)
        .map(|arxiv_id| S2PaperId::new(format!("ArXiv:{}", arxiv_id.id)))
}

fn s2_references_to_graph_text(references: &[S2Reference]) -> Vec<String> {
    let mut out = Vec::new();
    for reference in references {
        let Some(value) = s2_reference_to_graph_text(reference) else {
            continue;
        };
        push_unique(&mut out, value);
    }
    out
}

fn s2_reference_to_graph_text(reference: &S2Reference) -> Option<String> {
    let doi = lookup_external_id(&reference.external_ids, "DOI").and_then(parse_doi);
    let arxiv = lookup_external_id(&reference.external_ids, "ArXiv").and_then(parse_arxiv);
    let title = reference
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    if let Some(title) = title {
        if let Some(doi) = doi {
            return Some(format!("{title} DOI:{}", doi.normalized));
        }
        if let Some(arxiv_id) = arxiv {
            return Some(format!(
                "{title} arXiv:{}",
                normalized_arxiv_storage(&arxiv_id)
            ));
        }
        return Some(title);
    }

    if let Some(doi) = doi {
        return Some(format!("DOI:{}", doi.normalized));
    }
    if let Some(arxiv_id) = arxiv {
        return Some(format!("arXiv:{}", normalized_arxiv_storage(&arxiv_id)));
    }

    reference.paper_id.clone()
}

fn normalized_arxiv_storage(arxiv_id: &ArxivId) -> String {
    match arxiv_id.version {
        Some(version) => format!("{}v{version}", arxiv_id.id),
        None => arxiv_id.id.clone(),
    }
}

fn reference_to_graph_text(reference: &ExtractedReference) -> String {
    let resolved_title = reference
        .resolved_title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    if let Some(title) = resolved_title.clone() {
        if let Some(doi) = &reference.doi {
            return format!("{title} DOI:{}", doi.normalized);
        }
        if let Some(arxiv_id) = &reference.arxiv_id {
            return format!("{title} arXiv:{}", normalized_arxiv_storage(arxiv_id));
        }
        return title;
    }

    if let Some(doi) = &reference.doi {
        return format!("DOI:{}", doi.normalized);
    }
    if let Some(arxiv_id) = &reference.arxiv_id {
        return format!("arXiv:{}", normalized_arxiv_storage(arxiv_id));
    }
    reference.raw_text.trim().to_string()
}

fn first_isbn(card: &BookCard) -> Option<Isbn> {
    if let Some(identifiers) = card.identifiers.as_ref() {
        if let Some(isbn13) = identifiers.isbn13.as_deref().and_then(parse_isbn) {
            return Some(isbn13);
        }
        if let Some(isbn10) = identifiers.isbn10.as_deref().and_then(parse_isbn) {
            return Some(isbn10);
        }
    }

    card.metadata
        .isbn
        .iter()
        .find_map(|raw| parse_isbn(raw.as_str()))
}

fn parse_doi(value: &str) -> Option<Doi> {
    Doi::parse(value).ok()
}

fn parse_arxiv(value: &str) -> Option<ArxivId> {
    ArxivId::parse(value).ok()
}

fn parse_isbn(value: &str) -> Option<Isbn> {
    Isbn::parse(value).ok()
}

fn lookup_external_id<'a>(
    ids: &'a std::collections::HashMap<String, String>,
    key: &str,
) -> Option<&'a str> {
    ids.iter()
        .find(|(candidate, _)| candidate.eq_ignore_ascii_case(key))
        .map(|(_, value)| value.as_str())
}

fn map_unpaywall_to_open_access(result: &UnpaywallResult) -> BookOpenAccessInfo {
    let mut pdf_urls = Vec::new();
    if let Some(best_pdf) = result.best_pdf_url() {
        push_unique(&mut pdf_urls, best_pdf.to_string());
    }

    for location in &result.oa_locations {
        if let Some(url) = location.url_for_pdf.as_ref().or(location.url.as_ref()) {
            push_unique(&mut pdf_urls, url.clone());
        }
    }

    let status = result.oa_status.clone();
    let license = result
        .best_oa_location
        .as_ref()
        .and_then(|location| location.license.clone())
        .or_else(|| {
            result
                .oa_locations
                .iter()
                .find_map(|location| location.license.clone())
        });
    let oa_url = result
        .best_oa_location
        .as_ref()
        .and_then(|location| location.url.clone())
        .or_else(|| {
            result
                .oa_locations
                .iter()
                .find_map(|location| location.url.clone())
        });

    BookOpenAccessInfo {
        is_open: result.is_oa,
        status,
        license,
        oa_url,
        pdf_urls,
    }
}

fn run_file_stage_with_extractor(
    extractor: &dyn FileMetadataExtractor,
    card: &mut BookCard,
    report: &mut EnrichmentReport,
) {
    let Some(file) = card.file.clone() else {
        return;
    };
    let file_path = Path::new(file.path.as_str());

    match file.format {
        FileFormat::Pdf => {
            match extractor.extract_pdf_metadata(file_path) {
                Ok(partial) => {
                    let fields =
                        card.merge_metadata_with_trace(partial, MetadataSource::PdfInternal);
                    if !fields.is_empty() {
                        report.add_fields(fields);
                        report.add_step("PDF metadata extracted");
                        report.add_source("pdf_internal");
                    }
                }
                Err(err) => report.add_error(format!("pdf metadata extraction failed: {err}")),
            }

            if card
                .identifiers
                .as_ref()
                .and_then(|ids| ids.doi.as_ref())
                .is_none()
            {
                match find_doi_in_first_page(file_path) {
                    Ok(doi) => {
                        let fields = card.merge_metadata_with_trace(
                            PartialMetadata {
                                doi: Some(doi),
                                ..Default::default()
                            },
                            MetadataSource::PdfInternal,
                        );
                        if !fields.is_empty() {
                            report.add_fields(fields);
                            report.add_step("DOI found in first page text");
                            report.add_source("pdf_internal");
                        }
                    }
                    Err(err) => report.add_error(format!("doi extraction from pdf failed: {err}")),
                }
            }

            if card
                .identifiers
                .as_ref()
                .and_then(|ids| ids.arxiv_id.as_ref())
                .is_none()
            {
                match find_arxiv_id_in_pdf(file_path) {
                    Ok(arxiv_id) => {
                        let fields = card.merge_metadata_with_trace(
                            PartialMetadata {
                                arxiv_id: Some(arxiv_id),
                                ..Default::default()
                            },
                            MetadataSource::PdfInternal,
                        );
                        if !fields.is_empty() {
                            report.add_fields(fields);
                            report.add_step("arXiv ID found in PDF text");
                            report.add_source("pdf_internal");
                        }
                    }
                    Err(err) => {
                        report.add_error(format!("arxiv id extraction from pdf failed: {err}"))
                    }
                }
            }
        }
        FileFormat::Epub => match extractor.extract_epub_metadata(file_path) {
            Ok(partial) => {
                let fields = card.merge_metadata_with_trace(partial, MetadataSource::EpubOpf);
                if !fields.is_empty() {
                    report.add_fields(fields);
                    report.add_step("EPUB OPF metadata extracted");
                    report.add_source("epub_opf");
                }
            }
            Err(err) => report.add_error(format!("epub metadata extraction failed: {err}")),
        },
        FileFormat::Djvu => {
            report.add_step("DjVu metadata extraction skipped");
        }
        _ => {}
    }
}

fn parse_year_from_text(value: &str) -> Option<i32> {
    YEAR_RE
        .find(value)
        .and_then(|m| m.as_str().parse::<i32>().ok())
}

fn extract_pdf_page_count(pdf_path: &Path) -> Result<Option<u32>> {
    match Document::load(pdf_path) {
        Ok(document) => {
            let count = document.get_pages().len();
            if count > 0 {
                return Ok(u32::try_from(count).ok());
            }
        }
        Err(err) => {
            if let Some(pages) = extract_pdf_page_count_with_system_tools(pdf_path) {
                return Ok(Some(pages));
            }
            return Err(ScienceError::PdfExtraction(format!(
                "lopdf failed to open {} for page count: {err}",
                pdf_path.display()
            )));
        }
    }

    Ok(extract_pdf_page_count_with_system_tools(pdf_path))
}

fn extract_pdf_page_count_with_system_tools(pdf_path: &Path) -> Option<u32> {
    extract_pdf_page_count_with_pdfinfo(pdf_path)
        .or_else(|| extract_pdf_page_count_with_qpdf(pdf_path))
        .or_else(|| extract_pdf_page_count_with_mutool(pdf_path))
}

fn extract_pdf_page_count_with_pdfinfo(pdf_path: &Path) -> Option<u32> {
    let output = Command::new("pdfinfo").arg(pdf_path).output().ok()?;
    if !output.status.success() {
        return None;
    }
    parse_pdfinfo_pages_output(&String::from_utf8_lossy(&output.stdout))
}

fn extract_pdf_page_count_with_qpdf(pdf_path: &Path) -> Option<u32> {
    let output = Command::new("qpdf")
        .arg("--show-npages")
        .arg(pdf_path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u32>()
        .ok()
}

fn extract_pdf_page_count_with_mutool(pdf_path: &Path) -> Option<u32> {
    let output = Command::new("mutool")
        .arg("info")
        .arg(pdf_path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_pdfinfo_pages_output(&stdout).or_else(|| parse_mutool_pages_output(&stdout))
}

fn extract_pdf_text_with_lopdf(pdf_path: &Path, max_pages: usize) -> Result<String> {
    if max_pages == 0 {
        return Ok(String::new());
    }

    let document = Document::load(pdf_path).map_err(|err| {
        ScienceError::PdfExtraction(format!(
            "lopdf failed to open {}: {err}",
            pdf_path.display()
        ))
    })?;
    let pages = document.get_pages();
    if pages.is_empty() {
        return Ok(String::new());
    }
    let page_numbers = pages.keys().copied().take(max_pages).collect::<Vec<u32>>();

    document.extract_text(&page_numbers).map_err(|err| {
        ScienceError::PdfExtraction(format!(
            "lopdf failed to extract text from {}: {err}",
            pdf_path.display()
        ))
    })
}

fn extract_pdf_first_page_text_for_title(pdf_path: &Path) -> Result<String> {
    let lopdf_error = match extract_pdf_text_with_lopdf(pdf_path, 1) {
        Ok(text) if !text.trim().is_empty() => return Ok(text),
        Ok(_) => Some("lopdf extracted empty text from first page".to_string()),
        Err(err) => Some(err.to_string()),
    };

    match extract_pdf_text_with_pdftotext(pdf_path, 1, 1, Duration::from_secs(8)) {
        Ok(text) if !text.trim().is_empty() => Ok(text),
        Ok(_) => Err(ScienceError::PdfExtraction(
            "pdftotext extracted empty text from first page".to_string(),
        )),
        Err(pdftotext_error) => {
            let prefix = lopdf_error.unwrap_or_else(|| "lopdf extraction failed".to_string());
            Err(ScienceError::PdfExtraction(format!(
                "{prefix}; {pdftotext_error}"
            )))
        }
    }
}

fn extract_pdf_text_with_pdftotext(
    pdf_path: &Path,
    first_page: usize,
    last_page: usize,
    timeout: Duration,
) -> Result<String> {
    let output_path = std::env::temp_dir().join(format!(
        "omniscope_title_pdftotext_{}_{}.txt",
        std::process::id(),
        Uuid::now_v7()
    ));

    let mut child = Command::new("pdftotext")
        .arg("-f")
        .arg(first_page.to_string())
        .arg("-l")
        .arg(last_page.to_string())
        .arg(pdf_path)
        .arg(&output_path)
        .spawn()
        .map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                ScienceError::PdfExtraction("pdftotext is not installed".to_string())
            } else {
                ScienceError::PdfExtraction(format!("failed to run pdftotext: {err}"))
            }
        })?;

    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if started.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = fs::remove_file(&output_path);
                    return Err(ScienceError::PdfExtraction(
                        "pdftotext timed out while reading title page".to_string(),
                    ));
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(err) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = fs::remove_file(&output_path);
                return Err(ScienceError::PdfExtraction(format!(
                    "pdftotext process failed: {err}"
                )));
            }
        }
    };

    if !status.success() {
        let _ = fs::remove_file(&output_path);
        return Err(ScienceError::PdfExtraction(format!(
            "pdftotext exited with status {status}"
        )));
    }

    let text = fs::read_to_string(&output_path).map_err(|err| {
        ScienceError::PdfExtraction(format!(
            "failed to read pdftotext output {}: {err}",
            output_path.display()
        ))
    })?;
    let _ = fs::remove_file(&output_path);
    Ok(text)
}

fn normalize_inline_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn clean_pdf_metadata_field(raw: &str) -> Option<String> {
    let without_nul = raw.replace('\0', " ");
    let normalized = normalize_inline_whitespace(&without_nul);
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn split_pdf_authors(raw: &str) -> Vec<String> {
    let mut parts = if raw.contains(';') {
        raw.split(';').collect::<Vec<_>>()
    } else if raw.contains('\n') {
        raw.split('\n').collect::<Vec<_>>()
    } else if raw.contains(" and ") {
        raw.split(" and ").collect::<Vec<_>>()
    } else {
        vec![raw]
    };

    if parts.len() == 1 {
        let candidate = raw.trim();
        return if candidate.is_empty() {
            Vec::new()
        } else {
            vec![candidate.to_string()]
        };
    }

    parts
        .drain(..)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn extract_title_from_pdf_text(text: &str) -> Option<String> {
    text.lines()
        .take(50)
        .enumerate()
        .map(|(idx, raw)| (idx, normalize_inline_whitespace(raw)))
        .filter(|(_, line)| is_plausible_pdf_title(line))
        .filter_map(|(idx, line)| {
            let score = score_pdf_title_candidate(&line, idx);
            (score > 0).then_some((score, idx, line))
        })
        .max_by(|(score_a, idx_a, _), (score_b, idx_b, _)| {
            score_a.cmp(score_b).then_with(|| idx_b.cmp(idx_a))
        })
        .map(|(_, _, line)| line)
}

fn pick_best_title_candidate(
    metadata_title: Option<String>,
    text_title: Option<String>,
    filename_title: Option<String>,
) -> Option<String> {
    let mut candidates = Vec::new();

    if let Some(value) = metadata_title {
        candidates.push((score_pdf_title_candidate(&value, 0) + 6, value));
    }
    if let Some(value) = text_title {
        candidates.push((score_pdf_title_candidate(&value, 0) + 16, value));
    }
    if let Some(value) = filename_title {
        candidates.push((score_pdf_title_candidate(&value, 0) + 12, value));
    }

    candidates
        .into_iter()
        .max_by_key(|(score, _)| *score)
        .map(|(_, value)| value)
}

fn extract_title_from_filename(pdf_path: &Path, authors: &[String]) -> Option<String> {
    let stem = pdf_path.file_stem()?.to_string_lossy();
    let raw_tokens = stem
        .split(['_', '-', ' '])
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    if raw_tokens.len() < 2 {
        return None;
    }

    let author_last_names = authors
        .iter()
        .filter_map(|author| author.split_whitespace().last())
        .map(|token| token.to_ascii_lowercase())
        .collect::<Vec<_>>();

    let mut start = 0usize;
    while start < raw_tokens.len().saturating_sub(1)
        && author_last_names.contains(&raw_tokens[start].to_ascii_lowercase())
    {
        start += 1;
    }

    if let Some(first_hint_idx) = raw_tokens
        .iter()
        .position(|token| science_title_hint(token.as_str()))
        && first_hint_idx > start
        && first_hint_idx - start <= 3
        && raw_tokens[start..first_hint_idx]
            .iter()
            .all(|token| looks_like_nameish_token(token))
    {
        start = first_hint_idx;
    }

    if raw_tokens.len() == 2
        && looks_like_nameish_token(raw_tokens[0].as_str())
        && science_title_hint(raw_tokens[1].as_str())
    {
        start = 1;
    }

    let mut title_tokens = raw_tokens.iter().skip(start).cloned().collect::<Vec<_>>();
    trim_trailing_filename_noise(&mut title_tokens);
    if title_tokens.is_empty() {
        return None;
    }

    let title = title_tokens.join(" ");
    let normalized = normalize_inline_whitespace(&title);
    is_plausible_pdf_title(&normalized).then_some(normalized)
}

fn trim_trailing_filename_noise(tokens: &mut Vec<String>) {
    while tokens.len() > 2 {
        let should_trim = tokens
            .last()
            .map(|token| trailing_filename_noise_token(token))
            .unwrap_or(false);
        if !should_trim {
            break;
        }
        let _ = tokens.pop();
    }
}

fn trailing_filename_noise_token(token: &str) -> bool {
    let lowered = token.to_ascii_lowercase();
    matches!(
        lowered.as_str(),
        "paper"
            | "article"
            | "review"
            | "nature"
            | "draft"
            | "final"
            | "preprint"
            | "manuscript"
            | "v1"
            | "v2"
            | "v3"
            | "v4"
            | "v5"
            | "supplement"
            | "supplementary"
    )
}

fn is_plausible_pdf_title(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.len() < 8 || trimmed.len() > 220 {
        return false;
    }
    if REF_MARKER_RE.is_match(trimmed) {
        return false;
    }

    let lower = trimmed.to_ascii_lowercase();
    if REF_PREFIX_RE.is_match(trimmed) {
        return false;
    }
    if lower.starts_with("arxiv:")
        || lower.starts_with("doi:")
        || lower.starts_with("submitted")
        || lower.starts_with("editor:")
        || lower.starts_with("abstract")
        || lower.starts_with("keywords:")
        || lower.starts_with("published as a conference paper")
        || lower.starts_with("accepted at")
        || lower.starts_with("copyright")
        || lower.starts_with("proceedings of")
        || lower.contains("download from")
        || lower.contains("www.")
        || lower.contains("http://")
        || lower.contains("https://")
        || lower.contains("all rights reserved")
        || lower.contains("department of")
        || lower.contains("university of")
    {
        return false;
    }
    if JOURNAL_HEADER_RE.is_match(trimmed) {
        return false;
    }
    if looks_like_author_list(trimmed) {
        return false;
    }

    let word_count = trimmed.split_whitespace().count();
    let upper_token = trimmed
        .chars()
        .filter(|ch| ch.is_ascii_alphabetic())
        .all(|ch| ch.is_ascii_uppercase());
    if upper_token && word_count <= 2 {
        return false;
    }

    let signal_chars = trimmed
        .chars()
        .filter(|ch| ch.is_ascii_digit() || ch.is_ascii_punctuation())
        .count();
    let ratio = signal_chars as f32 / trimmed.chars().count() as f32;
    if ratio >= 0.35 {
        return false;
    }

    true
}

fn score_pdf_title_candidate(value: &str, line_index: usize) -> i32 {
    let mut score = 0i32;
    let trimmed = value.trim();
    let words = trimmed.split_whitespace().count();
    if (2..=14).contains(&words) {
        score += 32;
    } else if (15..=22).contains(&words) {
        score += 18;
    } else {
        score -= 8;
    }

    if line_index <= 24 {
        score += 24 - i32::try_from(line_index).unwrap_or(24);
    }

    if !trimmed.ends_with('.') {
        score += 4;
    } else {
        score -= 4;
    }
    if trimmed.chars().any(|ch| ch.is_ascii_uppercase())
        && trimmed.chars().any(|ch| ch.is_ascii_lowercase())
    {
        score += 4;
    }
    if trimmed.chars().any(|ch| ch == ':') {
        score += 2;
    }
    if trimmed.contains(',') {
        score -= 5;
    }
    if trimmed.contains('&') {
        score -= 6;
    }
    if trimmed.contains('@') {
        score -= 15;
    }
    if trimmed.chars().any(|ch| ch.is_ascii_digit()) {
        score -= 4;
    }
    if looks_like_author_list(trimmed) {
        score -= 36;
    }
    if trimmed
        .chars()
        .all(|ch| !ch.is_ascii_alphabetic() || ch.is_ascii_uppercase())
    {
        score -= 10;
    }

    score
}

fn looks_like_author_list(value: &str) -> bool {
    if value.contains('@') {
        return true;
    }

    let words = value.split_whitespace().collect::<Vec<_>>();
    if words.len() < 2 {
        return false;
    }

    let comma_count = value.matches(',').count();
    let has_and_separator = value.contains(" and ") || value.contains('&');
    let nameish_count = words
        .iter()
        .filter(|word| looks_like_nameish_token(word))
        .count();

    (comma_count >= 1 || has_and_separator) && nameish_count >= 3
}

fn looks_like_nameish_token(value: &str) -> bool {
    let letters = value
        .chars()
        .filter(|ch| ch.is_ascii_alphabetic())
        .collect::<String>();
    if letters.len() < 2 {
        return false;
    }

    let mut chars = letters.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_uppercase() {
        return false;
    }

    chars.any(|ch| ch.is_ascii_lowercase())
}

fn science_title_hint(token: &str) -> bool {
    matches!(
        token.to_ascii_lowercase().as_str(),
        "deep"
            | "learning"
            | "neural"
            | "network"
            | "networks"
            | "recognition"
            | "speech"
            | "dropout"
            | "generative"
            | "adversarial"
            | "memory"
            | "end"
            | "modeling"
            | "model"
            | "models"
            | "review"
            | "attention"
            | "acoustic"
    )
}

fn parse_pdfinfo_pages_output(stdout: &str) -> Option<u32> {
    stdout.lines().find_map(|line| {
        let trimmed = line.trim();
        if !trimmed.to_ascii_lowercase().starts_with("pages:") {
            return None;
        }
        trimmed
            .split(':')
            .nth(1)
            .and_then(|value| value.split_whitespace().next())
            .and_then(|value| value.parse::<u32>().ok())
    })
}

fn parse_mutool_pages_output(stdout: &str) -> Option<u32> {
    stdout.lines().find_map(|line| {
        let trimmed = line.trim();
        if !trimmed.to_ascii_lowercase().starts_with("pages:") {
            return None;
        }
        trimmed
            .split(':')
            .nth(1)
            .and_then(|value| value.split_whitespace().next())
            .and_then(|value| value.parse::<u32>().ok())
    })
}

fn env_first<const N: usize>(keys: [&str; N]) -> Option<String> {
    keys.into_iter()
        .find_map(|key| std::env::var(key).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_container_full_path(container_xml: &str) -> Option<String> {
    let regex = Regex::new(r#"full-path\s*=\s*["']([^"']+)["']"#).ok()?;
    regex
        .captures(container_xml)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

fn read_zip_entry_to_string(archive: &mut ZipArchive<File>, path: &str) -> Result<String> {
    let mut entry = archive
        .by_name(path)
        .map_err(|e| ScienceError::Parse(format!("missing EPUB entry {path}: {e}")))?;
    let mut buffer = String::new();
    entry
        .read_to_string(&mut buffer)
        .map_err(|e| ScienceError::Parse(format!("failed to read EPUB entry {path}: {e}")))?;
    Ok(buffer)
}

fn parse_epub_opf(opf_xml: &str) -> PartialMetadata {
    let title = capture_first_tag(opf_xml, "dc:title");
    let authors = capture_all_tags(opf_xml, "dc:creator");
    let publisher = capture_first_tag(opf_xml, "dc:publisher");
    let language = capture_first_tag(opf_xml, "dc:language");
    let year = capture_first_tag(opf_xml, "dc:date").and_then(|v| parse_year_from_text(&v));
    let mut doi = None;
    let mut arxiv_id = None;
    let mut isbn = Vec::new();

    for identifier in capture_all_tags(opf_xml, "dc:identifier") {
        if doi.is_none() {
            doi = parse_doi(&identifier);
        }
        if arxiv_id.is_none() {
            arxiv_id = parse_arxiv(&identifier);
        }
        if let Some(parsed_isbn) =
            parse_isbn(&identifier).or_else(|| extract_isbn_from_text(&identifier))
        {
            isbn.push(parsed_isbn);
        }
    }

    PartialMetadata {
        title,
        authors,
        year,
        publisher,
        language,
        doi,
        arxiv_id,
        isbn,
        ..Default::default()
    }
}

fn capture_first_tag(xml: &str, tag: &str) -> Option<String> {
    capture_all_tags(xml, tag).into_iter().next()
}

fn capture_all_tags(xml: &str, tag: &str) -> Vec<String> {
    let pattern = format!(r"(?is)<{tag}\b[^>]*>(.*?)</{tag}>");
    let Ok(re) = Regex::new(&pattern) else {
        return Vec::new();
    };

    re.captures_iter(xml)
        .filter_map(|caps| caps.get(1))
        .map(|m| strip_xml_tags(m.as_str()))
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
        .collect()
}

fn strip_xml_tags(value: &str) -> String {
    XML_TAG_RE.replace_all(value, "").to_string()
}

fn push_unique(target: &mut Vec<String>, value: String) {
    if target.iter().any(|existing| existing == &value) {
        return;
    }
    target.push(value);
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use mockito::{Matcher, Server};
    use omniscope_core::models::ScientificIdentifiers;
    use serde_json::json;

    use super::*;

    struct FixtureExtractor {
        pdf: PartialMetadata,
        epub: PartialMetadata,
    }

    impl FixtureExtractor {
        fn new(pdf: PartialMetadata, epub: PartialMetadata) -> Self {
            Self { pdf, epub }
        }
    }

    impl FileMetadataExtractor for FixtureExtractor {
        fn extract_pdf_metadata(&self, _pdf_path: &Path) -> Result<PartialMetadata> {
            Ok(self.pdf.clone())
        }

        fn extract_epub_metadata(&self, _epub_path: &Path) -> Result<PartialMetadata> {
            Ok(self.epub.clone())
        }
    }

    #[tokio::test]
    async fn pipeline_enriches_card_from_all_identifier_sources() {
        let mut server = Server::new_async().await;

        let crossref_mock = server
            .mock("GET", "/works/10.1000%2Ftest")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "message": {
                        "DOI": "10.1000/test",
                        "title": ["CrossRef Title"],
                        "author": [{"given":"Ada","family":"Lovelace"}],
                        "published-print": {"date-parts": [[2021, 6, 1]]},
                        "type": "journal-article",
                        "container-title": ["Journal of Testing"],
                        "publisher": "Test Publisher",
                        "ISBN": ["9780306406157"],
                        "reference-count": 13,
                        "is-referenced-by-count": 34
                    }
                })
                .to_string(),
            )
            .expect(1)
            .create_async()
            .await;

        let arxiv_mock = server
            .mock("GET", "/api/query")
            .match_query(Matcher::UrlEncoded(
                "id_list".to_string(),
                "1706.03762".to_string(),
            ))
            .with_status(200)
            .with_header("content-type", "application/atom+xml")
            .with_body(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:arxiv="http://arxiv.org/schemas/atom">
  <entry>
    <id>http://arxiv.org/abs/1706.03762v7</id>
    <updated>2023-08-02T17:54:37Z</updated>
    <published>2017-06-12T17:57:40Z</published>
    <title>Attention Is All You Need</title>
    <summary>Transformer architecture.</summary>
    <author><name>Ashish Vaswani</name></author>
    <arxiv:doi>10.48550/arXiv.1706.03762</arxiv:doi>
    <link rel="related" type="application/pdf" href="http://arxiv.org/pdf/1706.03762v7" />
    <arxiv:primary_category term="cs.CL" />
    <category term="cs.CL" />
  </entry>
</feed>"#,
            )
            .expect(1)
            .create_async()
            .await;

        let openlibrary_mock = server
            .mock("GET", "/api/books")
            .match_query(Matcher::AllOf(vec![
                Matcher::UrlEncoded("bibkeys".to_string(), "ISBN:9780306406157".to_string()),
                Matcher::UrlEncoded("format".to_string(), "json".to_string()),
                Matcher::UrlEncoded("jscmd".to_string(), "data".to_string()),
            ]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "ISBN:9780306406157": {
                        "title": "Open Library Title",
                        "authors": [{"name":"Ada Lovelace"}],
                        "publish_date": "2020",
                        "publishers": [{"name":"Open Library Publisher"}],
                        "subjects": [{"name":"math"}],
                        "key": "/books/OL123M"
                    }
                })
                .to_string(),
            )
            .expect(1)
            .create_async()
            .await;

        let s2_mock = server
            .mock("GET", "/graph/v1/paper/DOI:10.1000%2Ftest")
            .match_query(Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "paperId": "s2-paper",
                    "externalIds": {
                        "DOI": "10.1000/test",
                        "ArXiv": "1706.03762",
                        "PubMed": "12345678",
                        "MAG": "7654321",
                        "DBLP": "conf/nips/VaswaniSPUJGKP17"
                    },
                    "title": "Attention Is All You Need",
                    "abstract": "Sequence transduction model without recurrence.",
                    "year": 2017,
                    "authors": [{"name":"Ashish Vaswani"}],
                    "citationCount": 12000,
                    "referenceCount": 41,
                    "influentialCitationCount": 2800,
                    "isOpenAccess": true,
                    "openAccessPdf": {"url":"https://arxiv.org/pdf/1706.03762.pdf"},
                    "tldr": {"model":"test","text":"Transformers replace recurrence."}
                })
                .to_string(),
            )
            .expect(1)
            .create_async()
            .await;

        let s2_references_mock = server
            .mock("GET", "/graph/v1/paper/DOI:10.1000%2Ftest/references")
            .match_query(Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": [
                        {
                            "citedPaper": {
                                "paperId": "ref-1",
                                "externalIds": {"DOI": "10.5555/ref.one"},
                                "title": "Resolved reference one",
                                "year": 2020,
                                "authors": [{"name":"Alice"}]
                            }
                        },
                        {
                            "citedPaper": {
                                "paperId": "ref-2",
                                "externalIds": {"ArXiv": "1706.03762"},
                                "title": "Resolved reference two",
                                "year": 2017,
                                "authors": [{"name":"Bob"}]
                            }
                        }
                    ]
                })
                .to_string(),
            )
            .expect(1)
            .create_async()
            .await;

        let s2_citations_mock = server
            .mock("GET", "/graph/v1/paper/DOI:10.1000%2Ftest/citations")
            .match_query(Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": [
                        {
                            "citingPaper": {
                                "paperId": "cit-1",
                                "externalIds": {"DOI": "10.7777/cit.one"},
                                "title": "Citing paper",
                                "year": 2024,
                                "authors": [{"name":"Carol"}]
                            }
                        }
                    ]
                })
                .to_string(),
            )
            .expect(1)
            .create_async()
            .await;

        let unpaywall_mock = server
            .mock("GET", "/v2/10.1000%2Ftest")
            .match_query(Matcher::UrlEncoded(
                "email".to_string(),
                "test@example.com".to_string(),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "doi": "10.1000/test",
                    "is_oa": true,
                    "oa_status": "green",
                    "best_oa_location": {
                        "url": "https://example.org/landing",
                        "url_for_pdf": "https://example.org/paper.pdf",
                        "license": "cc-by-4.0"
                    },
                    "oa_locations": [
                        {"url_for_pdf": "https://mirror.example.org/paper.pdf"}
                    ],
                    "journal_is_oa": false
                })
                .to_string(),
            )
            .expect(1)
            .create_async()
            .await;

        let pipeline = EnrichmentPipeline::new(
            Arc::new(CrossRefSource::new_for_tests(server.url())),
            Arc::new(SemanticScholarSource::new_for_tests(format!(
                "{}/graph/v1",
                server.url()
            ))),
            Arc::new(OpenAlexSource::new_for_tests(server.url())),
            Arc::new(UnpaywallSource::new_for_tests(
                format!("{}/v2", server.url()),
                "test@example.com".to_string(),
            )),
            Arc::new(OpenLibrarySource::new_for_tests(server.url())),
            Arc::new(ArxivClient::new_for_tests(format!(
                "{}/api/query",
                server.url()
            ))),
        );

        let mut card = BookCard::new("Seed");
        card.identifiers = Some(ScientificIdentifiers {
            doi: Some("10.1000/test".to_string()),
            arxiv_id: Some("1706.03762".to_string()),
            isbn13: Some("9780306406157".to_string()),
            ..Default::default()
        });

        let report = pipeline.enrich(&mut card).await;

        crossref_mock.assert_async().await;
        arxiv_mock.assert_async().await;
        openlibrary_mock.assert_async().await;
        s2_mock.assert_async().await;
        s2_references_mock.assert_async().await;
        s2_citations_mock.assert_async().await;
        unpaywall_mock.assert_async().await;

        assert_eq!(card.metadata.title, "CrossRef Title");
        assert_eq!(card.citation_graph.citation_count, 12000);
        assert_eq!(card.citation_graph.reference_count, 41);
        assert_eq!(card.citation_graph.influential_citation_count, 2800);
        assert!(!card.citation_graph.references.is_empty());
        assert!(!card.citation_graph.cited_by_sample.is_empty());
        assert!(card.citation_graph.last_updated.is_some());
        assert_eq!(
            card.ai.tldr.as_deref(),
            Some("Transformers replace recurrence.")
        );
        assert_eq!(
            card.identifiers
                .as_ref()
                .and_then(|ids| ids.pmid.as_deref()),
            Some("12345678")
        );
        assert_eq!(
            card.identifiers
                .as_ref()
                .and_then(|ids| ids.dblp_key.as_deref()),
            Some("conf/nips/VaswaniSPUJGKP17")
        );
        assert_eq!(card.web.openlibrary_id.as_deref(), Some("OL123M"));
        assert!(card.open_access.as_ref().is_some_and(|oa| oa.is_open));
        assert_eq!(
            card.open_access
                .as_ref()
                .and_then(|oa| oa.status.as_deref()),
            Some("green")
        );
        assert!(report.sources_used.contains(&"crossref".to_string()));
        assert!(report.sources_used.contains(&"arxiv_api".to_string()));
        assert!(report.sources_used.contains(&"openlibrary".to_string()));
        assert!(
            report
                .sources_used
                .contains(&"semantic_scholar".to_string())
        );
        assert!(report.sources_used.contains(&"unpaywall".to_string()));
        assert!(report.errors.is_empty());
    }

    #[test]
    fn parse_epub_opf_extracts_core_fields() {
        let opf = r#"
<package xmlns:dc="http://purl.org/dc/elements/1.1/">
  <metadata>
    <dc:title>Attention Is All You Need</dc:title>
    <dc:creator>Ashish Vaswani</dc:creator>
    <dc:publisher>NeurIPS</dc:publisher>
    <dc:date>2017-06-12</dc:date>
    <dc:language>en</dc:language>
    <dc:identifier>10.48550/arXiv.1706.03762</dc:identifier>
    <dc:identifier>ISBN:9780306406157</dc:identifier>
  </metadata>
</package>
"#;

        let parsed = parse_epub_opf(opf);
        assert_eq!(parsed.title.as_deref(), Some("Attention Is All You Need"));
        assert_eq!(parsed.authors, vec!["Ashish Vaswani"]);
        assert_eq!(parsed.publisher.as_deref(), Some("NeurIPS"));
        assert_eq!(parsed.year, Some(2017));
        assert_eq!(
            parsed.doi.as_ref().map(|doi| doi.normalized.as_str()),
            Some("10.48550/arxiv.1706.03762")
        );
        assert_eq!(parsed.isbn.len(), 1);
        assert_eq!(parsed.isbn[0].isbn13, "9780306406157");
    }

    #[test]
    fn pdf_title_heuristics_reject_common_noise_lines() {
        assert!(!is_plausible_pdf_title("LETTER"));
        assert!(!is_plausible_pdf_title("[1]"));
        assert!(!is_plausible_pdf_title(
            "Download from finelybook www.finelybook.com"
        ));
        assert!(!is_plausible_pdf_title(
            "Journal of Machine Learning Research 15 (2014) 1929-1958"
        ));
        assert!(!is_plausible_pdf_title(
            "Published as a conference paper at ICLR 2015"
        ));
    }

    #[test]
    fn pdf_title_heuristics_accepts_real_title_line() {
        assert!(is_plausible_pdf_title(
            "Understanding the difficulty of training deep feedforward neural networks"
        ));
    }

    #[test]
    fn extract_title_from_pdf_text_skips_noise() {
        let text = r#"
Download from finelybook www.finelybook.com
[1]
LETTER
Understanding the difficulty of training deep feedforward neural networks
"#;
        assert_eq!(
            extract_title_from_pdf_text(text).as_deref(),
            Some("Understanding the difficulty of training deep feedforward neural networks")
        );
    }

    #[test]
    fn extract_title_from_pdf_text_ignores_author_line() {
        let text = r#"
REVIEW
doi:10.1038/nature14539
Deep learning
Yann LeCun1,2, Yoshua Bengio3 & Geoffrey Hinton4,5
"#;
        assert_eq!(
            extract_title_from_pdf_text(text).as_deref(),
            Some("Deep learning")
        );
    }

    #[test]
    fn extract_title_from_filename_strips_author_prefix() {
        let path = Path::new("Deep_Learning_1/Goodfellow_Bengio_Deep_Learning.pdf");
        assert_eq!(
            extract_title_from_filename(path, &[]).as_deref(),
            Some("Deep Learning")
        );
    }

    #[test]
    fn extract_title_from_filename_trims_trailing_noise_tokens() {
        let path = Path::new("Deep_Learning_1/LeCun_Bengio_Hinton_Deep_Learning_Nature_Review.pdf");
        assert_eq!(
            extract_title_from_filename(path, &[]).as_deref(),
            Some("Deep Learning")
        );
    }

    #[test]
    fn parse_pdfinfo_output_reads_pages() {
        let output = r#"
Title:          Example
Pages:          524
Encrypted:      no
"#;
        assert_eq!(parse_pdfinfo_pages_output(output), Some(524));
    }

    #[test]
    fn parse_mutool_output_reads_pages() {
        let output = r#"
PDF-1.7
pages: 311
"#;
        assert_eq!(parse_mutool_pages_output(output), Some(311));
    }

    #[tokio::test]
    #[ignore = "requires network and CI_INTEGRATION=1"]
    async fn integration_real_arxiv_1706_03762() {
        if std::env::var("CI_INTEGRATION").ok().as_deref() != Some("1") {
            return;
        }

        let pipeline = EnrichmentPipeline::new(
            Arc::new(CrossRefSource::new(None)),
            Arc::new(SemanticScholarSource::new(None)),
            Arc::new(OpenAlexSource::new()),
            Arc::new(UnpaywallSource::new("ci@example.com".to_string())),
            Arc::new(OpenLibrarySource::new()),
            Arc::new(ArxivClient::new()),
        );

        let mut card = BookCard::new("Seed");
        card.identifiers = Some(ScientificIdentifiers {
            arxiv_id: Some("1706.03762".to_string()),
            ..Default::default()
        });

        let _report = pipeline.enrich(&mut card).await;

        assert!(card.citation_graph.citation_count > 0);
        assert!(card.open_access.as_ref().is_some_and(|oa| oa.is_open));
    }

    #[test]
    fn custom_file_extractor_is_used_for_epub_stage() {
        let mut card = BookCard::new("Seed");
        card.file = Some(omniscope_core::models::BookFile {
            path: "/tmp/fake.epub".to_string(),
            format: FileFormat::Epub,
            size_bytes: 1,
            hash_sha256: None,
            added_at: Utc::now(),
        });

        let pipeline = EnrichmentPipeline::new(
            Arc::new(CrossRefSource::new(None)),
            Arc::new(SemanticScholarSource::new(None)),
            Arc::new(OpenAlexSource::new()),
            Arc::new(UnpaywallSource::new("test@example.com".to_string())),
            Arc::new(OpenLibrarySource::new()),
            Arc::new(ArxivClient::new()),
        )
        .with_file_extractor(Arc::new(FixtureExtractor::new(
            PartialMetadata::default(),
            PartialMetadata {
                title: Some("EPUB title".to_string()),
                authors: vec!["Author".to_string()],
                ..Default::default()
            },
        )));

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let report = runtime.block_on(async {
            let mut working = card.clone();
            let report = pipeline.enrich(&mut working).await;
            assert_eq!(working.metadata.title, "EPUB title");
            assert_eq!(working.metadata.authors, vec!["Author"]);
            report
        });

        assert!(report.steps.iter().any(|step| step.contains("EPUB")));
    }
}
