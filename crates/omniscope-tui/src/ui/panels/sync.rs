use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let border_style = Style::default().fg(app.theme.frost_ice());
    let block = Block::default()
        .title(" SYNC STATUS ")
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    // Provide a simple message if there is no report
    let report = match &app.sync_report {
        Some(r) => r,
        None => {
            frame.render_widget(
                Paragraph::new("No sync report generated.")
                    .style(Style::default().fg(app.theme.muted())),
                inner_area,
            );
            return;
        }
    };

    let mut lines = Vec::new();

    // Summary header
    let lib_path = app
        .library_root
        .as_ref()
        .map(|lr| lr.root().display().to_string())
        .unwrap_or_else(|| "Unknown".to_string());
    lines.push(Line::from(vec![
        Span::raw("  Library: "),
        Span::styled(lib_path, Style::default().fg(app.theme.fg_bright())),
        Span::raw("   "),
        Span::styled(
            format!("(In Sync: {})", report.in_sync),
            Style::default().fg(app.theme.muted()),
        ),
    ]));
    lines.push(Line::from(""));

    // NEW (Untracked Files and New on disk folders)
    let new_count = report.new_on_disk.len() + report.untracked_files.len();
    lines.push(Line::from(vec![
        Span::styled(
            format!("  ⊕ NEW ({}) ", new_count),
            Style::default()
                .fg(app.theme.frost_mint())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "─".repeat((area.width as usize).saturating_sub(15)),
            Style::default().fg(app.theme.border()),
        ),
    ]));
    lines.push(Line::from(Span::styled(
        "  Files and folders on disk, no card in library:",
        Style::default().fg(app.theme.muted()),
    )));
    lines.push(Line::from(""));

    let mut item_idx = 0;
    for dir in &report.new_on_disk {
        let is_selected = item_idx == app.sync_selected;
        let prefix = if is_selected { "  [▶] " } else { "  [ ] " };
        let style = if is_selected {
            Style::default()
                .fg(app.theme.frost_blue())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.theme.fg())
        };
        lines.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(app.theme.frost_blue())),
            Span::styled(format!("{}/", dir), style),
            Span::styled(" (Folder)", Style::default().fg(app.theme.muted())),
        ]));
        item_idx += 1;
    }
    for file in &report.untracked_files {
        let is_selected = item_idx == app.sync_selected;
        let prefix = if is_selected { "  [▶] " } else { "  [ ] " };
        let style = if is_selected {
            Style::default()
                .fg(app.theme.frost_blue())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.theme.fg())
        };
        lines.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(app.theme.frost_blue())),
            Span::styled(format!("{}", file.display()), style),
            Span::styled(" (File)", Style::default().fg(app.theme.muted())),
        ]));
        item_idx += 1;
    }
    lines.push(Line::from(""));

    // DETACHED (Books without files or missing on disk)
    let detached_count = report.missing_on_disk.len() + app.detached_books.len();
    lines.push(Line::from(vec![
        Span::styled(
            format!("  ⚠ DETACHED ({}) ", detached_count),
            Style::default()
                .fg(app.theme.orange())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "─".repeat((area.width as usize).saturating_sub(20)),
            Style::default().fg(app.theme.border()),
        ),
    ]));
    lines.push(Line::from(Span::styled(
        "  Cards exist, files missing or folders missing on disk:",
        Style::default().fg(app.theme.muted()),
    )));
    lines.push(Line::from(""));

    for missing_dir in &report.missing_on_disk {
        lines.push(Line::from(vec![
            Span::styled("  [󰈖] ", Style::default().fg(app.theme.muted())),
            Span::styled(
                format!("(Folder) {}", missing_dir),
                Style::default().fg(app.theme.orange()),
            ),
        ]));
    }
    for book in &app.detached_books {
        lines.push(Line::from(vec![
            Span::styled("  [󰈖] ", Style::default().fg(app.theme.muted())),
            Span::styled(
                format!("\"{}\"", book.title),
                Style::default().fg(app.theme.fg()),
            ),
            Span::styled(" → ", Style::default().fg(app.theme.muted())),
            Span::styled(
                book.path.as_deref().unwrap_or("No path associated"),
                Style::default().fg(app.theme.orange()),
            ),
        ]));
    }
    lines.push(Line::from(""));

    // Footer actions
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(4), Constraint::Length(2)])
        .split(inner_area);

    let paragraph = Paragraph::new(lines).scroll((0, 0)); // No scrolling implemented yet
    frame.render_widget(paragraph, layout[0]);

    // Footer instructions
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            "  [i] ",
            Style::default()
                .fg(app.theme.frost_blue())
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Import selected  "),
        Span::styled(
            "[I] ",
            Style::default()
                .fg(app.theme.frost_blue())
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Import all  "),
        Span::styled(
            "[a] ",
            Style::default()
                .fg(app.theme.frost_blue())
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Auto sync  "),
        Span::styled(
            "[Esc] ",
            Style::default()
                .fg(app.theme.frost_blue())
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Close"),
    ]));
    frame.render_widget(footer, layout[1]);
}
