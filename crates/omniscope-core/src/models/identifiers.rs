use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ScientificIdentifiers {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doi: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arxiv_id: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isbn13: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isbn10: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pmid: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pmcid: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openalex_id: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_scholar_id: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mag_id: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dblp_key: Option<String>,
}

impl ScientificIdentifiers {
    pub fn is_empty(&self) -> bool {
        self.doi.is_none()
            && self.arxiv_id.is_none()
            && self.isbn13.is_none()
            && self.isbn10.is_none()
            && self.pmid.is_none()
            && self.pmcid.is_none()
            && self.openalex_id.is_none()
            && self.semantic_scholar_id.is_none()
            && self.mag_id.is_none()
            && self.dblp_key.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identifiers_is_empty() {
        let empty = ScientificIdentifiers::default();
        assert!(empty.is_empty());

        let with_doi = ScientificIdentifiers {
            doi: Some("10.1234/test".to_string()),
            ..Default::default()
        };
        assert!(!with_doi.is_empty());
    }
}
