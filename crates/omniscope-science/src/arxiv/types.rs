use serde::{Deserialize, Serialize};
use crate::identifiers::{arxiv::ArxivId, doi::Doi};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArxivMetadata {
    pub arxiv_id: ArxivId,
    pub doi: Option<Doi>,
    pub title: String,
    pub authors: Vec<ArxivAuthor>,
    pub abstract_text: String,
    pub published: chrono::DateTime<chrono::Utc>,
    pub updated: chrono::DateTime<chrono::Utc>,
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
        todo!()
    }
}
