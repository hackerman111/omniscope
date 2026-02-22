use omniscope_core::models::book::BookCard;

pub struct DuplicateFinder;

impl DuplicateFinder {
    /// Returns a similarity score between 0.0 and 1.0.
    /// 1.0 means certain duplicate.
    pub fn similarity(a: &BookCard, b: &BookCard) -> f32 {
        // 1. Strict identifier matches
        if let (Some(ids_a), Some(ids_b)) = (&a.identifiers, &b.identifiers) {
            if let (Some(doi_a), Some(doi_b)) = (&ids_a.doi, &ids_b.doi) {
                if doi_a.to_lowercase() == doi_b.to_lowercase() {
                    return 1.0;
                }
            }
            if let (Some(arxiv_a), Some(arxiv_b)) = (&ids_a.arxiv_id, &ids_b.arxiv_id) {
                if arxiv_a.to_lowercase() == arxiv_b.to_lowercase() {
                    return 1.0;
                }
            }
        }

        // ISBN match
        for isbn_a in &a.metadata.isbn {
            for isbn_b in &b.metadata.isbn {
                if isbn_a == isbn_b {
                    return 1.0;
                }
            }
        }

        // 2. Fuzzy matching
        let mut score = 0.0;

        // Title similarity
        let title_sim = title_similarity(&a.metadata.title, &b.metadata.title);
        if title_sim > 0.9 {
            score += 0.7;
        } else if title_sim > 0.7 {
            score += 0.4;
        }

        // Year match
        if let (Some(y_a), Some(y_b)) = (a.metadata.year, b.metadata.year) {
            if y_a == y_b {
                score += 0.2;
            } else if (y_a - y_b).abs() <= 1 {
                score += 0.1;
            } else {
                score -= 0.2;
            }
        }

        // Author overlap
        let author_sim = author_similarity(&a.metadata.authors, &b.metadata.authors);
        score += author_sim * 0.3;

        score.clamp(0.0, 1.0)
    }

    pub fn is_duplicate(a: &BookCard, b: &BookCard) -> bool {
        Self::similarity(a, b) > 0.85
    }
}

fn title_similarity(s1: &str, s2: &str) -> f32 {
    let s1 = normalize_title(s1);
    let s2 = normalize_title(s2);
    
    if s1.is_empty() || s2.is_empty() {
        return 0.0;
    }

    if s1 == s2 {
        return 1.0;
    }

    // Simple word overlap (Jaccard)
    let words1: std::collections::HashSet<_> = s1.split_whitespace().collect();
    let words2: std::collections::HashSet<_> = s2.split_whitespace().collect();
    
    let intersection = words1.intersection(&words2).count() as f32;
    let union = words1.union(&words2).count() as f32;
    
    intersection / union
}

fn normalize_title(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect()
}

fn author_similarity(a1: &[String], a2: &[String]) -> f32 {
    if a1.is_empty() || a2.is_empty() {
        return 0.0;
    }

    let mut matches = 0;
    for auth1 in a1 {
        let auth1_norm = normalize_author(auth1);
        for auth2 in a2 {
            let auth2_norm = normalize_author(auth2);
            if auth1_norm.contains(&auth2_norm) || auth2_norm.contains(&auth1_norm) {
                matches += 1;
                break;
            }
        }
    }

    matches as f32 / (a1.len().max(a2.len()) as f32)
}

fn normalize_author(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use omniscope_core::models::book::BookCard;

    #[test]
    fn test_similarity_doi() {
        let mut card1 = BookCard::new("Title A");
        let mut ids1 = omniscope_core::models::ScientificIdentifiers::default();
        ids1.doi = Some("10.1234/5678".to_string());
        card1.identifiers = Some(ids1);

        let mut card2 = BookCard::new("Title B");
        let mut ids2 = omniscope_core::models::ScientificIdentifiers::default();
        ids2.doi = Some("10.1234/5678".to_string());
        card2.identifiers = Some(ids2);

        assert_eq!(DuplicateFinder::similarity(&card1, &card2), 1.0);
    }

    #[test]
    fn test_similarity_fuzzy() {
        let mut card1 = BookCard::new("Attention Is All You Need");
        card1.metadata.authors = vec!["Vaswani".to_string()];
        card1.metadata.year = Some(2017);

        let mut card2 = BookCard::new("Attention is all you need!");
        card2.metadata.authors = vec!["Vaswani, A.".to_string()];
        card2.metadata.year = Some(2017);

        assert!(DuplicateFinder::similarity(&card1, &card2) > 0.85);
    }
}
