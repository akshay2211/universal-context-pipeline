use super::Chunk;
use std::path::Path;

/// Ingest Claude / Cursor / ChatGPT export JSON files. Tier 1 differentiator.
pub fn ingest(_path: &Path) -> anyhow::Result<Vec<Chunk>> {
    // TODO Week 3: detect format, extract user+assistant turns with timestamps,
    // emit one chunk per turn with file_path = export path, lines = turn index.
    todo!("conversation export ingester")
}
