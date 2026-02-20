use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem};
use ratatui::text::Span;
use ratatui::Frame;

use crate::app::{ActivePanel, App, SidebarItem};

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.active_panel == ActivePanel::Sidebar;
    let border_color = if is_focused { app.theme.active_panel() } else { app.theme.border() };

    let block = Block::default()
        .title(" LIBRARIES ") // Updated to match Step 3
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(app.theme.bg()));

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
                        format!("{prefix}󰂺 All Books    {count}"),
                        Style::default().fg(app.theme.fg_bright()),
                    )
                }
                SidebarItem::Library { name, count } => {
                    let prefix = if is_selected { "▶ " } else { "  " };
                    (
                        format!("{prefix}󰂺 {name}    {count}"),
                        Style::default().fg(app.theme.frost_mint()),
                    )
                }
                SidebarItem::TagHeader => (
                    " ─── TAGS ───".to_string(), // Step 3 style
                    Style::default()
                        .fg(app.theme.muted())
                        .add_modifier(Modifier::DIM),
                ),
                SidebarItem::Tag { name, count } => {
                    let prefix = if is_selected { "▶ " } else { "  " };
                    (
                        format!("{prefix}󰌒 {name}    {count}"),
                        Style::default().fg(app.theme.frost_blue()),
                    )
                }
                SidebarItem::FolderHeader => (
                    " ─── FOLDERS ───".to_string(),
                    Style::default()
                        .fg(app.theme.muted())
                        .add_modifier(Modifier::DIM),
                ),
                SidebarItem::Folder { path } => {
                    let prefix = if is_selected { "▶ " } else { "  " };
                    (
                        format!("{prefix}󰉋 {path}"),
                        Style::default().fg(app.theme.green()),
                    )
                }
            };

            let item_style = if is_selected {
                style.bg(app.theme.bg_secondary()).add_modifier(Modifier::BOLD)
            } else {
                style
            };

            ListItem::new(Span::styled(text, item_style))
        })
        .collect();

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}
