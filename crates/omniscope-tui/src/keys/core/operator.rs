use crate::app::App;

/// Standard Vim operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    Delete,
    Yank,
    Change,
    Move,
    Put,
    AddTag,
    RemoveTag,
    Normalize,
    Filter, // e.g. =
}

/// Execute an operator on a range of book indices.
pub fn execute_operator(app: &mut App, op: Operator, range: Vec<usize>) {
    match op {
        Operator::Delete => app.delete_indices(&range),
        Operator::Yank => app.yank_indices(&range),
        Operator::Change => {
            // Change: open edit tags for the range (same as "change metadata")
            if !range.is_empty() {
                let idx = range[0];
                app.selected_index = idx;
                app.open_edit_tags();
                app.status_message = format!("Change {} items (editing tags)", range.len());
            }
        }
        Operator::AddTag => {
            // Open AddTagPrompt with the full range
            if !range.is_empty() {
                app.popup = Some(crate::popup::Popup::AddTagPrompt {
                    indices: range.clone(),
                    input: String::new(),
                    cursor: 0,
                });
                app.status_message = format!("Add tag to {} items", range.len());
            }
        }
        Operator::RemoveTag => {
            // Collect unique tags from all items in range
            if !range.is_empty() {
                let mut tags: Vec<String> = Vec::new();
                for &i in &range {
                    if let Some(book) = app.books.get(i) {
                        for tag in &book.tags {
                            if !tags.contains(tag) {
                                tags.push(tag.clone());
                            }
                        }
                    }
                }
                if tags.is_empty() {
                    app.status_message = "No tags to remove".to_string();
                } else {
                    app.popup = Some(crate::popup::Popup::RemoveTagPrompt {
                        indices: range.clone(),
                        available_tags: tags,
                        selected: 0,
                    });
                    app.status_message = format!("Remove tag from {} items", range.len());
                }
            }
        }
        _ => {}
    }
}
