use crate::ingestion::ChunkSource;
use anyhow::Result;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct MatchedChunk {
    pub text: String,
    pub source: ChunkSource,
    pub last_modified: i64,
    pub score: f32,
}

pub struct VectorStore {
    // TODO Week 2: rusqlite::Connection with sqlite-vec extension loaded,
    // plus FTS5 virtual table for BM25.
    _private: (),
}

impl VectorStore {
    pub fn open(_path: &Path) -> Result<Self> {
        // TODO Week 2: open rusqlite (bundled), load sqlite-vec, initialize schema:
        //   chunks(id, document_id, file_path, start_line, end_line, content, mtime, content_hash)
        //   vec_chunks USING vec0(chunk_id, embedding float[768])  -- nomic-embed-text dims
        //   fts_chunks USING fts5(content, content='chunks', content_rowid='id')
        todo!("open store")
    }

    pub fn insert_chunk(
        &mut self,
        _document_id: &str,
        _content: &str,
        _source: &ChunkSource,
        _embedding: &[f32],
        _content_hash: &[u8; 32],
    ) -> Result<()> {
        todo!("insert chunk + vec + fts")
    }

    /// Reciprocal-rank fusion of FTS5 BM25 and vec0 nearest-neighbor.
    pub fn hybrid_search(
        &self,
        _query: &str,
        _query_embedding: &[f32],
        _limit: usize,
        _folder_filter: Option<&Path>,
    ) -> Result<Vec<MatchedChunk>> {
        // TODO Week 2: run both queries, RRF-merge with k=60.
        todo!("hybrid search via RRF")
    }
}
