use crate::identifiers::{arxiv::ArxivId, doi::Doi};
use serde::{Deserialize, Serialize};

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
        let mut parts = Vec::new();

        if let Some(value) = non_empty(&self.all) {
            parts.push(format!("all:{}", encode_value(value)));
        }
        if let Some(value) = non_empty(&self.title) {
            parts.push(format!("ti:{}", encode_value(value)));
        }
        if let Some(value) = non_empty(&self.author) {
            parts.push(format!("au:{}", encode_value(value)));
        }
        if let Some(value) = non_empty(&self.abstract_text) {
            parts.push(format!("abs:{}", encode_value(value)));
        }
        if let Some(value) = non_empty(&self.category) {
            parts.push(format!("cat:{}", encode_value(value)));
        }
        if let Some(value) = non_empty(&self.journal) {
            parts.push(format!("jr:{}", encode_value(value)));
        }

        if !self.id_list.is_empty() {
            let ids = self
                .id_list
                .iter()
                .map(|id| format!("id:{}", encode_value(id)))
                .collect::<Vec<_>>()
                .join("+OR+");
            parts.push(ids);
        }

        if self.date_from.is_some() || self.date_to.is_some() {
            let from = self
                .date_from
                .map(|d| d.format("%Y%m%d").to_string())
                .unwrap_or_else(|| "19910101".to_string());
            let to = self
                .date_to
                .map(|d| d.format("%Y%m%d").to_string())
                .unwrap_or_else(|| "20991231".to_string());
            parts.push(format!("submittedDate:[{from}0000+TO+{to}2359]"));
        }

        if parts.is_empty() {
            return "all:*".to_string();
        }

        parts.join("+AND+")
    }
}

fn non_empty(value: &Option<String>) -> Option<&str> {
    value.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

fn encode_value(value: &str) -> String {
    value
        .split_whitespace()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("+")
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;

    #[test]
    fn builds_compound_query() {
        let query = ArxivSearchQuery {
            title: Some("attention".to_string()),
            author: Some("Vaswani".to_string()),
            ..Default::default()
        };

        assert_eq!(query.to_query_string(), "ti:attention+AND+au:Vaswani");
    }

    #[test]
    fn normalizes_spaces_in_values() {
        let query = ArxivSearchQuery {
            title: Some("Attention Is   All You Need".to_string()),
            ..Default::default()
        };

        assert_eq!(query.to_query_string(), "ti:Attention+Is+All+You+Need");
    }

    #[test]
    fn supports_id_list_and_date_range() {
        let query = ArxivSearchQuery {
            id_list: vec!["1706.03762".to_string(), "2301.04567".to_string()],
            date_from: NaiveDate::from_ymd_opt(2017, 1, 1),
            date_to: NaiveDate::from_ymd_opt(2017, 12, 31),
            ..Default::default()
        };

        assert_eq!(
            query.to_query_string(),
            "id:1706.03762+OR+id:2301.04567+AND+submittedDate:[201701010000+TO+201712312359]"
        );
    }

    #[test]
    fn defaults_to_all_wildcard_when_empty() {
        let query = ArxivSearchQuery::default();
        assert_eq!(query.to_query_string(), "all:*");
    }
}
