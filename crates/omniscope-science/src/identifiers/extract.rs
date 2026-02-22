use std::path::Path;
use once_cell::sync::Lazy;
use regex::Regex;
use crate::error::{Result, ScienceError};
use crate::identifiers::{doi::Doi, arxiv::ArxivId, isbn::Isbn};
use crate::sources::Metadata;
use lopdf::Object;

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
        if let Some(isbn) = m.get(2).and_then(|raw| Isbn::parse(raw.as_str()).ok()) {
            return Some(isbn);
        }
    }
    None
}

fn extract_text_from_first_page(doc: &lopdf::Document) -> Result<String> {
    let pages = doc.get_pages();
    let first_page_id = *pages.keys().min()
        .ok_or_else(|| ScienceError::Parse("PDF has no pages".to_string()))?;
        
    let text = doc.extract_text(&[first_page_id])
        .map_err(|e| ScienceError::Parse(format!("Failed to extract text: {}", e)))?;
        
    Ok(text)
}

pub fn find_doi_in_first_page(pdf_path: &Path) -> Result<Doi> {
    let doc = lopdf::Document::load(pdf_path)
        .map_err(|e| ScienceError::Parse(format!("Failed to load PDF: {}", e)))?;
        
    let text = extract_text_from_first_page(&doc)?;
    let dois = extract_dois_from_text(&text);
    
    dois.into_iter().next()
        .ok_or_else(|| ScienceError::IdentifierNotFound("No DOI found in PDF".to_string()))
}

pub fn find_arxiv_id_in_pdf(pdf_path: &Path) -> Result<ArxivId> {
    let doc = lopdf::Document::load(pdf_path)
        .map_err(|e| ScienceError::Parse(format!("Failed to load PDF: {}", e)))?;
        
    let text = extract_text_from_first_page(&doc)?;
    let ids = extract_arxiv_ids_from_text(&text);
    
    ids.into_iter().next()
        .ok_or_else(|| ScienceError::IdentifierNotFound("No arXiv ID found in PDF".to_string()))
}

pub fn extract_pdf_metadata(pdf_path: &Path) -> Result<Metadata> {
    let doc = lopdf::Document::load(pdf_path)
        .map_err(|e| ScienceError::Parse(format!("Failed to load PDF: {}", e)))?;

    // Try to get Info dictionary
    let info_dict = if let Ok(info_ref) = doc.trailer.get(b"Info") {
        match info_ref {
            Object::Reference(id) => doc.get_object(*id).ok().and_then(|o| o.as_dict().ok()),
            Object::Dictionary(dict) => Some(dict),
            _ => None,
        }
    } else {
        None
    };

    let mut title = String::new();
    let mut authors = Vec::new();
    let mut year = None;
    let mut subject = None;

    if let Some(info) = info_dict {
        if let Ok(Some(t)) = info.get(b"Title").map(|o| o.as_str_or_string()) {
            title = t.to_string();
        }
        if let Ok(Some(a)) = info.get(b"Author").map(|o| o.as_str_or_string()) {
            let a_str = a.to_string();
            if !a_str.is_empty() {
                authors.push(a_str);
            }
        }
        if let Ok(Some(s)) = info.get(b"Subject").map(|o| o.as_str_or_string()) {
            subject = Some(s.to_string());
        }
        if let Ok(Some(d)) = info.get(b"CreationDate").map(|o| o.as_str_or_string()) {
            let d_str = d.to_string();
            if d_str.starts_with("D:") && d_str.len() >= 6
                && let Ok(y) = d_str[2..6].parse::<i32>()
            {
                year = Some(y);
            }
        }
    }

    Ok(Metadata {
        title,
        authors,
        year,
        abstract_text: subject,
        doi: None,
        isbn: None,
        publisher: None,
        journal: None,
        volume: None,
        issue: None,
        pages: None,
    })
}

trait AsStrOrString {
    fn as_str_or_string<'a>(&'a self) -> Option<std::borrow::Cow<'a, str>>;
}

impl AsStrOrString for Object {
    fn as_str_or_string<'a>(&'a self) -> Option<std::borrow::Cow<'a, str>> {
        match self {
            Object::String(bytes, _) => Some(String::from_utf8_lossy(bytes)),
            Object::Name(bytes) => Some(String::from_utf8_lossy(bytes)),
            _ => None,
        }
    }
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
