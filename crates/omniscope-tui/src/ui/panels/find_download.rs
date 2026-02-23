use crate::app::{App, find_download::{SearchColumn, SearchResultItem}};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let state = match app.active_overlay.as_ref() {
        Some(crate::app::OverlayState::FindDownload(s)) => s,
        _ => return,
    };

    let block = Block::default()
        .title(" FIND & DOWNLOAD ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.active_panel()))
        .style(Style::default().bg(app.theme.bg()));

    let inner_area = block.inner(area);

    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Header
            Constraint::Length(1), // Spacer
            Constraint::Min(1),    // Columns
        ])
        .split(inner_area);

    frame.render_widget(block, area);

    // Render Header (Search String & Indicators)
    let header_line = Line::from(vec![
        Span::styled(" Query: ", Style::default().fg(app.theme.muted())),
        Span::styled(format!("{} ", state.query), Style::default().add_modifier(Modifier::BOLD)),
        Span::styled("[A✓] [S✓] [O✓] [G✓]", Style::default().fg(app.theme.green())), // Mocked statuses
    ]);
    frame.render_widget(Paragraph::new(header_line), main_layout[0]);

    // Render Columns
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(main_layout[2]);

    render_column(
        frame,
        app,
        columns[0],
        "Anna's Archive & Sci-Hub",
        &state.left_results,
        state.left_cursor,
        state.active_column == SearchColumn::Left,
        state.left_loading,
    );

    render_column(
        frame,
        app,
        columns[1],
        "Semantic Scholar & OpenAlex",
        &state.right_results,
        state.right_cursor,
        state.active_column == SearchColumn::Right,
        state.right_loading,
    );
}

fn render_column(
    frame: &mut Frame,
    app: &App,
    area: Rect,
    title: &str,
    results: &[SearchResultItem],
    cursor: usize,
    is_active: bool,
    is_loading: bool,
) {
    let mut block = Block::default().title(format!(" {} ", title)).borders(Borders::ALL);
    if is_active {
        block = block.border_style(Style::default().fg(app.theme.frost_ice()));
    } else {
        block = block.border_style(Style::default().fg(app.theme.border()));
    }

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    if is_loading {
        let text = Paragraph::new("Loading...").style(Style::default().fg(app.theme.muted()));
        frame.render_widget(text, inner_area);
        return;
    }

    if results.is_empty() {
        let text = Paragraph::new("No results found.").style(Style::default().fg(app.theme.muted()));
        frame.render_widget(text, inner_area);
        return;
    }

    let visible_height = inner_area.height as usize;
    let scroll_offset = if cursor >= visible_height {
        cursor.saturating_sub(visible_height - 1)
    } else {
        0
    };

    let mut items = Vec::new();
    for (i, res) in results.iter().enumerate().skip(scroll_offset).take(visible_height) {
        let is_selected = is_active && i == cursor;
        let mut style = Style::default().fg(app.theme.fg());
        if is_selected {
            style = style.bg(app.theme.bg_secondary()).fg(app.theme.fg_bright());
        }

        let year_str = res.year.map(|y| format!(" ({})", y)).unwrap_or_default();
        let title_line = Line::from(vec![
            Span::styled(super::super::truncate(&res.title, 50), style.add_modifier(Modifier::BOLD)),
            Span::styled(year_str, style),
        ]);

        let mut bottom_spans = vec![
            Span::styled(super::super::truncate(&res.authors, 30), style),
            Span::styled(" • ", style.fg(app.theme.muted())),
            Span::styled(format!("{} ", res.source), style.fg(app.theme.frost_ice())),
            Span::styled(format!("{} ", res.format_or_metrics), style),
        ];

        if res.in_library {
            bottom_spans.push(Span::styled("✓ In library ", style.fg(app.theme.green())));
        }
        
        let action_span = if res.download_available {
            "[D]ownload [M]eta [↗]open"
        } else {
            "[M]eta [↗]open"
        };
        bottom_spans.push(Span::styled(action_span, style.fg(app.theme.yellow())));

        items.push(ListItem::new(vec![title_line, Line::from(bottom_spans)]));
    }

    // Since each item takes 2 lines, we need to adjust constraints or just rely on List wrapping
    let list = List::new(items);
    frame.render_widget(list, inner_area);
}
