use std::path::Path;

use anyhow::Result;
use clap::{Parser, Subcommand};

use omniscope_core::{AppConfig, BookCard, Database};
use omniscope_tui::app::App;

#[derive(Parser)]
#[command(
    name = "omniscope",
    about = "Terminal book library manager — Yazi for books",
    version,
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Output in JSON format (for AI agents and scripts).
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// List all books in the library.
    List {
        /// Maximum number of results.
        #[arg(long, default_value = "50")]
        limit: usize,

        /// Offset for pagination.
        #[arg(long, default_value = "0")]
        offset: usize,
    },

    /// Search books by query.
    Search {
        /// Search query.
        query: String,

        /// Maximum number of results.
        #[arg(long, default_value = "20")]
        limit: usize,
    },

    /// Operations on a single book.
    Book {
        #[command(subcommand)]
        action: BookAction,
    },

    /// Add a book to the library.
    Add {
        /// Path to book file (optional — omit for metadata-only card).
        #[arg(long)]
        file: Option<String>,

        /// Book title.
        #[arg(long)]
        title: Option<String>,

        /// Author(s).
        #[arg(long, action = clap::ArgAction::Append)]
        author: Vec<String>,

        /// Publication year.
        #[arg(long)]
        year: Option<i32>,

        /// Tag(s).
        #[arg(long, action = clap::ArgAction::Append)]
        tag: Vec<String>,

        /// Library to add to.
        #[arg(long)]
        library: Option<String>,
    },

    /// Import a directory of book files.
    Import {
        /// Directory to scan for books.
        dir: String,

        /// Recursive scan.
        #[arg(long)]
        recursive: bool,
    },

    /// Tag management.
    Tag {
        #[command(subcommand)]
        action: TagAction,
    },

    /// Library management.
    Library {
        #[command(subcommand)]
        action: LibraryAction,
    },

    /// Show library statistics.
    Stats,

    /// Run diagnostics.
    Doctor,

    /// Show version information.
    Version,
}

#[derive(Subcommand)]
enum BookAction {
    /// Get a book card by ID.
    Get {
        /// Book UUID.
        id: String,
    },

    /// Update a book card.
    Update {
        /// Book UUID.
        id: String,

        /// Set title.
        #[arg(long)]
        title: Option<String>,

        /// Set author(s).
        #[arg(long, action = clap::ArgAction::Append)]
        author: Vec<String>,

        /// Set year.
        #[arg(long)]
        year: Option<i32>,

        /// Set rating (0-5, 0 clears).
        #[arg(long)]
        rating: Option<u8>,

        /// Set read status (unread/reading/read/dnf).
        #[arg(long)]
        status: Option<String>,
    },

    /// Delete a book.
    Delete {
        /// Book UUID.
        id: String,

        /// Skip confirmation.
        #[arg(long)]
        confirm: bool,
    },

    /// Add or remove tags.
    Tag {
        /// Book UUID.
        id: String,

        /// Tags to add.
        #[arg(long, action = clap::ArgAction::Append)]
        add: Vec<String>,

        /// Tags to remove.
        #[arg(long, action = clap::ArgAction::Append)]
        remove: Vec<String>,
    },
}

#[derive(Subcommand)]
enum TagAction {
    /// List all tags.
    List,
}

#[derive(Subcommand)]
enum LibraryAction {
    /// List all libraries.
    List,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = AppConfig::load()?;

    match cli.command {
        None => {
            let mut app = App::new(config);
            omniscope_tui::run_tui(&mut app)?;
        }

        Some(Commands::List { limit, offset }) => {
            let db = open_db(&config)?;
            let books = db.list_books(limit, offset)?;

            if cli.json {
                let output = serde_json::json!({
                    "status": "ok",
                    "data": { "items": books, "total": db.count_books()?, "limit": limit, "offset": offset }
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else if books.is_empty() {
                println!("No books in library. Use `omniscope add` to add books.");
            } else {
                for book in &books {
                    let year = book.year.map(|y| y.to_string()).unwrap_or_default();
                    let authors = book.authors.join(", ");
                    println!(
                        "{id}  {title:<40}  {authors:<25}  {year}",
                        id = &book.id.to_string()[..8],
                        title = book.title,
                    );
                }
            }
        }

        Some(Commands::Search { query, limit }) => {
            let db = open_db(&config)?;
            let results = db.search_fts(&query, limit)?;

            if cli.json {
                let output = serde_json::json!({
                    "status": "ok",
                    "data": { "items": results, "total": results.len(), "query": query }
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else if results.is_empty() {
                println!("No results for: {query}");
            } else {
                println!("Found {} results:", results.len());
                for book in &results {
                    println!("  {} — {}", &book.id.to_string()[..8], book.title);
                }
            }
        }

        Some(Commands::Book { action }) => match action {
            BookAction::Get { id } => {
                let db = open_db(&config)?;
                let card_path = config.cards_dir().join(format!("{id}.json"));
                if card_path.exists() {
                    let card = omniscope_core::storage::json_cards::load_card(&card_path)?;
                    if cli.json {
                        println!("{}", serde_json::to_string_pretty(&serde_json::json!({"status": "ok", "data": card}))?);
                    } else {
                        println!("{}", serde_json::to_string_pretty(&card)?);
                    }
                } else {
                    let summary = db.get_book_summary(&id)?;
                    if cli.json {
                        println!("{}", serde_json::to_string_pretty(&serde_json::json!({"status": "ok", "data": summary}))?);
                    } else {
                        println!("{}", serde_json::to_string_pretty(&summary)?);
                    }
                }
            }

            BookAction::Update { id, title, author, year, rating, status } => {
                let cards_dir = config.cards_dir();
                let uuid = uuid::Uuid::parse_str(&id)?;
                let mut card = omniscope_core::storage::json_cards::load_card_by_id(&cards_dir, &uuid)?;

                if let Some(t) = title { card.metadata.title = t; }
                if !author.is_empty() { card.metadata.authors = author; }
                if let Some(y) = year { card.metadata.year = Some(y); }
                if let Some(r) = rating { card.organization.rating = if r == 0 { None } else { Some(r) }; }
                if let Some(s) = status {
                    card.organization.read_status = match s.as_str() {
                        "reading" => omniscope_core::ReadStatus::Reading,
                        "read" => omniscope_core::ReadStatus::Read,
                        "dnf" => omniscope_core::ReadStatus::Dnf,
                        _ => omniscope_core::ReadStatus::Unread,
                    };
                }

                card.updated_at = chrono::Utc::now();
                omniscope_core::storage::json_cards::save_card(&cards_dir, &card)?;
                let db = open_db(&config)?;
                db.upsert_book(&card)?;

                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&serde_json::json!({"status": "ok", "data": card}))?);
                } else {
                    println!("Updated: {}", card.metadata.title);
                }
            }

            BookAction::Delete { id, confirm } => {
                if !confirm {
                    eprintln!("Use --confirm to skip confirmation prompt.");
                    std::process::exit(8);
                }
                let db = open_db(&config)?;
                db.delete_book(&id)?;
                omniscope_core::storage::json_cards::delete_card(&config.cards_dir(), &uuid::Uuid::parse_str(&id)?)?;
                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&serde_json::json!({"status": "ok", "data": {"deleted": id}}))?);
                } else {
                    println!("Deleted book: {id}");
                }
            }

            BookAction::Tag { id, add, remove } => {
                let cards_dir = config.cards_dir();
                let uuid = uuid::Uuid::parse_str(&id)?;
                let mut card = omniscope_core::storage::json_cards::load_card_by_id(&cards_dir, &uuid)?;

                for tag in add {
                    if !card.organization.tags.contains(&tag) {
                        card.organization.tags.push(tag);
                    }
                }
                for tag in &remove {
                    card.organization.tags.retain(|t| t != tag);
                }

                card.updated_at = chrono::Utc::now();
                omniscope_core::storage::json_cards::save_card(&cards_dir, &card)?;
                let db = open_db(&config)?;
                db.upsert_book(&card)?;

                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&serde_json::json!({"status": "ok", "data": {"tags": card.organization.tags}}))?);
                } else {
                    println!("Tags: {}", card.organization.tags.join(", "));
                }
            }
        },

        Some(Commands::Add { file, title, author, year, tag, library }) => {
            let mut card = if let Some(ref file_path) = file {
                omniscope_core::file_import::import_file(Path::new(file_path))?
            } else {
                BookCard::new(title.as_deref().unwrap_or("Untitled"))
            };

            if let Some(t) = title { card.metadata.title = t; }
            if !author.is_empty() { card.metadata.authors = author; }
            if let Some(y) = year { card.metadata.year = Some(y); }
            card.organization.tags = tag;
            if let Some(lib) = library { card.organization.libraries.push(lib); }

            let cards_dir = config.cards_dir();
            omniscope_core::storage::json_cards::save_card(&cards_dir, &card)?;
            let db = open_db(&config)?;
            db.upsert_book(&card)?;

            if cli.json {
                println!("{}", serde_json::to_string_pretty(&serde_json::json!({"status": "ok", "data": card}))?);
            } else {
                println!("Added: {} ({})", card.metadata.title, card.id);
            }
        }

        Some(Commands::Import { dir, recursive }) => {
            let cards = omniscope_core::file_import::scan_directory(Path::new(&dir), recursive)?;
            let db = open_db(&config)?;
            let cards_dir = config.cards_dir();
            let mut count = 0;

            for card in &cards {
                omniscope_core::storage::json_cards::save_card(&cards_dir, card)?;
                db.upsert_book(card)?;
                count += 1;
                if !cli.json {
                    println!("  Imported: {}", card.metadata.title);
                }
            }

            if cli.json {
                println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                    "status": "ok", "data": { "imported": count, "total": db.count_books()? }
                }))?);
            } else {
                println!("Imported {count} book(s).");
            }
        }

        Some(Commands::Tag { action }) => match action {
            TagAction::List => {
                let db = open_db(&config)?;
                let tags = db.list_tags()?;
                if cli.json {
                    let items: Vec<serde_json::Value> = tags.iter().map(|(n, c)| serde_json::json!({"name": n, "count": c})).collect();
                    println!("{}", serde_json::to_string_pretty(&serde_json::json!({"status": "ok", "data": items}))?);
                } else if tags.is_empty() {
                    println!("No tags.");
                } else {
                    for (name, count) in &tags { println!("  #{name} ({count})"); }
                }
            }
        },

        Some(Commands::Library { action }) => match action {
            LibraryAction::List => {
                let db = open_db(&config)?;
                let libs = db.list_libraries()?;
                if cli.json {
                    let items: Vec<serde_json::Value> = libs.iter().map(|(n, c)| serde_json::json!({"name": n, "count": c})).collect();
                    println!("{}", serde_json::to_string_pretty(&serde_json::json!({"status": "ok", "data": items}))?);
                } else if libs.is_empty() {
                    println!("No libraries.");
                } else {
                    for (name, count) in &libs { println!("  📁 {name} ({count})"); }
                }
            }
        },

        Some(Commands::Stats) => {
            let db = open_db(&config)?;
            let count = db.count_books()?;
            let tags = db.list_tags()?;
            let libs = db.list_libraries()?;

            if cli.json {
                println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                    "status": "ok", "data": { "total_books": count, "total_tags": tags.len(), "total_libraries": libs.len() }
                }))?);
            } else {
                println!("Library statistics:");
                println!("  Total books:     {count}");
                println!("  Total tags:      {}", tags.len());
                println!("  Total libraries: {}", libs.len());
            }
        }

        Some(Commands::Doctor) => {
            let config_path = AppConfig::config_path();
            if config_path.exists() {
                println!("✓ Config: {}", config_path.display());
            } else {
                println!("○ Config: not found (using defaults)");
            }

            let db_path = config.database_path();
            let mut issues = 0;
            match Database::open(&db_path) {
                Ok(db) => {
                    let count = db.count_books().unwrap_or(0);
                    println!("✓ Database: {} ({count} books)", db_path.display());
                }
                Err(e) => { issues += 1; println!("✗ Database: {e}"); }
            }

            let cards_dir = config.cards_dir();
            if cards_dir.exists() {
                let cc = std::fs::read_dir(&cards_dir)
                    .map(|rd| rd.filter(|e| e.as_ref().is_ok_and(|e| e.path().extension().is_some_and(|ext| ext == "json"))).count())
                    .unwrap_or(0);
                println!("✓ Cards: {} ({cc} files)", cards_dir.display());
            } else {
                println!("○ Cards: directory not created yet");
            }

            if issues == 0 { println!("\nAll checks passed ✓"); }
            else { println!("\n{issues} issues found"); std::process::exit(1); }
        }

        Some(Commands::Version) => {
            let version = env!("CARGO_PKG_VERSION");
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&serde_json::json!({"status": "ok", "data": {"version": version}}))?);
            } else {
                println!("omniscope v{version}");
            }
        }
    }

    Ok(())
}

fn open_db(config: &AppConfig) -> Result<Database> {
    let db_path = config.database_path();
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(Database::open(&db_path)?)
}
