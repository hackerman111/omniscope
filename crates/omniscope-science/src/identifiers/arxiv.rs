use crate::error::{Result, ScienceError};
use serde::{Deserialize, Serialize};
use once_cell::sync::Lazy;
use regex::Regex;

// New format: YYMM.NNNNN or YYMM.NNNNNN (with optional version)
static NEW_FORMAT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(\d{4}\.\d{4,5})(v(\d+))?$").unwrap()
});

// Old format: category/YYMMNNN
static OLD_FORMAT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^([a-zA-Z\-]+(?:\.[A-Z]{2})?/\d{7})(v(\d+))?$").unwrap()
});

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArxivId {
    pub raw: String,
    pub id: String,
    pub version: Option<u8>,
    pub abs_url: String,
    pub pdf_url: String,
    pub category: Option<String>,
}

impl ArxivId {
    pub fn parse(input: &str) -> Result<Self> {
        let input = input.trim();

        // Strip known prefixes
        let stripped = if let Some(s) = input.strip_prefix("https://arxiv.org/abs/") {
            s
        } else if let Some(s) = input.strip_prefix("http://arxiv.org/abs/") {
            s
        } else if let Some(s) = input.strip_prefix("https://arxiv.org/pdf/") {
            s.trim_end_matches(".pdf")
        } else if let Some(s) = input.strip_prefix("http://arxiv.org/pdf/") {
            s.trim_end_matches(".pdf")
        } else if let Some(s) = input.strip_prefix("arXiv:") {
            s
        } else if let Some(s) = input.strip_prefix("arxiv:") {
            s
        } else {
            input
        };

        // Try new format
        if let Some(caps) = NEW_FORMAT.captures(stripped) {
            let id = caps.get(1).unwrap().as_str().to_string();
            let version = caps.get(3).and_then(|v| v.as_str().parse::<u8>().ok());
            return Ok(Self {
                raw: input.to_string(),
                abs_url: format!("https://arxiv.org/abs/{id}"),
                pdf_url: format!("https://arxiv.org/pdf/{id}"),
                id,
                version,
                category: None,
            });
        }

        // Try old format: category/YYMMNNN
        if let Some(caps) = OLD_FORMAT.captures(stripped) {
            let full_id = caps.get(1).unwrap().as_str();
            let version = caps.get(3).and_then(|v| v.as_str().parse::<u8>().ok());
            let slash_pos = full_id.find('/').unwrap();
            let category = full_id[..slash_pos].to_string();
            let id = full_id.to_string();
            return Ok(Self {
                raw: input.to_string(),
                abs_url: format!("https://arxiv.org/abs/{id}"),
                pdf_url: format!("https://arxiv.org/pdf/{id}"),
                id,
                version,
                category: Some(category),
            });
        }

        Err(ScienceError::InvalidArxivId(input.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_format_bare() {
        let id = ArxivId::parse("2301.04567").unwrap();
        assert_eq!(id.id, "2301.04567");
        assert_eq!(id.version, None);
        assert_eq!(id.abs_url, "https://arxiv.org/abs/2301.04567");
        assert_eq!(id.pdf_url, "https://arxiv.org/pdf/2301.04567");
    }

    #[test]
    fn new_format_with_version() {
        let id = ArxivId::parse("2301.04567v2").unwrap();
        assert_eq!(id.id, "2301.04567");
        assert_eq!(id.version, Some(2));
    }

    #[test]
    fn old_format_with_category() {
        let id = ArxivId::parse("cs.AI/0601001").unwrap();
        assert_eq!(id.id, "cs.AI/0601001");
        assert_eq!(id.category, Some("cs.AI".to_string()));
        assert_eq!(id.version, None);
    }

    #[test]
    fn arxiv_colon_prefix() {
        let id = ArxivId::parse("arxiv:2301.04567").unwrap();
        assert_eq!(id.id, "2301.04567");
    }

    #[test]
    fn arxiv_capital_prefix() {
        let id = ArxivId::parse("arXiv:2301.04567v5").unwrap();
        assert_eq!(id.id, "2301.04567");
        assert_eq!(id.version, Some(5));
    }

    #[test]
    fn abs_url() {
        let id = ArxivId::parse("https://arxiv.org/abs/2301.04567").unwrap();
        assert_eq!(id.id, "2301.04567");
    }

    #[test]
    fn pdf_url() {
        let id = ArxivId::parse("https://arxiv.org/pdf/2301.04567").unwrap();
        assert_eq!(id.id, "2301.04567");
    }

    #[test]
    fn reject_plain_number() {
        assert!(ArxivId::parse("12345").is_err());
    }

    #[test]
    fn reject_not_arxiv() {
        assert!(ArxivId::parse("not-arxiv").is_err());
    }

    #[test]
    fn reject_too_short() {
        assert!(ArxivId::parse("123.456").is_err());
    }
}
