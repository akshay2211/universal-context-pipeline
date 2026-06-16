use super::{token_count, Chunk, ChunkSource, LineIndex};
use std::path::Path;

pub struct ProseChunker;

impl ProseChunker {
    pub fn split(
        file_path: &Path,
        text: &str,
        max_tokens: usize,
        overlap_sentences: usize,
    ) -> Vec<Chunk> {
        let line_index = LineIndex::new(text);

        let mut sentences: Vec<(usize, &str)> = Vec::new();
        let mut offset = 0;
        for sentence in text.split_inclusive(|c: char| matches!(c, '.' | '!' | '?')) {
            if !sentence.trim().is_empty() {
                sentences.push((offset, sentence));
            }
            offset += sentence.len();
        }

        let mut chunks = Vec::new();
        let mut buf: Vec<(usize, &str)> = Vec::new();
        let mut buf_tokens = 0;

        for (off, sent) in sentences {
            let st = token_count(sent);
            if buf_tokens + st > max_tokens && !buf.is_empty() {
                emit(&mut chunks, &buf, &line_index, file_path);
                let drain_until = buf.len().saturating_sub(overlap_sentences);
                buf.drain(..drain_until);
                buf_tokens = buf.iter().map(|(_, s)| token_count(s)).sum();
            }
            buf.push((off, sent));
            buf_tokens += st;
        }
        if !buf.is_empty() {
            emit(&mut chunks, &buf, &line_index, file_path);
        }
        chunks
    }
}

fn emit(
    chunks: &mut Vec<Chunk>,
    buf: &[(usize, &str)],
    line_index: &LineIndex,
    file_path: &Path,
) {
    let text: String = buf.iter().map(|(_, s)| *s).collect();
    let tokens = token_count(&text);
    let start_offset = buf[0].0;
    let (last_off, last_sent) = buf[buf.len() - 1];
    let end_offset = last_off + last_sent.len().saturating_sub(1);
    chunks.push(Chunk {
        text,
        token_count: tokens,
        source: ChunkSource {
            file_path: file_path.to_path_buf(),
            start_line: line_index.line_for(start_offset),
            end_line: line_index.line_for(end_offset),
        },
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn empty_text_yields_no_chunks() {
        let chunks = ProseChunker::split(Path::new("/test.md"), "", 100, 0);
        assert!(chunks.is_empty());
    }

    #[test]
    fn short_text_fits_in_one_chunk() {
        let text = "Line one sentence.\nLine two sentence.\nLine three sentence.";
        let chunks = ProseChunker::split(Path::new("/test.md"), text, 1000, 0);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].source.start_line, 1);
        assert_eq!(chunks[0].source.end_line, 3);
        assert_eq!(chunks[0].source.file_path, PathBuf::from("/test.md"));
    }

    #[test]
    fn splits_when_budget_exceeded() {
        let text = "one. two. three. four. five. six. seven. eight.";
        let chunks = ProseChunker::split(Path::new("/test.md"), text, 5, 1);
        assert!(chunks.len() > 1, "expected multi-chunk split, got {}", chunks.len());
        for chunk in &chunks {
            assert!(!chunk.text.trim().is_empty());
        }
    }

    #[test]
    fn overlap_carries_sentences_forward() {
        let text = "one. two. three. four. five. six.";
        let no_overlap = ProseChunker::split(Path::new("/test.md"), text, 4, 0);
        let with_overlap = ProseChunker::split(Path::new("/test.md"), text, 4, 1);
        let joined_no: String = no_overlap.iter().map(|c| c.text.clone()).collect();
        let joined_with: String = with_overlap.iter().map(|c| c.text.clone()).collect();
        assert!(joined_with.len() > joined_no.len());
    }

    #[test]
    fn tracks_line_ranges_across_multiline_sentences() {
        let text = "First sentence on line one.\nSecond sentence\nspans line two\nand three.\nThird on four.";
        let chunks = ProseChunker::split(Path::new("/x.md"), text, 1000, 0);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].source.start_line, 1);
        assert_eq!(chunks[0].source.end_line, 5);
    }

    #[test]
    fn skips_whitespace_only_sentences() {
        let text = "Real sentence here.   .   Another real one.";
        let chunks = ProseChunker::split(Path::new("/x.md"), text, 1000, 0);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].text.contains("Real sentence here"));
        assert!(chunks[0].text.contains("Another real one"));
    }
}
