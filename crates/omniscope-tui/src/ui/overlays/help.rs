use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::App;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let overlay_area = crate::ui::centered_rect(70, 70, area);
    frame.render_widget(Clear, overlay_area);

    let block = Block::default()
        .title(Span::styled(
            " HELP — omniscope vim motions ",
            Style::default()
                .fg(app.theme.frost_ice())
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.frost_blue()))
        .style(Style::default().bg(app.theme.bg()));

    let inner = block.inner(overlay_area);
    frame.render_widget(block, overlay_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),
            Constraint::Length(1), // Footer
        ])
        .split(inner);

    let help_text = vec![
        Line::from(vec![
            Span::styled(
                "НАВИГАЦИЯ         ",
                Style::default()
                    .fg(app.theme.frost_ice())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "ОПЕРАТОРЫ           ",
                Style::default()
                    .fg(app.theme.frost_ice())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "РЕЖИМЫ",
                Style::default()
                    .fg(app.theme.frost_ice())
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("j/k ", Style::default().fg(app.theme.yellow())),
            Span::raw("вниз/вверх   "),
            Span::styled("d ", Style::default().fg(app.theme.yellow())),
            Span::raw("delete           "),
            Span::styled("i/a/o ", Style::default().fg(app.theme.yellow())),
            Span::raw("INSERT"),
        ]),
        Line::from(vec![
            Span::styled("h/l ", Style::default().fg(app.theme.yellow())),
            Span::raw("панель        "),
            Span::styled("y ", Style::default().fg(app.theme.yellow())),
            Span::raw("yank             "),
            Span::styled("v     ", Style::default().fg(app.theme.yellow())),
            Span::raw("VISUAL"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "TEXT OBJECTS       ",
                Style::default()
                    .fg(app.theme.frost_ice())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "g-КОМАНДЫ           ",
                Style::default()
                    .fg(app.theme.frost_ice())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "MACROS",
                Style::default()
                    .fg(app.theme.frost_ice())
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("il ", Style::default().fg(app.theme.yellow())),
            Span::raw("inner library  "),
            Span::styled("gh ", Style::default().fg(app.theme.yellow())),
            Span::raw("home            "),
            Span::styled("q{a} ", Style::default().fg(app.theme.yellow())),
            Span::raw("запись"),
        ]),
    ];

    frame.render_widget(Paragraph::new(help_text), chunks[0]);

    let footer = Line::from(vec![
        Span::styled(" Esc ", Style::default().fg(app.theme.yellow())),
        Span::styled("закрыть", Style::default().fg(app.theme.muted())),
        Span::styled("  Tab ", Style::default().fg(app.theme.yellow())),
        Span::styled("следующая секция", Style::default().fg(app.theme.muted())),
    ]);
    frame.render_widget(
        Paragraph::new(footer).alignment(ratatui::layout::Alignment::Center),
        chunks[1],
    );
}
