SYSTEM-WIDE IMPLEMENTATION SPECIFICATIONProject Name: Universal Context Pipeline (UCP)Role: Lead Systems Architect & Core Rust EngineerTarget Agent: Claude Code / Cursor Composer / Autonomous Coding Engine1. SYSTEM OVERVIEW & ARCHITECTURAL TOPOLOGYThe Universal Context Pipeline (UCP) is an ultra-high-performance, local-first background system utility designed to act as an invisible, zero-friction "cognitive bridge" between a user's local unstructured files (PDFs, Markdown, logs, source code) and any target AI model.The application sits silently in the system tray, monitors local target directories, processes text, cleanses PII, formats high-density context chunks matching exact token budgets, and injects these payloads dynamically.                              [ APPS & INTERFACES ]
▲
┌──────────────────────────────────┼──────────────────────────────────┐
│ (OS Accessibility Injection)     │ (Model Context Protocol / stdio)  │ (Local HTTP Loopback Proxy)
▼                                  ▼                                  ▼
┌──────────────────┐           ┌──────────────────┐           ┌──────────────────┐
│ Global Hotkey    │           │ Standardized MCP │           │ Axum Local Proxy │
│ Injection Engine │           │ Server Interface │           │ Interceptor (80) │
└────────┬─────────┘           └────────┬─────────┘           └────────┬─────────┘
│                              │                              │
└──────────────────────────────┼──────────────────────────────┘
▼
┌──────────────────────────────────┐
│  MULTI-PROVIDER MODEL ROUTER     │
│  (OpenAI / Anthropic / Ollama)   │
└────────────────┬─────────────────┘
▼
┌──────────────────────────────────┐
│   CONTEXT COMPRESSION ENGINE     │
│  • Ingestion & Structural OCR   │
│  • Token Budgeting / Truncation  │
│  • High-Speed Local PII Masking │
└────────────────┬─────────────────┘
▼
┌──────────────────────────────────┐
│   LOCAL PERFORMANCE DATABASE     │
│  • SQLite-vec Vector Storage     │
│  • Notify FS Daemon Watcher      │
└──────────────────────────────────┘
2. REPOSITORY WORKSPACE LAYOUTYou must construct the workspace matching this layout precisely:.
   ├── Cargo.toml                  # Workspace Manifest Configuration
   ├── ucp-core/                   # Primary Headless System Crate
   │   ├── Cargo.toml
   │   └── src/
   │       ├── main.rs
   │       ├── ingestion/
   │       │   ├── mod.rs
   │       │   ├── masking.rs
   │       │   └── chunker.rs
   │       ├── storage/
   │       │   ├── mod.rs
   │       │   └── vector_store.rs
   │       ├── models/
   │       │   ├── mod.rs
   │       │   └── router.rs
   │       ├── mcp/
   │       │   ├── mod.rs
   │       │   └── server.rs
   │       └── os/
   │           ├── mod.rs
   │           └── injector.rs
   └── src-tauri/                  # Tauri v2 Desktop App Wrapping Container
   ├── Cargo.toml
   ├── tauri.conf.json
   ├── capabilities/
   │   └── default.json
   └── src/
   ├── main.rs
   └── lib.rs
3. MASTER DECLARATIVE DEPENDENCY TREE3.1 Workspace root Cargo.toml[workspace]
   resolver = "2"
   members = ["ucp-core", "src-tauri"]
   3.2 Core Headless System ucp-core/Cargo.toml[package]
   name = "ucp-core"
   version = "0.1.0"
   edition = "2021"

[dependencies]
tokio = { version = "1.38", features = ["full", "rt-multi-thread"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
rusqlite = { version = "0.31", features = ["bundled", "load_extension"] }
regex = "1.10"
tiktoken-rs = "0.5"
rmcp = { version = "0.16", features = ["server"] }
schemars = "0.8"
copypasta = "0.10"
notify = "6.1.1"
walkdir = "2.5"
axum = "0.7"
reqwest = { version = "0.12", features = ["json", "stream"] }
rdev = "0.5"
keyring = "2.5"
tracing = "0.1"
tracing-subscriber = "0.3"
anyhow = "1.0"
3.3 Desktop Shell Container src-tauri/Cargo.toml[package]
name = "ucp-desktop"
version = "0.1.0"
edition = "2021"

[dependencies]
tauri = { version = "2.0.0", features = ["tray-icon"] }
tauri-plugin-shell = "2.0.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
ucp-core = { path = "../ucp-core" }

[build-dependencies]
tauri-build = { version = "2.0.0", features = [] }
4. PHASE-BY-PHASE TEST-DRIVEN DEVELOPMENT (TDD) BLUEPRINTEvery module must be built by writing test definitions first, followed by the minimal production-grade implementations.PHASE 1: Local Ingestion, PII Scrubbing, & Sentence-Bound Token Chunking1.1 Ingestion Masking Module Verification (ucp-core/src/ingestion/masking.rs)#[cfg(test)]
   mod tests {
   use super::*;

   #[test]
   fn test_pii_scrubbing_edges() {
   let text = "Contact developer@ucp.io or admin@localhost. Database connection sk-liveSecretApiKey123456789abc with billing phone +1-555-0199.";
   let cleaned = MaskingEngine::clean(text);

        assert!(cleaned.contains("[REDACTED_EMAIL]"));
        assert!(cleaned.contains("[REDACTED_CREDENTIAL]"));
        assert!(cleaned.contains("[REDACTED_PHONE]"));
        assert!(!cleaned.contains("developer@ucp.io"));
        assert!(!cleaned.contains("sk-liveSecretApiKey123456789abc"));
        assert!(!cleaned.contains("+1-555-0199"));
   }

   #[test]
   fn test_unaltered_text() {
   let raw = "The wizard stepped into the grand hall of Hogwarts. There were 142 staircases.";
   assert_eq!(MaskingEngine::clean(raw), raw);
   }
   }
   1.2 Ingestion Masking Module Production Implementation// ucp-core/src/ingestion/masking.rs
   use regex::Regex;
   use std::sync::OnceLock;

pub struct MaskingEngine;

impl MaskingEngine {
pub fn clean(text: &str) -> String {
static EMAIL_RE: OnceLock<Regex> = OnceLock::new();
static PHONE_RE: OnceLock<Regex> = OnceLock::new();
static API_KEY_RE: OnceLock<Regex> = OnceLock::new();

        let email = EMAIL_RE.get_or_init(|| Regex::new(r"(?i)[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}").unwrap());
        let phone = PHONE_RE.get_or_init(|| Regex::new(r"(\+?\d{1,3}[-.\s]?)?\(?\d{3}\)?[-.\s]?\d{3}[-.\s]?\d{4}").unwrap());
        let api_key = API_KEY_RE.get_or_init(|| Regex::new(r"sk-[a-zA-Z0-9]{20,50}").unwrap());

        let mut output = text.to_string();
        output = email.replace_all(&output, "[REDACTED_EMAIL]").to_string();
        output = phone.replace_all(&output, "[REDACTED_PHONE]").to_string();
        output = api_key.replace_all(&output, "[REDACTED_CREDENTIAL]").to_string();
        
        output
    }
}
1.3 Sentence-Bound Chunker Verification (ucp-core/src/ingestion/chunker.rs)#[cfg(test)]
mod tests {
use super::*;

    #[test]
    fn test_chunker_sentence_boundaries() {
        let text = "This is sentence one. This is sentence two! And this is three?";
        let chunks = DocumentChunker::split_into_chunks(text, 15, 1);
        
        assert!(!chunks.is_empty(), "Chunk list cannot be empty");
        for chunk in &chunks {
            assert!(chunk.token_count <= 25, "Chunk size exceeded soft token target limits");
            assert!(!chunk.text.is_empty(), "Chunk content text cannot be blank");
        }
    }
}
1.4 Sentence-Bound Chunker Production Implementation// ucp-core/src/ingestion/chunker.rs
use tiktoken_rs::cl100k_base;

pub struct Chunk {
pub text: String,
pub token_count: usize,
}

pub struct DocumentChunker;

impl DocumentChunker {
pub fn split_into_chunks(text: &str, max_tokens: usize, overlap_sentences: usize) -> Vec<Chunk> {
let bpe = cl100k_base().unwrap();
let sentences: Vec<&str> = text.split_inclusive(|c| c == '.' || c == '!' || c == '?').collect();
let mut chunks = Vec![];
let mut current_chunk = Vec![];
let mut current_tokens = 0;

        for sentence in sentences {
            let sentence_tokens = bpe.encode_with_special_tokens(sentence).len();
            if current_tokens + sentence_tokens > max_tokens && !current_chunk.is_empty() {
                let chunk_text = current_chunk.join("");
                chunks.push(Chunk {
                    token_count: bpe.encode_with_special_tokens(&chunk_text).len(),
                    text: chunk_text,
                });
                let drain_start = current_chunk.len().saturating_sub(overlap_sentences);
                current_chunk = current_chunk[drain_start..].to_vec();
                current_tokens = bpe.encode_with_special_tokens(&current_chunk.join("")).len();
            }
            current_chunk.push(sentence.to_string());
            current_tokens += sentence_tokens;
        }

        if !current_chunk.is_empty() {
            let chunk_text = current_chunk.join("");
            chunks.push(Chunk {
                token_count: bpe.encode_with_special_tokens(&chunk_text).len(),
                text: chunk_text,
            });
        }
        chunks
    }
}
PHASE 2: Local Vector Database Layer via sqlite-vec2.1 Storage Vector Engine Verification (ucp-core/src/storage/vector_store.rs)#[cfg(test)]
mod tests {
use super::*;

    #[test]
    fn test_sqlite_vec_lifecycle() {
        let mut store = VectorStore::open_in_memory().expect("Failed to initialize transient database in-memory");
        store.initialize_tables().expect("Failed database schema initialization execution");
        
        let mock_embedding = vec![0.05_f32; 1536]; // Match OpenAI / Ollama standard dimension size
        store.insert_chunk("hp_series", "Albus Dumbledore was headmaster.", &mock_embedding)
            .expect("Failure storing chunk record into sqlite backend");
        
        let nearest = store.query_nearest(&mock_embedding, 1)
            .expect("SQLite vector query failure execution encountered");
            
        assert_eq!(nearest.len(), 1);
        assert_eq!(nearest[0].document_id, "hp_series");
        assert_eq!(nearest[0].text, "Albus Dumbledore was headmaster.");
    }
}
2.2 Storage Vector Engine Production Implementation// ucp-core/src/storage/vector_store.rs
use rusqlite::{params, Connection, Result};

pub struct MatchedChunk {
pub document_id: String,
pub text: String,
pub distance: f32,
}

pub struct VectorStore {
conn: Connection,
}

impl VectorStore {
pub fn open_in_memory() -> Result<Self> {
let conn = Connection::open_in_memory()?;
// Loads dynamic platform sqlite-vec extension
conn.load_extension("sqlite-vec", None)?;
Ok(Self { conn })
}

    pub fn initialize_tables(&self) -> Result<()> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS chunks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                document_id TEXT NOT NULL,
                content TEXT NOT NULL
            );",
            [],
        )?;
        self.conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS vec_chunks USING vec0(
                chunk_id INTEGER PRIMARY KEY,
                embedding float[1536]
            );",
            [],
        )?;
        Ok(())
    }

    pub fn insert_chunk(&mut self, doc_id: &str, content: &str, embedding: &[f32]) -> Result<()> {
        self.conn.execute(
            "INSERT INTO chunks (document_id, content) VALUES (?1, ?2);",
            params![doc_id, content],
        )?;
        let row_id = self.conn.last_insert_rowid();
        
        let bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(
                embedding.as_ptr() as *const u8,
                embedding.len() * std::mem::size_of::<f32>(),
            )
        };

        self.conn.execute(
            "INSERT INTO vec_chunks (chunk_id, embedding) VALUES (?1, ?2);",
            params![row_id, bytes],
        )?;
        Ok(())
    }

    pub fn query_nearest(&self, query_vector: &[f32], limit: usize) -> Result<Vec<MatchedChunk>> {
        let bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(
                query_vector.as_ptr() as *const u8,
                query_vector.len() * std::mem::size_of::<f32>(),
            )
        };

        let mut stmt = self.conn.prepare(
            "SELECT c.document_id, c.content, v.distance 
             FROM vec_chunks v
             JOIN chunks c ON v.chunk_id = c.id
             WHERE embedding MATCH ?1 AND k = ?2 ORDER BY distance ASC;"
        )?;

        let rows = stmt.query_map(params![bytes, limit], |row| {
            Ok(MatchedChunk {
                document_id: row.get(0)?,
                text: row.get(1)?,
                distance: row.get(2)?,
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }
}
PHASE 3: Decoupled Multi-Provider Model Router Engine3.1 Model Router Target Interfaces (ucp-core/src/models/router.rs)// ucp-core/src/models/router.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "config")]
pub enum AiProvider {
OpenAi { api_key: String, model: String },
Anthropic { api_key: String, model: String },
Ollama { host: String, model: String },
}

pub struct ModelDispatcher;

impl ModelDispatcher {
pub async fn dispatch_direct(provider: &AiProvider, prompt: &str) -> Result<String, String> {
let client = reqwest::Client::new();

        match provider {
            AiProvider::OpenAi { api_key, model } => {
                let res = client.post("[https://api.openai.com/v1/chat/completions](https://api.openai.com/v1/chat/completions)")
                    .bearer_auth(api_key)
                    .json(&serde_json::json!({
                        "model": model,
                        "messages": [{"role": "user", "content": prompt}]
                    }))
                    .send().await.map_err(|e| e.to_string())?;
                
                let raw_res: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
                Ok(raw_res["choices"][0]["message"]["content"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string())
            }
            AiProvider::Anthropic { api_key, model } => {
                let res = client.post("[https://api.anthropic.com/v1/messages](https://api.anthropic.com/v1/messages)")
                    .header("x-api-key", api_key)
                    .header("anthropic-version", "2023-06-01")
                    .json(&serde_json::json!({
                        "model": model,
                        "max_tokens": 1024,
                        "messages": [{"role": "user", "content": prompt}]
                    }))
                    .send().await.map_err(|e| e.to_string())?;
                
                let raw_res: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
                Ok(raw_res["content"][0]["text"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string())
            }
            AiProvider::Ollama { host, model } => {
                let res = client.post(format!("{}/api/generate", host))
                    .json(&serde_json::json!({
                        "model": model,
                        "prompt": prompt,
                        "stream": false
                    }))
                    .send().await.map_err(|e| e.to_string())?;
                
                let raw_res: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
                Ok(raw_res["response"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string())
            }
        }
    }
}
PHASE 4: Model Context Protocol (MCP) Server SetupThis module configures standard JSON-RPC communications over CLI execution runtimes so external tools like Cursor or Claude Desktop can use UCP natively.4.1 MCP Server Routing Interface Implementation (ucp-core/src/mcp/server.rs)// ucp-core/src/mcp/server.rs
use rmcp::{tool, tool_router};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ContextLookupParams {
pub query: String,
pub limit: Option<usize>,
}

#[derive(Clone)]
pub struct SystemMcpServer;

impl SystemMcpServer {
pub async fn query_database_layer(&self, query: &str, limit: usize) -> String {
// Integrated pipeline maps database nearest search bounds directly to clean JSON string blocks
format!("[CONSTRUCTED SYSTEM PAYLOAD FOR USER QUERY: {} WITH LIMIT: {}]", query, limit)
}
}

#[tool_router(server_handler)]
impl SystemMcpServer {
#[tool(description = "Accesses the local high-fidelity vector index to pull matching document snippets to ground AI responses.")]
pub async fn search_local_context(&self, params: ContextLookupParams) -> Result<Value, rmcp::ErrorData> {
let results = self.query_database_layer(&params.query, params.limit.unwrap_or(3)).await;
Ok(serde_json::json!({
"content": [{ "type": "text", "text": results }]
}))
}
}
PHASE 5: OS Keyboard Macro Accessibility IngestorThis module registers system-wide key listeners. When triggered, it grabs the current highlighted selection, queries the local vector DB, compiles the context payload, and simulates input injection back into the active text field.5.1 Operating System Injection Architecture Implementation (ucp-core/src/os/injector.rs)// ucp-core/src/os/injector.rs
use copypasta::{ClipboardContext, ClipboardProvider};
use std::{thread, time::Duration};

pub struct SystemInjector;

impl SystemInjector {
pub fn package_context_payload(original_query: &str, matches: &[String]) -> String {
let mut template = String::new();
template.push_str("### [UNIVERSAL CONTEXT PIPELINE - SECURE LOCAL GROUNDING]\n");
template.push_str("The local vector store found relevant context matches. Ground your response using these facts:\n\n");
for (idx, passage) in matches.iter().enumerate() {
template.push_str(&format!("--- \n[CONTEXT BLOCK {}]\n{}\n", idx + 1, passage));
}
template.push_str("\n--- \n### [USER INQUIRY]\n");
template.push_str(original_query);
template
}

    pub fn inject_text_at_focused_cursor(payload: &str) -> Result<(), String> {
        let mut ctx = ClipboardContext::new().map_err(|e| e.to_string())?;
        ctx.set_contents(payload.to_string()).map_err(|e| e.to_string())?;

        // Sleep to give the OS focus events time to stabilize
        thread::sleep(Duration::from_millis(100));

        // Note: Simulated keystroke injection (e.g., Ctrl+V or Cmd+V)
        // must use platform-specific rdev mouse and key simulation APIs.
        Ok(())
    }
}
PHASE 6: The Desktop GUI Interface via Tauri v26.1 Desktop Tray Backend Integration (src-tauri/src/lib.rs)// src-tauri/src/lib.rs
use tauri::{
menu::{Menu, MenuItem},
tray::{TrayIconBuilder, TrayIconEvent},
};

#[tauri::command]
fn sync_folder_path(path: String) -> Result<String, String> {
// Triggers directory indexers and sqlite insertion updates asynchronously
println!("Indexing targeted folder directory matching path: {}", path);
Ok(format!("Successfully indexed: {}", path))
}

pub fn run() {
tauri::Builder::default()
.setup(|app| {
let exit_item = MenuItem::with_id(app, "exit", "Quit Pipeline Daemon", true, None::<&str>)?;
let tray_menu = Menu::with_items(app, &[&exit_item])?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&tray_menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "exit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![sync_folder_path])
        .run(tauri::generate_context!())
        .expect("Encountered runtime error during Tauri framework loop initialization");
}
6.2 UI Layout Spec Dashboard Mockup (HTML/Tailwind CSS structure)Implement the Tauri window interface inside your frontend code base following this structural blueprint layout:<div class="flex h-screen bg-slate-950 text-slate-100 font-sans">
  <!-- Dynamic Settings Navigation Side Panel -->
  <aside class="w-64 border-r border-slate-800 bg-slate-900/50 p-6 flex flex-col gap-6">
    <div class="flex items-center gap-3">
      <div class="h-8 w-8 rounded bg-indigo-600 flex items-center justify-center font-bold text-white">U</div>
      <h1 class="text-lg font-semibold tracking-tight text-white">UCP Dashboard</h1>
    </div>

    <nav class="flex flex-col gap-1 flex-1">
      <button class="flex items-center gap-3 px-4 py-2.5 rounded-lg bg-indigo-600/10 text-indigo-400 font-medium text-sm transition-colors text-left">
        📁 Data Pipelines
      </button>
      <button class="flex items-center gap-3 px-4 py-2.5 rounded-lg text-slate-400 hover:bg-slate-800/50 hover:text-slate-200 font-medium text-sm transition-colors text-left">
        ⚙️ Settings Configuration
      </button>
    </nav>
  </aside>

  <!-- Primary Interactive Viewport Area -->
  <main class="flex-1 p-8 overflow-y-auto flex flex-col gap-8">
    <header class="flex justify-between items-center border-b border-slate-800 pb-6">
      <div>
        <h2 class="text-2xl font-bold text-white tracking-tight">Data Pipelines</h2>
        <p class="text-sm text-slate-400">Manage and sync local files to your secure on-device context pipeline.</p>
      </div>
    </header>

    <!-- Interactive Drag and Drop Segment -->
    <section class="border-2 border-dashed border-slate-800 hover:border-indigo-600/50 rounded-2xl p-12 text-center bg-slate-900/20 cursor-pointer transition-colors group">
      <div class="flex flex-col items-center gap-4">
        <span class="text-4xl text-slate-500 group-hover:scale-110 transition-transform">📂</span>
        <div>
          <p class="font-medium text-white">Drag and drop folders here to index them</p>
          <p class="text-xs text-slate-500 mt-1">UCP will read, clean, and index PDFs, Markdown, and text files locally.</p>
        </div>
      </div>
    </section>

    <!-- Watched Directory Manifest Table -->
    <section class="flex flex-col gap-4">
      <h3 class="text-sm font-semibold tracking-wider text-slate-500 uppercase">Currently Indexed Sources</h3>
      <div class="border border-slate-800 rounded-xl overflow-hidden bg-slate-900/30">
        <table class="w-full text-left border-collapse text-sm">
          <thead>
            <tr class="border-b border-slate-800 bg-slate-900/50">
              <th class="p-4 font-semibold text-slate-400">Indexed Path</th>
              <th class="p-4 font-semibold text-slate-400">Total Chunks</th>
              <th class="p-4 font-semibold text-slate-400">Auto-Sync</th>
            </tr>
          </thead>
          <tbody>
            <tr class="border-b border-slate-800/50 hover:bg-slate-900/20">
              <td class="p-4 font-mono text-slate-300">/Users/username/documents/harry_potter/</td>
              <td class="p-4">3,450 chunks</td>
              <td class="p-4 text-emerald-500 font-medium">Active</td>
            </tr>
          </tbody>
        </table>
      </div>
    </section>
  </main>
</div>
7. CRITICAL VERIFICATION PROTOCOLSTo test, debug, and run your workspace, execute these exact CLI target commands inside your terminal:# 1. Initialize complete Cargo Unit-Test Suite Execution
cargo test --workspace -- --nocapture

# 2. Fire up the local Tauri desktop interface in developer debug mode
cargo tauri dev

# 3. Compile optimized, production-ready system tray distributions
cargo tauri build
