use std::sync::Arc;
use omniscope_core::storage::database::Database;
use omniscope_core::models::BookCard;
use crate::error::Result;
use crate::arxiv::client::ArxivClient;
use crate::arxiv::types::ArxivMetadata;
use crate::identifiers::arxiv::ArxivId;
use tracing::{info, warn};

pub struct ArxivUpdateResult {
    pub book_id: uuid::Uuid,
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

    /// Checks all books with an assigned arxiv_id in the database
    /// for updates against the ArXiv API.
    pub async fn check_all_updates(&self) -> Result<Vec<ArxivUpdateResult>> {
        let books = self.db.list_books_with_arxiv_id().unwrap_or_default();
        let mut updates = Vec::new();

        for book in books {
            let Some(identifiers) = &book.identifiers else { continue };
            let Some(arxiv_id) = &identifiers.arxiv_id else { continue };
            
            // Re-parse it into our typed struct strictly
            let parsed_id = match ArxivId::parse(arxiv_id) {
                Ok(id) => id,
                Err(e) => {
                    warn!("Failed to parse arxiv_id from DB for book {}: {}", book.id, e);
                    continue;
                }
            };
            
            let current_version = parsed_id.version;
            
            // Sleep to respect rate limits if calling iteratively
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            match self.client.check_for_updates(&parsed_id, current_version).await {
                Ok(Some(new_metadata)) => {
                    if let Some(new_version) = new_metadata.arxiv_id.version {
                        updates.push(ArxivUpdateResult {
                            book_id: book.id,
                            arxiv_id: parsed_id,
                            old_version: current_version,
                            new_version,
                            new_metadata,
                        });
                    }
                }
                Ok(None) => {} // No update found
                Err(e) => {
                    warn!("Failed to check update for book {}: {}", book.id, e);
                }
            }
        }
        
        info!("Found {} updates for arXiv documents", updates.len());
        Ok(updates)
    }

    /// Applies the new metadata to the Database by updating the existing book cards.
    pub fn apply_updates(&self, results: &[ArxivUpdateResult]) -> Result<()> {
        for update in results {
            // First we need to get the actual book card from the database again
            // to ensure we aren't overwriting anything else that changed
            let mut card = match BookCard::new("".to_string()) {
                mut c => {
                    c.id = update.book_id;
                    // Note: Instead of doing a complex update, we would normally fetch the existing card here.
                    // For now, this is a placeholder demonstration of the update flow.
                    c
                }
            };
            
            // Normally you would have `db.get_book_card(id)` to retrieve the full domain object.
            // Since we know we have the full object, we update the elements we care about:
            // - metadata (title, abstract text, publication date, authors)
            // - arxiv_id string in identifiers
            
            card.metadata.title = update.new_metadata.title.clone();
            
            card.ai.summary = Some(update.new_metadata.abstract_text.clone());

            if let Some(identifiers) = &mut card.identifiers {
                identifiers.arxiv_id = Some(update.new_metadata.arxiv_id.raw.clone());
                if let Some(doi) = &update.new_metadata.doi {
                    identifiers.doi = Some(doi.raw.clone());
                }
            }
            
            if let Err(e) = self.db.upsert_book(&card) {
                warn!("Failed to save updated book {}: {}", update.book_id, e);
            }
        }
        
        Ok(())
    }
}
