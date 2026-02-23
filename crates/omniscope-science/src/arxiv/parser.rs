use chrono::{DateTime, Utc};
use quick_xml::de::from_str;
use serde::Deserialize;

use crate::arxiv::types::{ArxivAuthor, ArxivMetadata};
use crate::error::{Result, ScienceError};
use crate::identifiers::{arxiv::ArxivId, doi::Doi};

#[derive(Debug, Deserialize)]
struct AtomFeed {
    #[serde(rename = "entry", default)]
    entries: Vec<AtomEntry>,
}

#[derive(Debug, Deserialize)]
struct AtomEntry {
    id: String,
    title: String,
    summary: String,
    published: String,
    updated: String,
    #[serde(rename = "author", default)]
    authors: Vec<AtomAuthor>,
    #[serde(rename = "category", default)]
    categories: Vec<AtomCategory>,
    #[serde(rename = "arxiv:primary_category", alias = "primary_category")]
    primary_category: Option<AtomCategory>,
    #[serde(rename = "arxiv:comment", alias = "comment")]
    comment: Option<String>,
    #[serde(rename = "arxiv:journal_ref", alias = "journal_ref")]
    journal_ref: Option<String>,
    #[serde(rename = "arxiv:doi", alias = "doi")]
    doi: Option<String>,
    #[serde(rename = "link", default)]
    links: Vec<AtomLink>,
}

#[derive(Debug, Deserialize)]
struct AtomAuthor {
    name: String,
    #[serde(rename = "arxiv:affiliation", alias = "affiliation")]
    affiliation: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AtomCategory {
    #[serde(rename = "@term")]
    term: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AtomLink {
    #[serde(rename = "@href")]
    href: Option<String>,
    #[serde(rename = "@type")]
    link_type: Option<String>,
}

pub fn parse_atom_response(xml: &str) -> Result<Vec<ArxivMetadata>> {
    let feed: AtomFeed =
        from_str(xml).map_err(|e| ScienceError::Parse(format!("invalid atom xml: {e}")))?;

    feed.entries.into_iter().map(parse_entry).collect()
}

fn parse_entry(entry: AtomEntry) -> Result<ArxivMetadata> {
    let arxiv_id = ArxivId::parse(entry.id.trim())
        .map_err(|_| ScienceError::Parse(format!("invalid arXiv id in entry: {}", entry.id)))?;

    let title = clean_text(&entry.title);
    let abstract_text = clean_text(&entry.summary);

    let published = parse_rfc3339(&entry.published, "published")?;
    let updated = parse_rfc3339(&entry.updated, "updated")?;

    let authors = entry
        .authors
        .into_iter()
        .map(|author| ArxivAuthor {
            name: clean_text(&author.name),
            affiliation: clean_optional(author.affiliation),
        })
        .collect::<Vec<_>>();

    let categories = entry
        .categories
        .into_iter()
        .filter_map(|category| clean_optional(category.term))
        .collect::<Vec<_>>();

    let primary_category = entry
        .primary_category
        .and_then(|category| clean_optional(category.term))
        .or_else(|| categories.first().cloned())
        .unwrap_or_default();

    let doi = entry.doi.and_then(|value| Doi::parse(value.trim()).ok());

    let pdf_url = entry
        .links
        .iter()
        .find(|link| link.link_type.as_deref() == Some("application/pdf"))
        .and_then(|link| link.href.as_ref())
        .map(|url| normalize_arxiv_url(url))
        .unwrap_or_else(|| arxiv_id.pdf_url.clone());

    Ok(ArxivMetadata {
        arxiv_id: arxiv_id.clone(),
        doi,
        title,
        authors,
        abstract_text,
        published,
        updated,
        categories,
        primary_category,
        comment: clean_optional(entry.comment),
        journal_ref: clean_optional(entry.journal_ref),
        pdf_url,
        abs_url: arxiv_id.abs_url,
    })
}

fn parse_rfc3339(value: &str, field_name: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value.trim())
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| ScienceError::Parse(format!("invalid {field_name} datetime: {e}")))
}

fn clean_text(input: &str) -> String {
    input
        .split_whitespace()
        .filter(|chunk| !chunk.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value.map(|v| clean_text(&v)).filter(|v| !v.is_empty())
}

fn normalize_arxiv_url(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("http://arxiv.org/") {
        return format!("https://arxiv.org/{rest}");
    }
    url.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ATTENTION_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom"
      xmlns:opensearch="http://a9.com/-/spec/opensearch/1.1/"
      xmlns:arxiv="http://arxiv.org/schemas/atom">
  <id>http://arxiv.org/api/query?search_query=id:1706.03762</id>
  <updated>2024-01-01T00:00:00Z</updated>
  <entry>
    <id>http://arxiv.org/abs/1706.03762v7</id>
    <updated>2023-08-02T17:54:37Z</updated>
    <published>2017-06-12T17:57:40Z</published>
    <title>
      Attention Is All You Need
    </title>
    <summary>
      The dominant sequence transduction models are based on recurrent or convolutional neural networks.
    </summary>
    <author>
      <name>Ashish Vaswani</name>
      <arxiv:affiliation>Google Brain</arxiv:affiliation>
    </author>
    <author>
      <name>Noam Shazeer</name>
      <arxiv:affiliation>Google Brain</arxiv:affiliation>
    </author>
    <arxiv:comment>Accepted at NIPS 2017</arxiv:comment>
    <arxiv:journal_ref>NeurIPS 2017</arxiv:journal_ref>
    <arxiv:doi>10.48550/arXiv.1706.03762</arxiv:doi>
    <link rel="alternate" type="text/html" href="http://arxiv.org/abs/1706.03762v7" />
    <link title="pdf" rel="related" type="application/pdf" href="http://arxiv.org/pdf/1706.03762v7" />
    <arxiv:primary_category term="cs.CL" scheme="http://arxiv.org/schemas/atom"/>
    <category term="cs.CL" scheme="http://arxiv.org/schemas/atom"/>
    <category term="cs.AI" scheme="http://arxiv.org/schemas/atom"/>
  </entry>
</feed>
"#;

    #[test]
    fn parses_attention_fixture() {
        let entries = parse_atom_response(ATTENTION_XML).unwrap();
        assert_eq!(entries.len(), 1);

        let item = &entries[0];
        assert_eq!(item.arxiv_id.id, "1706.03762");
        assert_eq!(item.arxiv_id.version, Some(7));
        assert_eq!(item.title, "Attention Is All You Need");
        assert!(
            item.abstract_text
                .contains("dominant sequence transduction models")
        );
        assert_eq!(item.authors.len(), 2);
        assert_eq!(item.authors[0].name, "Ashish Vaswani");
        assert_eq!(item.authors[0].affiliation.as_deref(), Some("Google Brain"));
        assert_eq!(item.primary_category, "cs.CL");
        assert_eq!(
            item.categories,
            vec!["cs.CL".to_string(), "cs.AI".to_string()]
        );
        assert_eq!(item.comment.as_deref(), Some("Accepted at NIPS 2017"));
        assert_eq!(item.journal_ref.as_deref(), Some("NeurIPS 2017"));
        assert_eq!(
            item.doi.as_ref().map(|doi| doi.normalized.as_str()),
            Some("10.48550/arxiv.1706.03762")
        );
        assert_eq!(item.pdf_url, "https://arxiv.org/pdf/1706.03762v7");
        assert_eq!(item.abs_url, "https://arxiv.org/abs/1706.03762");
        assert_eq!(item.published.to_rfc3339(), "2017-06-12T17:57:40+00:00");
        assert_eq!(item.updated.to_rfc3339(), "2023-08-02T17:54:37+00:00");
    }
}
