use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};

use super::centered_rect;
use crate::app::App;
use crate::popup::Popup;
use crate::ui::overlays;

pub(crate) fn render_popup(frame: &mut Frame, app: &App, popup: &Popup, area: Rect) {
    match popup {
        Popup::AddBook(form) => {
            let popup_area = centered_rect(60, 50, area);
            frame.render_widget(Clear, popup_area);

            let block = Block::default()
                .title(" Add Book ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.theme.frost_blue()))
                .style(Style::default().bg(app.theme.bg()));

            let inner = block.inner(popup_area);
            frame.render_widget(block, popup_area);

            let mut lines = Vec::new();
            for (i, field) in form.fields.iter().enumerate() {
                let is_active = i == form.active_field;
                let label_style = if is_active {
                    Style::default()
                        .fg(app.theme.frost_ice())
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(app.theme.muted())
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
                    Span::styled(indicator, Style::default().fg(app.theme.frost_ice())),
                    Span::styled(format!("{}: ", field.label), label_style),
                    Span::styled(value, Style::default().fg(app.theme.fg())),
                ]));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  Tab: next field  Enter: submit  Esc: cancel",
                Style::default()
                    .fg(app.theme.muted())
                    .add_modifier(Modifier::DIM),
            )));

            frame.render_widget(Paragraph::new(lines), inner);

            // ── Autocomplete Dropdown ──
            if form.autocomplete.active && !form.autocomplete.visible.is_empty() {
                let x = inner.x + 13;
                let y = inner.y + 6;
                let width = (inner.width as u16).saturating_sub(15).max(20);
                let height = form.autocomplete.visible.len().min(5) as u16 + 2;

                let sug_area = Rect {
                    x,
                    y,
                    width,
                    height,
                };
                frame.render_widget(Clear, sug_area);

                let items: Vec<ListItem> = form
                    .autocomplete
                    .visible
                    .iter()
                    .enumerate()
                    .map(|(i, s)| {
                        let is_sel = form.autocomplete.selected == Some(i);
                        let style = if is_sel {
                            Style::default()
                                .bg(app.theme.green())
                                .fg(app.theme.bg())
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(app.theme.fg())
                        };
                        ListItem::new(Span::styled(format!(" {s} "), style))
                    })
                    .collect();

                let block = Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(app.theme.green()))
                    .style(Style::default().bg(app.theme.bg_secondary()));

                frame.render_widget(List::new(items).block(block), sug_area);
            }
        }

        Popup::DeleteConfirm { title, .. } => {
            let popup_area = centered_rect(50, 20, area);
            frame.render_widget(Clear, popup_area);

            let block = Block::default()
                .title(" Confirm Delete ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.theme.red()))
                .style(Style::default().bg(app.theme.bg()));

            let inner = block.inner(popup_area);
            frame.render_widget(block, popup_area);

            let lines = vec![
                Line::from(""),
                Line::from(Span::styled(
                    format!("  Delete \"{title}\"?"),
                    Style::default().fg(app.theme.fg()),
                )),
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        "  [y]",
                        Style::default()
                            .fg(app.theme.red())
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("es  ", Style::default().fg(app.theme.muted())),
                    Span::styled(
                        "[n]",
                        Style::default()
                            .fg(app.theme.green())
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("o  ", Style::default().fg(app.theme.muted())),
                    Span::styled(
                        "Esc",
                        Style::default()
                            .fg(app.theme.muted())
                            .add_modifier(Modifier::DIM),
                    ),
                    Span::styled(
                        ": cancel",
                        Style::default()
                            .fg(app.theme.muted())
                            .add_modifier(Modifier::DIM),
                    ),
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
                .border_style(Style::default().fg(app.theme.star_color()))
                .style(Style::default().bg(app.theme.bg()));

            let inner = block.inner(popup_area);
            frame.render_widget(block, popup_area);

            let current_display = current
                .map(|r| "★".repeat(r as usize) + &"☆".repeat(5 - r as usize))
                .unwrap_or_else(|| "No rating".to_string());

            let lines = vec![
                Line::from(""),
                Line::from(Span::styled(
                    format!("  Current: {current_display}"),
                    Style::default().fg(app.theme.star_color()),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "  Press 1-5 to set, 0 to clear",
                    Style::default().fg(app.theme.muted()),
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
                .border_style(Style::default().fg(app.theme.frost_blue()))
                .style(Style::default().bg(app.theme.bg()));

            let inner = block.inner(popup_area);
            frame.render_widget(block, popup_area);

            let mut lines = vec![Line::from("")];

            if form.tags.is_empty() {
                lines.push(Line::from(Span::styled(
                    "  No tags",
                    Style::default().fg(app.theme.muted()),
                )));
            } else {
                let tags_display: Vec<Span> = form
                    .tags
                    .iter()
                    .flat_map(|t| {
                        vec![
                            Span::styled(
                                format!(" #{t} "),
                                Style::default()
                                    .fg(app.theme.frost_blue())
                                    .bg(app.theme.bg_secondary()),
                            ),
                            Span::raw(" "),
                        ]
                    })
                    .collect();
                lines.push(Line::from(
                    vec![Span::raw("  ")]
                        .into_iter()
                        .chain(tags_display)
                        .collect::<Vec<_>>(),
                ));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("  Add tag: ", Style::default().fg(app.theme.muted())),
                Span::styled(
                    format!("{}█", form.input),
                    Style::default().fg(app.theme.fg_bright()),
                ),
            ]));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  Enter: add tag  Backspace: remove  Esc: cancel  Enter(empty): save",
                Style::default()
                    .fg(app.theme.muted())
                    .add_modifier(Modifier::DIM),
            )));
            frame.render_widget(Paragraph::new(lines), inner);
        }

        Popup::Telescope(_) => {
            overlays::telescope::render(frame, app, area);
        }

        Popup::Help => {
            overlays::help::render(frame, app, area);
        }

        Popup::Marks => {
            overlays::marks::render(frame, app, area);
        }

        Popup::Registers => {
            overlays::registers::render(frame, app, area);
        }

        Popup::ScienceReferences { panel, book_title } => {
            let popup_area = centered_rect(94, 90, area);
            frame.render_widget(Clear, popup_area);

            let mut cloned = panel.clone();
            cloned.render(frame, popup_area, &app.theme, book_title);
        }

        Popup::ScienceCitationGraph(panel) => {
            let popup_area = centered_rect(94, 90, area);
            frame.render_widget(Clear, popup_area);

            let mut cloned = panel.clone();
            cloned.render(frame, popup_area, &app.theme);
        }

        Popup::ScienceFindDownload(panel) => {
            let popup_area = centered_rect(94, 90, area);
            frame.render_widget(Clear, popup_area);

            let mut cloned = panel.clone();
            cloned.render(frame, popup_area, &app.theme);
        }

        Popup::TextViewer {
            title,
            body,
            scroll,
        } => {
            let popup_area = centered_rect(82, 76, area);
            frame.render_widget(Clear, popup_area);

            let block = Block::default()
                .title(format!(" {title} "))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.theme.frost_blue()))
                .style(Style::default().bg(app.theme.bg()));
            let inner = block.inner(popup_area);
            frame.render_widget(block, popup_area);

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(2), Constraint::Length(1)])
                .split(inner);

            let mut lines = body
                .lines()
                .map(|line| {
                    Line::from(Span::styled(
                        line.to_string(),
                        Style::default().fg(app.theme.fg()),
                    ))
                })
                .collect::<Vec<_>>();

            if lines.is_empty() {
                lines.push(Line::from(Span::styled(
                    "(empty)",
                    Style::default()
                        .fg(app.theme.muted())
                        .add_modifier(Modifier::DIM),
                )));
            }

            let body_height = usize::from(chunks[0].height);
            let max_scroll = lines.len().saturating_sub(body_height);
            let start = (*scroll).min(max_scroll);
            let visible = lines
                .into_iter()
                .skip(start)
                .take(body_height)
                .collect::<Vec<_>>();

            frame.render_widget(Paragraph::new(visible), chunks[0]);
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(
                        "[j/k]",
                        Style::default()
                            .fg(app.theme.yellow())
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        " scroll  ",
                        Style::default()
                            .fg(app.theme.muted())
                            .add_modifier(Modifier::DIM),
                    ),
                    Span::styled(
                        "[g/G]",
                        Style::default()
                            .fg(app.theme.yellow())
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        " top/bottom  ",
                        Style::default()
                            .fg(app.theme.muted())
                            .add_modifier(Modifier::DIM),
                    ),
                    Span::styled(
                        "[Esc]",
                        Style::default()
                            .fg(app.theme.yellow())
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        " close",
                        Style::default()
                            .fg(app.theme.muted())
                            .add_modifier(Modifier::DIM),
                    ),
                ])),
                chunks[1],
            );
        }

        Popup::SetStatus { current, .. } => {
            let popup_area = centered_rect(45, 20, area);
            frame.render_widget(Clear, popup_area);

            let block = Block::default()
                .title(" Set Status ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.theme.frost_blue()))
                .style(Style::default().bg(app.theme.bg()));

            let inner = block.inner(popup_area);
            frame.render_widget(block, popup_area);

            let statuses = [
                ("1", "○ Unread", omniscope_core::ReadStatus::Unread),
                ("2", "● Reading", omniscope_core::ReadStatus::Reading),
                ("3", "✓ Read", omniscope_core::ReadStatus::Read),
                ("4", "✕ DNF", omniscope_core::ReadStatus::Dnf),
            ];

            let mut lines = vec![Line::from("")];
            lines.push(Line::from(Span::styled(
                format!("  Current: {current}"),
                Style::default().fg(app.theme.fg()),
            )));
            lines.push(Line::from(""));
            for (key, label, status) in &statuses {
                let is_current = current == status;
                let style = if is_current {
                    Style::default()
                        .fg(app.theme.frost_ice())
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(app.theme.fg())
                };
                let marker = if is_current { " ◀" } else { "" };
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("  [{key}] "),
                        Style::default().fg(app.theme.muted()),
                    ),
                    Span::styled(format!("{label}{marker}"), style),
                ]));
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  Space: cycle  Esc: cancel",
                Style::default()
                    .fg(app.theme.muted())
                    .add_modifier(Modifier::DIM),
            )));

            frame.render_widget(Paragraph::new(lines), inner);
        }

        Popup::EasyMotion(state) => {
            // EasyMotion: render labels as an overlay on the book list
            let popup_area = centered_rect(60, 60, area);
            frame.render_widget(Clear, popup_area);

            let block = Block::default()
                .title(if state.pending {
                    " EasyMotion: type first letter "
                } else {
                    " EasyMotion: type label "
                })
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.theme.yellow()))
                .style(Style::default().bg(app.theme.bg()));

            let inner = block.inner(popup_area);
            frame.render_widget(block, popup_area);

            if state.pending {
                let lines = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Type the first letter of a book title...",
                        Style::default().fg(app.theme.muted()),
                    )),
                ];
                frame.render_widget(Paragraph::new(lines), inner);
            } else {
                // Show target labels
                let items: Vec<ListItem> = state
                    .targets
                    .iter()
                    .map(|&(label, idx)| {
                        let title = app
                            .books
                            .get(idx)
                            .map(|b| b.title.as_str())
                            .unwrap_or("???");
                        let line = Line::from(vec![
                            Span::styled(
                                format!(" {label} "),
                                Style::default()
                                    .fg(app.theme.bg())
                                    .bg(app.theme.yellow())
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::raw("  "),
                            Span::styled(title, Style::default().fg(app.theme.fg())),
                        ]);
                        ListItem::new(line)
                    })
                    .collect();

                frame.render_widget(List::new(items), inner);
            }
        }

        Popup::EditYear { input, .. } => {
            let popup_area = centered_rect(40, 12, area);
            frame.render_widget(Clear, popup_area);

            let block = Block::default()
                .title(" Edit Year ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.theme.frost_blue()))
                .style(Style::default().bg(app.theme.bg()));

            let inner = block.inner(popup_area);
            frame.render_widget(block, popup_area);

            let lines = vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled("  Year: ", Style::default().fg(app.theme.muted())),
                    Span::styled(
                        format!("{input}█"),
                        Style::default().fg(app.theme.fg_bright()),
                    ),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "  Enter: save  Esc: cancel",
                    Style::default()
                        .fg(app.theme.muted())
                        .add_modifier(Modifier::DIM),
                )),
            ];
            frame.render_widget(Paragraph::new(lines), inner);
        }

        Popup::EditAuthors { input, .. } => {
            let popup_area = centered_rect(55, 12, area);
            frame.render_widget(Clear, popup_area);

            let block = Block::default()
                .title(" Edit Authors ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.theme.frost_blue()))
                .style(Style::default().bg(app.theme.bg()));

            let inner = block.inner(popup_area);
            frame.render_widget(block, popup_area);

            let lines = vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled("  Authors: ", Style::default().fg(app.theme.muted())),
                    Span::styled(
                        format!("{input}█"),
                        Style::default().fg(app.theme.fg_bright()),
                    ),
                ]),
                Line::from(Span::styled(
                    "  (comma-separated)",
                    Style::default()
                        .fg(app.theme.muted())
                        .add_modifier(Modifier::DIM),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "  Enter: save  Esc: cancel",
                    Style::default()
                        .fg(app.theme.muted())
                        .add_modifier(Modifier::DIM),
                )),
            ];
            frame.render_widget(Paragraph::new(lines), inner);
        }

        Popup::EditDoi { input, .. } => {
            let popup_area = centered_rect(62, 12, area);
            frame.render_widget(Clear, popup_area);

            let block = Block::default()
                .title(" Edit DOI ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.theme.frost_blue()))
                .style(Style::default().bg(app.theme.bg()));
            let inner = block.inner(popup_area);
            frame.render_widget(block, popup_area);

            let lines = vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled("  DOI: ", Style::default().fg(app.theme.muted())),
                    Span::styled(
                        format!("{input}█"),
                        Style::default().fg(app.theme.fg_bright()),
                    ),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "  Enter: save  Esc: cancel",
                    Style::default()
                        .fg(app.theme.muted())
                        .add_modifier(Modifier::DIM),
                )),
            ];
            frame.render_widget(Paragraph::new(lines), inner);
        }

        Popup::EditArxivId { input, .. } => {
            let popup_area = centered_rect(62, 12, area);
            frame.render_widget(Clear, popup_area);

            let block = Block::default()
                .title(" Edit arXiv ID ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.theme.frost_blue()))
                .style(Style::default().bg(app.theme.bg()));
            let inner = block.inner(popup_area);
            frame.render_widget(block, popup_area);

            let lines = vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled("  arXiv: ", Style::default().fg(app.theme.muted())),
                    Span::styled(
                        format!("{input}█"),
                        Style::default().fg(app.theme.fg_bright()),
                    ),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "  Enter: save  Esc: cancel",
                    Style::default()
                        .fg(app.theme.muted())
                        .add_modifier(Modifier::DIM),
                )),
            ];
            frame.render_widget(Paragraph::new(lines), inner);
        }

        Popup::AddTagPrompt { input, indices, .. } => {
            let popup_area = centered_rect(45, 12, area);
            frame.render_widget(Clear, popup_area);

            let block = Block::default()
                .title(format!(" Add Tag ({} books) ", indices.len()))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.theme.green()))
                .style(Style::default().bg(app.theme.bg()));

            let inner = block.inner(popup_area);
            frame.render_widget(block, popup_area);

            let lines = vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled("  Tag: ", Style::default().fg(app.theme.muted())),
                    Span::styled(
                        format!("{input}█"),
                        Style::default().fg(app.theme.fg_bright()),
                    ),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "  Enter: add  Esc: cancel",
                    Style::default()
                        .fg(app.theme.muted())
                        .add_modifier(Modifier::DIM),
                )),
            ];
            frame.render_widget(Paragraph::new(lines), inner);
        }

        Popup::RemoveTagPrompt {
            available_tags,
            selected,
            indices,
            ..
        } => {
            let popup_area = centered_rect(45, 25, area);
            frame.render_widget(Clear, popup_area);

            let block = Block::default()
                .title(format!(" Remove Tag ({} books) ", indices.len()))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.theme.red()))
                .style(Style::default().bg(app.theme.bg()));

            let inner = block.inner(popup_area);
            frame.render_widget(block, popup_area);

            let items: Vec<ListItem> = available_tags
                .iter()
                .enumerate()
                .map(|(i, tag)| {
                    let is_sel = i == *selected;
                    let style = if is_sel {
                        Style::default()
                            .bg(app.theme.red())
                            .fg(app.theme.bg())
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(app.theme.fg())
                    };
                    ListItem::new(Span::styled(format!("  #{tag}  "), style))
                })
                .collect();

            frame.render_widget(List::new(items), inner);
        }
    }
}
