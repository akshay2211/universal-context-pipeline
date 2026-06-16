# UCP Roadmap

Living document. Source of truth for what v0.1 ships and what waits. Supersedes the original `Universal Context Pipeline Specification.md` where they conflict.

## v0.1 — "Local folder → MCP context server"

**Pitch:** Local-first MCP server with hybrid search, tree-sitter code chunking, full citations, and your old AI conversations as searchable memory. Single binary, zero telemetry, zero cloud.

### Shipping in v0.1

| Area | Detail |
|---|---|
| Crate shape | Single crate `ucp`, headless binary + lib. No workspace, no Tauri. |
| Ingestion | `.md`, `.txt`, `.pdf` (via `pdf-extract`, flat text — no page-aware citations yet), source files. No OCR. |
| Chunking — prose | Sentence-bounded via `tiktoken-rs` cl100k, configurable max_tokens + overlap. |
| Chunking — code | Tree-sitter: chunk by function/class/impl-block for `.rs`/`.py`/`.ts`/`.go`. |
| PII masking | Email + API-key regex. **No phone regex** (false positives). `--no-mask` flag. |
| Embeddings | Ollama only (`nomic-embed-text` default). Cloud providers deferred. |
| Storage | `rusqlite` + `sqlite-vec` bundled crate. SQLite FTS5 alongside `vec0`. |
| Retrieval | Hybrid: BM25 (FTS5) + vector (vec0) merged via reciprocal-rank fusion. |
| Citations | Every chunk returns `file_path`, `start_line`, `end_line`, `last_modified`. |
| Embedding cache | SHA-256 of normalized chunk text → embedding. Never re-embed unchanged content. |
| Watcher | `notify` crate, debounced, incremental re-index by file mtime + content hash. Handles deletes. |
| MCP server | `rmcp` over stdio. One tool: `search_local_context(query, limit, folder_filter?)`. |
| CLI | `ucp index <path>`, `ucp serve`, `ucp watch <path>`, `ucp status`. |
| Config | TOML at `~/.config/ucp/config.toml`. Watched folders, Ollama host, embedding model, chunk params. |
| Bonus ingester | Claude / Cursor / ChatGPT export JSON → searchable memory. |

### Success criteria
1. `ucp index ~/Documents/notes` indexes 1000 markdown files under 60s on M-series Mac.
2. Claude Desktop pointed at `ucp serve` can call `search_local_context("foo")` and get grounded chunks back with citations.
3. Editing a file triggers re-index of just that file within ~2s.
4. End-to-end install: `cargo install ucp` + `ollama pull nomic-embed-text` + Claude Desktop config edit.

### Timeline
- **Week 1** — ingestion + sentence + tree-sitter chunking + masking + content-hash cache.
- **Week 2** — FTS5 + vec0 + RRF hybrid search + MCP server + Ollama embeddings + citations.
- **Week 3** — `notify` watcher + conversation-export ingester + `ucp ask` + README + release artifacts.

---

## Feature tier reference

The full evaluation that produced the v0.1 scope. Use when prioritizing future work.

### Tier 1 — In v0.1 (real differentiators)

- **Hybrid search (BM25 + vector).** Pure vector search is mediocre for code and exact terms. FTS5 alongside `vec0`, merged via RRF. ~1 day, big recall win.
- **Code-aware chunking via tree-sitter.** Chunk by function/class for source files. Single biggest quality lever for code RAG. ~2 days.
- **Citations with line ranges.** Every chunk includes file path + line range + mtime. LLM can cite, user can click. Almost free.
- **Content-hash embedding cache.** SHA-256 of normalized chunk text → embedding. Never re-embed unchanged content. Major iteration perf win.
- **Conversation export ingestion.** Eat Claude / Cursor / ChatGPT export JSON. Past conversations become searchable. Sleeper feature — no competitor ships this polished.

### Tier 2 — v0.2 candidates

- **`ucp ask <question>` CLI.** Local Ollama chat model + search tool. Terminal Q&A without needing any MCP client. Huge first-run experience win.
- **Memory tool (`remember_this`).** Second MCP tool the LLM can call to write notes back into the index. Turns UCP from read-only RAG into bidirectional long-term memory.
- **Per-folder profiles.** Config-driven: `~/code/*` uses tree-sitter, `~/notes/*` uses prose chunking, `~/papers/*` uses section-aware splitting.

### Tier 3 — Later

- Local cross-encoder reranker (`bge-reranker-base` via Ollama or candle). Quality bump, adds a model dependency.
- Git-aware indexing (respect `.gitignore`, attach commit SHA to chunks).
- Time-decay scoring (boost recent files).
- Privacy tags (secret/internal/public per folder, MCP tool refuses to surface secret content unless explicitly asked).
- Multiple embedding models in one DB (different models per content type).
- **Page-aware PDF citations.** Currently PDFs are extracted as flat text with line numbers referring to the plaintext. Better: per-page chunks with `[file.pdf:page 3]` citations. Needs lower-level pdf-extract usage (PlainTextOutput) or switching to `lopdf` directly.
- **Image ingestion.** Three candidate paths (ROADMAP entry):
  - OCR on screenshots/scans via `tesseract` (adds a system dep).
  - Local vision-LM captions via `ollama pull llava` — same Ollama runtime, no new system deps.
  - CLIP-style multimodal embeddings (best UX, needs separate vector pipeline).

### Deferred from original spec (may never ship)

- **OS hotkey injector** (`rdev` + clipboard paste). Months of platform pain — macOS Accessibility permissions, Wayland, Windows UIAccess. Cursor's `@`-mention already covers this in-app. Revisit only if users ask.
- **HTTP loopback proxy on :80 intercepting model APIs.** Port 80 needs root; real provider calls are HTTPS so MITM cert install is required; duplicates MCP cleanly. Likely cut entirely.
- **Tauri tray UI.** Add only after the headless core has users.
- **Multi-provider model router (OpenAI/Anthropic).** Chat lives in the MCP client; v0.1 only needs *embeddings*.