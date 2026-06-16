# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repository State

Greenfield Rust project, scaffolded but not implemented. `src/` has stub modules with `todo!()` bodies that compile via `cargo check` but panic if run. The CLI shell (clap subcommands) is wired.

**Source of truth for scope: `ROADMAP.md`.** It defines v0.1 (the narrowed scope we're actually building) and tiers everything else. The original `Universal Context Pipeline Specification.md` is preserved for reference but its OS injector, HTTP proxy, model router, and Tauri tray are deferred or cut — do **not** implement those for v0.1. When ROADMAP.md and the spec conflict, ROADMAP.md wins.

## Build & Test Commands

```bash
cargo check                # fastest signal — stubs compile
cargo build
cargo run -- --help        # CLI surface
cargo run -- index <path>  # subcommands (currently log TODO and exit)
cargo test
cargo test <name>          # single test by name substring
cargo test -- --nocapture  # show println! output
```

## v0.1 Architecture

Single crate (`ucp-local` on crates.io), headless. Binary on PATH is `ucp-local`, library identifier is `ucp_local`. No workspace, no Tauri. Module layout in `src/`:

| Module | Role |
|---|---|
| `main.rs` | clap CLI: `index`, `serve`, `watch`, `status` |
| `config.rs` | TOML config at `~/.config/ucp/config.toml`, watched folders + Ollama settings + chunking params |
| `ingestion.rs` | `MaskingEngine` (PII regex), `ProseChunker` (sentence-bounded via tiktoken), `CodeChunker` (tree-sitter per language), `conversation_export` ingester |
| `storage.rs` | `VectorStore` — rusqlite + `sqlite-vec` for ANN + FTS5 for BM25, hybrid retrieval via reciprocal-rank fusion |
| `embeddings.rs` | `OllamaClient` (POST `/api/embeddings`) + `EmbeddingCache` (SHA-256 content hash → skip re-embedding) |
| `mcp.rs` | `rmcp` stdio server exposing one tool: `search_local_context(query, limit, folder_filter?)` |
| `watcher.rs` | `notify`-based debounced folder watcher, incremental re-index on change |

### Data flow

```
fs/conversation-export → MaskingEngine → ProseChunker | CodeChunker
                                              ↓
                                    EmbeddingCache.hash(text)
                                              ↓
                              hit? skip embed : OllamaClient.embed
                                              ↓
                                      VectorStore.insert
                                              ↓
                              vec0 (ANN)  +  FTS5 (BM25)

MCP client → search_local_context → embed query → VectorStore.hybrid_search (RRF)
                                              ↓
                            chunks with citations: file_path, start_line, end_line, mtime
```

### Build order (mirrors ROADMAP.md weekly plan)

1. **Week 1:** `ingestion.rs` (`MaskingEngine`, `ProseChunker`, `CodeChunker` with tree-sitter grammars) + `EmbeddingCache::hash`. Add deps: `regex`, `tiktoken-rs`, `tree-sitter`, `tree-sitter-rust/python/typescript/go`, `sha2`, `walkdir`, `pdf-extract`.
2. **Week 2:** `storage.rs` (rusqlite + sqlite-vec + FTS5 + RRF) → `embeddings.rs::OllamaClient` → `mcp.rs` (rmcp stdio). Add deps: `rusqlite` (bundled), `sqlite-vec`, `reqwest`, `rmcp`.
3. **Week 3:** `watcher.rs` (notify) + `conversation_export::ingest` + `ucp-local ask` if scope allows. Add dep: `notify`.

## Known issues in the original spec (preserved for reference)

The `Universal Context Pipeline Specification.md` is a design document, not validated code. When mining it for snippets:

- Chunker uses `Vec![]` (capital V) — should be `vec![]`.
- URLs in `models/router.rs` are wrapped as markdown links — pass bare URL strings.
- `rusqlite::Connection::load_extension` signature varies by version — prefer the `sqlite-vec` crate's bundled extension over manual `load_extension`.
- `rmcp` macro surface (`#[tool_router(server_handler)]`, `#[tool(description = ...)]`) must be checked against the actual published crate before relying on the spec's attribute syntax.
- Spec's `phone` regex eats ordinary numeric strings — **omit it** (decision recorded in ROADMAP.md).
