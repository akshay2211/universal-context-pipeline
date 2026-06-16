use super::{token_count, Chunk, ChunkSource, ProseChunker};
use std::path::Path;
use tree_sitter::{Language, Parser};

pub struct CodeChunker;

impl CodeChunker {
    /// Returns None when the file extension isn't tree-sitter supported, so
    /// the caller can fall back to ProseChunker.
    pub fn try_split(
        file_path: &Path,
        source: &str,
        max_tokens: usize,
        overlap_sentences: usize,
    ) -> Option<Vec<Chunk>> {
        let lang = SupportedLanguage::from_extension(file_path.extension()?.to_str()?)?;
        Some(split_with(lang, file_path, source, max_tokens, overlap_sentences))
    }
}

#[derive(Clone, Copy)]
enum SupportedLanguage {
    Rust,
    Python,
    TypeScript,
}

impl SupportedLanguage {
    fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "rs" => Some(Self::Rust),
            "py" => Some(Self::Python),
            "ts" | "tsx" | "js" | "jsx" | "mjs" => Some(Self::TypeScript),
            _ => None,
        }
    }

    fn ts_language(self) -> Language {
        match self {
            Self::Rust => tree_sitter_rust::language(),
            Self::Python => tree_sitter_python::language(),
            Self::TypeScript => tree_sitter_typescript::language_typescript(),
        }
    }

    fn item_kinds(self) -> &'static [&'static str] {
        match self {
            Self::Rust => &[
                "function_item",
                "impl_item",
                "struct_item",
                "enum_item",
                "trait_item",
                "mod_item",
                "type_item",
                "const_item",
                "static_item",
            ],
            Self::Python => &[
                "function_definition",
                "class_definition",
                "decorated_definition",
            ],
            Self::TypeScript => &[
                "function_declaration",
                "class_declaration",
                "interface_declaration",
                "type_alias_declaration",
                "enum_declaration",
                "export_statement",
                "lexical_declaration",
            ],
        }
    }
}

fn split_with(
    lang: SupportedLanguage,
    file_path: &Path,
    source: &str,
    max_tokens: usize,
    overlap_sentences: usize,
) -> Vec<Chunk> {
    let mut parser = Parser::new();
    if parser.set_language(&lang.ts_language()).is_err() {
        return ProseChunker::split(file_path, source, max_tokens, overlap_sentences);
    }
    let Some(tree) = parser.parse(source, None) else {
        return ProseChunker::split(file_path, source, max_tokens, overlap_sentences);
    };

    let kinds = lang.item_kinds();
    let root = tree.root_node();
    let mut cursor = root.walk();
    let oversize_limit = max_tokens.saturating_mul(3) / 2;
    let mut chunks = Vec::new();

    for child in root.children(&mut cursor) {
        if !kinds.contains(&child.kind()) {
            continue;
        }
        let span = &source[child.byte_range()];
        if span.trim().is_empty() {
            continue;
        }
        let tokens = token_count(span);
        let start_line = child.start_position().row + 1;
        let end_line = child.end_position().row + 1;

        if tokens > oversize_limit {
            let sub = ProseChunker::split(file_path, span, max_tokens, overlap_sentences);
            let shift = start_line.saturating_sub(1);
            for mut sc in sub {
                sc.source.start_line += shift;
                sc.source.end_line += shift;
                chunks.push(sc);
            }
        } else {
            chunks.push(Chunk {
                text: span.to_string(),
                token_count: tokens,
                source: ChunkSource {
                    file_path: file_path.to_path_buf(),
                    start_line,
                    end_line,
                },
            });
        }
    }

    if chunks.is_empty() && !source.trim().is_empty() {
        return ProseChunker::split(file_path, source, max_tokens, overlap_sentences);
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_splits_into_top_level_items() {
        let source = r#"
fn alpha() {
    println!("a");
}

struct Beta {
    field: i32,
}

impl Beta {
    fn method(&self) -> i32 { self.field }
}
"#;
        let chunks = CodeChunker::try_split(Path::new("/lib.rs"), source, 1000, 1).unwrap();
        assert_eq!(chunks.len(), 3, "expected one chunk per top-level item");
        assert!(chunks[0].text.contains("fn alpha"));
        assert!(chunks[1].text.contains("struct Beta"));
        assert!(chunks[2].text.contains("impl Beta"));
    }

    #[test]
    fn rust_tracks_line_ranges() {
        let source = "fn one() {}\nfn two() {}\nfn three() {}\n";
        let chunks = CodeChunker::try_split(Path::new("/x.rs"), source, 1000, 0).unwrap();
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].source.start_line, 1);
        assert_eq!(chunks[1].source.start_line, 2);
        assert_eq!(chunks[2].source.start_line, 3);
    }

    #[test]
    fn python_splits_classes_and_functions() {
        let source = r#"
def alpha():
    return 1

class Beta:
    def method(self):
        return 2

@decorator
def gamma():
    return 3
"#;
        let chunks = CodeChunker::try_split(Path::new("/m.py"), source, 1000, 0).unwrap();
        assert_eq!(chunks.len(), 3);
        assert!(chunks[0].text.contains("def alpha"));
        assert!(chunks[1].text.contains("class Beta"));
        assert!(chunks[2].text.contains("def gamma"));
    }

    #[test]
    fn typescript_splits_top_level_constructs() {
        let source = r#"
export function alpha(): number { return 1; }

class Beta {
    method(): number { return 2; }
}

interface Gamma {
    field: string;
}
"#;
        let chunks = CodeChunker::try_split(Path::new("/m.ts"), source, 1000, 0).unwrap();
        assert!(chunks.len() >= 3, "expected at least 3 chunks, got {}", chunks.len());
    }

    #[test]
    fn unsupported_extension_returns_none() {
        assert!(CodeChunker::try_split(Path::new("/x.cpp"), "int main() {}", 100, 0).is_none());
        assert!(CodeChunker::try_split(Path::new("/x"), "no ext", 100, 0).is_none());
    }

    #[test]
    fn oversize_function_falls_back_to_prose() {
        let mut body = String::from("fn enormous() {\n");
        for i in 0..300 {
            body.push_str(&format!("    let var_{i} = compute_value({i}).unwrap();\n"));
        }
        body.push_str("}\n");
        let chunks = CodeChunker::try_split(Path::new("/big.rs"), &body, 50, 1).unwrap();
        assert!(chunks.len() > 1, "expected oversize function to split into multiple chunks");
        let max_lines = body.lines().count();
        for chunk in &chunks {
            assert!(chunk.source.start_line >= 1);
            assert!(chunk.source.end_line <= max_lines, "chunk end_line {} > file lines {}", chunk.source.end_line, max_lines);
        }
    }

    #[test]
    fn empty_source_yields_empty_chunks() {
        let chunks = CodeChunker::try_split(Path::new("/x.rs"), "", 100, 0).unwrap();
        assert!(chunks.is_empty());
    }
}
