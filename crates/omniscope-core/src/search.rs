use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

use crate::models::BookSummaryView;

/// A scored search result.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub book: BookSummaryView,
    pub score: u32,
    /// Indices of matched characters in the title (for highlighting).
    pub match_indices: Vec<u32>,
}

/// Fuzzy searcher using nucleo-matcher.
pub struct FuzzySearcher {
    matcher: Matcher,
}

impl FuzzySearcher {
    pub fn new() -> Self {
        Self {
            matcher: Matcher::new(Config::DEFAULT.match_paths()),
        }
    }

    /// Search books by query, returning scored results sorted by relevance.
    pub fn search(&mut self, query: &str, books: &[BookSummaryView]) -> Vec<SearchResult> {
        if query.is_empty() {
            return books
                .iter()
                .cloned()
                .map(|book| SearchResult {
                    book,
                    score: 0,
                    match_indices: vec![],
                })
                .collect();
        }

        let pattern = Pattern::new(
            query,
            CaseMatching::Ignore,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );
        let mut buf = Vec::new();
        let mut results: Vec<SearchResult> = Vec::new();

        for book in books {
            // Build searchable text: title + authors + tags
            let searchable = format!(
                "{} {} {}",
                book.title,
                book.authors.join(" "),
                book.tags.join(" ")
            );

            let haystack = Utf32Str::new(&searchable, &mut buf);
            let mut indices = Vec::new();

            if let Some(score) = pattern.indices(haystack, &mut self.matcher, &mut indices) {
                results.push(SearchResult {
                    book: book.clone(),
                    score,
                    match_indices: indices,
                });
            }
        }

        results.sort_by(|a, b| b.score.cmp(&a.score));
        results
    }
}

impl Default for FuzzySearcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{BookCard, ReadStatus};

    fn make_summary(title: &str, authors: &[&str], tags: &[&str]) -> BookSummaryView {
        let mut card = BookCard::new(title);
        card.metadata.authors = authors.iter().map(|s| s.to_string()).collect();
        card.organization.tags = tags.iter().map(|s| s.to_string()).collect();
        BookSummaryView::from(&card)
    }

    #[test]
    fn test_fuzzy_search_basic() {
        let books = vec![
            make_summary("The Rust Programming Language", &["Klabnik"], &["rust"]),
            make_summary("Python Cookbook", &["Beazley"], &["python"]),
            make_summary("Programming Rust 2nd Ed", &["Blandy"], &["rust", "systems"]),
        ];

        let mut searcher = FuzzySearcher::new();
        let results = searcher.search("rust", &books);

        assert!(results.len() >= 2);
        // Both Rust books should be found
        let titles: Vec<&str> = results.iter().map(|r| r.book.title.as_str()).collect();
        assert!(titles.contains(&"The Rust Programming Language"));
        assert!(titles.contains(&"Programming Rust 2nd Ed"));
    }

    #[test]
    fn test_fuzzy_search_empty_query() {
        let books = vec![
            make_summary("Book A", &[], &[]),
            make_summary("Book B", &[], &[]),
        ];

        let mut searcher = FuzzySearcher::new();
        let results = searcher.search("", &books);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_fuzzy_search_typo() {
        let books = vec![make_summary(
            "The Rust Programming Language",
            &["Klabnik"],
            &["rust"],
        )];

        let mut searcher = FuzzySearcher::new();
        let results = searcher.search("rut prog", &books);
        assert!(
            !results.is_empty(),
            "fuzzy should match 'rut prog' -> 'Rust Programming'"
        );
    }

    #[test]
    fn test_fuzzy_search_by_author() {
        let books = vec![
            make_summary("Book One", &["Steve Klabnik"], &[]),
            make_summary("Book Two", &["Jane Doe"], &[]),
        ];

        let mut searcher = FuzzySearcher::new();
        let results = searcher.search("klabnik", &books);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].book.title, "Book One");
    }

    #[test]
    fn test_fuzzy_search_by_tag() {
        let books = vec![
            make_summary("Book A", &[], &["systems", "rust"]),
            make_summary("Book B", &[], &["python", "web"]),
        ];

        let mut searcher = FuzzySearcher::new();
        let results = searcher.search("systems", &books);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].book.title, "Book A");
    }
}
