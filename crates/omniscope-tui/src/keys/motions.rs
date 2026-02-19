use crate::app::App;

/// Returns the range of indices covered by a motion.
/// The range includes the start position but may exclude the end depending on the motion type,
/// consistent with Vim's inclusive/exclusive behavior.
/// For visual selection, we often want inclusive.
pub fn get_motion_range(app: &App, motion: char, count: usize) -> Option<Vec<usize>> {
    let current = app.selected_index;
    let max_idx = app.books.len().saturating_sub(1);

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
             let target = if count <= 1 {
                 max_idx
             } else {
                 (count - 1).min(max_idx)
             };
             
             let (min, max) = if target < current { (target, current) } else { (current, target) };
             Some((min..=max).collect())
        }
        // gg: go to top (or line N)
        // We will represent 'gg' as just the character 'g' passed to this function
        // because correct dispatch in pending.rs will identify 'gg' sequence and call this with 'g'.
        'g' => {
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
             // range from 0 to current.
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
