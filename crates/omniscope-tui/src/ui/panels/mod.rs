pub mod ai_panel;
pub mod center;
pub mod cmdline;
pub mod left;
pub mod quickfix;
pub mod right;
pub mod statusbar;

use crate::app::App;
use ratatui::layout::Rect;
use ratatui::Frame;

pub fn render_body(frame: &mut Frame, app: &App, area: Rect) {
    let body_area = if app.quickfix_show {
        let chunks = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Min(3),
                ratatui::layout::Constraint::Length(10.min(area.height / 3)),
            ])
            .split(area);
        render_quickfix(frame, app, chunks[1]);
        chunks[0]
    } else {
        area
    };

    let (left_area, center_area, right_area) = super::layout::LayoutManager::split_main(body_area);

    if !left_area.is_empty() {
        left::render(frame, app, left_area);
    }

    // Center is always rendered per Rule 2 adaptivity
    center::render(frame, app, center_area);

    if !right_area.is_empty() {
        ai_panel::render(frame, app, right_area);
    }
}

pub fn render_quickfix(frame: &mut Frame, app: &App, area: Rect) {
    quickfix::render(frame, app, area);
}
