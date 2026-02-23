use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use omniscope_core::models::{BookCard, FileFormat};
use uuid::Uuid;

use crate::error::{Result, ScienceError};
use crate::identifiers::{
    arxiv::ArxivId,
    doi::Doi,
    extract::{extract_arxiv_ids_from_text, extract_dois_from_text, extract_isbn_from_text},
};
use crate::references::parser::{find_references_section, parse_reference_lines};
use crate::references::resolver::{ExtractedReference, ResolutionMethod, resolve_unidentified};
use crate::sources::crossref::CrossRefSource;
use crate::sources::semantic_scholar::{S2PaperId, S2Reference, SemanticScholarSource};

pub trait LibraryLookup: Send + Sync {
    fn find_by_doi(&self, doi: &Doi) -> Option<Uuid>;
    fn find_by_arxiv(&self, arxiv_id: &ArxivId) -> Option<Uuid>;
}

pub trait PdfTextExtractor: Send + Sync {
    fn extract_text(&self, pdf_path: &Path) -> Result<String>;
}

struct PdftotextExtractor;

impl PdfTextExtractor for PdftotextExtractor {
    fn extract_text(&self, pdf_path: &Path) -> Result<String> {
        let output = Command::new("pdftotext")
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

        String::from_utf8(output.stdout).map_err(|e| {
            ScienceError::PdfExtraction(format!("pdftotext returned non-UTF8 output: {e}"))
        })
    }
}

#[derive(Clone)]
pub struct ReferenceExtractor {
    pub crossref: Arc<CrossRefSource>,
    pub s2: Arc<SemanticScholarSource>,
    library_lookup: Option<Arc<dyn LibraryLookup>>,
    pdf_text_extractor: Arc<dyn PdfTextExtractor>,
}

impl ReferenceExtractor {
    pub fn new(crossref: Arc<CrossRefSource>, s2: Arc<SemanticScholarSource>) -> Self {
        Self {
            crossref,
            s2,
            library_lookup: None,
            pdf_text_extractor: Arc::new(PdftotextExtractor),
        }
    }

    pub fn with_library_lookup(mut self, lookup: Arc<dyn LibraryLookup>) -> Self {
        self.library_lookup = Some(lookup);
        self
    }

    pub fn with_pdf_extractor(mut self, extractor: Arc<dyn PdfTextExtractor>) -> Self {
        self.pdf_text_extractor = extractor;
        self
    }

    pub async fn extract(&self, card: &BookCard) -> Result<Vec<ExtractedReference>> {
        if let Some(paper_id) = semantic_scholar_paper_id(card)
            && let Ok(mut references) = self.extract_from_semantic_scholar(&paper_id).await
            && !references.is_empty()
        {
            self.mark_in_library(&mut references);
            return Ok(references);
        }

        if let Some(pdf_path) = pdf_path_from_card(card) {
            let mut references = self.extract_from_pdf(pdf_path).await?;
            self.mark_in_library(&mut references);
            return Ok(references);
        }

        Ok(Vec::new())
    }

    async fn extract_from_semantic_scholar(
        &self,
        paper_id: &S2PaperId,
    ) -> Result<Vec<ExtractedReference>> {
        let references = self.s2.fetch_references(paper_id).await?;
        Ok(references
            .into_iter()
            .enumerate()
            .map(|(idx, reference)| map_s2_reference(idx + 1, reference))
            .collect())
    }

    async fn extract_from_pdf(&self, pdf_path: &Path) -> Result<Vec<ExtractedReference>> {
        let text = self.pdf_text_extractor.extract_text(pdf_path)?;
        let Some(section) = find_references_section(&text) else {
            return Ok(Vec::new());
        };

        let mut references = parse_reference_lines(section)
            .into_iter()
            .enumerate()
            .map(|(idx, raw)| map_raw_reference(idx + 1, raw))
            .collect::<Vec<_>>();

        resolve_unidentified(&mut references, self.crossref.as_ref()).await?;
        Ok(references)
    }

    fn mark_in_library(&self, references: &mut [ExtractedReference]) {
        let Some(lookup) = &self.library_lookup else {
            return;
        };

        for reference in references.iter_mut() {
            if let Some(doi) = &reference.doi
                && let Some(book_id) = lookup.find_by_doi(doi)
            {
                reference.is_in_library = Some(book_id);
                continue;
            }
            if let Some(arxiv_id) = &reference.arxiv_id {
                reference.is_in_library = lookup.find_by_arxiv(arxiv_id);
            }
        }
    }
}

fn map_raw_reference(index: usize, raw: String) -> ExtractedReference {
    let mut reference = ExtractedReference::from_raw(index, raw.clone());
    reference.doi = extract_dois_from_text(&raw).into_iter().next();
    reference.arxiv_id = extract_arxiv_ids_from_text(&raw).into_iter().next();
    reference.isbn = extract_isbn_from_text(&raw);
    reference
}

fn map_s2_reference(index: usize, reference: S2Reference) -> ExtractedReference {
    let doi = lookup_external_id(&reference.external_ids, "DOI").and_then(parse_doi);
    let arxiv_id = lookup_external_id(&reference.external_ids, "ArXiv").and_then(parse_arxiv_id);

    let raw_text = reference
        .title
        .clone()
        .or_else(|| {
            doi.as_ref()
                .map(|parsed| format!("DOI:{}", parsed.normalized))
        })
        .or_else(|| {
            arxiv_id
                .as_ref()
                .map(|parsed| format!("arXiv:{}", normalized_arxiv_key(parsed)))
        })
        .or(reference.paper_id.clone())
        .unwrap_or_else(|| format!("Reference #{index}"));

    let mut extracted = ExtractedReference::from_raw(index, raw_text);
    extracted.doi = doi;
    extracted.arxiv_id = arxiv_id;
    extracted.resolved_title = reference.title;
    extracted.resolved_authors = reference
        .authors
        .into_iter()
        .map(|author| author.name.trim().to_string())
        .filter(|name| !name.is_empty())
        .collect();
    extracted.resolved_year = reference.year;
    extracted.confidence = 1.0;
    extracted.resolution_method = ResolutionMethod::SemanticScholar;
    extracted
}

fn semantic_scholar_paper_id(card: &BookCard) -> Option<S2PaperId> {
    let identifiers = card.identifiers.as_ref()?;

    if let Some(id) = identifiers
        .semantic_scholar_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return Some(S2PaperId::new(id.to_string()));
    }

    if let Some(doi) = identifiers.doi.as_deref().and_then(parse_doi) {
        return Some(S2PaperId::new(format!("DOI:{}", doi.normalized)));
    }

    identifiers
        .arxiv_id
        .as_deref()
        .and_then(parse_arxiv_id)
        .map(|arxiv_id| S2PaperId::new(format!("ArXiv:{}", normalized_arxiv_key(&arxiv_id))))
}

fn pdf_path_from_card(card: &BookCard) -> Option<&Path> {
    let file = card.file.as_ref()?;
    if file.format != FileFormat::Pdf {
        return None;
    }
    Some(Path::new(file.path.as_str()))
}

fn lookup_external_id<'a>(external_ids: &'a HashMap<String, String>, key: &str) -> Option<&'a str> {
    external_ids
        .iter()
        .find(|(candidate, _)| candidate.eq_ignore_ascii_case(key))
        .map(|(_, value)| value.as_str())
}

fn parse_doi(raw: &str) -> Option<Doi> {
    Doi::parse(raw).ok()
}

fn parse_arxiv_id(raw: &str) -> Option<ArxivId> {
    ArxivId::parse(raw).ok()
}

fn normalized_arxiv_key(arxiv_id: &ArxivId) -> String {
    match arxiv_id.version {
        Some(version) => format!("{}v{version}", arxiv_id.id),
        None => arxiv_id.id.clone(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::Utc;
    use mockito::{Matcher, Server};
    use omniscope_core::models::{BookFile, ScientificIdentifiers};
    use serde_json::json;

    use super::*;

    struct FixturePdfTextExtractor {
        text: String,
    }

    impl FixturePdfTextExtractor {
        fn new(text: impl Into<String>) -> Self {
            Self { text: text.into() }
        }
    }

    impl PdfTextExtractor for FixturePdfTextExtractor {
        fn extract_text(&self, _pdf_path: &Path) -> Result<String> {
            Ok(self.text.clone())
        }
    }

    #[derive(Default)]
    struct InMemoryLookup {
        doi_to_book: HashMap<String, Uuid>,
        arxiv_to_book: HashMap<String, Uuid>,
    }

    impl LibraryLookup for InMemoryLookup {
        fn find_by_doi(&self, doi: &Doi) -> Option<Uuid> {
            self.doi_to_book.get(&doi.normalized).copied()
        }

        fn find_by_arxiv(&self, arxiv_id: &ArxivId) -> Option<Uuid> {
            self.arxiv_to_book
                .get(&normalized_arxiv_key(arxiv_id))
                .copied()
        }
    }

    #[tokio::test]
    async fn uses_semantic_scholar_references_first_when_identifier_exists() {
        let mut server = Server::new_async().await;
        let body = json!({
            "data": [
                {
                    "citedPaper": {
                        "paperId": "s2-ref-1",
                        "externalIds": {"DOI": "10.5555/ref.one"},
                        "title": "A Semantically Resolved Reference",
                        "year": 2020,
                        "authors": [{"name": "Alice Doe"}]
                    }
                },
                {
                    "citedPaper": {
                        "paperId": "s2-ref-2",
                        "externalIds": {"ArXiv": "1706.03762"},
                        "title": "Attention Is All You Need",
                        "year": 2017,
                        "authors": [{"name": "Ashish Vaswani"}]
                    }
                }
            ]
        });

        let mock = server
            .mock("GET", "/graph/v1/paper/abc123/references")
            .match_query(Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body.to_string())
            .create_async()
            .await;

        let s2 = Arc::new(SemanticScholarSource::new_for_tests(format!(
            "{}/graph/v1",
            server.url()
        )));
        let crossref = Arc::new(CrossRefSource::new(None));
        let extractor = ReferenceExtractor::new(crossref, s2);

        let mut card = BookCard::new("Paper Under Test");
        card.identifiers = Some(ScientificIdentifiers {
            semantic_scholar_id: Some("abc123".to_string()),
            ..Default::default()
        });

        let references = extractor.extract(&card).await.unwrap();
        mock.assert_async().await;

        assert_eq!(references.len(), 2);
        assert_eq!(
            references[0].resolution_method,
            ResolutionMethod::SemanticScholar
        );
        assert_eq!(
            references[0]
                .doi
                .as_ref()
                .map(|doi| doi.normalized.as_str()),
            Some("10.5555/ref.one")
        );
        assert_eq!(references[1].resolved_year, Some(2017));
        assert!(references[1].arxiv_id.is_some());
    }

    #[tokio::test]
    async fn extracts_pdf_references_and_marks_library_matches() {
        let pdf_text = r#"
Introduction
References
[1] Vaswani, A. et al. Attention Is All You Need. arXiv:1706.03762.
[2] Brown, T. et al. Language Models are Few-Shot Learners. DOI:10.48550/arXiv.2005.14165.

Appendix
Supplementary details that must not be parsed as references.
"#;

        let mut lookup = InMemoryLookup::default();
        let arxiv_book = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let doi_book = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();
        lookup
            .arxiv_to_book
            .insert("1706.03762".to_string(), arxiv_book);
        lookup
            .doi_to_book
            .insert("10.48550/arxiv.2005.14165".to_string(), doi_book);

        let extractor = ReferenceExtractor::new(
            Arc::new(CrossRefSource::new(None)),
            Arc::new(SemanticScholarSource::new(None)),
        )
        .with_pdf_extractor(Arc::new(FixturePdfTextExtractor::new(pdf_text)))
        .with_library_lookup(Arc::new(lookup));

        let mut card = BookCard::new("PDF Paper");
        card.file = Some(BookFile {
            path: "/tmp/paper.pdf".to_string(),
            format: FileFormat::Pdf,
            size_bytes: 1024,
            hash_sha256: None,
            added_at: Utc::now(),
        });

        let references = extractor.extract(&card).await.unwrap();

        assert_eq!(references.len(), 2);
        assert_eq!(
            references[0].resolution_method,
            ResolutionMethod::DirectArxiv
        );
        assert_eq!(references[0].is_in_library, Some(arxiv_book));
        assert_eq!(references[1].resolution_method, ResolutionMethod::DirectDoi);
        assert_eq!(references[1].is_in_library, Some(doi_book));
    }
}
