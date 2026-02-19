mod panels;
mod popups;

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{App, Mode};

/// Color palette — Catppuccin Mocha inspired defaults.
pub(crate) mod colors {
    use ratatui::style::Color;

    pub const BASE: Color = Color::Rgb(30, 30, 46);
    pub const SURFACE0: Color = Color::Rgb(49, 50, 68);
    pub const SURFACE1: Color = Color::Rgb(69, 71, 90);
    pub const TEXT: Color = Color::Rgb(205, 214, 244);
    pub const SUBTEXT0: Color = Color::Rgb(166, 173, 200);
    pub const LAVENDER: Color = Color::Rgb(180, 190, 254);
    pub const BLUE: Color = Color::Rgb(137, 180, 250);
    pub const GREEN: Color = Color::Rgb(166, 227, 161);
    pub const YELLOW: Color = Color::Rgb(249, 226, 175);
    pub const PEACH: Color = Color::Rgb(250, 179, 135);
    pub const RED: Color = Color::Rgb(243, 139, 168);
    pub const MAUVE: Color = Color::Rgb(203, 166, 247);
}

/// Render the entire UI.
pub fn render(frame: &mut Frame, app: &App) {
    let size = frame.area();

    // Main vertical layout: header + body + status bar
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header
            Constraint::Min(3),   // body
            Constraint::Length(1), // status bar
        ])
        .split(size);

    render_header(frame, app, main_layout[0]);
    panels::render_body(frame, app, main_layout[1]);
    render_status_bar(frame, app, main_layout[2]);

    // Popup overlay (on top of everything)
    if let Some(ref popup) = app.popup {
        popups::render_popup(frame, popup, size);
    }
}

// ─── Header ────────────────────────────────────────────────

fn render_header(frame: &mut Frame, app: &App, area: Rect) {
    let mode_style = match app.mode {
        Mode::Normal  => Style::default().fg(colors::BLUE).add_modifier(Modifier::BOLD),
        Mode::Insert  => Style::default().fg(colors::GREEN).add_modifier(Modifier::BOLD),
        Mode::Search  => Style::default().fg(colors::YELLOW).add_modifier(Modifier::BOLD),
        Mode::Command => Style::default().fg(colors::PEACH).add_modifier(Modifier::BOLD),
        Mode::Visual | Mode::VisualLine | Mode::VisualBlock => Style::default().fg(colors::MAUVE).add_modifier(Modifier::BOLD),
        // Pending: orange — signals waiting for a second key (e.g. after 'd')
        Mode::Pending => Style::default().fg(colors::PEACH).add_modifier(Modifier::BOLD),
    };

    let filter_text = match &app.sidebar_filter {
        crate::app::SidebarFilter::All => "All books".to_string(),
        crate::app::SidebarFilter::Library(name) => format!("📁 {name}"),
        crate::app::SidebarFilter::Tag(name) => format!("#{name}"),
    };

    let header = Line::from(vec![
        Span::styled(" 󰂺 omniscope ", Style::default().fg(colors::LAVENDER).add_modifier(Modifier::BOLD)),
        Span::styled(format!("  {filter_text}  "), Style::default().fg(colors::SUBTEXT0)),
        Span::raw(" ".repeat(area.width.saturating_sub(filter_text.len() as u16 + 30) as usize)),
        Span::styled(format!(" {} ", app.mode), mode_style),
    ]);

    frame.render_widget(
        Paragraph::new(header).style(Style::default().bg(colors::SURFACE0)),
        area,
    );
}

// ─── Status Bar ────────────────────────────────────────────

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let content = match app.mode {
        Mode::Command => {
            Line::from(vec![
                Span::styled(":", Style::default().fg(colors::PEACH)),
                Span::styled(&app.command_input, Style::default().fg(colors::TEXT)),
                Span::styled("█", Style::default().fg(colors::TEXT)),
            ])
        }
        Mode::Search => {
            Line::from(vec![
                Span::styled("/", Style::default().fg(colors::YELLOW)),
                Span::styled(&app.search_input, Style::default().fg(colors::TEXT)),
                Span::styled("█", Style::default().fg(colors::TEXT)),
            ])
        }
        _ => {
            if app.status_message.is_empty() {
                // Show count prefix if accumulating
                let count_hint = if app.vim_count > 0 {
                    format!(" {}…", app.vim_count)
                } else {
                    String::new()
                };
                Line::from(vec![
                    Span::styled(count_hint, Style::default().fg(colors::YELLOW).add_modifier(Modifier::BOLD)),
                    Span::styled(" ?", Style::default().fg(colors::SUBTEXT0)),
                    Span::styled(":help  ", Style::default().fg(colors::SUBTEXT0).add_modifier(Modifier::DIM)),
                    Span::styled("/", Style::default().fg(colors::SUBTEXT0)),
                    Span::styled(":search  ", Style::default().fg(colors::SUBTEXT0).add_modifier(Modifier::DIM)),
                    Span::styled("a", Style::default().fg(colors::SUBTEXT0)),
                    Span::styled(":add  ", Style::default().fg(colors::SUBTEXT0).add_modifier(Modifier::DIM)),
                    Span::styled("S", Style::default().fg(colors::SUBTEXT0)),
                    Span::styled(":sort  ", Style::default().fg(colors::SUBTEXT0).add_modifier(Modifier::DIM)),
                    Span::styled("u", Style::default().fg(colors::SUBTEXT0)),
                    Span::styled(":undo  ", Style::default().fg(colors::SUBTEXT0).add_modifier(Modifier::DIM)),
                    Span::styled("q", Style::default().fg(colors::SUBTEXT0)),
                    Span::styled(":quit", Style::default().fg(colors::SUBTEXT0).add_modifier(Modifier::DIM)),
                ])
            } else {
                Line::from(Span::styled(
                    format!(" {}", app.status_message),
                    Style::default().fg(colors::TEXT),
                ))
            }
        }
    };

    frame.render_widget(
        Paragraph::new(content).style(Style::default().bg(colors::SURFACE0)),
        area,
    );
}

// ─── Helpers ───────────────────────────────────────────────

pub(crate) fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        format!("{s:<max$}")
    } else {
        let truncated: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{truncated}…")
    }
}

/// Create a centered rectangle.
pub(crate) fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
