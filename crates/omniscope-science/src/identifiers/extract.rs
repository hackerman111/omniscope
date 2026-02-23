use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use lopdf::Document;
use once_cell::sync::Lazy;
use regex::Regex;
use uuid::Uuid;

use crate::error::{Result, ScienceError};
use crate::identifiers::{arxiv::ArxivId, doi::Doi, isbn::Isbn};

static DOI_REGEX: Lazy<std::result::Result<Regex, regex::Error>> = Lazy::new(|| {
    Regex::new(r#"(?i)\b(?:doi:\s*|https?://(?:dx\.)?doi\.org/)?(10\.\d{4,9}/[^\s"'<>]+)"#)
});

static ARXIV_PREFIX_REGEX: Lazy<std::result::Result<Regex, regex::Error>> = Lazy::new(|| {
    Regex::new(
        r"(?i)\barxiv:\s*([a-zA-Z\-]+(?:\.[A-Z]{2})?/\d{7}(?:v\d+)?|\d{4}\.\d{4,5}(?:v\d+)?)\b",
    )
});

static ARXIV_URL_REGEX: Lazy<std::result::Result<Regex, regex::Error>> = Lazy::new(|| {
    Regex::new(
        r"(?i)https?://arxiv\.org/(?:abs|pdf)/([a-zA-Z\-]+(?:\.[A-Z]{2})?/\d{7}(?:v\d+)?|\d{4}\.\d{4,5}(?:v\d+)?)(?:\.pdf)?",
    )
});

static ARXIV_BRACKET_REGEX: Lazy<std::result::Result<Regex, regex::Error>> =
    Lazy::new(|| Regex::new(r"(?m)^\s*\[([0-9]{4}\.[0-9]{4,5}(?:v\d+)?)\]"));

static ISBN13_REGEX: Lazy<std::result::Result<Regex, regex::Error>> =
    Lazy::new(|| Regex::new(r"(?i)\b(?:isbn(?:-13)?:?\s*)?((?:97[89][-\s]?)(?:\d[-\s]?){9}\d)\b"));

static ISBN10_REGEX: Lazy<std::result::Result<Regex, regex::Error>> =
    Lazy::new(|| Regex::new(r"(?i)\b(?:isbn(?:-10)?:?\s*)?((?:\d[-\s]?){9}[\dXx])\b"));

pub fn extract_dois_from_text(text: &str) -> Vec<Doi> {
    let Ok(re) = DOI_REGEX.as_ref() else {
        return Vec::new();
    };

    let mut seen = HashSet::new();
    let mut out = Vec::new();

    for caps in re.captures_iter(text) {
        let Some(m) = caps.get(1) else {
            continue;
        };
        let cleaned = trim_trailing_punctuation(m.as_str());
        if let Ok(doi) = Doi::parse(cleaned)
            && seen.insert(doi.normalized.clone())
        {
            out.push(doi);
        }
    }

    out
}

pub fn extract_arxiv_ids_from_text(text: &str) -> Vec<ArxivId> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();

    for regex in [
        &*ARXIV_PREFIX_REGEX,
        &*ARXIV_URL_REGEX,
        &*ARXIV_BRACKET_REGEX,
    ] {
        let Ok(re) = regex.as_ref() else {
            continue;
        };

        for caps in re.captures_iter(text) {
            let Some(m) = caps.get(1) else {
                continue;
            };
            let candidate = trim_trailing_punctuation(m.as_str());
            if let Ok(arxiv_id) = ArxivId::parse(candidate) {
                let key = match arxiv_id.version {
                    Some(v) => format!("{}v{v}", arxiv_id.id),
                    None => arxiv_id.id.clone(),
                };
                if seen.insert(key) {
                    out.push(arxiv_id);
                }
            }
        }
    }

    out
}

pub fn extract_isbn_from_text(text: &str) -> Option<Isbn> {
    if let Ok(re13) = ISBN13_REGEX.as_ref() {
        for caps in re13.captures_iter(text) {
            let Some(m) = caps.get(1) else {
                continue;
            };
            if let Ok(isbn) = Isbn::parse(m.as_str()) {
                return Some(isbn);
            }
        }
    }

    if let Ok(re10) = ISBN10_REGEX.as_ref() {
        for caps in re10.captures_iter(text) {
            let Some(m) = caps.get(1) else {
                continue;
            };
            if let Ok(isbn) = Isbn::parse(m.as_str()) {
                return Some(isbn);
            }
        }
    }

    None
}

pub fn find_doi_in_first_page(pdf_path: &Path) -> Result<Doi> {
    let text = extract_pdf_first_pages_text(pdf_path)?;
    extract_dois_from_text(&text)
        .into_iter()
        .next()
        .ok_or_else(|| {
            ScienceError::PdfExtraction("DOI not found in the first pages of PDF".to_string())
        })
}

pub fn find_arxiv_id_in_pdf(pdf_path: &Path) -> Result<ArxivId> {
    let text = extract_pdf_first_pages_text(pdf_path)?;
    extract_arxiv_ids_from_text(&text)
        .into_iter()
        .next()
        .ok_or_else(|| {
            ScienceError::PdfExtraction("arXiv ID not found in the first pages of PDF".to_string())
        })
}

fn extract_pdf_first_pages_text(pdf_path: &Path) -> Result<String> {
    let lopdf_error = match extract_pdf_first_pages_text_with_lopdf(pdf_path, 2) {
        Ok(text) if !text.trim().is_empty() => return Ok(text),
        Ok(_) => Some("lopdf extracted empty text from first pages".to_string()),
        Err(err) => Some(err.to_string()),
    };

    match extract_pdf_first_pages_text_with_pdftotext(pdf_path) {
        Ok(text) if !text.trim().is_empty() => Ok(text),
        Ok(_) => Err(ScienceError::PdfExtraction(
            "pdftotext extracted empty text from first pages".to_string(),
        )),
        Err(pdftotext_error) => {
            let prefix = lopdf_error.unwrap_or_else(|| "lopdf extraction failed".to_string());
            Err(ScienceError::PdfExtraction(format!(
                "{prefix}; {pdftotext_error}"
            )))
        }
    }
}

fn extract_pdf_first_pages_text_with_lopdf(pdf_path: &Path, max_pages: usize) -> Result<String> {
    if max_pages == 0 {
        return Ok(String::new());
    }

    let document = Document::load(pdf_path).map_err(|err| {
        ScienceError::PdfExtraction(format!(
            "lopdf failed to open {}: {err}",
            pdf_path.display()
        ))
    })?;
    let pages = document.get_pages();
    if pages.is_empty() {
        return Ok(String::new());
    }
    let page_numbers = pages.keys().copied().take(max_pages).collect::<Vec<u32>>();

    document.extract_text(&page_numbers).map_err(|err| {
        ScienceError::PdfExtraction(format!(
            "lopdf failed to extract text from {}: {err}",
            pdf_path.display()
        ))
    })
}

fn extract_pdf_first_pages_text_with_pdftotext(pdf_path: &Path) -> Result<String> {
    let output_path = std::env::temp_dir().join(format!(
        "omniscope_pdftotext_{}_{}.txt",
        std::process::id(),
        Uuid::now_v7()
    ));

    let mut child = Command::new("pdftotext")
        .arg("-f")
        .arg("1")
        .arg("-l")
        .arg("2")
        .arg(pdf_path)
        .arg(&output_path)
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ScienceError::PdfExtraction("pdftotext is not installed".to_string())
            } else {
                ScienceError::PdfExtraction(format!("failed to run pdftotext: {e}"))
            }
        })?;

    let deadline = Duration::from_secs(8);
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if started.elapsed() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = fs::remove_file(&output_path);
                    return Err(ScienceError::PdfExtraction(
                        "pdftotext timed out while reading first pages".to_string(),
                    ));
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(err) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = fs::remove_file(&output_path);
                return Err(ScienceError::PdfExtraction(format!(
                    "pdftotext process failed: {err}"
                )));
            }
        }
    };

    if !status.success() {
        let _ = fs::remove_file(&output_path);
        return Err(ScienceError::PdfExtraction(format!(
            "pdftotext exited with status {status}"
        )));
    }

    let text = fs::read_to_string(&output_path).map_err(|err| {
        ScienceError::PdfExtraction(format!(
            "failed to read pdftotext output {}: {err}",
            output_path.display()
        ))
    })?;
    let _ = fs::remove_file(&output_path);
    Ok(text)
}

fn trim_trailing_punctuation(value: &str) -> &str {
    value.trim_end_matches(['.', ',', ';', ':', ')', ']', '}'])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_dois_in_multiple_formats() {
        let text = r#"
            DOI: 10.1000/XYZ123
            doi:10.2000/abc-def
            https://doi.org/10.48550/arXiv.1706.03762.
        "#;

        let found = extract_dois_from_text(text);
        let normalized = found
            .iter()
            .map(|d| d.normalized.as_str())
            .collect::<Vec<_>>();

        assert_eq!(found.len(), 3);
        assert!(normalized.contains(&"10.1000/xyz123"));
        assert!(normalized.contains(&"10.2000/abc-def"));
        assert!(normalized.contains(&"10.48550/arxiv.1706.03762"));
    }

    #[test]
    fn extract_dois_avoids_false_positives() {
        let text = "Numbers 10.5 and 2021.01 are not DOI. Also invalid DOI: 10.1000";
        let found = extract_dois_from_text(text);
        assert!(found.is_empty());
    }

    #[test]
    fn extracts_arxiv_ids_from_supported_patterns() {
        let text = r#"
            arXiv:2301.04567v2
            Link: https://arxiv.org/abs/1706.03762
            [2201.12345]
        "#;

        let found = extract_arxiv_ids_from_text(text);
        let keys = found
            .iter()
            .map(|id| match id.version {
                Some(v) => format!("{}v{v}", id.id),
                None => id.id.clone(),
            })
            .collect::<Vec<_>>();

        assert_eq!(found.len(), 3);
        assert!(keys.contains(&"2301.04567v2".to_string()));
        assert!(keys.contains(&"1706.03762".to_string()));
        assert!(keys.contains(&"2201.12345".to_string()));
    }

    #[test]
    fn extract_arxiv_avoids_false_positives() {
        let text = "Noise: 123.456, 2021.01, arxiv:12345, not-arxiv";
        let found = extract_arxiv_ids_from_text(text);
        assert!(found.is_empty());
    }

    #[test]
    fn extracts_isbn13() {
        let text = "ISBN-13: 978-0-306-40615-7";
        let isbn = extract_isbn_from_text(text).expect("isbn should be found");
        assert_eq!(isbn.isbn13, "9780306406157");
    }

    #[test]
    fn extracts_isbn10_with_x() {
        let text = "Classic text, ISBN 007462542X.";
        let isbn = extract_isbn_from_text(text).expect("isbn should be found");
        assert_eq!(isbn.isbn10.as_deref(), Some("007462542X"));
    }

    #[test]
    fn returns_none_when_isbn_missing() {
        let text = "There is no valid isbn in this line: 123-456";
        assert!(extract_isbn_from_text(text).is_none());
    }
}
