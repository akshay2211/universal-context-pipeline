use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[async_trait]
pub trait Embedder: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
}

#[async_trait]
impl Embedder for OllamaClient {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        OllamaClient::embed(self, text).await
    }
}

#[derive(Clone)]
pub struct OllamaClient {
    host: String,
    model: String,
    client: reqwest::Client,
}

impl OllamaClient {
    pub fn new(host: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            host: host.into().trim_end_matches('/').to_string(),
            model: model.into(),
            client: reqwest::Client::new(),
        }
    }

    pub fn default_local() -> Self {
        Self::new("http://localhost:11434", "nomic-embed-text")
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let url = format!("{}/api/embeddings", self.host);
        let body = EmbeddingRequest { model: &self.model, prompt: text };

        let res = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .with_context(|| format!("POST {url}"))?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            anyhow::bail!("ollama {status}: {text}");
        }

        let parsed: EmbeddingResponse = res
            .json()
            .await
            .context("decoding ollama embeddings response")?;
        Ok(parsed.embedding)
    }

    /// Non-streaming chat completion. Used by `ucp ask`.
    pub async fn chat(&self, model: &str, messages: &[ChatMessage<'_>]) -> Result<String> {
        let url = format!("{}/api/chat", self.host);
        let body = ChatRequest { model, messages, stream: false };

        let res = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .with_context(|| format!("POST {url}"))?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            anyhow::bail!("ollama {status}: {text}");
        }

        let parsed: ChatResponse = res
            .json()
            .await
            .context("decoding ollama chat response")?;
        Ok(parsed.message.content)
    }
}

#[derive(Serialize)]
pub struct ChatMessage<'a> {
    pub role: &'a str,
    pub content: &'a str,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage<'a>],
    stream: bool,
}

#[derive(Deserialize)]
struct ChatResponse {
    message: ChatResponseMessage,
}

#[derive(Deserialize)]
struct ChatResponseMessage {
    content: String,
}

#[derive(Serialize)]
struct EmbeddingRequest<'a> {
    model: &'a str,
    prompt: &'a str,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    embedding: Vec<f32>,
}

/// SHA-256 content-hash → embedding cache. Avoids re-embedding unchanged chunks
/// across re-indexes. Stored alongside chunks in SQLite.
pub struct EmbeddingCache;

impl EmbeddingCache {
    pub fn hash(text: &str) -> [u8; 32] {
        use sha2::{Digest, Sha256};

        let normalized = normalize(text);
        let mut hasher = Sha256::new();
        hasher.update(normalized.as_bytes());
        hasher.finalize().into()
    }
}

fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last_was_space = false;
    for ch in text.trim().chars() {
        if ch.is_whitespace() {
            if !last_was_space {
                out.push(' ');
                last_was_space = true;
            }
        } else {
            out.push(ch);
            last_was_space = false;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_deterministic() {
        let a = EmbeddingCache::hash("hello world");
        let b = EmbeddingCache::hash("hello world");
        assert_eq!(a, b);
    }

    #[test]
    fn hash_ignores_surrounding_whitespace() {
        let a = EmbeddingCache::hash("hello world");
        let b = EmbeddingCache::hash("   hello world\n");
        assert_eq!(a, b);
    }

    #[test]
    fn hash_collapses_internal_whitespace_runs() {
        let a = EmbeddingCache::hash("hello world");
        let b = EmbeddingCache::hash("hello    world");
        let c = EmbeddingCache::hash("hello\tworld");
        let d = EmbeddingCache::hash("hello\n world");
        assert_eq!(a, b);
        assert_eq!(a, c);
        assert_eq!(a, d);
    }

    #[test]
    fn hash_differs_for_different_text() {
        let a = EmbeddingCache::hash("hello world");
        let b = EmbeddingCache::hash("hello there");
        assert_ne!(a, b);
    }

    #[test]
    fn ollama_client_normalizes_host_and_constructs_payload() {
        // Exercises the constructor path and the request body shape without making a network call.
        let client = OllamaClient::new("http://localhost:11434/", "nomic-embed-text");
        assert_eq!(client.host, "http://localhost:11434");
        assert_eq!(client.model, "nomic-embed-text");

        let body = EmbeddingRequest { model: client.model(), prompt: "hello" };
        let json = serde_json::to_string(&body).unwrap();
        assert!(json.contains("\"model\":\"nomic-embed-text\""));
        assert!(json.contains("\"prompt\":\"hello\""));
    }

    #[test]
    fn ollama_client_default_local_uses_localhost() {
        let client = OllamaClient::default_local();
        assert_eq!(client.host, "http://localhost:11434");
        assert_eq!(client.model, "nomic-embed-text");
    }

    #[test]
    fn chat_payload_shape() {
        let msgs = [
            ChatMessage { role: "system", content: "be concise" },
            ChatMessage { role: "user", content: "hello" },
        ];
        let body = ChatRequest { model: "llama3.2", messages: &msgs, stream: false };
        let json = serde_json::to_string(&body).unwrap();
        assert!(json.contains("\"model\":\"llama3.2\""));
        assert!(json.contains("\"stream\":false"));
        assert!(json.contains("\"role\":\"system\""));
        assert!(json.contains("\"content\":\"hello\""));
    }
}
