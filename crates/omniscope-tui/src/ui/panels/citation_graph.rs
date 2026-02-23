use crate::app::{App, citation_graph::GraphMode};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
    Frame,
};

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let state = match app.active_overlay.as_ref() {
        Some(crate::app::OverlayState::CitationGraph(s)) => s,
        _ => return,
    };

    let block = Block::default()
        .title(format!(" CITATION GRAPH — \"{}\" ", state.book.metadata.title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.active_panel()))
        .style(Style::default().bg(app.theme.bg()));

    let inner_area = block.inner(area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(inner_area);

    let filter_area = layout[0];
    let list_area = layout[1];

    frame.render_widget(block, area);

    // Filter drawing
    let mut filter_spans = vec![Span::raw(" View: ")];
    let filters = [
        (GraphMode::References, "References"),
        (GraphMode::CitedBy, "Cited By"),
        (GraphMode::Related, "Related"),
    ];

    for (filter, label) in filters {
        let is_active = state.mode == filter;
        let mut style = Style::default().fg(app.theme.muted());
        if is_active {
            style = style.fg(app.theme.frost_ice()).add_modifier(Modifier::BOLD);
        }
        filter_spans.push(Span::styled(format!("[{}] ", label), style));
    }

    frame.render_widget(ratatui::widgets::Paragraph::new(Line::from(filter_spans)), filter_area);

    // Tree rendering
    let visible_items = state.visible_items();
    let mut items: Vec<ListItem> = Vec::new();
    
    // Root node
    let root_year = state.book.metadata.year.map(|y| format!(" ({})", y)).unwrap_or_default();
    let root_line = Line::from(vec![
        Span::styled("◉ ", Style::default().fg(app.theme.frost_ice())),
        Span::styled(super::super::truncate(&format!("{}{}", state.book.metadata.title, root_year), 120), Style::default().add_modifier(Modifier::BOLD)),
    ]);
    items.push(ListItem::new(root_line));

    // Children
    let visible_height = list_area.height.saturating_sub(1) as usize; // -1 for root
    let scroll_offset = if state.cursor >= visible_height {
        state.cursor.saturating_sub(visible_height - 1)
    } else {
        0
    };

    let count = visible_items.len();
    for (i, edge) in visible_items.iter().enumerate().skip(scroll_offset).take(visible_height) {
        let is_last = i == count - 1;
        let prefix = if is_last { "└── " } else { "├── " };
        
        let in_lib = if edge.in_library { "[✓]" } else { "[✗]" };
        let in_lib_style = if edge.in_library { Style::default().fg(app.theme.green()) } else { Style::default().fg(app.theme.red()) };
        
        let year_str = edge.year.map(|y| format!(" ({})", y)).unwrap_or_default();
        let id_str = edge.id_type.as_ref().map(|t| format!(" [{}]", t)).unwrap_or_default();
        
        let is_selected = i == state.cursor;
        let mut title_style = Style::default().fg(app.theme.fg());
        if is_selected {
            title_style = title_style.bg(app.theme.bg_secondary()).fg(app.theme.fg_bright());
        }

        let line = Line::from(vec![
            Span::styled(prefix, Style::default().fg(app.theme.border())),
            Span::styled(format!("{} ", in_lib), in_lib_style),
            Span::styled(super::super::truncate(&format!("{}{}{}", edge.title, year_str, id_str), 100), title_style),
        ]);
        items.push(ListItem::new(line));
    }

    let list = List::new(items);
    frame.render_widget(list, list_area);
}
