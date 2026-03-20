<p align="center">
  <img src="src/assets/archivum-logo.png" width="160" height="160" style="border-radius: 32px;" alt="Archivum">
</p>

<h1 align="center">Archivum</h1>

<p align="center">
  <strong>Your personal library, infinitely organized.</strong><br>
  A native macOS app for acquiring, archiving, and reading digital text content.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/platform-macOS-333?logo=apple" alt="macOS">
  <img src="https://img.shields.io/badge/built_with-Rust-dea584?logo=rust" alt="Rust">
  <img src="https://img.shields.io/badge/shell-Tauri_v2-24c8db?logo=tauri" alt="Tauri">
  <img src="https://img.shields.io/badge/license-Apache_2.0-blue" alt="License">
</p>

---

## What It Does

Archivum is a document archive with AI-powered understanding. Drop in your PDFs, EPUBs, and other documents — it extracts text, enriches metadata, builds a searchable index, and lets you research across your entire collection with an agentic AI assistant.

**Key capabilities:**

- **AI-Powered Import** — Claude or Ollama automatically extract titles, authors, descriptions, tags, and full text from any document
- **Agentic RAG Chat** — Multi-step research assistant that searches, reads summaries, refines queries, and synthesizes answers with citations
- **Smart Search** — Full-text keyword search (FTS5) + semantic vector search across all documents
- **Hierarchical Summaries** — Section, document, and library-level AI summaries that give the agent a navigational map
- **Local-First** — SQLite database, local embeddings (BGE-small), optional local LLMs via Ollama. Documents never leave your Mac
- **Hardware-Aware Model Picker** — Recommends Ollama models ranked by your Mac's actual RAM/GPU/CPU via llmfit-core
- **Format Support** — PDF, EPUB, MOBI, DjVu, HTML, Markdown, CBZ/CBR, audio/video (Whisper transcription)
- **Collection Import** — One-click import of curated document bundles from JSON manifests
- **ZIM Import** — Import articles from Kiwix ZIM files (offline Wikipedia, medical references)
- **Reranked Search** — Combines semantic similarity with keyword overlap scoring and source diversity

## How It Works

```
Import  ──>  AI Enrichment  ──>  Research
```

1. **Import** — Drop files or paste URLs. Archivum deduplicates by SHA-256 hash, stores originals in content-addressable storage, and extracts text locally
2. **Enrich** — AI extracts metadata, generates section summaries, creates embeddings, and indexes every passage
3. **Research** — Ask questions. The agentic assistant iterates: search → evaluate → refine → synthesize, using tools like `search_content`, `get_section_summaries`, `keyword_search`, and `get_related_documents`

## Stack

| Layer | Technology |
|-------|-----------|
| App shell | Tauri v2 |
| Backend | Rust (async, tokio) |
| Frontend | React + Radix UI + Panda CSS + Framer Motion |
| Database | SQLite via rusqlite |
| Search | FTS5 + sqlite-vec (384-dim embeddings) |
| Embeddings | fastembed (BGE-small-en-v1.5) |
| AI | Claude API + Ollama (local LLMs) |
| Transcription | Whisper (via whisper-rs) |
| Model selection | llmfit-core |

## Getting Started

### Prerequisites

- macOS 11.0+
- [Node.js](https://nodejs.org/) 18+
- [Rust](https://rustup.rs/) toolchain
- (Optional) [Ollama](https://ollama.com/) for local LLMs
- (Optional) `ANTHROPIC_API_KEY` env var for Claude

### Development

```bash
# Install dependencies
npm install

# Start dev mode (Vite + Tauri)
npm run tauri dev

# Production build
MACOSX_DEPLOYMENT_TARGET=11.0 npm run tauri build
```

### Other Commands

```bash
npm run build                                        # Build frontend only
npx panda codegen                                    # Regenerate Panda CSS
cargo check --manifest-path src-tauri/Cargo.toml     # Type-check Rust
```

## Architecture

```
src/                        React frontend
  components/               ChatPanel, etc.
  pages/                    DocumentView, Settings, Library
  lib/api.ts                Tauri IPC bindings

src-tauri/src/              Rust backend
  lib.rs                    App setup, state management
  db.rs                     SQLite schema, queries, models
  storage.rs                Content-addressable file storage
  claude.rs                 Claude API client
  ollama.rs                 Ollama client (chat, metadata, summaries)
  pipeline.rs               Import pipeline (hash > dedup > store > enrich > index)
  agent.rs                  Agentic RAG loop (tool-use with Claude/Ollama)
  rerank.rs                 Search result reranking (keyword + semantic + diversity)
  collections.rs            Curated collection manifest system
  embeddings.rs             BGE-small embedding engine
  commands.rs               Tauri command handlers (IPC bridge)
```

## License

[Apache 2.0](LICENSE)
