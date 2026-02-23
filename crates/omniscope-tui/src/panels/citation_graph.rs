use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use omniscope_core::models::BookCard;
use omniscope_science::identifiers::{arxiv::ArxivId, doi::Doi};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use uuid::Uuid;

use crate::theme::NordTheme;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GraphMode {
    #[default]
    References,
    CitedBy,
    Related,
}

impl GraphMode {
    const ORDER: [Self; 3] = [Self::References, Self::CitedBy, Self::Related];

    pub fn next(self) -> Self {
        let idx = Self::ORDER.iter().position(|candidate| *candidate == self);
        let Some(idx) = idx else {
            return Self::References;
        };
        Self::ORDER[(idx + 1) % Self::ORDER.len()]
    }

    pub fn previous(self) -> Self {
        let idx = Self::ORDER.iter().position(|candidate| *candidate == self);
        let Some(idx) = idx else {
            return Self::References;
        };
        if idx == 0 {
            Self::ORDER[Self::ORDER.len() - 1]
        } else {
            Self::ORDER[idx - 1]
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::References => "References",
            Self::CitedBy => "Cited By",
            Self::Related => "Related",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CitationEdge {
    pub source_id: Option<Uuid>,
    pub doi: Option<Doi>,
    pub arxiv_id: Option<ArxivId>,
    pub title: String,
    pub authors: Vec<String>,
    pub year: Option<i32>,
    pub citation_context: Option<String>,
    pub is_influential: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CitationGraphPanelAction {
    OpenBook(Uuid),
    AddToLibrary {
        mode: GraphMode,
        edge_index: usize,
        target: Option<CitationAddTarget>,
    },
    FindOnline {
        mode: GraphMode,
        edge_index: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CitationAddTarget {
    Doi(String),
    Arxiv(String),
}

#[derive(Debug, Clone)]
pub struct CitationGraphPanel {
    pub book: BookCard,
    pub mode: GraphMode,
    pub references: Vec<CitationEdge>,
    pub cited_by: Vec<CitationEdge>,
    pub related: Vec<CitationEdge>,
    pub cursor: usize,
}

impl CitationGraphPanel {
    pub fn new(
        book: BookCard,
        references: Vec<CitationEdge>,
        cited_by: Vec<CitationEdge>,
        related: Vec<CitationEdge>,
    ) -> Self {
        Self {
            book,
            mode: GraphMode::References,
            references,
            cited_by,
            related,
            cursor: 0,
        }
    }

    pub fn set_mode(&mut self, mode: GraphMode) {
        self.mode = mode;
        self.sync_cursor();
    }

    pub fn cycle_mode(&mut self) {
        self.mode = self.mode.next();
        self.sync_cursor();
    }

    pub fn cycle_mode_back(&mut self) {
        self.mode = self.mode.previous();
        self.sync_cursor();
    }

    pub fn move_down(&mut self) {
        let len = self.active_len();
        if len == 0 {
            self.cursor = 0;
            return;
        }

        if self.cursor + 1 < len {
            self.cursor += 1;
        }
    }

    pub fn move_up(&mut self) {
        if self.active_len() == 0 {
            self.cursor = 0;
            return;
        }
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn selected_edge_index(&self) -> Option<usize> {
        let len = self.active_len();
        if len == 0 {
            return None;
        }
        Some(self.cursor.min(len.saturating_sub(1)))
    }

    pub fn selected_edge(&self) -> Option<&CitationEdge> {
        let idx = self.selected_edge_index()?;
        self.active_edges().get(idx)
    }

    pub fn add_target_for(&self, edge_index: usize) -> Option<CitationAddTarget> {
        let edge = self.active_edges().get(edge_index)?;
        if let Some(doi) = &edge.doi {
            return Some(CitationAddTarget::Doi(doi.normalized.clone()));
        }
        if let Some(arxiv_id) = &edge.arxiv_id {
            let normalized = if let Some(version) = arxiv_id.version {
                format!("{}v{version}", arxiv_id.id)
            } else {
                arxiv_id.id.clone()
            };
            return Some(CitationAddTarget::Arxiv(normalized));
        }
        None
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<CitationGraphPanelAction> {
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.move_down();
                None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_up();
                None
            }
            KeyCode::Tab => {
                self.cycle_mode();
                None
            }
            KeyCode::BackTab => {
                self.cycle_mode_back();
                None
            }
            KeyCode::Char('1') if key.modifiers == KeyModifiers::NONE => {
                self.set_mode(GraphMode::References);
                None
            }
            KeyCode::Char('2') if key.modifiers == KeyModifiers::NONE => {
                self.set_mode(GraphMode::CitedBy);
                None
            }
            KeyCode::Char('3') if key.modifiers == KeyModifiers::NONE => {
                self.set_mode(GraphMode::Related);
                None
            }
            KeyCode::Enter => {
                let edge = self.selected_edge()?;
                edge.source_id.map(CitationGraphPanelAction::OpenBook)
            }
            KeyCode::Char('a') if key.modifiers == KeyModifiers::NONE => {
                let edge_index = self.selected_edge_index()?;
                Some(CitationGraphPanelAction::AddToLibrary {
                    mode: self.mode,
                    edge_index,
                    target: self.add_target_for(edge_index),
                })
            }
            KeyCode::Char('f') if key.modifiers == KeyModifiers::NONE => {
                let edge_index = self.selected_edge_index()?;
                Some(CitationGraphPanelAction::FindOnline {
                    mode: self.mode,
                    edge_index,
                })
            }
            _ => None,
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, theme: &NordTheme) {
        if area.is_empty() {
            return;
        }

        let title = format!(" CITATION GRAPH — \"{}\" ", self.book.metadata.title);
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

        self.sync_cursor();

        let sections = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(2),
                Constraint::Length(1),
            ])
            .split(inner);

        frame.render_widget(
            Paragraph::new(render_mode_line(self.mode, theme)),
            sections[0],
        );

        let lines = self.build_tree_lines(usize::from(sections[1].width), theme);
        let body_height = usize::from(sections[1].height);
        let selected_edge = self.selected_edge_index();
        let selected_line = selected_edge.and_then(|edge_idx| {
            lines
                .iter()
                .position(|(_, row_edge_idx)| *row_edge_idx == Some(edge_idx))
        });
        let start = viewport_start(lines.len(), body_height, selected_line);
        let visible_lines = lines
            .into_iter()
            .skip(start)
            .take(body_height)
            .map(|(line, _)| line)
            .collect::<Vec<_>>();

        frame.render_widget(Paragraph::new(visible_lines), sections[1]);
        frame.render_widget(footer_hint(theme), sections[2]);
    }

    fn active_edges(&self) -> &[CitationEdge] {
        match self.mode {
            GraphMode::References => &self.references,
            GraphMode::CitedBy => &self.cited_by,
            GraphMode::Related => &self.related,
        }
    }

    fn active_len(&self) -> usize {
        self.active_edges().len()
    }

    fn sync_cursor(&mut self) {
        let len = self.active_len();
        if len == 0 {
            self.cursor = 0;
            return;
        }

        if self.cursor >= len {
            self.cursor = len - 1;
        }
    }

    fn build_tree_lines(
        &self,
        max_width: usize,
        theme: &NordTheme,
    ) -> Vec<(Line<'static>, Option<usize>)> {
        let mut lines = Vec::new();

        let root_text = root_node_text(&self.book);
        lines.push((
            styled_line(
                root_text,
                max_width,
                Style::default()
                    .fg(theme.frost_ice())
                    .add_modifier(Modifier::BOLD),
            ),
            None,
        ));

        lines.push((
            styled_line(String::new(), max_width, Style::default()),
            None,
        ));

        let selected_edge = self.selected_edge_index();
        let edges = self.active_edges();
        let edge_count = edges.len();

        match self.mode {
            GraphMode::References => {
                let references_total = self
                    .book
                    .citation_graph
                    .reference_count
                    .max(u32::try_from(edge_count).unwrap_or(u32::MAX));
                lines.push((
                    styled_line(
                        format!("├── cites ({references_total})"),
                        max_width,
                        Style::default().fg(theme.muted()),
                    ),
                    None,
                ));

                if edge_count == 0 {
                    let empty_text = if references_total > 0 {
                        format!("│   └── ({references_total} total, sample unavailable)")
                    } else {
                        "│   └── (no references)".to_string()
                    };
                    lines.push((
                        styled_line(
                            empty_text,
                            max_width,
                            Style::default()
                                .fg(theme.muted())
                                .add_modifier(Modifier::DIM),
                        ),
                        None,
                    ));
                } else {
                    for (idx, edge) in edges.iter().enumerate() {
                        let prefix = if idx + 1 == edge_count {
                            "│   └── "
                        } else {
                            "│   ├── "
                        };
                        let line = edge_tree_text(prefix, edge, max_width);
                        let style = edge_line_style(edge, selected_edge == Some(idx), theme);
                        lines.push((styled_line(line, max_width, style), Some(idx)));
                    }
                }
            }
            GraphMode::CitedBy => {
                let cited_by_count = self
                    .book
                    .citation_graph
                    .citation_count
                    .max(u32::try_from(edge_count).unwrap_or(u32::MAX));
                lines.push((
                    styled_line(
                        format!("└── cited by ({cited_by_count} papers)"),
                        max_width,
                        Style::default().fg(theme.muted()),
                    ),
                    None,
                ));

                if edge_count == 0 {
                    let empty_text = if cited_by_count > 0 {
                        format!("    └── ({cited_by_count} total, sample unavailable)")
                    } else {
                        "    └── (no citations found)".to_string()
                    };
                    lines.push((
                        styled_line(
                            empty_text,
                            max_width,
                            Style::default()
                                .fg(theme.muted())
                                .add_modifier(Modifier::DIM),
                        ),
                        None,
                    ));
                } else {
                    for (idx, edge) in edges.iter().enumerate() {
                        let prefix = if idx + 1 == edge_count {
                            "    └── "
                        } else {
                            "    ├── "
                        };
                        let line = edge_tree_text(prefix, edge, max_width);
                        let style = edge_line_style(edge, selected_edge == Some(idx), theme);
                        lines.push((styled_line(line, max_width, style), Some(idx)));
                    }
                }
            }
            GraphMode::Related => {
                lines.push((
                    styled_line(
                        "Related (bibliographic coupling):".to_string(),
                        max_width,
                        Style::default().fg(theme.muted()),
                    ),
                    None,
                ));

                if edge_count == 0 {
                    lines.push((
                        styled_line(
                            "    └── (no related papers)".to_string(),
                            max_width,
                            Style::default()
                                .fg(theme.muted())
                                .add_modifier(Modifier::DIM),
                        ),
                        None,
                    ));
                } else {
                    for (idx, edge) in edges.iter().enumerate() {
                        let prefix = if idx + 1 == edge_count {
                            "    └── "
                        } else {
                            "    ├── "
                        };
                        let line = edge_tree_text(prefix, edge, max_width);
                        let style = edge_line_style(edge, selected_edge == Some(idx), theme);
                        lines.push((styled_line(line, max_width, style), Some(idx)));
                    }
                }
            }
        }

        lines
    }
}

fn root_node_text(book: &BookCard) -> String {
    if let Some(year) = book.metadata.year {
        format!("◉ {} ({year})", book.metadata.title)
    } else {
        format!("◉ {}", book.metadata.title)
    }
}

fn edge_tree_text(prefix: &str, edge: &CitationEdge, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }

    let status = if edge.source_id.is_some() {
        "✓"
    } else {
        "✗"
    };
    let suffix = format!(" [{}]", edge_id_label(edge));
    let base_prefix = format!("{prefix}[{status}] ");
    let title = edge_summary(edge);

    let reserved = base_prefix.chars().count() + suffix.chars().count();
    let title_width = max_width.saturating_sub(reserved);
    let title = truncate_text(&title, title_width);
    format!("{base_prefix}{title}{suffix}")
}

fn edge_summary(edge: &CitationEdge) -> String {
    let title = edge.title.trim();
    match (edge.authors.first(), edge.year) {
        (Some(author), Some(year)) => format!("{title} ({author} {year})"),
        (Some(author), None) => format!("{title} ({author})"),
        (None, Some(year)) => format!("{title} ({year})"),
        (None, None) => title.to_string(),
    }
}

fn edge_id_label(edge: &CitationEdge) -> &'static str {
    if edge.arxiv_id.is_some() {
        "arXiv"
    } else if edge.doi.is_some() {
        "DOI"
    } else {
        "OpenAI"
    }
}

fn edge_line_style(edge: &CitationEdge, is_selected: bool, theme: &NordTheme) -> Style {
    let mut style = if edge.source_id.is_some() {
        Style::default().fg(theme.green())
    } else {
        Style::default().fg(theme.fg())
    };

    if is_selected {
        style = style.bg(theme.bg_secondary()).add_modifier(Modifier::BOLD);
    }

    style
}

fn styled_line(text: String, max_width: usize, style: Style) -> Line<'static> {
    Line::from(vec![Span::styled(truncate_text(&text, max_width), style)])
}

fn truncate_text(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }

    if s.chars().count() <= max {
        return s.to_string();
    }

    let truncated: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{truncated}…")
}

fn viewport_start(total: usize, view: usize, selected_line: Option<usize>) -> usize {
    if view == 0 || total <= view {
        return 0;
    }

    let selected_line = selected_line.unwrap_or(0);
    let mut start = selected_line.saturating_sub(view / 2);
    let max_start = total.saturating_sub(view);
    if start > max_start {
        start = max_start;
    }
    start
}

fn render_mode_line(mode: GraphMode, theme: &NordTheme) -> Line<'static> {
    let mut spans = vec![Span::styled(
        "Mode:",
        Style::default()
            .fg(theme.muted())
            .add_modifier(Modifier::DIM),
    )];

    for candidate in GraphMode::ORDER {
        let number = match candidate {
            GraphMode::References => "1",
            GraphMode::CitedBy => "2",
            GraphMode::Related => "3",
        };

        let text = format!(" [{number} {}] ", candidate.label());
        if candidate == mode {
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
            "[Tab]",
            Style::default()
                .fg(theme.yellow())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " mode  ",
            Style::default()
                .fg(theme.muted())
                .add_modifier(Modifier::DIM),
        ),
        Span::styled(
            "[Enter]",
            Style::default()
                .fg(theme.yellow())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " open  ",
            Style::default()
                .fg(theme.muted())
                .add_modifier(Modifier::DIM),
        ),
        Span::styled(
            "[a]",
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
            "[f]",
            Style::default()
                .fg(theme.yellow())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " find PDF",
            Style::default()
                .fg(theme.muted())
                .add_modifier(Modifier::DIM),
        ),
    ]);
    Paragraph::new(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn book() -> BookCard {
        let mut book = BookCard::new("Attention Is All You Need");
        book.metadata.year = Some(2017);
        book.citation_graph.citation_count = 87_654;
        book
    }

    fn edge(
        title: &str,
        author: &str,
        year: i32,
        in_library: bool,
        doi: Option<&str>,
        arxiv: Option<&str>,
    ) -> CitationEdge {
        CitationEdge {
            source_id: in_library.then(Uuid::new_v4),
            doi: doi.map(|value| Doi::parse(value).expect("valid doi")),
            arxiv_id: arxiv.map(|value| ArxivId::parse(value).expect("valid arxiv id")),
            title: title.to_string(),
            authors: vec![author.to_string()],
            year: Some(year),
            citation_context: None,
            is_influential: false,
        }
    }

    fn line_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>()
    }

    #[test]
    fn tab_and_numeric_keys_switch_modes() {
        let mut panel = CitationGraphPanel::new(book(), Vec::new(), Vec::new(), Vec::new());
        assert_eq!(panel.mode, GraphMode::References);

        panel.handle_key(key(KeyCode::Tab));
        assert_eq!(panel.mode, GraphMode::CitedBy);

        panel.handle_key(key(KeyCode::Tab));
        assert_eq!(panel.mode, GraphMode::Related);

        panel.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE));
        assert_eq!(panel.mode, GraphMode::CitedBy);

        panel.handle_key(key(KeyCode::Char('1')));
        assert_eq!(panel.mode, GraphMode::References);

        panel.handle_key(key(KeyCode::Char('3')));
        assert_eq!(panel.mode, GraphMode::Related);
    }

    #[test]
    fn enter_opens_only_items_already_in_library() {
        let in_library = edge("BERT", "Devlin", 2018, true, None, Some("1810.04805"));
        let external = edge("GPT", "Radford", 2018, false, None, None);
        let mut panel =
            CitationGraphPanel::new(book(), vec![in_library, external], Vec::new(), Vec::new());

        let action = panel.handle_key(key(KeyCode::Enter));
        assert!(matches!(
            action,
            Some(CitationGraphPanelAction::OpenBook(_))
        ));

        panel.move_down();
        let action = panel.handle_key(key(KeyCode::Enter));
        assert_eq!(action, None);
    }

    #[test]
    fn add_and_find_actions_include_mode_and_targets() {
        let doi_edge = edge(
            "LSTM",
            "Hochreiter",
            1997,
            false,
            Some("10.1162/neco.1997.9.8.1735"),
            None,
        );
        let mut panel = CitationGraphPanel::new(book(), vec![doi_edge], Vec::new(), Vec::new());

        assert_eq!(
            panel.handle_key(key(KeyCode::Char('a'))),
            Some(CitationGraphPanelAction::AddToLibrary {
                mode: GraphMode::References,
                edge_index: 0,
                target: Some(CitationAddTarget::Doi(
                    "10.1162/neco.1997.9.8.1735".to_string()
                )),
            })
        );

        assert_eq!(
            panel.handle_key(key(KeyCode::Char('f'))),
            Some(CitationGraphPanelAction::FindOnline {
                mode: GraphMode::References,
                edge_index: 0
            })
        );
    }

    #[test]
    fn graph_lines_include_ascii_tree_indicators_and_suffixes() {
        let references = vec![
            edge("Bahdanau", "Bahdanau", 2014, true, None, Some("1409.0473")),
            edge(
                "Hochreiter",
                "Hochreiter",
                1997,
                false,
                Some("10.1162/neco.1997.9.8.1735"),
                None,
            ),
        ];
        let panel = CitationGraphPanel::new(book(), references, Vec::new(), Vec::new());
        let theme = NordTheme::default();

        let lines = panel
            .build_tree_lines(140, &theme)
            .into_iter()
            .map(|(line, _)| line_text(&line))
            .collect::<Vec<_>>();

        assert!(
            lines
                .iter()
                .any(|line| line.contains("◉ Attention Is All You Need (2017)"))
        );
        assert!(lines.iter().any(|line| line.contains("├── cites")));
        assert!(lines.iter().any(|line| line.contains("│   ├── [✓]")));
        assert!(lines.iter().any(|line| line.contains("│   └── [✗]")));
        assert!(lines.iter().any(|line| line.contains("[arXiv]")));
        assert!(lines.iter().any(|line| line.contains("[DOI]")));
    }

    #[test]
    fn openai_suffix_is_used_when_no_arxiv_or_doi() {
        let related = vec![edge("GPT", "Radford", 2018, false, None, None)];
        let mut panel = CitationGraphPanel::new(book(), Vec::new(), Vec::new(), related);
        panel.set_mode(GraphMode::Related);

        let theme = NordTheme::default();
        let lines = panel
            .build_tree_lines(120, &theme)
            .into_iter()
            .map(|(line, _)| line_text(&line))
            .collect::<Vec<_>>();

        assert!(lines.iter().any(|line| line.contains("[OpenAI]")));
    }

    #[test]
    fn cursor_is_clamped_when_switching_to_shorter_mode() {
        let references = vec![
            edge("A", "Author", 2020, false, None, Some("2001.00001")),
            edge("B", "Author", 2021, false, None, Some("2001.00002")),
            edge("C", "Author", 2022, false, None, Some("2001.00003")),
        ];
        let related = vec![edge("R", "Author", 2019, false, None, None)];
        let mut panel = CitationGraphPanel::new(book(), references, Vec::new(), related);
        panel.cursor = 2;
        panel.set_mode(GraphMode::Related);

        assert_eq!(panel.cursor, 0);
        assert_eq!(panel.selected_edge_index(), Some(0));
    }
}
