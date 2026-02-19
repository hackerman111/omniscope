use std::path::Path;
use std::time::Instant;

use anyhow::Result;
use clap::{Parser, Subcommand};

use omniscope_core::{AppConfig, BookCard, Database};
use omniscope_tui::app::App;

// ─── CLI Definition ─────────────────────────────────────────────────────────

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
    /// Also enabled by setting OMNISCOPE_JSON=1.
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// List all books in the library.
    List {
        #[arg(long, default_value = "50")]
        limit: usize,
        #[arg(long, default_value = "0")]
        offset: usize,
    },

    /// Search books by query.
    Search {
        query: String,
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
        #[arg(long)]
        file: Option<String>,
        #[arg(long)]
        title: Option<String>,
        #[arg(long, action = clap::ArgAction::Append)]
        author: Vec<String>,
        #[arg(long)]
        year: Option<i32>,
        #[arg(long, action = clap::ArgAction::Append)]
        tag: Vec<String>,
        #[arg(long)]
        library: Option<String>,
    },

    /// Import a directory of book files.
    Import {
        dir: String,
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

    /// Folder management.
    Folder {
        #[command(subcommand)]
        action: FolderAction,
    },

    /// Config management.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Show library statistics.
    Stats,

    /// Run diagnostics.
    Doctor,

    /// Show version information.
    Version,
}

// ─── Book Actions ───────────────────────────────────────────────────────────

#[derive(Subcommand)]
enum BookAction {
    /// Get a book card by ID.
    Get { id: String },

    /// Update a book card.
    Update {
        id: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long, action = clap::ArgAction::Append)]
        author: Vec<String>,
        #[arg(long)]
        year: Option<i32>,
        #[arg(long)]
        rating: Option<u8>,
        #[arg(long)]
        status: Option<String>,
    },

    /// Delete a book.
    Delete {
        id: String,
        #[arg(long)]
        confirm: bool,
    },

    /// Add or remove tags.
    Tag {
        id: String,
        #[arg(long, action = clap::ArgAction::Append)]
        add: Vec<String>,
        #[arg(long, action = clap::ArgAction::Append)]
        remove: Vec<String>,
    },

    /// Manage notes on a book.
    Note {
        id: String,
        #[command(subcommand)]
        action: NoteAction,
    },
}

#[derive(Subcommand)]
enum NoteAction {
    /// List notes on a book.
    List,
    /// Add a note.
    Add {
        content: String,
        #[arg(long)]
        heading: Option<String>,
    },
    /// Delete a note by index (0-based).
    Delete {
        index: usize,
    },
}

// ─── Tag Actions ────────────────────────────────────────────────────────────

#[derive(Subcommand)]
enum TagAction {
    /// List all tags.
    List,
    /// Create a tag.
    Create { name: String },
    /// Delete a tag (removes from all books).
    Delete { name: String, #[arg(long)] confirm: bool },
    /// Rename a tag.
    Rename { old: String, new: String },
}

// ─── Library Actions ─────────────────────────────────────────────────────────

#[derive(Subcommand)]
enum LibraryAction {
    /// List all libraries.
    List,
    /// Create a library.
    Create { name: String },
    /// Delete a library (books are NOT deleted).
    Delete { name: String, #[arg(long)] confirm: bool },
    /// Rename a library.
    Rename { old: String, new: String },
}

// ─── Folder Actions ──────────────────────────────────────────────────────────

#[derive(Subcommand)]
enum FolderAction {
    /// List folders.
    List {
        /// Parent folder ID (omit for top-level).
        #[arg(long)]
        parent: Option<String>,
    },
    /// Create a folder.
    Create {
        name: String,
        #[arg(long)]
        parent: Option<String>,
        #[arg(long)]
        library: Option<String>,
    },
    /// Delete a folder.
    Delete { id: String },
    /// Rename a folder.
    Rename { id: String, name: String },
}

// ─── Config Actions ──────────────────────────────────────────────────────────

#[derive(Subcommand)]
enum ConfigAction {
    /// Show all config values.
    List,
    /// Get a specific config key.
    Get { key: String },
    /// Set a config key (not yet implemented — planned for Phase 2).
    Set { key: String, value: String },
}

// ─── Main ────────────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    let start = Instant::now();
    let cli = Cli::parse();

    // ── Env var overrides ──────────────────────────────────────────────────
    let json_output = cli.json || std::env::var("OMNISCOPE_JSON").as_deref() == Ok("1");

    let timing = std::env::var("OMNISCOPE_TIMING").as_deref() == Ok("1");

    // Load config (honors OMNISCOPE_LIBRARY_PATH if set)
    let mut config = AppConfig::load()?;
    if let Ok(lib_path) = std::env::var("OMNISCOPE_LIBRARY_PATH") {
        config.set_library_path(lib_path.into());
    }

    if timing {
        eprintln!("[timing] config loaded in {:.1}ms", start.elapsed().as_secs_f64() * 1000.0);
    }

    match cli.command {
        None => {
            let mut app = App::new(config);
            omniscope_tui::run_tui(&mut app)?;
        }

        Some(Commands::List { limit, offset }) => {
            let db = open_db(&config)?;
            let books = db.list_books(limit, offset)?;
            let dur = start.elapsed().as_millis();

            if json_output {
                let total = db.count_books()?;
                print_json(&serde_json::json!({
                    "status": "ok",
                    "data": { "items": books, "total": total, "limit": limit, "offset": offset },
                    "meta": { "duration_ms": dur }
                }))?;
            } else if books.is_empty() {
                println!("No books in library. Use `omniscope add` to add books.");
            } else {
                for book in &books {
                    let authors = book.authors.join(", ");
                    let year = book.year.map(|y| y.to_string()).unwrap_or_default();
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
            let dur = start.elapsed().as_millis();

            if json_output {
                print_json(&serde_json::json!({
                    "status": "ok",
                    "data": { "items": results, "total": results.len(), "query": query },
                    "meta": { "duration_ms": dur }
                }))?;
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
                let cards_dir = config.cards_dir();
                let dur = start.elapsed().as_millis();
                let card_path = cards_dir.join(format!("{id}.json"));
                if card_path.exists() {
                    let card = omniscope_core::storage::json_cards::load_card(&card_path)?;
                    if json_output {
                        print_json(&serde_json::json!({"status":"ok","data":card,"meta":{"duration_ms":dur}}))?;
                    } else {
                        println!("{}", serde_json::to_string_pretty(&card)?);
                    }
                } else {
                    let db = open_db(&config)?;
                    match db.get_book_summary(&id) {
                        Ok(summary) => {
                            if json_output {
                                print_json(&serde_json::json!({"status":"ok","data":summary,"meta":{"duration_ms":dur}}))?;
                            } else {
                                println!("{}", serde_json::to_string_pretty(&summary)?);
                            }
                        }
                        Err(_) => {
                            if json_output {
                                print_json(&serde_json::json!({"status":"error","error":"not_found","message":format!("Book {id} not found"),"meta":{"duration_ms":dur}}))?;
                            } else {
                                eprintln!("Book not found: {id}");
                            }
                            std::process::exit(2);
                        }
                    }
                }
            }

            BookAction::Update { id, title, author, year, rating, status } => {
                let cards_dir = config.cards_dir();
                let uuid = match uuid::Uuid::parse_str(&id) {
                    Ok(u) => u,
                    Err(_) => {
                        eprintln!("Invalid UUID: {id}");
                        std::process::exit(2);
                    }
                };
                let mut card = match omniscope_core::storage::json_cards::load_card_by_id(&cards_dir, &uuid) {
                    Ok(c) => c,
                    Err(_) => {
                        eprintln!("Book not found: {id}");
                        std::process::exit(2);
                    }
                };

                if let Some(t) = title { card.metadata.title = t; }
                if !author.is_empty() { card.metadata.authors = author; }
                if let Some(y) = year { card.metadata.year = Some(y); }
                if let Some(r) = rating { card.organization.rating = if r == 0 { None } else { Some(r) }; }
                if let Some(s) = status {
                    card.organization.read_status = match s.as_str() {
                        "reading" => omniscope_core::ReadStatus::Reading,
                        "read"    => omniscope_core::ReadStatus::Read,
                        "dnf"     => omniscope_core::ReadStatus::Dnf,
                        _         => omniscope_core::ReadStatus::Unread,
                    };
                }

                card.updated_at = chrono::Utc::now();
                omniscope_core::storage::json_cards::save_card(&cards_dir, &card)?;
                let db = open_db(&config)?;
                db.upsert_book(&card)?;
                let dur = start.elapsed().as_millis();

                if json_output {
                    print_json(&serde_json::json!({"status":"ok","data":card,"meta":{"duration_ms":dur}}))?;
                } else {
                    println!("Updated: {}", card.metadata.title);
                }
            }

            BookAction::Delete { id, confirm } => {
                if !confirm {
                    eprintln!("Add --confirm to delete without prompt.");
                    std::process::exit(8);
                }
                let db = open_db(&config)?;
                db.delete_book(&id)?;
                omniscope_core::storage::json_cards::delete_card(
                    &config.cards_dir(),
                    &uuid::Uuid::parse_str(&id)?,
                )?;
                let dur = start.elapsed().as_millis();
                if json_output {
                    print_json(&serde_json::json!({"status":"ok","data":{"deleted":id},"meta":{"duration_ms":dur}}))?;
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
                let dur = start.elapsed().as_millis();

                if json_output {
                    print_json(&serde_json::json!({"status":"ok","data":{"tags":card.organization.tags},"meta":{"duration_ms":dur}}))?;
                } else {
                    println!("Tags: {}", card.organization.tags.join(", "));
                }
            }

            BookAction::Note { id, action } => {
                let cards_dir = config.cards_dir();
                let uuid = uuid::Uuid::parse_str(&id)?;
                let mut card = omniscope_core::storage::json_cards::load_card_by_id(&cards_dir, &uuid)?;
                let dur;

                match action {
                    NoteAction::List => {
                        dur = start.elapsed().as_millis();
                        if json_output {
                            print_json(&serde_json::json!({"status":"ok","data":{"notes":card.notes},"meta":{"duration_ms":dur}}))?;
                        } else if card.notes.is_empty() {
                            println!("No notes.");
                        } else {
                            for (i, note) in card.notes.iter().enumerate() {
                                println!("[{i}] {}", note.text);
                            }
                        }
                    }
                    NoteAction::Add { content, heading } => {
                        let note = omniscope_core::BookNote {
                            id: uuid::Uuid::now_v7(),
                            text: content,
                            author: heading.unwrap_or_else(|| "human".to_string()),
                            created_at: chrono::Utc::now(),
                        };
                        card.notes.push(note);
                        card.updated_at = chrono::Utc::now();
                        omniscope_core::storage::json_cards::save_card(&cards_dir, &card)?;
                        dur = start.elapsed().as_millis();
                        if json_output {
                            print_json(&serde_json::json!({"status":"ok","data":{"note_count":card.notes.len()},"meta":{"duration_ms":dur}}))?;
                        } else {
                            println!("Note added ({} total).", card.notes.len());
                        }
                    }
                    NoteAction::Delete { index } => {
                        if index >= card.notes.len() {
                            eprintln!("Note index {index} out of range ({})", card.notes.len());
                            std::process::exit(2);
                        }
                        card.notes.remove(index);
                        card.updated_at = chrono::Utc::now();
                        omniscope_core::storage::json_cards::save_card(&cards_dir, &card)?;
                        dur = start.elapsed().as_millis();
                        if json_output {
                            print_json(&serde_json::json!({"status":"ok","data":{"note_count":card.notes.len()},"meta":{"duration_ms":dur}}))?;
                        } else {
                            println!("Note deleted ({} remaining).", card.notes.len());
                        }
                    }
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
            let dur = start.elapsed().as_millis();

            if json_output {
                print_json(&serde_json::json!({"status":"ok","data":card,"meta":{"duration_ms":dur}}))?;
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
                if !json_output {
                    println!("  Imported: {}", card.metadata.title);
                }
            }
            let dur = start.elapsed().as_millis();

            if json_output {
                print_json(&serde_json::json!({
                    "status":"ok","data":{"imported":count,"total":db.count_books()?},
                    "meta":{"duration_ms":dur}
                }))?;
            } else {
                println!("Imported {count} book(s).");
            }
        }

        // ── Tag ────────────────────────────────────────────────────────────

        Some(Commands::Tag { action }) => match action {
            TagAction::List => {
                let db = open_db(&config)?;
                let tags = db.list_tags()?;
                let dur = start.elapsed().as_millis();
                if json_output {
                    let items: Vec<serde_json::Value> = tags.iter().map(|(n, c)| serde_json::json!({"name":n,"count":c})).collect();
                    print_json(&serde_json::json!({"status":"ok","data":items,"meta":{"duration_ms":dur}}))?;
                } else if tags.is_empty() {
                    println!("No tags.");
                } else {
                    for (name, count) in &tags { println!("  #{name} ({count})"); }
                }
            }
            TagAction::Create { name } => {
                // Tags are created implicitly when assigned to a book.
                // Here we just confirm the intent and show the name.
                let dur = start.elapsed().as_millis();
                if json_output {
                    print_json(&serde_json::json!({"status":"ok","data":{"name":name,"note":"Tags are created automatically when assigned to books"},"meta":{"duration_ms":dur}}))?;
                } else {
                    println!("Tag '#{name}' registered. Assign it to books with `omniscope book tag --add {name}`.");
                }
            }
            TagAction::Delete { name, confirm } => {
                if !confirm {
                    eprintln!("Add --confirm to delete tag from all books.");
                    std::process::exit(8);
                }
                let cards_dir = config.cards_dir();
                let db = open_db(&config)?;
                let mut removed = 0usize;
                if let Ok(cards) = omniscope_core::storage::json_cards::list_cards(&cards_dir) {
                    for mut card in cards {
                        let before = card.organization.tags.len();
                        card.organization.tags.retain(|t| t != &name);
                        if card.organization.tags.len() != before {
                            card.updated_at = chrono::Utc::now();
                            let _ = omniscope_core::storage::json_cards::save_card(&cards_dir, &card);
                            let _ = db.upsert_book(&card);
                            removed += 1;
                        }
                    }
                }
                let dur = start.elapsed().as_millis();
                if json_output {
                    print_json(&serde_json::json!({"status":"ok","data":{"deleted":name,"affected_books":removed},"meta":{"duration_ms":dur}}))?;
                } else {
                    println!("Deleted tag '#{name}' from {removed} book(s).");
                }
            }
            TagAction::Rename { old, new } => {
                let cards_dir = config.cards_dir();
                let db = open_db(&config)?;
                let mut renamed = 0usize;
                if let Ok(cards) = omniscope_core::storage::json_cards::list_cards(&cards_dir) {
                    for mut card in cards {
                        let changed = card.organization.tags.iter_mut().any(|t| {
                            if t == &old { *t = new.clone(); true } else { false }
                        });
                        if changed {
                            card.updated_at = chrono::Utc::now();
                            let _ = omniscope_core::storage::json_cards::save_card(&cards_dir, &card);
                            let _ = db.upsert_book(&card);
                            renamed += 1;
                        }
                    }
                }
                let dur = start.elapsed().as_millis();
                if json_output {
                    print_json(&serde_json::json!({"status":"ok","data":{"old":old,"new":new,"affected_books":renamed},"meta":{"duration_ms":dur}}))?;
                } else {
                    println!("Renamed tag '#{old}' → '#{new}' on {renamed} book(s).");
                }
            }
        },

        // ── Library ────────────────────────────────────────────────────────

        Some(Commands::Library { action }) => match action {
            LibraryAction::List => {
                let db = open_db(&config)?;
                let libs = db.list_libraries()?;
                let dur = start.elapsed().as_millis();
                if json_output {
                    let items: Vec<serde_json::Value> = libs.iter().map(|(n, c)| serde_json::json!({"name":n,"count":c})).collect();
                    print_json(&serde_json::json!({"status":"ok","data":items,"meta":{"duration_ms":dur}}))?;
                } else if libs.is_empty() {
                    println!("No libraries.");
                } else {
                    for (name, count) in &libs { println!("  📁 {name} ({count})"); }
                }
            }
            LibraryAction::Create { name } => {
                let dur = start.elapsed().as_millis();
                if json_output {
                    print_json(&serde_json::json!({"status":"ok","data":{"name":name,"note":"Libraries are created automatically when assigned to books"},"meta":{"duration_ms":dur}}))?;
                } else {
                    println!("Library '{name}' registered. Add books with `omniscope add --library \"{name}\"`.");
                }
            }
            LibraryAction::Delete { name, confirm } => {
                if !confirm {
                    eprintln!("Add --confirm to remove library from all books.");
                    std::process::exit(8);
                }
                let cards_dir = config.cards_dir();
                let db = open_db(&config)?;
                let mut removed = 0usize;
                if let Ok(cards) = omniscope_core::storage::json_cards::list_cards(&cards_dir) {
                    for mut card in cards {
                        let before = card.organization.libraries.len();
                        card.organization.libraries.retain(|l| l != &name);
                        if card.organization.libraries.len() != before {
                            card.updated_at = chrono::Utc::now();
                            let _ = omniscope_core::storage::json_cards::save_card(&cards_dir, &card);
                            let _ = db.upsert_book(&card);
                            removed += 1;
                        }
                    }
                }
                let dur = start.elapsed().as_millis();
                if json_output {
                    print_json(&serde_json::json!({"status":"ok","data":{"deleted":name,"affected_books":removed},"meta":{"duration_ms":dur}}))?;
                } else {
                    println!("Removed library '{name}' from {removed} book(s).");
                }
            }
            LibraryAction::Rename { old, new } => {
                let cards_dir = config.cards_dir();
                let db = open_db(&config)?;
                let mut renamed = 0usize;
                if let Ok(cards) = omniscope_core::storage::json_cards::list_cards(&cards_dir) {
                    for mut card in cards {
                        let changed = card.organization.libraries.iter_mut().any(|l| {
                            if l == &old { *l = new.clone(); true } else { false }
                        });
                        if changed {
                            card.updated_at = chrono::Utc::now();
                            let _ = omniscope_core::storage::json_cards::save_card(&cards_dir, &card);
                            let _ = db.upsert_book(&card);
                            renamed += 1;
                        }
                    }
                }
                let dur = start.elapsed().as_millis();
                if json_output {
                    print_json(&serde_json::json!({"status":"ok","data":{"old":old,"new":new,"affected_books":renamed},"meta":{"duration_ms":dur}}))?;
                } else {
                    println!("Renamed library '{old}' → '{new}' on {renamed} book(s).");
                }
            }
        },

        // ── Folder ─────────────────────────────────────────────────────────

        Some(Commands::Folder { action }) => match action {
            FolderAction::List { parent } => {
                let db = open_db(&config)?;
                let folders = db.list_folders(parent.as_deref())?;
                let dur = start.elapsed().as_millis();
                if json_output {
                    let items: Vec<serde_json::Value> = folders.iter().map(|f| serde_json::json!({"id":f.id,"name":f.name,"parent_id":f.parent_id,"library_id":f.library_id})).collect();
                    print_json(&serde_json::json!({"status":"ok","data":items,"meta":{"duration_ms":dur}}))?;
                } else if folders.is_empty() {
                    println!("No folders.");
                } else {
                    for f in &folders { println!("  {} — {}", &f.id[..8], f.name); }
                }
            }
            FolderAction::Create { name, parent, library } => {
                let db = open_db(&config)?;
                let id = db.create_folder(&name, parent.as_deref(), library.as_deref())?;
                let dur = start.elapsed().as_millis();
                if json_output {
                    print_json(&serde_json::json!({"status":"ok","data":{"id":id,"name":name},"meta":{"duration_ms":dur}}))?;
                } else {
                    println!("Created folder '{}' ({}).", name, &id[..8]);
                }
            }
            FolderAction::Delete { id } => {
                let db = open_db(&config)?;
                db.delete_folder(&id)?;
                let dur = start.elapsed().as_millis();
                if json_output {
                    print_json(&serde_json::json!({"status":"ok","data":{"deleted":id},"meta":{"duration_ms":dur}}))?;
                } else {
                    println!("Deleted folder: {id}");
                }
            }
            FolderAction::Rename { id, name } => {
                let db = open_db(&config)?;
                db.rename_folder(&id, &name)?;
                let dur = start.elapsed().as_millis();
                if json_output {
                    print_json(&serde_json::json!({"status":"ok","data":{"id":id,"name":name},"meta":{"duration_ms":dur}}))?;
                } else {
                    println!("Renamed folder to '{name}'.");
                }
            }
        },

        // ── Config ─────────────────────────────────────────────────────────

        Some(Commands::Config { action }) => {
            let dur = start.elapsed().as_millis();
            match action {
                ConfigAction::List => {
                    let kv = config_key_values(&config);
                    if json_output {
                        print_json(&serde_json::json!({"status":"ok","data":kv,"meta":{"duration_ms":dur}}))?;
                    } else {
                        for (k, v) in &kv {
                            println!("{k} = {v}");
                        }
                    }
                }
                ConfigAction::Get { key } => {
                    let kv = config_key_values(&config);
                    match kv.get(key.as_str()) {
                        Some(val) => {
                            if json_output {
                                print_json(&serde_json::json!({"status":"ok","data":{"key":key,"value":val},"meta":{"duration_ms":dur}}))?;
                            } else {
                                println!("{val}");
                            }
                        }
                        None => {
                            eprintln!("Unknown config key: {key}");
                            std::process::exit(2);
                        }
                    }
                }
                ConfigAction::Set { key, value } => {
                    // Not persisted yet — planned for Phase 2 (figment integration)
                    eprintln!("Config set is not yet implemented. Edit the config file directly.");
                    eprintln!("  Key: {key}, Value: {value}");
                    std::process::exit(1);
                }
            }
        }

        // ── Stats ──────────────────────────────────────────────────────────

        Some(Commands::Stats) => {
            let db = open_db(&config)?;
            let count = db.count_books()?;
            let tags = db.list_tags()?;
            let libs = db.list_libraries()?;
            let dur = start.elapsed().as_millis();

            if json_output {
                print_json(&serde_json::json!({
                    "status":"ok",
                    "data":{"total_books":count,"total_tags":tags.len(),"total_libraries":libs.len()},
                    "meta":{"duration_ms":dur}
                }))?;
            } else {
                println!("Library statistics:");
                println!("  Total books:     {count}");
                println!("  Total tags:      {}", tags.len());
                println!("  Total libraries: {}", libs.len());
            }
        }

        // ── Doctor ─────────────────────────────────────────────────────────

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

        // ── Version ────────────────────────────────────────────────────────

        Some(Commands::Version) => {
            let version = env!("CARGO_PKG_VERSION");
            let dur = start.elapsed().as_millis();
            if json_output {
                print_json(&serde_json::json!({"status":"ok","data":{"version":version},"meta":{"duration_ms":dur}}))?;
            } else {
                println!("omniscope v{version}");
            }
        }
    }

    if timing {
        eprintln!("[timing] total {:.1}ms", start.elapsed().as_secs_f64() * 1000.0);
    }

    Ok(())
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn print_json(val: &serde_json::Value) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(val)?);
    Ok(())
}

fn open_db(config: &AppConfig) -> Result<Database> {
    let db_path = config.database_path();
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(Database::open(&db_path)?)
}

fn config_key_values(config: &AppConfig) -> std::collections::HashMap<&'static str, String> {
    let mut map = std::collections::HashMap::new();
    map.insert("library_path", config.library_path().to_string_lossy().to_string());
    map.insert("cards_dir", config.cards_dir().to_string_lossy().to_string());
    map.insert("database_path", config.database_path().to_string_lossy().to_string());
    map
}
