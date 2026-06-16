# UCP — Universal Context Pipeline

A local-first MCP server that grounds LLMs in your own files.

UCP indexes folders on your machine — notes, code, conversation exports — and exposes them to any MCP-compatible client (Claude Desktop, Cursor, etc.) as a single tool: `search_local_context`. Hybrid retrieval (BM25 + vector), tree-sitter-aware code chunking, full citations, content-hash embedding cache. Single binary. No telemetry. No cloud.

## Status

v0.1, headless. Track scope in [ROADMAP.md](ROADMAP.md). What ships:

- Hybrid search: SQLite FTS5 (BM25) ⨉ `sqlite-vec` (ANN) merged via reciprocal-rank fusion.
- Tree-sitter chunking for Rust, Python, TypeScript/JavaScript. Heading-aware Markdown. Sentence-bounded prose fallback.
- Conversation memory: ingest your Claude `conversations.json` export and search across past chats.
- PII masking on by default — email, OpenAI `sk-`, AWS keys, GitHub PATs, JWT.
- Content-hash embedding cache: re-indexing unchanged content makes zero Ollama calls.
- Filesystem watcher: edit a file, the index updates in ~500ms.

What's not in v0.1:

- Desktop UI / tray (deferred — was in original spec, now in ROADMAP tier 2+).
- OS hotkey injector and HTTP proxy interceptor (cut from the original spec).
- OpenAI / Anthropic embedding providers (Ollama only for now).
- Cursor and ChatGPT export formats (Claude only; others later).

## Prerequisites

- Rust (stable, edition 2024).
- [Ollama](https://ollama.ai) running locally with an embedding model:
  ```bash
  ollama pull nomic-embed-text
  ```

## Install (from source)

```bash
git clone <repo-url> ucp
cd ucp
cargo build --release
# Binary at target/release/ucp
```

Optional: `cargo install --path .` to put it on your PATH.

## Usage

```bash
# Index a folder
ucp index ~/Documents/notes

# Watch a folder and re-index on changes (initial pass runs first)
ucp watch ~/code/my-project

# Ingest a Claude conversations.json export
ucp ingest-conversations ~/Downloads/claude-export/conversations.json

# Show config + index status
ucp status

# Run the MCP server over stdio (this is what MCP clients launch)
ucp serve
```

### Wire up Claude Desktop

Add to your Claude Desktop config (`~/Library/Application Support/Claude/claude_desktop_config.json` on macOS):

```json
{
  "mcpServers": {
    "ucp": {
      "command": "/full/path/to/ucp",
      "args": ["serve"]
    }
  }
}
```

Restart Claude Desktop. The `search_local_context` tool will be available — ask something grounded in your indexed files and it'll cite them inline.

## Config

`~/.config/ucp/config.toml` (or the platform equivalent — `ucp status` prints the resolved path). All fields optional; defaults shown:

```toml
[ollama]
host = "http://localhost:11434"
embedding_model = "nomic-embed-text"

[chunking]
max_tokens = 512
overlap_sentences = 1
```

## What gets indexed

By extension: `md`, `markdown`, `txt`, `rs`, `py`, `ts`, `tsx`, `js`, `jsx`, `mjs`, `go`, `pdf`.

> **PDFs:** text is extracted via `pdf-extract` and chunked as prose. Works well for digitally generated PDFs (papers, docs, exported notes). Falls down on scanned image-only PDFs — those need OCR (v0.2+). Citation line numbers reference the extracted plaintext, not PDF page numbers; page-aware citations are on the v0.2 list.

Skipped directories: `.git`, `.idea`, `.vscode`, `target`, `node_modules`, `__pycache__`, `.venv`, `venv`, `dist`, `build`, `.next`, `.nuxt`, `coverage`, `.pytest_cache`, `.mypy_cache`. Dotfiles are skipped.

## Architecture

| Module | Role |
|---|---|
| `ingestion` | Masking + per-format chunkers (prose / markdown / code via tree-sitter) + dispatcher |
| `storage` | `rusqlite` + `sqlite-vec` + FTS5; hybrid search via RRF |
| `embeddings` | `OllamaClient` + content-hash cache via `EmbeddingCache::hash` |
| `indexer` | Walk + read + chunk + embed + insert; single-file and bulk-chunk paths |
| `watcher` | `notify`-based debounced re-index |
| `mcp` | JSON-RPC 2.0 stdio server, one tool: `search_local_context` |

See [CLAUDE.md](CLAUDE.md) for the developer-facing architecture summary, and [Universal Context Pipeline Specification.md](Universal%20Context%20Pipeline%20Specification.md) for the original (now narrower in scope) design doc.

## Development

```bash
cargo test                    # all 75+ tests
cargo test --lib ingestion    # one module
cargo run -- index <path>     # iterate against the dev build
RUST_LOG=ucp=info cargo run -- watch <path>   # verbose
```

## License

Dual-licensed under MIT or Apache-2.0.
