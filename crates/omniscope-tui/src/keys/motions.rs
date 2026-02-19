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
            // 'G' -> go to line count-1 (default 1 -> max_idx)
            // If count is passed as 1 (default), it means "bottom".
            // If explicit count is given (e.g. 5G), it means line 5.
            // But verify: vim says "G" goes to line [count], default last line.
            // Our architecture passes explicit count or 1.
            // We need to know if count was explicit. We don't have that info here easily
            // unless we change how count is passed or assume 1 means default.
            // But 1G is valid (goto line 1).
            // For now, let's assume if the user typed '1G', they get line 1.
            // If they typed nothing, count is 1, so they get line 1? No, G default is bottom.
            // This ambiguity is resolved in `mod.rs` usually.
            // Let's implement standard Vim behavior:
            // If we treat "1" as "default/no-count", we can't distinguish "1G" from "G".
            // In `mod.rs`, we might handle 'G' specifically.
            // Here, let's assume:
            // if we are called with count 1, it might be default.
            // BUT `get_nav_target` should probably just take the "resolved" target?
            // Actually, for 'G', typical vim behavior:
            // [count]G -> Go to line [count], default last line.
            // so we implement:
            if count == 0 { // Should not happen if we use count_or_one, but safety check
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
            // For G, the target is defined as:
            // [count]G -> line [count]. Default last line.
            // However, distinguishing default '1' from explicit '1' is hard if we just pass usize.
            // Let's assume for ranges (dG), it means "to end of file" if count is 1?
            // "dG" deletes to end of file. "d1G" deletes to top.
            // Ideally we need `is_explicit_count` bool.
            // For now, let's assume 'G' with count 1 is "to bottom".
            // Because '1G' is rare compared to 'G'.
            let target = if count <= 1 {
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
