use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::popup::Popup;
use super::{colors, truncate, centered_rect};

pub(crate) fn render_popup(frame: &mut Frame, popup: &Popup, area: Rect) {
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
                let x = inner.x + 13;
                let y = inner.y + 6;
                let width = (inner.width as u16).saturating_sub(15).max(20);
                let height = form.autocomplete.visible.len().min(5) as u16 + 2;

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
                let sug_height = state.autocomplete.visible.len().min(8) as u16 + 2;
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

            let dim  = Style::default().fg(colors::SUBTEXT0).add_modifier(Modifier::DIM);
            let key  = Style::default().fg(colors::LAVENDER).add_modifier(Modifier::BOLD);
            let desc = Style::default().fg(colors::TEXT);
            let head = Style::default().fg(colors::BLUE).add_modifier(Modifier::BOLD);

            macro_rules! kv {
                ($k:expr, $d:expr) => {
                    Line::from(vec![
                        Span::styled(format!("  {:<14}", $k), key),
                        Span::styled($d, desc),
                    ])
                };
            }

            let lines = vec![
                // ── Navigation ──────────────────────────────────────────
                Line::from(Span::styled(" Navigation", head)),
                kv!("[N]j / [N]k",  "Move down / up  (N = count, e.g. 5j)"),
                kv!("h / l",         "Switch panel left / right"),
                kv!("gg / G",        "Jump to top / bottom"),
                kv!("0",             "Jump to top (alternate)"),
                kv!("Ctrl-d/u",      "Half-page scroll down / up"),
                kv!("Tab / S-Tab",   "Next / previous panel"),
                Line::from(""),
                // ── Book Operations ──────────────────────────────────────
                Line::from(Span::styled(" Book Operations", head)),
                kv!("a",            "Add new book"),
                kv!("dd",           "Delete book (with confirm)"),
                kv!("o",            "Open file in viewer"),
                kv!("R / gr",       "Set rating 1–5"),
                kv!("s / gs",       "Cycle read status"),
                kv!("t",            "Edit tags"),
                kv!("yy",           "Yank book into register"),
                kv!("gt",           "Quick-edit title"),
                Line::from(""),
                // ── Vim Extras ───────────────────────────────────────────
                Line::from(Span::styled(" Vim Extras", head)),
                kv!("u",            "Undo last edit"),
                kv!("Ctrl-r",       "Redo"),
                kv!("S",            "Cycle sort order"),
                kv!("m<a-z>",       "Set named mark"),
                kv!("'<a-z>",       "Jump to named mark"),
                kv!("zz",           "Show current line number"),
                Line::from(""),
                // ── Search & Commands ────────────────────────────────────
                Line::from(Span::styled(" Search & Commands", head)),
                kv!("/",            "Telescope fuzzy search (DSL)"),
                kv!(":",            "Command mode  (:q :w :sort ...)"),
                kv!("q",            "Quit"),
                kv!("?",            "This help"),
                Line::from(""),
                // ── DSL Syntax ───────────────────────────────────────────
                Line::from(Span::styled(" Search DSL", head)),
                Line::from(Span::styled("  @author:name  #tag  y:2020-2023", desc)),
                Line::from(Span::styled("  r:>=4  s:unread  f:pdf  has:file", desc)),
                Line::from(Span::styled("  NOT #python  lib:programming", desc)),
                Line::from(""),
                Line::from(Span::styled(" Press any key to close", dim)),
            ];

            frame.render_widget(Paragraph::new(lines), inner);
        }

        Popup::SetStatus { .. } => {
            // Cycle status is handled inline by 's'
        }
    }
}
