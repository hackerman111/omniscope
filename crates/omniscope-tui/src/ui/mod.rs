pub(crate) mod layout;
pub(crate) mod overlays;
pub(crate) mod panels;
pub(crate) mod popups;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{App, Mode};

// Removed colors module, using app.theme instead

/// Render the entire UI.
pub fn render(frame: &mut Frame, app: &App) {
    let size = frame.area();

    // Main vertical layout: body + status bar + command line
    let show_cmdline = app.mode == Mode::Command || app.mode == Mode::Search;
    let main_constraints = if show_cmdline {
        vec![
            Constraint::Min(3),    // body
            Constraint::Length(1), // status bar
            Constraint::Length(1), // command line
        ]
    } else {
        vec![
            Constraint::Min(3),    // body
            Constraint::Length(1), // status bar
        ]
    };

    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(main_constraints)
        .split(size);

    panels::render_body(frame, app, main_layout[0]);
    panels::statusbar::render(frame, app, main_layout[1]);

    if show_cmdline {
        panels::cmdline::render(frame, app, main_layout[2]);
    }

    // Popup overlay (on top of everything)
    if let Some(ref popup) = app.popup {
        popups::render_popup(frame, app, popup, size);
    }

    // Key Hints overlay (contextual)
    let show_hints = app.pending_key.is_some()
        || app.pending_operator.is_some()
        || app.mode == Mode::Visual
        || app.mode == Mode::VisualLine
        || app.mode == Mode::VisualBlock
        || app.pending_register_select;

    if show_hints {
        render_key_hints(frame, app, size);
    }
}

fn render_key_hints(frame: &mut Frame, app: &App, area: Rect) {
    use crate::keys::ui::hints::get_hints;
    let hints = get_hints(app);
    if hints.is_empty() {
        return;
    }

    // Multi-row hint card, positioned above status bar/command line.
    let rows = if hints.len() > 10 { 3u16 } else { 2u16 };
    let height = rows + 1;
    let statusbar_height = 1u16;
    let cmdline_height = if app.mode == Mode::Command || app.mode == Mode::Search {
        1u16
    } else {
        0u16
    };

    let hint_area = Rect {
        x: area.x,
        y: area
            .height
            .saturating_sub(height + statusbar_height + cmdline_height),
        width: area.width,
        height,
    };

    frame.render_widget(ratatui::widgets::Clear, hint_area);

    let block = ratatui::widgets::Block::default()
        .title(Span::styled(
            hint_title(app),
            Style::default()
                .fg(app.theme.frost_ice())
                .add_modifier(Modifier::BOLD),
        ))
        .borders(ratatui::widgets::Borders::TOP)
        .border_style(
            Style::default()
                .fg(app.theme.border())
                .add_modifier(Modifier::DIM),
        )
        .style(Style::default().bg(app.theme.bg()));

    let inner_area = block.inner(hint_area);
    frame.render_widget(block, hint_area);

    let rows_usize = usize::from(rows);
    let chunk = hints.len().div_ceil(rows_usize);
    let mut lines = Vec::new();

    for row_idx in 0..rows_usize {
        let start = row_idx.saturating_mul(chunk);
        if start >= hints.len() {
            break;
        }
        let end = (start + chunk).min(hints.len());

        let mut spans = Vec::new();
        for hint in &hints[start..end] {
            spans.push(Span::styled(
                format!(" [{}] ", hint.key),
                Style::default()
                    .fg(app.theme.yellow())
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(
                format!("{}  ", hint.desc),
                Style::default().fg(app.theme.muted()),
            ));
        }
        lines.push(Line::from(spans));
    }

    let paragraph = Paragraph::new(lines)
        .wrap(ratatui::widgets::Wrap { trim: true })
        .style(Style::default().bg(app.theme.bg()));

    frame.render_widget(paragraph, inner_area);
}

fn hint_title(app: &App) -> &'static str {
    if app.pending_register_select {
        return " Registers ";
    }
    if app.pending_operator.is_some() {
        return " Operator Pending ";
    }
    if let Some(pending) = app.pending_key {
        return match pending {
            'g' => " g-commands ",
            'z' => " z-commands ",
            '@' => " @-commands ",
            'S' => " Sort ",
            _ => " Pending ",
        };
    }
    if matches!(
        app.mode,
        Mode::Visual | Mode::VisualLine | Mode::VisualBlock
    ) {
        return " Visual Mode ";
    }
    " Hints "
}

// Status bar is now in panels/statusbar.rs

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
