use omniscope_core::BookSummaryView;
use omniscope_core::models::BookCard;
use omniscope_core::storage::json_cards;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

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
        .constraints([Constraint::Min(1), Constraint::Length(2)])
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
    frame.render_widget(footer_lines(app, body_width, start, max_scroll), chunks[1]);
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
        .and_then(|value| value.metadata.pages)
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

fn footer_lines(app: &App, width: usize, start: usize, max_scroll: usize) -> Paragraph<'static> {
    let scroll_label = if max_scroll == 0 {
        "scroll: 1/1".to_string()
    } else {
        format!("scroll: {}/{}", start + 1, max_scroll + 1)
    };

    let line_1 = format!("  [j/k] scroll  [h/l] focus  [r] refs  [c] cited-by  [f] find/download");
    let line_2 =
        "  [o] open pdf  [e] bibtex  [gr] refs  [gR] cited  [gs] related  [@m/@e/@r] meta/ai/refs"
            .to_string();

    Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                truncate_text(&line_1, width),
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
        Line::from(Span::styled(
            truncate_text(&line_2, width),
            Style::default()
                .fg(app.theme.yellow())
                .add_modifier(Modifier::BOLD),
        )),
    ])
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
