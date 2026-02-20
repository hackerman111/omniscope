use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{App, Mode};

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let content = match app.mode {
        Mode::Command => {
            Line::from(vec![
                Span::styled(" : ", Style::default().fg(app.theme.frost_ice()).add_modifier(Modifier::BOLD)),
                Span::styled(&app.command_input, Style::default().fg(app.theme.fg_bright())),
                Span::styled("█", Style::default().fg(app.theme.frost_ice())),
            ])
        }
        Mode::Search => {
            Line::from(vec![
                Span::styled(" / ", Style::default().fg(app.theme.yellow()).add_modifier(Modifier::BOLD)),
                Span::styled(&app.search_input, Style::default().fg(app.theme.fg_bright())),
                Span::styled("█", Style::default().fg(app.theme.frost_ice())),
            ])
        }
        _ => return,
    };

    frame.render_widget(
        Paragraph::new(content).style(Style::default().bg(app.theme.bg())),
        area,
    );
}
