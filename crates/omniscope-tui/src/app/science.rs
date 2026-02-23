use super::{App, MetadataTaskResult, MetadataTaskState, Register, RegisterContent};
use crate::panels::citation_graph::{CitationEdge, CitationGraphPanel, GraphMode};
use crate::panels::find_download::{FindDownloadPanel, FindResult, SearchIdentifierKind};
use crate::panels::references::ReferencesPanel;
use crate::popup::Popup;
use chrono::Utc;
use omniscope_core::models::{BookCard, BookPublication, DocumentType, ScientificIdentifiers};
use omniscope_core::storage::json_cards;
use omniscope_science::enrichment::EnrichmentPipeline;
use omniscope_science::formats::bibtex::{BibTeXOptions, generate_bibtex};
use omniscope_science::formats::csl::CslProcessor;
use omniscope_science::identifiers::arxiv::ArxivId;
use omniscope_science::identifiers::doi::Doi;
use omniscope_science::identifiers::extract::{
    extract_arxiv_ids_from_text, extract_dois_from_text,
};
use omniscope_science::references::{
    ExtractedReference, LibraryLookup, ReferenceExtractor, ResolutionMethod,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc;
use std::sync::mpsc::TryRecvError;
use std::thread;
use std::time::Instant;
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
        let query_text = panel.query.trim().to_string();

        if !query_text.is_empty() {
            let query_doi = Doi::parse(query_text.as_str()).ok();
            let query_arxiv = ArxivId::parse(query_text.as_str()).ok();

            panel.semantic_scholar_results.push(FindResult {
                title: query_text.clone(),
                authors: Vec::new(),
                year: None,
                primary_id: query_doi
                    .as_ref()
                    .map(|value| format!("DOI:{}", value.normalized))
                    .or_else(|| {
                        query_arxiv
                            .as_ref()
                            .map(|value| format!("arXiv:{}", format_arxiv_for_storage(value)))
                    }),
                file_format: None,
                file_size: None,
                citation_count: None,
                in_library: false,
                open_url: query_doi
                    .as_ref()
                    .map(|value| value.url.clone())
                    .or_else(|| query_arxiv.as_ref().map(|value| value.pdf_url.clone())),
            });
        }

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
        self.run_metadata_enrichment("AI: enrich metadata");
        self.ai_panel_active = false;
    }

    pub fn trigger_metadata_enrich(&mut self) {
        self.run_metadata_enrichment("Metadata enrich");
    }

    fn run_metadata_enrichment(&mut self, status_prefix: &str) {
        if self.metadata_task.is_some() {
            self.status_message = format!("{status_prefix}: already running");
            return;
        }

        let Some(selected) = self.selected_book() else {
            self.status_message = format!("{status_prefix}: no selected book");
            return;
        };

        let Ok(mut card) = json_cards::load_card_by_id(&self.cards_dir(), &selected.id) else {
            self.status_message = format!("{status_prefix}: card not found");
            return;
        };

        if card.file.is_none() {
            self.status_message = format!("{status_prefix}: selected card has no file");
            return;
        }

        let before = card.clone();
        let (tx, rx) = mpsc::channel::<MetadataTaskResult>();
        thread::spawn(move || {
            let report = EnrichmentPipeline::enrich_full_metadata_blocking(&mut card);
            let _ = tx.send(MetadataTaskResult::Completed {
                before,
                after: card,
                report,
            });
        });

        self.metadata_task = Some(MetadataTaskState {
            receiver: rx,
            started_at: Instant::now(),
            spinner_frame: 0,
            status_prefix: status_prefix.to_string(),
        });
        self.status_message =
            format!("{status_prefix}: running... (large PDF files can take ~10-30s)");
    }

    pub fn poll_background_tasks(&mut self) {
        let mut finished = None;
        let mut disconnected = None;

        if let Some(task) = self.metadata_task.as_mut() {
            match task.receiver.try_recv() {
                Ok(result) => {
                    finished = Some((task.status_prefix.clone(), result));
                }
                Err(TryRecvError::Empty) => {
                    task.spinner_frame = task.spinner_frame.wrapping_add(1);
                    let frames = ["|", "/", "-", "\\"];
                    let frame = frames[task.spinner_frame % frames.len()];
                    let elapsed = task.started_at.elapsed().as_secs();
                    self.status_message =
                        format!("{}: running {frame} ({elapsed}s)", task.status_prefix);
                }
                Err(TryRecvError::Disconnected) => {
                    disconnected = Some(task.status_prefix.clone());
                }
            }
        }

        if let Some(prefix) = disconnected {
            self.metadata_task = None;
            self.status_message = format!("{prefix}: worker thread disconnected");
            return;
        }

        if let Some((status_prefix, result)) = finished {
            self.metadata_task = None;
            self.apply_metadata_task_result(&status_prefix, result);
        }
    }

    fn apply_metadata_task_result(&mut self, status_prefix: &str, result: MetadataTaskResult) {
        match result {
            MetadataTaskResult::Failed(err) => {
                self.status_message = format!("{status_prefix}: {err}");
            }
            MetadataTaskResult::Completed {
                before,
                after,
                report,
            } => {
                if report.fields_updated.is_empty() {
                    if let Some(first_error) = report.errors.first() {
                        self.status_message =
                            format!("{status_prefix}: no updates ({first_error})");
                    } else {
                        self.status_message = format!("{status_prefix}: no new fields found");
                    }
                    return;
                }

                self.push_undo(
                    format!("{status_prefix}: {}", after.metadata.title),
                    omniscope_core::undo::UndoAction::UpsertCards(vec![before]),
                );

                let cards_dir = self.cards_dir();
                if let Err(err) = json_cards::save_card(&cards_dir, &after) {
                    self.status_message = format!("{status_prefix}: save failed: {err}");
                    return;
                }
                if let Some(ref db) = self.db {
                    let _ = db.upsert_book(&after);
                }
                self.refresh_books();
                let _ = self.select_book_by_id(after.id);

                if report.errors.is_empty() {
                    self.status_message = format!(
                        "{status_prefix}: {} field(s) updated",
                        report.fields_updated.len()
                    );
                } else {
                    self.status_message = format!(
                        "{status_prefix}: {} field(s) updated, {} warning(s)",
                        report.fields_updated.len(),
                        report.errors.len()
                    );
                }
            }
        }
    }

    pub fn trigger_ai_extract_references(&mut self) {
        self.ai_panel_active = true;
        self.ai_input = "@r extract references".to_string();
        self.run_reference_extraction("AI: extract references");
        self.ai_panel_active = false;
    }

    fn run_reference_extraction(&mut self, status_prefix: &str) {
        let Some(selected) = self.selected_book() else {
            self.status_message = format!("{status_prefix}: no selected book");
            return;
        };

        let cards_dir = self.cards_dir();
        let Ok(mut card) = json_cards::load_card_by_id(&cards_dir, &selected.id) else {
            self.status_message = format!("{status_prefix}: card not found");
            return;
        };

        let all_cards = match json_cards::list_cards(&cards_dir) {
            Ok(cards) => cards,
            Err(err) => {
                self.status_message =
                    format!("{status_prefix}: failed to read library cards: {err}");
                return;
            }
        };

        let lookup = Arc::new(CardLibraryLookup::from_cards(&all_cards, Some(card.id)));
        let extractor = ReferenceExtractor::from_env().with_library_lookup(lookup);
        let references = match extractor.extract_blocking(&card) {
            Ok(value) => value,
            Err(err) => {
                self.status_message = format!("{status_prefix}: extraction failed: {err}");
                return;
            }
        };

        if references.is_empty() {
            self.status_message = format!("{status_prefix}: no references found");
            return;
        }

        let before = card.clone();
        let changed = apply_extracted_references(&mut card, &references);
        if !changed {
            self.status_message = format!("{status_prefix}: references are already up to date");
            return;
        }

        self.push_undo(
            format!("{status_prefix}: {}", card.metadata.title),
            omniscope_core::undo::UndoAction::UpsertCards(vec![before]),
        );

        if let Err(err) = json_cards::save_card(&cards_dir, &card) {
            self.status_message = format!("{status_prefix}: save failed: {err}");
            return;
        }
        if let Some(ref db) = self.db {
            let _ = db.upsert_book(&card);
        }
        self.refresh_books();

        let linked_count = references
            .iter()
            .filter(|reference| reference.is_in_library.is_some())
            .count();
        self.status_message = format!(
            "{status_prefix}: {} reference(s) extracted, {} linked to library",
            references.len(),
            linked_count
        );
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

    pub fn add_science_entry_to_library(
        &mut self,
        title_hint: &str,
        authors: &[String],
        year: Option<i32>,
        doi_hint: Option<&str>,
        arxiv_hint: Option<&str>,
        source_label: &str,
    ) {
        let doi = doi_hint
            .and_then(|value| Doi::parse(value).ok())
            .map(|value| value.normalized);
        let arxiv_id = arxiv_hint
            .and_then(|value| ArxivId::parse(value).ok())
            .map(|value| format_arxiv_for_storage(&value));

        if doi.is_none() && arxiv_id.is_none() {
            self.status_message = format!("{source_label}: no DOI/arXiv on selected item");
            return;
        }

        let cards_dir = self.cards_dir();
        let cards = match json_cards::list_cards(&cards_dir) {
            Ok(value) => value,
            Err(err) => {
                self.status_message =
                    format!("{source_label}: failed to read library cards: {err}");
                return;
            }
        };

        if let Some(existing) = cards
            .iter()
            .find(|card| identifiers_match(card, doi.as_deref(), arxiv_id.as_deref()))
        {
            let _ = self.select_book_by_id(existing.id);
            self.status_message = format!("{source_label}: already in library");
            return;
        }

        let fallback_title = doi
            .as_ref()
            .map(|value| format!("DOI:{value}"))
            .or_else(|| arxiv_id.as_ref().map(|value| format!("arXiv:{value}")))
            .unwrap_or_else(|| "Untitled reference".to_string());
        let title = title_hint.trim();

        let mut card = BookCard::new(if title.is_empty() {
            fallback_title.as_str()
        } else {
            title
        });
        card.metadata.authors = authors.to_vec();
        card.metadata.year = year;
        card.publication = Some(BookPublication {
            doc_type: DocumentType::Article,
            ..Default::default()
        });
        card.identifiers = Some(ScientificIdentifiers {
            doi,
            arxiv_id,
            ..Default::default()
        });

        let report = EnrichmentPipeline::enrich_full_metadata_blocking(&mut card);
        let card_id = card.id;
        let saved_title = card.metadata.title.clone();

        if let Err(err) = json_cards::save_card(&cards_dir, &card) {
            self.status_message = format!("{source_label}: save failed: {err}");
            return;
        }
        if let Some(ref db) = self.db {
            let _ = db.upsert_book(&card);
        }
        self.refresh_books();
        let _ = self.select_book_by_id(card_id);

        self.status_message = format!(
            "{source_label}: added \"{saved_title}\" ({} update(s), {} warning(s))",
            report.fields_updated.len(),
            report.errors.len()
        );
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

    let without_identifiers = without_prefix
        .split_once(" DOI:")
        .map(|(head, _)| head)
        .or_else(|| without_prefix.split_once(" doi:").map(|(head, _)| head))
        .or_else(|| without_prefix.split_once(" arXiv:").map(|(head, _)| head))
        .or_else(|| without_prefix.split_once(" arxiv:").map(|(head, _)| head))
        .unwrap_or(without_prefix);

    let cleaned = without_identifiers
        .trim()
        .trim_end_matches([',', ';', ':', '.', '-'])
        .trim();
    if cleaned.is_empty() {
        trimmed.to_string()
    } else {
        cleaned.to_string()
    }
}

fn short_uuid(id: Uuid) -> String {
    let text = id.to_string();
    text.chars().take(8).collect()
}

fn identifiers_match(card: &BookCard, doi: Option<&str>, arxiv_id: Option<&str>) -> bool {
    let ids = match card.identifiers.as_ref() {
        Some(value) => value,
        None => return false,
    };

    let doi_match = match (doi, ids.doi.as_deref()) {
        (Some(target), Some(existing)) => Doi::parse(existing)
            .map(|parsed| parsed.normalized == target)
            .unwrap_or_else(|_| existing.eq_ignore_ascii_case(target)),
        _ => false,
    };

    let arxiv_match = match (arxiv_id, ids.arxiv_id.as_deref()) {
        (Some(target), Some(existing)) => ArxivId::parse(existing)
            .map(|parsed| format_arxiv_for_storage(&parsed) == target)
            .unwrap_or_else(|_| existing.eq_ignore_ascii_case(target)),
        _ => false,
    };

    doi_match || arxiv_match
}

fn apply_extracted_references(card: &mut BookCard, references: &[ExtractedReference]) -> bool {
    let mut raw_references = Vec::new();
    let mut source_ids = Vec::new();

    for reference in references {
        let raw = format_extracted_reference(reference);
        if !raw.trim().is_empty() && !raw_references.contains(&raw) {
            raw_references.push(raw);
        }
        if let Some(source_id) = reference.is_in_library
            && !source_ids.contains(&source_id)
        {
            source_ids.push(source_id);
        }
    }

    source_ids.sort_unstable();
    let mut changed = false;

    if card.citation_graph.references != raw_references {
        card.citation_graph.references = raw_references;
        changed = true;
    }
    if card.citation_graph.references_ids != source_ids {
        card.citation_graph.references_ids = source_ids;
        changed = true;
    }

    let extracted_count = u32::try_from(card.citation_graph.references.len()).unwrap_or(u32::MAX);
    if card.citation_graph.reference_count < extracted_count {
        card.citation_graph.reference_count = extracted_count;
        changed = true;
    }

    if changed {
        card.citation_graph.last_updated = Some(Utc::now());
        card.touch();
    }

    changed
}

fn format_extracted_reference(reference: &ExtractedReference) -> String {
    if let Some(doi) = &reference.doi {
        return format!("DOI:{}", doi.normalized);
    }
    if let Some(arxiv_id) = &reference.arxiv_id {
        return format!("arXiv:{}", format_arxiv_for_storage(arxiv_id));
    }
    reference
        .resolved_title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| reference.raw_text.trim().to_string())
}

#[derive(Default)]
struct CardLibraryLookup {
    doi_to_book: HashMap<String, Uuid>,
    arxiv_to_book: HashMap<String, Uuid>,
}

impl CardLibraryLookup {
    fn from_cards(cards: &[BookCard], skip_id: Option<Uuid>) -> Self {
        let mut lookup = Self::default();
        for card in cards {
            if skip_id.is_some_and(|id| id == card.id) {
                continue;
            }

            let Some(identifiers) = card.identifiers.as_ref() else {
                continue;
            };

            if let Some(doi_raw) = identifiers.doi.as_deref()
                && let Ok(doi) = Doi::parse(doi_raw)
            {
                lookup
                    .doi_to_book
                    .entry(doi.normalized.clone())
                    .or_insert(card.id);
            }

            if let Some(arxiv_raw) = identifiers.arxiv_id.as_deref()
                && let Ok(arxiv_id) = ArxivId::parse(arxiv_raw)
            {
                let key = format_arxiv_for_storage(&arxiv_id);
                lookup.arxiv_to_book.entry(key).or_insert(card.id);
            }
        }

        lookup
    }
}

impl LibraryLookup for CardLibraryLookup {
    fn find_by_doi(&self, doi: &Doi) -> Option<Uuid> {
        self.doi_to_book.get(&doi.normalized).copied()
    }

    fn find_by_arxiv(&self, arxiv_id: &ArxivId) -> Option<Uuid> {
        let key = format_arxiv_for_storage(arxiv_id);
        self.arxiv_to_book.get(&key).copied()
    }
}
