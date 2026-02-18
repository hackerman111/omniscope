use std::fs;
use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::models::BookCard;

/// Save a BookCard as a JSON file: `{cards_dir}/{id}.json`.
pub fn save_card(cards_dir: &Path, card: &BookCard) -> Result<PathBuf> {
    fs::create_dir_all(cards_dir)?;
    let path = cards_dir.join(format!("{}.json", card.id));
    let json = serde_json::to_string_pretty(card)?;
    fs::write(&path, json)?;
    Ok(path)
}

/// Load a single BookCard from a JSON file.
pub fn load_card(path: &Path) -> Result<BookCard> {
    let contents = fs::read_to_string(path)?;
    let card: BookCard = serde_json::from_str(&contents)?;
    Ok(card)
}

/// Load a BookCard by ID from the cards directory.
pub fn load_card_by_id(cards_dir: &Path, id: &uuid::Uuid) -> Result<BookCard> {
    let path = cards_dir.join(format!("{id}.json"));
    if !path.exists() {
        return Err(crate::error::OmniscopeError::BookNotFound(id.to_string()));
    }
    load_card(&path)
}

/// Delete a BookCard JSON file by ID.
pub fn delete_card(cards_dir: &Path, id: &uuid::Uuid) -> Result<()> {
    let path = cards_dir.join(format!("{id}.json"));
    if path.exists() {
        fs::remove_file(&path)?;
    }
    Ok(())
}

/// List all BookCards in the cards directory.
pub fn list_cards(cards_dir: &Path) -> Result<Vec<BookCard>> {
    if !cards_dir.exists() {
        return Ok(Vec::new());
    }

    let mut cards = Vec::new();
    for entry in fs::read_dir(cards_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            match load_card(&path) {
                Ok(card) => cards.push(card),
                Err(e) => {
                    eprintln!("Warning: skipping invalid card {}: {e}", path.display());
                }
            }
        }
    }
    Ok(cards)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_save_and_load_card() {
        let dir = TempDir::new().unwrap();
        let cards_dir = dir.path().join("cards");

        let card = BookCard::new("Test Book");
        let id = card.id;

        save_card(&cards_dir, &card).unwrap();

        let loaded = load_card_by_id(&cards_dir, &id).unwrap();
        assert_eq!(loaded.id, id);
        assert_eq!(loaded.metadata.title, "Test Book");
    }

    #[test]
    fn test_list_cards() {
        let dir = TempDir::new().unwrap();
        let cards_dir = dir.path().join("cards");

        let card1 = BookCard::new("Book One");
        let card2 = BookCard::new("Book Two");
        save_card(&cards_dir, &card1).unwrap();
        save_card(&cards_dir, &card2).unwrap();

        let cards = list_cards(&cards_dir).unwrap();
        assert_eq!(cards.len(), 2);
    }

    #[test]
    fn test_delete_card() {
        let dir = TempDir::new().unwrap();
        let cards_dir = dir.path().join("cards");

        let card = BookCard::new("Deletable");
        let id = card.id;
        save_card(&cards_dir, &card).unwrap();

        delete_card(&cards_dir, &id).unwrap();
        assert!(load_card_by_id(&cards_dir, &id).is_err());
    }

    #[test]
    fn test_list_cards_empty_dir() {
        let dir = TempDir::new().unwrap();
        let cards = list_cards(dir.path()).unwrap();
        assert!(cards.is_empty());
    }

    #[test]
    fn test_list_cards_nonexistent_dir() {
        let cards = list_cards(Path::new("/tmp/nonexistent_omniscope_dir")).unwrap();
        assert!(cards.is_empty());
    }
}
