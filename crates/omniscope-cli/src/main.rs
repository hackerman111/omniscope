use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::Result;
use clap::{Parser, Subcommand};

use omniscope_core::{
    AppConfig, BookCard, Database, FolderTemplate, GlobalConfig, InitOptions, LibraryRoot,
    ScanOptions, init_library, scaffold_template, scan_library, sync_folders,
};
use omniscope_science::enrichment::EnrichmentPipeline;
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

    /// Initialize a new library in a directory.
    Init {
        /// Directory to initialize (defaults to current directory).
        #[arg(default_value = ".")]
        path: String,

        /// Name for this library.
        #[arg(long)]
        name: Option<String>,

        /// Create the directory if it doesn't exist.
        #[arg(long)]
        create_dir: bool,

        /// Scan existing files and create book cards.
        #[arg(long)]
        scan_existing: bool,
    },

    /// Scan the library directory for new/changed book files.
    Scan {
        /// Automatically create cards for discovered files.
        #[arg(long)]
        auto_create_cards: bool,
    },

    /// Sync the file system with the library database.
    Sync,

    /// Manage known libraries.
    Libraries {
        #[command(subcommand)]
        action: LibrariesAction,
    },
}

// ─── Libraries Actions ──────────────────────────────────────────────────────

#[derive(Subcommand)]
enum LibrariesAction {
    /// List all known libraries.
    List,
    /// Forget a library (remove from registry; data is NOT deleted).
    Forget { path: String },
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
    Delete { index: usize },
}

// ─── Tag Actions ────────────────────────────────────────────────────────────

#[derive(Subcommand)]
enum TagAction {
    /// List all tags.
    List,
    /// Create a tag.
    Create { name: String },
    /// Delete a tag (removes from all books).
    Delete {
        name: String,
        #[arg(long)]
        confirm: bool,
    },
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
    Delete {
        name: String,
        #[arg(long)]
        confirm: bool,
    },
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
    /// Scaffold directories from a template.
    Scaffold {
        /// Template name: research, personal, technical.
        template: String,
        /// Preview only — don't create anything.
        #[arg(long)]
        dry_run: bool,
    },
    /// Sync disk folders with the library database.
    Sync,
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

    // Load global config
    let global_config = GlobalConfig::load()?;

    // Load legacy AppConfig (for backward compat during migration)
    let mut config = AppConfig::load()?;
    if let Ok(lib_path) = std::env::var("OMNISCOPE_LIBRARY_PATH") {
        config.set_library_path(lib_path.into());
    }

    // Discover library root from CWD using the full fallback chain
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let library_root = LibraryRoot::discover_with_fallbacks(&cwd, &global_config);

    if timing {
        if let Some(ref lr) = library_root {
            eprintln!(
                "[timing] library discovered at {} in {:.1}ms",
                lr.root().display(),
                start.elapsed().as_secs_f64() * 1000.0
            );
        } else {
            eprintln!(
                "[timing] no library found, config loaded in {:.1}ms",
                start.elapsed().as_secs_f64() * 1000.0
            );
        }
    }

    match cli.command {
        None => {
            let mut app = App::new(config, library_root.clone());
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
                        print_json(
                            &serde_json::json!({"status":"ok","data":card,"meta":{"duration_ms":dur}}),
                        )?;
                    } else {
                        println!("{}", serde_json::to_string_pretty(&card)?);
                    }
                } else {
                    let db = open_db(&config)?;
                    match db.get_book_summary(&id) {
                        Ok(summary) => {
                            if json_output {
                                print_json(
                                    &serde_json::json!({"status":"ok","data":summary,"meta":{"duration_ms":dur}}),
                                )?;
                            } else {
                                println!("{}", serde_json::to_string_pretty(&summary)?);
                            }
                        }
                        Err(_) => {
                            if json_output {
                                print_json(
                                    &serde_json::json!({"status":"error","error":"not_found","message":format!("Book {id} not found"),"meta":{"duration_ms":dur}}),
                                )?;
                            } else {
                                eprintln!("Book not found: {id}");
                            }
                            std::process::exit(2);
                        }
                    }
                }
            }

            BookAction::Update {
                id,
                title,
                author,
                year,
                rating,
                status,
            } => {
                let cards_dir = config.cards_dir();
                let uuid = match uuid::Uuid::parse_str(&id) {
                    Ok(u) => u,
                    Err(_) => {
                        eprintln!("Invalid UUID: {id}");
                        std::process::exit(2);
                    }
                };
                let mut card =
                    match omniscope_core::storage::json_cards::load_card_by_id(&cards_dir, &uuid) {
                        Ok(c) => c,
                        Err(_) => {
                            eprintln!("Book not found: {id}");
                            std::process::exit(2);
                        }
                    };

                if let Some(t) = title {
                    card.metadata.title = t;
                }
                if !author.is_empty() {
                    card.metadata.authors = author;
                }
                if let Some(y) = year {
                    card.metadata.year = Some(y);
                }
                if let Some(r) = rating {
                    card.organization.rating = if r == 0 { None } else { Some(r) };
                }
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
                let dur = start.elapsed().as_millis();

                if json_output {
                    print_json(
                        &serde_json::json!({"status":"ok","data":card,"meta":{"duration_ms":dur}}),
                    )?;
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
                    print_json(
                        &serde_json::json!({"status":"ok","data":{"deleted":id},"meta":{"duration_ms":dur}}),
                    )?;
                } else {
                    println!("Deleted book: {id}");
                }
            }

            BookAction::Tag { id, add, remove } => {
                let cards_dir = config.cards_dir();
                let uuid = uuid::Uuid::parse_str(&id)?;
                let mut card =
                    omniscope_core::storage::json_cards::load_card_by_id(&cards_dir, &uuid)?;

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
                    print_json(
                        &serde_json::json!({"status":"ok","data":{"tags":card.organization.tags},"meta":{"duration_ms":dur}}),
                    )?;
                } else {
                    println!("Tags: {}", card.organization.tags.join(", "));
                }
            }

            BookAction::Note { id, action } => {
                let cards_dir = config.cards_dir();
                let uuid = uuid::Uuid::parse_str(&id)?;
                let mut card =
                    omniscope_core::storage::json_cards::load_card_by_id(&cards_dir, &uuid)?;
                let dur;

                match action {
                    NoteAction::List => {
                        dur = start.elapsed().as_millis();
                        if json_output {
                            print_json(
                                &serde_json::json!({"status":"ok","data":{"notes":card.notes},"meta":{"duration_ms":dur}}),
                            )?;
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
                            print_json(
                                &serde_json::json!({"status":"ok","data":{"note_count":card.notes.len()},"meta":{"duration_ms":dur}}),
                            )?;
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
                            print_json(
                                &serde_json::json!({"status":"ok","data":{"note_count":card.notes.len()},"meta":{"duration_ms":dur}}),
                            )?;
                        } else {
                            println!("Note deleted ({} remaining).", card.notes.len());
                        }
                    }
                }
            }
        },

        Some(Commands::Add {
            file,
            title,
            author,
            year,
            tag,
            library,
        }) => {
            let mut enrichment_report = None;
            let mut card = if let Some(ref file_path) = file {
                let mut imported = omniscope_core::file_import::import_file(Path::new(file_path))?;
                enrichment_report = Some(enrich_card_metadata(&mut imported));
                imported
            } else {
                BookCard::new(title.as_deref().unwrap_or("Untitled"))
            };

            if let Some(t) = title {
                card.metadata.title = t;
            }
            if !author.is_empty() {
                card.metadata.authors = author;
            }
            if let Some(y) = year {
                card.metadata.year = Some(y);
            }
            card.organization.tags = tag;
            if let Some(lib) = library {
                card.organization.libraries.push(lib);
            }

            let cards_dir = resolve_cards_dir(&library_root, &config);
            omniscope_core::storage::json_cards::save_card(&cards_dir, &card)?;
            let db = resolve_db(&library_root, &config)?;
            db.upsert_book(&card)?;
            let dur = start.elapsed().as_millis();

            if json_output {
                let enrichment_json = enrichment_report.as_ref().map(|report| {
                    serde_json::json!({
                        "fields_updated": report.fields_updated.len(),
                        "warnings": report.errors.len(),
                    })
                });
                print_json(
                    &serde_json::json!({"status":"ok","data":card,"enrichment":enrichment_json,"meta":{"duration_ms":dur}}),
                )?;
            } else {
                println!("Added: {} ({})", card.metadata.title, card.id);
                if let Some(report) = enrichment_report {
                    println!(
                        "  Metadata: {} field(s) updated, {} warning(s)",
                        report.fields_updated.len(),
                        report.errors.len()
                    );
                }
            }
        }

        Some(Commands::Import { dir, recursive }) => {
            let cards = omniscope_core::file_import::scan_directory(Path::new(&dir), recursive)?;
            let db = resolve_db(&library_root, &config)?;
            let cards_dir = resolve_cards_dir(&library_root, &config);
            let mut count = 0;
            let mut created_count = 0usize;
            let mut updated_existing_count = 0usize;
            let mut cards_with_metadata_updates = 0usize;
            let mut updated_fields_total = 0usize;
            let mut warnings_total = 0usize;

            let mut existing_by_path = std::collections::HashMap::new();
            if let Ok(existing_cards) = omniscope_core::storage::json_cards::list_cards(&cards_dir)
            {
                for existing_card in existing_cards {
                    if let Some(file) = existing_card.file.as_ref() {
                        existing_by_path.insert(file.path.clone(), existing_card);
                    }
                }
            }

            for scanned_card in cards {
                let (mut card, is_update) =
                    if let Some(path) = scanned_card.file.as_ref().map(|file| file.path.clone()) {
                        if let Some(existing_card) = existing_by_path.remove(&path) {
                            updated_existing_count += 1;
                            (existing_card, true)
                        } else {
                            created_count += 1;
                            (scanned_card, false)
                        }
                    } else {
                        created_count += 1;
                        (scanned_card, false)
                    };

                let report = enrich_card_metadata(&mut card);
                if !report.fields_updated.is_empty() {
                    cards_with_metadata_updates += 1;
                    updated_fields_total += report.fields_updated.len();
                }
                warnings_total += report.errors.len();

                omniscope_core::storage::json_cards::save_card(&cards_dir, &card)?;
                db.upsert_book(&card)?;
                count += 1;
                if !json_output {
                    if is_update {
                        println!("  Updated: {}", card.metadata.title);
                    } else {
                        println!("  Imported: {}", card.metadata.title);
                    }
                }
            }
            let dur = start.elapsed().as_millis();

            if json_output {
                print_json(&serde_json::json!({
                    "status":"ok","data":{"processed":count,"created":created_count,"updated":updated_existing_count,"total":db.count_books()?,"metadata":{"cards_updated":cards_with_metadata_updates,"fields_updated":updated_fields_total,"warnings":warnings_total}},
                    "meta":{"duration_ms":dur}
                }))?;
            } else {
                println!(
                    "Processed {count} book(s): created {created_count}, updated {updated_existing_count}."
                );
                println!(
                    "Metadata enriched: {} card(s), {} field(s), {} warning(s).",
                    cards_with_metadata_updates, updated_fields_total, warnings_total
                );
            }
        }

        // ── Tag ────────────────────────────────────────────────────────────
        Some(Commands::Tag { action }) => match action {
            TagAction::List => {
                let db = open_db(&config)?;
                let tags = db.list_tags()?;
                let dur = start.elapsed().as_millis();
                if json_output {
                    let items: Vec<serde_json::Value> = tags
                        .iter()
                        .map(|(n, c)| serde_json::json!({"name":n,"count":c}))
                        .collect();
                    print_json(
                        &serde_json::json!({"status":"ok","data":items,"meta":{"duration_ms":dur}}),
                    )?;
                } else if tags.is_empty() {
                    println!("No tags.");
                } else {
                    for (name, count) in &tags {
                        println!("  #{name} ({count})");
                    }
                }
            }
            TagAction::Create { name } => {
                // Tags are created implicitly when assigned to a book.
                // Here we just confirm the intent and show the name.
                let dur = start.elapsed().as_millis();
                if json_output {
                    print_json(
                        &serde_json::json!({"status":"ok","data":{"name":name,"note":"Tags are created automatically when assigned to books"},"meta":{"duration_ms":dur}}),
                    )?;
                } else {
                    println!(
                        "Tag '#{name}' registered. Assign it to books with `omniscope book tag --add {name}`."
                    );
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
                            let _ =
                                omniscope_core::storage::json_cards::save_card(&cards_dir, &card);
                            let _ = db.upsert_book(&card);
                            removed += 1;
                        }
                    }
                }
                let dur = start.elapsed().as_millis();
                if json_output {
                    print_json(
                        &serde_json::json!({"status":"ok","data":{"deleted":name,"affected_books":removed},"meta":{"duration_ms":dur}}),
                    )?;
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
                            if t == &old {
                                *t = new.clone();
                                true
                            } else {
                                false
                            }
                        });
                        if changed {
                            card.updated_at = chrono::Utc::now();
                            let _ =
                                omniscope_core::storage::json_cards::save_card(&cards_dir, &card);
                            let _ = db.upsert_book(&card);
                            renamed += 1;
                        }
                    }
                }
                let dur = start.elapsed().as_millis();
                if json_output {
                    print_json(
                        &serde_json::json!({"status":"ok","data":{"old":old,"new":new,"affected_books":renamed},"meta":{"duration_ms":dur}}),
                    )?;
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
                    let items: Vec<serde_json::Value> = libs
                        .iter()
                        .map(|(n, c)| serde_json::json!({"name":n,"count":c}))
                        .collect();
                    print_json(
                        &serde_json::json!({"status":"ok","data":items,"meta":{"duration_ms":dur}}),
                    )?;
                } else if libs.is_empty() {
                    println!("No libraries.");
                } else {
                    for (name, count) in &libs {
                        println!("  📁 {name} ({count})");
                    }
                }
            }
            LibraryAction::Create { name } => {
                let dur = start.elapsed().as_millis();
                if json_output {
                    print_json(
                        &serde_json::json!({"status":"ok","data":{"name":name,"note":"Libraries are created automatically when assigned to books"},"meta":{"duration_ms":dur}}),
                    )?;
                } else {
                    println!(
                        "Library '{name}' registered. Add books with `omniscope add --library \"{name}\"`."
                    );
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
                            let _ =
                                omniscope_core::storage::json_cards::save_card(&cards_dir, &card);
                            let _ = db.upsert_book(&card);
                            removed += 1;
                        }
                    }
                }
                let dur = start.elapsed().as_millis();
                if json_output {
                    print_json(
                        &serde_json::json!({"status":"ok","data":{"deleted":name,"affected_books":removed},"meta":{"duration_ms":dur}}),
                    )?;
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
                            if l == &old {
                                *l = new.clone();
                                true
                            } else {
                                false
                            }
                        });
                        if changed {
                            card.updated_at = chrono::Utc::now();
                            let _ =
                                omniscope_core::storage::json_cards::save_card(&cards_dir, &card);
                            let _ = db.upsert_book(&card);
                            renamed += 1;
                        }
                    }
                }
                let dur = start.elapsed().as_millis();
                if json_output {
                    print_json(
                        &serde_json::json!({"status":"ok","data":{"old":old,"new":new,"affected_books":renamed},"meta":{"duration_ms":dur}}),
                    )?;
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
                    print_json(
                        &serde_json::json!({"status":"ok","data":items,"meta":{"duration_ms":dur}}),
                    )?;
                } else if folders.is_empty() {
                    println!("No folders.");
                } else {
                    for f in &folders {
                        println!("  {} — {}", &f.id[..8], f.name);
                    }
                }
            }
            FolderAction::Create {
                name,
                parent,
                library,
            } => {
                let db = open_db(&config)?;
                let id = db.create_folder(&name, parent.as_deref(), library.as_deref())?;
                let dur = start.elapsed().as_millis();
                if json_output {
                    print_json(
                        &serde_json::json!({"status":"ok","data":{"id":id,"name":name},"meta":{"duration_ms":dur}}),
                    )?;
                } else {
                    println!("Created folder '{}' ({}).", name, &id[..8]);
                }
            }
            FolderAction::Delete { id } => {
                let db = open_db(&config)?;
                db.delete_folder(&id)?;
                let dur = start.elapsed().as_millis();
                if json_output {
                    print_json(
                        &serde_json::json!({"status":"ok","data":{"deleted":id},"meta":{"duration_ms":dur}}),
                    )?;
                } else {
                    println!("Deleted folder: {id}");
                }
            }
            FolderAction::Rename { id, name } => {
                let db = open_db(&config)?;
                db.rename_folder(&id, &name)?;
                let dur = start.elapsed().as_millis();
                if json_output {
                    print_json(
                        &serde_json::json!({"status":"ok","data":{"id":id,"name":name},"meta":{"duration_ms":dur}}),
                    )?;
                } else {
                    println!("Renamed folder to '{name}'.");
                }
            }
            FolderAction::Scaffold { template, dry_run } => {
                let lr = require_library(&library_root, json_output)?;
                let db = open_db_from_root(&lr)?;

                let tmpl = FolderTemplate::from_str(&template).ok_or_else(|| {
                    anyhow::anyhow!(
                        "Unknown template '{}'. Available: research, personal, technical",
                        template
                    )
                })?;

                let created = scaffold_template(&lr, &db, tmpl, dry_run)?;
                let dur = start.elapsed().as_millis();

                if json_output {
                    print_json(&serde_json::json!({
                        "status": "ok",
                        "data": { "created": created, "dry_run": dry_run },
                        "meta": { "duration_ms": dur }
                    }))?;
                } else if dry_run {
                    println!("Would create {} directories:", created.len());
                    for d in &created {
                        println!("  📁 {}/{}", lr.root().display(), d);
                    }
                    println!("\nRun without --dry-run to apply.");
                } else if created.is_empty() {
                    println!("All directories already exist. Nothing to create.");
                } else {
                    println!("Created {} directories:", created.len());
                    for d in &created {
                        println!("  📁 {}", d);
                    }
                }
            }
            FolderAction::Sync => {
                let lr = require_library(&library_root, json_output)?;
                let db = open_db_from_root(&lr)?;

                let report = sync_folders(&lr, &db)?;
                let dur = start.elapsed().as_millis();

                if json_output {
                    print_json(&serde_json::json!({
                        "status": "ok",
                        "data": {
                            "in_sync": report.in_sync,
                            "new_on_disk": report.new_on_disk,
                            "missing_on_disk": report.missing_on_disk,
                            "untracked_files": report.untracked_files.iter()
                                .map(|p| p.display().to_string()).collect::<Vec<_>>(),
                        },
                        "meta": { "duration_ms": dur }
                    }))?;
                } else {
                    println!("Folder sync report:");
                    println!("  ✓  {} folder(s) in sync", report.in_sync);
                    if !report.new_on_disk.is_empty() {
                        println!(
                            "  ⊕  {} new folder(s) on disk (not tracked):",
                            report.new_on_disk.len()
                        );
                        for f in &report.new_on_disk {
                            println!("       {}", f);
                        }
                    }
                    if !report.missing_on_disk.is_empty() {
                        println!(
                            "  ⊘  {} folder(s) missing on disk:",
                            report.missing_on_disk.len()
                        );
                        for f in &report.missing_on_disk {
                            println!("       {}", f);
                        }
                    }
                    if !report.untracked_files.is_empty() {
                        println!("  ？ {} untracked file(s):", report.untracked_files.len());
                        for f in &report.untracked_files {
                            println!("       {}", f.display());
                        }
                    }
                    if report.is_clean() {
                        println!("\nAll clean ✓");
                    }
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
                        print_json(
                            &serde_json::json!({"status":"ok","data":kv,"meta":{"duration_ms":dur}}),
                        )?;
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
                                print_json(
                                    &serde_json::json!({"status":"ok","data":{"key":key,"value":val},"meta":{"duration_ms":dur}}),
                                )?;
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

            let mut issues = 0;

            if let Some(ref lr) = library_root {
                println!("✓ Library: {}", lr.root().display());
                let db_path = lr.database_path();
                match Database::open(&db_path) {
                    Ok(db) => {
                        let count = db.count_books().unwrap_or(0);
                        println!("✓ Database: {} ({count} books)", db_path.display());
                    }
                    Err(e) => {
                        issues += 1;
                        println!("✗ Database: {e}");
                    }
                }

                let cards_dir = lr.cards_dir();
                if cards_dir.exists() {
                    let cc = std::fs::read_dir(&cards_dir)
                        .map(|rd| {
                            rd.filter(|e| {
                                e.as_ref().is_ok_and(|e| {
                                    e.path().extension().is_some_and(|ext| ext == "json")
                                })
                            })
                            .count()
                        })
                        .unwrap_or(0);
                    println!("✓ Cards: {} ({cc} files)", cards_dir.display());
                } else {
                    println!("○ Cards: directory not created yet");
                }
            } else {
                println!("○ Library: not found (run 'omniscope init' to create one)");
                // Fall back to legacy path check
                let db_path = config.database_path();
                if db_path.exists() {
                    println!("  ↳ Legacy DB found at: {}", db_path.display());
                }
            }

            if issues == 0 {
                println!("\nAll checks passed ✓");
            } else {
                println!("\n{issues} issues found");
                std::process::exit(1);
            }
        }

        // ── Version ────────────────────────────────────────────────────────
        Some(Commands::Version) => {
            let version = env!("CARGO_PKG_VERSION");
            let dur = start.elapsed().as_millis();
            if json_output {
                print_json(
                    &serde_json::json!({"status":"ok","data":{"version":version},"meta":{"duration_ms":dur}}),
                )?;
            } else {
                println!("omniscope v{version}");
            }
        }

        // ── Init ───────────────────────────────────────────────────────────
        Some(Commands::Init {
            path,
            name,
            create_dir,
            scan_existing,
        }) => {
            let root = PathBuf::from(&path)
                .canonicalize()
                .unwrap_or_else(|_| PathBuf::from(&path));
            let opts = InitOptions {
                name: name.clone(),
                create_dir,
                scan_existing,
            };

            match init_library(&root, opts) {
                Ok(lr) => {
                    let manifest = lr.load_manifest()?;
                    let dur = start.elapsed().as_millis();

                    // Register in global config
                    let mut gc = GlobalConfig::load()?;
                    gc.add_library(&manifest.library.name, lr.root(), &manifest.library.id);
                    let _ = gc.save();

                    if json_output {
                        print_json(&serde_json::json!({
                            "status": "ok",
                            "data": {
                                "root": lr.root().display().to_string(),
                                "name": manifest.library.name,
                                "id": manifest.library.id,
                            },
                            "meta": { "duration_ms": dur }
                        }))?;
                    } else {
                        println!(
                            "Initialized library '{}' at {}",
                            manifest.library.name,
                            lr.root().display()
                        );
                        println!("  ID: {}", manifest.library.id);
                        println!(
                            "  .libr/ directory created with: cards/, db/, cache/, undo/, backups/"
                        );
                        if scan_existing {
                            // Scan will be implemented in Phase C
                            println!("  (--scan-existing is not yet implemented)");
                        }
                    }
                }
                Err(e) => {
                    if json_output {
                        print_json(&serde_json::json!({"status":"error","error":e.to_string()}))?;
                    } else {
                        eprintln!("Error: {e}");
                    }
                    std::process::exit(1);
                }
            }
        }

        // ── Scan ───────────────────────────────────────────────────────────
        Some(Commands::Scan { auto_create_cards }) => {
            let lr = require_library(&library_root, json_output)?;
            let db = open_db_from_root(&lr)?;

            let opts = ScanOptions {
                auto_create_cards,
                recursive: true,
                subdirectory: None,
            };

            let result = scan_library(&lr, &db, opts)?;
            let dur = start.elapsed().as_millis();

            if json_output {
                print_json(&serde_json::json!({
                    "status": "ok",
                    "data": {
                        "total_files": result.total_files,
                        "known_files": result.known_files,
                        "new_files": result.new_files.len(),
                        "cards_created": result.cards_created,
                        "errors": result.errors.len(),
                    },
                    "meta": { "duration_ms": dur }
                }))?;
            } else {
                println!("Scan complete:");
                println!("  Total files: {}", result.total_files);
                println!("  Known:       {}", result.known_files);
                println!("  New:         {}", result.new_files.len());
                if auto_create_cards {
                    println!("  Created:     {}", result.cards_created);
                }
                if !result.errors.is_empty() {
                    println!("  Errors:      {}", result.errors.len());
                    for (path, err) in &result.errors {
                        eprintln!("    {} — {}", path.display(), err);
                    }
                }
                if !result.new_files.is_empty() && !auto_create_cards {
                    println!("\n  Use --auto-create-cards to create cards for new files.");
                }
            }
        }

        // ── Sync ───────────────────────────────────────────────────────────
        Some(Commands::Sync) => {
            let lr = require_library(&library_root, json_output)?;
            let db = open_db_from_root(&lr)?;
            let cards_dir = lr.cards_dir();
            if cards_dir.exists() {
                let count = db.sync_from_cards(&cards_dir)?;
                let dur = start.elapsed().as_millis();
                if json_output {
                    print_json(
                        &serde_json::json!({"status":"ok","data":{"synced":count},"meta":{"duration_ms":dur}}),
                    )?;
                } else {
                    println!("Synced {count} card(s) into database.");
                }
            } else {
                println!("No cards directory found. Nothing to sync.");
            }
        }

        // ── Libraries ──────────────────────────────────────────────────────
        Some(Commands::Libraries { action }) => match action {
            LibrariesAction::List => {
                let gc = GlobalConfig::load()?;
                let dur = start.elapsed().as_millis();
                if json_output {
                    let items: Vec<serde_json::Value> = gc
                        .known_libraries()
                        .iter()
                        .map(|l| serde_json::json!({"name": l.name, "path": l.path, "id": l.id}))
                        .collect();
                    print_json(
                        &serde_json::json!({"status":"ok","data":items,"meta":{"duration_ms":dur}}),
                    )?;
                } else if gc.known_libraries().is_empty() {
                    println!(
                        "No known libraries. Run 'omniscope init' in a directory to create one."
                    );
                } else {
                    for lib in gc.known_libraries() {
                        let active = library_root
                            .as_ref()
                            .is_some_and(|lr| lr.root().to_string_lossy() == lib.path);
                        let marker = if active { " *" } else { "" };
                        println!("  📚 {} — {}{marker}", lib.name, lib.path);
                    }
                }
            }
            LibrariesAction::Forget { path } => {
                let mut gc = GlobalConfig::load()?;
                gc.remove_library(Path::new(&path));
                gc.save()?;
                let dur = start.elapsed().as_millis();
                if json_output {
                    print_json(
                        &serde_json::json!({"status":"ok","data":{"forgotten":path},"meta":{"duration_ms":dur}}),
                    )?;
                } else {
                    println!(
                        "Removed '{}' from known libraries. Data is NOT deleted.",
                        path
                    );
                }
            }
        },
    }

    if timing {
        eprintln!(
            "[timing] total {:.1}ms",
            start.elapsed().as_secs_f64() * 1000.0
        );
    }

    Ok(())
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn print_json(val: &serde_json::Value) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(val)?);
    Ok(())
}

/// Open a database from a discovered LibraryRoot.
fn open_db_from_root(lr: &LibraryRoot) -> Result<Database> {
    let db_path = lr.database_path();
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(Database::open(&db_path)?)
}

/// Legacy: open DB using config paths (for backward compat when no .libr/ exists).
fn open_db(config: &AppConfig) -> Result<Database> {
    let db_path = config.database_path();
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(Database::open(&db_path)?)
}

/// Resolve the database: prefer LibraryRoot, fall back to legacy config.
fn resolve_db(library_root: &Option<LibraryRoot>, config: &AppConfig) -> Result<Database> {
    if let Some(lr) = library_root {
        open_db_from_root(lr)
    } else {
        open_db(config)
    }
}

/// Resolve the cards directory: prefer LibraryRoot, fall back to legacy config.
fn resolve_cards_dir(library_root: &Option<LibraryRoot>, config: &AppConfig) -> PathBuf {
    if let Some(lr) = library_root {
        lr.cards_dir()
    } else {
        config.cards_dir()
    }
}

/// Require a library root (exit with error if not found).
fn require_library(library_root: &Option<LibraryRoot>, json_output: bool) -> Result<LibraryRoot> {
    match library_root {
        Some(lr) => Ok(lr.clone()),
        None => {
            if json_output {
                print_json(&serde_json::json!({
                    "status": "error",
                    "error": "no_library",
                    "message": "No library found. Run 'omniscope init' in your books directory."
                }))?;
            } else {
                eprintln!("No library found. Run 'omniscope init' in your books directory.");
            }
            std::process::exit(1);
        }
    }
}

fn enrich_card_metadata(card: &mut BookCard) -> omniscope_science::enrichment::EnrichmentReport {
    EnrichmentPipeline::enrich_full_metadata_blocking(card)
}

fn config_key_values(config: &AppConfig) -> std::collections::HashMap<&'static str, String> {
    let mut map = std::collections::HashMap::new();
    map.insert(
        "library_path",
        config.library_path().to_string_lossy().to_string(),
    );
    map.insert(
        "cards_dir",
        config.cards_dir().to_string_lossy().to_string(),
    );
    map.insert(
        "database_path",
        config.database_path().to_string_lossy().to_string(),
    );
    map
}
