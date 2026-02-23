use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{App, Mode};

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let content = match app.mode {
        Mode::Command => Line::from(vec![
            Span::styled(
                " : ",
                Style::default()
                    .fg(app.theme.frost_ice())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                &app.command_input,
                Style::default().fg(app.theme.fg_bright()),
            ),
            Span::styled("█", Style::default().fg(app.theme.frost_ice())),
        ]),
        Mode::Search => Line::from(vec![
            Span::styled(
                " / ",
                Style::default()
                    .fg(app.theme.yellow())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                &app.search_input,
                Style::default().fg(app.theme.fg_bright()),
            ),
            Span::styled("█", Style::default().fg(app.theme.frost_ice())),
        ]),
        _ => return,
    };

    frame.render_widget(
        Paragraph::new(content).style(Style::default().bg(app.theme.bg())),
        area,
    );

    if app.mode == Mode::Command && !app.command_suggestions.is_empty() {
        render_suggestions(frame, app, area);
    }
}

fn render_suggestions(frame: &mut Frame, app: &App, cmdline_area: Rect) {
    let max_suggestions = 5.min(app.command_suggestions.len()) as u16;

    if cmdline_area.y < max_suggestions {
        return;
    }

    let suggestion_area = Rect {
        x: 3,
        y: cmdline_area.y.saturating_sub(max_suggestions),
        width: cmdline_area.width.saturating_sub(3),
        height: max_suggestions,
    };

    frame.render_widget(ratatui::widgets::Clear, suggestion_area);

    let selected_idx = app.command_suggestion_idx.unwrap_or(0);

    let lines: Vec<Line> = app
        .command_suggestions
        .iter()
        .take(max_suggestions as usize)
        .enumerate()
        .map(|(i, suggestion)| {
            let is_selected = i == selected_idx;
            let style = if is_selected {
                Style::default()
                    .fg(app.theme.fg_bright())
                    .bg(app.theme.frost_dark())
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme.muted())
            };
            Line::from(Span::styled(format!(" {}", suggestion), style))
        })
        .collect();

    let paragraph = Paragraph::new(lines).style(Style::default().bg(app.theme.bg()));

    frame.render_widget(paragraph, suggestion_area);
}
