use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::popup::Popup;
use super::centered_rect;
use crate::app::App;
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
                    Style::default().fg(app.theme.frost_ice()).add_modifier(Modifier::BOLD)
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
                Style::default().fg(app.theme.muted()).add_modifier(Modifier::DIM),
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
                            Style::default().bg(app.theme.green()).fg(app.theme.bg()).add_modifier(Modifier::BOLD)
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
                    Span::styled("  [y]", Style::default().fg(app.theme.red()).add_modifier(Modifier::BOLD)),
                    Span::styled("es  ", Style::default().fg(app.theme.muted())),
                    Span::styled("[n]", Style::default().fg(app.theme.green()).add_modifier(Modifier::BOLD)),
                    Span::styled("o  ", Style::default().fg(app.theme.muted())),
                    Span::styled("Esc", Style::default().fg(app.theme.muted()).add_modifier(Modifier::DIM)),
                    Span::styled(": cancel", Style::default().fg(app.theme.muted()).add_modifier(Modifier::DIM)),
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
                lines.push(Line::from(Span::styled("  No tags", Style::default().fg(app.theme.muted()))));
            } else {
                let tags_display: Vec<Span> = form.tags.iter().flat_map(|t| {
                    vec![
                        Span::styled(format!(" #{t} "), Style::default().fg(app.theme.frost_blue()).bg(app.theme.bg_secondary())),
                        Span::raw(" "),
                    ]
                }).collect();
                lines.push(Line::from(vec![Span::raw("  ")].into_iter().chain(tags_display).collect::<Vec<_>>()));
            }
            
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("  Add tag: ", Style::default().fg(app.theme.muted())),
                Span::styled(format!("{}█", form.input), Style::default().fg(app.theme.fg_bright())),
            ]));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  Enter: add tag  Backspace: remove  Esc: cancel  Enter(empty): save",
                Style::default().fg(app.theme.muted()).add_modifier(Modifier::DIM),
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

        Popup::SetStatus { .. } => {}
        Popup::EasyMotion(_) => {}
    }
}
