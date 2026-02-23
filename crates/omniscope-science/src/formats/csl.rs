use omniscope_core::models::{BookCard, DocumentType};

use crate::error::{Result, ScienceError};
use crate::identifiers::{arxiv::ArxivId, doi::Doi};

pub const BUNDLED_STYLES: &[&str] = &[
    "apa",
    "apa-6th-edition",
    "ieee",
    "gost-r-7-0-5-2008",
    "russian-gost-r-7-0-5-2008",
];

#[derive(Debug, Clone)]
pub struct CslProcessor {
    pub locale: String,
}

impl Default for CslProcessor {
    fn default() -> Self {
        Self {
            locale: "en-US".to_string(),
        }
    }
}

impl CslProcessor {
    pub fn new(locale: impl Into<String>) -> Self {
        Self {
            locale: locale.into(),
        }
    }

    pub fn format_citation(&self, card: &BookCard, style: &str) -> Result<String> {
        let _ = &self.locale;
        let item = card_to_csl_item(card);
        format_item(&item, style)
    }

    pub fn format_bibliography(&self, cards: &[&BookCard], style: &str) -> Result<Vec<String>> {
        let _ = &self.locale;
        cards
            .iter()
            .map(|card| self.format_citation(card, style))
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CslItem {
    pub csl_type: String,
    pub title: String,
    pub authors: Vec<String>,
    pub year: Option<i32>,
    pub journal: Option<String>,
    pub volume: Option<String>,
    pub doi: Option<String>,
    pub url: Option<String>,
    pub publisher: Option<String>,
}

pub fn format_bibliography(cards: &[&BookCard], style: &str) -> Result<Vec<String>> {
    CslProcessor::default().format_bibliography(cards, style)
}

pub fn card_to_csl_item(card: &BookCard) -> CslItem {
    let csl_type = card
        .publication
        .as_ref()
        .map(|publication| publication_type_to_csl(publication.doc_type))
        .unwrap_or("book")
        .to_string();

    let doi = card
        .identifiers
        .as_ref()
        .and_then(|identifiers| identifiers.doi.as_deref())
        .map(normalize_doi_for_export)
        .or_else(|| Some(String::new()))
        .filter(|value| !value.is_empty());

    let url = derive_url(card);
    let (journal, volume) = if let Some(publication) = card.publication.as_ref() {
        (
            publication
                .journal
                .clone()
                .or_else(|| publication.venue.clone())
                .or_else(|| publication.conference.clone()),
            publication.volume.clone(),
        )
    } else {
        (None, None)
    };

    CslItem {
        csl_type,
        title: card.metadata.title.clone(),
        authors: card.metadata.authors.clone(),
        year: card.metadata.year,
        journal,
        volume,
        doi,
        url,
        publisher: card.metadata.publisher.clone(),
    }
}

fn format_item(item: &CslItem, style: &str) -> Result<String> {
    let style_key = normalize_style(style);
    match style_key.as_str() {
        "apa" | "apa-6th-edition" => Ok(format_apa(item)),
        "ieee" => Ok(format_ieee(item)),
        "gost-r-7-0-5-2008" | "russian-gost-r-7-0-5-2008" => Ok(format_gost(item)),
        _ => Err(ScienceError::Parse(format!(
            "unsupported CSL style: {style}"
        ))),
    }
}

fn format_apa(item: &CslItem) -> String {
    let authors = format_authors_apa(&item.authors);
    let year = item
        .year
        .map(|y| y.to_string())
        .unwrap_or_else(|| "n.d.".to_string());

    let mut parts = Vec::new();
    parts.push(format!("{authors} ({year})."));
    parts.push(format!("{}.", item.title.trim()));

    if let Some(journal) = item
        .journal
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        if let Some(volume) = item
            .volume
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            parts.push(format!("{journal}, {volume}."));
        } else {
            parts.push(format!("{journal}."));
        }
    }

    if let Some(doi) = item.doi.as_deref() {
        parts.push(format!("doi:{doi}"));
    } else if let Some(url) = item.url.as_deref() {
        parts.push(url.to_string());
    }

    parts.join(" ")
}

fn format_ieee(item: &CslItem) -> String {
    let authors = format_authors_ieee(&item.authors);
    let mut out = format!("{authors}, \"{}\"", item.title.trim());

    if let Some(journal) = item
        .journal
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        out.push_str(&format!(", in {journal}"));
    }
    if let Some(volume) = item
        .volume
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        out.push_str(&format!(", vol. {volume}"));
    }
    if let Some(year) = item.year {
        out.push_str(&format!(", {year}"));
    }
    out.push('.');

    if let Some(doi) = item.doi.as_deref() {
        out.push_str(&format!(" doi: {doi}"));
    }

    out
}

fn format_gost(item: &CslItem) -> String {
    let mut authors = format_authors_gost(&item.authors);
    if !authors.ends_with('.') {
        authors.push('.');
    }
    let mut out = format!("{authors} {} ", item.title.trim());

    if let Some(journal) = item
        .journal
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        out.push_str(&format!("// {journal}. "));
    }

    if let Some(year) = item.year {
        out.push_str(&format!("{year}. "));
    }
    if let Some(volume) = item
        .volume
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        out.push_str(&format!("Vol. {volume}. "));
    }
    if let Some(doi) = item.doi.as_deref() {
        out.push_str(&format!("DOI: {doi}"));
    } else if let Some(url) = item.url.as_deref() {
        out.push_str(url);
    }

    out.trim().to_string()
}

fn publication_type_to_csl(doc_type: DocumentType) -> &'static str {
    match doc_type {
        DocumentType::Article | DocumentType::MagazineArticle => "article-journal",
        DocumentType::Book => "book",
        DocumentType::ConferencePaper => "paper-conference",
        DocumentType::Preprint => "article",
        DocumentType::Thesis => "thesis",
        DocumentType::Report => "report",
        DocumentType::Dataset => "dataset",
        DocumentType::Software => "software",
        DocumentType::WebPage => "webpage",
        _ => "article",
    }
}

fn normalize_doi_for_export(raw: &str) -> String {
    Doi::parse(raw)
        .map(|doi| doi.normalized)
        .unwrap_or_else(|_| raw.trim().to_string())
}

fn derive_url(card: &BookCard) -> Option<String> {
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

fn normalize_style(style: &str) -> String {
    style.trim().to_ascii_lowercase()
}

fn format_authors_apa(authors: &[String]) -> String {
    let formatted = authors
        .iter()
        .filter_map(|author| {
            let parts = split_name(author)?;
            Some(format!(
                "{}, {}",
                parts.family,
                initials(&parts.given_names).join(" ")
            ))
        })
        .collect::<Vec<_>>();

    match formatted.len() {
        0 => "Unknown Author".to_string(),
        1 => formatted[0].clone(),
        2 => format!("{} & {}", formatted[0], formatted[1]),
        _ => {
            let mut joined = formatted[..formatted.len() - 1].join(", ");
            joined.push_str(", & ");
            joined.push_str(&formatted[formatted.len() - 1]);
            joined
        }
    }
}

fn format_authors_ieee(authors: &[String]) -> String {
    let formatted = authors
        .iter()
        .filter_map(|author| {
            let parts = split_name(author)?;
            let initials = initials(&parts.given_names).join(" ");
            let label = if initials.is_empty() {
                parts.family
            } else {
                format!("{initials} {}", parts.family)
            };
            Some(label)
        })
        .collect::<Vec<_>>();

    match formatted.len() {
        0 => "Unknown Author".to_string(),
        1 => formatted[0].clone(),
        2 => format!("{} and {}", formatted[0], formatted[1]),
        _ => format!("{} et al.", formatted[0]),
    }
}

fn format_authors_gost(authors: &[String]) -> String {
    let formatted = authors
        .iter()
        .filter_map(|author| {
            let parts = split_name(author)?;
            let initials = initials(&parts.given_names).join(" ");
            let label = if initials.is_empty() {
                parts.family
            } else {
                format!("{} {}", parts.family, initials)
            };
            Some(label)
        })
        .collect::<Vec<_>>();

    if formatted.is_empty() {
        "Unknown Author".to_string()
    } else {
        formatted.join(", ")
    }
}

fn initials(parts: &[String]) -> Vec<String> {
    parts
        .iter()
        .filter_map(|part| {
            part.chars()
                .find(|ch| ch.is_ascii_alphabetic())
                .map(|ch| format!("{}.", ch.to_ascii_uppercase()))
        })
        .collect()
}

#[derive(Debug, Clone)]
struct NameParts {
    family: String,
    given_names: Vec<String>,
}

fn split_name(value: &str) -> Option<NameParts> {
    let cleaned = value.trim();
    if cleaned.is_empty() {
        return None;
    }

    if let Some((family, given)) = cleaned.split_once(',') {
        let family = family.trim();
        if family.is_empty() {
            return None;
        }
        let given_names = given
            .split_whitespace()
            .filter(|part| !part.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        return Some(NameParts {
            family: family.to_string(),
            given_names,
        });
    }

    let tokens = cleaned.split_whitespace().collect::<Vec<_>>();
    if tokens.is_empty() {
        return None;
    }
    if tokens.len() == 1 {
        return Some(NameParts {
            family: tokens[0].to_string(),
            given_names: Vec::new(),
        });
    }

    let family = tokens[tokens.len() - 1].to_string();
    let given_names = tokens[..tokens.len() - 1]
        .iter()
        .map(|token| (*token).to_string())
        .collect::<Vec<_>>();

    Some(NameParts {
        family,
        given_names,
    })
}

#[cfg(test)]
mod tests {
    use omniscope_core::models::{BookPublication, ScientificIdentifiers};

    use super::*;

    fn attention_card() -> BookCard {
        let mut card = BookCard::new("Attention Is All You Need");
        card.metadata.authors = vec![
            "Ashish Vaswani".to_string(),
            "Noam Shazeer".to_string(),
            "Niki Parmar".to_string(),
        ];
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
    fn formats_apa_citation() {
        let card = attention_card();
        let processor = CslProcessor::new("en-US");
        let formatted = processor
            .format_citation(&card, "apa")
            .expect("apa formatting should succeed");

        assert_eq!(
            formatted,
            "Vaswani, A., Shazeer, N., & Parmar, N. (2017). Attention Is All You Need. Advances in Neural Information Processing Systems, 30. doi:10.48550/arxiv.1706.03762"
        );
    }

    #[test]
    fn formats_ieee_citation() {
        let card = attention_card();
        let processor = CslProcessor::default();
        let formatted = processor
            .format_citation(&card, "ieee")
            .expect("ieee formatting should succeed");

        assert_eq!(
            formatted,
            "A. Vaswani et al., \"Attention Is All You Need\", in Advances in Neural Information Processing Systems, vol. 30, 2017. doi: 10.48550/arxiv.1706.03762"
        );
    }

    #[test]
    fn formats_gost_citation() {
        let card = attention_card();
        let processor = CslProcessor::new("ru-RU");
        let formatted = processor
            .format_citation(&card, "gost-r-7-0-5-2008")
            .expect("gost formatting should succeed");

        assert_eq!(
            formatted,
            "Vaswani A., Shazeer N., Parmar N. Attention Is All You Need // Advances in Neural Information Processing Systems. 2017. Vol. 30. DOI: 10.48550/arxiv.1706.03762"
        );
    }

    #[test]
    fn format_bibliography_for_list() {
        let first = attention_card();
        let mut second = attention_card();
        second.metadata.title = "A Different Paper".to_string();

        let entries = format_bibliography(&[&first, &second], "apa")
            .expect("bibliography formatting should succeed");

        assert_eq!(entries.len(), 2);
        assert!(entries[0].contains("Attention Is All You Need"));
        assert!(entries[1].contains("A Different Paper"));
    }
}
