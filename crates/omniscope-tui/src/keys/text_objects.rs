use crate::app::App;

pub enum TextObjectKind {
    Inner,
    Around,
}

pub fn get_text_object_range(app: &App, object: char, _kind: TextObjectKind) -> Option<Vec<usize>> {
    let current_book = app.selected_book()?;
    
    match object {
        // l: library
        'l' => {
             // BookSummaryView doesn't have library information currently.
             // We cannot implement this efficiently without loading all cards.
             // Disabling for now.
             None
        }
        // a: author (all authors match? or any?)
        // Spec: "all books of this author"
        // Since a book can have multiple authors, we match if they share AT LEAST ONE author?
        // Or if the author list is identical?
        // Let's go with: if the current book has primary author X, match all books with author X.
        'a' => {
             // simplify: match if the first author is same, or exact author string match
             let authors = &current_book.authors; 
             if authors.is_empty() { return None; }
             let primary = &authors[0];
             
             let indices: Vec<usize> = app.books.iter().enumerate()
                .filter(|(_, b)| b.authors.contains(primary))
                .map(|(i, _)| i)
                .collect();
             if indices.is_empty() { None } else { Some(indices) }
        }
        // y: year
        'y' => {
             let year = current_book.year;
             let indices: Vec<usize> = app.books.iter().enumerate()
                .filter(|(_, b)| b.year == year)
                .map(|(i, _)| i)
                .collect();
             if indices.is_empty() { None } else { Some(indices) }
        }
         _ => None,
    }
}
