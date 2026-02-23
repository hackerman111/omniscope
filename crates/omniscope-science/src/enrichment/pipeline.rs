use std::sync::Arc;
use crate::sources::crossref::CrossRefSource;
use crate::sources::semantic_scholar::SemanticScholarSource;
use crate::sources::openalex::OpenAlexSource;
use crate::sources::unpaywall::UnpaywallSource;
use crate::sources::openlibrary::OpenLibrarySource;
use crate::arxiv::client::ArxivClient;
use crate::identifiers::arxiv::ArxivId;
use crate::identifiers::doi::Doi;
use crate::identifiers::isbn::Isbn;
use crate::enrichment::merge::{MetadataSource, MergeStrategy, MergeMetadata};
use omniscope_core::models::book::BookCard;
use omniscope_core::models::ScientificIdentifiers;
use crate::config::ScienceConfig;

use serde::Serialize;

#[derive(Debug, Default, Serialize)]
pub struct EnrichmentReport {
// ...
// (rest of the file)
    pub steps: Vec<String>,
    pub fields_updated: Vec<String>,
    pub sources_used: Vec<String>,
    pub errors: Vec<String>,
}

pub struct EnrichmentPipeline {
    crossref: Arc<CrossRefSource>,
    s2: Arc<SemanticScholarSource>,
    openalex: Arc<OpenAlexSource>,
    unpaywall: Arc<UnpaywallSource>,
    openlibrary: Arc<OpenLibrarySource>,
    arxiv_client: Arc<ArxivClient>,
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
        }
    }

    pub fn default_with_config(config: &ScienceConfig) -> Self {
        let email = config.polite_pool_email.clone();
        
        let crossref = Arc::new(CrossRefSource::new(email.clone()));
        let s2 = Arc::new(SemanticScholarSource::new(config.semantic_scholar_api_key.clone()));
        let openalex = Arc::new(OpenAlexSource::new());
        let unpaywall = Arc::new(UnpaywallSource::new(email.unwrap_or_else(|| "test@example.com".to_string())));
        let openlibrary = Arc::new(OpenLibrarySource::new());
        let arxiv_client = Arc::new(ArxivClient::new());

        Self::new(crossref, s2, openalex, unpaywall, openlibrary, arxiv_client)
    }

    pub async fn enrich(&self, card: &mut BookCard) -> EnrichmentReport {
        let mut report = EnrichmentReport::default();

        // Ensure identifiers struct exists
        if card.identifiers.is_none() {
            card.identifiers = Some(ScientificIdentifiers::default());
        }

        // 2. By Identifiers
        // DOI -> CrossRef + Unpaywall
        let doi_opt = card.identifiers.as_ref().and_then(|ids| ids.doi.clone());
        if let Some(doi_str) = doi_opt
            && let Ok(doi) = Doi::parse(&doi_str) {
                // CrossRef
                report.steps.push("Querying CrossRef by DOI".to_string());
                match self.crossref.fetch_by_doi(&doi).await {
                    Ok(work) => {
                        card.merge_metadata(work.into_metadata(), MetadataSource::CrossRef, MergeStrategy::Concat);
                        report.sources_used.push("CrossRef".to_string());
                        report.fields_updated.push("metadata".to_string());
                    },
                    Err(e) => report.errors.push(format!("CrossRef error: {}", e)),
                }

                // Unpaywall
                report.steps.push("Checking Open Access via Unpaywall".to_string());
                match self.unpaywall.check_oa(&doi).await {
                    Ok(res) => {
                        card.open_access = Some(res.into_oa_info());
                        report.sources_used.push("Unpaywall".to_string());
                        report.fields_updated.push("open_access".to_string());
                    },
                    Err(e) => report.errors.push(format!("Unpaywall error: {}", e)),
                }
            }

        // ArXiv ID -> ArXiv API
        let arxiv_opt = card.identifiers.as_ref().and_then(|ids| ids.arxiv_id.clone());
        if let Some(arxiv_str) = arxiv_opt
            && let Ok(arxiv_id) = ArxivId::parse(&arxiv_str) {
                report.steps.push("Querying ArXiv API".to_string());
                match self.arxiv_client.fetch_metadata(&arxiv_id).await {
                    Ok(meta) => {
                        if let Some(doi) = &meta.doi
                             && let Some(ids) = &mut card.identifiers
                                 && ids.doi.is_none() {
                                     ids.doi = Some(doi.normalized.clone());
                                     report.fields_updated.push("doi".to_string());
                                 }
                        card.merge_metadata(meta.into_metadata(), MetadataSource::ArxivApi, MergeStrategy::Concat);
                        report.sources_used.push("ArxivApi".to_string());
                        report.fields_updated.push("metadata".to_string());
                    },
                    Err(e) => report.errors.push(format!("ArXiv error: {}", e)),
                }
            }
        
        // ISBN -> OpenLibrary
        let isbns = card.metadata.isbn.clone();
        for isbn_str in isbns {
            if let Ok(isbn) = Isbn::parse(&isbn_str) {
                report.steps.push(format!("Querying OpenLibrary by ISBN {}", isbn.formatted));
                match self.openlibrary.fetch_by_isbn(&isbn).await {
                    Ok(Some(work)) => {
                        card.merge_metadata(work.into_metadata(), MetadataSource::OpenLibrary, MergeStrategy::Concat);
                        report.sources_used.push("OpenLibrary".to_string());
                    },
                    Ok(None) => {}, // Not found
                    Err(e) => report.errors.push(format!("OpenLibrary error: {}", e)),
                }
            }
        }

        // 3. Semantic Scholar
        // Determine ID
        let s2_id_opt = card.identifiers.as_ref().and_then(|ids| ids.semantic_scholar_id.clone());
        let s2_lookup_id = if let Some(s2) = s2_id_opt {
            Some(s2)
        } else if let Some(doi_str) = card.identifiers.as_ref().and_then(|ids| ids.doi.clone()) {
            Doi::parse(&doi_str).ok().map(|d| format!("DOI:{}", d.normalized))
        } else if let Some(arxiv_str) = card.identifiers.as_ref().and_then(|ids| ids.arxiv_id.clone()) {
             ArxivId::parse(&arxiv_str).ok().map(|a| format!("ArXiv:{}", a.id))
        } else {
            None
        };

        if let Some(id) = s2_lookup_id {
            report.steps.push(format!("Querying Semantic Scholar by {}", id));
            match self.s2.fetch_paper(&id).await {
                Ok(paper) => {
                    // Update external IDs
                    if let Some(ids) = &mut card.identifiers {
                        if ids.semantic_scholar_id.is_none() {
                            ids.semantic_scholar_id = Some(paper.paper_id.clone());
                        }
                        if let Some(pmid) = paper.external_ids.get("PubMed")
                            && ids.pmid.is_none() { ids.pmid = Some(pmid.clone()); }
                        if let Some(mag) = paper.external_ids.get("MAG")
                            && ids.mag_id.is_none() { ids.mag_id = Some(mag.clone()); }
                    }

                    if let Some(cc) = paper.citation_count {
                        card.citation_graph.citation_count = cc;
                    }

                    card.merge_metadata(paper.into_metadata(), MetadataSource::SemanticScholar, MergeStrategy::Concat);
                    
                    report.sources_used.push("SemanticScholar".to_string());
                    report.fields_updated.push("citations".to_string());
                },
                Err(e) => report.errors.push(format!("Semantic Scholar error: {}", e)),
            }
        }

        // 4. OpenAlex (if not enough data from S2 or just enrichment)
        let oa_id_opt = card.identifiers.as_ref().and_then(|ids| ids.openalex_id.clone());
        let oa_lookup_id = if let Some(oa) = oa_id_opt {
            Some(oa)
        } else if let Some(doi_str) = card.identifiers.as_ref().and_then(|ids| ids.doi.clone()) {
            Doi::parse(&doi_str).ok().map(|d| format!("https://doi.org/{}", d.normalized)) 
        } else {
            None
        };

        if let Some(id) = oa_lookup_id {
             report.steps.push(format!("Querying OpenAlex by {}", id));
             match self.openalex.fetch_work(&id).await {
                 Ok(work) => {
                     card.merge_metadata(work.into_metadata(), MetadataSource::OpenAlex, MergeStrategy::Concat);
                     report.sources_used.push("OpenAlex".to_string());
                 },
                 Err(e) => report.errors.push(format!("OpenAlex error: {}", e)),
             }
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;
    use std::time::Duration;

    #[tokio::test]
    async fn test_enrichment_pipeline_doi_flow() {
        let mut server = Server::new_async().await;
        let base_url = server.url();
        let test_doi = "10.1000/unique-test";

        // Mock CrossRef
        let _m_crossref = server.mock("GET", format!("/works/{}", test_doi).as_str())
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(format!(r#"{{
                "status": "ok",
                "message": {{
                    "DOI": "{}",
                    "title": ["Deep Learning"],
                    "author": [{{"given": "Yann", "family": "LeCun"}}],
                    "published-print": {{"date-parts": [[2015]]}},
                    "type": "journal-article"
                }}
            }}"#, test_doi))
            .create_async().await;

        // Mock Unpaywall
        let _m_unpaywall = server.mock("GET", format!("/{}?email=test@example.com", test_doi).as_str())
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(format!(r#"{{
                "doi": "{}",
                "is_oa": true,
                "oa_status": "gold",
                "updated": "2023-01-01",
                "oa_locations": [],
                "journal_is_oa": false
            }}"#, test_doi))
            .create_async().await;
            
        // Mock S2 (Semantic Scholar)
        let _m_s2 = server.mock("GET", format!("/paper/DOI:{}?fields=title,authors,year,abstract,externalIds,citationCount,referenceCount,influentialCitationCount,fieldsOfStudy,isOpenAccess,openAccessPdf,tldr,references,citations", test_doi).as_str())
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(format!(r#"{{
                "paperId": "S2ID",
                "title": "Deep Learning S2",
                "citationCount": 50000,
                "externalIds": {{"DOI": "{}"}}
            }}"#, test_doi))
            .create_async().await;

        // Mock OpenAlex (optional call)
        let _m_oa = server.mock("GET", format!("/works/https://doi.org/{}", test_doi).as_str())
             .with_status(404) 
             .create_async().await;

        let crossref = Arc::new(CrossRefSource::with_params(&base_url, Duration::ZERO, None));
        let s2 = Arc::new(SemanticScholarSource::with_params(&base_url, Duration::ZERO, None));
        let openalex = Arc::new(OpenAlexSource::with_params(&base_url, Duration::ZERO));
        let unpaywall = Arc::new(UnpaywallSource::with_params(&base_url, Duration::ZERO, "test@example.com".to_string()));
        let openlibrary = Arc::new(OpenLibrarySource::with_params(&base_url, Duration::ZERO));
        let arxiv_client = Arc::new(ArxivClient::with_params(&base_url, Duration::ZERO));

        let pipeline = EnrichmentPipeline::new(crossref, s2, openalex, unpaywall, openlibrary, arxiv_client);

        let mut card = BookCard::new("");
        let ids = ScientificIdentifiers {
            doi: Some(test_doi.to_string()),
            ..Default::default()
        };
        card.identifiers = Some(ids);

        let report = pipeline.enrich(&mut card).await;

        // Check CrossRef merge
        assert_eq!(card.metadata.title, "Deep Learning"); // Overwrote "Draft Title" because user manual source wasn't tracked or "Draft Title" is weak
        assert_eq!(card.metadata.authors.len(), 1);
        assert_eq!(card.metadata.authors[0], "LeCun, Yann");
        assert_eq!(card.metadata.year, Some(2015));

        // Check Unpaywall
        assert!(card.open_access.is_some());
        assert!(card.open_access.as_ref().unwrap().is_open);

        // Check S2
        assert_eq!(card.citation_graph.citation_count, 50000);
        assert_eq!(card.identifiers.as_ref().unwrap().semantic_scholar_id.as_deref(), Some("S2ID"));

        assert!(report.sources_used.contains(&"CrossRef".to_string()));
        assert!(report.sources_used.contains(&"Unpaywall".to_string()));
        assert!(report.sources_used.contains(&"SemanticScholar".to_string()));
    }
}
