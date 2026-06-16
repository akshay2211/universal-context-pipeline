use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use ucp::config::Config;
use ucp::embeddings::OllamaClient;
use ucp::indexer::{self, IndexOptions};
use ucp::ingestion::conversation_export;
use ucp::mcp::McpServer;
use ucp::storage::VectorStore;
use ucp::watcher;

#[derive(Parser)]
#[command(name = "ucp", version, about = "Universal Context Pipeline — local MCP context server")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Index a folder (or single file) into the local vector + FTS store
    Index {
        path: PathBuf,
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
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr) // never pollute stdio for `ucp serve`
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Index { path, no_mask } => cmd_index(path, no_mask).await,
        Command::Serve => cmd_serve().await,
        Command::Watch { path, no_mask } => cmd_watch(path, no_mask).await,
        Command::Status => cmd_status().await,
        Command::IngestConversations { path } => cmd_ingest_conversations(path).await,
    }
}

async fn cmd_index(path: PathBuf, no_mask: bool) -> Result<()> {
    let config = Config::load()?;
    let mut store = open_store()?;
    let ollama = OllamaClient::new(&config.ollama.host, &config.ollama.embedding_model);

    let opts = IndexOptions {
        no_mask,
        max_tokens: config.chunking.max_tokens,
        overlap_sentences: config.chunking.overlap_sentences,
    };

    tracing::info!(?path, "indexing");
    let stats = indexer::index_path(&path, &mut store, &ollama, &opts)
        .await
        .context("indexing failed")?;

    println!(
        "indexed {} files ({} skipped) → {} chunks  [embed: {} live, {} cached]",
        stats.files_processed,
        stats.files_skipped,
        stats.chunks_inserted,
        stats.embed_calls,
        stats.cache_hits,
    );
    Ok(())
}

async fn cmd_serve() -> Result<()> {
    let config = Config::load()?;
    let store = open_store()?;
    let ollama = OllamaClient::new(&config.ollama.host, &config.ollama.embedding_model);
    let server = McpServer::new(store, ollama);
    server.run_stdio().await
}

async fn cmd_watch(path: PathBuf, no_mask: bool) -> Result<()> {
    let config = Config::load()?;
    let mut store = open_store()?;
    let ollama = OllamaClient::new(&config.ollama.host, &config.ollama.embedding_model);
    let opts = IndexOptions {
        no_mask,
        max_tokens: config.chunking.max_tokens,
        overlap_sentences: config.chunking.overlap_sentences,
    };

    // Do an initial pass so the store reflects current state before listening.
    let stats = indexer::index_path(&path, &mut store, &ollama, &opts).await?;
    tracing::info!(
        files = stats.files_processed,
        chunks = stats.chunks_inserted,
        cached = stats.cache_hits,
        "initial index complete"
    );

    watcher::watch_folder(&path, &mut store, &ollama, &opts).await
}

async fn cmd_status() -> Result<()> {
    let config_path = Config::config_path()?;
    let data_path = Config::data_path()?;
    let store = open_store()?;

    println!("config: {}", config_path.display());
    println!("config present: {}", config_path.exists());
    println!("index db: {}", data_path.display());
    println!("chunks: {}", store.chunk_count()?);
    Ok(())
}

fn open_store() -> Result<VectorStore> {
    let path = Config::data_path()?;
    VectorStore::open(&path)
}

async fn cmd_ingest_conversations(path: PathBuf) -> Result<()> {
    let config = Config::load()?;
    let mut store = open_store()?;
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

    let stats = indexer::index_chunks(chunks, mtime, &mut store, &ollama).await?;
    println!(
        "ingested {count} turns → {} chunks  [embed: {} live, {} cached]",
        stats.chunks_inserted, stats.embed_calls, stats.cache_hits,
    );
    Ok(())
}
