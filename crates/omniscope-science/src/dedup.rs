use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use omniscope_core::models::book::BookCard;
use crate::error::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DedupStrategy {
    ByDoi,
    ByIsbn,
    ByTitleFuzzy,
}

#[derive(Debug, Clone)]
pub struct DuplicateGroup {
    pub canonical: Uuid,
    pub duplicates: Vec<Uuid>,
    pub strategy: DedupStrategy,
}

pub struct DuplicateFinder;

impl DuplicateFinder {
    /// Group books by identical normalized DOI.
    pub fn find_by_doi(books: &[BookCard]) -> Vec<DuplicateGroup> {
        let mut groups: HashMap<String, Vec<&BookCard>> = HashMap::new();

        for book in books {
            if let Some(ids) = &book.identifiers
                && let Some(doi) = &ids.doi {
                    let normalized = crate::identifiers::doi::Doi::parse(doi)
                        .map(|d| d.normalized)
                        .unwrap_or_else(|_| doi.clone());
                    groups
                        .entry(normalized.to_lowercase())
                        .or_default()
                        .push(book);
                }
        }

        Self::build_groups(groups, DedupStrategy::ByDoi)
    }

    /// Group books by identical ISBN-13.
    pub fn find_by_isbn(books: &[BookCard]) -> Vec<DuplicateGroup> {
        let mut groups: HashMap<String, Vec<&BookCard>> = HashMap::new();

        for book in books {
            if let Some(ids) = &book.identifiers {
                if let Some(isbn13) = &ids.isbn13 {
                    // Try to parse to ensure it's fully normalized, fallback to raw string
                    let normalized = crate::identifiers::isbn::Isbn::parse(isbn13)
                        .map(|i| i.isbn13)
                        .unwrap_or_else(|_| isbn13.clone());
                    groups.entry(normalized).or_default().push(book);
                } else if let Some(isbn10) = &ids.isbn10 {
                    let normalized = crate::identifiers::isbn::Isbn::parse(isbn10)
                        .map(|i| i.isbn13)
                        .unwrap_or_else(|_| isbn10.clone());
                    groups.entry(normalized).or_default().push(book);
                }
            }
        }

        Self::build_groups(groups, DedupStrategy::ByIsbn)
    }

    /// Group books by fuzzy title similarity using Levenshtein distance.
    pub fn find_by_title_fuzzy(books: &[BookCard]) -> Vec<DuplicateGroup> {
        let mut groups: Vec<Vec<&BookCard>> = Vec::new();
        let mut assigned = HashSet::new();

        for (i, book_a) in books.iter().enumerate() {
            if assigned.contains(&book_a.id) {
                continue;
            }

            let title_a = normalize_title(&book_a.metadata.title);
            if title_a.is_empty() {
                continue;
            }

            let mut current_group = vec![book_a];
            assigned.insert(book_a.id);

            for book_b in books.iter().skip(i + 1) {
                if assigned.contains(&book_b.id) {
                    continue;
                }

                let title_b = normalize_title(&book_b.metadata.title);
                if title_b.is_empty() {
                    continue;
                }

                // Fast path: length difference check to avoid expensive Levenshtein calculations
                let len_diff = (title_a.len() as isize - title_b.len() as isize).abs();
                let max_len = title_a.len().max(title_b.len());

                // If lengths differ by more than 10%, they can't have > 0.9 similarity
                if (len_diff as f32) / (max_len as f32) > 0.1 {
                    continue;
                }

                // Calculate Levenshtein similarity
                let distance = strsim::levenshtein(&title_a, &title_b);
                let similarity = 1.0 - (distance as f32 / max_len as f32);

                if similarity > 0.9 {
                    current_group.push(book_b);
                    assigned.insert(book_b.id);
                }
            }

            if current_group.len() > 1 {
                groups.push(current_group);
            }
        }

        groups
            .into_iter()
            .map(|group| {
                let mut ids: Vec<Uuid> = group.into_iter().map(|b| b.id).collect();
                // Select the first as canonical
                let canonical = ids.remove(0);
                DuplicateGroup {
                    canonical,
                    duplicates: ids,
                    strategy: DedupStrategy::ByTitleFuzzy,
                }
            })
            .collect()
    }

    fn build_groups(
        groups: HashMap<String, Vec<&BookCard>>,
        strategy: DedupStrategy,
    ) -> Vec<DuplicateGroup> {
        groups
            .into_values()
            .filter(|group| group.len() > 1)
            .map(|group| {
                let mut ids: Vec<Uuid> = group.into_iter().map(|b| b.id).collect();
                let canonical = ids.remove(0);
                DuplicateGroup {
                    canonical,
                    duplicates: ids,
                    strategy: strategy.clone(),
                }
            })
            .collect()
    }
}

/// Pure function to merge duplicates.
/// Returns a tuple of (merged_canonical_card, list_of_ids_to_delete).
/// The caller is responsible for updating the DB and deleting the merged cards.
pub fn merge_duplicates(canonical: BookCard, to_merge: &[BookCard]) -> Result<(BookCard, Vec<Uuid>)> {
    let mut merged = canonical.clone();
    let mut to_delete = Vec::new();

    for card in to_merge {
        if card.id == merged.id {
            continue; // Skip canonical itself if erroneously passed
        }
        to_delete.push(card.id);

        // Simple merge strategy: collect distinct tags, authors, etc.
        // Tags
        for tag in &card.organization.tags {
            if !merged.organization.tags.contains(tag) {
                merged.organization.tags.push(tag.clone());
            }
        }

        // Authors
        for author in &card.metadata.authors {
            if !merged.metadata.authors.contains(author) {
                merged.metadata.authors.push(author.clone());
            }
        }

        // Year: keep canonical's year, or fill if missing
        if merged.metadata.year.is_none() && card.metadata.year.is_some() {
            merged.metadata.year = card.metadata.year;
        }

        // We assume actual detailed merging logic will be expanded alongside Step 12 (Enrichment pipeline)
    }

    Ok((merged, to_delete))
}

fn normalize_title(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use omniscope_core::models::book::BookCard;
    use omniscope_core::models::identifiers::ScientificIdentifiers;

    fn create_test_card(title: &str, doi_str: Option<&str>, isbn_str: Option<&str>) -> BookCard {
        let mut card = BookCard::new(title);
        
        if doi_str.is_some() || isbn_str.is_some() {
            let mut ids = ScientificIdentifiers::default();
            
            if let Some(ds) = doi_str {
                ids.doi = Some(ds.to_string());
            }
            if let Some(is) = isbn_str {
                ids.isbn13 = Some(is.to_string());
            }
            
            card.identifiers = Some(ids);
        }
        
        card
    }

    #[test]
    fn test_find_by_doi() {
        let books = vec![
            create_test_card("Title 1", Some("10.1234/a"), None),
            create_test_card("Title 2", Some("10.1234/b"), None),
            create_test_card("Title 1 dup", Some("10.1234/A"), None), // Identical normalized DOI
            create_test_card("Title 3", Some("10.5678/c"), None),
            create_test_card("Title 1 dup 2", Some("10.1234/a"), None),
        ];

        let groups = DuplicateFinder::find_by_doi(&books);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].strategy, DedupStrategy::ByDoi);
        assert_eq!(groups[0].duplicates.len(), 2); // 1 canonical + 2 dups = 3 items in group
    }

    #[test]
    fn test_find_by_isbn() {
        let books = vec![
            create_test_card("Title 1", None, Some("978-0-306-40615-7")), // ISBN-13
            create_test_card("Title 2", None, Some("0306406152")),        // Equivalent ISBN-10
            create_test_card("Title 3", None, Some("978-1-111-11111-1")),
        ];

        let groups = DuplicateFinder::find_by_isbn(&books);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].strategy, DedupStrategy::ByIsbn);
        assert_eq!(groups[0].duplicates.len(), 1);
    }

    #[test]
    fn test_find_by_title_fuzzy() {
        let books = vec![
            create_test_card("Attention Is All You Need", None, None),
            create_test_card("Attention is all you need!", None, None),
            create_test_card("Attention  is all you   need", None, None),
            create_test_card("BERT: Pre-training of Deep Bidirectional Transformers", None, None),
            create_test_card("bert pretraining of deep bidirectional transformers", None, None),
            create_test_card("Some completely unrelated long title", None, None),
        ];

        let groups = DuplicateFinder::find_by_title_fuzzy(&books);
        assert_eq!(groups.len(), 2);
        
        // One group for Attention... (3 items) and one for BERT... (2 items)
        let group_sizes: Vec<usize> = groups.iter().map(|g| g.duplicates.len()).collect();
        assert!(group_sizes.contains(&2)); // 1 canonical + 2 dups = 3
        assert!(group_sizes.contains(&1)); // 1 canonical + 1 dup = 2
    }
}
