use crate::app::{App, references::RefsFilter};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Row, Table},
    Frame,
};

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let state = match app.active_overlay.as_ref() {
        Some(crate::app::OverlayState::References(s)) => s,
        _ => return,
    };

    let block = Block::default()
        .title(format!(" REFERENCES — \"{}\" [{} references] ", state.book_title, state.references.len()))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.active_panel()))
        .style(Style::default().bg(app.theme.bg()));

    let inner_area = block.inner(area);

    // Layout: Filter row at the top, table below
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(inner_area);

    let filter_area = layout[0];
    let table_area = layout[1];

    frame.render_widget(block, area);

    // Filter drawing
    let mut filter_spans = vec![Span::raw(" Filter: ")];
    let filters = [
        (RefsFilter::All, "all"),
        (RefsFilter::Resolved, "resolved"),
        (RefsFilter::Unresolved, "unresolved"),
        (RefsFilter::InLibrary, "in-library"),
        (RefsFilter::NotInLibrary, "not-in-library"),
    ];

    for (filter, label) in filters {
        let is_active = state.filter == filter;
        let mut style = Style::default().fg(app.theme.muted());
        if is_active {
            style = style.fg(app.theme.frost_ice()).add_modifier(Modifier::BOLD);
        }
        filter_spans.push(Span::styled(format!("[{}] ", label), style));
    }

    if !state.search_query.is_empty() {
        filter_spans.push(Span::styled(
            format!("(Search: '{}') ", state.search_query),
            Style::default().fg(app.theme.yellow()),
        ));
    }

    frame.render_widget(ratatui::widgets::Paragraph::new(Line::from(filter_spans)), filter_area);

    // Table drawing
    let visible_refs = state.visible_references();
    
    let header_style = Style::default().fg(app.theme.border()).add_modifier(Modifier::BOLD);
    let selected_style = Style::default().bg(app.theme.bg_secondary()).fg(app.theme.fg_bright());

    let header = Row::new(vec![
        " # ",
        " Reference ",
        " ID ",
        " In Library ",
    ])
    .style(header_style)
    .bottom_margin(1);

    let inner_height = table_area.height as usize;
    let visible_height = inner_height.saturating_sub(2); // Header takes some space
    let scroll_offset = if state.selected >= visible_height {
        state.selected.saturating_sub(visible_height - 1)
    } else {
        0
    };

    let rows: Vec<Row> = visible_refs
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(visible_height)
        .map(|(i, (original_idx, r))| {
            let actual_idx = i + scroll_offset;
            let is_selected = state.selected == actual_idx;
            
            // Format ID column
            let id_str = if r.arxiv_id.is_some() {
                "arXiv"
            } else if r.doi.is_some() {
                "DOI"
            } else if r.isbn.is_some() {
                "ISBN"
            } else {
                "---"
            };

            // Format In Library column
            let in_library_str = if r.is_in_library.is_some() {
                Span::styled("✓ In library", Style::default().fg(app.theme.green()))
            } else {
                if r.resolution_method == omniscope_science::references::resolver::ResolutionMethod::Unresolved {
                    Span::styled("✗ Not found [A]dd [F]ind", Style::default().fg(app.theme.red()))
                } else {
                    Span::styled("✗ Not found", Style::default().fg(app.theme.red()))
                }
            };

            // Format reference column
            let mut ref_style = Style::default().fg(app.theme.fg());
            if r.confidence < 0.7 {
                ref_style = ref_style.add_modifier(Modifier::DIM);
            }
            if is_selected {
                ref_style = ref_style.fg(app.theme.fg_bright());
            }
            
            let mut ref_title = r.raw_text.clone();
            if let Some(rt) = &r.resolved_title {
                let authors = if !r.resolved_authors.is_empty() {
                    let mut a = r.resolved_authors[0].clone();
                    if r.resolved_authors.len() > 1 {
                        a.push_str(" et al.");
                    }
                    if let Some(y) = r.resolved_year {
                        a.push_str(&format!(", {}", y));
                    }
                    format!("{} — ", a)
                } else {
                    "".to_string()
                };
                ref_title = format!("{}{}", authors, rt);
            }
            
            let ref_cell = if !state.search_query.is_empty() {
                highlight_search(ref_title, &state.search_query, app.theme.yellow(), ref_style)
            } else {
                Line::from(vec![Span::styled(super::super::truncate(&ref_title, 80), ref_style)])
            };

            Row::new(vec![
                ratatui::widgets::Cell::from(format!(" {:2} ", original_idx + 1)),
                ratatui::widgets::Cell::from(ref_cell),
                ratatui::widgets::Cell::from(format!(" {} ", id_str)),
                ratatui::widgets::Cell::from(Line::from(vec![Span::raw(" "), in_library_str])),
            ])
            .style(if is_selected {
                selected_style
            } else {
                Style::default()
            })
        })
        .collect();

    let widths = [
        Constraint::Length(5),
        Constraint::Percentage(60),
        Constraint::Length(10),
        Constraint::Length(30),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .column_spacing(1);

    frame.render_widget(table, table_area);
}

fn highlight_search(text: String, query: &str, highlight_color: ratatui::style::Color, base_style: Style) -> Line<'static> {
    let lower_text = text.to_lowercase();
    let lower_query = query.to_lowercase();
    
    if let Some(idx) = lower_text.find(&lower_query) {
        let (left, rest) = text.split_at(idx);
        let (middle, right) = rest.split_at(query.len());
        
        Line::from(vec![
            Span::styled(left.to_string(), base_style),
            Span::styled(middle.to_string(), base_style.fg(highlight_color).add_modifier(Modifier::BOLD)),
            Span::styled(right.to_string(), base_style),
        ])
    } else {
        Line::from(vec![Span::styled(super::super::truncate(&text, 80), base_style)])
    }
}
