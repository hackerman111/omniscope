use std::path::Path;
use once_cell::sync::Lazy;
use regex::Regex;
use crate::error::{Result, ScienceError};
use crate::identifiers::{doi::Doi, arxiv::ArxivId, isbn::Isbn};

static DOI_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)10\.\d{4,9}/[-._;()/:A-Z0-9]+[A-Z0-9/]").unwrap()
});

static ARXIV_REGEX_NEW: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)arxiv:(\d{4}\.\d{4,5}(v\d+)?)").unwrap()
});

static ARXIV_REGEX_OLD: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)arxiv:([a-z\-]+(\.[A-Z]{2})?/\d{7})").unwrap()
});

static ARXIV_RAW_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(\d{4}\.\d{4,5}(v\d+)?)").unwrap()
});

static ISBN_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)isbn(-13|-10)?[:\s]?([0-9Xx\-\s]{10,20})").unwrap()
});

pub fn extract_dois_from_text(text: &str) -> Vec<Doi> {
    DOI_REGEX.find_iter(text)
        .filter_map(|m| {
            let s = m.as_str();
            Doi::parse(s).ok()
        })
        .collect()
}

pub fn extract_arxiv_ids_from_text(text: &str) -> Vec<ArxivId> {
    let mut ids = Vec::new();

    // Check with prefixes
    for m in ARXIV_REGEX_NEW.captures_iter(text) {
        if let Some(id) = m.get(1).and_then(|g| ArxivId::parse(g.as_str()).ok()) {
            ids.push(id);
        }
    }
    
    for m in ARXIV_REGEX_OLD.captures_iter(text) {
        if let Some(id) = m.get(1).and_then(|g| ArxivId::parse(g.as_str()).ok()) {
            ids.push(id);
        }
    }

    // Also check for raw IDs to catch things like "and 1801.00001"
    for m in ARXIV_RAW_REGEX.find_iter(text) {
        if let Ok(id) = ArxivId::parse(m.as_str()) {
            ids.push(id);
        }
    }

    ids.sort_by(|a, b| a.id.cmp(&b.id));
    ids.dedup_by(|a, b| a.id == b.id);
    ids
}

pub fn extract_isbn_from_text(text: &str) -> Option<Isbn> {
    for m in ISBN_REGEX.captures_iter(text) {
        if let Some(raw) = m.get(2) {
            if let Ok(isbn) = Isbn::parse(raw.as_str()) {
                return Some(isbn);
            }
        }
    }
    None
}

pub fn find_doi_in_first_page(_pdf_path: &Path) -> Result<Doi> {
    // Placeholder for Step 10 (PDF processing)
    Err(ScienceError::IdentifierNotFound("PDF extraction not yet implemented".to_string()))
}

pub fn find_arxiv_id_in_pdf(_pdf_path: &Path) -> Result<ArxivId> {
    // Placeholder for Step 10
    Err(ScienceError::IdentifierNotFound("PDF extraction not yet implemented".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_dois() {
        let text = "Check out 10.1145/3313831.3376166 and also 10.1038/s41586-021-03819-2.";
        let dois = extract_dois_from_text(text);
        assert_eq!(dois.len(), 2);
        assert_eq!(dois[0].normalized, "10.1145/3313831.3376166");
        assert_eq!(dois[1].normalized, "10.1038/s41586-021-03819-2");
    }

    #[test]
    fn test_extract_arxiv() {
        let text = "Papers: arXiv:1706.03762v5 and 1801.00001";
        let ids = extract_arxiv_ids_from_text(text);
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0].id, "1706.03762");
        assert_eq!(ids[1].id, "1801.00001");
    }

    #[test]
    fn test_extract_isbn() {
        let text = "My book ISBN-13: 978-3-16-148410-0 is great.";
        let isbn = extract_isbn_from_text(text).unwrap();
        assert_eq!(isbn.isbn13, "9783161484100");
    }
}
