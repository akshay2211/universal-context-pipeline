# Changelog

All notable changes to `ucp-local` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] — 2026-06-17

Initial public release. First version published to crates.io as `ucp-local`.

### Added

#### CLI

- `ucp-local index <path>...` — index one or more folders into the local store.
- `ucp-local watch <path>` — continuously re-index a folder on file changes (debounced).
- `ucp-local serve` — run the MCP server over stdio for use with Claude Desktop, Cursor, LM Studio, and other MCP-compatible clients.
- `ucp-local search <query>` — terminal-side search of the index without an LLM (useful for debugging).
- `ucp-local ask <question>` — end-to-end Q&A: search + local chat model + cited answer.
- `ucp-local ingest-conversations <path>` — import a Claude `conversations.json` export as searchable memory.
- `ucp-local status` — print resolved config path, watched folders, and index stats.
- `ucp-local clear [path]` — soft clear (keeps the embedding cache) or `--hard --yes` for a full wipe.

#### Ingestion

- File-type support: `.md`, `.markdown`, `.txt`, `.rs`, `.py`, `.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`, `.go`, `.pdf`.
- Tree-sitter code chunking for Rust, Python, TypeScript, and JavaScript (chunked by function / class / impl block).
- Heading-aware Markdown chunking.
- Sentence-bounded prose fallback via `tiktoken-rs` (cl100k), with configurable `max_tokens` and overlap.
- PDF text extraction via `pdf-extract`, with optional fallback to Poppler's `pdftotext` when available on PATH (better recovery for PDFs missing a ToUnicode CMap).
- Conversation-export ingester: Claude `conversations.json` → searchable past-chat memory.
- Skipped directories: `.git`, `.idea`, `.vscode`, `target`, `node_modules`, `__pycache__`, `.venv`, `venv`, `dist`, `build`, `.next`, `.nuxt`, `coverage`, `.pytest_cache`, `.mypy_cache`. Dotfiles skipped.

#### Storage & retrieval

- `rusqlite` + `sqlite-vec` (bundled) for ANN vector search.
- SQLite FTS5 (BM25) full-text index alongside the vector table.
- Hybrid retrieval merging BM25 + vector via reciprocal-rank fusion.
- Every returned chunk includes `file_path`, `start_line`, `end_line`, and `last_modified` for citations.
- Optional `folder_filter` scope on every query.

#### Embeddings

- Ollama client (`POST /api/embeddings`), default model `nomic-embed-text`.
- Content-hash embedding cache (SHA-256 of normalized chunk text) — unchanged content is never re-embedded.

#### Privacy

- PII masking on by default: email addresses, OpenAI `sk-` keys, AWS access keys, GitHub PATs, JWTs.
- `--no-mask` flag to disable masking when indexing trusted folders.
- No telemetry. No network calls beyond the local Ollama host.

#### Watcher

- `notify`-based debounced filesystem watcher.
- Incremental re-index by file mtime + content hash. Handles deletes.
- Per-file re-index completes in ~500ms after the debounce window.

#### MCP server

- `rmcp` stdio server exposing one tool: `search_local_context(query, limit, folder_filter?)`.
- Wires up to Claude Desktop, Cursor, LM Studio, Zed, Continue.dev, Goose, and any other MCP-spec-compliant client with the same `command` + `args` config shape.

#### Configuration

- TOML config at `~/.config/ucp/config.toml` (or platform equivalent).
- Configurable Ollama host, embedding model, watched folders, chunk size, and sentence overlap.

#### Distribution

- Published to crates.io as `ucp-local`.
- Licensed under Apache-2.0.
- Demo GIFs (conversation memory, air-gap RAG, quick start) embedded in the README.
- Sample corpus under `samples/` for first-run validation.

### Known limitations in 0.1.0

These are intentional scope cuts, tracked in [ROADMAP.md](ROADMAP.md):

- PDF citations reference extracted-plaintext line numbers, not PDF page numbers. Page-aware citations are on the v0.2 list.
- Scanned image-only PDFs are not OCR'd. Vision / OCR ingestion deferred to v0.2+.
- Only Claude conversation exports are supported. Cursor and ChatGPT export parsers are deferred.
- Only Ollama is supported for embeddings. OpenAI / Anthropic / Cohere embedding providers deferred.
- No desktop UI / tray. Headless binary only.
- No OS-level hotkey injector and no HTTP proxy interceptor (both cut from the original spec).
- No git-aware indexing (`.gitignore` respect, commit SHA on chunks) — v0.3+ candidate.
- No cross-encoder reranker — v0.3+ candidate.

### Install

```bash
cargo install ucp-local
ollama pull nomic-embed-text
```

Wire it into any MCP client by pointing the client's MCP server config at `ucp-local serve`. See the README for Claude Desktop, Cursor, and LM Studio snippets.

[Unreleased]: https://github.com/akshay2211/universal-context-pipeline/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/akshay2211/universal-context-pipeline/releases/tag/v0.1.0