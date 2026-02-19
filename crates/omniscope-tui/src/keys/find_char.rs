use crate::app::App;

/// Returns the target index for a find character motion (f, F, t, T)
pub fn get_find_char_target(app: &App, motion: char, target_char: char, count: usize) -> Option<usize> {
    let current = app.selected_index;
    if current >= app.books.len() {
        return None;
    }

    let target_char_lower = target_char.to_lowercase().next().unwrap_or(target_char);

    match motion {
        'f' | 't' => {
            // Forward search
            let mut found_count = 0;
            for i in (current + 1)..app.books.len() {
                if app.books[i].title.to_lowercase().starts_with(target_char_lower) {
                    found_count += 1;
                    if found_count == count {
                        return if motion == 'f' {
                            Some(i)
                        } else {
                            // 't' goes to the item *before* the match
                            Some(i.saturating_sub(1).max(current))
                        };
                    }
                }
            }
        }
        'F' | 'T' => {
            // Backward search
            let mut found_count = 0;
            for i in (0..current).rev() {
                if app.books[i].title.to_lowercase().starts_with(target_char_lower) {
                    found_count += 1;
                    if found_count == count {
                        return if motion == 'F' {
                            Some(i)
                        } else {
                            // 'T' goes to the item *after* the match
                            Some((i + 1).min(current))
                        };
                    }
                }
            }
        }
        _ => {}
    }

    None
}

/// Returns the range for a find char motion when used with an operator (e.g., dfc)
pub fn get_find_char_range(app: &App, motion: char, target_char: char, count: usize) -> Option<Vec<usize>> {
    let current = app.selected_index;
    let target = get_find_char_target(app, motion, target_char, count)?;

    let min = current.min(target);
    let max = current.max(target);
    Some((min..=max).collect())
}
