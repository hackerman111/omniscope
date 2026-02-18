use crate::config::AppConfig;
use crate::models::BookCard;

/// Open a book's file in the configured external viewer.
pub fn open_book(card: &BookCard, config: &AppConfig) -> anyhow::Result<()> {
    let file = card
        .file
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Book has no attached file"))?;

    let viewer = get_viewer_for_format(&file.format.to_string(), config);

    if viewer == "xdg-open" || viewer.starts_with('$') {
        // Use the `open` crate for system default
        open::that(&file.path)?;
    } else {
        // Use specific viewer
        std::process::Command::new(&viewer)
            .arg(&file.path)
            .spawn()?;
    }

    Ok(())
}

/// Open a book's file with a specific application.
pub fn open_book_with(card: &BookCard, app_name: &str) -> anyhow::Result<()> {
    let file = card
        .file
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Book has no attached file"))?;

    open::with(&file.path, app_name)?;
    Ok(())
}

/// Get the configured viewer for a file format.
fn get_viewer_for_format(format: &str, config: &AppConfig) -> String {
    match format {
        "pdf" => config.viewers.pdf.clone(),
        "epub" => config.viewers.epub.clone(),
        "djvu" => config.viewers.djvu.clone(),
        "mobi" => config.viewers.mobi.clone(),
        "txt" => config.viewers.txt.clone(),
        "html" => config.viewers.html.clone(),
        _ => "xdg-open".to_string(),
    }
}

/// Get list of alternative viewers for a format.
pub fn get_alternatives(format: &str, config: &AppConfig) -> Vec<String> {
    config
        .viewers
        .alternatives
        .get(format)
        .cloned()
        .unwrap_or_default()
}
