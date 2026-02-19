use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{ActivePanel, App, Mode, SidebarItem};
use crate::popup::Popup;

/// Color palette — Catppuccin Mocha inspired defaults.
mod colors {
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
    render_body(frame, app, main_layout[1]);
    render_status_bar(frame, app, main_layout[2]);

    // Popup overlay (on top of everything)
    if let Some(ref popup) = app.popup {
        render_popup(frame, popup, size);
    }
}

// ─── Header ────────────────────────────────────────────────

fn render_header(frame: &mut Frame, app: &App, area: Rect) {
    let mode_style = match app.mode {
        Mode::Normal => Style::default().fg(colors::BLUE).add_modifier(Modifier::BOLD),
        Mode::Insert => Style::default().fg(colors::GREEN).add_modifier(Modifier::BOLD),
        Mode::Search => Style::default().fg(colors::YELLOW).add_modifier(Modifier::BOLD),
        Mode::Command => Style::default().fg(colors::PEACH).add_modifier(Modifier::BOLD),
        Mode::Visual => Style::default().fg(colors::MAUVE).add_modifier(Modifier::BOLD),
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

// ─── Body (3 panels) ───────────────────────────────────────

fn render_body(frame: &mut Frame, app: &App, area: Rect) {
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
                "  [o]pen [R]ate [s]tatus [t]ags [?]help",
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
                Line::from(vec![
                    Span::styled(" ?", Style::default().fg(colors::SUBTEXT0)),
                    Span::styled(":help  ", Style::default().fg(colors::SUBTEXT0).add_modifier(Modifier::DIM)),
                    Span::styled("/", Style::default().fg(colors::SUBTEXT0)),
                    Span::styled(":search  ", Style::default().fg(colors::SUBTEXT0).add_modifier(Modifier::DIM)),
                    Span::styled("a", Style::default().fg(colors::SUBTEXT0)),
                    Span::styled(":add  ", Style::default().fg(colors::SUBTEXT0).add_modifier(Modifier::DIM)),
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

// ─── Popup Overlay ─────────────────────────────────────────

fn render_popup(frame: &mut Frame, popup: &Popup, area: Rect) {
    match popup {
        Popup::AddBook(form) => {
            let popup_area = centered_rect(60, 50, area);
            frame.render_widget(Clear, popup_area);

            let block = Block::default()
                .title(" Add Book ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(colors::LAVENDER))
                .style(Style::default().bg(colors::BASE));

            let inner = block.inner(popup_area);
            frame.render_widget(block, popup_area);

            let mut lines = Vec::new();
            for (i, field) in form.fields.iter().enumerate() {
                let is_active = i == form.active_field;
                let label_style = if is_active {
                    Style::default().fg(colors::LAVENDER).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(colors::SUBTEXT0)
                };

                let value = if is_active {
                    format!("{}█", field.value)
                } else if field.value.is_empty() {
                    "─".to_string()
                } else {
                    field.value.clone()
                };

                let indicator = if is_active { "▶ " } else { "  " };

                lines.push(Line::from(vec![
                    Span::styled(indicator, Style::default().fg(colors::LAVENDER)),
                    Span::styled(format!("{}: ", field.label), label_style),
                    Span::styled(value, Style::default().fg(colors::TEXT)),
                ]));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  Tab: next field  Enter: submit  Esc: cancel",
                Style::default().fg(colors::SUBTEXT0).add_modifier(Modifier::DIM),
            )));

            frame.render_widget(Paragraph::new(lines), inner);

            // ── Autocomplete Dropdown ──
            if form.autocomplete.active && !form.autocomplete.visible.is_empty() {
                // Determine position
                // "File path" is index 5, so Line 5 relative to inner.
                // We want to draw it below the input line.
                // "  File path: " is approx 13 chars wide.
                let x = inner.x + 13;
                let y = inner.y + 6;
                let width = (inner.width as u16).saturating_sub(15).max(20);
                let height = form.autocomplete.visible.len().min(5) as u16 + 2; // +2 for borders

                let sug_area = Rect { x, y, width, height };
                frame.render_widget(Clear, sug_area);

                let items: Vec<ListItem> = form.autocomplete.visible.iter()
                    .enumerate()
                    .map(|(i, s)| {
                        let is_sel = form.autocomplete.selected == Some(i);
                        let style = if is_sel {
                            Style::default().bg(colors::GREEN).fg(colors::BASE).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(colors::TEXT)
                        };
                        ListItem::new(Span::styled(format!(" {s} "), style))
                    })
                    .collect();

                let block = Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(colors::GREEN))
                    .style(Style::default().bg(colors::SURFACE0));
                
                frame.render_widget(List::new(items).block(block), sug_area);
            }
        }

        Popup::DeleteConfirm { title, .. } => {
            let popup_area = centered_rect(50, 20, area);
            frame.render_widget(Clear, popup_area);

            let block = Block::default()
                .title(" Confirm Delete ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(colors::RED))
                .style(Style::default().bg(colors::BASE));

            let inner = block.inner(popup_area);
            frame.render_widget(block, popup_area);

            let lines = vec![
                Line::from(""),
                Line::from(Span::styled(
                    format!("  Delete \"{title}\"?"),
                    Style::default().fg(colors::TEXT),
                )),
                Line::from(""),
                Line::from(vec![
                    Span::styled("  [y]", Style::default().fg(colors::RED).add_modifier(Modifier::BOLD)),
                    Span::styled("es  ", Style::default().fg(colors::SUBTEXT0)),
                    Span::styled("[n]", Style::default().fg(colors::GREEN).add_modifier(Modifier::BOLD)),
                    Span::styled("o  ", Style::default().fg(colors::SUBTEXT0)),
                    Span::styled("Esc", Style::default().fg(colors::SUBTEXT0).add_modifier(Modifier::DIM)),
                    Span::styled(": cancel", Style::default().fg(colors::SUBTEXT0).add_modifier(Modifier::DIM)),
                ]),
            ];

            frame.render_widget(Paragraph::new(lines), inner);
        }

        Popup::SetRating { current, .. } => {
            let popup_area = centered_rect(40, 15, area);
            frame.render_widget(Clear, popup_area);

            let block = Block::default()
                .title(" Set Rating ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(colors::YELLOW))
                .style(Style::default().bg(colors::BASE));

            let inner = block.inner(popup_area);
            frame.render_widget(block, popup_area);

            let current_display = current
                .map(|r| "★".repeat(r as usize) + &"☆".repeat(5 - r as usize))
                .unwrap_or_else(|| "No rating".to_string());

            let lines = vec![
                Line::from(""),
                Line::from(Span::styled(
                    format!("  Current: {current_display}"),
                    Style::default().fg(colors::YELLOW),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "  Press 1-5 to set, 0 to clear",
                    Style::default().fg(colors::SUBTEXT0),
                )),
            ];

            frame.render_widget(Paragraph::new(lines), inner);
        }

        Popup::EditTags(form) => {
            let popup_area = centered_rect(55, 30, area);
            frame.render_widget(Clear, popup_area);

            let block = Block::default()
                .title(" Edit Tags ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(colors::BLUE))
                .style(Style::default().bg(colors::BASE));

            let inner = block.inner(popup_area);
            frame.render_widget(block, popup_area);

            let mut lines = vec![Line::from("")];

            // Current tags
            if form.tags.is_empty() {
                lines.push(Line::from(Span::styled(
                    "  No tags",
                    Style::default().fg(colors::SUBTEXT0),
                )));
            } else {
                let tags_display: Vec<Span> = form
                    .tags
                    .iter()
                    .flat_map(|t| {
                        vec![
                            Span::styled(format!(" #{t} "), Style::default().fg(colors::BLUE).bg(colors::SURFACE0)),
                            Span::raw(" "),
                        ]
                    })
                    .collect();
                lines.push(Line::from(vec![Span::raw("  ")]
                    .into_iter()
                    .chain(tags_display)
                    .collect::<Vec<_>>()));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("  Add tag: ", Style::default().fg(colors::SUBTEXT0)),
                Span::styled(format!("{}█", form.input), Style::default().fg(colors::TEXT)),
            ]));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  Enter: add tag  Backspace: remove  Esc: cancel  Enter(empty): save",
                Style::default().fg(colors::SUBTEXT0).add_modifier(Modifier::DIM),
            )));

            frame.render_widget(Paragraph::new(lines), inner);
        }

        Popup::Telescope(state) => {
            use crate::popup::TelescopeMode;
            
            // Near full-screen overlay (90% × 85%)
            let popup_area = centered_rect(90, 85, area);
            frame.render_widget(Clear, popup_area);

            let mode_str = match state.mode {
                TelescopeMode::Insert => " INSERT ",
                TelescopeMode::Normal => " NORMAL ",
            };

            let mode_style = match state.mode {
                TelescopeMode::Insert => Style::default().bg(colors::GREEN).fg(colors::BASE).add_modifier(Modifier::BOLD),
                TelescopeMode::Normal => Style::default().bg(colors::BLUE).fg(colors::BASE).add_modifier(Modifier::BOLD),
            };

            let block = Block::default()
                .title(Line::from(vec![
                    Span::styled(" 🔭 SEARCH ", Style::default().fg(colors::LAVENDER)),
                    Span::styled(mode_str, mode_style),
                ]))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(if state.mode == TelescopeMode::Insert { colors::GREEN } else { colors::BLUE }))
                .style(Style::default().bg(colors::BASE));

            let inner = block.inner(popup_area);
            frame.render_widget(block, popup_area);

            // Layout: search bar + filter chips + results/preview
            let telescope_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1), // search input
                    Constraint::Length(1), // filter chips + help
                    Constraint::Min(3),   // results + preview
                    Constraint::Length(1), // footer
                ])
                .split(inner);

            // ── Search input ──
            let cursor_style = if state.mode == TelescopeMode::Insert {
                Style::default().fg(colors::TEXT).add_modifier(Modifier::SLOW_BLINK)
            } else {
                Style::default().fg(colors::TEXT)
            };
            
            let mut query_spans = vec![
                Span::styled("  > ", Style::default().fg(colors::LAVENDER).add_modifier(Modifier::BOLD)),
            ];
            
            // Split query at cursor to insert cursor block
            if state.cursor >= state.query.len() {
                query_spans.push(Span::styled(&state.query, Style::default().fg(colors::TEXT)));
                if state.mode == TelescopeMode::Insert {
                    query_spans.push(Span::styled("█", cursor_style));
                }
            } else {
                let before = &state.query[..state.cursor];
                let cursor_char = state.query[state.cursor..].chars().next().unwrap().to_string();
                let after = &state.query[state.cursor + cursor_char.len()..];
                
                query_spans.push(Span::styled(before, Style::default().fg(colors::TEXT)));
                if state.mode == TelescopeMode::Insert {
                    query_spans.push(Span::styled(cursor_char.clone(), cursor_style.bg(colors::SURFACE1)));
                } else {
                    query_spans.push(Span::styled(cursor_char, Style::default().fg(colors::TEXT)));
                }
                query_spans.push(Span::styled(after, Style::default().fg(colors::TEXT)));
            }

            frame.render_widget(Paragraph::new(Line::from(query_spans)), telescope_layout[0]);

            // ── Filter chips ──
            let mut chips: Vec<Span> = vec![Span::raw("  ")];
            if state.active_filters.is_empty() {
                chips.push(Span::styled(
                    "Try: @author:name  #tag  y:2020-2023  r:>=4  s:unread  f:pdf  has:file",
                    Style::default().fg(colors::SURFACE1),
                ));
            } else {
                for chip in &state.active_filters {
                    chips.push(Span::styled(
                        format!(" {chip} "),
                        Style::default().fg(colors::BASE).bg(colors::BLUE),
                    ));
                    chips.push(Span::raw(" "));
                }
            }
            frame.render_widget(Paragraph::new(Line::from(chips)), telescope_layout[1]);

            // ── Results + Preview split ──
            let results_preview = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(55),
                    Constraint::Percentage(45),
                ])
                .split(telescope_layout[2]);

            // Results list
            let results_area = results_preview[0];
            let result_count = state.results.len();
            let visible_h = results_area.height as usize;
            // Use state.scroll that we added to TelescopeState
            let scroll_off = state.scroll;

            let result_items: Vec<ListItem> = state.results.iter()
                .enumerate()
                .skip(scroll_off)
                .take(visible_h)
                .map(|(i, book)| {
                    let is_sel = i == state.selected;
                    let prefix = if is_sel { "▶ " } else { "  " };

                    let status_icon = match book.read_status {
                        omniscope_core::ReadStatus::Read => "✓",
                        omniscope_core::ReadStatus::Reading => "◐",
                        omniscope_core::ReadStatus::Dnf => "✗",
                        omniscope_core::ReadStatus::Unread => "○",
                    };

                    let rating_str = match book.rating {
                        Some(5) => "★★★★★",
                        Some(4) => "★★★★☆",
                        Some(3) => "★★★☆☆",
                        Some(2) => "★★☆☆☆",
                        Some(1) => "★☆☆☆☆",
                        _ => "",
                    };

                    let max_title = (results_area.width as usize).saturating_sub(25);
                    let tags_str = if !book.tags.is_empty() {
                        book.tags.iter().take(3).map(|t| format!("#{t}")).collect::<Vec<_>>().join(" ")
                    } else {
                        String::new()
                    };

                    let year = book.year.map(|y| y.to_string()).unwrap_or_default();

                    let line = Line::from(vec![
                        Span::styled(prefix, Style::default().fg(colors::LAVENDER)),
                        Span::styled(status_icon, Style::default().fg(match book.read_status {
                            omniscope_core::ReadStatus::Read => colors::GREEN,
                            omniscope_core::ReadStatus::Reading => colors::YELLOW,
                            omniscope_core::ReadStatus::Dnf => colors::RED,
                            omniscope_core::ReadStatus::Unread => colors::SURFACE1,
                        })),
                        Span::raw(" "),
                        Span::styled(truncate(&book.title, max_title), Style::default().fg(colors::TEXT)),
                        Span::raw("  "),
                        Span::styled(tags_str, Style::default().fg(colors::BLUE).add_modifier(Modifier::DIM)),
                        Span::raw("  "),
                        Span::styled(year, Style::default().fg(colors::SUBTEXT0)),
                        Span::raw(" "),
                        Span::styled(rating_str, Style::default().fg(colors::YELLOW)),
                    ]);

                    let style = if is_sel {
                        Style::default().bg(colors::SURFACE0)
                    } else {
                        Style::default()
                    };

                    ListItem::new(line).style(style)
                })
                .collect();

            let results_title = format!(" Results ({result_count}) ");
            let results_block = Block::default()
                .title(results_title)
                .borders(Borders::RIGHT)
                .border_style(Style::default().fg(colors::SURFACE1));

            let rlist = List::new(result_items).block(results_block);
            frame.render_widget(rlist, results_area);

            // ── Preview panel (right side) ──
            let preview_area = results_preview[1];
            let preview_content = if let Some(book) = state.selected_result() {
                let authors = if book.authors.is_empty() { "—".to_string() } else { book.authors.join(", ") };
                let year = book.year.map(|y| y.to_string()).unwrap_or("—".to_string());
                let tags = if book.tags.is_empty() { "—".to_string() } else {
                    book.tags.iter().map(|t| format!("#{t}")).collect::<Vec<_>>().join(" ")
                };
                let rating = match book.rating {
                    Some(r) => "★".repeat(r as usize) + &"☆".repeat(5 - r as usize),
                    None => "—".to_string(),
                };

                vec![
                    Line::from(""),
                    Line::from(Span::styled(format!("  📖 {}", book.title), Style::default().fg(colors::LAVENDER).add_modifier(Modifier::BOLD))),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("  By:     ", Style::default().fg(colors::SUBTEXT0)),
                        Span::styled(authors, Style::default().fg(colors::TEXT)),
                    ]),
                    Line::from(vec![
                        Span::styled("  Year:   ", Style::default().fg(colors::SUBTEXT0)),
                        Span::styled(year, Style::default().fg(colors::TEXT)),
                    ]),
                    Line::from(vec![
                        Span::styled("  Rating: ", Style::default().fg(colors::SUBTEXT0)),
                        Span::styled(rating, Style::default().fg(colors::YELLOW)),
                    ]),
                    Line::from(vec![
                        Span::styled("  Status: ", Style::default().fg(colors::SUBTEXT0)),
                        Span::styled(book.read_status.to_string(), Style::default().fg(match book.read_status {
                            omniscope_core::ReadStatus::Read => colors::GREEN,
                            omniscope_core::ReadStatus::Reading => colors::YELLOW,
                            omniscope_core::ReadStatus::Dnf => colors::RED,
                            omniscope_core::ReadStatus::Unread => colors::SUBTEXT0,
                        })),
                    ]),
                    Line::from(vec![
                        Span::styled("  Tags:   ", Style::default().fg(colors::SUBTEXT0)),
                        Span::styled(tags, Style::default().fg(colors::BLUE)),
                    ]),
                ]
            } else {
                vec![
                    Line::from(""),
                    Line::from(Span::styled("  No results", Style::default().fg(colors::SUBTEXT0))),
                ]
            };

            frame.render_widget(Paragraph::new(preview_content), preview_area);

            // ── Autocomplete suggestions (overlay) ──
            if state.autocomplete.active && !state.autocomplete.visible.is_empty() {
                // Determine X position based on cursor logic (approximate)
                // We'll place it slightly offset from the start for now
                let sug_height = state.autocomplete.visible.len().min(8) as u16 + 2; // +2 for borders
                let sug_area = Rect {
                    x: telescope_layout[0].x + 4,
                    y: telescope_layout[0].y + 1,
                    width: 45.min(inner.width),
                    height: sug_height,
                };
                frame.render_widget(Clear, sug_area);

                let sug_items: Vec<ListItem> = state.autocomplete.visible.iter()
                    .enumerate()
                    .map(|(i, s)| {
                        let is_sel = state.autocomplete.selected == Some(i);
                        let style = if is_sel {
                            Style::default().bg(colors::GREEN).fg(colors::BASE).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(colors::TEXT)
                        };
                        ListItem::new(Span::styled(format!("  {s}"), style))
                    })
                    .collect();

                let sug_block = Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(colors::GREEN))
                    .style(Style::default().bg(colors::SURFACE0));
                
                frame.render_widget(List::new(sug_items).block(sug_block), sug_area);
            }

            // ── Footer ──
            let footer = match state.mode {
                TelescopeMode::Insert => Line::from(vec![
                    Span::styled("  Esc", Style::default().fg(colors::LAVENDER)),
                    Span::styled(": normal  ", Style::default().fg(colors::SUBTEXT0).add_modifier(Modifier::DIM)),
                    Span::styled("Tab", Style::default().fg(colors::LAVENDER)),
                    Span::styled(": next  ", Style::default().fg(colors::SUBTEXT0).add_modifier(Modifier::DIM)),
                    Span::styled("Enter", Style::default().fg(colors::LAVENDER)),
                    Span::styled(": select  ", Style::default().fg(colors::SUBTEXT0).add_modifier(Modifier::DIM)),
                ]),
                TelescopeMode::Normal => Line::from(vec![
                    Span::styled("  i", Style::default().fg(colors::LAVENDER)),
                    Span::styled(": insert  ", Style::default().fg(colors::SUBTEXT0).add_modifier(Modifier::DIM)),
                    Span::styled("j/k", Style::default().fg(colors::LAVENDER)),
                    Span::styled(": navigate  ", Style::default().fg(colors::SUBTEXT0).add_modifier(Modifier::DIM)),
                    Span::styled("Enter", Style::default().fg(colors::LAVENDER)),
                    Span::styled(": open  ", Style::default().fg(colors::SUBTEXT0).add_modifier(Modifier::DIM)),
                    Span::styled("q/Esc", Style::default().fg(colors::LAVENDER)),
                    Span::styled(": close", Style::default().fg(colors::SUBTEXT0).add_modifier(Modifier::DIM)),
                ]),
            };
            frame.render_widget(Paragraph::new(footer), telescope_layout[3]);
        }

        Popup::Help => {
            let popup_area = centered_rect(65, 80, area);
            frame.render_widget(Clear, popup_area);

            let block = Block::default()
                .title(" Omniscope — Help ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(colors::LAVENDER))
                .style(Style::default().bg(colors::BASE));

            let inner = block.inner(popup_area);
            frame.render_widget(block, popup_area);

            let lines = vec![
                Line::from(Span::styled(" Navigation", Style::default().fg(colors::LAVENDER).add_modifier(Modifier::BOLD))),
                Line::from(Span::styled("  j/k        Move down/up", Style::default().fg(colors::TEXT))),
                Line::from(Span::styled("  h/l        Switch panels left/right", Style::default().fg(colors::TEXT))),
                Line::from(Span::styled("  gg/G       Jump to top/bottom", Style::default().fg(colors::TEXT))),
                Line::from(Span::styled("  Ctrl-d/u   Half-page scroll", Style::default().fg(colors::TEXT))),
                Line::from(Span::styled("  Tab        Next panel", Style::default().fg(colors::TEXT))),
                Line::from(""),
                Line::from(Span::styled(" Book Operations", Style::default().fg(colors::LAVENDER).add_modifier(Modifier::BOLD))),
                Line::from(Span::styled("  a          Add new book", Style::default().fg(colors::TEXT))),
                Line::from(Span::styled("  d          Delete book", Style::default().fg(colors::TEXT))),
                Line::from(Span::styled("  o          Open in viewer", Style::default().fg(colors::TEXT))),
                Line::from(Span::styled("  R          Set rating (1-5)", Style::default().fg(colors::TEXT))),
                Line::from(Span::styled("  s          Cycle read status", Style::default().fg(colors::TEXT))),
                Line::from(Span::styled("  t          Edit tags", Style::default().fg(colors::TEXT))),
                Line::from(Span::styled("  y          Yank file path", Style::default().fg(colors::TEXT))),
                Line::from(Span::styled("  v          Visual select", Style::default().fg(colors::TEXT))),
                Line::from(""),
                Line::from(Span::styled(" Search & Commands", Style::default().fg(colors::LAVENDER).add_modifier(Modifier::BOLD))),
                Line::from(Span::styled("  /          Telescope search (DSL)", Style::default().fg(colors::TEXT))),
                Line::from(Span::styled("  :          Command mode", Style::default().fg(colors::TEXT))),
                Line::from(Span::styled("  q          Quit", Style::default().fg(colors::TEXT))),
                Line::from(""),
                Line::from(Span::styled(" DSL Syntax", Style::default().fg(colors::LAVENDER).add_modifier(Modifier::BOLD))),
                Line::from(Span::styled("  @author:name  #tag  y:2020-2023", Style::default().fg(colors::TEXT))),
                Line::from(Span::styled("  r:>=4  s:unread  f:pdf  has:file", Style::default().fg(colors::TEXT))),
                Line::from(Span::styled("  NOT #python  lib:programming", Style::default().fg(colors::TEXT))),
                Line::from(""),
                Line::from(Span::styled(" Press any key to close", Style::default().fg(colors::SUBTEXT0).add_modifier(Modifier::DIM))),
            ];

            frame.render_widget(Paragraph::new(lines), inner);
        }

        Popup::SetStatus { .. } => {
            // Cycle status is handled inline by 's'
        }
    }
}

// ─── Helpers ───────────────────────────────────────────────

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        format!("{s:<max$}")
    } else {
        let truncated: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{truncated}…")
    }
}

/// Create a centered rectangle.
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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
