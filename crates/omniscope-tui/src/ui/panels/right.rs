use omniscope_core::BookSummaryView;
use omniscope_core::models::BookCard;
use omniscope_core::storage::json_cards;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::app::{ActivePanel, App};
use crate::panels::article_card;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    if area.is_empty() {
        return;
    }

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

    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.is_empty() {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(5)])
        .split(inner);

    let body_width = usize::from(chunks[0].width.saturating_sub(1)).max(20);
    let content = if let Some(book) = app.selected_book() {
        let maybe_card = json_cards::load_card_by_id(&app.cards_dir(), &book.id).ok();
        if let Some(card) = maybe_card
            .as_ref()
            .filter(|card| article_card::is_scientific_article(card))
        {
            article_card::build_preview_lines(card, body_width, &app.theme)
        } else {
            render_default_preview(book, maybe_card.as_ref(), body_width, app)
        }
    } else {
        vec![
            Line::from(""),
            Line::from(Span::styled(
                "  Select a book to preview",
                Style::default().fg(app.theme.muted()),
            )),
        ]
    };

    let body_height = usize::from(chunks[0].height);
    let max_scroll = content.len().saturating_sub(body_height);
    let start = app.preview_scroll.min(max_scroll);
    let visible = content
        .into_iter()
        .skip(start)
        .take(body_height)
        .collect::<Vec<_>>();

    frame.render_widget(Paragraph::new(visible), chunks[0]);
    frame.render_widget(hints_block(app, body_width, start, max_scroll), chunks[1]);
}

fn render_default_preview(
    book: &BookSummaryView,
    card: Option<&BookCard>,
    body_width: usize,
    app: &App,
) -> Vec<Line<'static>> {
    let authors = if book.authors.is_empty() {
        "Unknown".to_string()
    } else {
        book.authors.join(", ")
    };

    let year = book
        .year
        .map(|y| y.to_string())
        .unwrap_or_else(|| "—".to_string());
    let pages = card
        .and_then(|value| {
            value.metadata.pages.or_else(|| {
                value
                    .publication
                    .as_ref()
                    .and_then(|publication| publication.pages.as_deref())
                    .and_then(parse_pages_from_publication)
            })
        })
        .map(|value| value.to_string())
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
            format!("  {}", "─".repeat(body_width.saturating_sub(4).max(1))),
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
                pages,
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
                book.has_file
                    .then_some("file attached")
                    .unwrap_or("no file")
            ),
            Style::default().fg(app.theme.frost_mint()),
        )),
    ]
}

fn hints_block(app: &App, width: usize, start: usize, max_scroll: usize) -> Paragraph<'static> {
    let scroll_label = if max_scroll == 0 {
        "scroll: 1/1".to_string()
    } else {
        format!("scroll: {}/{}", start + 1, max_scroll + 1)
    };

    Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                " Hints ",
                Style::default()
                    .fg(app.theme.frost_ice())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "  Preview panel controls",
                Style::default().fg(app.theme.muted()),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                " [j/k] ",
                Style::default()
                    .fg(app.theme.yellow())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("scroll  ", Style::default().fg(app.theme.muted())),
            Span::styled(
                " [h/l] ",
                Style::default()
                    .fg(app.theme.yellow())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("focus  ", Style::default().fg(app.theme.muted())),
            Span::styled(
                " [r] ",
                Style::default()
                    .fg(app.theme.yellow())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("refs  ", Style::default().fg(app.theme.muted())),
            Span::styled(
                " [c] ",
                Style::default()
                    .fg(app.theme.yellow())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("cited-by  ", Style::default().fg(app.theme.muted())),
            Span::styled(
                " [f] ",
                Style::default()
                    .fg(app.theme.yellow())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("find/download", Style::default().fg(app.theme.muted())),
        ]),
        Line::from(vec![
            Span::styled(
                " [o] ",
                Style::default()
                    .fg(app.theme.yellow())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("open PDF  ", Style::default().fg(app.theme.muted())),
            Span::styled(
                " [e] ",
                Style::default()
                    .fg(app.theme.yellow())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("BibTeX  ", Style::default().fg(app.theme.muted())),
            Span::styled(
                " [gr] ",
                Style::default()
                    .fg(app.theme.yellow())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("refs  ", Style::default().fg(app.theme.muted())),
            Span::styled(
                " [gR] ",
                Style::default()
                    .fg(app.theme.yellow())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("cited  ", Style::default().fg(app.theme.muted())),
            Span::styled(
                " [gs] ",
                Style::default()
                    .fg(app.theme.yellow())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("related", Style::default().fg(app.theme.muted())),
        ]),
        Line::from(vec![
            Span::styled(
                truncate_text(
                    " [@m] metadata  [@e] AI metadata  [@r] AI references",
                    width.saturating_sub(16),
                ),
                Style::default()
                    .fg(app.theme.yellow())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                scroll_label,
                Style::default()
                    .fg(app.theme.muted())
                    .add_modifier(Modifier::DIM),
            ),
        ]),
    ])
    .wrap(Wrap { trim: true })
    .block(
        Block::default().borders(Borders::TOP).border_style(
            Style::default()
                .fg(app.theme.border())
                .add_modifier(Modifier::DIM),
        ),
    )
}

fn truncate_text(text: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }

    if text.chars().count() <= max_width {
        return text.to_string();
    }

    let truncated: String = text.chars().take(max_width.saturating_sub(1)).collect();
    format!("{truncated}…")
}

fn parse_pages_from_publication(value: &str) -> Option<u32> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    trimmed
        .split(|ch: char| !ch.is_ascii_digit())
        .find_map(|part| {
            (!part.is_empty())
                .then(|| part.parse::<u32>().ok())
                .flatten()
        })
}
