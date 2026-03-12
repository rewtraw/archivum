# Archivum

A native macOS desktop app for acquiring, archiving, and reading digital text content (ebooks, PDFs, etc.).

## Stack

- **App shell**: Tauri v2
- **Backend**: Rust (file I/O, hashing, SQLite, Claude API calls)
- **Frontend**: React + Radix UI + Panda CSS + Framer Motion
- **Database**: SQLite via rusqlite
- **AI**: Claude API (primary extraction engine)
- **Search**: SQLite FTS5

## Commands

```bash
npm run tauri dev     # Start dev mode (Vite + Tauri)
npm run build         # Build frontend
npx panda codegen     # Regenerate Panda CSS styled-system
cargo check --manifest-path src-tauri/Cargo.toml  # Type-check Rust
```

## Architecture

- `src/` — React frontend (pages, components, lib)
- `src-tauri/src/` — Rust backend
  - `lib.rs` — App setup, state management, plugin registration
  - `db.rs` — SQLite database (schema, queries, models)
  - `storage.rs` — Content-addressable file storage
  - `claude.rs` — Claude API client for document extraction
  - `pipeline.rs` — Import pipeline (hash → dedup → store → Claude → index)
  - `commands.rs` — Tauri command handlers (IPC bridge)
- `styled-system/` — Generated Panda CSS output (gitignored)
- `panda.config.ts` — Design tokens and global styles

## Key Patterns

- Claude is the PRIMARY extraction engine — sends documents to Claude API, gets back structured JSON with metadata + markdown
- Content-addressable storage: originals stored by SHA-256 hash
- All content normalized to markdown for search and reading
- FTS5 for full-text search across metadata and content
- Async import pipeline: imports don't block the UI

## Environment

- `ANTHROPIC_API_KEY` must be set for Claude extraction to work
