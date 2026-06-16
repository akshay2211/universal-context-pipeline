use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "ucp", version, about = "Universal Context Pipeline — local MCP context server")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Index a folder into the local vector + FTS store
    Index {
        path: PathBuf,
        #[arg(long)]
        no_mask: bool,
    },
    /// Run the MCP server over stdio (point Claude Desktop / Cursor at this)
    Serve,
    /// Watch a folder and re-index on change
    Watch { path: PathBuf },
    /// Show index status: folders, chunk counts, last sync
    Status,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Index { path, no_mask } => {
            tracing::info!(?path, no_mask, "index — TODO");
        }
        Command::Serve => {
            tracing::info!("serve — TODO");
        }
        Command::Watch { path } => {
            tracing::info!(?path, "watch — TODO");
        }
        Command::Status => {
            tracing::info!("status — TODO");
        }
    }
    Ok(())
}
