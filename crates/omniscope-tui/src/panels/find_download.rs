use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::theme::NordTheme;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FindColumn {
    #[default]
    Left,
    Right,
}

impl FindColumn {
    pub fn toggle(self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchIdentifierKind {
    #[default]
    Doi,
    Arxiv,
    Isbn,
    Pmid,
}

impl SearchIdentifierKind {
    const ORDER: [Self; 4] = [Self::Doi, Self::Arxiv, Self::Isbn, Self::Pmid];

    pub fn next(self) -> Self {
        let idx = Self::ORDER.iter().position(|candidate| *candidate == self);
        let Some(idx) = idx else {
            return Self::Doi;
        };
        Self::ORDER[(idx + 1) % Self::ORDER.len()]
    }

    pub fn previous(self) -> Self {
        let idx = Self::ORDER.iter().position(|candidate| *candidate == self);
        let Some(idx) = idx else {
            return Self::Doi;
        };
        if idx == 0 {
            Self::ORDER[Self::ORDER.len() - 1]
        } else {
            Self::ORDER[idx - 1]
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Doi => "DOI",
            Self::Arxiv => "arXiv",
            Self::Isbn => "ISBN",
            Self::Pmid => "PMID",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindSource {
    AnnaArchive,
    SciHub,
    OpenAlex,
    SemanticGraph,
}

impl FindSource {
    fn section_title(self, count: usize) -> String {
        match self {
            Self::AnnaArchive => format!("Anna's Archive ({count})"),
            Self::SciHub => format!("Sci-Hub ({count})"),
            Self::SemanticGraph => format!("Semantic Scholar ({count})"),
            Self::OpenAlex => format!("OpenAlex ({count})"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceAvailability {
    pub anna: bool,
    pub sci_hub: bool,
    pub open_alex: bool,
    pub semantic_graph: bool,
}

impl Default for SourceAvailability {
    fn default() -> Self {
        Self {
            anna: true,
            sci_hub: true,
            open_alex: true,
            semantic_graph: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FindResult {
    pub title: String,
    pub authors: Vec<String>,
    pub year: Option<i32>,
    pub primary_id: Option<String>,
    pub file_format: Option<String>,
    pub file_size: Option<String>,
    pub citation_count: Option<u32>,
    pub in_library: bool,
    pub open_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SelectedResultRef {
    source: FindSource,
    result_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FindDownloadPanelAction {
    Download {
        source: FindSource,
        result_index: usize,
    },
    ImportMetadata {
        source: FindSource,
        result_index: usize,
    },
    OpenInBrowser {
        source: FindSource,
        result_index: usize,
    },
    Close,
}

#[derive(Debug, Clone)]
pub struct FindDownloadPanel {
    pub query: String,
    pub identifier_kind: SearchIdentifierKind,
    pub availability: SourceAvailability,
    pub anna_results: Vec<FindResult>,
    pub sci_hub_results: Vec<FindResult>,
    pub semantic_scholar_results: Vec<FindResult>,
    pub open_alex_results: Vec<FindResult>,
    pub focus: FindColumn,
    pub left_cursor: usize,
    pub right_cursor: usize,
}

impl FindDownloadPanel {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            identifier_kind: SearchIdentifierKind::Doi,
            availability: SourceAvailability::default(),
            anna_results: Vec::new(),
            sci_hub_results: Vec::new(),
            semantic_scholar_results: Vec::new(),
            open_alex_results: Vec::new(),
            focus: FindColumn::Left,
            left_cursor: 0,
            right_cursor: 0,
        }
    }

    pub fn set_identifier_kind(&mut self, kind: SearchIdentifierKind) {
        self.identifier_kind = kind;
    }

    pub fn cycle_identifier_next(&mut self) {
        self.identifier_kind = self.identifier_kind.next();
    }

    pub fn cycle_identifier_previous(&mut self) {
        self.identifier_kind = self.identifier_kind.previous();
    }

    pub fn toggle_focus(&mut self) {
        self.focus = self.focus.toggle();
        self.sync_cursors();
    }

    pub fn move_down(&mut self) {
        let len = self.focused_len();
        if len == 0 {
            *self.focused_cursor_mut() = 0;
            return;
        }

        let cursor = self.focused_cursor_mut();
        if *cursor + 1 < len {
            *cursor += 1;
        }
    }

    pub fn move_up(&mut self) {
        if self.focused_len() == 0 {
            *self.focused_cursor_mut() = 0;
            return;
        }

        let cursor = self.focused_cursor_mut();
        *cursor = cursor.saturating_sub(1);
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<FindDownloadPanelAction> {
        match key.code {
            KeyCode::Tab | KeyCode::BackTab => {
                self.toggle_focus();
                None
            }
            KeyCode::Left | KeyCode::Char('h') if key.modifiers == KeyModifiers::NONE => {
                self.focus = FindColumn::Left;
                self.sync_cursors();
                None
            }
            KeyCode::Right | KeyCode::Char('l') if key.modifiers == KeyModifiers::NONE => {
                self.focus = FindColumn::Right;
                self.sync_cursors();
                None
            }
            KeyCode::Down | KeyCode::Char('j') if key.modifiers == KeyModifiers::NONE => {
                self.move_down();
                None
            }
            KeyCode::Up | KeyCode::Char('k') if key.modifiers == KeyModifiers::NONE => {
                self.move_up();
                None
            }
            KeyCode::Char('1') if key.modifiers == KeyModifiers::NONE => {
                self.identifier_kind = SearchIdentifierKind::Doi;
                None
            }
            KeyCode::Char('2') if key.modifiers == KeyModifiers::NONE => {
                self.identifier_kind = SearchIdentifierKind::Arxiv;
                None
            }
            KeyCode::Char('3') if key.modifiers == KeyModifiers::NONE => {
                self.identifier_kind = SearchIdentifierKind::Isbn;
                None
            }
            KeyCode::Char('4') if key.modifiers == KeyModifiers::NONE => {
                self.identifier_kind = SearchIdentifierKind::Pmid;
                None
            }
            KeyCode::Char(']') if key.modifiers == KeyModifiers::NONE => {
                self.cycle_identifier_next();
                None
            }
            KeyCode::Char('[') if key.modifiers == KeyModifiers::NONE => {
                self.cycle_identifier_previous();
                None
            }
            KeyCode::Char('D') | KeyCode::Char('d') if plain_or_shift(key.modifiers) => {
                let selected = self.selected_result_ref()?;
                Some(FindDownloadPanelAction::Download {
                    source: selected.source,
                    result_index: selected.result_index,
                })
            }
            KeyCode::Char('M') | KeyCode::Char('m') if plain_or_shift(key.modifiers) => {
                let selected = self.selected_result_ref()?;
                Some(FindDownloadPanelAction::ImportMetadata {
                    source: selected.source,
                    result_index: selected.result_index,
                })
            }
            KeyCode::Char('↗') if plain_or_shift(key.modifiers) => {
                let selected = self.selected_result_ref()?;
                Some(FindDownloadPanelAction::OpenInBrowser {
                    source: selected.source,
                    result_index: selected.result_index,
                })
            }
            KeyCode::Char('o') if key.modifiers == KeyModifiers::NONE => {
                let selected = self.selected_result_ref()?;
                Some(FindDownloadPanelAction::OpenInBrowser {
                    source: selected.source,
                    result_index: selected.result_index,
                })
            }
            KeyCode::Esc => Some(FindDownloadPanelAction::Close),
            _ => None,
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, theme: &NordTheme) {
        if area.is_empty() {
            return;
        }

        let block = Block::default()
            .title(" 🌐 FIND & DOWNLOAD ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.active_panel()))
            .style(Style::default().bg(theme.bg()));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.height < 8 || inner.width < 40 {
            return;
        }

        self.sync_cursors();

        let sections = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(3),
                Constraint::Length(1),
            ])
            .split(inner);

        frame.render_widget(
            Paragraph::new(render_sources_line(self.availability, theme)),
            sections[0],
        );
        frame.render_widget(
            Paragraph::new(render_query_line(
                &self.query,
                usize::from(sections[1].width),
                theme,
            )),
            sections[1],
        );
        frame.render_widget(
            Paragraph::new(render_identifier_line(self.identifier_kind, theme)),
            sections[2],
        );

        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(sections[3]);

        let left_block = Block::default()
            .borders(Borders::RIGHT)
            .border_style(Style::default().fg(theme.border()))
            .style(Style::default().bg(theme.bg()));
        let left_inner = left_block.inner(columns[0]);
        frame.render_widget(left_block, columns[0]);

        let right_block = Block::default().style(Style::default().bg(theme.bg()));
        let right_inner = right_block.inner(columns[1]);
        frame.render_widget(right_block, columns[1]);

        let left_lines = self.build_left_column_lines(usize::from(left_inner.width), theme);
        let right_lines = self.build_right_column_lines(usize::from(right_inner.width), theme);

        frame.render_widget(Paragraph::new(left_lines), left_inner);
        frame.render_widget(Paragraph::new(right_lines), right_inner);

        frame.render_widget(footer_hint(theme), sections[4]);
    }

    fn build_left_column_lines(&self, max_width: usize, theme: &NordTheme) -> Vec<Line<'static>> {
        let selected = if self.focus == FindColumn::Left {
            self.selected_result_ref()
        } else {
            None
        };

        let mut lines = Vec::new();
        self.append_source_group(
            &mut lines,
            FindSource::AnnaArchive,
            &self.anna_results,
            max_width,
            selected,
            theme,
        );
        if !lines.is_empty() {
            lines.push(styled_line(String::new(), Style::default(), max_width));
        }
        self.append_source_group(
            &mut lines,
            FindSource::SciHub,
            &self.sci_hub_results,
            max_width,
            selected,
            theme,
        );
        lines
    }

    fn build_right_column_lines(&self, max_width: usize, theme: &NordTheme) -> Vec<Line<'static>> {
        let selected = if self.focus == FindColumn::Right {
            self.selected_result_ref()
        } else {
            None
        };

        let mut lines = Vec::new();
        self.append_source_group(
            &mut lines,
            FindSource::SemanticGraph,
            &self.semantic_scholar_results,
            max_width,
            selected,
            theme,
        );
        if !lines.is_empty() {
            lines.push(styled_line(String::new(), Style::default(), max_width));
        }
        self.append_source_group(
            &mut lines,
            FindSource::OpenAlex,
            &self.open_alex_results,
            max_width,
            selected,
            theme,
        );
        lines
    }

    fn append_source_group(
        &self,
        lines: &mut Vec<Line<'static>>,
        source: FindSource,
        results: &[FindResult],
        max_width: usize,
        selected: Option<SelectedResultRef>,
        theme: &NordTheme,
    ) {
        lines.push(styled_line(
            source.section_title(results.len()),
            Style::default()
                .fg(theme.frost_ice())
                .add_modifier(Modifier::BOLD),
            max_width,
        ));

        if results.is_empty() {
            lines.push(styled_line(
                "  (no results)".to_string(),
                Style::default()
                    .fg(theme.muted())
                    .add_modifier(Modifier::DIM),
                max_width,
            ));
            return;
        }

        for (idx, result) in results.iter().enumerate() {
            let is_selected = selected
                .map(|entry| entry.source == source && entry.result_index == idx)
                .unwrap_or(false);
            let line_style = result_line_style(is_selected, theme);

            let pointer = if is_selected { "▶ " } else { "  " };
            let title = format!("{pointer}{}", truncate_text(&result.title, max_width));
            lines.push(styled_line(title, line_style, max_width));

            let mut meta = String::from("  ");
            meta.push_str(&author_year_line(result));
            if result.in_library {
                meta.push_str("  ✓ In library");
            }
            lines.push(styled_line(meta, line_style, max_width));

            let details = format!("  {}", details_line(source, result));
            lines.push(styled_line(details, line_style, max_width));

            let action_style = line_style.fg(theme.yellow()).add_modifier(Modifier::BOLD);
            lines.push(styled_line(
                "  [D]ownload [M]eta [↗]open".to_string(),
                action_style,
                max_width,
            ));

            lines.push(styled_line(String::new(), line_style, max_width));
        }
    }

    fn focused_len(&self) -> usize {
        match self.focus {
            FindColumn::Left => self.anna_results.len() + self.sci_hub_results.len(),
            FindColumn::Right => self.semantic_scholar_results.len() + self.open_alex_results.len(),
        }
    }

    fn focused_cursor(&self) -> usize {
        match self.focus {
            FindColumn::Left => self.left_cursor,
            FindColumn::Right => self.right_cursor,
        }
    }

    fn focused_cursor_mut(&mut self) -> &mut usize {
        match self.focus {
            FindColumn::Left => &mut self.left_cursor,
            FindColumn::Right => &mut self.right_cursor,
        }
    }

    fn sync_cursors(&mut self) {
        let left_len = self.anna_results.len() + self.sci_hub_results.len();
        if left_len == 0 {
            self.left_cursor = 0;
        } else if self.left_cursor >= left_len {
            self.left_cursor = left_len - 1;
        }

        let right_len = self.semantic_scholar_results.len() + self.open_alex_results.len();
        if right_len == 0 {
            self.right_cursor = 0;
        } else if self.right_cursor >= right_len {
            self.right_cursor = right_len - 1;
        }
    }

    fn selected_result_ref(&self) -> Option<SelectedResultRef> {
        match self.focus {
            FindColumn::Left => {
                let anna_len = self.anna_results.len();
                let sci_len = self.sci_hub_results.len();
                let total = anna_len + sci_len;
                if total == 0 {
                    return None;
                }

                let cursor = self.focused_cursor().min(total.saturating_sub(1));
                if cursor < anna_len {
                    Some(SelectedResultRef {
                        source: FindSource::AnnaArchive,
                        result_index: cursor,
                    })
                } else {
                    Some(SelectedResultRef {
                        source: FindSource::SciHub,
                        result_index: cursor - anna_len,
                    })
                }
            }
            FindColumn::Right => {
                let semantic_len = self.semantic_scholar_results.len();
                let open_alex_len = self.open_alex_results.len();
                let total = semantic_len + open_alex_len;
                if total == 0 {
                    return None;
                }

                let cursor = self.focused_cursor().min(total.saturating_sub(1));
                if cursor < semantic_len {
                    Some(SelectedResultRef {
                        source: FindSource::SemanticGraph,
                        result_index: cursor,
                    })
                } else {
                    Some(SelectedResultRef {
                        source: FindSource::OpenAlex,
                        result_index: cursor - semantic_len,
                    })
                }
            }
        }
    }
}

fn plain_or_shift(modifiers: KeyModifiers) -> bool {
    modifiers == KeyModifiers::NONE || modifiers == KeyModifiers::SHIFT
}

fn result_line_style(is_selected: bool, theme: &NordTheme) -> Style {
    let mut style = Style::default().fg(theme.fg());
    if is_selected {
        style = style.bg(theme.bg_secondary()).add_modifier(Modifier::BOLD);
    }
    style
}

fn render_sources_line(availability: SourceAvailability, theme: &NordTheme) -> Line<'static> {
    let mut spans = vec![Span::styled(
        "Sources:",
        Style::default()
            .fg(theme.muted())
            .add_modifier(Modifier::DIM),
    )];

    spans.push(source_status_span("A", availability.anna, theme));
    spans.push(Span::raw(" "));
    spans.push(source_status_span("S", availability.sci_hub, theme));
    spans.push(Span::raw(" "));
    spans.push(source_status_span("O", availability.open_alex, theme));
    spans.push(Span::raw(" "));
    spans.push(source_status_span("G", availability.semantic_graph, theme));

    Line::from(spans)
}

fn source_status_span(label: &str, available: bool, theme: &NordTheme) -> Span<'static> {
    let marker = if available { "✓" } else { "✗" };
    let color = if available {
        theme.green()
    } else {
        theme.red()
    };
    Span::styled(
        format!("[{label}{marker}]"),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    )
}

fn render_query_line(query: &str, max_width: usize, theme: &NordTheme) -> Line<'static> {
    let text = format!("> {}", query.trim());
    styled_line(
        text,
        Style::default().fg(theme.fg_bright()),
        max_width.max(1),
    )
}

fn render_identifier_line(kind: SearchIdentifierKind, theme: &NordTheme) -> Line<'static> {
    let mut spans = vec![Span::styled(
        "or search by:",
        Style::default()
            .fg(theme.muted())
            .add_modifier(Modifier::DIM),
    )];

    for candidate in SearchIdentifierKind::ORDER {
        let text = format!(" [{}] ", candidate.label());
        if candidate == kind {
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

fn author_year_line(result: &FindResult) -> String {
    let author = result
        .authors
        .first()
        .map(|value| value.as_str())
        .unwrap_or("Unknown");
    match result.year {
        Some(year) => format!("{author} {year}"),
        None => author.to_string(),
    }
}

fn details_line(source: FindSource, result: &FindResult) -> String {
    let mut parts = Vec::new();
    if let Some(primary_id) = &result.primary_id {
        parts.push(primary_id.clone());
    }

    match source {
        FindSource::AnnaArchive => {
            match (result.file_format.as_deref(), result.file_size.as_deref()) {
                (Some(format), Some(size)) => parts.push(format!("{format} {size}")),
                (Some(format), None) => parts.push(format.to_string()),
                (None, Some(size)) => parts.push(size.to_string()),
                (None, None) => {}
            }
        }
        FindSource::SemanticGraph => {
            if let Some(count) = result.citation_count {
                parts.push(format!("{} citations", format_count(count)));
            }
        }
        FindSource::SciHub | FindSource::OpenAlex => {}
    }

    if parts.is_empty() {
        "metadata available".to_string()
    } else {
        parts.join("  ")
    }
}

fn format_count(value: u32) -> String {
    let chars = value.to_string().chars().rev().collect::<Vec<_>>();
    let mut out = String::new();
    for (idx, ch) in chars.iter().enumerate() {
        if idx > 0 && idx % 3 == 0 {
            out.push(',');
        }
        out.push(*ch);
    }
    out.chars().rev().collect()
}

fn styled_line(text: String, style: Style, max_width: usize) -> Line<'static> {
    Line::from(Span::styled(truncate_text(&text, max_width), style))
}

fn truncate_text(text: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    let width = text.chars().count();
    if width <= max_width {
        return text.to_string();
    }
    let truncated: String = text.chars().take(max_width.saturating_sub(1)).collect();
    format!("{truncated}…")
}

fn footer_hint(theme: &NordTheme) -> Paragraph<'static> {
    Paragraph::new(Line::from(vec![
        Span::styled(
            "[Tab]",
            Style::default()
                .fg(theme.yellow())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " switch column  ",
            Style::default()
                .fg(theme.muted())
                .add_modifier(Modifier::DIM),
        ),
        Span::styled(
            "[D]",
            Style::default()
                .fg(theme.yellow())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " download  ",
            Style::default()
                .fg(theme.muted())
                .add_modifier(Modifier::DIM),
        ),
        Span::styled(
            "[M]",
            Style::default()
                .fg(theme.yellow())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " import metadata  ",
            Style::default()
                .fg(theme.muted())
                .add_modifier(Modifier::DIM),
        ),
        Span::styled(
            "[↗]",
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
            "[Esc]",
            Style::default()
                .fg(theme.yellow())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " close",
            Style::default()
                .fg(theme.muted())
                .add_modifier(Modifier::DIM),
        ),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn line_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>()
    }

    fn sample_result(title: &str) -> FindResult {
        FindResult {
            title: title.to_string(),
            authors: vec!["Author".to_string()],
            year: Some(2020),
            primary_id: None,
            file_format: None,
            file_size: None,
            citation_count: None,
            in_library: false,
            open_url: None,
        }
    }

    #[test]
    fn tab_switches_column_focus() {
        let mut panel = FindDownloadPanel::new("attention");
        assert_eq!(panel.focus, FindColumn::Left);

        panel.handle_key(key(KeyCode::Tab));
        assert_eq!(panel.focus, FindColumn::Right);

        panel.handle_key(key(KeyCode::BackTab));
        assert_eq!(panel.focus, FindColumn::Left);
    }

    #[test]
    fn identifier_switches_with_numeric_and_bracket_keys() {
        let mut panel = FindDownloadPanel::new("attention");
        assert_eq!(panel.identifier_kind, SearchIdentifierKind::Doi);

        panel.handle_key(key(KeyCode::Char('2')));
        assert_eq!(panel.identifier_kind, SearchIdentifierKind::Arxiv);

        panel.handle_key(key(KeyCode::Char(']')));
        assert_eq!(panel.identifier_kind, SearchIdentifierKind::Isbn);

        panel.handle_key(key(KeyCode::Char('[')));
        assert_eq!(panel.identifier_kind, SearchIdentifierKind::Arxiv);
    }

    #[test]
    fn actions_use_source_and_index_for_selected_result() {
        let mut panel = FindDownloadPanel::new("attention");
        panel.anna_results = vec![sample_result("A0")];
        panel.sci_hub_results = vec![sample_result("S0")];
        panel.semantic_scholar_results = vec![sample_result("G0")];

        assert_eq!(
            panel.handle_key(KeyEvent::new(KeyCode::Char('D'), KeyModifiers::SHIFT)),
            Some(FindDownloadPanelAction::Download {
                source: FindSource::AnnaArchive,
                result_index: 0,
            })
        );

        panel.move_down();
        assert_eq!(
            panel.handle_key(key(KeyCode::Char('M'))),
            Some(FindDownloadPanelAction::ImportMetadata {
                source: FindSource::SciHub,
                result_index: 0,
            })
        );

        panel.toggle_focus();
        assert_eq!(
            panel.handle_key(key(KeyCode::Char('↗'))),
            Some(FindDownloadPanelAction::OpenInBrowser {
                source: FindSource::SemanticGraph,
                result_index: 0,
            })
        );
    }

    #[test]
    fn esc_emits_close_action() {
        let mut panel = FindDownloadPanel::new("attention");
        assert_eq!(
            panel.handle_key(key(KeyCode::Esc)),
            Some(FindDownloadPanelAction::Close)
        );
    }

    #[test]
    fn source_line_contains_status_badges_with_colors() {
        let theme = NordTheme::default();
        let line = render_sources_line(
            SourceAvailability {
                anna: true,
                sci_hub: false,
                open_alex: true,
                semantic_graph: false,
            },
            &theme,
        );

        let text = line_text(&line);
        assert!(text.contains("[A✓]"));
        assert!(text.contains("[S✗]"));
        assert!(text.contains("[O✓]"));
        assert!(text.contains("[G✗]"));

        let status_spans = line
            .spans
            .iter()
            .filter(|span| span.content.contains('['))
            .collect::<Vec<_>>();
        assert_eq!(
            status_spans.get(0).and_then(|span| span.style.fg),
            Some(theme.green())
        );
        assert_eq!(
            status_spans.get(1).and_then(|span| span.style.fg),
            Some(theme.red())
        );
    }

    #[test]
    fn column_lines_include_required_result_details() {
        let mut panel = FindDownloadPanel::new("attention");

        let mut anna = sample_result("Attention Is All You Need");
        anna.authors = vec!["Vaswani".to_string()];
        anna.year = Some(2017);
        anna.file_format = Some("PDF".to_string());
        anna.file_size = Some("1.2MB".to_string());
        anna.in_library = true;
        panel.anna_results = vec![anna];

        let mut semantic = sample_result("Neural Machine Translation");
        semantic.primary_id = Some("arXiv:1409.0473".to_string());
        semantic.citation_count = Some(87_654);
        panel.semantic_scholar_results = vec![semantic];

        panel.focus = FindColumn::Left;
        let theme = NordTheme::default();
        let left = panel
            .build_left_column_lines(120, &theme)
            .into_iter()
            .map(|line| line_text(&line))
            .collect::<Vec<_>>();
        let right = panel
            .build_right_column_lines(120, &theme)
            .into_iter()
            .map(|line| line_text(&line))
            .collect::<Vec<_>>();

        assert!(left.iter().any(|line| line.contains("Anna's Archive (1)")));
        assert!(left.iter().any(|line| line.contains("PDF 1.2MB")));
        assert!(left.iter().any(|line| line.contains("✓ In library")));
        assert!(left
            .iter()
            .any(|line| line.contains("[D]ownload [M]eta [↗]open")));
        assert!(right
            .iter()
            .any(|line| line.contains("Semantic Scholar (1)")));
        assert!(right.iter().any(|line| line.contains("87,654 citations")));
    }

    #[test]
    fn cursor_is_clamped_when_column_has_fewer_results() {
        let mut panel = FindDownloadPanel::new("attention");
        panel.semantic_scholar_results = vec![
            sample_result("one"),
            sample_result("two"),
            sample_result("three"),
        ];
        panel.right_cursor = 2;
        panel.open_alex_results = Vec::new();

        panel.semantic_scholar_results.truncate(1);
        panel.sync_cursors();

        panel.focus = FindColumn::Right;
        let selected = panel.selected_result_ref();
        assert_eq!(
            selected,
            Some(SelectedResultRef {
                source: FindSource::SemanticGraph,
                result_index: 0
            })
        );
    }
}
