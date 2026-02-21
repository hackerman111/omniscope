use crate::app::App;

/// Returns the target index for a navigation motion in Normal mode.
/// This is used for simple movement (j, k, gg, G, etc.) without an operator.
pub fn get_nav_target(app: &App, motion: char, count: usize) -> Option<usize> {
    let current = app.selected_index;
    let max_idx = app.books.len().saturating_sub(1);

    match motion {
        'j' => Some((current + count).min(max_idx)),
        'k' => Some(current.saturating_sub(count)),
        'g' => {
            // 'gg' -> go to line count-1 (default 1 -> 0)
            // If count is 0 or 1, go to 0.
            if count <= 1 {
                Some(0)
            } else {
                Some((count - 1).min(max_idx))
            }
        }
        'G' => {
            // [count]G -> go to line [count], default last line.
            // count=0 means no explicit count -> go to last line (standard Vim).
            // count>0 means explicit count -> go to line N.
            if count == 0 {
                 Some(max_idx)
            } else {
                 Some((count - 1).min(max_idx))
            }
        }
        '0' => Some(0), // start of list (conceptually start of line)
        '$' => Some(max_idx), // end of list (conceptually end of line)
        // h/l don't move vertical index usually, but in file-manager style might change focus.
        // We'll leave horizontal nav to app methods for now.
        _ => None,
    }
}

/// Returns the range of indices covered by a motion.
/// The range includes the start position but may exclude the end depending on the motion type,
/// consistent with Vim's inclusive/exclusive behavior.
/// For visual selection, we often want inclusive.
pub fn get_motion_range(app: &App, motion: char, count: usize) -> Option<Vec<usize>> {
    let current = app.selected_index;
    let max_idx = app.books.len().saturating_sub(1);

    // We can reuse get_nav_target for the "endpoint", but ranges might differ.
    // e.g. 'delete' is linewise for j/k.
    
    match motion {
        // j: down
        'j' => {
            let target = (current + count).min(max_idx);
            Some((current..=target).collect())
        }
        // k: up
        'k' => {
            let target = current.saturating_sub(count);
            // Return range from target to current (inclusive) for operations
            Some((target..=current).collect())
        }
        // G: go to bottom (or line N)
        'G' => {
            // count=0 → to bottom (dG). count>0 → to line N (d5G).
            let target = if count == 0 {
                 max_idx
             } else {
                 (count - 1).min(max_idx)
             };
             
             let (min, max) = if target < current { (target, current) } else { (current, target) };
             Some((min..=max).collect())
        }
        // gg: go to line N (default 1)
        'g' => {
            // "dgg" deletes to line 1.
            // "d5gg" deletes to line 5.
            let target = if count <= 1 {
                 0
             } else {
                 (count - 1).min(max_idx)
             };
             let (min, max) = if target < current { (target, current) } else { (current, target) };
             Some((min..=max).collect())
        }
        // 0: go to top
        '0' => {
             // range from 0 to current? Refers to "start of line".
             // In list view, maybe logical start (index 0)?
             // Or maybe "0" is not a vertical motion here?
             // If we treat list as 1D, 0 goes to top.
             // d0 -> delete to top.
             Some((0..=current).collect())
        }
        // $: go to bottom
        '$' => {
             // range from current to max.
             Some((current..=max_idx).collect())
        }
        _ => None,
    }
}
