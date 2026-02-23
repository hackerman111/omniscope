use std::sync::Arc;

use chrono::Datelike;
use omniscope_core::models::{BookCard, DocumentType, WebSource};
use omniscope_core::storage::database::Database;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::arxiv::client::ArxivClient;
use crate::arxiv::types::ArxivMetadata;
use crate::enrichment::merge::{BookCardMergeExt, MetadataSource, PartialMetadata};
use crate::error::{Result, ScienceError};
use crate::identifiers::arxiv::ArxivId;

const SCAN_PAGE_SIZE: usize = 200;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArxivUpdateResult {
    pub book_id: Uuid,
    pub arxiv_id: ArxivId,
    pub old_version: Option<u8>,
    pub new_version: u8,
    pub new_metadata: ArxivMetadata,
}

pub struct ArxivUpdater {
    pub client: Arc<ArxivClient>,
    pub db: Arc<Database>,
}

impl ArxivUpdater {
    pub fn new(client: Arc<ArxivClient>, db: Arc<Database>) -> Self {
        Self { client, db }
    }

    pub fn from_db(db: Arc<Database>) -> Self {
        Self {
            client: Arc::new(ArxivClient::new()),
            db,
        }
    }

    pub async fn check_all_updates(&self) -> Result<Vec<ArxivUpdateResult>> {
        let tracked = tracked_arxiv_books(self.db.as_ref())?;
        let mut updates = Vec::new();

        for tracked_book in tracked {
            let maybe_updated = self
                .client
                .check_for_updates(&tracked_book.arxiv_id, tracked_book.old_version)
                .await?;
            let Some(new_metadata) = maybe_updated else {
                continue;
            };

            let new_version = new_metadata.arxiv_id.version.ok_or_else(|| {
                ScienceError::Parse(format!(
                    "arXiv API returned update without version: {}",
                    new_metadata.arxiv_id.id
                ))
            })?;

            updates.push(ArxivUpdateResult {
                book_id: tracked_book.book_id,
                arxiv_id: tracked_book.arxiv_id,
                old_version: tracked_book.old_version,
                new_version,
                new_metadata,
            });
        }

        updates.sort_by_key(|item| item.book_id.as_u128());
        Ok(updates)
    }

    pub fn apply_updates(&self, results: &[ArxivUpdateResult]) -> Result<()> {
        for result in results {
            let mut card = self
                .db
                .get_book(&result.book_id.to_string())
                .map_err(map_db_error)?;

            let metadata = result.new_metadata.clone();
            upsert_arxiv_web_source(&mut card, "arxiv_abs", &metadata.abs_url);
            upsert_arxiv_web_source(&mut card, "arxiv_pdf", &metadata.pdf_url);
            card.merge_metadata(partial_from_arxiv(metadata), MetadataSource::ArxivApi);
            card.touch();

            self.db.upsert_book(&card).map_err(map_db_error)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
struct TrackedArxivBook {
    book_id: Uuid,
    arxiv_id: ArxivId,
    old_version: Option<u8>,
}

fn tracked_arxiv_books(db: &Database) -> Result<Vec<TrackedArxivBook>> {
    let total = db.count_books().map_err(map_db_error)?;
    let mut offset = 0usize;
    let mut tracked = Vec::new();

    while offset < total {
        let summaries = db
            .list_books(SCAN_PAGE_SIZE, offset)
            .map_err(map_db_error)?;
        if summaries.is_empty() {
            break;
        }

        let scanned = summaries.len();
        for summary in summaries {
            let card = db.get_book(&summary.id.to_string()).map_err(map_db_error)?;
            let Some(arxiv_id) = card_arxiv_id(&card) else {
                continue;
            };

            tracked.push(TrackedArxivBook {
                book_id: card.id,
                old_version: arxiv_id.version,
                arxiv_id,
            });
        }

        offset += scanned;
    }

    Ok(tracked)
}

fn card_arxiv_id(card: &BookCard) -> Option<ArxivId> {
    let raw = card
        .identifiers
        .as_ref()
        .and_then(|identifiers| identifiers.arxiv_id.as_deref())?;
    ArxivId::parse(raw).ok()
}

fn partial_from_arxiv(metadata: ArxivMetadata) -> PartialMetadata {
    PartialMetadata {
        title: Some(metadata.title),
        authors: metadata
            .authors
            .into_iter()
            .map(|author| author.name)
            .collect(),
        year: Some(metadata.published.year()),
        abstract_text: Some(metadata.abstract_text),
        doi: metadata.doi,
        arxiv_id: Some(metadata.arxiv_id),
        doc_type: Some(DocumentType::Preprint),
        journal: metadata.journal_ref,
        ..Default::default()
    }
}

fn upsert_arxiv_web_source(card: &mut BookCard, name: &str, url: &str) {
    let name = name.trim();
    let url = url.trim();
    if name.is_empty() || url.is_empty() {
        return;
    }

    if card
        .web
        .sources
        .iter()
        .any(|source| source.name == name && source.url == url)
    {
        return;
    }

    card.web.sources.push(WebSource {
        name: name.to_string(),
        url: url.to_string(),
    });
}

fn map_db_error(err: omniscope_core::error::OmniscopeError) -> ScienceError {
    ScienceError::Parse(format!("database error: {err}"))
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use mockito::{Matcher, Server};
    use omniscope_core::models::ScientificIdentifiers;

    use super::*;
    use crate::arxiv::types::ArxivAuthor;

    #[tokio::test]
    async fn check_all_updates_returns_only_changed_versions() {
        let mut server = Server::new_async().await;

        let outdated_mock = server
            .mock("GET", "/api/query")
            .match_query(Matcher::UrlEncoded(
                "id_list".to_string(),
                "1706.03762".to_string(),
            ))
            .with_status(200)
            .with_header("content-type", "application/atom+xml")
            .with_body(atom_feed_for("1706.03762v7", "Updated Attention"))
            .expect(1)
            .create_async()
            .await;

        let current_mock = server
            .mock("GET", "/api/query")
            .match_query(Matcher::UrlEncoded(
                "id_list".to_string(),
                "2301.04567".to_string(),
            ))
            .with_status(200)
            .with_header("content-type", "application/atom+xml")
            .with_body(atom_feed_for("2301.04567v2", "Already Current"))
            .expect(1)
            .create_async()
            .await;

        let db = Arc::new(Database::open_in_memory().unwrap());

        let mut outdated = BookCard::new("Old Attention");
        outdated.identifiers = Some(ScientificIdentifiers {
            arxiv_id: Some("1706.03762v1".to_string()),
            ..Default::default()
        });
        db.upsert_book(&outdated).unwrap();

        let mut current = BookCard::new("Current Paper");
        current.identifiers = Some(ScientificIdentifiers {
            arxiv_id: Some("2301.04567v2".to_string()),
            ..Default::default()
        });
        db.upsert_book(&current).unwrap();

        let mut broken = BookCard::new("Broken ID");
        broken.identifiers = Some(ScientificIdentifiers {
            arxiv_id: Some("not-an-arxiv-id".to_string()),
            ..Default::default()
        });
        db.upsert_book(&broken).unwrap();

        let updater = ArxivUpdater::new(
            Arc::new(ArxivClient::new_for_tests(format!(
                "{}/api/query",
                server.url()
            ))),
            Arc::clone(&db),
        );

        let updates = updater.check_all_updates().await.unwrap();

        outdated_mock.assert_async().await;
        current_mock.assert_async().await;

        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].book_id, outdated.id);
        assert_eq!(updates[0].old_version, Some(1));
        assert_eq!(updates[0].new_version, 7);
        assert_eq!(updates[0].arxiv_id.id, "1706.03762");
        assert_eq!(updates[0].new_metadata.title, "Updated Attention");
    }

    #[test]
    fn apply_updates_merges_new_metadata_and_persists_book() {
        let db = Arc::new(Database::open_in_memory().unwrap());

        let mut card = BookCard::new("Old Title");
        card.identifiers = Some(ScientificIdentifiers {
            arxiv_id: Some("1706.03762v1".to_string()),
            ..Default::default()
        });
        db.upsert_book(&card).unwrap();

        let updater = ArxivUpdater::new(
            Arc::new(ArxivClient::new_for_tests(
                "http://127.0.0.1:9/api/query".to_string(),
            )),
            Arc::clone(&db),
        );

        let update = ArxivUpdateResult {
            book_id: card.id,
            arxiv_id: ArxivId::parse("1706.03762v1").unwrap(),
            old_version: Some(1),
            new_version: 7,
            new_metadata: sample_metadata("1706.03762v7", "Updated Title"),
        };

        updater.apply_updates(&[update]).unwrap();

        let stored = db.get_book(&card.id.to_string()).unwrap();
        assert_eq!(stored.metadata.title, "Updated Title");
        assert_eq!(
            stored
                .identifiers
                .as_ref()
                .and_then(|ids| ids.arxiv_id.as_deref()),
            Some("1706.03762v7")
        );
    }

    #[test]
    fn apply_updates_returns_error_for_missing_book() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let updater = ArxivUpdater::new(
            Arc::new(ArxivClient::new_for_tests(
                "http://127.0.0.1:9/api/query".to_string(),
            )),
            Arc::clone(&db),
        );

        let update = ArxivUpdateResult {
            book_id: Uuid::now_v7(),
            arxiv_id: ArxivId::parse("1706.03762v1").unwrap(),
            old_version: Some(1),
            new_version: 7,
            new_metadata: sample_metadata("1706.03762v7", "Updated Title"),
        };

        let err = updater.apply_updates(&[update]).unwrap_err();
        assert!(err.to_string().contains("database error"));
    }

    fn atom_feed_for(id_with_version: &str, title: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom"
      xmlns:arxiv="http://arxiv.org/schemas/atom">
  <entry>
    <id>http://arxiv.org/abs/{id_with_version}</id>
    <updated>2025-02-01T10:00:00Z</updated>
    <published>2024-12-01T09:00:00Z</published>
    <title>{title}</title>
    <summary>Update metadata payload.</summary>
    <author><name>Example Author</name></author>
    <link rel="related" type="application/pdf" href="http://arxiv.org/pdf/{id_with_version}" />
    <arxiv:primary_category term="cs.CL" />
    <category term="cs.CL" />
  </entry>
</feed>"#
        )
    }

    fn sample_metadata(id_with_version: &str, title: &str) -> ArxivMetadata {
        let arxiv_id = ArxivId::parse(id_with_version).unwrap();
        let pdf_url = format!("https://arxiv.org/pdf/{id_with_version}");

        ArxivMetadata {
            arxiv_id: arxiv_id.clone(),
            doi: None,
            title: title.to_string(),
            authors: vec![ArxivAuthor {
                name: "Example Author".to_string(),
                affiliation: None,
            }],
            abstract_text: "Updated abstract".to_string(),
            published: parse_utc("2024-12-01T09:00:00Z"),
            updated: parse_utc("2025-02-01T10:00:00Z"),
            categories: vec!["cs.CL".to_string()],
            primary_category: "cs.CL".to_string(),
            comment: None,
            journal_ref: Some("Test Journal".to_string()),
            pdf_url,
            abs_url: arxiv_id.abs_url,
        }
    }

    fn parse_utc(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .unwrap()
            .with_timezone(&Utc)
    }
}
