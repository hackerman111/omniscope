<div align="center">

```
 ██████╗ ███╗   ███╗███╗   ██╗██╗███████╗ ██████╗ ██████╗ ██████╗ ███████╗
██╔═══██╗████╗ ████║████╗  ██║██║██╔════╝██╔════╝██╔═══██╗██╔══██╗██╔════╝
██║   ██║██╔████╔██║██╔██╗ ██║██║███████╗██║     ██║   ██║██████╔╝█████╗  
██║   ██║██║╚██╔╝██║██║╚██╗██║██║╚════██║██║     ██║   ██║██╔═══╝ ██╔══╝  
╚██████╔╝██║ ╚═╝ ██║██║ ╚████║██║███████║╚██████╗╚██████╔╝██║     ███████╗
 ╚═════╝ ╚═╝     ╚═╝╚═╝  ╚═══╝╚═╝╚══════╝ ╚═════╝ ╚═════╝ ╚═╝     ╚══════╝
```

**A terminal-native library manager for books, papers, and research.**  
*Yazi for your bookshelf. Zotero without the Electron. Vim motions throughout.*

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Built with Rust](https://img.shields.io/badge/Built%20with-Rust-orange.svg)](https://www.rust-lang.org/)
[![Status: In Development](https://img.shields.io/badge/Status-In%20Development-yellow.svg)]()

</div>

---

> **⚠️ Omniscope is in active design and early development.**  
> No releases yet. Stars and watches are appreciated — they help gauge interest.

---

## What is Omniscope?

Omniscope is a TUI book and paper manager built around three ideas:

**Every book is a JSON card + a file.** The file is optional. The metadata lives locally, versioned, readable without the app. No proprietary databases, no vendor lock-in.

**The terminal is enough.** Three-panel layout. Vim grammar (`[count][verb][noun]`). Seven modes. Telescope-style search with a full DSL. Everything a GUI library manager does, but from the terminal, at the speed of keystrokes.

**The AI is a librarian, not a chatbot.** Omniscope's AI layer — named the Librarian — knows your entire collection through a compact Library Map. It extracts references from papers, suggests reading order, finds knowledge gaps, and acts through a structured Action Protocol that logs every change and supports undo. It works with Claude, GPT, Gemini, or a local Ollama model.

---

## Planned Features

```
Core
├── Three-panel TUI (libraries · books · preview)
├── Full vim grammar: operators, text-objects, marks, registers, macros
├── Telescope-style fuzzy search with DSL
│       @author:knuth #algorithms y:2000-2010 r:>=4 f:pdf
├── Frecency ranking (zoxide-style)
├── Open files in external viewers (zathura, foliate, $EDITOR, ...)
└── Full CLI with --json output for scripting and AI agents

Academic / Science
├── arXiv — add by ID, search, auto-update versions
├── DOI, ISBN, PMID, ISSN, OpenAlex, Semantic Scholar — all identifiers
├── Reference extraction from PDFs
├── Citation graph visualization
├── CrossRef, Semantic Scholar, OpenAlex, Unpaywall — metadata enrichment
├── Anna's Archive + Sci-Hub — PDF retrieval (scraping, no paid API)
└── BibTeX, BibLaTeX, RIS, CSL, Zotero, Calibre — import/export

Librarian (AI)
├── Three-layer memory: Library Map → Book Cards → RAG chunks
├── Action Protocol — AI returns structured JSON actions, not just text
├── Audit, deduplication, auto-tagging, reading plans
├── Proactive insights: new arXiv versions, stuck reading, missing citations
├── MCP server — works natively with Claude Desktop, Claude Code, etc.
├── Full CLI control for external AI agents
└── Token budget management + local model routing via Ollama

Server
└── HTTP/WebSocket sync server — transfer books to tablet / KOReader
```

---

## Design Principles

| Principle | Meaning |
|---|---|
| **Card + file** | Metadata lives in JSON. File is optional. |
| **Open, don't read** | Omniscope manages. External apps open. |
| **CLI = full interface** | Every TUI action is available as `omniscope ... --json` |
| **Performance is not optional** | Cold start < 100ms. Search < 50ms. Always. |
| **AI acts, doesn't lecture** | Every AI response includes executable actions, not just text. |
| **Every AI change is undoable** | Action log with snapshots. `:ai undo` works. |

---

## Tech Stack

- **Rust** — core, TUI (`ratatui`), CLI, AI, server
- **C/C++ via FFI** — `libmagic`, `poppler`, `libepub` for file metadata
- **SQLite + tantivy** — storage and full-text search
- **usearch** — HNSW vector index for semantic search
- **axum** — sync server
- **Ollama / Anthropic / OpenAI / Gemini** — AI providers, pluggable

---

## Roadmap

```
v0.1  Core TUI + vim motions + CRUD
v0.2  Search + DSL + frecency
v0.3  Scientific module (arXiv, DOI, references, Anna's Archive)
v0.4  Omniscope AI + Action Protocol + MCP
v0.5  Zotero / Calibre import-export
v0.6  Sync server
v1.0  Polish + plugins + documentation
```

---

## Contributing

Omniscope doesn't have a codebase to contribute to yet.

If you're interested in the project:
- **Watch** the repository for updates
- **Open an issue** if you have ideas, use cases, or questions
- **Star** if you'd use this — it's a meaningful signal

The design documents (architecture, vim motions spec, scientific module, AI system) are available in [`/docs`](./docs) if you want to read the thinking behind the project.

---

## License

MIT — see [LICENSE](LICENSE).

---

<div align="center">
<sub>Built for people who live in the terminal and have too many PDFs.</sub>
</div>