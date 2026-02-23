use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::app::{App, RegisterContent};

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let overlay_area = crate::ui::centered_rect(70, 50, area);
    frame.render_widget(Clear, overlay_area);

    let block = Block::default()
        .title(Span::styled(
            " REGISTERS ",
            Style::default()
                .fg(app.theme.frost_ice())
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.frost_ice()))
        .style(Style::default().bg(app.theme.bg()));

    let inner = block.inner(overlay_area);
    frame.render_widget(block, overlay_area);

    let mut lines = vec![];

    let mut sorted_regs: Vec<_> = app.registers.iter().collect();
    sorted_regs.sort_by_key(|(c, _)| *c);

    for (c, reg) in sorted_regs {
        let (type_label, content) = match &reg.content {
            RegisterContent::Card(_) => ("[book] ", "Book card"),
            RegisterContent::Path(p) => ("[path] ", p.as_str()),
            RegisterContent::Text(t) => ("[text] ", t.as_str()),
            RegisterContent::MultipleCards(_v) => ("[multi]", "Multiple books"),
        };

        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                format!("\"{c}   "),
                Style::default()
                    .fg(app.theme.yellow())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(type_label, Style::default().fg(app.theme.frost_blue())),
            Span::raw("  "),
            Span::styled(
                super::super::truncate(content, 40),
                Style::default().fg(app.theme.fg()),
            ),
        ]));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}
