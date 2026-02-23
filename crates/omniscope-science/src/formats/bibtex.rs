use std::collections::BTreeMap;

use omniscope_core::models::{BookCard, DocumentType};

use crate::error::{Result, ScienceError};
use crate::identifiers::{arxiv::ArxivId, doi::Doi};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BibEntry {
    pub entry_type: String,
    pub cite_key: String,
    pub fields: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BibTeXOptions {
    pub cite_key_scheme: CiteKeyScheme,
    pub utf8: bool,
}

impl Default for BibTeXOptions {
    fn default() -> Self {
        Self {
            cite_key_scheme: CiteKeyScheme::AuthorYearTitle,
            utf8: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CiteKeyScheme {
    AuthorYear,
    AuthorYearTitle,
    DoiBased,
    Custom(String),
}

pub fn generate_bibtex(card: &BookCard, opts: &BibTeXOptions) -> String {
    let entry = from_book_card(card, opts);
    render_bib_entry(&entry, opts.utf8)
}

pub fn from_book_card(card: &BookCard, opts: &BibTeXOptions) -> BibEntry {
    let cite_key = generate_cite_key(card, &opts.cite_key_scheme);
    let entry_type = document_type_to_bibtex(
        card.publication
            .as_ref()
            .map(|publication| publication.doc_type)
            .unwrap_or(DocumentType::Book),
    )
    .to_string();

    let mut fields = BTreeMap::new();
    insert_field(&mut fields, "title", Some(card.metadata.title.clone()));

    if !card.metadata.authors.is_empty() {
        let joined = card
            .metadata
            .authors
            .iter()
            .map(|author| author_to_bibtex(author))
            .collect::<Vec<_>>()
            .join(" and ");
        insert_field(&mut fields, "author", Some(joined));
    }

    insert_field(
        &mut fields,
        "year",
        card.metadata.year.map(|year| year.to_string()),
    );
    insert_field(&mut fields, "publisher", card.metadata.publisher.clone());

    if let Some(publication) = card.publication.as_ref() {
        match publication.doc_type {
            DocumentType::ConferencePaper => {
                let booktitle = publication
                    .conference
                    .clone()
                    .or_else(|| publication.venue.clone())
                    .or_else(|| publication.journal.clone());
                insert_field(&mut fields, "booktitle", booktitle);
            }
            _ => {
                let journal = publication
                    .journal
                    .clone()
                    .or_else(|| publication.venue.clone())
                    .or_else(|| publication.conference.clone());
                insert_field(&mut fields, "journal", journal);
            }
        }

        insert_field(&mut fields, "volume", publication.volume.clone());
        insert_field(&mut fields, "number", publication.issue.clone());
        insert_field(&mut fields, "pages", publication.pages.clone());
    }

    if let Some(identifiers) = card.identifiers.as_ref() {
        insert_field(
            &mut fields,
            "doi",
            identifiers
                .doi
                .as_deref()
                .map(normalize_doi_for_export)
                .or_else(|| Some(String::new()))
                .filter(|value| !value.is_empty()),
        );

        if let Some(arxiv_raw) = identifiers
            .arxiv_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let parsed = ArxivId::parse(arxiv_raw).ok();
            let arxiv_id = parsed
                .as_ref()
                .map(|id| id.id.clone())
                .unwrap_or_else(|| arxiv_raw.to_string());

            insert_field(&mut fields, "arxivid", Some(arxiv_id.clone()));
            insert_field(&mut fields, "eprint", Some(arxiv_id));
            insert_field(&mut fields, "archiveprefix", Some("arXiv".to_string()));
            insert_field(
                &mut fields,
                "primaryclass",
                parsed
                    .and_then(|id| id.category)
                    .or_else(|| extract_primary_class(card)),
            );
        }

        let isbn = identifiers
            .isbn13
            .as_deref()
            .or(identifiers.isbn10.as_deref())
            .map(|raw| raw.trim().to_string())
            .filter(|value| !value.is_empty());
        insert_field(&mut fields, "isbn", isbn);
    } else if !card.metadata.isbn.is_empty() {
        insert_field(&mut fields, "isbn", card.metadata.isbn.first().cloned());
    }

    insert_field(&mut fields, "url", export_url(card));

    BibEntry {
        entry_type,
        cite_key,
        fields,
    }
}

pub fn generate_cite_key(card: &BookCard, scheme: &CiteKeyScheme) -> String {
    let first_author = first_author_family_name(card);
    let year = year_fragment(card);
    let title_word = title_word(card);

    let mut key = match scheme {
        CiteKeyScheme::AuthorYear => format!("{first_author}{year}"),
        CiteKeyScheme::AuthorYearTitle => format!("{first_author}{year}{title_word}"),
        CiteKeyScheme::DoiBased => {
            doi_based_key(card).unwrap_or_else(|| format!("{first_author}{year}{title_word}"))
        }
        CiteKeyScheme::Custom(template) => template
            .replace("{first_author}", &first_author)
            .replace("{year}", &year)
            .replace("{title_word}", &title_word)
            .replace("{doi}", &doi_based_key(card).unwrap_or_default()),
    };

    if key.is_empty() {
        key = "untitled0".to_string();
    }
    sanitize_cite_key(key)
}

pub fn parse_bibtex(content: &str) -> Result<Vec<BibEntry>> {
    let mut parser = BibParser::new(content);
    parser.parse_entries()
}

fn document_type_to_bibtex(doc_type: DocumentType) -> &'static str {
    match doc_type {
        DocumentType::Article | DocumentType::MagazineArticle => "article",
        DocumentType::Book => "book",
        DocumentType::ConferencePaper => "inproceedings",
        DocumentType::Preprint => "misc",
        DocumentType::Thesis => "phdthesis",
        DocumentType::Report => "techreport",
        DocumentType::Dataset => "misc",
        DocumentType::Software => "misc",
        DocumentType::Patent => "misc",
        DocumentType::Standard => "manual",
        DocumentType::Chapter => "incollection",
        DocumentType::WebPage => "misc",
        DocumentType::Other => "misc",
    }
}

fn render_bib_entry(entry: &BibEntry, utf8: bool) -> String {
    let mut lines = Vec::new();
    lines.push(format!("@{}{{{},", entry.entry_type, entry.cite_key));

    let ordered = ordered_fields(&entry.fields);
    for (index, (name, value)) in ordered.iter().enumerate() {
        let escaped = escape_bibtex_value(value, utf8);
        let suffix = if index + 1 == ordered.len() { "" } else { "," };
        lines.push(format!("  {name} = {{{escaped}}}{suffix}"));
    }

    lines.push("}".to_string());
    lines.push(String::new());
    lines.join("\n")
}

fn ordered_fields(fields: &BTreeMap<String, String>) -> Vec<(String, String)> {
    const PREFERRED: &[&str] = &[
        "title",
        "author",
        "journal",
        "booktitle",
        "volume",
        "number",
        "pages",
        "year",
        "publisher",
        "doi",
        "arxivid",
        "eprint",
        "archiveprefix",
        "primaryclass",
        "isbn",
        "url",
    ];

    let mut ordered = Vec::new();
    for key in PREFERRED {
        if let Some(value) = fields.get(*key) {
            ordered.push(((*key).to_string(), value.clone()));
        }
    }

    for (key, value) in fields {
        if PREFERRED.iter().any(|preferred| preferred == &key.as_str()) {
            continue;
        }
        ordered.push((key.clone(), value.clone()));
    }

    ordered
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

fn extract_primary_class(card: &BookCard) -> Option<String> {
    let candidates = [
        card.publication
            .as_ref()
            .and_then(|publication| publication.venue.as_deref()),
        card.publication
            .as_ref()
            .and_then(|publication| publication.journal.as_deref()),
        card.publication
            .as_ref()
            .and_then(|publication| publication.conference.as_deref()),
    ];

    for candidate in candidates.into_iter().flatten() {
        if let Some(class) = candidate
            .split_whitespace()
            .find(|part| is_arxiv_primary_class(part))
        {
            return Some(class.trim_matches(|ch| ch == '[' || ch == ']').to_string());
        }
    }

    None
}

fn is_arxiv_primary_class(value: &str) -> bool {
    let value = value.trim_matches(|ch| ch == '[' || ch == ']');
    let mut parts = value.split('.');
    let Some(prefix) = parts.next() else {
        return false;
    };
    let Some(suffix) = parts.next() else {
        return false;
    };
    parts.next().is_none()
        && !prefix.is_empty()
        && !suffix.is_empty()
        && prefix.chars().all(|ch| ch.is_ascii_alphabetic())
        && suffix.chars().all(|ch| ch.is_ascii_alphabetic())
}

fn insert_field(fields: &mut BTreeMap<String, String>, name: &str, value: Option<String>) {
    let Some(value) = value else {
        return;
    };
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    fields.insert(name.to_string(), value.to_string());
}

fn normalize_doi_for_export(raw: &str) -> String {
    Doi::parse(raw)
        .map(|doi| doi.normalized)
        .unwrap_or_else(|_| raw.trim().to_string())
}

fn first_author_family_name(card: &BookCard) -> String {
    card.metadata
        .authors
        .first()
        .map(|author| author_family_name(author))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Unknown".to_string())
}

fn author_family_name(author: &str) -> String {
    let author = author.trim();
    if author.is_empty() {
        return String::new();
    }
    if let Some((family, _)) = author.split_once(',') {
        return capitalize_word(family);
    }

    author
        .split_whitespace()
        .last()
        .map(capitalize_word)
        .unwrap_or_default()
}

fn title_word(card: &BookCard) -> String {
    card.metadata
        .title
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .find(|word| !word.is_empty())
        .map(capitalize_word)
        .filter(|word| !word.is_empty())
        .unwrap_or_else(|| "Untitled".to_string())
}

fn year_fragment(card: &BookCard) -> String {
    card.metadata
        .year
        .map(|year| year.to_string())
        .unwrap_or_else(|| "0".to_string())
}

fn doi_based_key(card: &BookCard) -> Option<String> {
    let doi = card
        .identifiers
        .as_ref()
        .and_then(|identifiers| identifiers.doi.as_deref())?;
    let parsed = Doi::parse(doi).ok()?;
    let lowered = parsed.normalized.to_lowercase();

    if let Some(raw_arxiv) = lowered.strip_prefix("10.48550/arxiv.") {
        let clean = raw_arxiv
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '.')
            .collect::<String>();
        if !clean.is_empty() {
            return Some(format!("arXiv{clean}"));
        }
    }

    let compact = lowered
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>();
    if compact.is_empty() {
        None
    } else {
        Some(compact)
    }
}

fn sanitize_cite_key(input: String) -> String {
    input
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '-' || *ch == '.')
        .collect::<String>()
}

fn capitalize_word(value: &str) -> String {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut result = String::new();
    result.push(first.to_ascii_uppercase());
    result.push_str(chars.as_str());
    sanitize_cite_key(result)
}

fn author_to_bibtex(value: &str) -> String {
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

fn escape_bibtex_value(value: &str, utf8: bool) -> String {
    let mut escaped = value.replace('\n', " ");
    if !utf8 {
        escaped = escaped
            .chars()
            .map(|ch| if ch.is_ascii() { ch } else { '?' })
            .collect();
    }
    escaped.trim().to_string()
}

struct BibParser<'a> {
    input: &'a str,
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> BibParser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            bytes: input.as_bytes(),
            pos: 0,
        }
    }

    fn parse_entries(&mut self) -> Result<Vec<BibEntry>> {
        let mut entries = Vec::new();
        while let Some(at_pos) = self.find_next(b'@') {
            self.pos = at_pos + 1;
            self.skip_ws();
            let entry_type = self.parse_identifier("entry type")?.to_lowercase();

            self.skip_ws();
            let (open_delim, close_delim) = match self.peek_byte() {
                Some(b'{') => (b'{', b'}'),
                Some(b'(') => (b'(', b')'),
                _ => {
                    return Err(self.error("expected '{' or '(' after entry type"));
                }
            };
            self.pos += 1;
            let _ = open_delim;
            self.skip_ws();

            let cite_key = self.parse_cite_key(close_delim)?;
            let mut fields = BTreeMap::new();

            if self.peek_byte() == Some(close_delim) {
                self.pos += 1;
                entries.push(BibEntry {
                    entry_type,
                    cite_key,
                    fields,
                });
                continue;
            }

            self.expect_byte(b',', "expected ',' after cite key")?;
            loop {
                self.skip_ws_and_commas();
                if self.peek_byte() == Some(close_delim) {
                    self.pos += 1;
                    break;
                }
                if self.peek_byte().is_none() {
                    return Err(self.error("unterminated BibTeX entry"));
                }

                let name = self.parse_identifier("field name")?.to_lowercase();
                self.skip_ws();
                self.expect_byte(b'=', "expected '=' after field name")?;
                self.skip_ws();
                let value = self.parse_value(close_delim)?;
                fields.insert(name, value);
                self.skip_ws();
                if self.peek_byte() == Some(b',') {
                    self.pos += 1;
                }
            }

            entries.push(BibEntry {
                entry_type,
                cite_key,
                fields,
            });
        }
        Ok(entries)
    }

    fn parse_cite_key(&mut self, close_delim: u8) -> Result<String> {
        let start = self.pos;
        while let Some(byte) = self.peek_byte() {
            if byte == b',' || byte == close_delim {
                break;
            }
            self.pos += 1;
        }
        let value = self.input[start..self.pos].trim();
        if value.is_empty() {
            return Err(self.error("empty cite key"));
        }
        Ok(value.to_string())
    }

    fn parse_value(&mut self, close_delim: u8) -> Result<String> {
        match self.peek_byte() {
            Some(b'{') => self.parse_braced_value(),
            Some(b'"') => self.parse_quoted_value(),
            Some(_) => self.parse_bare_value(close_delim),
            None => Err(self.error("missing field value")),
        }
    }

    fn parse_braced_value(&mut self) -> Result<String> {
        self.expect_byte(b'{', "expected '{' to start value")?;
        let mut depth = 1usize;
        let mut out = String::new();

        while let Some(byte) = self.peek_byte() {
            self.pos += 1;
            match byte {
                b'{' => {
                    depth += 1;
                    out.push('{');
                }
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(out.trim().to_string());
                    }
                    out.push('}');
                }
                _ => out.push(byte as char),
            }
        }

        Err(self.error("unterminated braced value"))
    }

    fn parse_quoted_value(&mut self) -> Result<String> {
        self.expect_byte(b'"', "expected '\"' to start value")?;
        let mut out = String::new();
        let mut escaped = false;

        while let Some(byte) = self.peek_byte() {
            self.pos += 1;
            if escaped {
                out.push(byte as char);
                escaped = false;
                continue;
            }
            if byte == b'\\' {
                escaped = true;
                continue;
            }
            if byte == b'"' {
                return Ok(out.trim().to_string());
            }
            out.push(byte as char);
        }

        Err(self.error("unterminated quoted value"))
    }

    fn parse_bare_value(&mut self, close_delim: u8) -> Result<String> {
        let start = self.pos;
        while let Some(byte) = self.peek_byte() {
            if byte == b',' || byte == close_delim {
                break;
            }
            self.pos += 1;
        }
        let value = self.input[start..self.pos].trim();
        if value.is_empty() {
            return Err(self.error("empty bare value"));
        }
        Ok(value.to_string())
    }

    fn parse_identifier(&mut self, what: &str) -> Result<String> {
        let start = self.pos;
        while let Some(byte) = self.peek_byte() {
            let valid = byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b':' | b'.');
            if !valid {
                break;
            }
            self.pos += 1;
        }

        if self.pos == start {
            return Err(self.error(format!("expected {what}").as_str()));
        }

        Ok(self.input[start..self.pos].trim().to_string())
    }

    fn skip_ws(&mut self) {
        while let Some(byte) = self.peek_byte() {
            if !byte.is_ascii_whitespace() {
                break;
            }
            self.pos += 1;
        }
    }

    fn skip_ws_and_commas(&mut self) {
        loop {
            match self.peek_byte() {
                Some(byte) if byte.is_ascii_whitespace() || byte == b',' => self.pos += 1,
                _ => break,
            }
        }
    }

    fn find_next(&self, needle: u8) -> Option<usize> {
        self.bytes[self.pos..]
            .iter()
            .position(|byte| *byte == needle)
            .map(|offset| self.pos + offset)
    }

    fn expect_byte(&mut self, expected: u8, message: &str) -> Result<()> {
        match self.peek_byte() {
            Some(byte) if byte == expected => {
                self.pos += 1;
                Ok(())
            }
            _ => Err(self.error(message)),
        }
    }

    fn peek_byte(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn error(&self, message: &str) -> ScienceError {
        ScienceError::Parse(format!("{message} at byte {}", self.pos))
    }
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
            arxiv_id: Some("1706.03762".to_string()),
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
    fn generate_bibtex_attention_contains_expected_fields() {
        let card = attention_card();
        let opts = BibTeXOptions::default();
        let bibtex = generate_bibtex(&card, &opts);

        assert!(bibtex.contains("@article{Vaswani2017Attention,"));
        assert!(bibtex.contains("doi = {10.48550/arxiv.1706.03762}"));
        assert!(bibtex.contains("arxivid = {1706.03762}"));
        assert!(bibtex.contains("author = {Vaswani, Ashish and Shazeer, Noam and Parmar, Niki}"));
    }

    #[test]
    fn parse_bibtex_generated_output_roundtrip() {
        let card = attention_card();
        let opts = BibTeXOptions::default();
        let text = generate_bibtex(&card, &opts);
        let parsed = parse_bibtex(&text).expect("parser should handle generated BibTeX");

        assert_eq!(parsed.len(), 1);
        let first = &parsed[0];
        assert_eq!(first.entry_type, "article");
        assert_eq!(first.cite_key, "Vaswani2017Attention");
        assert_eq!(
            first.fields.get("doi").map(String::as_str),
            Some("10.48550/arxiv.1706.03762")
        );
    }

    #[test]
    fn generate_cite_key_doi_based_arxiv_special_case() {
        let card = attention_card();
        let key = generate_cite_key(&card, &CiteKeyScheme::DoiBased);
        assert_eq!(key, "arXiv1706.03762");
    }
}
