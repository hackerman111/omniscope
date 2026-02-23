use omniscope_core::models::{BookCard, BookOpenAccessInfo, DocumentType};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::theme::NordTheme;

pub fn is_scientific_article(card: &BookCard) -> bool {
    matches!(
        card.publication
            .as_ref()
            .map(|publication| publication.doc_type),
        Some(DocumentType::Article | DocumentType::ConferencePaper | DocumentType::Preprint)
    )
}

pub fn build_preview_lines(
    card: &BookCard,
    max_width: usize,
    theme: &NordTheme,
) -> Vec<Line<'static>> {
    let width = max_width.max(24);
    let mut lines = Vec::new();

    lines.push(styled_line(String::new(), Style::default(), width));
    lines.push(styled_line(
        format!("  📄 {}", title_line(card)),
        Style::default()
            .fg(theme.fg_bright())
            .add_modifier(Modifier::BOLD),
        width,
    ));
    lines.push(divider_line(width, theme));

    lines.push(styled_line(
        format!("  Authors:  {}", compact_authors(&card.metadata.authors)),
        Style::default().fg(theme.fg()),
        width,
    ));

    if let Some(venue) = venue_text(card) {
        lines.push(styled_line(
            format!("  Venue:    {venue}"),
            Style::default().fg(theme.fg()),
            width,
        ));
    }

    if let Some(journal) = journal_text(card) {
        lines.push(styled_line(
            format!("  Journal:  {journal}"),
            Style::default().fg(theme.fg()),
            width,
        ));
    }

    lines.push(divider_line(width, theme));
    push_section_title(&mut lines, "IDENTIFIERS", width, theme);
    append_identifier_lines(&mut lines, card, width, theme);

    lines.push(divider_line(width, theme));
    push_section_title(&mut lines, "METRICS", width, theme);
    append_metrics_lines(&mut lines, card, width, theme);

    lines.push(divider_line(width, theme));
    push_section_title(&mut lines, "OPEN ACCESS", width, theme);
    append_open_access_lines(&mut lines, card.open_access.as_ref(), width, theme);

    if let Some(tldr) = card
        .ai
        .tldr
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        lines.push(divider_line(width, theme));
        push_section_title(&mut lines, "TL;DR (Semantic Scholar AI):", width, theme);
        for wrapped in wrap_text(tldr, width.saturating_sub(4).max(8)) {
            lines.push(styled_line(
                format!("  {wrapped}"),
                Style::default().fg(theme.fg()),
                width,
            ));
        }
    }

    lines.push(divider_line(width, theme));
    lines.push(styled_line(
        "  [o]pen  [r]eferences  [c]itations  [e]xport BibTeX  [ai]  [f]ind".to_string(),
        Style::default()
            .fg(theme.yellow())
            .add_modifier(Modifier::BOLD),
        width,
    ));

    lines
}

fn title_line(card: &BookCard) -> String {
    let mut parts = Vec::new();
    parts.push(card.metadata.title.clone());

    if let Some(doc_type) = card
        .publication
        .as_ref()
        .map(|publication| publication.doc_type)
    {
        parts.push(format!("[{}]", doc_type_badge(doc_type)));
    }

    if let Some(year) = card.metadata.year {
        parts.push(format!("[{year}]"));
    }

    parts.join(" ")
}

fn doc_type_badge(doc_type: DocumentType) -> &'static str {
    match doc_type {
        DocumentType::Article => "article",
        DocumentType::ConferencePaper => "conference",
        DocumentType::Preprint => "preprint",
        _ => "document",
    }
}

fn compact_authors(authors: &[String]) -> String {
    if authors.is_empty() {
        return "Unknown".to_string();
    }

    const MAX_VISIBLE: usize = 4;
    if authors.len() <= MAX_VISIBLE {
        return authors.join(" · ");
    }

    let visible = authors
        .iter()
        .take(MAX_VISIBLE)
        .cloned()
        .collect::<Vec<_>>()
        .join(" · ");
    let rest = authors.len() - MAX_VISIBLE;
    format!("{visible} · (+{rest})")
}

fn venue_text(card: &BookCard) -> Option<String> {
    let publication = card.publication.as_ref()?;
    publication
        .venue
        .as_ref()
        .or(publication.conference.as_ref())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn journal_text(card: &BookCard) -> Option<String> {
    let publication = card.publication.as_ref()?;

    let mut parts = Vec::new();
    if let Some(journal) = publication.journal.as_deref() {
        let journal = journal.trim();
        if !journal.is_empty() {
            parts.push(journal.to_string());
        }
    }
    if let Some(volume) = publication.volume.as_deref() {
        let volume = volume.trim();
        if !volume.is_empty() {
            parts.push(format!("vol. {volume}"));
        }
    }
    if let Some(issue) = publication.issue.as_deref() {
        let issue = issue.trim();
        if !issue.is_empty() {
            parts.push(format!("issue {issue}"));
        }
    }
    if let Some(pages) = publication.pages.as_deref() {
        let pages = pages.trim();
        if !pages.is_empty() {
            parts.push(format!("pp. {pages}"));
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(", "))
    }
}

fn append_identifier_lines(
    lines: &mut Vec<Line<'static>>,
    card: &BookCard,
    width: usize,
    theme: &NordTheme,
) {
    let identifiers = card.identifiers.as_ref();

    let doi = identifiers
        .and_then(|ids| ids.doi.as_deref())
        .map(|doi| format!("{doi}  [↗ open]"))
        .unwrap_or_else(|| "—".to_string());
    let arxiv = identifiers
        .and_then(|ids| ids.arxiv_id.as_deref())
        .map(|arxiv_id| format!("{arxiv_id}  [↗ abs] [↗ pdf]"))
        .unwrap_or_else(|| "—".to_string());
    let semantic = identifiers
        .and_then(|ids| ids.semantic_scholar_id.as_deref())
        .unwrap_or("—")
        .to_string();
    let open_alex = identifiers
        .and_then(|ids| ids.openalex_id.as_deref())
        .unwrap_or("—")
        .to_string();

    lines.push(styled_line(
        format!("  DOI:      {doi}"),
        Style::default().fg(theme.frost_ice()),
        width,
    ));
    lines.push(styled_line(
        format!("  arXiv:    {arxiv}"),
        Style::default().fg(theme.frost_ice()),
        width,
    ));
    lines.push(styled_line(
        format!("  S2:       {semantic}"),
        Style::default().fg(theme.frost_ice()),
        width,
    ));
    lines.push(styled_line(
        format!("  OpenAlex: {open_alex}"),
        Style::default().fg(theme.frost_ice()),
        width,
    ));
}

fn append_metrics_lines(
    lines: &mut Vec<Line<'static>>,
    card: &BookCard,
    width: usize,
    theme: &NordTheme,
) {
    let citations = card.citation_graph.citation_count;
    let influential = card.citation_graph.influential_citation_count;
    let references = reference_count(card);
    let delta_last_month = citation_delta_last_month(card);
    let fields = fields_of_study(card);
    let pages = card
        .metadata
        .pages
        .map(|value| format_count(value))
        .unwrap_or_else(|| "—".to_string());

    lines.push(styled_line(
        format!(
            "  Citations: {}  (📈 +{} last month)",
            format_count(citations),
            format_count(delta_last_month)
        ),
        Style::default().fg(theme.fg()),
        width,
    ));
    lines.push(styled_line(
        format!(
            "  Influential: {}  │  References: {}",
            format_count(influential),
            format_count(references)
        ),
        Style::default().fg(theme.fg()),
        width,
    ));
    lines.push(styled_line(
        format!("  Pages: {pages}"),
        Style::default().fg(theme.fg()),
        width,
    ));
    lines.push(styled_line(
        format!("  Fields: {fields}"),
        Style::default().fg(theme.fg()),
        width,
    ));
}

fn append_open_access_lines(
    lines: &mut Vec<Line<'static>>,
    open_access: Option<&BookOpenAccessInfo>,
    width: usize,
    theme: &NordTheme,
) {
    let (status_text, status_style) = match open_access {
        Some(oa) if oa.is_open => {
            let label = oa
                .status
                .as_deref()
                .filter(|status| !status.trim().is_empty())
                .unwrap_or("Green OA");
            (
                format!("✓ {label}"),
                Style::default()
                    .fg(theme.green())
                    .add_modifier(Modifier::BOLD),
            )
        }
        Some(oa) => {
            let label = oa
                .status
                .as_deref()
                .filter(|status| !status.trim().is_empty())
                .unwrap_or("Closed");
            (
                format!("✗ {label}"),
                Style::default()
                    .fg(theme.red())
                    .add_modifier(Modifier::BOLD),
            )
        }
        None => (
            "✗ Closed".to_string(),
            Style::default()
                .fg(theme.red())
                .add_modifier(Modifier::BOLD),
        ),
    };

    lines.push(styled_line(
        format!("  OPEN ACCESS:  {status_text}"),
        status_style,
        width,
    ));

    let Some(oa) = open_access else {
        lines.push(styled_line(
            "  PDF:      —".to_string(),
            Style::default().fg(theme.muted()),
            width,
        ));
        return;
    };

    let urls = collect_pdf_urls(oa);
    if urls.is_empty() {
        lines.push(styled_line(
            "  PDF:      —".to_string(),
            Style::default().fg(theme.muted()),
            width,
        ));
        return;
    }

    let best = best_pdf_url(oa);
    for (idx, url) in urls.iter().enumerate() {
        let is_best = best.as_deref() == Some(url.as_str());
        let best_marker = if is_best { "  [★ Best]" } else { "" };
        let prefix = if idx == 0 {
            "  PDF:      "
        } else {
            "            "
        };
        lines.push(styled_line(
            format!("{prefix}{url}{best_marker}"),
            Style::default().fg(theme.frost_mint()),
            width,
        ));
    }
}

fn reference_count(card: &BookCard) -> u32 {
    let explicit = card.citation_graph.reference_count;
    if explicit > 0 {
        return explicit;
    }

    let from_ids = card.citation_graph.references_ids.len() as u32;
    let from_strings = card.citation_graph.references.len() as u32;
    from_ids.max(from_strings)
}

fn citation_delta_last_month(_card: &BookCard) -> u32 {
    0
}

fn fields_of_study(card: &BookCard) -> String {
    if card.organization.tags.is_empty() {
        "—".to_string()
    } else {
        card.organization.tags.join(", ")
    }
}

fn collect_pdf_urls(open_access: &BookOpenAccessInfo) -> Vec<String> {
    let mut urls = Vec::new();

    if let Some(url) = open_access.oa_url.as_deref().map(str::trim) {
        if !url.is_empty() {
            urls.push(url.to_string());
        }
    }

    for url in &open_access.pdf_urls {
        let url = url.trim();
        if url.is_empty() {
            continue;
        }
        if !urls.iter().any(|existing| existing == url) {
            urls.push(url.to_string());
        }
    }

    urls
}

fn best_pdf_url(open_access: &BookOpenAccessInfo) -> Option<String> {
    if let Some(url) = open_access.oa_url.as_deref().map(str::trim) {
        if !url.is_empty() {
            return Some(url.to_string());
        }
    }

    open_access
        .pdf_urls
        .iter()
        .map(|url| url.trim())
        .find(|url| !url.is_empty())
        .map(ToOwned::to_owned)
}

fn wrap_text(text: &str, line_width: usize) -> Vec<String> {
    if text.trim().is_empty() {
        return Vec::new();
    }

    let width = line_width.max(8);
    let mut lines = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
            continue;
        }

        let candidate_len = current.chars().count() + 1 + word.chars().count();
        if candidate_len <= width {
            current.push(' ');
            current.push_str(word);
            continue;
        }

        lines.push(current);
        current = word.to_string();
    }

    if !current.is_empty() {
        lines.push(current);
    }

    lines
}

fn push_section_title(
    lines: &mut Vec<Line<'static>>,
    title: &str,
    width: usize,
    theme: &NordTheme,
) {
    lines.push(styled_line(
        format!("  {title}"),
        Style::default()
            .fg(theme.muted())
            .add_modifier(Modifier::BOLD),
        width,
    ));
}

fn divider_line(width: usize, theme: &NordTheme) -> Line<'static> {
    let divider_width = width.saturating_sub(4).max(1);
    styled_line(
        format!("  {}", "─".repeat(divider_width)),
        Style::default().fg(theme.border()),
        width,
    )
}

fn styled_line(text: String, style: Style, max_width: usize) -> Line<'static> {
    Line::from(Span::styled(truncate_text(&text, max_width), style))
}

fn truncate_text(text: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }

    let width = text.chars().count();
    if width <= max_width {
        return text.to_string();
    }

    let truncated: String = text.chars().take(max_width.saturating_sub(1)).collect();
    format!("{truncated}…")
}

fn format_count(value: u32) -> String {
    let chars = value.to_string().chars().rev().collect::<Vec<_>>();
    let mut out = String::new();
    for (idx, ch) in chars.iter().enumerate() {
        if idx > 0 && idx % 3 == 0 {
            out.push(',');
        }
        out.push(*ch);
    }
    out.chars().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use omniscope_core::models::{BookPublication, ScientificIdentifiers};

    fn line_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>()
    }

    fn sample_article_card() -> BookCard {
        let mut card = BookCard::new("Attention Is All You Need");
        card.metadata.authors = vec![
            "Vaswani, A.".to_string(),
            "Shazeer, N.".to_string(),
            "Parmar, N.".to_string(),
            "Uszkoreit, J.".to_string(),
            "Jones, L.".to_string(),
        ];
        card.metadata.year = Some(2017);
        card.publication = Some(BookPublication {
            doc_type: DocumentType::Preprint,
            journal: Some("Advances in Neural Information Processing Systems".to_string()),
            conference: Some("NeurIPS 2017".to_string()),
            venue: Some("NeurIPS 2017".to_string()),
            volume: Some("30".to_string()),
            issue: None,
            pages: None,
        });
        card.identifiers = Some(ScientificIdentifiers {
            doi: Some("10.48550/arXiv.1706.03762".to_string()),
            arxiv_id: Some("1706.03762v5".to_string()),
            openalex_id: Some("W2963403868".to_string()),
            semantic_scholar_id: Some("204e3073870fae3d05bcbc2f6a8e263d9b72e776".to_string()),
            ..Default::default()
        });
        card.citation_graph.citation_count = 87_654;
        card.citation_graph.influential_citation_count = 12_450;
        card.citation_graph.reference_count = 41;
        card.organization.tags = vec![
            "cs.CL".to_string(),
            "cs.LG".to_string(),
            "cs.AI".to_string(),
        ];
        card.open_access = Some(BookOpenAccessInfo {
            is_open: true,
            status: Some("Green OA".to_string()),
            license: None,
            oa_url: Some("https://arxiv.org/pdf/1706.03762".to_string()),
            pdf_urls: vec![
                "https://arxiv.org/pdf/1706.03762".to_string(),
                "https://proceedings.neurips.cc/paper/2017/hash/example.pdf".to_string(),
            ],
        });
        card.ai.tldr = Some(
            "Introduces Transformer, a model based entirely on attention mechanisms.".to_string(),
        );
        card
    }

    #[test]
    fn scientific_type_detection_matches_step_requirements() {
        let mut card = BookCard::new("Any");
        assert!(!is_scientific_article(&card));

        card.publication = Some(BookPublication {
            doc_type: DocumentType::Article,
            ..Default::default()
        });
        assert!(is_scientific_article(&card));

        card.publication = Some(BookPublication {
            doc_type: DocumentType::ConferencePaper,
            ..Default::default()
        });
        assert!(is_scientific_article(&card));

        card.publication = Some(BookPublication {
            doc_type: DocumentType::Preprint,
            ..Default::default()
        });
        assert!(is_scientific_article(&card));
    }

    #[test]
    fn preview_contains_required_sections_and_actions() {
        let theme = NordTheme::default();
        let lines = build_preview_lines(&sample_article_card(), 180, &theme)
            .into_iter()
            .map(|line| line_text(&line))
            .collect::<Vec<_>>();

        assert!(lines.iter().any(|line| line.contains("IDENTIFIERS")));
        assert!(lines.iter().any(|line| line.contains("METRICS")));
        assert!(lines.iter().any(|line| line.contains("OPEN ACCESS")));
        assert!(lines.iter().any(|line| line.contains("TL;DR")));
        assert!(lines.iter().any(|line| {
            line.contains("[o]pen  [r]eferences  [c]itations  [e]xport BibTeX  [ai]  [f]ind")
        }));
    }

    #[test]
    fn identifier_lines_include_expected_hints() {
        let theme = NordTheme::default();
        let lines = build_preview_lines(&sample_article_card(), 180, &theme)
            .into_iter()
            .map(|line| line_text(&line))
            .collect::<Vec<_>>();

        assert!(
            lines
                .iter()
                .any(|line| line.contains("10.48550/arXiv.1706.03762  [↗ open]"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("1706.03762v5  [↗ abs] [↗ pdf]"))
        );
        assert!(lines.iter().any(|line| line.contains("S2:")));
        assert!(lines.iter().any(|line| line.contains("OpenAlex:")));
    }

    #[test]
    fn open_access_marks_best_pdf_and_colors_status_line() {
        let theme = NordTheme::default();
        let lines = build_preview_lines(&sample_article_card(), 180, &theme);

        let status_line = lines
            .iter()
            .find(|line| line_text(line).contains("OPEN ACCESS:"))
            .expect("missing open access status line");
        let status_style = status_line.spans.first().map(|span| span.style.fg);
        assert_eq!(status_style.flatten(), Some(theme.green()));

        let text_lines = lines
            .into_iter()
            .map(|line| line_text(&line))
            .collect::<Vec<_>>();
        assert!(
            text_lines
                .iter()
                .any(|line| line.contains("https://arxiv.org/pdf/1706.03762  [★ Best]"))
        );
    }

    #[test]
    fn tldr_section_is_hidden_when_absent() {
        let mut card = sample_article_card();
        card.ai.tldr = None;

        let theme = NordTheme::default();
        let lines = build_preview_lines(&card, 180, &theme)
            .into_iter()
            .map(|line| line_text(&line))
            .collect::<Vec<_>>();

        assert!(!lines.iter().any(|line| line.contains("TL;DR")));
    }
}
