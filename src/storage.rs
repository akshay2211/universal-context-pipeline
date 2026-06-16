use crate::ingestion::{Chunk, ChunkSource};
use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};
use std::sync::Once;

pub const EMBEDDING_DIM: usize = 768; // nomic-embed-text

#[derive(Debug, Clone)]
pub struct MatchedChunk {
    pub text: String,
    pub source: ChunkSource,
    pub mtime: i64,
    pub score: f32,
}

pub struct VectorStore {
    conn: Connection,
}

impl VectorStore {
    pub fn open_in_memory() -> Result<Self> {
        register_vec_extension();
        let conn = Connection::open_in_memory().context("opening in-memory sqlite")?;
        let store = Self { conn };
        store.initialize_schema()?;
        Ok(store)
    }

    pub fn open(path: &Path) -> Result<Self> {
        register_vec_extension();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("creating parent dir {}", parent.display()))?;
            }
        }
        let conn = Connection::open(path).with_context(|| format!("opening {}", path.display()))?;
        let store = Self { conn };
        store.initialize_schema()?;
        Ok(store)
    }

    fn initialize_schema(&self) -> Result<()> {
        self.conn.execute_batch(&format!(
            r#"
            CREATE TABLE IF NOT EXISTS chunks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_path TEXT NOT NULL,
                start_line INTEGER NOT NULL,
                end_line INTEGER NOT NULL,
                content TEXT NOT NULL,
                content_hash BLOB NOT NULL,
                mtime INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_chunks_file_path ON chunks(file_path);
            CREATE INDEX IF NOT EXISTS idx_chunks_content_hash ON chunks(content_hash);

            -- Persistent embedding cache keyed by content hash. Survives chunk
            -- deletes so re-indexing unchanged content never re-embeds.
            CREATE TABLE IF NOT EXISTS embeddings_cache (
                content_hash BLOB PRIMARY KEY,
                embedding BLOB NOT NULL
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS vec_chunks USING vec0(
                chunk_id INTEGER PRIMARY KEY,
                embedding float[{dim}]
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS fts_chunks USING fts5(
                content,
                content='chunks',
                content_rowid='id'
            );

            CREATE TRIGGER IF NOT EXISTS chunks_ai AFTER INSERT ON chunks BEGIN
                INSERT INTO fts_chunks(rowid, content) VALUES (new.id, new.content);
            END;
            CREATE TRIGGER IF NOT EXISTS chunks_ad_fts AFTER DELETE ON chunks BEGIN
                INSERT INTO fts_chunks(fts_chunks, rowid, content) VALUES('delete', old.id, old.content);
            END;
            CREATE TRIGGER IF NOT EXISTS chunks_ad_vec AFTER DELETE ON chunks BEGIN
                DELETE FROM vec_chunks WHERE chunk_id = old.id;
            END;
            "#,
            dim = EMBEDDING_DIM
        ))
        .context("initializing schema")?;
        Ok(())
    }

    /// Insert a chunk and its embedding. The chunk's content_hash should already
    /// be set by the caller; embedding bytes are written into vec_chunks under
    /// the new chunk's rowid.
    pub fn insert_chunk(
        &mut self,
        chunk: &Chunk,
        content_hash: &[u8; 32],
        embedding: &[f32],
        mtime: i64,
    ) -> Result<i64> {
        if embedding.len() != EMBEDDING_DIM {
            anyhow::bail!(
                "embedding dimension mismatch: got {}, expected {}",
                embedding.len(),
                EMBEDDING_DIM
            );
        }
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO chunks (file_path, start_line, end_line, content, content_hash, mtime)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                chunk.source.file_path.to_string_lossy(),
                chunk.source.start_line as i64,
                chunk.source.end_line as i64,
                chunk.text,
                content_hash.as_slice(),
                mtime,
            ],
        )?;
        let chunk_id = tx.last_insert_rowid();
        let bytes: &[u8] = bytemuck::cast_slice(embedding);
        tx.execute(
            "INSERT INTO vec_chunks (chunk_id, embedding) VALUES (?1, ?2)",
            params![chunk_id, bytes],
        )?;
        // Persist into the long-lived embedding cache so future re-indexes of
        // the same content skip the Ollama call even after this file is removed.
        tx.execute(
            "INSERT OR IGNORE INTO embeddings_cache (content_hash, embedding) VALUES (?1, ?2)",
            params![content_hash.as_slice(), bytes],
        )?;
        tx.commit()?;
        Ok(chunk_id)
    }

    /// Look up a cached embedding by content hash. Used to skip Ollama calls on
    /// re-index of unchanged content.
    pub fn find_cached_embedding(&self, content_hash: &[u8; 32]) -> Result<Option<Vec<f32>>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT embedding FROM embeddings_cache WHERE content_hash = ?1")?;
        let maybe_bytes: Option<Vec<u8>> = stmt
            .query_row(params![content_hash.as_slice()], |row| row.get(0))
            .ok();
        Ok(maybe_bytes.map(|b| bytemuck::cast_slice::<u8, f32>(&b).to_vec()))
    }

    /// Delete every chunk for a file path. Returns the number deleted.
    pub fn delete_chunks_for_path(&mut self, file_path: &Path) -> Result<usize> {
        let n = self.conn.execute(
            "DELETE FROM chunks WHERE file_path = ?1",
            params![file_path.to_string_lossy()],
        )?;
        Ok(n)
    }

    pub fn chunk_count(&self) -> Result<i64> {
        Ok(self.conn.query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get(0))?)
    }

    /// Hybrid retrieval: BM25 + ANN merged via reciprocal-rank fusion (k=60).
    pub fn hybrid_search(
        &self,
        query: &str,
        query_embedding: &[f32],
        limit: usize,
        folder_filter: Option<&Path>,
    ) -> Result<Vec<MatchedChunk>> {
        if query_embedding.len() != EMBEDDING_DIM {
            anyhow::bail!(
                "query embedding dimension mismatch: got {}, expected {}",
                query_embedding.len(),
                EMBEDDING_DIM
            );
        }
        let fetch = (limit * 4).max(20);

        let bm25_ids = self.bm25_search(query, fetch, folder_filter)?;
        let vec_ids = self.vec_search(query_embedding, fetch, folder_filter)?;

        let merged = rrf_merge(&bm25_ids, &vec_ids, 60.0, limit);
        self.hydrate_chunks(&merged)
    }

    fn bm25_search(
        &self,
        query: &str,
        limit: usize,
        folder_filter: Option<&Path>,
    ) -> Result<Vec<i64>> {
        let escaped = sanitize_fts_query(query);
        if escaped.trim().is_empty() {
            return Ok(Vec::new());
        }
        let mut sql = String::from(
            "SELECT c.id FROM fts_chunks f JOIN chunks c ON c.id = f.rowid
             WHERE fts_chunks MATCH ?1",
        );
        if folder_filter.is_some() {
            sql.push_str(" AND c.file_path LIKE ?2");
        }
        sql.push_str(" ORDER BY f.rank LIMIT ?");
        sql.push_str(if folder_filter.is_some() { "3" } else { "2" });

        let mut stmt = self.conn.prepare(&sql)?;
        let limit_i = limit as i64;
        let rows: Vec<i64> = if let Some(folder) = folder_filter {
            let prefix = folder_prefix(folder);
            stmt.query_map(params![&escaped, prefix, limit_i], |r| r.get::<_, i64>(0))?
                .collect::<rusqlite::Result<_>>()?
        } else {
            stmt.query_map(params![&escaped, limit_i], |r| r.get::<_, i64>(0))?
                .collect::<rusqlite::Result<_>>()?
        };
        Ok(rows)
    }

    fn vec_search(
        &self,
        query_embedding: &[f32],
        limit: usize,
        folder_filter: Option<&Path>,
    ) -> Result<Vec<i64>> {
        let bytes: &[u8] = bytemuck::cast_slice(query_embedding);
        let limit_i = limit as i64;
        let rows: Vec<i64> = if let Some(folder) = folder_filter {
            let prefix = folder_prefix(folder);
            let mut stmt = self.conn.prepare(
                "SELECT v.chunk_id FROM vec_chunks v
                 JOIN chunks c ON c.id = v.chunk_id
                 WHERE v.embedding MATCH ?1 AND k = ?2 AND c.file_path LIKE ?3
                 ORDER BY distance ASC",
            )?;
            stmt.query_map(params![bytes, limit_i, prefix], |r| r.get::<_, i64>(0))?
                .collect::<rusqlite::Result<_>>()?
        } else {
            let mut stmt = self.conn.prepare(
                "SELECT chunk_id FROM vec_chunks
                 WHERE embedding MATCH ?1 AND k = ?2
                 ORDER BY distance ASC",
            )?;
            stmt.query_map(params![bytes, limit_i], |r| r.get::<_, i64>(0))?
                .collect::<rusqlite::Result<_>>()?
        };
        Ok(rows)
    }

    fn hydrate_chunks(&self, scored: &[(i64, f32)]) -> Result<Vec<MatchedChunk>> {
        let mut out = Vec::with_capacity(scored.len());
        let mut stmt = self.conn.prepare_cached(
            "SELECT file_path, start_line, end_line, content, mtime FROM chunks WHERE id = ?1",
        )?;
        for &(id, score) in scored {
            let row = stmt.query_row(params![id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, i64>(4)?,
                ))
            });
            if let Ok((file_path, start_line, end_line, content, mtime)) = row {
                out.push(MatchedChunk {
                    text: content,
                    source: ChunkSource {
                        file_path: PathBuf::from(file_path),
                        start_line: start_line as usize,
                        end_line: end_line as usize,
                    },
                    mtime,
                    score,
                });
            }
        }
        Ok(out)
    }
}

fn register_vec_extension() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| unsafe {
        // sqlite_vec::sqlite3_vec_init is declared as `unsafe extern "C" fn()`
        // but the real C signature matches sqlite3_auto_extension's expectation.
        // Erase the fn item through a *const () to bypass Rust's signature check.
        let raw = sqlite_vec::sqlite3_vec_init as *const ();
        rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(raw)));
    });
}

fn folder_prefix(folder: &Path) -> String {
    let mut s = folder.to_string_lossy().into_owned();
    if !s.ends_with('/') {
        s.push('/');
    }
    s.push('%');
    s
}

/// FTS5 has a query syntax with operators (AND, OR, NOT, NEAR, ", :, *, etc).
/// For v0.1 we treat the query as a bag of words: tokenize on whitespace and
/// quote each token to disable operators.
fn sanitize_fts_query(query: &str) -> String {
    query
        .split_whitespace()
        .filter(|tok| !tok.is_empty())
        .map(|tok| {
            let escaped = tok.replace('"', "\"\"");
            format!("\"{escaped}\"")
        })
        .collect::<Vec<_>>()
        .join(" OR ")
}

fn rrf_merge(bm25: &[i64], vec: &[i64], k: f32, limit: usize) -> Vec<(i64, f32)> {
    use std::collections::HashMap;
    let mut scores: HashMap<i64, f32> = HashMap::new();
    for (rank, id) in bm25.iter().enumerate() {
        *scores.entry(*id).or_insert(0.0) += 1.0 / (k + (rank as f32 + 1.0));
    }
    for (rank, id) in vec.iter().enumerate() {
        *scores.entry(*id).or_insert(0.0) += 1.0 / (k + (rank as f32 + 1.0));
    }
    let mut sorted: Vec<(i64, f32)> = scores.into_iter().collect();
    sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    sorted.truncate(limit);
    sorted
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_embedding(seed: usize) -> Vec<f32> {
        let mut v = vec![0.0f32; EMBEDDING_DIM];
        // Place a strong positive at index `seed` so vectors are linearly separable.
        v[seed % EMBEDDING_DIM] = 1.0;
        v
    }

    fn make_chunk(file: &str, start: usize, end: usize, text: &str) -> Chunk {
        Chunk {
            text: text.to_string(),
            token_count: 0,
            source: ChunkSource {
                file_path: PathBuf::from(file),
                start_line: start,
                end_line: end,
            },
        }
    }

    #[test]
    fn opens_in_memory_with_schema() {
        let store = VectorStore::open_in_memory().expect("open");
        assert_eq!(store.chunk_count().unwrap(), 0);
    }

    #[test]
    fn insert_and_count_chunks() {
        let mut store = VectorStore::open_in_memory().unwrap();
        let chunk = make_chunk("/notes/a.md", 1, 5, "Hello world content");
        let hash = [1u8; 32];
        let emb = mock_embedding(7);
        let id = store.insert_chunk(&chunk, &hash, &emb, 1_700_000_000).unwrap();
        assert!(id > 0);
        assert_eq!(store.chunk_count().unwrap(), 1);
    }

    #[test]
    fn cached_embedding_lookup() {
        let mut store = VectorStore::open_in_memory().unwrap();
        let hash = [42u8; 32];
        let emb = mock_embedding(11);
        store
            .insert_chunk(&make_chunk("/a.md", 1, 1, "x"), &hash, &emb, 0)
            .unwrap();

        let hit = store.find_cached_embedding(&hash).unwrap();
        assert!(hit.is_some());
        let cached = hit.unwrap();
        assert_eq!(cached.len(), EMBEDDING_DIM);
        assert_eq!(cached[11], 1.0);

        let miss = store.find_cached_embedding(&[0u8; 32]).unwrap();
        assert!(miss.is_none());
    }

    #[test]
    fn delete_chunks_for_path_removes_all_artifacts() {
        let mut store = VectorStore::open_in_memory().unwrap();
        store
            .insert_chunk(
                &make_chunk("/a.md", 1, 1, "aaa"),
                &[1u8; 32],
                &mock_embedding(1),
                0,
            )
            .unwrap();
        store
            .insert_chunk(
                &make_chunk("/a.md", 2, 2, "bbb"),
                &[2u8; 32],
                &mock_embedding(2),
                0,
            )
            .unwrap();
        store
            .insert_chunk(
                &make_chunk("/b.md", 1, 1, "ccc"),
                &[3u8; 32],
                &mock_embedding(3),
                0,
            )
            .unwrap();

        let deleted = store.delete_chunks_for_path(Path::new("/a.md")).unwrap();
        assert_eq!(deleted, 2);
        assert_eq!(store.chunk_count().unwrap(), 1);

        // Verify vec_chunks rows for deleted chunks are gone (trigger cleanup).
        let remaining_vec_rows: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM vec_chunks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(remaining_vec_rows, 1);
    }

    #[test]
    fn rejects_wrong_dimension_embedding() {
        let mut store = VectorStore::open_in_memory().unwrap();
        let err = store
            .insert_chunk(
                &make_chunk("/a.md", 1, 1, "x"),
                &[0u8; 32],
                &vec![0.0; 32],
                0,
            )
            .unwrap_err();
        assert!(err.to_string().contains("dimension mismatch"));
    }

    #[test]
    fn hybrid_search_returns_relevant_chunks() {
        let mut store = VectorStore::open_in_memory().unwrap();
        let chunks = [
            ("/a.md", "Albus Dumbledore was the headmaster of Hogwarts."),
            ("/a.md", "Harry Potter attended Hogwarts School."),
            ("/b.md", "Rust is a systems programming language."),
            ("/c.md", "Pizza recipes from southern Italy."),
        ];
        for (i, (path, text)) in chunks.iter().enumerate() {
            store
                .insert_chunk(
                    &make_chunk(path, 1, 1, text),
                    &[i as u8; 32],
                    &mock_embedding(i),
                    1_700_000_000 + i as i64,
                )
                .unwrap();
        }

        let q_emb = mock_embedding(0); // closest to chunk 0
        let hits = store.hybrid_search("Hogwarts", &q_emb, 2, None).unwrap();
        assert!(!hits.is_empty());
        let top = &hits[0];
        assert!(top.text.contains("Hogwarts"));
        assert!(top.mtime >= 1_700_000_000);
    }

    #[test]
    fn folder_filter_constrains_results() {
        let mut store = VectorStore::open_in_memory().unwrap();
        for (i, path) in ["/notes/a.md", "/notes/b.md", "/code/x.rs"].iter().enumerate() {
            store
                .insert_chunk(
                    &make_chunk(path, 1, 1, "shared keyword here"),
                    &[i as u8; 32],
                    &mock_embedding(i),
                    0,
                )
                .unwrap();
        }
        let q = mock_embedding(0);
        let hits = store
            .hybrid_search("keyword", &q, 10, Some(Path::new("/notes")))
            .unwrap();
        assert!(!hits.is_empty());
        for h in &hits {
            assert!(h.source.file_path.to_string_lossy().starts_with("/notes/"));
        }
    }

    #[test]
    fn rrf_merge_prefers_documents_in_both_rankings() {
        let bm25 = vec![1i64, 2, 3];
        let vec = vec![3i64, 1, 2];
        let merged = rrf_merge(&bm25, &vec, 60.0, 3);
        // Doc 1: 1/61 + 1/62
        // Doc 2: 1/62 + 1/63
        // Doc 3: 1/63 + 1/61
        // Doc 1 wins (highest sum)
        assert_eq!(merged[0].0, 1);
    }

    #[test]
    fn sanitize_fts_query_escapes_operators() {
        let q = sanitize_fts_query("hello AND world\"thing");
        assert!(q.contains("\"hello\""));
        assert!(q.contains("\"AND\"")); // operator neutralized by quoting
        assert!(q.contains("\"world\"\"thing\"")); // embedded quote escaped
    }
}
