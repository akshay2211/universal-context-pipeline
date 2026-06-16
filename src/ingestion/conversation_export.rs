use super::{token_count, Chunk, ChunkSource};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Ingest a Claude `conversations.json` export. Each user/assistant turn
/// becomes one chunk, prefixed with the conversation title and role so the
/// retrieved text carries provenance even out of context.
///
/// Cursor and ChatGPT exports are v0.2 — their shapes differ enough to
/// warrant separate parsers.
pub fn ingest_claude_export(path: &Path) -> Result<Vec<Chunk>> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;
    let conversations: Vec<ClaudeConversation> = serde_json::from_str(&raw)
        .context("parsing Claude conversations.json — expected an array at the root")?;

    let file_path: PathBuf = path.to_path_buf();
    let mut chunks = Vec::new();
    let mut turn_counter = 1usize;

    for convo in conversations {
        let title = convo.name.as_deref().unwrap_or("(untitled)");
        for msg in convo.chat_messages {
            let body = message_text(&msg);
            if body.trim().is_empty() {
                continue;
            }
            let role = match msg.sender.as_str() {
                "human" => "user",
                other => other,
            };
            let text = format!("[Claude conversation · {title} · {role}]\n{body}");
            let tokens = token_count(&text);
            chunks.push(Chunk {
                text,
                token_count: tokens,
                source: ChunkSource {
                    file_path: file_path.clone(),
                    start_line: turn_counter,
                    end_line: turn_counter,
                },
            });
            turn_counter += 1;
        }
    }
    Ok(chunks)
}

fn message_text(msg: &ClaudeMessage) -> String {
    if let Some(t) = &msg.text {
        if !t.trim().is_empty() {
            return t.clone();
        }
    }
    if let Some(parts) = &msg.content {
        return parts
            .iter()
            .filter_map(|c| c.text.as_deref())
            .collect::<Vec<_>>()
            .join("\n");
    }
    String::new()
}

#[derive(Deserialize)]
struct ClaudeConversation {
    name: Option<String>,
    #[serde(default)]
    chat_messages: Vec<ClaudeMessage>,
}

#[derive(Deserialize)]
struct ClaudeMessage {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    content: Option<Vec<ClaudeContent>>,
    #[serde(default)]
    sender: String,
}

#[derive(Deserialize)]
struct ClaudeContent {
    #[serde(default)]
    text: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_fixture(json: &str) -> tempfile::NamedTempFile {
        let mut tmp = tempfile::Builder::new()
            .suffix(".json")
            .tempfile()
            .unwrap();
        use std::io::Write;
        tmp.write_all(json.as_bytes()).unwrap();
        tmp.flush().unwrap();
        tmp
    }

    #[test]
    fn parses_simple_two_turn_conversation() {
        let fx = write_fixture(
            r#"[
                {
                    "name": "Rust traits",
                    "chat_messages": [
                        {"sender": "human", "text": "What is a trait in Rust?"},
                        {"sender": "assistant", "text": "A trait is like an interface — a set of method signatures other types can implement."}
                    ]
                }
            ]"#,
        );
        let chunks = ingest_claude_export(fx.path()).unwrap();
        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].text.contains("Rust traits"));
        assert!(chunks[0].text.contains("user"));
        assert!(chunks[0].text.contains("What is a trait"));
        assert!(chunks[1].text.contains("assistant"));
        assert_eq!(chunks[0].source.start_line, 1);
        assert_eq!(chunks[1].source.start_line, 2);
    }

    #[test]
    fn skips_blank_messages() {
        let fx = write_fixture(
            r#"[
                {
                    "name": "x",
                    "chat_messages": [
                        {"sender": "human", "text": "real one"},
                        {"sender": "assistant", "text": ""},
                        {"sender": "assistant", "text": "   "}
                    ]
                }
            ]"#,
        );
        let chunks = ingest_claude_export(fx.path()).unwrap();
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn falls_back_to_content_parts_when_text_missing() {
        let fx = write_fixture(
            r#"[
                {
                    "name": "structured",
                    "chat_messages": [
                        {
                            "sender": "assistant",
                            "content": [
                                {"text": "First piece."},
                                {"text": "Second piece."}
                            ]
                        }
                    ]
                }
            ]"#,
        );
        let chunks = ingest_claude_export(fx.path()).unwrap();
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].text.contains("First piece."));
        assert!(chunks[0].text.contains("Second piece."));
    }

    #[test]
    fn multiple_conversations_increment_turn_counter_globally() {
        let fx = write_fixture(
            r#"[
                {"name": "A", "chat_messages": [{"sender": "human", "text": "a1"}, {"sender": "assistant", "text": "a2"}]},
                {"name": "B", "chat_messages": [{"sender": "human", "text": "b1"}]}
            ]"#,
        );
        let chunks = ingest_claude_export(fx.path()).unwrap();
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].source.start_line, 1);
        assert_eq!(chunks[1].source.start_line, 2);
        assert_eq!(chunks[2].source.start_line, 3);
        assert!(chunks[2].text.contains("B"));
    }

    #[test]
    fn untitled_conversation_uses_placeholder() {
        let fx = write_fixture(
            r#"[
                {"chat_messages": [{"sender": "human", "text": "hi"}]}
            ]"#,
        );
        let chunks = ingest_claude_export(fx.path()).unwrap();
        assert!(chunks[0].text.contains("(untitled)"));
    }

    #[test]
    fn bad_root_shape_is_descriptive_error() {
        let fx = write_fixture(r#"{"not": "an array"}"#);
        let err = ingest_claude_export(fx.path()).unwrap_err();
        assert!(err.to_string().contains("array at the root"));
    }

    #[test]
    fn human_sender_is_renamed_to_user_for_consistency() {
        let fx = write_fixture(
            r#"[{"name": "x", "chat_messages": [{"sender": "human", "text": "hi"}]}]"#,
        );
        let chunks = ingest_claude_export(fx.path()).unwrap();
        assert!(chunks[0].text.contains("· user"));
        assert!(!chunks[0].text.contains("· human"));
    }
}
