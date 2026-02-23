use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::app::App;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let overlay_area = crate::ui::centered_rect(60, 40, area);
    frame.render_widget(Clear, overlay_area);

    let block = Block::default()
        .title(Span::styled(
            " MARKS ",
            Style::default()
                .fg(app.theme.frost_mint())
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.frost_mint()))
        .style(Style::default().bg(app.theme.bg()));

    let inner = block.inner(overlay_area);
    frame.render_widget(block, overlay_area);

    let mut lines = vec![
        Line::from(vec![Span::styled(
            "  Mark  Position                      Book",
            Style::default().fg(app.theme.muted()),
        )]),
        Line::from(Span::styled(
            format!("  {}", "─".repeat(inner.width as usize - 4)),
            Style::default().fg(app.theme.border()),
        )),
    ];

    let mut sorted_marks: Vec<_> = app.marks.iter().collect();
    sorted_marks.sort_by_key(|(c, _)| *c);

    for (c, idx) in sorted_marks {
        let book_title = app
            .books
            .get(*idx)
            .map(|b| b.title.as_str())
            .unwrap_or("Unknown");
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                format!("{c}     "),
                Style::default()
                    .fg(app.theme.yellow())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("#{}                  ", idx),
                Style::default().fg(app.theme.frost_mint()),
            ),
            Span::styled(
                super::super::truncate(book_title, 25),
                Style::default().fg(app.theme.fg()),
            ),
        ]));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}
