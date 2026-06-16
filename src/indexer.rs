use crate::config::PdfConfig;
use crate::embeddings::{Embedder, EmbeddingCache};
use crate::ingestion::{chunk_file, Chunk, MaskingEngine};
use crate::storage::VectorStore;
use anyhow::{Context, Result};
use std::ffi::OsStr;
use std::path::Path;
use std::time::UNIX_EPOCH;
use walkdir::{DirEntry, WalkDir};

pub struct IndexOptions {
    pub no_mask: bool,
    pub max_tokens: usize,
    pub overlap_sentences: usize,
    pub pdf: PdfConfig,
}

impl Default for IndexOptions {
    fn default() -> Self {
        Self {
            no_mask: false,
            max_tokens: 512,
            overlap_sentences: 1,
            pdf: PdfConfig::default(),
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct IndexStats {
    pub files_processed: usize,
    pub files_skipped: usize,
    pub chunks_inserted: usize,
    pub cache_hits: usize,
    pub embed_calls: usize,
}

/// Events emitted while indexing. The CLI binds these to an `indicatif`
/// progress bar; the library stays UI-agnostic. Tests pass `None`.
pub enum IndexEvent<'a> {
    /// About to start a logical unit (one file, or one conversation export).
    Start { path: &'a Path, total_units: Option<usize>, unit_number: usize },
    /// Finished a unit. `chunks_added` is what landed in the store for that unit.
    Finish { path: &'a Path, chunks_added: usize },
    /// One chunk processed (embedded or cache hit). Useful for secondary counters.
    Chunk { from_cache: bool },
}

pub type ProgressFn<'a> = dyn Fn(IndexEvent<'_>) + Send + Sync + 'a;

/// Pre-walk a path and count supported files. Used to seed the progress bar's
/// "total" count. Cheap relative to indexing itself (no reads, just stat).
pub fn count_indexable_files(root: &Path) -> usize {
    if root.is_file() {
        return if is_supported(root) { 1 } else { 0 };
    }
    WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !is_ignored_entry(e))
        .filter_map(|r| r.ok())
        .filter(|e| e.file_type().is_file() && is_supported(e.path()))
        .count()
}

pub async fn index_path<E: Embedder>(
    root: &Path,
    store: &mut VectorStore,
    embedder: &E,
    opts: &IndexOptions,
    progress: Option<&ProgressFn<'_>>,
) -> Result<IndexStats> {
    let mut stats = IndexStats::default();
    let total_units = progress.map(|_| count_indexable_files(root));

    if root.is_file() {
        if is_supported(root) {
            if let Some(cb) = progress {
                cb(IndexEvent::Start { path: root, total_units, unit_number: 1 });
            }
            let before = stats.chunks_inserted;
            index_one_file(root, store, embedder, opts, &mut stats, progress).await?;
            if let Some(cb) = progress {
                cb(IndexEvent::Finish {
                    path: root,
                    chunks_added: stats.chunks_inserted - before,
                });
            }
        } else {
            stats.files_skipped += 1;
        }
        return Ok(stats);
    }

    let walker = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !is_ignored_entry(e));

    let mut unit_number = 0usize;
    for entry in walker {
        let entry = entry.context("walking directory")?;
        if !entry.file_type().is_file() {
            continue;
        }
        if !is_supported(entry.path()) {
            stats.files_skipped += 1;
            continue;
        }
        unit_number += 1;
        if let Some(cb) = progress {
            cb(IndexEvent::Start { path: entry.path(), total_units, unit_number });
        }
        let before = stats.chunks_inserted;
        index_one_file(entry.path(), store, embedder, opts, &mut stats, progress).await?;
        if let Some(cb) = progress {
            cb(IndexEvent::Finish {
                path: entry.path(),
                chunks_added: stats.chunks_inserted - before,
            });
        }
    }
    Ok(stats)
}

pub async fn index_one_file<E: Embedder>(
    path: &Path,
    store: &mut VectorStore,
    embedder: &E,
    opts: &IndexOptions,
    stats: &mut IndexStats,
    progress: Option<&ProgressFn<'_>>,
) -> Result<()> {
    let raw = read_file_text(path, &opts.pdf)?;
    let mtime = file_mtime(path);

    let cleaned = if opts.no_mask { raw } else { MaskingEngine::clean(&raw) };
    let chunks = chunk_file(path, &cleaned, opts.max_tokens, opts.overlap_sentences);

    if chunks.is_empty() {
        tracing::warn!(
            path = %path.display(),
            extracted_bytes = cleaned.len(),
            "no chunks produced — extraction returned no usable text"
        );
    }

    // Re-index semantics: drop existing chunks for this file before inserting new ones.
    store.delete_chunks_for_path(path)?;

    embed_and_store(chunks, mtime, store, embedder, stats, progress).await?;
    stats.files_processed += 1;
    Ok(())
}

/// Push pre-built chunks (e.g. from a conversation-export ingester) through
/// the embedding cache + store. Caller is responsible for any prior cleanup
/// (e.g. delete_chunks_for_path) since chunks may share a logical "file" but
/// have no on-disk re-index semantics.
pub async fn index_chunks<E: Embedder>(
    chunks: Vec<Chunk>,
    mtime: i64,
    store: &mut VectorStore,
    embedder: &E,
    progress: Option<&ProgressFn<'_>>,
) -> Result<IndexStats> {
    let mut stats = IndexStats::default();
    embed_and_store(chunks, mtime, store, embedder, &mut stats, progress).await?;
    Ok(stats)
}

async fn embed_and_store<E: Embedder>(
    chunks: Vec<Chunk>,
    mtime: i64,
    store: &mut VectorStore,
    embedder: &E,
    stats: &mut IndexStats,
    progress: Option<&ProgressFn<'_>>,
) -> Result<()> {
    for chunk in chunks {
        let hash = EmbeddingCache::hash(&chunk.text);
        let (embedding, from_cache) = match store.find_cached_embedding(&hash)? {
            Some(e) => {
                stats.cache_hits += 1;
                (e, true)
            }
            None => {
                stats.embed_calls += 1;
                let e = embedder
                    .embed(&chunk.text)
                    .await
                    .with_context(|| {
                        format!("embedding chunk from {}", chunk.source.file_path.display())
                    })?;
                (e, false)
            }
        };
        store.insert_chunk(&chunk, &hash, &embedding, mtime)?;
        stats.chunks_inserted += 1;
        if let Some(cb) = progress {
            cb(IndexEvent::Chunk { from_cache });
        }
    }
    Ok(())
}

fn file_mtime(path: &Path) -> i64 {
    std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn read_file_text(path: &Path, pdf: &PdfConfig) -> Result<String> {
    if is_pdf(path) {
        extract_pdf_text(path, pdf)
    } else {
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))
    }
}

fn extract_pdf_text(path: &Path, pdf: &PdfConfig) -> Result<String> {
    let primary = pdf_extract::extract_text(path);
    let primary_text = primary.unwrap_or_default();
    if primary_text.trim().len() >= pdf.min_useful_bytes {
        return Ok(primary_text);
    }

    tracing::info!(
        path = %path.display(),
        primary_len = primary_text.trim().len(),
        "pdf-extract produced little/no text; trying pdftotext fallback"
    );

    match pdftotext_extract(path, &pdf.pdftotext_command) {
        Ok(text) if text.trim().len() >= pdf.min_useful_bytes => Ok(text),
        Ok(empty) => {
            tracing::warn!(
                path = %path.display(),
                pdftotext_len = empty.trim().len(),
                "pdftotext also produced little/no text — PDF may be image-only (scanned) or have a broken text layer; consider `ocrmypdf --redo-ocr <file>`"
            );
            Ok(if primary_text.is_empty() { empty } else { primary_text })
        }
        Err(e) => {
            tracing::warn!(
                path = %path.display(),
                error = %e,
                "pdftotext fallback unavailable; install poppler (`brew install poppler`) for better PDF support"
            );
            Ok(primary_text)
        }
    }
}

fn pdftotext_extract(path: &Path, command: &str) -> Result<String> {
    use std::process::Command;
    let output = Command::new(command)
        .arg("-layout")
        .arg(path)
        .arg("-") // write to stdout
        .output()
        .with_context(|| format!("spawning {command}"))?;
    if !output.status.success() {
        anyhow::bail!(
            "{command} exited {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn is_pdf(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("pdf"))
        .unwrap_or(false)
}

fn is_ignored_entry(entry: &DirEntry) -> bool {
    if entry.depth() == 0 {
        return false;
    }
    let name = entry.file_name();
    if entry.file_type().is_dir() {
        is_ignored_dir_name(name)
    } else {
        // Hide dotfiles at file level too (e.g. .DS_Store).
        name.to_str().map(|s| s.starts_with('.')).unwrap_or(false)
    }
}

fn is_ignored_dir_name(name: &OsStr) -> bool {
    matches!(
        name.to_str(),
        Some(
            ".git" | ".idea" | ".vscode" | "target" | "node_modules"
                | "__pycache__" | ".venv" | "venv" | "dist" | "build"
                | ".next" | ".nuxt" | "coverage" | ".pytest_cache" | ".mypy_cache"
        )
    )
}

pub fn is_supported(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some(
            "md" | "markdown" | "txt" | "rs" | "py" | "ts" | "tsx" | "js" | "jsx" | "mjs"
                | "go" | "pdf" | "PDF"
        )
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::DEFAULT_EMBEDDING_DIM;
    use async_trait::async_trait;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingEmbedder {
        calls: AtomicUsize,
    }

    impl CountingEmbedder {
        fn new() -> Self {
            Self { calls: AtomicUsize::new(0) }
        }
        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl Embedder for CountingEmbedder {
        async fn embed(&self, text: &str) -> Result<Vec<f32>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            // Deterministic per-text vector so distinct chunks get distinct embeddings.
            let mut v = vec![0.0f32; DEFAULT_EMBEDDING_DIM];
            let idx = (text.bytes().map(|b| b as usize).sum::<usize>()) % DEFAULT_EMBEDDING_DIM;
            v[idx] = 1.0;
            Ok(v)
        }
    }

    fn write_file(dir: &Path, rel: &str, content: &str) -> PathBuf {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
        path
    }

    #[tokio::test]
    async fn indexes_supported_files_and_skips_others() {
        let tmp = tempfile::tempdir().unwrap();
        write_file(tmp.path(), "notes.md", "# Title\nSome body text.");
        write_file(tmp.path(), "code.rs", "fn alpha() {}\nfn beta() {}\n");
        write_file(tmp.path(), "image.png", "binaryish");
        write_file(tmp.path(), "data.csv", "a,b,c");

        let mut store = VectorStore::open_in_memory().unwrap();
        let embedder = CountingEmbedder::new();
        let stats = index_path(tmp.path(), &mut store, &embedder, &IndexOptions::default(), None)
            .await
            .unwrap();

        assert_eq!(stats.files_processed, 2);
        assert!(stats.files_skipped >= 2);
        assert!(stats.chunks_inserted >= 3, "got {} chunks", stats.chunks_inserted);
        assert_eq!(stats.embed_calls, stats.chunks_inserted);
    }

    #[tokio::test]
    async fn cache_hits_skip_embedding_calls_on_reindex() {
        let tmp = tempfile::tempdir().unwrap();
        write_file(tmp.path(), "a.md", "# A\nfirst section body.");
        write_file(tmp.path(), "b.md", "# B\nsecond section body.");

        let mut store = VectorStore::open_in_memory().unwrap();
        let embedder = CountingEmbedder::new();
        let first = index_path(tmp.path(), &mut store, &embedder, &IndexOptions::default(), None)
            .await
            .unwrap();
        let calls_after_first = embedder.calls();
        assert_eq!(first.cache_hits, 0);
        assert!(calls_after_first > 0);

        let second = index_path(tmp.path(), &mut store, &embedder, &IndexOptions::default(), None)
            .await
            .unwrap();
        // Identical content → every chunk's hash should hit the cache.
        assert_eq!(second.cache_hits, second.chunks_inserted);
        assert_eq!(second.embed_calls, 0);
        assert_eq!(embedder.calls(), calls_after_first, "no new embed calls on re-index");
    }

    #[tokio::test]
    async fn ignored_directories_are_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        write_file(tmp.path(), "ok.md", "# OK\nbody.");
        write_file(tmp.path(), "node_modules/junk.md", "# junk\nbody.");
        write_file(tmp.path(), "target/debug/leftover.rs", "fn ignored() {}");
        write_file(tmp.path(), ".git/HEAD", "ref: refs/heads/main");

        let mut store = VectorStore::open_in_memory().unwrap();
        let embedder = CountingEmbedder::new();
        let stats = index_path(tmp.path(), &mut store, &embedder, &IndexOptions::default(), None)
            .await
            .unwrap();

        assert_eq!(stats.files_processed, 1);
    }

    #[tokio::test]
    async fn single_file_target_indexes_that_file() {
        let tmp = tempfile::tempdir().unwrap();
        let file = write_file(tmp.path(), "lonely.md", "# Lonely\njust me.");

        let mut store = VectorStore::open_in_memory().unwrap();
        let embedder = CountingEmbedder::new();
        let stats = index_path(&file, &mut store, &embedder, &IndexOptions::default(), None)
            .await
            .unwrap();

        assert_eq!(stats.files_processed, 1);
        assert!(stats.chunks_inserted >= 1);
    }

    #[test]
    fn is_supported_accepts_pdf_case_insensitive() {
        assert!(is_supported(Path::new("/x/doc.pdf")));
        assert!(is_supported(Path::new("/x/doc.PDF")));
    }

    #[test]
    fn is_pdf_detects_extension() {
        assert!(is_pdf(Path::new("/x/y.pdf")));
        assert!(is_pdf(Path::new("/x/y.PDF")));
        assert!(!is_pdf(Path::new("/x/y.md")));
        assert!(!is_pdf(Path::new("/x/y")));
    }
}
