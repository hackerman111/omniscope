# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Omniscope is a terminal-based library manager for books and scientific papers, built in Rust with a Vim-first philosophy. It's a lightweight Zotero alternative with a TUI, CLI, and planned AI integration.

Core concept: **Book = JSON card + optional file**, stored in a `.libr/` directory alongside the actual files.

## Build & Run

```bash
# Build
cargo build --release

# Run TUI
cargo run --release

# Run tests
cargo test

# Run tests for a specific crate
cargo test -p omniscope-core

# Run a single test
cargo test -p omniscope-core test_name
```

The binary is `omniscope`. No separate install step needed for development.

## Workspace Structure

7 crates with clear separation of concerns:

- `omniscope-core/` — business logic, models, storage (SQLite + JSON cards), search, config
- `omniscope-tui/` — ratatui TUI, Vim engine, key handling, UI panels
- `omniscope-cli/` — binary entry point, clap CLI commands
- `omniscope-ai/` — AI integration (skeleton)
- `omniscope-science/` — DOI/arXiv identifiers (skeleton)
- `omniscope-server/` — HTTP/WebSocket server (skeleton)
- `omniscope-ffi/` — FFI bindings for libmagic, poppler, libepub (skeleton)

## Key Architecture

### Data Flow
CLI/TUI → `App` state (`omniscope-tui/src/app/mod.rs`) → Repositories → SQLite DB + JSON cards

### Storage (dual-write)
Every book is stored in two places simultaneously:
1. `{library}/.libr/cards/{id}.json` — full `BookCard` struct as JSON (source of truth)
2. `{library}/.libr/db/omniscope.db` — SQLite with FTS5 for search

Atomic writes: write to `.tmp` then rename. Repositories use a trait: `Repository<Entity, Id>`.

### Vim Engine
Located in `omniscope-tui/src/keys/`. Implements full Vim grammar: `[count][operator][motion/text-object]`.

7 modes: NORMAL, INSERT, VISUAL, VISUAL-LINE, VISUAL-BLOCK, COMMAND, SEARCH, PENDING.

Key files:
- `omniscope-tui/src/keys/core/` — mode state machine
- `omniscope-tui/src/keys/modes/` — per-mode handlers
- `omniscope-tui/src/keys/ext/` — extended features (EasyMotion, f/F/t/T, macros)

### Search
- Fuzzy: `nucleo-matcher` via `omniscope-core/src/search.rs`
- DSL: `omniscope-core/src/search_dsl.rs` — supports `@author:`, `#tag`, `y:2020`, `r:>=4`, `s:unread`, etc.
- Frecency: `omniscope-core/src/frecency.rs` — zoxide-inspired scoring

### Undo/Redo
`omniscope-core/src/undo.rs` — `UndoAction` enum with `UpsertCards`/`DeleteCards`. Infinite stack with timestamps. TUI exposes `u` / `Ctrl+r` / `:earlier` / `:later`.

## Important Files

| Purpose | Path |
|---|---|
| App state | `omniscope-tui/src/app/mod.rs` |
| BookCard model | `omniscope-core/src/models/book.rs` |
| Database schema | `omniscope-core/src/storage/database.rs` |
| JSON card storage | `omniscope-core/src/storage/json_cards.rs` |
| Repositories | `omniscope-core/src/storage/repositories.rs` |
| Search DSL | `omniscope-core/src/search_dsl.rs` |
| Config | `omniscope-core/src/config.rs` |
| CLI entry point | `omniscope-cli/src/main.rs` |

## Configuration

- Global config: `~/.config/omniscope/config.toml`
- Per-library manifest: `{library}/.libr/library.toml`
- Library ID format: ULID

## Performance SLAs

These are non-negotiable constraints — don't introduce regressions:
- Cold start TUI: < 100ms
- Fuzzy search (< 10K books): < 30ms
- TUI frame render: < 16ms
- CLI command: < 150ms

Release profile uses `lto = "fat"`, `codegen-units = 1`, `opt-level = 3`.

## CLI Flags

- `--json` — machine-readable JSON output
- `OMNISCOPE_JSON=1` — env var equivalent
- `OMNISCOPE_TIMING=1` — print timing info
- `OMNISCOPE_LIBRARY_PATH` — override library path

## Development Status

Pre-alpha (v0.1.0). Phases 0–2 are ~85–95% complete (core TUI, Vim engine, search). Phases 3–7 (AI, server, scientific module, macros, CI) are skeletal or unstarted. See `project_status.md` and `OMNISCOPE_MASTER_PLAN.md` for roadmap details.

## Design & Implementation Plans (AI plans/ directory)

The `AI plans/` directory is the authoritative source for design decisions, architecture, and implementation roadmaps. Consult these before making significant changes:

- **OMNISCOPE_MASTER_PLAN.md** — overall roadmap, phases, milestones
- **Omniscope_VIM_MOTIONS.md** — complete Vim spec (modes, operators, motions, text objects)
- **VIM_MOTIONS_IMPL_PLAN.md** — Vim implementation guide
- **OMNISCOPE_UI_DESIGN_PLAN.md** — TUI layout, panels, overlays, rendering strategy
- **OMNISCOPE_STORAGE.md** — dual-write architecture (JSON + SQLite), schema design
- **Omniscope_AI_SYSTEM.md** — AI integration architecture, LLM providers, MCP
- **Omniscope_SCIENCE.md** — scientific identifiers (DOI, arXiv, CrossRef, Semantic Scholar)
- **OMNISCOPE_SCIENCE_IMPL_PLAN.md** — scientific module implementation
- **OMNISCOPE_FOLDER_SYSTEM.md** — folder hierarchy and organization

When implementing features or fixing bugs, check the relevant plan document first to ensure alignment with the design.
