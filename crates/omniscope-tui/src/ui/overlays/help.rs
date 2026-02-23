use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::app::App;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let overlay_area = crate::ui::centered_rect(84, 82, area);
    frame.render_widget(Clear, overlay_area);

    let block = Block::default()
        .title(Span::styled(
            " HELP — Omniscope Keymap ",
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
            Constraint::Length(2), // Footer
        ])
        .split(inner);

    let help_text = vec![
        Line::from(vec![
            Span::styled(
                " НАВИГАЦИЯ ",
                Style::default()
                    .fg(app.theme.frost_ice())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "  j/k - вниз/вверх   h/l - смена панели   gg/G - начало/конец   / ? n N - поиск",
                Style::default().fg(app.theme.fg()),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                " РЕЖИМЫ ",
                Style::default()
                    .fg(app.theme.frost_ice())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "  i/a/o INSERT   v/V/C-v VISUAL   : COMMAND   Esc назад   u/C-r undo/redo",
                Style::default().fg(app.theme.fg()),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                " РЕДАКТИРОВАНИЕ ",
                Style::default()
                    .fg(app.theme.frost_ice())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "  d/y/c + motion   . повтор   \"<reg> регистры   q<reg> запись макро   @<reg> запуск",
                Style::default().fg(app.theme.fg()),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                " SCIENCE / PREVIEW ",
                Style::default()
                    .fg(app.theme.frost_ice())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "  r refs   c cited-by   f find/download   o open pdf   e bibtex",
                Style::default().fg(app.theme.fg()),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                " SCIENCE (g/@) ",
                Style::default()
                    .fg(app.theme.frost_ice())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "  gr refs   gR cited-by   gs related   @m metadata   @e ai-meta   @r ai-refs",
                Style::default().fg(app.theme.fg()),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                " POPUPS ",
                Style::default()
                    .fg(app.theme.frost_ice())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "  j/k move   Enter open/details   a add   f find   d download   m import metadata",
                Style::default().fg(app.theme.fg()),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                " БЫСТРЫЕ КОМАНДЫ ",
                Style::default()
                    .fg(app.theme.frost_ice())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "  :help  :cite [style]  :cited-by  :open  :w  :q",
                Style::default().fg(app.theme.fg()),
            ),
        ]),
    ];

    frame.render_widget(Paragraph::new(help_text), chunks[0]);

    let footer = Line::from(vec![
        Span::styled(" Esc ", Style::default().fg(app.theme.yellow())),
        Span::styled("закрыть", Style::default().fg(app.theme.muted())),
        Span::styled("  Tab ", Style::default().fg(app.theme.yellow())),
        Span::styled(
            "переключение в popups",
            Style::default().fg(app.theme.muted()),
        ),
        Span::styled("  j/k ", Style::default().fg(app.theme.yellow())),
        Span::styled(
            "скролл внутри popup",
            Style::default().fg(app.theme.muted()),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(vec![
            footer,
            Line::from(Span::styled(
                "Подсказки снизу экрана динамические: они зависят от режима, панели и префикса клавиши.",
                Style::default()
                    .fg(app.theme.muted())
                    .add_modifier(Modifier::DIM),
            )),
        ])
        .alignment(ratatui::layout::Alignment::Center),
        chunks[1],
    );
}
