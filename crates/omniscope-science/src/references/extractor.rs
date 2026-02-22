use std::path::Path;
use std::sync::Arc;
use std::process::Command;
use crate::error::{Result, ScienceError};
use crate::sources::crossref::CrossRefSource;
use crate::sources::semantic_scholar::{SemanticScholarSource, S2PaperRef};
use crate::references::parser::{find_references_section, parse_reference_lines};
use crate::references::resolver::{resolve_unidentified, ExtractedReference, ResolutionMethod};
use crate::identifiers::extract::{extract_dois_from_text, extract_arxiv_ids_from_text, extract_isbn_from_text};
use crate::identifiers::doi::Doi;
use crate::identifiers::arxiv::ArxivId;
use omniscope_core::models::book::{BookCard, FileFormat};

pub struct ReferenceExtractor {
    crossref: Arc<CrossRefSource>,
    s2: Arc<SemanticScholarSource>,
}

impl ReferenceExtractor {
    pub fn new(crossref: Arc<CrossRefSource>, s2: Arc<SemanticScholarSource>) -> Self {
        Self { crossref, s2 }
    }

    pub async fn extract(&self, card: &BookCard) -> Result<Vec<ExtractedReference>> {
        // 1. Try API fetch via Semantic Scholar if identifiers exist
        if let Some(ids) = &card.identifiers {
            let id_str = if let Some(s2_id) = &ids.semantic_scholar_id {
                Some(s2_id.clone())
            } else if let Some(doi_str) = &ids.doi {
                // Try to parse to normalize, or use as is
                if let Ok(d) = Doi::parse(doi_str) {
                    Some(format!("DOI:{}", d.normalized))
                } else {
                    Some(format!("DOI:{}", doi_str)) // Fallback
                }
            } else if let Some(arxiv_str) = &ids.arxiv_id {
                if let Ok(a) = ArxivId::parse(arxiv_str) {
                    Some(format!("ArXiv:{}", a.id))
                } else {
                    Some(format!("ArXiv:{}", arxiv_str))
                }
            } else {
                None
            };

            if let Some(id) = id_str
                && let Ok(paper) = self.s2.fetch_paper(&id).await
                && !paper.references.is_empty()
            {
                let mut extracted = Vec::new();
                for (i, ref_paper) in paper.references.into_iter().enumerate() {
                    extracted.push(self.s2_ref_to_extracted(i + 1, ref_paper));
                }
                return Ok(extracted);
            }
        }

        // 2. Extract from PDF
        if let Some(file) = &card.file
            && file.format == FileFormat::Pdf
        {
            return self.extract_from_pdf(Path::new(&file.path)).await;
        }

        Ok(Vec::new())
    }

    fn s2_ref_to_extracted(&self, index: usize, r: S2PaperRef) -> ExtractedReference {
        let authors: Vec<String> = r.authors.iter().map(|a| a.name.clone()).collect();
        let raw_text = format!(
            "{} ({}) - {}", 
            authors.first().map(|s| s.as_str()).unwrap_or("Unknown"),
            r.year.unwrap_or(0),
            r.title.clone().unwrap_or_default()
        );

        let doi = r.external_ids.get("DOI").and_then(|s| Doi::parse(s).ok());
        let arxiv_id = r.external_ids.get("ArXiv").and_then(|s| ArxivId::parse(s).ok());

        ExtractedReference {
            index,
            raw_text,
            doi,
            arxiv_id,
            isbn: None,
            resolved_title: r.title,
            resolved_authors: authors,
            resolved_year: r.year,
            confidence: 1.0,
            resolution_method: ResolutionMethod::SemanticScholar,
            is_in_library: None,
        }
    }

    async fn extract_from_pdf(&self, path: &Path) -> Result<Vec<ExtractedReference>> {
        let output = Command::new("pdftotext")
            .arg("-layout")
            .arg(path)
            .arg("-")
            .output()
            .map_err(|e| ScienceError::PdfExtraction(format!("pdftotext execution failed: {}", e)))?;

        if !output.status.success() {
            return Err(ScienceError::PdfExtraction("pdftotext returned non-zero exit code".to_string()));
        }

        let text = String::from_utf8_lossy(&output.stdout);

        let section = find_references_section(&text)
            .ok_or_else(|| ScienceError::PdfExtraction("References section not found".to_string()))?;

        let raw_refs = parse_reference_lines(section);
        
        let mut extracted = Vec::new();
        for (i, raw) in raw_refs.into_iter().enumerate() {
            let mut r = ExtractedReference::from_raw(i + 1, raw.clone());
            
            if let Some(doi) = extract_dois_from_text(&raw).into_iter().next() {
                r.doi = Some(doi);
                r.resolution_method = ResolutionMethod::DirectDoi;
                r.confidence = 1.0;
            }
            
            if let Some(arxiv) = extract_arxiv_ids_from_text(&raw).into_iter().next() {
                r.arxiv_id = Some(arxiv);
                r.resolution_method = ResolutionMethod::DirectArxiv;
                r.confidence = 1.0;
            }
            
            r.isbn = extract_isbn_from_text(&raw);
            
            extracted.push(r);
        }

        resolve_unidentified(&mut extracted, &self.crossref).await;

        Ok(extracted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;
    use omniscope_core::models::ScientificIdentifiers;
    use std::time::Duration;

    #[tokio::test]
    async fn test_extract_via_s2() {
        let mut server = Server::new_async().await;
        let base_url = server.url();

        let _m = server.mock("GET", "/paper/DOI:10.1000/1?fields=title,authors,year,abstract,externalIds,citationCount,referenceCount,influentialCitationCount,fieldsOfStudy,isOpenAccess,openAccessPdf,tldr,references,citations")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "paperId": "P1",
                "title": "Title",
                "references": [
                    {
                        "paperId": "P2",
                        "title": "Ref Title",
                        "year": 2020,
                        "authors": [{"name": "Ref Author"}],
                        "externalIds": {"DOI": "10.1000/2"}
                    }
                ]
            }"#)
            .create_async().await;

        let s2 = Arc::new(SemanticScholarSource::with_params(&base_url, Duration::ZERO, None));
        let crossref = Arc::new(CrossRefSource::with_params(&base_url, Duration::ZERO, None));
        let extractor = ReferenceExtractor::new(crossref, s2);

        let mut card = BookCard::new("Test");
        let mut ids = ScientificIdentifiers::default();
        ids.doi = Some("10.1000/1".to_string());
        card.identifiers = Some(ids);

        let refs = extractor.extract(&card).await.unwrap();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].resolved_title.as_deref(), Some("Ref Title"));
        assert_eq!(refs[0].doi.as_ref().unwrap().normalized, "10.1000/2");
    }
}
