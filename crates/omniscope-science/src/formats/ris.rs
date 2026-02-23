use std::collections::BTreeMap;

use omniscope_core::models::{BookCard, DocumentType};

use crate::error::{Result, ScienceError};
use crate::identifiers::{arxiv::ArxivId, doi::Doi};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RisEntry {
    pub entry_type: String,
    pub fields: BTreeMap<String, Vec<String>>,
}

impl RisEntry {
    pub fn first(&self, tag: &str) -> Option<&str> {
        self.fields
            .get(tag)
            .and_then(|values| values.first())
            .map(String::as_str)
    }
}

pub fn generate_ris(card: &BookCard) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "TY  - {}",
        document_type_to_ris(
            card.publication
                .as_ref()
                .map(|publication| publication.doc_type)
                .unwrap_or(DocumentType::Book),
        )
    ));

    push_line(&mut lines, "TI", Some(card.metadata.title.clone()));
    for author in &card.metadata.authors {
        push_line(&mut lines, "AU", Some(author_to_ris(author)));
    }
    push_line(
        &mut lines,
        "PY",
        card.metadata.year.map(|year| year.to_string()),
    );

    if let Some(publication) = card.publication.as_ref() {
        push_line(&mut lines, "JO", publication.journal.clone());
        push_line(&mut lines, "T2", publication.conference.clone());
        push_line(&mut lines, "VL", publication.volume.clone());
        push_line(&mut lines, "IS", publication.issue.clone());
        push_line(&mut lines, "SP", publication.pages.clone());
    }

    if let Some(identifiers) = card.identifiers.as_ref() {
        push_line(
            &mut lines,
            "DO",
            identifiers
                .doi
                .as_deref()
                .map(normalize_doi_for_export)
                .or_else(|| Some(String::new()))
                .filter(|value| !value.is_empty()),
        );
    }
    push_line(&mut lines, "UR", export_url(card));

    lines.push("ER  -".to_string());
    lines.push(String::new());
    lines.join("\n")
}

pub fn parse_ris(content: &str) -> Result<Vec<RisEntry>> {
    let mut entries = Vec::new();
    let mut current: Option<RisEntry> = None;

    for (line_no, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim_end();
        if line.trim().is_empty() {
            continue;
        }

        let parsed = line.split_once("  - ").or_else(|| line.split_once("  -"));
        let Some((tag, value)) = parsed else {
            return Err(ScienceError::Parse(format!(
                "invalid RIS line {}: {line}",
                line_no + 1
            )));
        };
        let tag = tag.trim();
        let value = value.trim();

        match tag {
            "TY" => {
                if current.is_some() {
                    return Err(ScienceError::Parse(format!(
                        "nested TY without ER at line {}",
                        line_no + 1
                    )));
                }
                current = Some(RisEntry {
                    entry_type: value.to_string(),
                    fields: BTreeMap::new(),
                });
            }
            "ER" => {
                let Some(entry) = current.take() else {
                    return Err(ScienceError::Parse(format!(
                        "ER without TY at line {}",
                        line_no + 1
                    )));
                };
                entries.push(entry);
            }
            _ => {
                let Some(entry) = current.as_mut() else {
                    return Err(ScienceError::Parse(format!(
                        "{tag} outside of RIS entry at line {}",
                        line_no + 1
                    )));
                };
                entry
                    .fields
                    .entry(tag.to_string())
                    .or_default()
                    .push(value.to_string());
            }
        }
    }

    if let Some(entry) = current.take() {
        entries.push(entry);
    }

    Ok(entries)
}

fn document_type_to_ris(doc_type: DocumentType) -> &'static str {
    match doc_type {
        DocumentType::Article | DocumentType::MagazineArticle => "JOUR",
        DocumentType::Book => "BOOK",
        DocumentType::ConferencePaper => "CONF",
        DocumentType::Preprint => "JOUR",
        DocumentType::Thesis => "THES",
        DocumentType::Report => "RPRT",
        _ => "GEN",
    }
}

fn push_line(lines: &mut Vec<String>, tag: &str, value: Option<String>) {
    let Some(value) = value else {
        return;
    };
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    lines.push(format!("{tag}  - {value}"));
}

fn author_to_ris(value: &str) -> String {
    let cleaned = value.trim();
    if cleaned.is_empty() {
        return String::new();
    }

    if cleaned.contains(',') {
        return cleaned.to_string();
    }

    let parts = cleaned.split_whitespace().collect::<Vec<_>>();
    if parts.len() < 2 {
        return cleaned.to_string();
    }
    let family = parts.last().copied().unwrap_or_default();
    let given = parts[..parts.len() - 1].join(" ");
    format!("{family}, {given}")
}

fn normalize_doi_for_export(raw: &str) -> String {
    Doi::parse(raw)
        .map(|doi| doi.normalized)
        .unwrap_or_else(|_| raw.trim().to_string())
}

fn export_url(card: &BookCard) -> Option<String> {
    let from_doi = card
        .identifiers
        .as_ref()
        .and_then(|identifiers| identifiers.doi.as_deref())
        .and_then(|raw| Doi::parse(raw).ok())
        .map(|doi| doi.url);
    if from_doi.is_some() {
        return from_doi;
    }

    let from_arxiv = card
        .identifiers
        .as_ref()
        .and_then(|identifiers| identifiers.arxiv_id.as_deref())
        .and_then(|raw| ArxivId::parse(raw).ok())
        .map(|id| id.abs_url);
    if from_arxiv.is_some() {
        return from_arxiv;
    }

    card.open_access
        .as_ref()
        .and_then(|oa| oa.oa_url.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use omniscope_core::models::{BookPublication, ScientificIdentifiers};

    use super::*;

    fn ris_card() -> BookCard {
        let mut card = BookCard::new("Attention Is All You Need");
        card.metadata.authors = vec!["Ashish Vaswani".to_string(), "Noam Shazeer".to_string()];
        card.metadata.year = Some(2017);
        card.identifiers = Some(ScientificIdentifiers {
            doi: Some("10.48550/arXiv.1706.03762".to_string()),
            ..Default::default()
        });
        card.publication = Some(BookPublication {
            doc_type: DocumentType::Article,
            journal: Some("Advances in Neural Information Processing Systems".to_string()),
            volume: Some("30".to_string()),
            ..Default::default()
        });
        card
    }

    #[test]
    fn ris_roundtrip_preserves_title_and_doi() {
        let card = ris_card();
        let generated = generate_ris(&card);
        let parsed = parse_ris(&generated).expect("generated RIS must parse");

        assert_eq!(parsed.len(), 1);
        let entry = &parsed[0];
        assert_eq!(entry.entry_type, "JOUR");
        assert_eq!(entry.first("TI"), Some("Attention Is All You Need"));
        assert_eq!(entry.first("DO"), Some("10.48550/arxiv.1706.03762"));
    }
}
