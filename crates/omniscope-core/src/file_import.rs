use std::fs;
use std::path::Path;

use chrono::Utc;

use crate::error::Result;
use crate::models::{BookCard, BookFile, FileFormat};

/// Known book file extensions.
const BOOK_EXTENSIONS: &[&str] = &[
    "pdf", "epub", "djvu", "mobi", "fb2", "txt", "html", "htm", "azw3", "cbz", "cbr",
];

/// Import a single file as a BookCard.
/// Extracts metadata from the filename: strips extension, replaces separators with spaces.
pub fn import_file(path: &Path) -> Result<BookCard> {
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

    // Extract title from filename
    let title = path
        .file_stem()
        .map(|s| {
            s.to_string_lossy()
                .replace('_', " ")
                .replace('-', " ")
                .replace('.', " ")
                .trim()
                .to_string()
        })
        .unwrap_or_else(|| "Untitled".to_string());

    // Detect format
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_string())
        .unwrap_or_default();
    let format = FileFormat::from_extension(&ext);

    // Get file size
    let metadata = fs::metadata(&path)?;
    let size_bytes = metadata.len();

    let mut card = BookCard::new(&title);
    card.file = Some(BookFile {
        path: path.to_string_lossy().to_string(),
        format,
        size_bytes,
        hash_sha256: None,
        added_at: Utc::now(),
    });

    // Try to extract author from "Author - Title" pattern
    if let Some(stem) = path.file_stem() {
        let stem_str = stem.to_string_lossy();
        if let Some((author, book_title)) = stem_str.split_once(" - ") {
            card.metadata.title = book_title.trim().to_string();
            card.metadata.authors = vec![author.trim().to_string()];
        }
    }

    Ok(card)
}

/// Scan a directory for book files and import them.
pub fn scan_directory(dir: &Path, recursive: bool) -> Result<Vec<BookCard>> {
    let mut cards = Vec::new();

    if !dir.exists() || !dir.is_dir() {
        return Ok(cards);
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() && recursive {
            let sub_cards = scan_directory(&path, true)?;
            cards.extend(sub_cards);
        } else if path.is_file() {
            if let Some(ext) = path.extension() {
                let ext_str = ext.to_string_lossy().to_lowercase();
                if BOOK_EXTENSIONS.contains(&ext_str.as_str()) {
                    match import_file(&path) {
                        Ok(card) => cards.push(card),
                        Err(e) => {
                            eprintln!("Warning: skipping {}: {e}", path.display());
                        }
                    }
                }
            }
        }
    }

    Ok(cards)
}

/// Check if a path looks like a book file.
pub fn is_book_file(path: &Path) -> bool {
    path.extension()
        .map(|ext| {
            let ext_str = ext.to_string_lossy().to_lowercase();
            BOOK_EXTENSIONS.contains(&ext_str.as_str())
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::TempDir;

    #[test]
    fn test_import_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("The_Rust_Book.pdf");
        File::create(&file_path).unwrap();

        let card = import_file(&file_path).unwrap();
        assert_eq!(card.metadata.title, "The Rust Book");
        assert!(card.file.is_some());
        assert_eq!(card.file.as_ref().unwrap().format, FileFormat::Pdf);
    }

    #[test]
    fn test_import_author_title_pattern() {
        let dir = TempDir::new().unwrap();
        let file_path = dir
            .path()
            .join("Steve Klabnik - The Rust Programming Language.epub");
        File::create(&file_path).unwrap();

        let card = import_file(&file_path).unwrap();
        assert_eq!(card.metadata.title, "The Rust Programming Language");
        assert_eq!(card.metadata.authors, vec!["Steve Klabnik"]);
        assert_eq!(card.file.as_ref().unwrap().format, FileFormat::Epub);
    }

    #[test]
    fn test_scan_directory() {
        let dir = TempDir::new().unwrap();
        File::create(dir.path().join("book1.pdf")).unwrap();
        File::create(dir.path().join("book2.epub")).unwrap();
        File::create(dir.path().join("readme.txt")).unwrap();
        File::create(dir.path().join("image.jpg")).unwrap(); // not a book

        let cards = scan_directory(dir.path(), false).unwrap();
        assert_eq!(cards.len(), 3); // pdf, epub, txt
    }

    #[test]
    fn test_scan_directory_recursive() {
        let dir = TempDir::new().unwrap();
        let sub = dir.path().join("subdir");
        fs::create_dir(&sub).unwrap();
        File::create(dir.path().join("root.pdf")).unwrap();
        File::create(sub.join("nested.epub")).unwrap();

        let cards = scan_directory(dir.path(), true).unwrap();
        assert_eq!(cards.len(), 2);
    }

    #[test]
    fn test_is_book_file() {
        assert!(is_book_file(Path::new("test.pdf")));
        assert!(is_book_file(Path::new("test.epub")));
        assert!(is_book_file(Path::new("test.DJVU")));
        assert!(!is_book_file(Path::new("test.jpg")));
        assert!(!is_book_file(Path::new("test.mp3")));
    }
}
