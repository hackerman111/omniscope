use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::app::{ActivePanel, App, Mode, CenterPanelMode, CenterItem};

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.active_panel == ActivePanel::BookList;
    let border_color = if is_focused {
        app.theme.active_panel()
    } else {
        app.theme.border()
    };

    let title = if app.center_panel_mode == CenterPanelMode::FolderView {
        let breadcrumb_name = if let Some(ref current_id) = app.current_folder {
            if let Some(ref tree) = app.folder_tree {
                if let Some(node) = tree.nodes.get(current_id) {
                    node.folder.name.clone()
                } else {
                    "Unknown".to_string()
                }
            } else {
                "ROOT".to_string()
            }
        } else {
            "ROOT".to_string()
        };
        format!(" Folder: {} ({}) ", breadcrumb_name, app.center_items.len())
    } else if !app.search_input.is_empty() && app.mode == Mode::Search {
        format!(" Search: {} ({}) ", app.search_input, app.books.len())
    } else {
        format!(" Books ({}) ", app.books.len())
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(app.theme.bg()));

    let is_empty = if app.center_panel_mode == CenterPanelMode::FolderView {
        app.center_items.is_empty()
    } else {
        app.books.is_empty()
    };

    if is_empty {
        let empty_msg = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "  Directory is empty",
                Style::default().fg(app.theme.muted()),
            )),
        ])
        .block(block);
        frame.render_widget(empty_msg, area);
        return;
    }

    let inner = block.inner(area);
    let visible_height = inner.height as usize;
    let scroll_offset = if app.selected_index >= visible_height {
        app.selected_index - visible_height + 1
    } else {
        0
    };

    let mut items: Vec<ListItem> = Vec::new();

    if app.center_panel_mode == CenterPanelMode::FolderView {
        for (i, item) in app.center_items.iter().enumerate().skip(scroll_offset).take(visible_height) {
            match item {
                CenterItem::Folder(folder) => {
                    items.push(render_folder_row(app, i, folder, is_focused, inner.width));
                }
                CenterItem::Book(book) => {
                    items.push(render_book_row(app, i, book, is_focused, inner.width));
                }
            }
        }
    } else {
        for (i, book) in app.books.iter().enumerate().skip(scroll_offset).take(visible_height) {
            items.push(render_book_row(app, i, book, is_focused, inner.width));
        }
    }

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

fn render_folder_row<'a>(
    app: &'a App,
    i: usize,
    folder: &'a omniscope_core::models::Folder,
    is_focused: bool,
    _width: u16,
) -> ListItem<'a> {
    let is_selected = i == app.selected_index;
    let is_visual = app.visual_selections.contains(&i);

    let prefix_str = if is_selected && is_focused { "▶ " } else if is_visual { "● " } else { "  " };
    let prefix_style = if is_selected && is_focused {
        Style::default().fg(app.theme.frost_ice()).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(app.theme.frost_ice())
    };

    let folder_icon = Span::styled(" ", Style::default().fg(app.theme.yellow()));
    let title_style = Style::default().fg(if is_selected { app.theme.fg_bright() } else { app.theme.fg() }).add_modifier(Modifier::BOLD);

    let line = Line::from(vec![
        Span::styled(prefix_str, prefix_style),
        folder_icon,
        Span::styled(folder.name.clone(), title_style),
        Span::styled(" / ", Style::default().fg(app.theme.muted())),
    ]);

    let bg_style = if is_selected && is_focused {
        Style::default().bg(app.theme.bg_secondary())
    } else if is_visual {
        Style::default().bg(app.theme.frost_dark()).fg(app.theme.fg_white())
    } else {
        Style::default()
    };

    ListItem::new(line).style(bg_style)
}

fn render_book_row<'a>(
    app: &'a App,
    i: usize,
    book: &'a omniscope_core::BookSummaryView,
    is_focused: bool,
    width: u16,
) -> ListItem<'a> {
    let is_selected = i == app.selected_index;
    let is_visual = app.visual_selections.contains(&i);
    let mark_char = app.marks.iter().find_map(|(&c, &idx)| if idx == i { Some(c) } else { None });

    let prefix_str = if is_selected && is_focused { "▶ " } else if is_visual { "● " } else { "  " };
    let prefix_style = if is_selected && is_focused {
        Style::default().fg(app.theme.frost_ice()).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(app.theme.frost_ice())
    };

    let status_icon = match book.read_status {
        omniscope_core::ReadStatus::Read => "✓",
        omniscope_core::ReadStatus::Reading => "●",
        omniscope_core::ReadStatus::Dnf => "✕",
        omniscope_core::ReadStatus::Unread => "○",
    };

    let rating = match book.rating {
        Some(r) => {
            let mut stars = String::new();
            for _ in 0..r { stars.push('★'); }
            for _ in r..5 { stars.push('☆'); }
            stars
        }
        _ => "     ".to_string(),
    };

    let year_str = book.year.map(|y| y.to_string()).unwrap_or_else(|| "----".to_string());
    let max_title = (width as usize).saturating_sub(44);

    let mut line_spans = vec![Span::styled(prefix_str, prefix_style)];

    if let Some(c) = mark_char {
        line_spans.push(Span::styled(format!("'{c} "), Style::default().fg(app.theme.yellow()).add_modifier(Modifier::BOLD)));
    }

    let ghost_indicator = if !book.has_file {
        Span::styled(" ○ ghost ", Style::default().fg(app.theme.muted()))
    } else {
        Span::raw("")
    };

    let title_style = if !book.has_file {
        Style::default().fg(app.theme.muted())
    } else if is_selected {
        Style::default().fg(app.theme.fg_bright())
    } else {
        Style::default().fg(app.theme.fg())
    };

    line_spans.extend(vec![
        Span::styled(
            status_icon,
            Style::default().fg(match book.read_status {
                omniscope_core::ReadStatus::Read => app.theme.green(),
                omniscope_core::ReadStatus::Reading => app.theme.frost_ice(),
                omniscope_core::ReadStatus::Dnf => app.theme.red(),
                omniscope_core::ReadStatus::Unread => app.theme.muted(),
            }),
        ),
        Span::raw(" "),
        Span::styled(super::super::truncate(&book.title, max_title), title_style),
        ghost_indicator,
        Span::raw("  "),
        Span::styled(rating.chars().take(book.rating.unwrap_or(0) as usize).collect::<String>(), Style::default().fg(app.theme.yellow())),
        Span::styled(rating.chars().skip(book.rating.unwrap_or(0) as usize).collect::<String>(), Style::default().fg(app.theme.border())),
        Span::raw(" "),
        Span::styled(year_str, Style::default().fg(app.theme.muted())),
    ]);

    let bg_style = if is_selected && is_focused {
        Style::default().bg(app.theme.bg_secondary())
    } else if is_visual {
        Style::default().bg(app.theme.frost_dark()).fg(app.theme.fg_white())
    } else {
        Style::default()
    };

    ListItem::new(Line::from(line_spans)).style(bg_style)
}
