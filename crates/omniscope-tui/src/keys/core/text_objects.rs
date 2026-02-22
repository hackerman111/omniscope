use crate::app::{App, CenterItem, CenterPanelMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextObjectKind {
    Inner,
    Around,
}

pub fn get_text_object_range(app: &App, object: char, kind: TextObjectKind) -> Option<Vec<usize>> {
    let current = app.selected_index;
    let current_book = app.selected_book()?;

    let in_folder_view = app.center_panel_mode == CenterPanelMode::FolderView;

    match object {
        // b: book (current book only)
        'b' => Some(vec![current]),
        // l: library — all books sharing a library with the current book
        'l' => {
            // Load the current card to get its library info
            let cards_dir = app.cards_dir();
            if let Ok(card) =
                omniscope_core::storage::json_cards::load_card_by_id(&cards_dir, &current_book.id)
            {
                if card.organization.libraries.is_empty() {
                    return None;
                }
                let target_lib = &card.organization.libraries[0];

                // Find all books in the same library
                let indices: Vec<usize> = if in_folder_view {
                    app.center_items
                        .iter()
                        .enumerate()
                        .filter(|(_, item)| {
                            if let CenterItem::Book(b) = item {
                                if let Ok(c) = omniscope_core::storage::json_cards::load_card_by_id(
                                    &cards_dir, &b.id,
                                ) {
                                    c.organization.libraries.contains(target_lib)
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        })
                        .map(|(i, _)| i)
                        .collect()
                } else {
                    app.books
                        .iter()
                        .enumerate()
                        .filter(|(_, b)| {
                            if let Ok(c) = omniscope_core::storage::json_cards::load_card_by_id(
                                &cards_dir, &b.id,
                            ) {
                                c.organization.libraries.contains(target_lib)
                            } else {
                                false
                            }
                        })
                        .map(|(i, _)| i)
                        .collect()
                };

                if indices.is_empty() {
                    None
                } else {
                    Some(indices)
                }
            } else {
                None
            }
        }
        // a: author — all books of the same primary author
        'a' => {
            let authors = &current_book.authors;
            if authors.is_empty() {
                return None;
            }
            let primary = &authors[0];

            let indices: Vec<usize> = if in_folder_view {
                app.center_items
                    .iter()
                    .enumerate()
                    .filter(|(_, item)| {
                        if let CenterItem::Book(b) = item {
                            b.authors.contains(primary)
                        } else {
                            false
                        }
                    })
                    .map(|(i, _)| i)
                    .collect()
            } else {
                app.books
                    .iter()
                    .enumerate()
                    .filter(|(_, b)| b.authors.contains(primary))
                    .map(|(i, _)| i)
                    .collect()
            };
            if indices.is_empty() {
                None
            } else {
                Some(indices)
            }
        }
        // t: tag — all books sharing any tag with the current book
        't' => {
            let current_tags = &current_book.tags;
            if current_tags.is_empty() {
                return None;
            }

            let indices: Vec<usize> = if in_folder_view {
                app.center_items
                    .iter()
                    .enumerate()
                    .filter(|(_, item)| {
                        if let CenterItem::Book(b) = item {
                            match kind {
                                TextObjectKind::Inner => {
                                    current_tags.iter().all(|t| b.tags.contains(t))
                                }
                                TextObjectKind::Around => {
                                    current_tags.iter().any(|t| b.tags.contains(t))
                                }
                            }
                        } else {
                            false
                        }
                    })
                    .map(|(i, _)| i)
                    .collect()
            } else {
                app.books
                    .iter()
                    .enumerate()
                    .filter(|(_, b)| match kind {
                        TextObjectKind::Inner => current_tags.iter().all(|t| b.tags.contains(t)),
                        TextObjectKind::Around => current_tags.iter().any(|t| b.tags.contains(t)),
                    })
                    .map(|(i, _)| i)
                    .collect()
            };
            if indices.is_empty() {
                None
            } else {
                Some(indices)
            }
        }
        // f: folder — all books in current filter/view
        'f' => {
            let max_idx = if in_folder_view {
                app.center_items.len()
            } else {
                app.books.len()
            };
            match kind {
                TextObjectKind::Inner => {
                    let indices: Vec<usize> = (0..max_idx).collect();
                    if indices.is_empty() {
                        None
                    } else {
                        Some(indices)
                    }
                }
                TextObjectKind::Around => {
                    let indices: Vec<usize> = (0..max_idx).collect();
                    if indices.is_empty() {
                        None
                    } else {
                        Some(indices)
                    }
                }
            }
        }
        // y: year — all books of the same year
        'y' => {
            let year = current_book.year;
            let indices: Vec<usize> = if in_folder_view {
                app.center_items
                    .iter()
                    .enumerate()
                    .filter(|(_, item)| {
                        if let CenterItem::Book(b) = item {
                            b.year == year
                        } else {
                            false
                        }
                    })
                    .map(|(i, _)| i)
                    .collect()
            } else {
                app.books
                    .iter()
                    .enumerate()
                    .filter(|(_, b)| b.year == year)
                    .map(|(i, _)| i)
                    .collect()
            };
            if indices.is_empty() {
                None
            } else {
                Some(indices)
            }
        }
        _ => None,
    }
}
