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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    pub host: String,
    pub embedding_model: String,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            host: "http://localhost:11434".to_string(),
            embedding_model: "nomic-embed-text".to_string(),
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

impl Default for Config {
    fn default() -> Self {
        Self {
            folders: Vec::new(),
            ollama: OllamaConfig::default(),
            chunking: ChunkingConfig::default(),
        }
    }
}

impl Config {
    pub fn config_path() -> Result<PathBuf> {
        let dirs = ProjectDirs::from("io", "ucp", "ucp").context("could not resolve config dir")?;
        Ok(dirs.config_dir().join("config.toml"))
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
