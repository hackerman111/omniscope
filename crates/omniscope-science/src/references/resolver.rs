use crate::identifiers::{doi::Doi, arxiv::ArxivId, isbn::Isbn};
use crate::sources::crossref::CrossRefSource;
use serde::{Deserialize, Serialize};
use futures::StreamExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedReference {
    pub index: usize,
    pub raw_text: String,
    pub doi: Option<Doi>,
    pub arxiv_id: Option<ArxivId>,
    pub isbn: Option<Isbn>,
    pub resolved_title: Option<String>,
    pub resolved_authors: Vec<String>,
    pub resolved_year: Option<i32>,
    pub confidence: f32,
    pub resolution_method: ResolutionMethod,
    pub is_in_library: Option<String>, // BookId
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ResolutionMethod {
    DirectDoi,
    DirectArxiv,
    CrossRefQuery,
    SemanticScholar,
    Unresolved,
}

impl ExtractedReference {
    pub fn from_raw(index: usize, text: String) -> Self {
        Self {
            index,
            raw_text: text,
            doi: None,
            arxiv_id: None,
            isbn: None,
            resolved_title: None,
            resolved_authors: Vec::new(),
            resolved_year: None,
            confidence: 0.0,
            resolution_method: ResolutionMethod::Unresolved,
            is_in_library: None,
        }
    }
}

pub async fn resolve_unidentified(refs: &mut Vec<ExtractedReference>, crossref: &CrossRefSource) {
    let mut queries = Vec::new();

    for (i, reference) in refs.iter().enumerate() {
        if reference.doi.is_some() || reference.arxiv_id.is_some() {
            continue;
        }
        queries.push((i, reference.raw_text.clone()));
    }

    let mut stream = futures::stream::iter(queries)
        .map(|(i, text)| async move { 
            let res = crossref.query_by_text(&text).await;
            (i, res) 
        })
        .buffer_unordered(3);

    while let Some((i, result)) = stream.next().await {
        if let Ok(Some((doi, score))) = result {
            if let Some(r) = refs.get_mut(i) {
                r.doi = Some(doi);
                r.confidence = score / 100.0;
                r.resolution_method = ResolutionMethod::CrossRefQuery;
            }
        }
    }
}
