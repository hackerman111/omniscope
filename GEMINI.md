# Gemini CLI Context: Omniscope

This document provides essential context and instructions for AI agents working on the Omniscope project.

## Project Overview

**Omniscope** is a high-performance, terminal-based library manager for books and scientific papers, built in Rust (Edition 2024). It is designed as a lightweight, Vim-first alternative to Zotero, featuring a Ratatui-powered TUI, powerful search capabilities, and planned AI integration.

### Core Architecture

- **Workspace:** The project is a Rust workspace with 7 crates:
    - `omniscope-core`: Business logic, models, storage, and search.
    - `omniscope-tui`: Terminal User Interface and Vim engine.
    - `omniscope-cli`: CLI entry point and commands.
    - `omniscope-science`: (In development) Metadata extraction (DOI, arXiv, BibTeX).
    - `omniscope-ai`: (In development) AI-powered search, summarization, and tagging.
    - `omniscope-server`: (Planned) Sync server.
    - `omniscope-ffi`: (Planned) Native bindings for document processing.
- **Storage (Dual-Write):**
    - **JSON Cards:** Individual `{id}.json` files in `.libr/cards/` are the source of truth for each book.
    - **SQLite:** A local database (`omniscope.db`) with FTS5 is used for fast searching and filtering.
- **Vim Engine:** A custom implementation of Vim grammar (`[count][operator][motion/text-object]`) handles all TUI interactions.
- **Search:** Supports fuzzy search (via `nucleo`) and a custom Search DSL (e.g., `@author:knuth #algorithms`).

## Building and Running

### Prerequisites
- Rust toolchain (Edition 2024)
- SQLite (usually bundled via `rusqlite`)

### Key Commands
- **Build:** `cargo build --release`
- **Run TUI:** `cargo run --release`
- **Run Tests:** `cargo test`
- **Crate-specific tests:** `cargo test -p omniscope-core`
- **Linting:** `cargo clippy`
- **Formatting:** `cargo fmt`

## Development Conventions

### Code Style & Structure
- **Error Handling:** Use `thiserror` for library errors and `anyhow` for application-level errors.
- **Async:** Use `tokio` for all asynchronous operations.
- **Models:** Central models are located in `omniscope-core/src/models/`. The `BookCard` is the primary data structure.
- **Persistence:** Use the Repository pattern (`omniscope-core/src/storage/repositories/`).
- **TUI Components:** Follow the pattern in `omniscope-tui/src/ui/`. Components should be stateless where possible, with state managed in `App`.

### Performance SLAs
- **TUI Cold Start:** < 100ms
- **Fuzzy Search:** < 30ms (up to 10k books)
- **TUI Frame Render:** < 16ms

### Documentation
- **AI Plans:** Refer to the `AI plans/` directory for detailed design specifications and implementation roadmaps.
- **README:** The root `README.md` contains the high-level status and feature list.

## Important Files

| Purpose | Path |
|---|---|
| Main App State | `crates/omniscope-tui/src/app/mod.rs` |
| BookCard Model | `crates/omniscope-core/src/models/book/mod.rs` |
| Database Schema | `crates/omniscope-core/src/storage/database/schema.rs` |
| Vim Key Handling | `crates/omniscope-tui/src/keys/` |
| Search Logic | `crates/omniscope-core/src/search.rs` |
| CLI Commands | `crates/omniscope-cli/src/main.rs` |
