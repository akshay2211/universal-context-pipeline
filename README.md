# UCP — Universal Context Pipeline

A local-first MCP server that grounds LLMs in your own files.

UCP indexes folders on your machine — notes, code, conversation exports — and exposes them to any MCP-compatible client (Claude Desktop, Cursor, LM Studio, and other local-agent runtimes) as a single tool: `search_local_context`. Hybrid retrieval (BM25 + vector), tree-sitter-aware code chunking, full citations, content-hash embedding cache. Single binary. No telemetry. No cloud.

Paired with a local model in LM Studio (or Ollama via `ucp-local ask`), the whole stack — indexing, embeddings, retrieval, and the chat model — runs fully offline. Works on a plane, in an air-gapped facility, or anywhere a cloud LLM isn't an option.

## Demos

**Conversation memory — make every past Claude chat searchable across every future session.**

![Conversation memory demo](demo/conversation-memory.gif)

**Air-gap RAG — local Ollama + local index, zero network traffic.**

![Air-gap RAG demo](demo/air-gap-rag.gif)

**Quick start — install, index, ask, in under a minute.**

![Quick start demo](demo/quick-start.gif)

## Who is this for?

| If you are… | UCP gives you… |
|---|---|
| A **Claude / Cursor / LM Studio power user** | A searchable archive of every past AI conversation, callable from any future session as the `search_local_context` tool. |
| A **software engineer** | Code + private docs + sibling repos + past Claude chats unified under one MCP tool — surfaced inside Cursor or Claude Code alongside their native indexers. |
| A **researcher, writer, or academic** | A PDF + notes corpus you can ask grounded questions against, with line-level citations, without anything leaving the machine. |
| In a **privacy-regulated workflow** (legal, medical, defense, NDA-bound IP) | A single Rust binary with zero telemetry and zero cloud. Pair with LM Studio for a fully offline, end-to-end RAG stack. |
| A **solo founder or consultant** | Per-folder client isolation via `folder_filter` — no risk of leaking client A's context into client B's session. |

Full audience analysis, competitive comparison, and the two wedges UCP is explicitly built to win on: see [POSITIONING.md](POSITIONING.md).

## Status

v0.1, headless. Track scope in [ROADMAP.md](ROADMAP.md).

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

### Wire up an MCP client

UCP speaks MCP over stdio, so any client that launches MCP servers can use it. Same `serve` command, different config file per client.

#### Claude Desktop

Add to `~/Library/Application Support/Claude/claude_desktop_config.json` on macOS (`%APPDATA%\Claude\claude_desktop_config.json` on Windows):

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

#### Cursor

Cursor reads MCP servers from `~/.cursor/mcp.json` (or per-project `.cursor/mcp.json`):

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

Reload Cursor. The chat sidebar will surface `search_local_context` as a tool — useful for grounding the agent in repos and docs Cursor's own `@codebase` indexer can't reach (private notes, conversation history, sibling repos).

#### LM Studio (fully offline)

LM Studio 0.3.17+ supports MCP. Open the chat settings, find the **MCP servers** section, and add:

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

Pair UCP with any local model you've downloaded in LM Studio (Llama, Qwen, Mistral, etc.). Now your indexing, embeddings, retrieval, and chat model all run on the same machine — no cloud, no network — and the LLM can still call `search_local_context` to ground its answers in your files.

#### Other MCP clients

Any client following the MCP spec (Zed, Continue.dev, Goose, custom Agent SDK apps, etc.) takes the same `command` + `args` shape. If your client expects a JSON-RPC stdio server, point it at `ucp-local serve` and you're done.

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

Under Apache-2.0.
