use std::collections::{HashMap, HashSet};

use omniscope_core::{BookCard, Database, ReadStatus};
use uuid::Uuid;

use crate::identifiers::{arxiv::ArxivId, doi::Doi, isbn::Isbn};
use crate::{Result, ScienceError};

pub type BookId = Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DedupStrategy {
    Doi,
    Isbn,
    TitleFuzzy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateGroup {
    pub canonical: BookId,
    pub duplicates: Vec<BookId>,
    pub strategy: DedupStrategy,
}

#[derive(Debug, Clone)]
pub struct DuplicateFinder {
    title_similarity_threshold: f64,
}

impl Default for DuplicateFinder {
    fn default() -> Self {
        Self {
            title_similarity_threshold: 0.91,
        }
    }
}

impl DuplicateFinder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_title_threshold(mut self, threshold: f64) -> Self {
        self.title_similarity_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    pub fn find_by_doi(&self, books: &[BookCard]) -> Vec<DuplicateGroup> {
        let mut doi_buckets: HashMap<String, Vec<usize>> = HashMap::new();
        for (idx, card) in books.iter().enumerate() {
            if let Some(doi) = normalized_doi(card) {
                doi_buckets.entry(doi).or_default().push(idx);
            }
        }

        let mut groups = Vec::new();
        for indexes in doi_buckets.into_values() {
            if indexes.len() < 2 {
                continue;
            }

            let mut by_arxiv_version: HashMap<(String, Option<u8>), Vec<usize>> = HashMap::new();
            let mut generic = Vec::new();
            for idx in indexes {
                if let Some((base_id, version)) = arxiv_base_and_version(&books[idx]) {
                    by_arxiv_version
                        .entry((base_id, version))
                        .or_default()
                        .push(idx);
                } else {
                    generic.push(idx);
                }
            }

            if generic.len() > 1 {
                groups.push(self.build_group(generic, books, DedupStrategy::Doi));
            }
            for group_indexes in by_arxiv_version.into_values() {
                if group_indexes.len() > 1 {
                    groups.push(self.build_group(group_indexes, books, DedupStrategy::Doi));
                }
            }
        }

        sort_groups_deterministically(&mut groups);
        groups
    }

    pub fn find_by_isbn(&self, books: &[BookCard]) -> Vec<DuplicateGroup> {
        let mut isbn_buckets: HashMap<String, Vec<usize>> = HashMap::new();
        let mut has_isbn = vec![false; books.len()];

        for (idx, card) in books.iter().enumerate() {
            let normalized = normalized_isbn13_values(card);
            if normalized.is_empty() {
                continue;
            }
            has_isbn[idx] = true;
            for isbn13 in normalized {
                isbn_buckets.entry(isbn13).or_default().push(idx);
            }
        }

        let mut dsu = DisjointSet::new(books.len());
        for indexes in isbn_buckets.into_values() {
            if let Some((first, rest)) = indexes.split_first() {
                for idx in rest {
                    dsu.union(*first, *idx);
                }
            }
        }

        let mut components: HashMap<usize, Vec<usize>> = HashMap::new();
        for (idx, has_key) in has_isbn.iter().copied().enumerate() {
            if has_key {
                let root = dsu.find(idx);
                components.entry(root).or_default().push(idx);
            }
        }

        let mut groups = Vec::new();
        for indexes in components.into_values() {
            if indexes.len() > 1 {
                groups.push(self.build_group(indexes, books, DedupStrategy::Isbn));
            }
        }

        sort_groups_deterministically(&mut groups);
        groups
    }

    pub fn find_by_title_fuzzy(&self, books: &[BookCard]) -> Vec<DuplicateGroup> {
        let normalized_titles: Vec<String> = books
            .iter()
            .map(|card| normalize_title(&card.metadata.title))
            .collect();

        let mut dsu = DisjointSet::new(books.len());
        for i in 0..normalized_titles.len() {
            for j in (i + 1)..normalized_titles.len() {
                if similar_titles(
                    &normalized_titles[i],
                    &normalized_titles[j],
                    self.title_similarity_threshold,
                ) {
                    dsu.union(i, j);
                }
            }
        }

        let mut components: HashMap<usize, Vec<usize>> = HashMap::new();
        for (idx, normalized) in normalized_titles.iter().enumerate() {
            if normalized.is_empty() {
                continue;
            }
            let root = dsu.find(idx);
            components.entry(root).or_default().push(idx);
        }

        let mut groups = Vec::new();
        for indexes in components.into_values() {
            if indexes.len() > 1 {
                groups.push(self.build_group(indexes, books, DedupStrategy::TitleFuzzy));
            }
        }

        sort_groups_deterministically(&mut groups);
        groups
    }

    fn build_group(
        &self,
        indexes: Vec<usize>,
        books: &[BookCard],
        strategy: DedupStrategy,
    ) -> DuplicateGroup {
        let canonical_idx = choose_canonical_index(&indexes, books);
        let canonical = books[canonical_idx].id;

        let mut duplicates: Vec<BookId> = indexes
            .into_iter()
            .filter(|idx| *idx != canonical_idx)
            .map(|idx| books[idx].id)
            .collect();
        duplicates.sort_by_key(Uuid::as_u128);

        DuplicateGroup {
            canonical,
            duplicates,
            strategy,
        }
    }
}

pub fn merge_duplicates(canonical: &BookId, to_merge: &[BookId], db: &Database) -> Result<()> {
    let mut merge_ids = Vec::with_capacity(to_merge.len() + 1);
    merge_ids.push(*canonical);
    for id in to_merge {
        if !merge_ids.contains(id) {
            merge_ids.push(*id);
        }
    }

    if merge_ids.len() < 2 {
        return Ok(());
    }

    let mut cards = Vec::with_capacity(merge_ids.len());
    for id in &merge_ids {
        let card = db
            .get_book(&id.to_string())
            .map_err(|e| ScienceError::Parse(format!("database error: {e}")))?;
        cards.push(card);
    }

    let best_idx = choose_best_card_index(&cards);
    let best_original_id = cards[best_idx].id;

    let mut merged = cards[best_idx].clone();
    merged.id = *canonical;
    merged.updated_at = chrono::Utc::now();

    for card in &cards {
        if card.id != best_original_id {
            merge_card_data(&mut merged, card);
        }
    }

    db.upsert_book(&merged)
        .map_err(|e| ScienceError::Parse(format!("database error: {e}")))?;

    for id in merge_ids {
        if id != *canonical {
            db.delete_book(&id.to_string())
                .map_err(|e| ScienceError::Parse(format!("database error: {e}")))?;
        }
    }

    Ok(())
}

fn normalized_doi(card: &BookCard) -> Option<String> {
    card.identifiers
        .as_ref()
        .and_then(|ids| ids.doi.as_deref())
        .and_then(|raw| Doi::parse(raw).ok().map(|doi| doi.normalized))
}

fn arxiv_base_and_version(card: &BookCard) -> Option<(String, Option<u8>)> {
    card.identifiers
        .as_ref()
        .and_then(|ids| ids.arxiv_id.as_deref())
        .and_then(|raw| ArxivId::parse(raw).ok())
        .map(|parsed| (parsed.id.to_ascii_lowercase(), parsed.version))
}

fn normalized_isbn13_values(card: &BookCard) -> Vec<String> {
    let mut values = HashSet::new();

    if let Some(ids) = card.identifiers.as_ref() {
        if let Some(isbn13) = ids.isbn13.as_deref()
            && let Ok(parsed) = Isbn::parse(isbn13)
        {
            values.insert(parsed.isbn13);
        }
        if let Some(isbn10) = ids.isbn10.as_deref()
            && let Ok(parsed) = Isbn::parse(isbn10)
        {
            values.insert(parsed.isbn13);
        }
    }

    for raw in &card.metadata.isbn {
        if let Ok(parsed) = Isbn::parse(raw) {
            values.insert(parsed.isbn13);
        }
    }

    let mut normalized: Vec<String> = values.into_iter().collect();
    normalized.sort();
    normalized
}

fn normalize_title(title: &str) -> String {
    let lowercase = title.to_lowercase();
    let cleaned: String = lowercase
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() {
                c
            } else {
                ' '
            }
        })
        .collect();
    cleaned
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn similar_titles(a: &str, b: &str, threshold: f64) -> bool {
    if a.is_empty() || b.is_empty() {
        return false;
    }

    if a == b {
        return true;
    }

    if a.len() < 5 || b.len() < 5 {
        return false;
    }

    strsim::normalized_levenshtein(a, b) >= threshold
}

fn sort_groups_deterministically(groups: &mut [DuplicateGroup]) {
    groups.sort_by_key(|g| g.canonical.as_u128());
}

fn choose_canonical_index(indexes: &[usize], books: &[BookCard]) -> usize {
    let mut best_idx = indexes[0];
    let mut best_score = metadata_completeness_score(&books[best_idx]);

    for idx in indexes.iter().copied().skip(1) {
        let score = metadata_completeness_score(&books[idx]);
        if score > best_score {
            best_score = score;
            best_idx = idx;
        }
    }

    best_idx
}

fn choose_best_card_index(cards: &[BookCard]) -> usize {
    let mut best_idx = 0usize;
    let mut best_score = metadata_completeness_score(&cards[0]);

    for (idx, card) in cards.iter().enumerate().skip(1) {
        let score = metadata_completeness_score(card);
        if score > best_score {
            best_score = score;
            best_idx = idx;
        }
    }

    best_idx
}

fn metadata_completeness_score(card: &BookCard) -> usize {
    let mut score = 0usize;

    if !card.metadata.title.trim().is_empty() {
        score += 2;
    }
    score += card
        .metadata
        .authors
        .iter()
        .filter(|author| !author.trim().is_empty())
        .count();
    if card.metadata.year.is_some() {
        score += 1;
    }
    if card.metadata.subtitle.is_some() {
        score += 1;
    }
    if !card.metadata.isbn.is_empty() {
        score += 1;
    }
    if card.metadata.publisher.is_some() {
        score += 1;
    }
    if card.metadata.language.is_some() {
        score += 1;
    }
    if card.metadata.pages.is_some() {
        score += 1;
    }
    if card.metadata.edition.is_some() {
        score += 1;
    }
    if card.metadata.series.is_some() {
        score += 1;
    }
    if card.metadata.series_index.is_some() {
        score += 1;
    }

    if let Some(ids) = card.identifiers.as_ref() {
        score += ids.doi.iter().count();
        score += ids.arxiv_id.iter().count();
        score += ids.isbn13.iter().count();
        score += ids.isbn10.iter().count();
        score += ids.pmid.iter().count();
        score += ids.pmcid.iter().count();
        score += ids.openalex_id.iter().count();
        score += ids.semantic_scholar_id.iter().count();
        score += ids.mag_id.iter().count();
        score += ids.dblp_key.iter().count();
    }

    if let Some(publication) = card.publication.as_ref() {
        score += 1;
        score += publication.journal.iter().count();
        score += publication.conference.iter().count();
        score += publication.venue.iter().count();
        score += publication.volume.iter().count();
        score += publication.issue.iter().count();
        score += publication.pages.iter().count();
    }

    if card.open_access.is_some() {
        score += 1;
    }
    if card.file.is_some() {
        score += 2;
    }
    if card.organization.rating.is_some() {
        score += 1;
    }
    if card.organization.read_status != ReadStatus::Unread {
        score += 1;
    }
    if !card.organization.tags.is_empty() {
        score += 1;
    }
    if !card.organization.libraries.is_empty() {
        score += 1;
    }
    if !card.organization.folders.is_empty() {
        score += 1;
    }
    if card.ai.summary.is_some() {
        score += 1;
    }
    if card.ai.tldr.is_some() {
        score += 1;
    }
    if !card.ai.key_topics.is_empty() {
        score += 1;
    }
    if card.web.cover_url.is_some() {
        score += 1;
    }
    if !card.web.sources.is_empty() {
        score += 1;
    }
    if card.citation_graph.citation_count > 0 {
        score += 1;
    }
    if card.citation_graph.reference_count > 0 {
        score += 1;
    }
    if !card.notes.is_empty() {
        score += 1;
    }
    if !card.metadata_sources.is_empty() {
        score += 1;
    }

    score
}

fn merge_card_data(target: &mut BookCard, incoming: &BookCard) {
    if target.metadata.title.trim().is_empty() && !incoming.metadata.title.trim().is_empty() {
        target.metadata.title = incoming.metadata.title.clone();
    }
    merge_option_string(&mut target.metadata.subtitle, &incoming.metadata.subtitle);
    append_unique(&mut target.metadata.authors, &incoming.metadata.authors);
    if target.metadata.year.is_none() {
        target.metadata.year = incoming.metadata.year;
    }
    append_unique(&mut target.metadata.isbn, &incoming.metadata.isbn);
    merge_option_string(&mut target.metadata.publisher, &incoming.metadata.publisher);
    merge_option_string(&mut target.metadata.language, &incoming.metadata.language);
    if target.metadata.pages.is_none() {
        target.metadata.pages = incoming.metadata.pages;
    }
    if target.metadata.edition.is_none() {
        target.metadata.edition = incoming.metadata.edition;
    }
    merge_option_string(&mut target.metadata.series, &incoming.metadata.series);
    if target.metadata.series_index.is_none() {
        target.metadata.series_index = incoming.metadata.series_index;
    }

    match (&mut target.identifiers, &incoming.identifiers) {
        (None, Some(ids)) => target.identifiers = Some(ids.clone()),
        (Some(target_ids), Some(incoming_ids)) => {
            merge_option_string(&mut target_ids.doi, &incoming_ids.doi);
            merge_option_string(&mut target_ids.arxiv_id, &incoming_ids.arxiv_id);
            merge_option_string(&mut target_ids.isbn13, &incoming_ids.isbn13);
            merge_option_string(&mut target_ids.isbn10, &incoming_ids.isbn10);
            merge_option_string(&mut target_ids.pmid, &incoming_ids.pmid);
            merge_option_string(&mut target_ids.pmcid, &incoming_ids.pmcid);
            merge_option_string(&mut target_ids.openalex_id, &incoming_ids.openalex_id);
            merge_option_string(
                &mut target_ids.semantic_scholar_id,
                &incoming_ids.semantic_scholar_id,
            );
            merge_option_string(&mut target_ids.mag_id, &incoming_ids.mag_id);
            merge_option_string(&mut target_ids.dblp_key, &incoming_ids.dblp_key);
        }
        _ => {}
    }

    match (&mut target.publication, &incoming.publication) {
        (None, Some(publication)) => target.publication = Some(publication.clone()),
        (Some(target_publication), Some(incoming_publication)) => {
            merge_option_string(
                &mut target_publication.journal,
                &incoming_publication.journal,
            );
            merge_option_string(
                &mut target_publication.conference,
                &incoming_publication.conference,
            );
            merge_option_string(&mut target_publication.venue, &incoming_publication.venue);
            merge_option_string(&mut target_publication.volume, &incoming_publication.volume);
            merge_option_string(&mut target_publication.issue, &incoming_publication.issue);
            merge_option_string(&mut target_publication.pages, &incoming_publication.pages);
        }
        _ => {}
    }

    target.citation_graph.citation_count = target
        .citation_graph
        .citation_count
        .max(incoming.citation_graph.citation_count);
    target.citation_graph.reference_count = target
        .citation_graph
        .reference_count
        .max(incoming.citation_graph.reference_count);
    target.citation_graph.influential_citation_count = target
        .citation_graph
        .influential_citation_count
        .max(incoming.citation_graph.influential_citation_count);
    if target.citation_graph.last_updated.is_none() {
        target.citation_graph.last_updated = incoming.citation_graph.last_updated;
    }
    append_unique(
        &mut target.citation_graph.references,
        &incoming.citation_graph.references,
    );
    append_unique(
        &mut target.citation_graph.cited_by_sample,
        &incoming.citation_graph.cited_by_sample,
    );
    append_unique(
        &mut target.citation_graph.references_ids,
        &incoming.citation_graph.references_ids,
    );
    append_unique(
        &mut target.citation_graph.cited_by_ids,
        &incoming.citation_graph.cited_by_ids,
    );

    match (&mut target.open_access, &incoming.open_access) {
        (None, Some(oa)) => target.open_access = Some(oa.clone()),
        (Some(target_oa), Some(incoming_oa)) => {
            target_oa.is_open |= incoming_oa.is_open;
            merge_option_string(&mut target_oa.status, &incoming_oa.status);
            merge_option_string(&mut target_oa.license, &incoming_oa.license);
            merge_option_string(&mut target_oa.oa_url, &incoming_oa.oa_url);
            append_unique(&mut target_oa.pdf_urls, &incoming_oa.pdf_urls);
        }
        _ => {}
    }

    if target.file.is_none() && incoming.file.is_some() {
        target.file = incoming.file.clone();
    }

    append_unique(
        &mut target.organization.libraries,
        &incoming.organization.libraries,
    );
    append_unique(
        &mut target.organization.folders,
        &incoming.organization.folders,
    );
    append_unique(&mut target.organization.tags, &incoming.organization.tags);
    target.organization.rating = match (target.organization.rating, incoming.organization.rating) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (None, right) => right,
        (left, None) => left,
    };
    if read_status_rank(incoming.organization.read_status)
        > read_status_rank(target.organization.read_status)
    {
        target.organization.read_status = incoming.organization.read_status;
    }
    if priority_rank(incoming.organization.priority) > priority_rank(target.organization.priority) {
        target.organization.priority = incoming.organization.priority;
    }
    for (key, value) in &incoming.organization.custom_fields {
        target
            .organization
            .custom_fields
            .entry(key.clone())
            .or_insert_with(|| value.clone());
    }

    merge_option_string(&mut target.ai.summary, &incoming.ai.summary);
    merge_option_string(&mut target.ai.tldr, &incoming.ai.tldr);
    append_unique_toc_entries(
        &mut target.ai.table_of_contents,
        &incoming.ai.table_of_contents,
    );
    append_unique(&mut target.ai.key_topics, &incoming.ai.key_topics);
    merge_option_string(&mut target.ai.difficulty, &incoming.ai.difficulty);
    merge_option_string(&mut target.ai.ai_notes, &incoming.ai.ai_notes);
    if target.ai.indexed_at.is_none() {
        target.ai.indexed_at = incoming.ai.indexed_at;
    }
    if target.ai.index_version.is_none() {
        target.ai.index_version = incoming.ai.index_version;
    }
    merge_option_string(&mut target.ai.embedding_model, &incoming.ai.embedding_model);
    target.ai.embedding_stored |= incoming.ai.embedding_stored;

    merge_option_string(&mut target.web.openlibrary_id, &incoming.web.openlibrary_id);
    merge_option_string(&mut target.web.goodreads_id, &incoming.web.goodreads_id);
    merge_option_string(&mut target.web.cover_url, &incoming.web.cover_url);
    append_unique_web_sources(&mut target.web.sources, &incoming.web.sources);

    append_unique_notes(&mut target.notes, &incoming.notes);

    for (field, source) in &incoming.metadata_sources {
        target
            .metadata_sources
            .entry(field.clone())
            .or_insert_with(|| source.clone());
    }

    target.version = target.version.max(incoming.version);
    target.created_at = target.created_at.min(incoming.created_at);
    target.updated_at = target.updated_at.max(incoming.updated_at);
}

fn merge_option_string(target: &mut Option<String>, incoming: &Option<String>) {
    if let Some(incoming_value) = incoming
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        match target.as_ref().map(|s| s.trim()) {
            None => {
                *target = Some(incoming_value.to_string());
            }
            Some(current) if incoming_value.len() > current.len() => {
                *target = Some(incoming_value.to_string());
            }
            _ => {}
        }
    }
}

fn append_unique<T>(target: &mut Vec<T>, incoming: &[T])
where
    T: Clone + PartialEq,
{
    for item in incoming {
        if !target.contains(item) {
            target.push(item.clone());
        }
    }
}

fn append_unique_toc_entries(
    target: &mut Vec<omniscope_core::TocEntry>,
    incoming: &[omniscope_core::TocEntry],
) {
    let mut seen: HashSet<(u32, String, Option<u32>)> = target
        .iter()
        .map(|entry| (entry.chapter, entry.title.clone(), entry.page))
        .collect();

    for entry in incoming {
        let key = (entry.chapter, entry.title.clone(), entry.page);
        if seen.insert(key) {
            target.push(entry.clone());
        }
    }
}

fn append_unique_web_sources(
    target: &mut Vec<omniscope_core::WebSource>,
    incoming: &[omniscope_core::WebSource],
) {
    let mut seen: HashSet<(String, String)> = target
        .iter()
        .map(|source| (source.name.clone(), source.url.clone()))
        .collect();

    for source in incoming {
        let key = (source.name.clone(), source.url.clone());
        if seen.insert(key) {
            target.push(source.clone());
        }
    }
}

fn append_unique_notes(
    target: &mut Vec<omniscope_core::BookNote>,
    incoming: &[omniscope_core::BookNote],
) {
    let mut seen: HashSet<Uuid> = target.iter().map(|note| note.id).collect();

    for note in incoming {
        if seen.insert(note.id) {
            target.push(note.clone());
        }
    }
}

fn read_status_rank(status: ReadStatus) -> u8 {
    match status {
        ReadStatus::Unread => 0,
        ReadStatus::Reading => 1,
        ReadStatus::Dnf => 2,
        ReadStatus::Read => 3,
    }
}

fn priority_rank(priority: omniscope_core::Priority) -> u8 {
    match priority {
        omniscope_core::Priority::None => 0,
        omniscope_core::Priority::Low => 1,
        omniscope_core::Priority::Medium => 2,
        omniscope_core::Priority::High => 3,
    }
}

#[derive(Debug, Clone)]
struct DisjointSet {
    parent: Vec<usize>,
    rank: Vec<u8>,
}

impl DisjointSet {
    fn new(size: usize) -> Self {
        Self {
            parent: (0..size).collect(),
            rank: vec![0; size],
        }
    }

    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            let root = self.find(self.parent[x]);
            self.parent[x] = root;
        }
        self.parent[x]
    }

    fn union(&mut self, left: usize, right: usize) {
        let left_root = self.find(left);
        let right_root = self.find(right);

        if left_root == right_root {
            return;
        }

        let left_rank = self.rank[left_root];
        let right_rank = self.rank[right_root];

        if left_rank < right_rank {
            self.parent[left_root] = right_root;
        } else if left_rank > right_rank {
            self.parent[right_root] = left_root;
        } else {
            self.parent[right_root] = left_root;
            self.rank[left_root] += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use omniscope_core::ScientificIdentifiers;

    fn build_card(id: u128, title: &str) -> BookCard {
        let mut card = BookCard::new(title);
        card.id = Uuid::from_u128(id);
        card
    }

    #[test]
    fn find_by_doi_detects_three_duplicate_groups_in_ten_books() {
        let mut books = Vec::new();

        let mut b1 = build_card(1, "Alpha A");
        b1.identifiers = Some(ScientificIdentifiers {
            doi: Some("10.1000/ALPHA".to_string()),
            ..Default::default()
        });
        b1.metadata.authors = vec!["Alice".to_string()];
        books.push(b1);

        let mut b2 = build_card(2, "Alpha B");
        b2.identifiers = Some(ScientificIdentifiers {
            doi: Some("https://doi.org/10.1000/alpha".to_string()),
            ..Default::default()
        });
        books.push(b2);

        let mut b3 = build_card(3, "Beta A");
        b3.identifiers = Some(ScientificIdentifiers {
            doi: Some("10.1000/beta".to_string()),
            ..Default::default()
        });
        books.push(b3);

        let mut b4 = build_card(4, "Beta B");
        b4.identifiers = Some(ScientificIdentifiers {
            doi: Some("doi:10.1000/BETA".to_string()),
            ..Default::default()
        });
        b4.metadata.year = Some(2024);
        books.push(b4);

        let mut b5 = build_card(5, "Gamma A");
        b5.identifiers = Some(ScientificIdentifiers {
            doi: Some("DOI: 10.1000/gamma".to_string()),
            ..Default::default()
        });
        books.push(b5);

        let mut b6 = build_card(6, "Gamma B");
        b6.identifiers = Some(ScientificIdentifiers {
            doi: Some("https://doi.org/10.1000/GAMMA".to_string()),
            ..Default::default()
        });
        books.push(b6);

        let mut b7 = build_card(7, "Unique DOI");
        b7.identifiers = Some(ScientificIdentifiers {
            doi: Some("10.1000/unique".to_string()),
            ..Default::default()
        });
        books.push(b7);

        let mut b8 = build_card(8, "arXiv v1");
        b8.identifiers = Some(ScientificIdentifiers {
            doi: Some("10.48550/arXiv.1706.03762".to_string()),
            arxiv_id: Some("1706.03762v1".to_string()),
            ..Default::default()
        });
        books.push(b8);

        let mut b9 = build_card(9, "arXiv v5");
        b9.identifiers = Some(ScientificIdentifiers {
            doi: Some("10.48550/arXiv.1706.03762".to_string()),
            arxiv_id: Some("1706.03762v5".to_string()),
            ..Default::default()
        });
        books.push(b9);

        books.push(build_card(10, "No DOI"));

        let finder = DuplicateFinder::new();
        let groups = finder.find_by_doi(&books);

        assert_eq!(groups.len(), 3);
        for group in groups {
            let mut ids = vec![group.canonical];
            ids.extend(group.duplicates);
            assert!(!ids.contains(&Uuid::from_u128(8)) || !ids.contains(&Uuid::from_u128(9)));
        }
    }

    #[test]
    fn find_by_isbn_normalizes_isbn10_and_isbn13() {
        let mut a = build_card(11, "Rust by Example");
        a.identifiers = Some(ScientificIdentifiers {
            isbn10: Some("0131103628".to_string()),
            isbn13: Some("9780131103627".to_string()),
            ..Default::default()
        });

        let mut b = build_card(12, "Rust by Example");
        b.identifiers = Some(ScientificIdentifiers {
            isbn13: Some("978-0-13-110362-7".to_string()),
            ..Default::default()
        });

        let mut c = build_card(13, "Different Book");
        c.identifiers = Some(ScientificIdentifiers {
            isbn13: Some("9781492056812".to_string()),
            ..Default::default()
        });

        let finder = DuplicateFinder::new();
        let groups = finder.find_by_isbn(&[a, b, c]);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].duplicates.len(), 1);
    }

    #[test]
    fn find_by_title_fuzzy_uses_normalized_titles() {
        let a = build_card(21, "Neural Networks in Practice");
        let b = build_card(22, "Neural Networks, in Practice!");
        let c = build_card(23, "Completely Different Title");

        let finder = DuplicateFinder::new().with_title_threshold(0.9);
        let groups = finder.find_by_title_fuzzy(&[a, b, c]);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].duplicates.len(), 1);
    }

    #[test]
    fn merge_duplicates_keeps_richer_metadata_and_deletes_rest() {
        let db = Database::open_in_memory().unwrap();

        let mut canonical = build_card(31, "Sparse Card");
        canonical.organization.tags = vec!["ml".to_string()];
        db.upsert_book(&canonical).unwrap();

        let mut rich = build_card(32, "Sparse Card");
        rich.metadata.authors = vec!["A. Researcher".to_string()];
        rich.metadata.year = Some(2021);
        rich.organization.tags = vec!["transformers".to_string()];
        rich.ai.summary = Some("Detailed summary".to_string());
        rich.identifiers = Some(ScientificIdentifiers {
            doi: Some("10.1000/merge-me".to_string()),
            ..Default::default()
        });
        db.upsert_book(&rich).unwrap();

        merge_duplicates(&canonical.id, &[rich.id], &db).unwrap();

        let merged = db.get_book(&canonical.id.to_string()).unwrap();
        assert_eq!(merged.metadata.year, Some(2021));
        assert_eq!(merged.metadata.authors, vec!["A. Researcher".to_string()]);
        assert!(merged.organization.tags.contains(&"ml".to_string()));
        assert!(
            merged
                .organization
                .tags
                .contains(&"transformers".to_string())
        );
        assert_eq!(merged.ai.summary.as_deref(), Some("Detailed summary"));
        assert_eq!(
            merged.identifiers.and_then(|ids| ids.doi),
            Some("10.1000/merge-me".to_string())
        );
        assert!(db.get_book(&rich.id.to_string()).is_err());
    }
}
