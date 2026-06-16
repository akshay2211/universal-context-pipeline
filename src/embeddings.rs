use anyhow::Result;

pub struct OllamaClient {
    // TODO Week 2: reqwest::Client + host + model name.
    _private: (),
}

impl OllamaClient {
    pub fn new(_host: &str, _model: &str) -> Self {
        todo!("ollama client — add reqwest")
    }

    pub async fn embed(&self, _text: &str) -> Result<Vec<f32>> {
        // TODO Week 2: POST {host}/api/embeddings, return data["embedding"].
        todo!("embed via ollama")
    }
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
}
