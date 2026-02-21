use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    if app.ai_panel_active {
        render_ai_chat(frame, app, area);
    } else {
        super::right::render(frame, app, area);
    }
}

fn render_ai_chat(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(Span::styled(
            " 󱤅 OMNISCOPE AI ",
            Style::default()
                .fg(app.theme.purple())
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.purple()))
        .style(Style::default().bg(app.theme.bg()));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Book title
            Constraint::Min(5),    // Chat history
            Constraint::Length(1), // Suggested actions
            Constraint::Length(3), // Input field
            Constraint::Length(1), // Hints
        ])
        .split(inner);

    if let Some(book) = app.selected_book() {
        // 1. Book title
        let title_line = Line::from(vec![
            Span::styled(" ─── ", Style::default().fg(app.theme.border())),
            Span::styled(&book.title, Style::default().fg(app.theme.fg())),
            Span::styled(" ─── ", Style::default().fg(app.theme.border())),
        ]);
        frame.render_widget(
            Paragraph::new(title_line).alignment(ratatui::layout::Alignment::Center),
            chunks[0],
        );
    }

    // 2. Mock Assistant response (per Step 14 example)
    let assistant_msg = vec![
        Line::from(Span::styled(
            " Assistant",
            Style::default().fg(app.theme.muted()),
        )),
        Line::from(vec![
            Span::styled(" ┌", Style::default().fg(app.theme.frost_blue())),
            Span::styled(
                "─".repeat(chunks[1].width as usize - 4),
                Style::default().fg(app.theme.border()),
            ),
            Span::styled("┐", Style::default().fg(app.theme.frost_blue())),
        ]),
        Line::from(vec![
            Span::styled(" │ ", Style::default().fg(app.theme.frost_blue())),
            Span::styled(
                "Это фундаментальная книга по Rust...",
                Style::default().fg(app.theme.fg()),
            ),
        ]),
        Line::from(vec![
            Span::styled(" └", Style::default().fg(app.theme.frost_blue())),
            Span::styled(
                "─".repeat(chunks[1].width as usize - 4),
                Style::default().fg(app.theme.border()),
            ),
            Span::styled("┘", Style::default().fg(app.theme.frost_blue())),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(assistant_msg).wrap(Wrap { trim: false }),
        chunks[1],
    );

    // 3. Suggested actions
    let suggested = Line::from(vec![
        Span::styled(
            " Suggested actions: ",
            Style::default().fg(app.theme.muted()),
        ),
        Span::styled(
            " [1] ",
            Style::default()
                .fg(app.theme.frost_ice())
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Добавить теги "),
        Span::styled(
            " [2] ",
            Style::default()
                .fg(app.theme.frost_ice())
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Интекс"),
    ]);
    frame.render_widget(Paragraph::new(suggested), chunks[2]);

    // 4. Input field
    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.frost_ice()))
        .style(Style::default().bg(app.theme.bg_secondary()));
    let input_para = Paragraph::new(app.ai_input.as_str()).block(input_block);
    frame.render_widget(input_para, chunks[3]);

    // 5. Hints
    let hints = Line::from(vec![
        Span::styled(" Ctrl+Enter ", Style::default().fg(app.theme.yellow())),
        Span::styled("отправить", Style::default().fg(app.theme.muted())),
    ]);
    frame.render_widget(
        Paragraph::new(hints).alignment(ratatui::layout::Alignment::Center),
        chunks[4],
    );
}
