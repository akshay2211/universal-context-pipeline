use super::{token_count, Chunk, ChunkSource, ProseChunker};
use std::path::Path;

pub struct MarkdownChunker;

impl MarkdownChunker {
    pub fn split(
        file_path: &Path,
        text: &str,
        max_tokens: usize,
        overlap_sentences: usize,
    ) -> Vec<Chunk> {
        let sections = split_sections(text);
        let oversize_limit = max_tokens.saturating_mul(3) / 2;
        let mut chunks = Vec::new();

        for section in sections {
            let tokens = token_count(&section.text);
            if tokens == 0 {
                continue;
            }
            let line_count = section.text.lines().count().max(1);
            let end_line = section.start_line + line_count - 1;

            if tokens > oversize_limit {
                // Fall back to prose chunking for huge sections; shift its line
                // numbers from section-relative (1-based) to file-relative.
                let sub = ProseChunker::split(file_path, &section.text, max_tokens, overlap_sentences);
                let shift = section.start_line - 1;
                for mut sc in sub {
                    sc.source.start_line += shift;
                    sc.source.end_line += shift;
                    chunks.push(sc);
                }
            } else {
                chunks.push(Chunk {
                    text: section.text,
                    token_count: tokens,
                    source: ChunkSource {
                        file_path: file_path.to_path_buf(),
                        start_line: section.start_line,
                        end_line,
                    },
                });
            }
        }
        chunks
    }
}

struct Section {
    start_line: usize,
    text: String,
}

fn split_sections(text: &str) -> Vec<Section> {
    let mut sections = Vec::new();
    let mut current = String::new();
    let mut current_start = 1usize;
    let mut in_fence = false;

    for (idx, line) in text.lines().enumerate() {
        let line_num = idx + 1;
        let trimmed = line.trim_start();

        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
        }

        let is_heading = !in_fence && is_atx_heading(line);

        if is_heading && !current.trim().is_empty() {
            sections.push(Section {
                start_line: current_start,
                text: std::mem::take(&mut current),
            });
            current_start = line_num;
        } else if is_heading && current.trim().is_empty() {
            // discard blank-only preamble and start a fresh section at this heading
            current.clear();
            current_start = line_num;
        }

        current.push_str(line);
        current.push('\n');
    }

    if !current.trim().is_empty() {
        sections.push(Section { start_line: current_start, text: current });
    }
    sections
}

fn is_atx_heading(line: &str) -> bool {
    let mut chars = line.chars();
    let mut hashes = 0;
    for c in chars.by_ref() {
        if c == '#' {
            hashes += 1;
            if hashes > 6 {
                return false;
            }
        } else {
            return hashes >= 1 && hashes <= 6 && c == ' ';
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_headings_yields_one_chunk() {
        let text = "Just some prose without any headings.\nLine two of it.";
        let chunks = MarkdownChunker::split(Path::new("/x.md"), text, 1000, 0);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].source.start_line, 1);
    }

    #[test]
    fn splits_on_atx_headings() {
        let text = "# Intro\nIntro body.\n\n# Section One\nFirst section.\n\n## Sub\nSub body.\n";
        let chunks = MarkdownChunker::split(Path::new("/x.md"), text, 1000, 0);
        assert_eq!(chunks.len(), 3);
        assert!(chunks[0].text.starts_with("# Intro"));
        assert!(chunks[1].text.starts_with("# Section One"));
        assert!(chunks[2].text.starts_with("## Sub"));
    }

    #[test]
    fn tracks_section_line_ranges() {
        let text = "# A\nbody a.\n# B\nbody b.\nmore b.\n# C\nbody c.";
        let chunks = MarkdownChunker::split(Path::new("/x.md"), text, 1000, 0);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].source.start_line, 1);
        assert_eq!(chunks[0].source.end_line, 2);
        assert_eq!(chunks[1].source.start_line, 3);
        assert_eq!(chunks[1].source.end_line, 5);
        assert_eq!(chunks[2].source.start_line, 6);
    }

    #[test]
    fn preamble_before_first_heading_becomes_its_own_chunk() {
        let text = "Intro paragraph before any heading.\n\n# First\nbody.";
        let chunks = MarkdownChunker::split(Path::new("/x.md"), text, 1000, 0);
        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].text.contains("Intro paragraph"));
        assert_eq!(chunks[0].source.start_line, 1);
        assert!(chunks[1].text.starts_with("# First"));
    }

    #[test]
    fn hash_inside_code_fence_does_not_split() {
        let text = "# Real heading\nintro.\n\n```python\n# this is a comment, not a heading\nprint('hi')\n```\n\nmore prose.";
        let chunks = MarkdownChunker::split(Path::new("/x.md"), text, 1000, 0);
        assert_eq!(chunks.len(), 1, "code-fence # comment must not split sections");
    }

    #[test]
    fn oversize_section_falls_back_to_prose() {
        let mut body = String::from("# Big\n");
        for i in 0..400 {
            body.push_str(&format!("Sentence number {i}. "));
        }
        let chunks = MarkdownChunker::split(Path::new("/x.md"), &body, 50, 1);
        assert!(chunks.len() > 1, "oversize heading section must fall back to prose splits");
        // All chunks should still be inside the file's actual line range.
        let max_lines = body.lines().count();
        for chunk in &chunks {
            assert!(chunk.source.start_line >= 1);
            assert!(chunk.source.end_line <= max_lines);
        }
    }

    #[test]
    fn empty_input_yields_no_chunks() {
        let chunks = MarkdownChunker::split(Path::new("/x.md"), "", 100, 0);
        assert!(chunks.is_empty());
    }

    #[test]
    fn detects_h1_through_h6() {
        for level in 1..=6 {
            let hashes = "#".repeat(level);
            assert!(is_atx_heading(&format!("{hashes} Title")));
        }
        assert!(!is_atx_heading("####### Too many")); // 7 hashes
        assert!(!is_atx_heading("#NoSpace"));
        assert!(!is_atx_heading("  # Indented")); // ATX must start at column 0
    }
}
