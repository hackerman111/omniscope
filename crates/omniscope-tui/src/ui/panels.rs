use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{ActivePanel, App, Mode, SidebarItem};
use super::{colors, truncate};

pub(crate) fn render_body(frame: &mut Frame, app: &App, area: Rect) {
    let panels = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(45),
            Constraint::Percentage(35),
        ])
        .split(area);

    render_sidebar(frame, app, panels[0]);
    render_book_list(frame, app, panels[1]);
    render_preview(frame, app, panels[2]);
}

fn render_sidebar(frame: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.active_panel == ActivePanel::Sidebar;
    let border_color = if is_focused { colors::LAVENDER } else { colors::SURFACE1 };

    let block = Block::default()
        .title(" Libraries ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(colors::BASE));

    let items: Vec<ListItem> = app
        .sidebar_items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let is_selected = i == app.sidebar_selected && is_focused;
            let (text, style) = match item {
                SidebarItem::AllBooks { count } => {
                    let prefix = if is_selected { "▶ " } else { "  " };
                    (
                        format!("{prefix}📚 All [{count}]"),
                        Style::default().fg(colors::TEXT),
                    )
                }
                SidebarItem::Library { name, count } => {
                    let prefix = if is_selected { "▶ " } else { "  " };
                    (
                        format!("{prefix}📁 {name} [{count}]"),
                        Style::default().fg(colors::PEACH),
                    )
                }
                SidebarItem::TagHeader => (
                    " ─ Tags ─".to_string(),
                    Style::default()
                        .fg(colors::SUBTEXT0)
                        .add_modifier(Modifier::DIM),
                ),
                SidebarItem::Tag { name, count } => {
                    let prefix = if is_selected { "▶ " } else { "  " };
                    (
                        format!("{prefix}#{name} [{count}]"),
                        Style::default().fg(colors::BLUE),
                    )
                }
                SidebarItem::FolderHeader => (
                    " ─ Folders ─".to_string(),
                    Style::default()
                        .fg(colors::SUBTEXT0)
                        .add_modifier(Modifier::DIM),
                ),
                SidebarItem::Folder { path } => {
                    let prefix = if is_selected { "▶ " } else { "  " };
                    (
                        format!("{prefix}📂 {path}"),
                        Style::default().fg(colors::GREEN),
                    )
                }
            };

            let item_style = if is_selected {
                style.bg(colors::SURFACE0)
            } else {
                style
            };

            ListItem::new(Span::styled(text, item_style))
        })
        .collect();

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

fn render_book_list(frame: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.active_panel == ActivePanel::BookList;
    let border_color = if is_focused { colors::LAVENDER } else { colors::SURFACE1 };

    let title = if !app.search_input.is_empty() && app.mode == Mode::Search {
        format!(" Search: {} ({}) ", app.search_input, app.books.len())
    } else {
        format!(" Books ({}) ", app.books.len())
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(colors::BASE));

    if app.books.is_empty() {
        let empty_msg = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "  No books yet",
                Style::default().fg(colors::SUBTEXT0),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Press 'a' to add a book",
                Style::default().fg(colors::SUBTEXT0).add_modifier(Modifier::DIM),
            )),
            Line::from(Span::styled(
                "  or use: omniscope add <file>",
                Style::default().fg(colors::SUBTEXT0).add_modifier(Modifier::DIM),
            )),
        ])
        .block(block);
        frame.render_widget(empty_msg, area);
        return;
    }

    // Calculate visible range for virtual scrolling
    let inner = block.inner(area);
    let visible_height = inner.height as usize;
    let scroll_offset = if app.selected_index >= visible_height {
        app.selected_index - visible_height + 1
    } else {
        0
    };

    let items: Vec<ListItem> = app
        .books
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(visible_height)
        .map(|(i, book)| {
            let is_selected = i == app.selected_index;
            let is_visual = app.visual_selections.contains(&i);
            let prefix = if is_selected && is_focused {
                "▶ "
            } else if is_visual {
                "● "
            } else {
                "  "
            };

            let status_icon = match book.read_status {
                omniscope_core::ReadStatus::Read => "✓",
                omniscope_core::ReadStatus::Reading => "◐",
                omniscope_core::ReadStatus::Dnf => "✗",
                omniscope_core::ReadStatus::Unread => "○",
            };

            let rating = match book.rating {
                Some(5) => "★★★★★",
                Some(4) => "★★★★☆",
                Some(3) => "★★★☆☆",
                Some(2) => "★★☆☆☆",
                Some(1) => "★☆☆☆☆",
                _ => "     ",
            };

            let year_str = book
                .year
                .map(|y| y.to_string())
                .unwrap_or_else(|| "----".to_string());

            let format_str = book
                .format
                .map(|f| f.to_string())
                .unwrap_or_else(|| "---".to_string());

            let max_title = (inner.width as usize).saturating_sub(30);
            let line = Line::from(vec![
                Span::styled(prefix, Style::default().fg(colors::LAVENDER)),
                Span::styled(status_icon, Style::default().fg(match book.read_status {
                    omniscope_core::ReadStatus::Read => colors::GREEN,
                    omniscope_core::ReadStatus::Reading => colors::YELLOW,
                    omniscope_core::ReadStatus::Dnf => colors::RED,
                    omniscope_core::ReadStatus::Unread => colors::SURFACE1,
                })),
                Span::raw(" "),
                Span::styled(
                    truncate(&book.title, max_title),
                    Style::default().fg(colors::TEXT),
                ),
                Span::raw("  "),
                Span::styled(rating, Style::default().fg(colors::YELLOW)),
                Span::raw(" "),
                Span::styled(year_str, Style::default().fg(colors::SUBTEXT0)),
                Span::raw(" "),
                Span::styled(format_str, Style::default().fg(colors::BLUE)),
            ]);

            let style = if is_selected && is_focused {
                Style::default().bg(colors::SURFACE0)
            } else if is_visual {
                Style::default().bg(colors::SURFACE1)
            } else {
                Style::default()
            };

            ListItem::new(line).style(style)
        })
        .collect();

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

fn render_preview(frame: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.active_panel == ActivePanel::Preview;
    let border_color = if is_focused { colors::LAVENDER } else { colors::SURFACE1 };

    let block = Block::default()
        .title(" Preview ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(colors::BASE));

    let content = if let Some(book) = app.selected_book() {
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
            book.tags.iter().map(|t| format!("#{t}")).collect::<Vec<_>>().join(" ")
        };

        let rating = match book.rating {
            Some(r) => "★".repeat(r as usize) + &"☆".repeat(5 - r as usize),
            None => "—".to_string(),
        };

        let file_info = if book.has_file {
            format!("✓ file attached ({})", book.format.map(|f| f.to_string()).unwrap_or("?".to_string()))
        } else {
            "✗ no file".to_string()
        };

        vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("  📖 {}", book.title),
                Style::default()
                    .fg(colors::LAVENDER)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Authors: ", Style::default().fg(colors::SUBTEXT0)),
                Span::styled(authors, Style::default().fg(colors::TEXT)),
            ]),
            Line::from(vec![
                Span::styled("  Year:    ", Style::default().fg(colors::SUBTEXT0)),
                Span::styled(year, Style::default().fg(colors::TEXT)),
            ]),
            Line::from(vec![
                Span::styled("  Status:  ", Style::default().fg(colors::SUBTEXT0)),
                Span::styled(
                    book.read_status.to_string(),
                    Style::default().fg(match book.read_status {
                        omniscope_core::ReadStatus::Read => colors::GREEN,
                        omniscope_core::ReadStatus::Reading => colors::YELLOW,
                        omniscope_core::ReadStatus::Dnf => colors::RED,
                        omniscope_core::ReadStatus::Unread => colors::SUBTEXT0,
                    }),
                ),
            ]),
            Line::from(vec![
                Span::styled("  Rating:  ", Style::default().fg(colors::SUBTEXT0)),
                Span::styled(rating, Style::default().fg(colors::YELLOW)),
            ]),
            Line::from(vec![
                Span::styled("  Tags:    ", Style::default().fg(colors::SUBTEXT0)),
                Span::styled(tags, Style::default().fg(colors::BLUE)),
            ]),
            Line::from(vec![
                Span::styled("  File:    ", Style::default().fg(colors::SUBTEXT0)),
                Span::styled(file_info, Style::default().fg(
                    if book.has_file { colors::GREEN } else { colors::RED }
                )),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "  [o]pen  [R]ate  [s]tatus  [t]ags  [dd]delete  [yy]yank  [S]sort  [u]undo  [?]help",
                Style::default().fg(colors::SUBTEXT0).add_modifier(Modifier::DIM),
            )),
        ]
    } else {
        vec![
            Line::from(""),
            Line::from(Span::styled(
                "  Select a book to preview",
                Style::default().fg(colors::SUBTEXT0),
            )),
        ]
    };

    let preview = Paragraph::new(content).block(block).wrap(Wrap { trim: false });
    frame.render_widget(preview, area);
}
