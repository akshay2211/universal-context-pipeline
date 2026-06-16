use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use ucp_local::config::Config;
use ucp_local::embeddings::OllamaClient;
use indicatif::{ProgressBar, ProgressStyle};
use std::io::IsTerminal;
use std::sync::atomic::{AtomicUsize, Ordering};
use ucp_local::embeddings::ChatMessage;
use ucp_local::indexer::{self, IndexEvent, IndexOptions, ProgressFn};
use ucp_local::ingestion::conversation_export;
use ucp_local::mcp::McpServer;
use ucp_local::storage::{MatchedChunk, VectorStore};
use ucp_local::watcher;

#[derive(Parser)]
#[command(name = "ucp-local", version, about = "Universal Context Pipeline — local MCP context server")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Index one or more folders (or files) into the local vector + FTS store
    Index {
        /// One or more paths. Each path is indexed sequentially into the same store.
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// Skip PII masking on file contents.
        #[arg(long)]
        no_mask: bool,
    },
    /// Run the MCP server over stdio (point Claude Desktop / Cursor at this)
    Serve,
    /// Watch a folder and re-index on change
    Watch {
        path: PathBuf,
        #[arg(long)]
        no_mask: bool,
    },
    /// Show config + index status
    Status,
    /// Ingest a Claude conversations.json export into the index
    IngestConversations { path: PathBuf },
    /// Run a hybrid search and print matching chunks with citations (no LLM)
    Search {
        query: String,
        #[arg(short, long, default_value_t = 5)]
        limit: usize,
        #[arg(long)]
        folder: Option<PathBuf>,
    },
    /// Ask a question — searches the index, then asks a local chat model to answer with citations
    Ask {
        question: String,
        #[arg(short, long, default_value_t = 5)]
        limit: usize,
        #[arg(long)]
        folder: Option<PathBuf>,
        /// Override the chat model (default from config.ollama.chat_model)
        #[arg(long)]
        model: Option<String>,
    },
    /// Clear the index. With no path: wipe everything (cache preserved). With a path: wipe only that prefix.
    Clear {
        /// Path prefix to clear. Omit to clear all chunks.
        path: Option<PathBuf>,
        /// Also wipe the embedding cache (forces re-embed on next index)
        #[arg(long)]
        hard: bool,
        /// Skip the confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr) // never pollute stdio for `ucp serve`
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Index { paths, no_mask } => cmd_index(paths, no_mask).await,
        Command::Serve => cmd_serve().await,
        Command::Watch { path, no_mask } => cmd_watch(path, no_mask).await,
        Command::Status => cmd_status().await,
        Command::IngestConversations { path } => cmd_ingest_conversations(path).await,
        Command::Search { query, limit, folder } => cmd_search(query, limit, folder).await,
        Command::Ask { question, limit, folder, model } => {
            cmd_ask(question, limit, folder, model).await
        }
        Command::Clear { path, hard, yes } => cmd_clear(path, hard, yes).await,
    }
}

async fn cmd_index(paths: Vec<PathBuf>, no_mask: bool) -> Result<()> {
    let config = Config::load()?;
    check_pdftotext(&config.pdf.pdftotext_command);
    let mut store = open_store(&config)?;
    let ollama = OllamaClient::new(&config.ollama.host, &config.ollama.embedding_model);

    let opts = IndexOptions {
        no_mask,
        max_tokens: config.chunking.max_tokens,
        overlap_sentences: config.chunking.overlap_sentences,
        pdf: config.pdf.clone(),
    };

    let mut total_files = 0usize;
    let mut total_skipped = 0usize;
    let mut total_chunks = 0usize;
    let mut total_embeds = 0usize;
    let mut total_cached = 0usize;

    for path in &paths {
        tracing::info!(?path, "indexing");
        let progress_ctx = ProgressContext::new_for_path(path);
        let callback = progress_ctx.callback();
        let stats = indexer::index_path(
            path,
            &mut store,
            &ollama,
            &opts,
            callback.as_ref().map(|f| f.as_ref()),
        )
        .await
        .with_context(|| format!("indexing {}", path.display()))?;
        progress_ctx.finish_with_summary();
        println!(
            "  {}: {} files ({} skipped) → {} chunks  [embed: {} live, {} cached]",
            path.display(),
            stats.files_processed,
            stats.files_skipped,
            stats.chunks_inserted,
            stats.embed_calls,
            stats.cache_hits,
        );
        total_files += stats.files_processed;
        total_skipped += stats.files_skipped;
        total_chunks += stats.chunks_inserted;
        total_embeds += stats.embed_calls;
        total_cached += stats.cache_hits;
    }
    if paths.len() > 1 {
        println!(
            "── total: {} files ({} skipped) → {} chunks  [embed: {} live, {} cached]",
            total_files, total_skipped, total_chunks, total_embeds, total_cached,
        );
    }
    Ok(())
}

async fn cmd_serve() -> Result<()> {
    let config = Config::load()?;
    let store = open_store(&config)?;
    let ollama = OllamaClient::new(&config.ollama.host, &config.ollama.embedding_model);
    let server = McpServer::new(store, ollama);
    server.run_stdio().await
}

async fn cmd_watch(path: PathBuf, no_mask: bool) -> Result<()> {
    let config = Config::load()?;
    check_pdftotext(&config.pdf.pdftotext_command);
    let mut store = open_store(&config)?;
    let ollama = OllamaClient::new(&config.ollama.host, &config.ollama.embedding_model);
    let opts = IndexOptions {
        no_mask,
        max_tokens: config.chunking.max_tokens,
        overlap_sentences: config.chunking.overlap_sentences,
        pdf: config.pdf.clone(),
    };

    // Do an initial pass so the store reflects current state before listening.
    let progress_ctx = ProgressContext::new_for_path(&path);
    let callback = progress_ctx.callback();
    let stats = indexer::index_path(
        &path,
        &mut store,
        &ollama,
        &opts,
        callback.as_ref().map(|f| f.as_ref()),
    )
    .await?;
    progress_ctx.finish_with_summary();
    tracing::info!(
        files = stats.files_processed,
        chunks = stats.chunks_inserted,
        cached = stats.cache_hits,
        "initial index complete"
    );

    watcher::watch_folder(&path, &mut store, &ollama, &opts, config.watcher.debounce_ms).await
}

async fn cmd_status() -> Result<()> {
    let config = Config::load()?;
    let config_path = Config::config_path()?;
    let data_path = Config::data_path()?;
    let store = open_store(&config)?;

    println!("config: {}", config_path.display());
    println!("config present: {}", config_path.exists());
    println!("index db: {}", data_path.display());
    println!("chunks: {}", store.chunk_count()?);
    Ok(())
}

fn open_store(config: &Config) -> Result<VectorStore> {
    let path = Config::data_path()?;
    VectorStore::open(&path, config.ollama.embedding_dim)
}

async fn cmd_clear(path: Option<PathBuf>, hard: bool, yes: bool) -> Result<()> {
    let config = Config::load()?;
    let mut store = open_store(&config)?;
    let total_chunks = store.chunk_count()?;
    let cache_count = store.embeddings_cache_count()?;

    // Build the action description for the confirmation prompt + log line.
    let scope = match path.as_deref() {
        Some(p) => format!("chunks under `{}`", p.display()),
        None => format!("all {total_chunks} chunks"),
    };
    let cache_action = if hard {
        format!(", and {cache_count} cached embeddings")
    } else {
        " (embedding cache preserved)".to_string()
    };

    if !yes {
        use std::io::Write;
        print!("Clear {scope}{cache_action}? [y/N] ");
        std::io::stdout().flush().ok();
        let mut reply = String::new();
        std::io::stdin().read_line(&mut reply)?;
        if !matches!(reply.trim(), "y" | "Y" | "yes" | "Yes") {
            println!("aborted");
            return Ok(());
        }
    }

    let removed = match path.as_deref() {
        Some(p) => store.delete_chunks_under(p)?,
        None => store.clear_chunks()?,
    };
    let cache_removed = if hard { store.clear_embeddings_cache()? } else { 0 };

    if hard {
        println!("cleared {removed} chunks and {cache_removed} cached embeddings");
    } else {
        println!("cleared {removed} chunks (embedding cache preserved)");
    }
    Ok(())
}

async fn cmd_search(
    query: String,
    limit: usize,
    folder: Option<PathBuf>,
) -> Result<()> {
    let config = Config::load()?;
    let store = open_store(&config)?;
    let ollama = OllamaClient::new(&config.ollama.host, &config.ollama.embedding_model);

    let embedding = ollama
        .embed(&query)
        .await
        .context("embedding query — is Ollama running?")?;
    let hits = store.hybrid_search(&query, &embedding, limit, folder.as_deref())?;

    if hits.is_empty() {
        println!("No matching context found.");
        return Ok(());
    }
    for (i, hit) in hits.iter().enumerate() {
        println!(
            "─── [{}] {}:{}-{}  (score {:.4})",
            i + 1,
            hit.source.file_path.display(),
            hit.source.start_line,
            hit.source.end_line,
            hit.score,
        );
        println!("{}\n", hit.text.trim());
    }
    Ok(())
}

async fn cmd_ask(
    question: String,
    limit: usize,
    folder: Option<PathBuf>,
    model_override: Option<String>,
) -> Result<()> {
    let config = Config::load()?;
    let store = open_store(&config)?;
    let ollama = OllamaClient::new(&config.ollama.host, &config.ollama.embedding_model);

    let chat_model = model_override.unwrap_or_else(|| config.ollama.chat_model.clone());

    let embedding = ollama
        .embed(&question)
        .await
        .context("embedding question — is Ollama running?")?;
    let hits = store.hybrid_search(&question, &embedding, limit, folder.as_deref())?;

    if hits.is_empty() {
        println!("(no local context found — answering without grounding)");
    }

    let context_block = format_context_block(&hits);
    let user_msg = format!("Context:\n{context_block}\n\nQuestion: {question}");
    let messages = [
        ChatMessage {
            role: "system",
            content: "You are a research assistant grounded in the user's local files. Answer using only the provided context. Cite sources inline as [file_path:start_line-end_line]. If the context does not contain the answer, say so plainly — do not invent facts.",
        },
        ChatMessage { role: "user", content: &user_msg },
    ];

    let answer = ollama
        .chat(&chat_model, &messages)
        .await
        .with_context(|| format!("calling chat model `{chat_model}` — is it pulled? (try: ollama pull {chat_model})"))?;
    println!("{}", answer.trim());
    Ok(())
}

fn format_context_block(hits: &[MatchedChunk]) -> String {
    if hits.is_empty() {
        return "(no context retrieved)".to_string();
    }
    hits.iter()
        .map(|h| {
            format!(
                "[{}:{}-{}]\n{}",
                h.source.file_path.display(),
                h.source.start_line,
                h.source.end_line,
                h.text.trim(),
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n---\n\n")
}

/// Bundles an indicatif progress bar with the counters needed to drive it
/// from indexer events. No-ops when stdout isn't a TTY so logs and CI output
/// stay clean.
struct ProgressContext {
    bar: Option<ProgressBar>,
    cached: std::sync::Arc<AtomicUsize>,
    live: std::sync::Arc<AtomicUsize>,
}

impl ProgressContext {
    fn new_for_path(path: &std::path::Path) -> Self {
        if !std::io::stderr().is_terminal() {
            return Self::disabled();
        }
        let total = indexer::count_indexable_files(path);
        Self::build_bar(total as u64, format!("indexing {}", path.display()))
    }

    fn new_for_chunks(total_chunks: usize) -> Self {
        if !std::io::stderr().is_terminal() {
            return Self::disabled();
        }
        Self::build_bar(total_chunks as u64, "ingesting conversation turns".to_string())
    }

    fn build_bar(total: u64, prefix: String) -> Self {
        let bar = ProgressBar::new(total);
        bar.set_style(
            ProgressStyle::with_template(
                "{prefix}\n  [{bar:40}] {pos}/{len}  {msg}  [{elapsed_precise} / eta {eta}]",
            )
            .unwrap()
            .progress_chars("█▉▊▋▌▍▎▏ "),
        );
        bar.set_prefix(prefix);
        Self {
            bar: Some(bar),
            cached: std::sync::Arc::new(AtomicUsize::new(0)),
            live: std::sync::Arc::new(AtomicUsize::new(0)),
        }
    }

    fn disabled() -> Self {
        Self {
            bar: None,
            cached: std::sync::Arc::new(AtomicUsize::new(0)),
            live: std::sync::Arc::new(AtomicUsize::new(0)),
        }
    }

    fn callback(&self) -> Option<Box<ProgressFn<'static>>> {
        let bar = self.bar.clone()?;
        let cached = self.cached.clone();
        let live = self.live.clone();
        Some(Box::new(move |event: IndexEvent<'_>| match event {
            IndexEvent::Start { path, .. } => {
                bar.set_message(format!("current: {}", path.display()));
            }
            IndexEvent::Finish { .. } => {
                bar.inc(1);
                let c = cached.load(Ordering::Relaxed);
                let l = live.load(Ordering::Relaxed);
                let total = c + l;
                if total > 0 {
                    bar.set_message(format!(
                        "embed: {l} live / {c} cached ({:.0}%)",
                        (c as f64 / total as f64) * 100.0
                    ));
                }
            }
            IndexEvent::Chunk { from_cache } => {
                if from_cache {
                    cached.fetch_add(1, Ordering::Relaxed);
                } else {
                    live.fetch_add(1, Ordering::Relaxed);
                }
            }
        }))
    }

    fn finish_with_summary(self) {
        if let Some(bar) = self.bar {
            bar.finish_and_clear();
        }
    }
}

/// Warn once if the configured pdftotext binary isn't runnable. Only relevant
/// for paths that may extract PDF content (index/watch/ingest-conversations).
/// The binary name comes from `config.pdf.pdftotext_command`.
fn check_pdftotext(command: &str) {
    let present = std::process::Command::new(command)
        .arg("-v")
        .output()
        .is_ok();
    if present {
        return;
    }
    let install = match std::env::consts::OS {
        "macos" => "brew install poppler",
        "linux" => "apt install poppler-utils   # or: dnf install poppler-utils",
        "windows" => "choco install poppler   # or download from poppler.freedesktop.org",
        _ => "install poppler for your platform",
    };
    tracing::warn!(
        "`{command}` not found on PATH — PDFs with broken or partial text layers \
         will only use the built-in pdf-extract path and may extract poorly. \
         Install poppler for better coverage: {install}  \
         (or set `pdf.pdftotext_command` in config.toml)"
    );
}

async fn cmd_ingest_conversations(path: PathBuf) -> Result<()> {
    let config = Config::load()?;
    let mut store = open_store(&config)?;
    let ollama = OllamaClient::new(&config.ollama.host, &config.ollama.embedding_model);

    tracing::info!(?path, "ingesting Claude conversations export");
    let chunks = conversation_export::ingest_claude_export(&path)?;
    let count = chunks.len();

    // Re-import semantics: replace any prior turns from this export path.
    store.delete_chunks_for_path(&path)?;

    let mtime = std::fs::metadata(&path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let progress_ctx = ProgressContext::new_for_chunks(count);
    let callback = progress_ctx.callback();
    let stats = indexer::index_chunks(
        chunks,
        mtime,
        &mut store,
        &ollama,
        callback.as_ref().map(|f| f.as_ref()),
    )
    .await?;
    progress_ctx.finish_with_summary();
    println!(
        "ingested {count} turns → {} chunks  [embed: {} live, {} cached]",
        stats.chunks_inserted, stats.embed_calls, stats.cache_hits,
    );
    Ok(())
}
