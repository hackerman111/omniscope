use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{Datelike, Utc};
use omniscope_core::models::{BookCard, BookFile, BookOpenAccessInfo, DocumentType, FileFormat};
use omniscope_core::storage::database::Database;

use crate::arxiv::client::ArxivClient;
use crate::arxiv::types::ArxivMetadata;
use crate::enrichment::merge::{BookCardMergeExt, MetadataSource, PartialMetadata};
use crate::error::{Result, ScienceError};
use crate::http::RateLimitedClient;
use crate::identifiers::arxiv::ArxivId;
use crate::identifiers::doi::Doi;
use crate::identifiers::isbn::Isbn;
use crate::sources::crossref::{CrossRefAuthor, CrossRefSource, CrossRefWork};
use crate::sources::semantic_scholar::{S2Paper, S2PaperId, SemanticScholarSource};
use crate::sources::unpaywall::{UnpaywallResult, UnpaywallSource};
use crate::types::DocumentType as ScienceDocumentType;

const DEFAULT_USER_AGENT: &str = "omniscope-science/0.1";
const ENV_POLITE_EMAIL: &str = "OMNISCOPE_POLITE_EMAIL";
const ENV_SEMANTIC_SCHOLAR_API_KEY: &str = "OMNISCOPE_SEMANTIC_SCHOLAR_API_KEY";
const DUPLICATE_SCAN_PAGE_SIZE: usize = 200;

#[derive(Debug, Clone, Default)]
pub struct ArxivAddOptions {
    pub download_pdf: bool,
    pub download_dir: Option<PathBuf>,
    pub auto_index: bool,
}

#[async_trait]
pub trait ScienceIndexer: Send + Sync {
    async fn index_book(&self, card: &mut BookCard) -> Result<()>;
}

pub struct ArxivAddService {
    arxiv_client: Arc<ArxivClient>,
    crossref: Arc<CrossRefSource>,
    semantic_scholar: Arc<SemanticScholarSource>,
    unpaywall: Option<Arc<UnpaywallSource>>,
    downloader: RateLimitedClient,
    indexer: Option<Arc<dyn ScienceIndexer>>,
}

impl Default for ArxivAddService {
    fn default() -> Self {
        Self::from_env()
    }
}

impl ArxivAddService {
    pub fn from_env() -> Self {
        let polite_email = env_var_non_empty(ENV_POLITE_EMAIL);
        let semantic_scholar_api_key = env_var_non_empty(ENV_SEMANTIC_SCHOLAR_API_KEY);
        Self::new(polite_email, semantic_scholar_api_key)
    }

    pub fn new(polite_email: Option<String>, semantic_scholar_api_key: Option<String>) -> Self {
        let unpaywall = polite_email
            .as_deref()
            .map(|email| UnpaywallSource::new(email.to_string()))
            .map(Arc::new);

        Self {
            arxiv_client: Arc::new(ArxivClient::new()),
            crossref: Arc::new(CrossRefSource::new(polite_email)),
            semantic_scholar: Arc::new(SemanticScholarSource::new(semantic_scholar_api_key)),
            unpaywall,
            downloader: RateLimitedClient::new(Duration::from_millis(300), 3, DEFAULT_USER_AGENT),
            indexer: None,
        }
    }

    pub fn with_indexer(mut self, indexer: Arc<dyn ScienceIndexer>) -> Self {
        self.indexer = Some(indexer);
        self
    }

    #[cfg(test)]
    pub(crate) fn with_clients(
        arxiv_client: Arc<ArxivClient>,
        crossref: Arc<CrossRefSource>,
        semantic_scholar: Arc<SemanticScholarSource>,
        unpaywall: Option<Arc<UnpaywallSource>>,
    ) -> Self {
        Self {
            arxiv_client,
            crossref,
            semantic_scholar,
            unpaywall,
            downloader: RateLimitedClient::new(Duration::from_millis(1), 1, DEFAULT_USER_AGENT),
            indexer: None,
        }
    }

    pub async fn add_from_arxiv(
        &self,
        id: &str,
        opts: ArxivAddOptions,
        db: &Database,
    ) -> Result<BookCard> {
        let arxiv_id = ArxivId::parse(id)?;
        if let Some(existing) = find_existing_card(db, |card| card_matches_arxiv(card, &arxiv_id))?
        {
            return Ok(existing);
        }

        let metadata = self.arxiv_client.fetch_metadata(&arxiv_id).await?;
        if let Some(doi) = metadata.doi.as_ref()
            && let Some(existing) = find_existing_card(db, |card| card_matches_doi(card, doi))?
        {
            return Ok(existing);
        }

        let s2_paper = self
            .semantic_scholar
            .fetch_paper(&S2PaperId::from_arxiv(&arxiv_id))
            .await
            .ok();
        let unpaywall = self.fetch_unpaywall(metadata.doi.as_ref()).await;

        let pdf_url = metadata.pdf_url.clone();
        let mut card = build_card_from_arxiv(metadata, s2_paper, unpaywall);

        if opts.download_pdf {
            card.file = Some(
                self.download_arxiv_pdf(&arxiv_id, &pdf_url, opts.download_dir.as_deref())
                    .await?,
            );
        }

        if opts.auto_index {
            self.run_auto_index(&mut card).await?;
        }

        card.touch();
        db.upsert_book(&card).map_err(map_db_error)?;
        Ok(card)
    }

    pub async fn add_from_doi(
        &self,
        doi: &str,
        opts: ArxivAddOptions,
        db: &Database,
    ) -> Result<BookCard> {
        let doi = Doi::parse(doi)?;
        if let Some(existing) = find_existing_card(db, |card| card_matches_doi(card, &doi))? {
            return Ok(existing);
        }

        let crossref_work = self.crossref.fetch_by_doi(&doi).await?;
        let s2_paper = self
            .semantic_scholar
            .fetch_paper(&S2PaperId::from_doi(&doi))
            .await
            .ok();
        let unpaywall = self.fetch_unpaywall(Some(&doi)).await;

        let mut card = build_card_from_crossref(crossref_work, s2_paper, unpaywall);
        if opts.auto_index {
            self.run_auto_index(&mut card).await?;
        }

        card.touch();
        db.upsert_book(&card).map_err(map_db_error)?;
        Ok(card)
    }

    async fn fetch_unpaywall(&self, doi: Option<&Doi>) -> Option<UnpaywallResult> {
        let doi = doi?;
        let client = self.unpaywall.as_ref()?;
        client.check_oa(doi).await.ok()
    }

    async fn run_auto_index(&self, card: &mut BookCard) -> Result<()> {
        if let Some(indexer) = self.indexer.as_ref() {
            indexer.index_book(card).await?;
        }
        Ok(())
    }

    async fn download_arxiv_pdf(
        &self,
        arxiv_id: &ArxivId,
        pdf_url: &str,
        download_dir: Option<&Path>,
    ) -> Result<BookFile> {
        let dir = resolve_download_dir(download_dir)?;
        tokio::fs::create_dir_all(&dir).await.map_err(|e| {
            ScienceError::Parse(format!("failed to create download directory: {e}"))
        })?;

        let pdf_bytes = self.downloader.get_bytes(pdf_url).await?;
        let file_path = dir.join(download_file_name(arxiv_id));
        tokio::fs::write(&file_path, &pdf_bytes)
            .await
            .map_err(|e| ScienceError::Parse(format!("failed to write downloaded PDF: {e}")))?;

        Ok(BookFile {
            path: file_path.to_string_lossy().to_string(),
            format: FileFormat::Pdf,
            size_bytes: pdf_bytes.len() as u64,
            hash_sha256: None,
            added_at: Utc::now(),
        })
    }
}

pub async fn add_from_arxiv(id: &str, opts: ArxivAddOptions, db: &Database) -> Result<BookCard> {
    ArxivAddService::default()
        .add_from_arxiv(id, opts, db)
        .await
}

pub async fn add_from_doi(doi: &str, opts: ArxivAddOptions, db: &Database) -> Result<BookCard> {
    ArxivAddService::default().add_from_doi(doi, opts, db).await
}

fn build_card_from_arxiv(
    metadata: ArxivMetadata,
    s2_paper: Option<S2Paper>,
    unpaywall: Option<UnpaywallResult>,
) -> BookCard {
    let abs_url = metadata.abs_url.clone();
    let pdf_url = metadata.pdf_url.clone();

    let mut card = BookCard::new(metadata.title.clone());
    card.merge_metadata(partial_from_arxiv(metadata), MetadataSource::ArxivApi);
    push_web_source(&mut card, "arxiv_abs", &abs_url);
    push_web_source(&mut card, "arxiv_pdf", &pdf_url);

    if let Some(paper) = s2_paper {
        merge_semantic_scholar_data(&mut card, paper);
    }
    if let Some(oa) = unpaywall {
        card.open_access = Some(map_unpaywall_to_open_access(&oa));
    }

    card
}

fn build_card_from_crossref(
    work: CrossRefWork,
    s2_paper: Option<S2Paper>,
    unpaywall: Option<UnpaywallResult>,
) -> BookCard {
    let seed_title = work
        .title
        .first()
        .cloned()
        .unwrap_or_else(|| work.doi.normalized.clone());
    let doi_url = work.doi.url.clone();
    let citation_count = work.citation_count;
    let reference_count = work.reference_count;

    let mut card = BookCard::new(seed_title);
    card.merge_metadata(partial_from_crossref(work), MetadataSource::CrossRef);
    if citation_count > 0 {
        card.citation_graph.citation_count = citation_count;
    }
    if reference_count > 0 {
        card.citation_graph.reference_count = reference_count;
    }
    if citation_count > 0 || reference_count > 0 {
        card.citation_graph.last_updated = Some(Utc::now());
    }
    push_web_source(&mut card, "doi", &doi_url);

    if let Some(paper) = s2_paper {
        merge_semantic_scholar_data(&mut card, paper);
    }
    if let Some(oa) = unpaywall {
        card.open_access = Some(map_unpaywall_to_open_access(&oa));
    }

    card
}

fn merge_semantic_scholar_data(card: &mut BookCard, paper: S2Paper) {
    card.citation_graph.citation_count = paper.citation_count;
    card.citation_graph.reference_count = paper.reference_count;
    card.citation_graph.influential_citation_count = paper.influential_citation_count;
    card.citation_graph.last_updated = Some(Utc::now());
    card.merge_metadata(
        partial_from_semantic_scholar(paper),
        MetadataSource::SemanticScholar,
    );
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

fn partial_from_crossref(work: CrossRefWork) -> PartialMetadata {
    let authors = work
        .author
        .iter()
        .filter_map(CrossRefAuthor::display_name)
        .collect::<Vec<_>>();
    let isbn = work
        .isbn
        .into_iter()
        .filter_map(|value| Isbn::parse(&value).ok())
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

fn parse_doi(value: &str) -> Option<Doi> {
    Doi::parse(value).ok()
}

fn parse_arxiv(value: &str) -> Option<ArxivId> {
    ArxivId::parse(value).ok()
}

fn lookup_external_id<'a>(
    ids: &'a std::collections::HashMap<String, String>,
    key: &str,
) -> Option<&'a str> {
    ids.iter()
        .find(|(candidate, _)| candidate.eq_ignore_ascii_case(key))
        .map(|(_, value)| value.as_str())
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

fn map_unpaywall_to_open_access(result: &UnpaywallResult) -> BookOpenAccessInfo {
    let mut pdf_urls = Vec::new();
    if let Some(best_pdf) = result.best_pdf_url() {
        push_unique_string(&mut pdf_urls, best_pdf);
    }
    for location in &result.oa_locations {
        if let Some(url) = location.url_for_pdf.as_deref().or(location.url.as_deref()) {
            push_unique_string(&mut pdf_urls, url);
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

fn push_web_source(card: &mut BookCard, name: &str, url: &str) {
    let name = name.trim();
    let url = url.trim();
    if name.is_empty() || url.is_empty() {
        return;
    }
    if card
        .web
        .sources
        .iter()
        .any(|source| source.name == name && source.url == url)
    {
        return;
    }
    card.web.sources.push(omniscope_core::models::WebSource {
        name: name.to_string(),
        url: url.to_string(),
    });
}

fn push_unique_string(target: &mut Vec<String>, value: &str) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    if !target.iter().any(|existing| existing == value) {
        target.push(value.to_string());
    }
}

fn find_existing_card<F>(db: &Database, mut matcher: F) -> Result<Option<BookCard>>
where
    F: FnMut(&BookCard) -> bool,
{
    let total = db.count_books().map_err(map_db_error)?;
    let mut offset = 0;

    while offset < total {
        let summaries = db
            .list_books(DUPLICATE_SCAN_PAGE_SIZE, offset)
            .map_err(map_db_error)?;
        if summaries.is_empty() {
            break;
        }

        for summary in &summaries {
            let card = db.get_book(&summary.id.to_string()).map_err(map_db_error)?;
            if matcher(&card) {
                return Ok(Some(card));
            }
        }

        offset += summaries.len();
    }

    Ok(None)
}

fn card_matches_arxiv(card: &BookCard, target: &ArxivId) -> bool {
    let Some(raw) = card
        .identifiers
        .as_ref()
        .and_then(|identifiers| identifiers.arxiv_id.as_deref())
    else {
        return false;
    };

    if let Ok(parsed) = ArxivId::parse(raw) {
        return parsed.id == target.id;
    }

    let trimmed = raw.trim();
    if trimmed.eq_ignore_ascii_case(&target.id) {
        return true;
    }

    match target.version {
        Some(version) => trimmed.eq_ignore_ascii_case(&format!("{}v{version}", target.id)),
        None => false,
    }
}

fn card_matches_doi(card: &BookCard, target: &Doi) -> bool {
    let Some(raw) = card
        .identifiers
        .as_ref()
        .and_then(|identifiers| identifiers.doi.as_deref())
    else {
        return false;
    };

    Doi::parse(raw)
        .map(|parsed| parsed.normalized == target.normalized)
        .unwrap_or_else(|_| raw.trim().eq_ignore_ascii_case(&target.normalized))
}

fn map_db_error(err: omniscope_core::error::OmniscopeError) -> ScienceError {
    ScienceError::Parse(format!("database error: {err}"))
}

fn resolve_download_dir(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(path.to_path_buf());
    }
    if let Some(path) = dirs::download_dir() {
        return Ok(path.join("omniscope"));
    }

    std::env::current_dir()
        .map(|path| path.join("downloads"))
        .map_err(|e| ScienceError::Parse(format!("failed to resolve download directory: {e}")))
}

fn download_file_name(arxiv_id: &ArxivId) -> String {
    let raw = match arxiv_id.version {
        Some(version) => format!("{}v{version}", arxiv_id.id),
        None => arxiv_id.id.clone(),
    };
    let sanitized = raw
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '-' | '_' => ch,
            _ => '_',
        })
        .collect::<String>();
    format!("{sanitized}.pdf")
}

fn env_var_non_empty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use mockito::{Matcher, Server};
    use omniscope_core::models::ScientificIdentifiers;
    use serde_json::json;

    use super::*;

    static TMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn build_service(server: &Server, with_unpaywall: bool) -> ArxivAddService {
        let unpaywall = if with_unpaywall {
            Some(Arc::new(UnpaywallSource::new_for_tests(
                format!("{}/v2", server.url()),
                "test@example.com".to_string(),
            )))
        } else {
            None
        };

        ArxivAddService::with_clients(
            Arc::new(ArxivClient::new_for_tests(format!(
                "{}/api/query",
                server.url()
            ))),
            Arc::new(CrossRefSource::new_for_tests(server.url())),
            Arc::new(SemanticScholarSource::new_for_tests(format!(
                "{}/graph/v1",
                server.url()
            ))),
            unpaywall,
        )
    }

    fn make_temp_download_dir() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "omniscope_science_add_test_{}_{}",
            std::process::id(),
            TMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn attention_atom_with_pdf(pdf_url: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom"
      xmlns:arxiv="http://arxiv.org/schemas/atom">
  <entry>
    <id>http://arxiv.org/abs/1706.03762v7</id>
    <updated>2023-08-02T17:54:37Z</updated>
    <published>2017-06-12T17:57:40Z</published>
    <title>Attention Is All You Need</title>
    <summary>Transformer architecture.</summary>
    <author><name>Ashish Vaswani</name></author>
    <arxiv:doi>10.48550/arXiv.1706.03762</arxiv:doi>
    <link rel="related" type="application/pdf" href="{pdf_url}" />
    <arxiv:primary_category term="cs.CL" />
    <category term="cs.CL" />
  </entry>
</feed>"#
        )
    }

    #[tokio::test]
    async fn add_from_arxiv_downloads_pdf_and_persists_card() {
        let mut server = Server::new_async().await;
        let pdf_url = format!("{}/pdf/1706.03762v7", server.url());

        let arxiv_mock = server
            .mock("GET", "/api/query")
            .match_query(Matcher::UrlEncoded(
                "id_list".to_string(),
                "1706.03762".to_string(),
            ))
            .with_status(200)
            .with_header("content-type", "application/atom+xml")
            .with_body(attention_atom_with_pdf(&pdf_url))
            .expect(1)
            .create_async()
            .await;

        let pdf_mock = server
            .mock("GET", "/pdf/1706.03762v7")
            .with_status(200)
            .with_header("content-type", "application/pdf")
            .with_body(vec![0x25, 0x50, 0x44, 0x46]) // %PDF
            .expect(1)
            .create_async()
            .await;

        let s2_mock = server
            .mock("GET", "/graph/v1/paper/ArXiv:1706.03762")
            .match_query(Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "paperId": "s2-paper",
                    "externalIds": {
                        "DOI": "10.48550/arXiv.1706.03762",
                        "ArXiv": "1706.03762",
                        "OpenAlex": "W123"
                    },
                    "title": "Attention Is All You Need",
                    "year": 2017,
                    "authors": [{"name":"Ashish Vaswani"}],
                    "citationCount": 12000,
                    "referenceCount": 41,
                    "influentialCitationCount": 2800,
                    "isOpenAccess": true,
                    "tldr": {"model":"test","text":"Transformers replace recurrence."}
                })
                .to_string(),
            )
            .expect(1)
            .create_async()
            .await;

        let unpaywall_mock = server
            .mock("GET", "/v2/10.48550%2Farxiv.1706.03762")
            .match_query(Matcher::UrlEncoded(
                "email".to_string(),
                "test@example.com".to_string(),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "doi": "10.48550/arXiv.1706.03762",
                    "is_oa": true,
                    "oa_status": "green",
                    "best_oa_location": {
                        "url": "https://arxiv.org/abs/1706.03762",
                        "url_for_pdf": "https://arxiv.org/pdf/1706.03762.pdf",
                        "license": "cc-by-4.0"
                    },
                    "oa_locations": []
                })
                .to_string(),
            )
            .expect(1)
            .create_async()
            .await;

        let service = build_service(&server, true);
        let db = Database::open_in_memory().unwrap();
        let download_dir = make_temp_download_dir();

        let card = service
            .add_from_arxiv(
                "1706.03762",
                ArxivAddOptions {
                    download_pdf: true,
                    download_dir: Some(download_dir.clone()),
                    auto_index: false,
                },
                &db,
            )
            .await
            .unwrap();

        arxiv_mock.assert_async().await;
        pdf_mock.assert_async().await;
        s2_mock.assert_async().await;
        unpaywall_mock.assert_async().await;

        assert_eq!(card.metadata.title, "Attention Is All You Need");
        assert_eq!(
            card.identifiers
                .as_ref()
                .and_then(|ids| ids.arxiv_id.as_deref()),
            Some("1706.03762v7")
        );
        assert_eq!(card.citation_graph.citation_count, 12000);
        assert_eq!(card.citation_graph.reference_count, 41);
        assert!(card.open_access.as_ref().is_some_and(|oa| oa.is_open));
        assert_eq!(
            card.ai.tldr.as_deref(),
            Some("Transformers replace recurrence.")
        );
        assert!(
            card.file
                .as_ref()
                .is_some_and(|file| file.path.ends_with(".pdf"))
        );
        assert!(
            card.file
                .as_ref()
                .is_some_and(|file| std::path::Path::new(&file.path).exists())
        );

        let stored = db.get_book(&card.id.to_string()).unwrap();
        assert_eq!(stored.metadata.title, card.metadata.title);

        let _ = std::fs::remove_dir_all(download_dir);
    }

    #[tokio::test]
    async fn add_from_arxiv_returns_existing_duplicate_without_network() {
        let server = Server::new_async().await;
        let service = build_service(&server, false);
        let db = Database::open_in_memory().unwrap();

        let mut existing = BookCard::new("Existing");
        existing.identifiers = Some(ScientificIdentifiers {
            arxiv_id: Some("1706.03762v1".to_string()),
            ..Default::default()
        });
        db.upsert_book(&existing).unwrap();

        let returned = service
            .add_from_arxiv("1706.03762", ArxivAddOptions::default(), &db)
            .await
            .unwrap();

        assert_eq!(returned.id, existing.id);
        assert_eq!(db.count_books().unwrap(), 1);
    }

    #[tokio::test]
    async fn add_from_doi_enriches_from_crossref_semantic_scholar_and_unpaywall() {
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
                        "PubMed": "12345678"
                    },
                    "title": "CrossRef Title",
                    "year": 2021,
                    "authors": [{"name":"Ada Lovelace"}],
                    "citationCount": 77,
                    "referenceCount": 12,
                    "influentialCitationCount": 9,
                    "isOpenAccess": true
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
                    "oa_status": "gold",
                    "best_oa_location": {
                        "url": "https://example.org/landing",
                        "url_for_pdf": "https://example.org/file.pdf",
                        "license": "cc-by-4.0"
                    },
                    "oa_locations": []
                })
                .to_string(),
            )
            .expect(1)
            .create_async()
            .await;

        let service = build_service(&server, true);
        let db = Database::open_in_memory().unwrap();

        let card = service
            .add_from_doi(
                "10.1000/test",
                ArxivAddOptions {
                    download_pdf: true,
                    download_dir: None,
                    auto_index: false,
                },
                &db,
            )
            .await
            .unwrap();

        crossref_mock.assert_async().await;
        s2_mock.assert_async().await;
        unpaywall_mock.assert_async().await;

        assert_eq!(card.metadata.title, "CrossRef Title");
        assert_eq!(card.metadata.year, Some(2021));
        assert_eq!(card.citation_graph.citation_count, 77);
        assert_eq!(card.citation_graph.reference_count, 12);
        assert_eq!(
            card.identifiers
                .as_ref()
                .and_then(|ids| ids.pmid.as_deref()),
            Some("12345678")
        );
        assert!(card.open_access.as_ref().is_some_and(|oa| oa.is_open));
        assert!(card.file.is_none());
    }

    #[tokio::test]
    async fn add_from_arxiv_keeps_working_when_s2_and_unpaywall_are_unavailable() {
        let mut server = Server::new_async().await;
        let pdf_url = format!("{}/pdf/1706.03762v7", server.url());

        let arxiv_mock = server
            .mock("GET", "/api/query")
            .match_query(Matcher::UrlEncoded(
                "id_list".to_string(),
                "1706.03762".to_string(),
            ))
            .with_status(200)
            .with_header("content-type", "application/atom+xml")
            .with_body(attention_atom_with_pdf(&pdf_url))
            .expect(1)
            .create_async()
            .await;

        let s2_mock = server
            .mock("GET", "/graph/v1/paper/ArXiv:1706.03762")
            .match_query(Matcher::Any)
            .with_status(500)
            .with_body("down")
            .expect(1)
            .create_async()
            .await;

        let unpaywall_mock = server
            .mock("GET", "/v2/10.48550%2Farxiv.1706.03762")
            .match_query(Matcher::UrlEncoded(
                "email".to_string(),
                "test@example.com".to_string(),
            ))
            .with_status(503)
            .with_body("down")
            .expect(1)
            .create_async()
            .await;

        let service = build_service(&server, true);
        let db = Database::open_in_memory().unwrap();
        let card = service
            .add_from_arxiv("1706.03762", ArxivAddOptions::default(), &db)
            .await
            .unwrap();

        arxiv_mock.assert_async().await;
        s2_mock.assert_async().await;
        unpaywall_mock.assert_async().await;

        assert_eq!(card.metadata.title, "Attention Is All You Need");
        assert!(card.open_access.is_none());
    }

    #[tokio::test]
    async fn integration_add_from_arxiv_real_api_when_enabled() {
        if std::env::var("CI_INTEGRATION").ok().as_deref() != Some("1") {
            return;
        }

        let db = Database::open_in_memory().unwrap();
        let service = ArxivAddService::from_env();
        let card = service
            .add_from_arxiv("1706.03762", ArxivAddOptions::default(), &db)
            .await
            .unwrap();

        assert!(card.metadata.title.to_lowercase().contains("attention"));
    }
}
