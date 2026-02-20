use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub struct LayoutManager;

impl LayoutManager {
    pub fn split_main(area: Rect) -> (Rect, Rect, Rect) {
        let width = area.width;
        
        // Step 2 adaptivity:
        // ≥ 160 col : три панели (22 / auto / 40)
        // ≥ 100 col : две панели (22 / auto), preview по Enter
        // < 100 col : одна центральная, левая по Tab
        
        if width >= 160 {
            let parts = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Length(22),
                    Constraint::Min(20),
                    Constraint::Length(40),
                ])
                .split(area);
            (parts[0], parts[1], parts[2])
        } else if width >= 100 {
            let parts = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Length(22),
                    Constraint::Min(20),
                ])
                .split(area);
            (parts[0], parts[1], Rect::default()) // No preview
        } else {
            (Rect::default(), area, Rect::default()) // Only center
        }
    }
}
