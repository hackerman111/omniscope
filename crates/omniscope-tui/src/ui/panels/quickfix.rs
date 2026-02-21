use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::app::App;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let title = format!(" QUICKFIX ({}) ", app.quickfix_list.len());
    let block = Block::default()
        .title(Span::styled(title, Style::default().fg(app.theme.orange())))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.orange()))
        .style(Style::default().bg(app.theme.bg()));

    if app.quickfix_list.is_empty() {
        let empty_msg =
            Paragraph::new("  Quickfix list is empty (use Ctrl+q to populate)").block(block);
        frame.render_widget(empty_msg, area);
        return;
    }

    let inner = block.inner(area);
    let visible_height = inner.height as usize;
    let scroll_offset = if app.quickfix_selected >= visible_height {
        app.quickfix_selected.saturating_sub(visible_height - 1)
    } else {
        0
    };

    let items: Vec<ListItem> = app
        .quickfix_list
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(visible_height)
        .map(|(i, book)| {
            let is_selected = i == app.quickfix_selected;
            let prefix = if is_selected { "▶ " } else { "  " };
            let max_title = (inner.width as usize).saturating_sub(10);
            let line = Line::from(vec![
                Span::styled(prefix, Style::default().fg(app.theme.orange())),
                Span::styled(
                    super::super::truncate(&book.title, max_title),
                    Style::default().fg(app.theme.fg()),
                ),
            ]);
            let style = if is_selected {
                Style::default().bg(app.theme.bg_secondary())
            } else {
                Style::default()
            };
            ListItem::new(line).style(style)
        })
        .collect();

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}
