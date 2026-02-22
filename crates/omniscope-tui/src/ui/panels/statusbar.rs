use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{App, Mode};

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40), // Left zone: Page/Path
            Constraint::Percentage(20), // Center: Count
            Constraint::Length(15),     // Mode
            Constraint::Min(10),        // Right zone: Indicators
        ])
        .split(area);

    render_left_zone(frame, app, chunks[0]);
    render_center_zone(frame, app, chunks[1]);
    render_mode_zone(frame, app, chunks[2]);
    render_right_zone(frame, app, chunks[3]);
}

fn render_left_zone(frame: &mut Frame, app: &App, area: Rect) {
    let filter_text = match &app.sidebar_filter {
        crate::app::SidebarFilter::All => "all books".to_string(),
        crate::app::SidebarFilter::Library(name) => name.to_lowercase(),
        crate::app::SidebarFilter::Tag(name) => name.to_lowercase(),
        crate::app::SidebarFilter::VirtualFolder(name) => format!("virtual: {}", name),
        crate::app::SidebarFilter::Folder(path) => std::path::Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.clone()),
    };

    let content = Line::from(vec![
        Span::styled(
            " 󰂺 omniscope ",
            Style::default()
                .fg(app.theme.frost_ice())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" › ", Style::default().fg(app.theme.muted())),
        Span::styled(filter_text, Style::default().fg(app.theme.frost_mint())),
    ]);

    frame.render_widget(
        Paragraph::new(content).style(Style::default().bg(app.theme.bg_secondary())),
        area,
    );
}

fn render_center_zone(frame: &mut Frame, app: &App, area: Rect) {
    let count = app.books.len();
    let text = format!("{} books", count);
    let content = Line::from(Span::styled(text, Style::default().fg(app.theme.muted())));
    frame.render_widget(
        Paragraph::new(content)
            .style(Style::default().bg(app.theme.bg_secondary()))
            .alignment(ratatui::layout::Alignment::Center),
        area,
    );
}

fn render_mode_zone(frame: &mut Frame, app: &App, area: Rect) {
    let (label, bg, fg) = match app.mode {
        Mode::Normal => (" NORMAL ", app.theme.frost_dark(), app.theme.fg_white()),
        Mode::Insert => (" INSERT ", app.theme.green(), app.theme.bg()),
        Mode::Visual | Mode::VisualLine | Mode::VisualBlock => {
            (" VISUAL ", app.theme.orange(), app.theme.bg())
        }
        Mode::Command => (" COMMAND ", app.theme.frost_blue(), app.theme.fg_white()),
        Mode::Search => (" SEARCH ", app.theme.yellow(), app.theme.bg()),
        Mode::Pending => (" PENDING ", app.theme.muted(), app.theme.fg_bright()),
    };

    let content = Line::from(Span::styled(
        label,
        Style::default().bg(bg).fg(fg).add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(
        Paragraph::new(content).alignment(ratatui::layout::Alignment::Center),
        area,
    );
}

fn render_right_zone(frame: &mut Frame, app: &App, area: Rect) {
    let mut spans = Vec::new();

    if let Some(reg) = app.vim_register {
        spans.push(Span::styled(
            format!(" \"{} ", reg),
            Style::default().fg(app.theme.yellow()),
        ));
    }

    if !app.marks.is_empty() {
        spans.push(Span::styled(
            " m. ",
            Style::default().fg(app.theme.frost_mint()),
        ));
    }

    // AI Indicator
    spans.push(Span::styled(
        " ●AI ",
        Style::default().fg(app.theme.purple()),
    ));

    let content = Line::from(spans);
    frame.render_widget(
        Paragraph::new(content)
            .style(Style::default().bg(app.theme.bg_secondary()))
            .alignment(ratatui::layout::Alignment::Right),
        area,
    );
}
