# UCP — Universal Context Pipeline

A local-first MCP server that grounds LLMs in your own files.

UCP indexes folders on your machine — notes, code, conversation exports — and exposes them to any MCP-compatible client (Claude Desktop, Cursor, etc.) as a single tool: `search_local_context`. Hybrid retrieval (BM25 + vector), tree-sitter-aware code chunking, full citations, content-hash embedding cache. Single binary. No telemetry. No cloud.

## Status

v0.1, headless. Track scope in [ROADMAP.md](ROADMAP.md). Who this is for and how it compares to existing tools: [POSITIONING.md](POSITIONING.md).

What ships:

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

UCP needs three things on your machine: Rust (to build), Ollama (to embed and optionally chat), and Poppler (for robust PDF text extraction — recommended).

### macOS

```bash
brew install ollama poppler
ollama serve &              # or use the menu-bar app
ollama pull nomic-embed-text
# Optional, for `ucp-local ask`:  ollama pull llama3.2
```

### Linux (Debian/Ubuntu)

```bash
sudo apt install poppler-utils
curl -fsSL https://ollama.com/install.sh | sh
ollama pull nomic-embed-text
# Optional, for `ucp-local ask`:  ollama pull llama3.2
```

### Linux (Fedora/RHEL)

```bash
sudo dnf install poppler-utils
curl -fsSL https://ollama.com/install.sh | sh
ollama pull nomic-embed-text
```

### Windows

```powershell
choco install poppler ollama   # or install each manually
ollama pull nomic-embed-text
```

Rust (stable, edition 2024) is needed only to build from source. If you install a pre-built UCP binary, skip the Rust install.

> **Poppler is optional but recommended.** Without it, UCP only uses the bundled `pdf-extract` for PDFs, which struggles with PDFs whose body fonts lack a ToUnicode CMap (you'll see headings extract but body text go missing). With `pdftotext` from Poppler on PATH, UCP falls back to it automatically.

## Install

> **Note on the name.** The crate is published as **`ucp-local`** on crates.io (the bare `ucp` name was taken). The binary itself is still called **`ucp`** — that's what you type on the command line — and the library is still imported as `use ucp::...`. Only the install command uses the longer name.

### From crates.io (once published)

```bash
cargo install ucp-local
# Puts the `ucp-local` binary on your PATH
```

### From source

```bash
git clone <repo-url> ucp-local
cd ucp-local
cargo build --release
# Binary at target/release/ucp-local
cargo install --path .   # optional, to put `ucp-local` on your PATH
```

## Usage

```bash
# Index one folder
ucp-local index ~/Documents/notes

# Index multiple folders into the same store
ucp-local index ~/Documents/notes ~/code/my-project ~/research

# Watch a folder and re-index on changes (initial pass runs first)
ucp-local watch ~/code/my-project

# Clear the index — soft (keeps the embedding cache so re-index is fast)
ucp-local clear

# Clear only one folder's chunks
ucp-local clear ~/Documents/notes

# Hard reset — also wipes the embedding cache, forces re-embed on next index
ucp-local clear --hard --yes

# Ingest a Claude conversations.json export
ucp-local ingest-conversations ~/Downloads/claude-export/conversations.json

# Show config + index status
ucp-local status

# Run the MCP server over stdio (this is what MCP clients launch)
ucp-local serve

# Search the index from the terminal (no LLM) — best for debugging "did indexing actually capture this?"
ucp-local search "your query here"
ucp-local search "rate limiting" --folder ~/code/my-project --limit 10

# Ask a question — runs search internally, then a local chat model answers with citations
ucp-local ask "what does the rate limiter do when a token bucket runs out?"
ucp-local ask "summarize my Q3 plan" --model qwen2.5
```

### Wire up Claude Desktop

Add to your Claude Desktop config (`~/Library/Application Support/Claude/claude_desktop_config.json` on macOS):

```json
{
  "mcpServers": {
    "ucp-local": {
      "command": "/full/path/to/ucp-local",
      "args": ["serve"]
    }
  }
}
```

Restart Claude Desktop. The `search_local_context` tool will be available — ask something grounded in your indexed files and it'll cite them inline.

## Config

`~/.config/ucp/config.toml` (or the platform equivalent — `ucp-local status` prints the resolved path). All fields optional; defaults shown:

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
cargo test                    # full test suite
cargo test --lib ingestion    # one module
cargo run -- index <path>     # iterate against the dev build
RUST_LOG=ucp_local=info cargo run -- watch <path>   # verbose
```

## License

Dual-licensed under MIT or Apache-2.0.
