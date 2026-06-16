use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub folders: Vec<PathBuf>,
    #[serde(default)]
    pub ollama: OllamaConfig,
    #[serde(default)]
    pub chunking: ChunkingConfig,
    #[serde(default)]
    pub pdf: PdfConfig,
    #[serde(default)]
    pub watcher: WatcherConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    pub host: String,
    pub embedding_model: String,
    #[serde(default = "default_embedding_dim")]
    pub embedding_dim: usize,
    #[serde(default = "default_chat_model")]
    pub chat_model: String,
}

fn default_chat_model() -> String {
    "llama3.2".to_string()
}

fn default_embedding_dim() -> usize {
    768
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            host: "http://localhost:11434".to_string(),
            embedding_model: "nomic-embed-text".to_string(),
            embedding_dim: default_embedding_dim(),
            chat_model: default_chat_model(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkingConfig {
    pub max_tokens: usize,
    pub overlap_sentences: usize,
}

impl Default for ChunkingConfig {
    fn default() -> Self {
        Self { max_tokens: 512, overlap_sentences: 1 }
    }
}

/// PDF extraction settings. `pdftotext_command` is the poppler binary used as
/// the fallback when the in-process `pdf-extract` crate yields little/no text.
/// Set it to a full path (e.g. `/opt/homebrew/bin/pdftotext`) if poppler is
/// installed somewhere not on `PATH`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfConfig {
    pub pdftotext_command: String,
    pub min_useful_bytes: usize,
}

impl Default for PdfConfig {
    fn default() -> Self {
        Self {
            pdftotext_command: "pdftotext".to_string(),
            min_useful_bytes: 32,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatcherConfig {
    pub debounce_ms: u64,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self { debounce_ms: 500 }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            folders: Vec::new(),
            ollama: OllamaConfig::default(),
            chunking: ChunkingConfig::default(),
            pdf: PdfConfig::default(),
            watcher: WatcherConfig::default(),
        }
    }
}

impl Config {
    fn dirs() -> Result<ProjectDirs> {
        ProjectDirs::from("io", "ak1", "ucp-local").context("could not resolve user dirs")
    }

    pub fn config_path() -> Result<PathBuf> {
        Ok(Self::dirs()?.config_dir().join("config.toml"))
    }

    pub fn data_path() -> Result<PathBuf> {
        Ok(Self::dirs()?.data_dir().join("index.sqlite"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("reading config at {}", path.display()))?;
        toml::from_str(&raw).context("parsing config TOML")
    }
}