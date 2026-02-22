use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{ActivePanel, App};

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.active_panel == ActivePanel::Preview;
    let border_color = if is_focused {
        app.theme.active_panel()
    } else {
        app.theme.border()
    };

    let block = Block::default()
        .title(" PREVIEW ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(app.theme.bg()));

    let content = if app.center_panel_mode == crate::app::CenterPanelMode::FolderView {
        if let Some(item) = app.center_items.get(app.selected_index) {
            match item {
                crate::app::CenterItem::Folder(folder) => render_folder_preview(app, folder),
                crate::app::CenterItem::Book(book) => render_book_preview(app, book, area),
            }
        } else {
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  Select an item to preview",
                    Style::default().fg(app.theme.muted()),
                )),
            ]
        }
    } else if let Some(book) = app.selected_book() {
        render_book_preview(app, book, area)
    } else {
        vec![
            Line::from(""),
            Line::from(Span::styled(
                "  Select a book to preview",
                Style::default().fg(app.theme.muted()),
            )),
        ]
    };

    let preview = Paragraph::new(content)
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(preview, area);
}

fn render_folder_preview<'a>(app: &'a App, folder: &omniscope_core::models::Folder) -> Vec<Line<'a>> {
    vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("    {}", folder.name),
            Style::default()
                .fg(app.theme.yellow())
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("  {}", "─".repeat(20)),
            Style::default().fg(app.theme.border()),
        )),
        Line::from(""),
        Line::from(Span::styled("  DIRECTORY", Style::default().fg(app.theme.muted()))),
        Line::from(Span::styled(
            format!("  ID: {}", folder.id),
            Style::default().fg(app.theme.fg()),
        )),
        Line::from(""),
    ]
}

fn render_book_preview<'a>(
    app: &'a App,
    book: &omniscope_core::BookSummaryView,
    area: Rect,
) -> Vec<Line<'a>> {
    let authors = if book.authors.is_empty() {
        "Unknown".to_string()
    } else {
        book.authors.join(", ")
    };

    let year = book
        .year
        .map(|y| y.to_string())
        .unwrap_or_else(|| "—".to_string());

    let tags = if book.tags.is_empty() {
        "—".to_string()
    } else {
        book.tags
            .iter()
            .map(|t| format!("[{t}]"))
            .collect::<Vec<_>>()
            .join(" ")
    };

    let status_text = match book.read_status {
        omniscope_core::ReadStatus::Read => "read",
        omniscope_core::ReadStatus::Reading => "reading",
        omniscope_core::ReadStatus::Dnf => "dnf",
        omniscope_core::ReadStatus::Unread => "unread",
    };

    let stars = match book.rating {
        Some(r) => "★".repeat(r as usize) + &"☆".repeat(5 - r as usize),
        None => "☆☆☆☆☆".to_string(),
    };

    vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("  󰂺  {}", book.title),
            Style::default()
                .fg(app.theme.fg_bright())
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                stars
                    .chars()
                    .take(book.rating.unwrap_or(0) as usize)
                    .collect::<String>(),
                Style::default().fg(app.theme.yellow()),
            ),
            Span::styled(
                stars
                    .chars()
                    .skip(book.rating.unwrap_or(0) as usize)
                    .collect::<String>(),
                Style::default().fg(app.theme.border()),
            ),
            Span::raw("                  "),
            Span::styled(
                status_text,
                Style::default().fg(match book.read_status {
                    omniscope_core::ReadStatus::Read => app.theme.green(),
                    omniscope_core::ReadStatus::Reading => app.theme.frost_ice(),
                    _ => app.theme.muted(),
                }),
            ),
        ]),
        Line::from(Span::styled(
            format!("  {}", "─".repeat(area.width as usize - 6)),
            Style::default().fg(app.theme.border()),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  AUTHORS",
            Style::default().fg(app.theme.muted()),
        )),
        Line::from(Span::styled(
            format!("  {}", authors),
            Style::default().fg(app.theme.fg()),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  META",
            Style::default().fg(app.theme.muted()),
        )),
        Line::from(Span::styled(
            format!(
                "  {} · {} pages · {} · EN",
                year,
                0,
                book.format
                    .map(|f| f.to_string())
                    .unwrap_or("?".to_string())
            ),
            Style::default().fg(app.theme.fg()),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  TAGS",
            Style::default().fg(app.theme.muted()),
        )),
        Line::from(Span::styled(
            format!("  {}", tags),
            Style::default().fg(app.theme.frost_blue()),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  PATH",
            Style::default().fg(app.theme.muted()),
        )),
        Line::from(Span::styled(
            format!(
                "  {}",
                book.has_file.then(|| "file attached").unwrap_or("no file")
            ),
            Style::default().fg(app.theme.frost_mint()),
        )),
    ]
}
