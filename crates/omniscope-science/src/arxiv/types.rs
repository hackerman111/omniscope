use serde::{Deserialize, Serialize};
use crate::identifiers::{arxiv::ArxivId, doi::Doi};
use crate::sources::Metadata;
use chrono::{Datelike, DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArxivMetadata {
    pub arxiv_id: ArxivId,
    pub doi: Option<Doi>,
    pub title: String,
    pub authors: Vec<ArxivAuthor>,
    pub abstract_text: String,
    pub published: DateTime<Utc>,
    pub updated: DateTime<Utc>,
    pub categories: Vec<String>,
    pub primary_category: String,
    pub comment: Option<String>,
    pub journal_ref: Option<String>,
    pub pdf_url: String,
    pub abs_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArxivAuthor {
    pub name: String,
    pub affiliation: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ArxivSearchQuery {
    pub all: Option<String>,
    pub title: Option<String>,
    pub author: Option<String>,
    pub abstract_text: Option<String>,
    pub category: Option<String>,
    pub journal: Option<String>,
    pub id_list: Vec<String>,
    pub sort_by: Option<String>,
    pub max_results: Option<u32>,
    pub start: Option<u32>,
    pub date_from: Option<chrono::NaiveDate>,
    pub date_to: Option<chrono::NaiveDate>,
}

impl ArxivSearchQuery {
    pub fn to_query_string(&self) -> String {
        let mut parts = Vec::new();

        let encode = |s: &str| s.replace(' ', "+");

        if let Some(ref q) = self.all {
            parts.push(format!("all:{}", encode(q)));
        }
        if let Some(ref q) = self.title {
            parts.push(format!("ti:{}", encode(q)));
        }
        if let Some(ref q) = self.author {
            parts.push(format!("au:{}", encode(q)));
        }
        if let Some(ref q) = self.abstract_text {
            parts.push(format!("abs:{}", encode(q)));
        }
        if let Some(ref q) = self.category {
            parts.push(format!("cat:{}", encode(q)));
        }
        if let Some(ref q) = self.journal {
            parts.push(format!("jr:{}", encode(q)));
        }

        parts.join("+AND+")
    }
}

impl ArxivMetadata {
    pub fn into_metadata(self) -> Metadata {
        Metadata {
            title: self.title,
            authors: self.authors.into_iter().map(|a| a.name).collect(),
            year: Some(self.published.year()),
            abstract_text: Some(self.abstract_text),
            doi: self.doi.map(|d| d.normalized),
            isbn: None,
            publisher: None,
            journal: self.journal_ref,
            volume: None,
            issue: None,
            pages: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arxiv_query_string() {
        let query = ArxivSearchQuery {
            title: Some("attention is all you need".to_string()),
            author: Some("Vaswani".to_string()),
            ..Default::default()
        };
        assert_eq!(query.to_query_string(), "ti:attention+is+all+you+need+AND+au:Vaswani");
    }

    #[test]
    fn test_arxiv_query_all() {
        let query = ArxivSearchQuery {
            all: Some("electron".to_string()),
            ..Default::default()
        };
        assert_eq!(query.to_query_string(), "all:electron");
    }
}
