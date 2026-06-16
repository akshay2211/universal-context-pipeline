use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tiktoken_rs::CoreBPE;

pub mod code;
pub mod conversation_export;
pub mod markdown;
pub mod masking;
pub mod prose;

pub use code::CodeChunker;
pub use markdown::MarkdownChunker;
pub use masking::MaskingEngine;
pub use prose::ProseChunker;

#[derive(Debug, Clone)]
pub struct Chunk {
    pub text: String,
    pub token_count: usize,
    pub source: ChunkSource,
}

#[derive(Debug, Clone)]
pub struct ChunkSource {
    pub file_path: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
}

/// Pick a chunker based on file extension. Caller decides whether to
/// pre-mask via `MaskingEngine::clean` before calling this.
pub fn chunk_file(
    file_path: &Path,
    source: &str,
    max_tokens: usize,
    overlap_sentences: usize,
) -> Vec<Chunk> {
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    match ext {
        "md" | "markdown" => {
            MarkdownChunker::split(file_path, source, max_tokens, overlap_sentences)
        }
        "rs" | "py" | "ts" | "tsx" | "js" | "jsx" | "mjs" => {
            CodeChunker::try_split(file_path, source, max_tokens, overlap_sentences)
                .unwrap_or_else(|| ProseChunker::split(file_path, source, max_tokens, overlap_sentences))
        }
        _ => ProseChunker::split(file_path, source, max_tokens, overlap_sentences),
    }
}

pub(crate) fn bpe() -> &'static CoreBPE {
    static BPE: OnceLock<CoreBPE> = OnceLock::new();
    BPE.get_or_init(|| tiktoken_rs::cl100k_base().expect("cl100k_base bundled with tiktoken-rs"))
}

pub(crate) fn token_count(text: &str) -> usize {
    bpe().encode_with_special_tokens(text).len()
}

pub(crate) struct LineIndex {
    newline_offsets: Vec<usize>,
}

impl LineIndex {
    pub(crate) fn new(text: &str) -> Self {
        let newline_offsets = text
            .bytes()
            .enumerate()
            .filter_map(|(i, b)| (b == b'\n').then_some(i))
            .collect();
        Self { newline_offsets }
    }

    pub(crate) fn line_for(&self, byte_offset: usize) -> usize {
        self.newline_offsets.partition_point(|&n| n < byte_offset) + 1
    }
}

#[cfg(test)]
mod dispatcher_tests {
    use super::*;

    #[test]
    fn markdown_extension_uses_markdown_chunker() {
        let text = "# A\nbody.\n# B\nmore body.";
        let chunks = chunk_file(Path::new("/x.md"), text, 1000, 0);
        assert_eq!(chunks.len(), 2, "markdown dispatch should produce per-heading chunks");
    }

    #[test]
    fn rust_extension_uses_code_chunker() {
        let text = "fn one() {}\nfn two() {}\n";
        let chunks = chunk_file(Path::new("/x.rs"), text, 1000, 0);
        assert_eq!(chunks.len(), 2, "rust dispatch should produce one chunk per fn");
    }

    #[test]
    fn python_extension_uses_code_chunker() {
        let chunks = chunk_file(Path::new("/x.py"), "def a():\n    pass\n\ndef b():\n    pass\n", 1000, 0);
        assert_eq!(chunks.len(), 2);
    }

    #[test]
    fn unknown_extension_falls_through_to_prose() {
        let chunks = chunk_file(Path::new("/notes.txt"), "Plain prose. Two sentences.", 1000, 0);
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn no_extension_falls_through_to_prose() {
        let chunks = chunk_file(Path::new("/CHANGELOG"), "Stuff happened.", 1000, 0);
        assert_eq!(chunks.len(), 1);
    }
}
