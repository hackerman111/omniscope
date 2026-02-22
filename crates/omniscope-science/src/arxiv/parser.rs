use serde::Deserialize;
use crate::error::{Result, ScienceError};
use crate::arxiv::types::{ArxivMetadata, ArxivAuthor};
use crate::identifiers::arxiv::ArxivId;
use crate::identifiers::doi::Doi;


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
    #[serde(rename = "link", default)]
    links: Vec<AtomLink>,
    #[serde(rename = "primary_category")]
    primary_category: AtomCategory,
    #[serde(rename = "category", default)]
    categories: Vec<AtomCategory>,
    #[serde(rename = "doi")]
    doi: Option<String>,
    #[serde(rename = "journal_ref")]
    journal_ref: Option<String>,
    #[serde(rename = "comment")]
    comment: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AtomAuthor {
    name: String,
    #[serde(rename = "affiliation", default)]
    affiliations: Vec<AtomAffiliation>,
}

#[derive(Debug, Deserialize)]
struct AtomAffiliation {
    #[serde(rename = "$value")]
    name: String,
}

#[derive(Debug, Deserialize)]
struct AtomLink {
    #[serde(rename = "@href")]
    href: String,
    #[serde(rename = "@title")]
    title: Option<String>,
    #[serde(rename = "@type")]
    link_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AtomCategory {
    #[serde(rename = "@term")]
    term: String,
}

pub fn parse_atom_response(xml: &str) -> Result<Vec<ArxivMetadata>> {
    let feed: AtomFeed = quick_xml::de::from_str(xml)
        .map_err(|e| ScienceError::Parse(format!("Failed to parse Atom feed: {}", e)))?;

    let mut results = Vec::new();

    for entry in feed.entries {
        // Extract ArXiv ID from URL like http://arxiv.org/abs/1706.03762v5
        let raw_id = entry.id.split('/').next_back().unwrap_or(&entry.id);
        let arxiv_id = ArxivId::parse(raw_id)?;

        let doi = if let Some(d) = entry.doi {
            Some(Doi::parse(&d)?)
        } else {
            None
        };

        let authors = entry.authors.into_iter().map(|a| ArxivAuthor {
            name: a.name,
            affiliation: a.affiliations.first().map(|aff| aff.name.clone()),
        }).collect();

        let pdf_url = entry.links.iter()
            .find(|l| l.link_type.as_deref() == Some("application/pdf"))
            .map(|l| l.href.clone())
            .unwrap_or_else(|| format!("https://arxiv.org/pdf/{}", arxiv_id.id));

        let abs_url = entry.links.iter()
            .find(|l| l.link_type.as_deref() == Some("text/html") || l.title.is_none())
            .map(|l| l.href.clone())
            .unwrap_or_else(|| format!("https://arxiv.org/abs/{}", arxiv_id.id));

        results.push(ArxivMetadata {
            arxiv_id,
            doi,
            title: entry.title.replace('\n', " ").trim().to_string(),
            authors,
            abstract_text: entry.summary.replace('\n', " ").trim().to_string(),
            published: chrono::DateTime::parse_from_rfc3339(&entry.published)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .map_err(|e| ScienceError::Parse(format!("Invalid published date: {}", e)))?,
            updated: chrono::DateTime::parse_from_rfc3339(&entry.updated)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .map_err(|e| ScienceError::Parse(format!("Invalid updated date: {}", e)))?,
            categories: entry.categories.into_iter().map(|c| c.term).collect(),
            primary_category: entry.primary_category.term,
            comment: entry.comment,
            journal_ref: entry.journal_ref,
            pdf_url,
            abs_url,
        });
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_arxiv_atom() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:arxiv="http://arxiv.org/schemas/atom">
  <entry>
    <id>http://arxiv.org/abs/1706.03762v5</id>
    <updated>2023-08-02T03:09:44Z</updated>
    <published>2017-06-12T17:57:34Z</published>
    <title>Attention Is All You Need</title>
    <summary>The dominant sequence transduction models...</summary>
    <author>
      <name>Ashish Vaswani</name>
      <arxiv:affiliation>Google Brain</arxiv:affiliation>
    </author>
    <arxiv:doi xmlns:arxiv="http://arxiv.org/schemas/atom">10.48550/arXiv.1706.03762</arxiv:doi>
    <arxiv:primary_category xmlns:arxiv="http://arxiv.org/schemas/atom" term="cs.CL" scheme="http://arxiv.org/schemas/atom"/>
    <category term="cs.CL" scheme="http://arxiv.org/schemas/atom"/>
    <category term="cs.LG" scheme="http://arxiv.org/schemas/atom"/>
    <link href="http://arxiv.org/abs/1706.03762v5" rel="alternate" type="text/html"/>
    <link title="pdf" href="http://arxiv.org/pdf/1706.03762v5" rel="related" type="application/pdf"/>
  </entry>
</feed>"#;

        let results = parse_atom_response(xml).unwrap();
        assert_eq!(results.len(), 1);
        let meta = &results[0];
        assert_eq!(meta.title, "Attention Is All You Need");
        assert_eq!(meta.arxiv_id.id, "1706.03762");
        assert_eq!(meta.arxiv_id.version, Some(5));
        assert_eq!(meta.authors[0].name, "Ashish Vaswani");
        assert_eq!(meta.authors[0].affiliation, Some("Google Brain".to_string()));
        assert_eq!(meta.primary_category, "cs.CL");
        assert!(meta.doi.is_some());
        assert_eq!(meta.doi.as_ref().unwrap().normalized, "10.48550/arxiv.1706.03762");
    }
}
