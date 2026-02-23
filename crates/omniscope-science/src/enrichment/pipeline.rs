use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use chrono::{Datelike, Utc};
use omniscope_core::models::{BookCard, BookOpenAccessInfo, DocumentType, FileFormat};
use once_cell::sync::Lazy;
use regex::Regex;
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
use crate::sources::crossref::{CrossRefAuthor, CrossRefSource, CrossRefWork};
use crate::sources::openalex::OpenAlexSource;
use crate::sources::openlibrary::{OpenLibrarySource, OpenLibraryWork};
use crate::sources::semantic_scholar::{S2Paper, S2PaperId, SemanticScholarSource};
use crate::sources::unpaywall::{UnpaywallResult, UnpaywallSource};
use crate::types::DocumentType as ScienceDocumentType;

static YEAR_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b(19|20)\d{2}\b").expect("valid regex"));
static XML_TAG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?is)<[^>]+>").expect("valid regex"));

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
        let output = Command::new("pdftotext")
            .arg("-f")
            .arg("1")
            .arg("-l")
            .arg("1")
            .arg(pdf_path)
            .arg("-")
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    ScienceError::PdfExtraction("pdftotext is not installed".to_string())
                } else {
                    ScienceError::PdfExtraction(format!("failed to run pdftotext: {e}"))
                }
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let message = if stderr.is_empty() {
                "pdftotext failed without stderr output".to_string()
            } else {
                format!("pdftotext failed: {stderr}")
            };
            return Err(ScienceError::PdfExtraction(message));
        }

        let text = String::from_utf8(output.stdout).map_err(|e| {
            ScienceError::PdfExtraction(format!("pdftotext returned non-UTF8 output: {e}"))
        })?;

        let title = text
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .map(ToOwned::to_owned);
        let year = parse_year_from_text(&text);

        Ok(PartialMetadata {
            title,
            year,
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

    pub async fn enrich(&self, card: &mut BookCard) -> EnrichmentReport {
        let mut report = EnrichmentReport::default();

        self.run_file_stage(card, &mut report);
        self.run_identifier_stage(card, &mut report).await;
        self.run_semantic_scholar_stage(card, &mut report).await;
        self.run_open_access_stage(card, &mut report).await;

        report
    }

    fn run_file_stage(&self, card: &mut BookCard, report: &mut EnrichmentReport) {
        let Some(file) = card.file.clone() else {
            return;
        };
        let file_path = Path::new(file.path.as_str());

        match file.format {
            FileFormat::Pdf => {
                match self.file_metadata_extractor.extract_pdf_metadata(file_path) {
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
                        Err(err) => {
                            report.add_error(format!("doi extraction from pdf failed: {err}"))
                        }
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
            FileFormat::Epub => match self
                .file_metadata_extractor
                .extract_epub_metadata(file_path)
            {
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

                report.add_step("Enriched from Semantic Scholar");
                report.add_source("semantic_scholar");
            }
            Err(err) => report.add_error(format!("semantic scholar enrichment failed: {err}")),
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

fn parse_year_from_text(value: &str) -> Option<i32> {
    YEAR_RE
        .find(value)
        .and_then(|m| m.as_str().parse::<i32>().ok())
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
        unpaywall_mock.assert_async().await;

        assert_eq!(card.metadata.title, "CrossRef Title");
        assert_eq!(card.citation_graph.citation_count, 12000);
        assert_eq!(card.citation_graph.reference_count, 41);
        assert_eq!(card.citation_graph.influential_citation_count, 2800);
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
