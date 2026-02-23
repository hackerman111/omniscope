use super::{App, Register, RegisterContent};
use crate::panels::citation_graph::{CitationEdge, CitationGraphPanel, GraphMode};
use crate::panels::find_download::{FindDownloadPanel, FindResult, SearchIdentifierKind};
use crate::panels::references::ReferencesPanel;
use crate::popup::Popup;
use omniscope_core::models::{BookCard, DocumentType};
use omniscope_core::storage::json_cards;
use omniscope_science::formats::bibtex::{generate_bibtex, BibTeXOptions};
use omniscope_science::formats::csl::CslProcessor;
use omniscope_science::identifiers::arxiv::ArxivId;
use omniscope_science::identifiers::doi::Doi;
use omniscope_science::identifiers::extract::{
    extract_arxiv_ids_from_text, extract_dois_from_text,
};
use omniscope_science::references::{ExtractedReference, ResolutionMethod};
use uuid::Uuid;

impl App {
    pub fn has_science_context(&self) -> bool {
        let Some(card) = self.selected_card() else {
            return false;
        };

        let has_identifiers = card
            .identifiers
            .as_ref()
            .map(|ids| {
                ids.doi.is_some()
                    || ids.arxiv_id.is_some()
                    || ids.semantic_scholar_id.is_some()
                    || ids.openalex_id.is_some()
            })
            .unwrap_or(false);

        let has_graph = card.citation_graph.citation_count > 0
            || card.citation_graph.reference_count > 0
            || !card.citation_graph.references.is_empty()
            || !card.citation_graph.references_ids.is_empty();

        let is_scientific_doc = matches!(
            card.publication
                .as_ref()
                .map(|publication| publication.doc_type),
            Some(DocumentType::Article | DocumentType::ConferencePaper | DocumentType::Preprint)
        );

        has_identifiers || has_graph || is_scientific_doc
    }

    pub fn open_science_references_panel(&mut self) {
        let Some(card) = self.selected_card() else {
            self.status_message = "No selected book".to_string();
            return;
        };

        let references = build_extracted_references(&card);
        let title = card.metadata.title.clone();
        self.popup = Some(Popup::ScienceReferences {
            panel: ReferencesPanel::new(references),
            book_title: title,
        });
    }

    pub fn open_science_citation_graph_panel(&mut self, mode: GraphMode) {
        let Some(card) = self.selected_card() else {
            self.status_message = "No selected book".to_string();
            return;
        };

        let references = build_citation_edges(
            &card.citation_graph.references,
            &card.citation_graph.references_ids,
        );
        let cited_by = build_citation_edges(
            &card.citation_graph.cited_by_sample,
            &card.citation_graph.cited_by_ids,
        );
        let related = Vec::new();

        let mut panel = CitationGraphPanel::new(card, references, cited_by, related);
        panel.set_mode(mode);
        self.popup = Some(Popup::ScienceCitationGraph(panel));
    }

    pub fn open_science_related_panel(&mut self) {
        self.open_science_citation_graph_panel(GraphMode::Related);
    }

    pub fn open_science_find_download_panel(&mut self, query: Option<String>) {
        let selected_card = self.selected_card();
        let inferred_query = query.or_else(|| {
            selected_card
                .as_ref()
                .map(preferred_find_query)
                .filter(|value| !value.trim().is_empty())
        });

        let mut panel = FindDownloadPanel::new(inferred_query.unwrap_or_default());
        if let Some(card) = selected_card.as_ref() {
            panel.set_identifier_kind(match best_identifier_kind(card) {
                Some(kind) => kind,
                None => SearchIdentifierKind::Doi,
            });

            panel.semantic_scholar_results.push(FindResult {
                title: card.metadata.title.clone(),
                authors: card.metadata.authors.clone(),
                year: card.metadata.year,
                primary_id: preferred_identifier_text(card),
                file_format: None,
                file_size: None,
                citation_count: (card.citation_graph.citation_count > 0)
                    .then_some(card.citation_graph.citation_count),
                in_library: true,
                open_url: best_open_url(card),
            });
        }

        self.popup = Some(Popup::ScienceFindDownload(panel));
    }

    pub fn open_science_doi_in_browser(&mut self) {
        let Some(card) = self.selected_card() else {
            self.status_message = "No selected book".to_string();
            return;
        };

        let Some(doi) = parsed_doi(&card) else {
            self.status_message = "No DOI on current card".to_string();
            return;
        };

        self.open_external_url(&doi.url, "DOI");
    }

    pub fn open_science_arxiv_abs(&mut self) {
        let Some(card) = self.selected_card() else {
            self.status_message = "No selected book".to_string();
            return;
        };

        let Some(arxiv_id) = parsed_arxiv_id(&card) else {
            self.status_message = "No arXiv ID on current card".to_string();
            return;
        };

        self.open_external_url(&arxiv_id.abs_url, "arXiv abs");
    }

    pub fn open_science_arxiv_pdf(&mut self) {
        let Some(card) = self.selected_card() else {
            self.status_message = "No selected book".to_string();
            return;
        };

        let Some(arxiv_id) = parsed_arxiv_id(&card) else {
            self.status_message = "No arXiv ID on current card".to_string();
            return;
        };

        self.open_external_url(&arxiv_id.pdf_url, "arXiv PDF");
    }

    pub fn find_science_open_pdf(&mut self) {
        let Some(card) = self.selected_card() else {
            self.status_message = "No selected book".to_string();
            return;
        };

        if let Some(url) = best_open_url(&card) {
            self.open_external_url(&url, "open PDF");
            return;
        }

        if let Some(arxiv_id) = parsed_arxiv_id(&card) {
            self.open_external_url(&arxiv_id.pdf_url, "arXiv PDF");
            return;
        }

        if let Some(doi) = parsed_doi(&card) {
            let core_search = format!("https://core.ac.uk/search?q=doi:{}", doi.normalized);
            self.open_external_url(&core_search, "CORE search");
            return;
        }

        self.status_message = "No Open Access source found (Unpaywall → arXiv → CORE)".to_string();
    }

    pub fn yank_science_doi(&mut self) {
        let Some(card) = self.selected_card() else {
            self.status_message = "No selected book".to_string();
            return;
        };

        let Some(doi) = parsed_doi(&card) else {
            self.status_message = "No DOI on current card".to_string();
            return;
        };

        self.copy_text_to_clipboard("DOI", &doi.normalized);
    }

    pub fn yank_science_arxiv(&mut self) {
        let Some(card) = self.selected_card() else {
            self.status_message = "No selected book".to_string();
            return;
        };

        let Some(arxiv_id) = parsed_arxiv_id(&card) else {
            self.status_message = "No arXiv ID on current card".to_string();
            return;
        };

        let value = format_arxiv_for_storage(&arxiv_id);
        self.copy_text_to_clipboard("arXiv ID", &value);
    }

    pub fn yank_science_bibtex(&mut self) {
        let Some(card) = self.selected_card() else {
            self.status_message = "No selected book".to_string();
            return;
        };

        let bibtex = generate_bibtex(&card, &BibTeXOptions::default());
        self.copy_text_to_clipboard("BibTeX", &bibtex);
    }

    pub fn yank_science_citation(&mut self, style: Option<&str>) {
        let Some(card) = self.selected_card() else {
            self.status_message = "No selected book".to_string();
            return;
        };

        let style = style
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| self.default_citation_style());

        let processor = CslProcessor::default();
        match processor.format_citation(&card, &style) {
            Ok(citation) => self.copy_text_to_clipboard(&format!("citation ({style})"), &citation),
            Err(err) => {
                self.status_message = format!("Citation format error ({style}): {err}");
            }
        }
    }

    pub fn start_edit_science_doi(&mut self) {
        let Some(card) = self.selected_card() else {
            self.status_message = "No selected book".to_string();
            return;
        };

        let value = card
            .identifiers
            .as_ref()
            .and_then(|ids| ids.doi.clone())
            .unwrap_or_default();
        let cursor = value.len();
        self.popup = Some(Popup::EditDoi {
            book_id: card.id.to_string(),
            input: value,
            cursor,
        });
    }

    pub fn start_edit_science_arxiv_id(&mut self) {
        let Some(card) = self.selected_card() else {
            self.status_message = "No selected book".to_string();
            return;
        };

        let value = card
            .identifiers
            .as_ref()
            .and_then(|ids| ids.arxiv_id.clone())
            .unwrap_or_default();
        let cursor = value.len();
        self.popup = Some(Popup::EditArxivId {
            book_id: card.id.to_string(),
            input: value,
            cursor,
        });
    }

    pub fn submit_edit_science_doi(&mut self, book_id: &str, input: &str) {
        let Ok(parsed_book_id) = Uuid::parse_str(book_id) else {
            self.status_message = "Invalid book ID".to_string();
            self.popup = None;
            return;
        };

        let trimmed = input.trim();
        if trimmed.is_empty() {
            self.status_message = "DOI cannot be empty".to_string();
            self.popup = None;
            return;
        }

        let parsed = match Doi::parse(trimmed) {
            Ok(value) => value,
            Err(err) => {
                self.status_message = format!("Invalid DOI: {err}");
                self.popup = None;
                return;
            }
        };

        let cards_dir = self.cards_dir();
        let Ok(mut card) = json_cards::load_card_by_id(&cards_dir, &parsed_book_id) else {
            self.status_message = "Book card not found".to_string();
            self.popup = None;
            return;
        };

        self.push_undo(
            format!("Set DOI for: {}", card.metadata.title),
            omniscope_core::undo::UndoAction::UpsertCards(vec![card.clone()]),
        );

        let mut identifiers = card.identifiers.unwrap_or_default();
        identifiers.doi = Some(parsed.normalized.clone());
        card.identifiers = if identifiers.is_empty() {
            None
        } else {
            Some(identifiers)
        };
        card.touch();

        if let Err(err) = json_cards::save_card(&cards_dir, &card) {
            self.status_message = format!("Failed to save DOI: {err}");
            self.popup = None;
            return;
        }
        if let Some(ref db) = self.db {
            let _ = db.upsert_book(&card);
        }

        self.popup = None;
        self.refresh_books();
        self.status_message = format!("DOI set: {}", parsed.normalized);
    }

    pub fn submit_edit_science_arxiv_id(&mut self, book_id: &str, input: &str) {
        let Ok(parsed_book_id) = Uuid::parse_str(book_id) else {
            self.status_message = "Invalid book ID".to_string();
            self.popup = None;
            return;
        };

        let trimmed = input.trim();
        if trimmed.is_empty() {
            self.status_message = "arXiv ID cannot be empty".to_string();
            self.popup = None;
            return;
        }

        let parsed = match ArxivId::parse(trimmed) {
            Ok(value) => value,
            Err(err) => {
                self.status_message = format!("Invalid arXiv ID: {err}");
                self.popup = None;
                return;
            }
        };

        let cards_dir = self.cards_dir();
        let Ok(mut card) = json_cards::load_card_by_id(&cards_dir, &parsed_book_id) else {
            self.status_message = "Book card not found".to_string();
            self.popup = None;
            return;
        };

        self.push_undo(
            format!("Set arXiv ID for: {}", card.metadata.title),
            omniscope_core::undo::UndoAction::UpsertCards(vec![card.clone()]),
        );

        let mut identifiers = card.identifiers.unwrap_or_default();
        let arxiv_value = format_arxiv_for_storage(&parsed);
        identifiers.arxiv_id = Some(arxiv_value.clone());
        card.identifiers = if identifiers.is_empty() {
            None
        } else {
            Some(identifiers)
        };
        card.touch();

        if let Err(err) = json_cards::save_card(&cards_dir, &card) {
            self.status_message = format!("Failed to save arXiv ID: {err}");
            self.popup = None;
            return;
        }
        if let Some(ref db) = self.db {
            let _ = db.upsert_book(&card);
        }

        self.popup = None;
        self.refresh_books();
        self.status_message = format!("arXiv ID set: {arxiv_value}");
    }

    pub fn trigger_ai_enrich_metadata(&mut self) {
        self.ai_panel_active = true;
        self.ai_input = "@e enrich metadata".to_string();
        self.status_message = "AI: enrich metadata queued".to_string();
    }

    pub fn trigger_ai_extract_references(&mut self) {
        self.ai_panel_active = true;
        self.ai_input = "@r extract references".to_string();
        self.status_message = "AI: extract and resolve references queued".to_string();
    }

    pub fn show_science_citation(&mut self, style: Option<&str>) {
        let Some(card) = self.selected_card() else {
            self.status_message = "No selected book".to_string();
            return;
        };

        let style = style
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| self.default_citation_style());

        let processor = CslProcessor::default();
        match processor.format_citation(&card, &style) {
            Ok(citation) => {
                self.open_science_text_viewer(format!(" Citation ({style}) "), citation)
            }
            Err(err) => {
                self.status_message = format!("Citation format error ({style}): {err}");
            }
        }
    }

    pub fn show_science_bibtex(&mut self) {
        let Some(card) = self.selected_card() else {
            self.status_message = "No selected book".to_string();
            return;
        };

        let bibtex = generate_bibtex(&card, &BibTeXOptions::default());
        self.open_science_text_viewer(" BibTeX ".to_string(), bibtex);
    }

    pub fn open_science_text_viewer(&mut self, title: String, body: String) {
        self.popup = Some(Popup::TextViewer {
            title,
            body,
            scroll: 0,
        });
    }

    pub fn select_book_by_id(&mut self, id: Uuid) -> bool {
        if let Some(pos) = self.books.iter().position(|book| book.id == id) {
            self.selected_index = pos;
            return true;
        }

        self.refresh_books();
        if let Some(pos) = self.books.iter().position(|book| book.id == id) {
            self.selected_index = pos;
            return true;
        }

        false
    }

    pub fn open_external_url(&mut self, url: &str, label: &str) {
        match open::that(url) {
            Ok(_) => {
                self.status_message = format!("Opened {label}: {url}");
            }
            Err(err) => {
                self.status_message = format!("Failed to open {label}: {err}");
            }
        }
    }

    fn selected_card(&self) -> Option<BookCard> {
        let selected = self.selected_book()?;
        json_cards::load_card_by_id(&self.cards_dir(), &selected.id).ok()
    }

    fn default_citation_style(&self) -> String {
        std::env::var("OMNISCOPE_CITATION_STYLE").unwrap_or_else(|_| "ieee".to_string())
    }

    fn copy_text_to_clipboard(&mut self, label: &str, value: &str) {
        let text = value.trim();
        if text.is_empty() {
            self.status_message = format!("{label} is empty");
            return;
        }

        if let Some(clipboard) = self.clipboard.as_mut() {
            if let Err(err) = clipboard.set_text(text.to_string()) {
                self.status_message = format!("Clipboard error: {err}");
                return;
            }
        }

        let register = Register {
            content: RegisterContent::Text(text.to_string()),
            is_append: false,
        };
        self.registers.insert('+', register.clone());
        self.registers.insert('*', register);

        self.status_message = format!("Copied {label} to clipboard");
    }
}

fn parsed_doi(card: &BookCard) -> Option<Doi> {
    card.identifiers
        .as_ref()
        .and_then(|ids| ids.doi.as_deref())
        .and_then(|raw| Doi::parse(raw).ok())
}

fn parsed_arxiv_id(card: &BookCard) -> Option<ArxivId> {
    card.identifiers
        .as_ref()
        .and_then(|ids| ids.arxiv_id.as_deref())
        .and_then(|raw| ArxivId::parse(raw).ok())
}

fn format_arxiv_for_storage(value: &ArxivId) -> String {
    if let Some(version) = value.version {
        format!("{}v{version}", value.id)
    } else {
        value.id.clone()
    }
}

fn best_open_url(card: &BookCard) -> Option<String> {
    if let Some(oa_url) = card
        .open_access
        .as_ref()
        .and_then(|oa| oa.oa_url.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(oa_url.to_string());
    }

    card.open_access
        .as_ref()
        .and_then(|oa| oa.pdf_urls.first())
        .map(|url| url.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn preferred_identifier_text(card: &BookCard) -> Option<String> {
    let ids = card.identifiers.as_ref()?;
    ids.doi
        .as_deref()
        .map(|value| format!("DOI:{value}"))
        .or_else(|| {
            ids.arxiv_id
                .as_deref()
                .map(|value| format!("arXiv:{value}"))
        })
        .or_else(|| {
            ids.semantic_scholar_id
                .as_deref()
                .map(|value| format!("S2:{value}"))
        })
}

fn preferred_find_query(card: &BookCard) -> String {
    if let Some(doi) = card
        .identifiers
        .as_ref()
        .and_then(|ids| ids.doi.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return doi.to_string();
    }
    if let Some(arxiv_id) = card
        .identifiers
        .as_ref()
        .and_then(|ids| ids.arxiv_id.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return arxiv_id.to_string();
    }
    card.metadata.title.clone()
}

fn best_identifier_kind(card: &BookCard) -> Option<SearchIdentifierKind> {
    let ids = card.identifiers.as_ref()?;
    if ids.doi.is_some() {
        return Some(SearchIdentifierKind::Doi);
    }
    if ids.arxiv_id.is_some() {
        return Some(SearchIdentifierKind::Arxiv);
    }
    if ids.isbn13.is_some() || ids.isbn10.is_some() {
        return Some(SearchIdentifierKind::Isbn);
    }
    if ids.pmid.is_some() {
        return Some(SearchIdentifierKind::Pmid);
    }
    None
}

fn build_extracted_references(card: &BookCard) -> Vec<ExtractedReference> {
    let mut references = Vec::new();

    for raw in &card.citation_graph.references {
        let index = references.len() + 1;
        references.push(reference_from_raw(index, raw));
    }

    for source_id in &card.citation_graph.references_ids {
        if references
            .iter()
            .any(|reference| reference.is_in_library == Some(*source_id))
        {
            continue;
        }

        let index = references.len() + 1;
        references.push(ExtractedReference {
            index,
            raw_text: format!("Library item {source_id}"),
            doi: None,
            arxiv_id: None,
            isbn: None,
            resolved_title: Some(format!("Library item {}", short_uuid(*source_id))),
            resolved_authors: Vec::new(),
            resolved_year: None,
            confidence: 1.0,
            resolution_method: ResolutionMethod::SemanticScholar,
            is_in_library: Some(*source_id),
        });
    }

    references
}

fn reference_from_raw(index: usize, raw: &str) -> ExtractedReference {
    let mut reference = ExtractedReference::from_raw(index, raw.trim().to_string());

    if let Some(doi) = extract_dois_from_text(raw).into_iter().next() {
        reference.doi = Some(doi);
        reference.confidence = 1.0;
        reference.resolution_method = ResolutionMethod::DirectDoi;
    }

    if let Some(arxiv_id) = extract_arxiv_ids_from_text(raw).into_iter().next() {
        reference.arxiv_id = Some(arxiv_id);
        if reference.confidence <= 0.0 {
            reference.confidence = 1.0;
            reference.resolution_method = ResolutionMethod::DirectArxiv;
        }
    }

    if reference.resolved_title.is_none() {
        let title = raw.trim();
        if !title.is_empty() {
            reference.resolved_title = Some(title.to_string());
        }
    }

    reference
}

fn build_citation_edges(raw_refs: &[String], source_ids: &[Uuid]) -> Vec<CitationEdge> {
    let mut edges = raw_refs
        .iter()
        .map(|value| edge_from_raw(value))
        .collect::<Vec<_>>();

    for source_id in source_ids {
        if edges.iter().any(|edge| edge.source_id == Some(*source_id)) {
            continue;
        }

        edges.push(CitationEdge {
            source_id: Some(*source_id),
            doi: None,
            arxiv_id: None,
            title: format!("Library item {}", short_uuid(*source_id)),
            authors: Vec::new(),
            year: None,
            citation_context: None,
            is_influential: false,
        });
    }

    edges
}

fn edge_from_raw(raw: &str) -> CitationEdge {
    let doi = extract_dois_from_text(raw).into_iter().next();
    let arxiv_id = extract_arxiv_ids_from_text(raw).into_iter().next();
    CitationEdge {
        source_id: Uuid::parse_str(raw.trim()).ok(),
        doi,
        arxiv_id,
        title: sanitize_reference_title(raw),
        authors: Vec::new(),
        year: None,
        citation_context: None,
        is_influential: false,
    }
}

fn sanitize_reference_title(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "Untitled reference".to_string();
    }

    let without_prefix = trimmed
        .strip_prefix("DOI:")
        .or_else(|| trimmed.strip_prefix("doi:"))
        .or_else(|| trimmed.strip_prefix("arXiv:"))
        .or_else(|| trimmed.strip_prefix("arxiv:"))
        .unwrap_or(trimmed);

    without_prefix.trim().to_string()
}

fn short_uuid(id: Uuid) -> String {
    let text = id.to_string();
    text.chars().take(8).collect()
}
