use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use omniscope_science::references::{ExtractedReference, ResolutionMethod};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};
use uuid::Uuid;

use crate::theme::NordTheme;
use crate::ui::truncate;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RefsFilter {
    #[default]
    All,
    Resolved,
    Unresolved,
    InLibrary,
    NotInLibrary,
}

impl RefsFilter {
    const ORDER: [Self; 5] = [
        Self::All,
        Self::Resolved,
        Self::Unresolved,
        Self::InLibrary,
        Self::NotInLibrary,
    ];

    pub fn next(self) -> Self {
        let idx = Self::ORDER.iter().position(|candidate| *candidate == self);
        let Some(idx) = idx else {
            return Self::All;
        };
        Self::ORDER[(idx + 1) % Self::ORDER.len()]
    }

    pub fn previous(self) -> Self {
        let idx = Self::ORDER.iter().position(|candidate| *candidate == self);
        let Some(idx) = idx else {
            return Self::All;
        };
        if idx == 0 {
            Self::ORDER[Self::ORDER.len() - 1]
        } else {
            Self::ORDER[idx - 1]
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Resolved => "resolved",
            Self::Unresolved => "unresolved",
            Self::InLibrary => "in-library",
            Self::NotInLibrary => "not-in-library",
        }
    }

    fn matches(self, reference: &ExtractedReference) -> bool {
        match self {
            Self::All => true,
            Self::Resolved => !is_unresolved(reference),
            Self::Unresolved => is_unresolved(reference),
            Self::InLibrary => reference.is_in_library.is_some(),
            Self::NotInLibrary => reference.is_in_library.is_none(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferencesPanelAction {
    OpenBook(Uuid),
    ShowDetails {
        reference_index: usize,
    },
    AddReference {
        reference_index: usize,
        target: Option<ReferenceAddTarget>,
    },
    FindOnline {
        reference_index: usize,
    },
    AddAllUnresolved {
        reference_indices: Vec<usize>,
    },
    Export {
        reference_indices: Vec<usize>,
    },
    StartSearch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceAddTarget {
    Doi(String),
    Arxiv(String),
}

#[derive(Debug, Clone)]
pub struct ReferencesPanel {
    pub references: Vec<ExtractedReference>,
    pub filter: RefsFilter,
    pub cursor: usize,
    pub scroll: usize,
}

impl ReferencesPanel {
    pub fn new(references: Vec<ExtractedReference>) -> Self {
        Self {
            references,
            filter: RefsFilter::All,
            cursor: 0,
            scroll: 0,
        }
    }

    pub fn filtered_len(&self) -> usize {
        self.references
            .iter()
            .filter(|reference| self.filter.matches(reference))
            .count()
    }

    pub fn selected_reference_index(&self) -> Option<usize> {
        let filtered = self.filtered_indices();
        if filtered.is_empty() {
            return None;
        }
        let selected = self.cursor.min(filtered.len().saturating_sub(1));
        filtered.get(selected).copied()
    }

    pub fn selected_reference(&self) -> Option<&ExtractedReference> {
        let idx = self.selected_reference_index()?;
        self.references.get(idx)
    }

    pub fn cycle_filter(&mut self) {
        self.filter = self.filter.next();
        self.cursor = 0;
        self.scroll = 0;
    }

    pub fn cycle_filter_back(&mut self) {
        self.filter = self.filter.previous();
        self.cursor = 0;
        self.scroll = 0;
    }

    pub fn move_down(&mut self, viewport_height: usize) {
        let len = self.filtered_len();
        if len == 0 {
            self.cursor = 0;
            self.scroll = 0;
            return;
        }

        if self.cursor + 1 < len {
            self.cursor += 1;
        }
        self.sync_viewport(viewport_height);
    }

    pub fn move_up(&mut self, viewport_height: usize) {
        if self.filtered_len() == 0 {
            self.cursor = 0;
            self.scroll = 0;
            return;
        }

        self.cursor = self.cursor.saturating_sub(1);
        self.sync_viewport(viewport_height);
    }

    pub fn search(&mut self, query: &str, viewport_height: usize) -> bool {
        let needle = query.trim().to_lowercase();
        if needle.is_empty() {
            return false;
        }

        let filtered = self.filtered_indices();
        if filtered.is_empty() {
            return false;
        }

        let len = filtered.len();
        let start = self.cursor.saturating_add(1).min(len);
        let positions = (start..len).chain(0..start);

        for pos in positions {
            let Some(reference) = self.references.get(filtered[pos]) else {
                continue;
            };
            if searchable_reference_text(reference).contains(&needle) {
                self.cursor = pos;
                self.sync_viewport(viewport_height);
                return true;
            }
        }

        false
    }

    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        viewport_height: usize,
    ) -> Option<ReferencesPanelAction> {
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.move_down(viewport_height);
                None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_up(viewport_height);
                None
            }
            KeyCode::Tab => {
                self.cycle_filter();
                None
            }
            KeyCode::BackTab => {
                self.cycle_filter_back();
                None
            }
            KeyCode::Enter => {
                let selected = self.selected_reference_index()?;
                let book_id = self
                    .references
                    .get(selected)
                    .and_then(|reference| reference.is_in_library);
                if let Some(book_id) = book_id {
                    Some(ReferencesPanelAction::OpenBook(book_id))
                } else {
                    Some(ReferencesPanelAction::ShowDetails {
                        reference_index: selected,
                    })
                }
            }
            KeyCode::Char('a') if key.modifiers == KeyModifiers::NONE => {
                let selected = self.selected_reference_index()?;
                Some(ReferencesPanelAction::AddReference {
                    reference_index: selected,
                    target: self.add_target_for(selected),
                })
            }
            KeyCode::Char('f') if key.modifiers == KeyModifiers::NONE => {
                let selected = self.selected_reference_index()?;
                Some(ReferencesPanelAction::FindOnline {
                    reference_index: selected,
                })
            }
            KeyCode::Char('A') => {
                let unresolved = self.unresolved_not_in_library_indices();
                Some(ReferencesPanelAction::AddAllUnresolved {
                    reference_indices: unresolved,
                })
            }
            KeyCode::Char('e') if key.modifiers == KeyModifiers::NONE => {
                Some(ReferencesPanelAction::Export {
                    reference_indices: self.filtered_indices(),
                })
            }
            KeyCode::Char('/') if key.modifiers == KeyModifiers::NONE => {
                Some(ReferencesPanelAction::StartSearch)
            }
            _ => None,
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, theme: &NordTheme, book_title: &str) {
        if area.is_empty() {
            return;
        }

        let filtered_count = self.filtered_len();
        let total_count = self.references.len();
        let title = if self.filter == RefsFilter::All {
            format!(" REFERENCES - \"{book_title}\" [{total_count} references] ")
        } else {
            format!(" REFERENCES - \"{book_title}\" [{filtered_count}/{total_count} references] ")
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.active_panel()))
            .style(Style::default().bg(theme.bg()));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.height < 4 || inner.width < 24 {
            return;
        }

        let sections = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(2),
                Constraint::Length(1),
            ])
            .split(inner);

        frame.render_widget(
            Paragraph::new(render_filter_line(self.filter, theme)),
            sections[0],
        );

        let body_height = usize::from(sections[1].height.saturating_sub(1));
        self.sync_viewport(body_height);

        if filtered_count == 0 {
            let empty = Paragraph::new(Span::styled(
                "No references for current filter",
                Style::default()
                    .fg(theme.muted())
                    .add_modifier(Modifier::DIM),
            ));
            frame.render_widget(empty, sections[1]);
            frame.render_widget(footer_hint(theme), sections[2]);
            return;
        }

        let max_reference_width = usize::from(sections[1].width.saturating_sub(29));
        let rows = self
            .filtered_indices()
            .into_iter()
            .enumerate()
            .skip(self.scroll)
            .take(body_height)
            .filter_map(|(filtered_pos, reference_idx)| {
                let reference = self.references.get(reference_idx)?;
                let number = if reference.index > 0 {
                    reference.index.to_string()
                } else {
                    (reference_idx + 1).to_string()
                };

                let reference_text = reference_cell_text(reference, max_reference_width);
                let id_text = reference_id_text(reference);
                let in_library = library_cell_text(reference);

                let mut row_style = row_style(reference, filtered_pos == self.cursor, theme);
                if filtered_pos == self.cursor {
                    row_style = row_style.add_modifier(Modifier::BOLD);
                }

                Some(
                    Row::new([
                        Cell::from(number),
                        Cell::from(reference_text),
                        Cell::from(id_text),
                        Cell::from(in_library),
                    ])
                    .style(row_style),
                )
            })
            .collect::<Vec<_>>();

        let header = Row::new([
            Cell::from("#"),
            Cell::from("Reference"),
            Cell::from("ID"),
            Cell::from("In Library"),
        ])
        .style(
            Style::default()
                .fg(theme.fg_bright())
                .add_modifier(Modifier::BOLD),
        );

        let table = Table::new(
            rows,
            [
                Constraint::Length(4),
                Constraint::Percentage(58),
                Constraint::Percentage(16),
                Constraint::Percentage(22),
            ],
        )
        .header(header)
        .column_spacing(1)
        .style(Style::default().bg(theme.bg()).fg(theme.fg()));
        frame.render_widget(table, sections[1]);

        frame.render_widget(footer_hint(theme), sections[2]);
    }

    fn unresolved_not_in_library_indices(&self) -> Vec<usize> {
        self.references
            .iter()
            .enumerate()
            .filter_map(|(idx, reference)| {
                if is_unresolved(reference) && reference.is_in_library.is_none() {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn add_target_for(&self, reference_index: usize) -> Option<ReferenceAddTarget> {
        let reference = self.references.get(reference_index)?;

        if let Some(doi) = &reference.doi {
            return Some(ReferenceAddTarget::Doi(doi.normalized.clone()));
        }

        if let Some(arxiv_id) = &reference.arxiv_id {
            let normalized = if let Some(version) = arxiv_id.version {
                format!("{}v{version}", arxiv_id.id)
            } else {
                arxiv_id.id.clone()
            };
            return Some(ReferenceAddTarget::Arxiv(normalized));
        }

        None
    }

    fn filtered_indices(&self) -> Vec<usize> {
        self.references
            .iter()
            .enumerate()
            .filter_map(|(idx, reference)| self.filter.matches(reference).then_some(idx))
            .collect()
    }

    fn sync_viewport(&mut self, viewport_height: usize) {
        let len = self.filtered_len();
        if len == 0 || viewport_height == 0 {
            self.cursor = 0;
            self.scroll = 0;
            return;
        }

        if self.cursor >= len {
            self.cursor = len - 1;
        }

        let max_scroll = len.saturating_sub(viewport_height);
        if self.scroll > max_scroll {
            self.scroll = max_scroll;
        }

        if self.cursor < self.scroll {
            self.scroll = self.cursor;
            return;
        }

        let window_bottom = self
            .scroll
            .saturating_add(viewport_height)
            .saturating_sub(1);
        if self.cursor > window_bottom {
            self.scroll = self
                .cursor
                .saturating_add(1)
                .saturating_sub(viewport_height);
        }
    }
}

fn is_unresolved(reference: &ExtractedReference) -> bool {
    reference.resolution_method == ResolutionMethod::Unresolved
}

fn reference_cell_text(reference: &ExtractedReference, max_width: usize) -> String {
    let title = reference
        .resolved_title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| reference.raw_text.trim());

    let mut text = title.to_string();
    if !reference.resolved_authors.is_empty() {
        text.push_str(" - ");
        text.push_str(&reference.resolved_authors.join(", "));
    }

    if max_width > 0 {
        truncate(&text, max_width)
    } else {
        text
    }
}

fn reference_id_text(reference: &ExtractedReference) -> String {
    if let Some(arxiv_id) = &reference.arxiv_id {
        if let Some(version) = arxiv_id.version {
            return format!("arXiv:{}v{version}", arxiv_id.id);
        }
        return format!("arXiv:{}", arxiv_id.id);
    }

    if let Some(doi) = &reference.doi {
        return format!("DOI:{}", doi.normalized);
    }

    if let Some(isbn) = &reference.isbn {
        return format!("ISBN:{}", isbn.formatted);
    }

    "—".to_string()
}

fn library_cell_text(reference: &ExtractedReference) -> String {
    if reference.is_in_library.is_some() {
        return "✓".to_string();
    }
    if is_unresolved(reference) {
        return "✗ [A]dd [F]ind".to_string();
    }
    "✗".to_string()
}

fn row_style(reference: &ExtractedReference, is_selected: bool, theme: &NordTheme) -> Style {
    let mut style = if reference.confidence < 0.7 {
        Style::default().fg(theme.nord3).add_modifier(Modifier::DIM)
    } else {
        Style::default().fg(theme.fg())
    };

    if is_selected {
        style = style.bg(theme.bg_secondary());
    }

    style
}

fn render_filter_line(filter: RefsFilter, theme: &NordTheme) -> Line<'static> {
    let mut spans = vec![Span::styled(
        "Filter:",
        Style::default()
            .fg(theme.muted())
            .add_modifier(Modifier::DIM),
    )];

    for candidate in RefsFilter::ORDER {
        let text = format!(" [{}] ", candidate.label());
        if candidate == filter {
            spans.push(Span::styled(
                text,
                Style::default()
                    .fg(theme.frost_ice())
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(text, Style::default().fg(theme.fg())));
        }
    }

    Line::from(spans)
}

fn footer_hint(theme: &NordTheme) -> Paragraph<'static> {
    let line = Line::from(vec![
        Span::styled(
            "[Enter]",
            Style::default()
                .fg(theme.yellow())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " open/details  ",
            Style::default()
                .fg(theme.muted())
                .add_modifier(Modifier::DIM),
        ),
        Span::styled(
            "[A/a]",
            Style::default()
                .fg(theme.yellow())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " add  ",
            Style::default()
                .fg(theme.muted())
                .add_modifier(Modifier::DIM),
        ),
        Span::styled(
            "[F/f]",
            Style::default()
                .fg(theme.yellow())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " find  ",
            Style::default()
                .fg(theme.muted())
                .add_modifier(Modifier::DIM),
        ),
        Span::styled(
            "[e] export  ",
            Style::default()
                .fg(theme.yellow())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "[/] search",
            Style::default()
                .fg(theme.yellow())
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    Paragraph::new(line)
}

fn searchable_reference_text(reference: &ExtractedReference) -> String {
    let mut parts = Vec::new();
    parts.push(reference.raw_text.to_lowercase());

    if let Some(title) = &reference.resolved_title {
        parts.push(title.to_lowercase());
    }

    if !reference.resolved_authors.is_empty() {
        parts.push(reference.resolved_authors.join(" ").to_lowercase());
    }

    if let Some(doi) = &reference.doi {
        parts.push(doi.normalized.to_lowercase());
    }

    if let Some(arxiv_id) = &reference.arxiv_id {
        let mut value = arxiv_id.id.to_lowercase();
        if let Some(version) = arxiv_id.version {
            value.push_str(&format!("v{version}"));
        }
        parts.push(value);
    }

    if let Some(isbn) = &reference.isbn {
        parts.push(isbn.isbn13.to_lowercase());
    }

    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use omniscope_science::identifiers::{arxiv::ArxivId, doi::Doi, isbn::Isbn};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn base_reference(index: usize, raw: &str) -> ExtractedReference {
        ExtractedReference::from_raw(index, raw)
    }

    #[test]
    fn filters_match_expected_reference_sets() {
        let mut unresolved = base_reference(1, "Unresolved raw reference");
        unresolved.confidence = 0.2;

        let mut resolved = base_reference(2, "Resolved by DOI");
        resolved.doi = Some(Doi::parse("10.1000/xyz123").expect("valid doi"));
        resolved.resolution_method = ResolutionMethod::DirectDoi;
        resolved.confidence = 1.0;

        let mut in_library = base_reference(3, "Reference in library");
        in_library.arxiv_id = Some(ArxivId::parse("1706.03762").expect("valid arxiv"));
        in_library.resolution_method = ResolutionMethod::DirectArxiv;
        in_library.is_in_library = Some(Uuid::new_v4());
        in_library.confidence = 1.0;

        let mut panel = ReferencesPanel::new(vec![unresolved, resolved, in_library]);

        assert_eq!(panel.filtered_indices(), vec![0, 1, 2]);

        panel.filter = RefsFilter::Resolved;
        assert_eq!(panel.filtered_indices(), vec![1, 2]);

        panel.filter = RefsFilter::Unresolved;
        assert_eq!(panel.filtered_indices(), vec![0]);

        panel.filter = RefsFilter::InLibrary;
        assert_eq!(panel.filtered_indices(), vec![2]);

        panel.filter = RefsFilter::NotInLibrary;
        assert_eq!(panel.filtered_indices(), vec![0, 1]);
    }

    #[test]
    fn tab_cycles_filters() {
        let mut panel = ReferencesPanel::new(Vec::new());
        assert_eq!(panel.filter, RefsFilter::All);

        panel.handle_key(key(KeyCode::Tab), 10);
        assert_eq!(panel.filter, RefsFilter::Resolved);

        panel.handle_key(key(KeyCode::Tab), 10);
        assert_eq!(panel.filter, RefsFilter::Unresolved);

        panel.handle_key(key(KeyCode::Tab), 10);
        assert_eq!(panel.filter, RefsFilter::InLibrary);

        panel.handle_key(key(KeyCode::Tab), 10);
        assert_eq!(panel.filter, RefsFilter::NotInLibrary);

        panel.handle_key(key(KeyCode::Tab), 10);
        assert_eq!(panel.filter, RefsFilter::All);
    }

    #[test]
    fn enter_opens_library_item_or_shows_details() {
        let in_library_id = Uuid::new_v4();

        let mut in_library = base_reference(1, "Known");
        in_library.is_in_library = Some(in_library_id);
        in_library.resolution_method = ResolutionMethod::DirectDoi;
        in_library.doi = Some(Doi::parse("10.1000/known").expect("valid doi"));

        let unknown = base_reference(2, "Unknown");

        let mut panel = ReferencesPanel::new(vec![in_library, unknown]);

        let action = panel.handle_key(key(KeyCode::Enter), 10);
        assert_eq!(action, Some(ReferencesPanelAction::OpenBook(in_library_id)));

        panel.move_down(10);
        let action = panel.handle_key(key(KeyCode::Enter), 10);
        assert_eq!(
            action,
            Some(ReferencesPanelAction::ShowDetails { reference_index: 1 })
        );
    }

    #[test]
    fn panel_actions_for_add_find_export_and_search_are_emitted() {
        let mut unresolved = base_reference(1, "No identifiers");
        unresolved.confidence = 0.1;
        let mut resolved = base_reference(2, "Resolved");
        resolved.doi = Some(Doi::parse("10.1000/resolved").expect("valid doi"));
        resolved.resolution_method = ResolutionMethod::DirectDoi;

        let mut panel = ReferencesPanel::new(vec![unresolved, resolved]);

        assert_eq!(
            panel.handle_key(key(KeyCode::Char('a')), 10),
            Some(ReferencesPanelAction::AddReference {
                reference_index: 0,
                target: None
            })
        );
        assert_eq!(
            panel.handle_key(key(KeyCode::Char('f')), 10),
            Some(ReferencesPanelAction::FindOnline { reference_index: 0 })
        );
        assert_eq!(
            panel.handle_key(KeyEvent::new(KeyCode::Char('A'), KeyModifiers::NONE), 10),
            Some(ReferencesPanelAction::AddAllUnresolved {
                reference_indices: vec![0]
            })
        );
        assert_eq!(
            panel.handle_key(key(KeyCode::Char('e')), 10),
            Some(ReferencesPanelAction::Export {
                reference_indices: vec![0, 1]
            })
        );
        assert_eq!(
            panel.handle_key(key(KeyCode::Char('/')), 10),
            Some(ReferencesPanelAction::StartSearch)
        );
    }

    #[test]
    fn search_moves_selection_to_matching_reference() {
        let mut first = base_reference(1, "paper one");
        first.resolved_title = Some("Neural language model".to_string());
        first.resolution_method = ResolutionMethod::SemanticScholar;

        let mut second = base_reference(2, "paper two");
        second.resolved_title = Some("Attention is all you need".to_string());
        second.resolution_method = ResolutionMethod::SemanticScholar;

        let mut panel = ReferencesPanel::new(vec![first, second]);
        panel.cursor = 0;

        assert!(panel.search("attention", 2));
        assert_eq!(panel.selected_reference_index(), Some(1));
    }

    #[test]
    fn low_confidence_rows_use_dim_nord3_style() {
        let mut reference = base_reference(1, "dim me");
        reference.confidence = 0.4;
        let theme = NordTheme::default();

        let style = row_style(&reference, false, &theme);
        assert_eq!(style.fg, Some(theme.nord3));
        assert!(style.add_modifier.contains(Modifier::DIM));
    }

    #[test]
    fn unresolved_row_shows_add_find_hint() {
        let reference = base_reference(1, "unresolved");
        assert_eq!(library_cell_text(&reference), "✗ [A]dd [F]ind");
    }

    #[test]
    fn reference_id_prefers_arxiv_then_doi_then_isbn() {
        let mut arxiv = base_reference(1, "arxiv");
        arxiv.arxiv_id = Some(ArxivId::parse("1706.03762v5").expect("valid arxiv"));
        assert_eq!(reference_id_text(&arxiv), "arXiv:1706.03762v5");

        let mut doi = base_reference(2, "doi");
        doi.doi = Some(Doi::parse("10.1000/xyz123").expect("valid doi"));
        assert_eq!(reference_id_text(&doi), "DOI:10.1000/xyz123");

        let mut isbn = base_reference(3, "isbn");
        isbn.isbn = Some(Isbn::parse("9780306406157").expect("valid isbn"));
        assert_eq!(reference_id_text(&isbn), "ISBN:978-0-3064-0615-7");
    }

    #[test]
    fn add_target_prefers_doi_then_arxiv() {
        let mut doi = base_reference(1, "doi");
        doi.doi = Some(Doi::parse("10.1000/xyz123").expect("valid doi"));
        doi.arxiv_id = Some(ArxivId::parse("1706.03762").expect("valid arxiv"));

        let mut arxiv = base_reference(2, "arxiv");
        arxiv.arxiv_id = Some(ArxivId::parse("1706.03762v5").expect("valid arxiv"));

        let none = base_reference(3, "none");

        let panel = ReferencesPanel::new(vec![doi, arxiv, none]);

        assert_eq!(
            panel.add_target_for(0),
            Some(ReferenceAddTarget::Doi("10.1000/xyz123".to_string()))
        );
        assert_eq!(
            panel.add_target_for(1),
            Some(ReferenceAddTarget::Arxiv("1706.03762v5".to_string()))
        );
        assert_eq!(panel.add_target_for(2), None);
    }
}
