use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::Result;
use crate::identifiers::{arxiv::ArxivId, doi::Doi, isbn::Isbn};
use crate::sources::crossref::CrossRefSource;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum ResolutionMethod {
    DirectDoi,
    DirectArxiv,
    CrossRefQuery,
    SemanticScholar,
    #[default]
    Unresolved,
}

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
    pub is_in_library: Option<Uuid>,
}

impl ExtractedReference {
    pub fn from_raw(index: usize, raw_text: impl Into<String>) -> Self {
        Self {
            index,
            raw_text: raw_text.into(),
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

#[allow(clippy::ptr_arg)]
pub async fn resolve_unidentified(
    refs: &mut Vec<ExtractedReference>,
    crossref: &CrossRefSource,
) -> Result<()> {
    for reference in refs.iter_mut() {
        if reference.doi.is_some() {
            reference.resolution_method = ResolutionMethod::DirectDoi;
            if reference.confidence <= 0.0 {
                reference.confidence = 1.0;
            }
            continue;
        }

        if reference.arxiv_id.is_some() {
            reference.resolution_method = ResolutionMethod::DirectArxiv;
            if reference.confidence <= 0.0 {
                reference.confidence = 1.0;
            }
        }
    }

    let unresolved = refs
        .iter()
        .enumerate()
        .filter(|(_, reference)| reference.doi.is_none() && reference.arxiv_id.is_none())
        .map(|(idx, reference)| (idx, reference.raw_text.clone()))
        .collect::<Vec<_>>();

    let resolved = stream::iter(unresolved.into_iter().map(|(idx, raw_text)| async move {
        let result = crossref.query_by_text(&raw_text).await;
        (idx, result)
    }))
    .buffer_unordered(3)
    .collect::<Vec<_>>()
    .await;

    for (idx, result) in resolved {
        let Ok(Some((doi, score))) = result else {
            continue;
        };
        let Some(reference) = refs.get_mut(idx) else {
            continue;
        };

        reference.doi = Some(doi);
        reference.confidence = normalize_crossref_score(score);
        reference.resolution_method = ResolutionMethod::CrossRefQuery;
    }

    Ok(())
}

fn normalize_crossref_score(score: f32) -> f32 {
    if !score.is_finite() || score <= 0.0 {
        return 0.0;
    }
    if score > 1.0 {
        return (score / 100.0).clamp(0.0, 1.0);
    }
    score.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identifiers::{arxiv::ArxivId, doi::Doi};

    #[test]
    fn builds_reference_from_raw_text() {
        let reference = ExtractedReference::from_raw(7, "Sample reference");

        assert_eq!(reference.index, 7);
        assert_eq!(reference.raw_text, "Sample reference");
        assert_eq!(reference.resolution_method, ResolutionMethod::Unresolved);
        assert_eq!(reference.confidence, 0.0);
        assert!(reference.doi.is_none());
        assert!(reference.arxiv_id.is_none());
    }

    #[tokio::test]
    async fn marks_direct_identifiers_without_crossref_queries() {
        let mut refs = vec![
            ExtractedReference {
                doi: Some(Doi::parse("10.1000/xyz123").expect("valid doi")),
                ..ExtractedReference::from_raw(1, "Direct DOI reference")
            },
            ExtractedReference {
                arxiv_id: Some(ArxivId::parse("2301.04567").expect("valid arxiv id")),
                ..ExtractedReference::from_raw(2, "Direct arXiv reference")
            },
        ];
        let source = CrossRefSource::new(None);

        resolve_unidentified(&mut refs, &source)
            .await
            .expect("resolver should succeed");

        assert_eq!(refs[0].resolution_method, ResolutionMethod::DirectDoi);
        assert_eq!(refs[1].resolution_method, ResolutionMethod::DirectArxiv);
        assert_eq!(refs[0].confidence, 1.0);
        assert_eq!(refs[1].confidence, 1.0);
    }

    #[test]
    fn normalizes_crossref_scores_to_0_1_range() {
        assert_eq!(normalize_crossref_score(95.0), 0.95);
        assert_eq!(normalize_crossref_score(0.8), 0.8);
        assert_eq!(normalize_crossref_score(-1.0), 0.0);
    }
}
