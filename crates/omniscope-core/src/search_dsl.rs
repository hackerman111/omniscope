/// Search DSL parser.
///
/// Supports the following syntax:
/// ```text
/// rust async              → fuzzy match on all fields
/// @author:klabnik        → filter by author
/// #rust                  → filter by tag
/// y:2023                 → exact year
/// y:2020-2023  y:>2020   → year range
/// r:4  r:>=4  r:>3       → rating filter
/// s:unread               → read status filter
/// f:pdf  f:epub          → format filter
/// lib:programming        → library filter
/// has:file  has:summary  → existence filter
/// NOT #python            → negate next token
/// ```
use crate::models::{BookSummaryView, ReadStatus};

/// A parsed search query.
#[derive(Debug, Clone, Default)]
pub struct SearchQuery {
    /// Free-text fuzzy search terms.
    pub fuzzy_terms: Vec<String>,
    /// Filters extracted from DSL tokens.
    pub filters: Vec<SearchFilter>,
}

/// A single filter extracted from DSL.
#[derive(Debug, Clone)]
pub enum SearchFilter {
    Author(String),
    Tag(String),
    NotTag(String),
    Year(YearFilter),
    Rating(CompareOp),
    Status(ReadStatus),
    Format(String),
    Library(String),
    HasFile,
    HasSummary,
    HasTags,
    Not(Box<SearchFilter>),
}

#[derive(Debug, Clone)]
pub enum YearFilter {
    Exact(i32),
    Range(i32, i32),
    GreaterThan(i32),
    LessThan(i32),
}

#[derive(Debug, Clone)]
pub enum CompareOp {
    Eq(u8),
    Gte(u8),
    Gt(u8),
    Lte(u8),
    Lt(u8),
}

impl SearchQuery {
    /// Parse a DSL query string.
    pub fn parse(input: &str) -> Self {
        let mut query = SearchQuery::default();
        let mut negate_next = false;

        let tokens = tokenize(input);

        for token in tokens {
            let token = token.trim();
            if token.is_empty() {
                continue;
            }

            // NOT keyword
            if token.eq_ignore_ascii_case("NOT") {
                negate_next = true;
                continue;
            }

            let filter = parse_token(token);

            match filter {
                Some(f) => {
                    let f = if negate_next {
                        SearchFilter::Not(Box::new(f))
                    } else {
                        f
                    };
                    query.filters.push(f);
                    negate_next = false;
                }
                None => {
                    // It's a fuzzy search term
                    if negate_next {
                        // "NOT someword" — we can't negate fuzzy terms, treat as fuzzy
                        negate_next = false;
                    }
                    query.fuzzy_terms.push(token.to_string());
                }
            }
        }

        query
    }

    /// Check whether a book matches all filters in this query.
    pub fn matches(&self, book: &BookSummaryView) -> bool {
        self.filters.iter().all(|f| filter_matches(f, book))
    }

    /// Is this a pure fuzzy query (no DSL filters)?
    pub fn is_fuzzy_only(&self) -> bool {
        self.filters.is_empty()
    }

    /// Get the fuzzy portion as a single string.
    pub fn fuzzy_text(&self) -> String {
        self.fuzzy_terms.join(" ")
    }
}

fn tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for ch in input.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
            }
            ' ' if !in_quotes => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(ch),
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

fn parse_token(token: &str) -> Option<SearchFilter> {
    // @author:name or @name
    if let Some(rest) = token.strip_prefix("@author:").or_else(|| token.strip_prefix("@")) {
        return Some(SearchFilter::Author(rest.to_string()));
    }

    // #tag
    if let Some(rest) = token.strip_prefix('#') {
        // Handle #tag:name syntax too
        let tag = rest.strip_prefix("tag:").unwrap_or(rest);
        return Some(SearchFilter::Tag(tag.to_string()));
    }

    // y:YEAR or y:FROM-TO or y:>YEAR
    if let Some(rest) = token.strip_prefix("y:") {
        return parse_year_filter(rest).map(SearchFilter::Year);
    }

    // r:RATING or r:>=RATING
    if let Some(rest) = token.strip_prefix("r:") {
        return parse_rating_filter(rest).map(SearchFilter::Rating);
    }

    // s:STATUS
    if let Some(rest) = token.strip_prefix("s:").or_else(|| token.strip_prefix("status:")) {
        let status = match rest {
            "unread" => ReadStatus::Unread,
            "reading" => ReadStatus::Reading,
            "read" => ReadStatus::Read,
            "dnf" => ReadStatus::Dnf,
            _ => return None,
        };
        return Some(SearchFilter::Status(status));
    }

    // f:FORMAT
    if let Some(rest) = token.strip_prefix("f:").or_else(|| token.strip_prefix("format:")) {
        return Some(SearchFilter::Format(rest.to_lowercase()));
    }

    // lib:NAME
    if let Some(rest) = token.strip_prefix("lib:").or_else(|| token.strip_prefix("library:")) {
        return Some(SearchFilter::Library(rest.to_string()));
    }

    // has:file, has:summary, has:tags
    if let Some(rest) = token.strip_prefix("has:") {
        return match rest {
            "file" => Some(SearchFilter::HasFile),
            "summary" => Some(SearchFilter::HasSummary),
            "tags" => Some(SearchFilter::HasTags),
            _ => None,
        };
    }

    None
}

fn parse_year_filter(s: &str) -> Option<YearFilter> {
    // y:>2020
    if let Some(rest) = s.strip_prefix(">=") {
        return rest.parse().ok().map(YearFilter::GreaterThan);
    }
    if let Some(rest) = s.strip_prefix('>') {
        return rest.parse::<i32>().ok().map(|y| YearFilter::GreaterThan(y + 1));
    }
    if let Some(rest) = s.strip_prefix("<=") {
        return rest.parse().ok().map(YearFilter::LessThan);
    }
    if let Some(rest) = s.strip_prefix('<') {
        return rest.parse::<i32>().ok().map(|y| YearFilter::LessThan(y - 1));
    }

    // y:2020-2023 or y:2020..2023
    if s.contains('-') || s.contains("..") {
        let sep = if s.contains("..") { ".." } else { "-" };
        let parts: Vec<&str> = s.splitn(2, sep).collect();
        if parts.len() == 2 {
            if let (Ok(from), Ok(to)) = (parts[0].parse(), parts[1].parse()) {
                return Some(YearFilter::Range(from, to));
            }
        }
    }

    // y:2023
    s.parse().ok().map(YearFilter::Exact)
}

fn parse_rating_filter(s: &str) -> Option<CompareOp> {
    if let Some(rest) = s.strip_prefix(">=") {
        return rest.parse().ok().map(CompareOp::Gte);
    }
    if let Some(rest) = s.strip_prefix('>') {
        return rest.parse().ok().map(CompareOp::Gt);
    }
    if let Some(rest) = s.strip_prefix("<=") {
        return rest.parse().ok().map(CompareOp::Lte);
    }
    if let Some(rest) = s.strip_prefix('<') {
        return rest.parse().ok().map(CompareOp::Lt);
    }
    // Plain number: treat as >=
    s.parse().ok().map(|v| {
        if s.ends_with('+') {
            CompareOp::Gte(v)
        } else {
            CompareOp::Gte(v)
        }
    })
}

fn filter_matches(filter: &SearchFilter, book: &BookSummaryView) -> bool {
    match filter {
        SearchFilter::Author(name) => {
            let name_lower = name.to_lowercase();
            book.authors.iter().any(|a| a.to_lowercase().contains(&name_lower))
        }
        SearchFilter::Tag(tag) => {
            let tag_lower = tag.to_lowercase();
            book.tags.iter().any(|t| t.to_lowercase() == tag_lower)
        }
        SearchFilter::NotTag(tag) => {
            let tag_lower = tag.to_lowercase();
            !book.tags.iter().any(|t| t.to_lowercase() == tag_lower)
        }
        SearchFilter::Year(yf) => {
            if let Some(year) = book.year {
                match yf {
                    YearFilter::Exact(y) => year == *y,
                    YearFilter::Range(from, to) => year >= *from && year <= *to,
                    YearFilter::GreaterThan(y) => year >= *y,
                    YearFilter::LessThan(y) => year <= *y,
                }
            } else {
                false
            }
        }
        SearchFilter::Rating(op) => {
            if let Some(rating) = book.rating {
                match op {
                    CompareOp::Eq(v) => rating == *v,
                    CompareOp::Gte(v) => rating >= *v,
                    CompareOp::Gt(v) => rating > *v,
                    CompareOp::Lte(v) => rating <= *v,
                    CompareOp::Lt(v) => rating < *v,
                }
            } else {
                false
            }
        }
        SearchFilter::Status(status) => book.read_status == *status,
        SearchFilter::Format(fmt) => {
            book.format
                .map(|f| f.to_string().to_lowercase() == *fmt)
                .unwrap_or(false)
        }
        SearchFilter::Library(_lib) => {
            // Library filter is applied at the DB level, always true here
            true
        }
        SearchFilter::HasFile => book.has_file,
        SearchFilter::HasSummary => false, // Not available in summary view
        SearchFilter::HasTags => !book.tags.is_empty(),
        SearchFilter::Not(inner) => !filter_matches(inner, book),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::BookCard;

    fn make_book(title: &str, authors: &[&str], tags: &[&str], year: Option<i32>, rating: Option<u8>) -> BookSummaryView {
        let mut card = BookCard::new(title);
        card.metadata.authors = authors.iter().map(|s| s.to_string()).collect();
        card.organization.tags = tags.iter().map(|s| s.to_string()).collect();
        card.metadata.year = year;
        card.organization.rating = rating;
        BookSummaryView::from(&card)
    }

    #[test]
    fn test_parse_fuzzy_only() {
        let q = SearchQuery::parse("rust async");
        assert!(q.is_fuzzy_only());
        assert_eq!(q.fuzzy_terms, vec!["rust", "async"]);
    }

    #[test]
    fn test_parse_author() {
        let q = SearchQuery::parse("@author:klabnik");
        assert_eq!(q.filters.len(), 1);
        assert!(matches!(&q.filters[0], SearchFilter::Author(a) if a == "klabnik"));
    }

    #[test]
    fn test_parse_tag() {
        let q = SearchQuery::parse("#rust");
        assert_eq!(q.filters.len(), 1);
        assert!(matches!(&q.filters[0], SearchFilter::Tag(t) if t == "rust"));
    }

    #[test]
    fn test_parse_year_exact() {
        let q = SearchQuery::parse("y:2023");
        assert!(matches!(&q.filters[0], SearchFilter::Year(YearFilter::Exact(2023))));
    }

    #[test]
    fn test_parse_year_range() {
        let q = SearchQuery::parse("y:2020-2023");
        assert!(matches!(&q.filters[0], SearchFilter::Year(YearFilter::Range(2020, 2023))));
    }

    #[test]
    fn test_parse_year_gt() {
        let q = SearchQuery::parse("y:>2020");
        assert!(matches!(&q.filters[0], SearchFilter::Year(YearFilter::GreaterThan(_))));
    }

    #[test]
    fn test_parse_rating() {
        let q = SearchQuery::parse("r:>=4");
        assert!(matches!(&q.filters[0], SearchFilter::Rating(CompareOp::Gte(4))));
    }

    #[test]
    fn test_parse_status() {
        let q = SearchQuery::parse("s:unread");
        assert!(matches!(&q.filters[0], SearchFilter::Status(ReadStatus::Unread)));
    }

    #[test]
    fn test_parse_format() {
        let q = SearchQuery::parse("f:pdf");
        assert!(matches!(&q.filters[0], SearchFilter::Format(f) if f == "pdf"));
    }

    #[test]
    fn test_parse_has_file() {
        let q = SearchQuery::parse("has:file");
        assert!(matches!(&q.filters[0], SearchFilter::HasFile));
    }

    #[test]
    fn test_parse_not() {
        let q = SearchQuery::parse("NOT #python");
        assert!(matches!(&q.filters[0], SearchFilter::Not(_)));
    }

    #[test]
    fn test_parse_complex() {
        let q = SearchQuery::parse("rust @author:klabnik #systems y:2020-2023 r:>=4");
        assert_eq!(q.fuzzy_terms, vec!["rust"]);
        assert_eq!(q.filters.len(), 4);
    }

    #[test]
    fn test_filter_matches_author() {
        let book = make_book("Rust Book", &["Steve Klabnik"], &[], None, None);
        let q = SearchQuery::parse("@author:klabnik");
        assert!(q.matches(&book));
    }

    #[test]
    fn test_filter_matches_tag() {
        let book = make_book("Rust Book", &[], &["rust", "systems"], None, None);
        let q = SearchQuery::parse("#rust");
        assert!(q.matches(&book));
    }

    #[test]
    fn test_filter_matches_year_range() {
        let book = make_book("Book", &[], &[], Some(2022), None);
        let q = SearchQuery::parse("y:2020-2023");
        assert!(q.matches(&book));

        let q2 = SearchQuery::parse("y:2024-2025");
        assert!(!q2.matches(&book));
    }

    #[test]
    fn test_filter_matches_rating() {
        let book = make_book("Book", &[], &[], None, Some(4));
        assert!(SearchQuery::parse("r:>=4").matches(&book));
        assert!(!SearchQuery::parse("r:>=5").matches(&book));
    }

    #[test]
    fn test_filter_not_tag() {
        let book = make_book("Book", &[], &["rust"], None, None);
        assert!(!SearchQuery::parse("NOT #rust").matches(&book));
        assert!(SearchQuery::parse("NOT #python").matches(&book));
    }
}
