use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Clear};
use ratatui::Frame;

use crate::app::App;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let state = if let Some(crate::popup::Popup::Telescope(ref st)) = app.popup {
        st
    } else {
        return;
    };

    // Telescope is a centered overlay
    let overlay_area = crate::ui::centered_rect(80, 80, area);
    
    // Clear the area
    frame.render_widget(Clear, overlay_area);

    let title_style = if state.mode == crate::popup::TelescopeMode::Insert {
        Style::default().fg(app.theme.green()).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(app.theme.frost_ice()).add_modifier(Modifier::BOLD)
    };
    
    let block = Block::default()
        .title(Span::styled(
            if state.mode == crate::popup::TelescopeMode::Insert { " omniscope [INSERT] " } else { " omniscope [NORMAL] " },
            title_style
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.frost_blue()))
        .style(Style::default().bg(app.theme.bg()));

    let inner = block.inner(overlay_area);
    frame.render_widget(block, overlay_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Search row
            Constraint::Length(1), // Separator
            Constraint::Min(5),    // Results
            Constraint::Length(1), // Separator
            Constraint::Length(6), // Preview
            Constraint::Length(1), // Help hints
        ])
        .split(inner);

    // 1. Search row
    // Handle cursor visualization
    let query = &state.query;
    let cursor = state.cursor;
    
    let before_cursor = &query[..cursor];
    let (at_cursor, after_cursor) = if cursor < query.len() {
        let next_char_len = query[cursor..].chars().next().unwrap().len_utf8();
        (&query[cursor..cursor+next_char_len], &query[cursor+next_char_len..])
    } else {
        (" ", "")
    };

    // Only highlight cursor box in Insert mode, else underline or normal box
    let cursor_style = if state.mode == crate::popup::TelescopeMode::Insert {
        Style::default().bg(app.theme.frost_ice()).fg(app.theme.bg())
    } else {
        Style::default().bg(app.theme.bg_secondary()).fg(app.theme.frost_ice()) // Dimmer cursor for normal mode
    };
    
    let search_content = Line::from(vec![
        Span::styled(" / ", Style::default().fg(app.theme.yellow()).add_modifier(Modifier::BOLD)),
        Span::styled(before_cursor, Style::default().fg(app.theme.fg_bright())),
        Span::styled(at_cursor, cursor_style),
        Span::styled(after_cursor, Style::default().fg(app.theme.fg_bright())),
        Span::raw(" ".repeat(chunks[0].width.saturating_sub(query.chars().count() as u16 + 15).max(0) as usize)),
        Span::styled(format!("{} results", state.results.len()), Style::default().fg(app.theme.frost_ice())),
    ]);
    frame.render_widget(Paragraph::new(search_content), chunks[0]);

    // 2. Separator
    frame.render_widget(Paragraph::new("─".repeat(chunks[1].width as usize)).style(Style::default().fg(app.theme.border())), chunks[1]);

    // 3. Results
    let visible_height = chunks[2].height as usize;
    let items: Vec<ListItem> = state.results.iter().skip(state.scroll).take(visible_height).enumerate().map(|(i, book)| {
        let actual_idx = state.scroll + i;
        let is_selected = actual_idx == state.selected;
        let prefix = if is_selected { "▶ " } else { "  " };
        let line = Line::from(vec![
            Span::styled(prefix, Style::default().fg(app.theme.frost_ice())),
            Span::styled(&book.title, Style::default().fg(if is_selected { app.theme.fg_bright() } else { app.theme.fg() })),
        ]);
        let style = if is_selected { Style::default().bg(app.theme.bg_secondary()) } else { Style::default() };
        ListItem::new(line).style(style)
    }).collect();
    frame.render_widget(List::new(items), chunks[2]);

    // 4. Separator
    frame.render_widget(Paragraph::new("─".repeat(chunks[3].width as usize)).style(Style::default().fg(app.theme.border())), chunks[3]);

    // 5. Preview
    if let Some(book) = state.results.get(state.selected) {
        let preview_text = vec![
            Line::from(Span::styled(format!("  󰂺  {}", book.title), Style::default().fg(app.theme.fg_bright()).add_modifier(Modifier::BOLD))),
            Line::from(Span::styled(format!("  {} · {}p", book.authors.join(", "), 0), Style::default().fg(app.theme.muted()))),
            Line::from(Span::styled(format!("  {}", book.tags.iter().map(|t| format!("[{t}]")).collect::<Vec<_>>().join(" ")), Style::default().fg(app.theme.frost_blue()))),
        ];
        frame.render_widget(Paragraph::new(preview_text), chunks[4]);
    }

    // 6. Help hints
    let hints = Line::from(vec![
        Span::styled(" Tab ", Style::default().fg(app.theme.yellow())), Span::styled("выбрать", Style::default().fg(app.theme.muted())),
        Span::styled("  Enter ", Style::default().fg(app.theme.yellow())), Span::styled("открыть", Style::default().fg(app.theme.muted())),
        Span::styled("  Esc ", Style::default().fg(app.theme.yellow())), Span::styled("закрыть", Style::default().fg(app.theme.muted())),
    ]);
    frame.render_widget(Paragraph::new(hints).alignment(ratatui::layout::Alignment::Center), chunks[5]);

    // 7. Autocomplete Dropdown
    if state.autocomplete.active && !state.autocomplete.visible.is_empty() {
        // Position it right below the search row
        let x = inner.x + 3 + before_cursor.len() as u16; // Roughly below cursor
        let y = inner.y + 1; // Below search row
        let width = (inner.width as u16).saturating_sub(x - inner.x).max(20).min(40);
        let height = state.autocomplete.visible.len().min(5) as u16 + 2;

        let sug_area = Rect { x, y, width, height };
        frame.render_widget(Clear, sug_area);

        let items: Vec<ListItem> = state.autocomplete.visible.iter()
            .enumerate()
            .map(|(i, s)| {
                let is_sel = state.autocomplete.selected == Some(i);
                let style = if is_sel {
                    Style::default().bg(app.theme.green()).fg(app.theme.bg()).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(app.theme.fg())
                };
                ListItem::new(Span::styled(format!(" {s} "), style))
            })
            .collect();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.green()))
            .style(Style::default().bg(app.theme.bg_secondary()));
        
        frame.render_widget(List::new(items).block(block), sug_area);
    }
}
