use crate::embeddings::Embedder;
use crate::storage::VectorStore;
use anyhow::Result;
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

const PROTOCOL_VERSION: &str = "2024-11-05";
const SERVER_NAME: &str = "ucp";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
const TOOL_NAME: &str = "search_local_context";

pub struct McpServer<E: Embedder> {
    store: VectorStore,
    embedder: E,
}

impl<E: Embedder> McpServer<E> {
    pub fn new(store: VectorStore, embedder: E) -> Self {
        Self { store, embedder }
    }

    pub async fn run_stdio(&self) -> Result<()> {
        let stdin = BufReader::new(tokio::io::stdin());
        let mut stdout = tokio::io::stdout();
        let mut lines = stdin.lines();

        while let Some(line) = lines.next_line().await? {
            if line.trim().is_empty() {
                continue;
            }
            let request: Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(_) => {
                    let err = error_response(Value::Null, -32700, "parse error");
                    write_line(&mut stdout, &err).await?;
                    continue;
                }
            };
            if let Some(response) = self.handle_request(request).await {
                write_line(&mut stdout, &response).await?;
            }
        }
        Ok(())
    }

    /// Dispatch one JSON-RPC message. Returns `None` for notifications.
    pub async fn handle_request(&self, req: Value) -> Option<Value> {
        let id = req.get("id").cloned();
        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = req.get("params").cloned().unwrap_or(Value::Null);

        // Notifications have no id; we send no reply.
        let is_notification = id.is_none();

        match method {
            "initialize" => Some(success(id?, json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": SERVER_NAME, "version": SERVER_VERSION },
            }))),
            "initialized" | "notifications/initialized" => None,
            "ping" => Some(success(id?, json!({}))),
            "tools/list" => Some(success(id?, json!({ "tools": [self.tool_descriptor()] }))),
            "tools/call" => Some(self.handle_tools_call(id?, params).await),
            _ if is_notification => None,
            _ => Some(error_response(id.unwrap_or(Value::Null), -32601, "method not found")),
        }
    }

    fn tool_descriptor(&self) -> Value {
        json!({
            "name": TOOL_NAME,
            "description": "Search the local UCP context store. Returns relevant text chunks from your indexed files with citation (file_path:start_line-end_line). Use this to ground responses in the user's own files instead of relying on training data.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Natural-language search query." },
                    "limit": { "type": "integer", "description": "Maximum results (default 5).", "minimum": 1, "maximum": 50 },
                    "folder_filter": { "type": "string", "description": "Restrict results to a folder prefix (optional)." }
                },
                "required": ["query"]
            }
        })
    }

    async fn handle_tools_call(&self, id: Value, params: Value) -> Value {
        let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name != TOOL_NAME {
            return error_response(id, -32601, &format!("unknown tool: {name}"));
        }
        let args = params.get("arguments").cloned().unwrap_or(Value::Null);
        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) if !q.trim().is_empty() => q.to_string(),
            _ => return error_response(id, -32602, "missing or empty `query`"),
        };
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|n| n.clamp(1, 50) as usize)
            .unwrap_or(5);
        let folder_filter = args
            .get("folder_filter")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(PathBuf::from);

        let embedding = match self.embedder.embed(&query).await {
            Ok(e) => e,
            Err(e) => return error_response(id, -32603, &format!("embedder failed: {e}")),
        };
        let hits = match self.store.hybrid_search(
            &query,
            &embedding,
            limit,
            folder_filter.as_deref(),
        ) {
            Ok(h) => h,
            Err(e) => return error_response(id, -32603, &format!("search failed: {e}")),
        };

        let content: Vec<Value> = if hits.is_empty() {
            vec![json!({ "type": "text", "text": "No matching context found." })]
        } else {
            hits.iter().map(format_hit).collect()
        };
        success(id, json!({ "content": content }))
    }
}

fn format_hit(hit: &crate::storage::MatchedChunk) -> Value {
    let citation = format!(
        "[{}:{}-{}]",
        hit.source.file_path.to_string_lossy(),
        hit.source.start_line,
        hit.source.end_line,
    );
    json!({
        "type": "text",
        "text": format!("{citation}\n{}", hit.text),
    })
}

fn success(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

async fn write_line<W: AsyncWriteExt + Unpin>(out: &mut W, value: &Value) -> Result<()> {
    let s = serde_json::to_string(value)?;
    out.write_all(s.as_bytes()).await?;
    out.write_all(b"\n").await?;
    out.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingestion::{Chunk, ChunkSource};
    use crate::storage::DEFAULT_EMBEDDING_DIM;
    use async_trait::async_trait;
    use std::path::Path;

    struct StubEmbedder;

    #[async_trait]
    impl Embedder for StubEmbedder {
        async fn embed(&self, _text: &str) -> Result<Vec<f32>> {
            let mut v = vec![0.0f32; DEFAULT_EMBEDDING_DIM];
            v[0] = 1.0;
            Ok(v)
        }
    }

    fn seeded_store() -> VectorStore {
        let mut store = VectorStore::open_in_memory().unwrap();
        let chunks = [
            ("/notes/a.md", "Albus Dumbledore was the headmaster of Hogwarts."),
            ("/notes/b.md", "Harry Potter attended Hogwarts School."),
            ("/code/x.rs", "fn main() { println!(\"hi\"); }"),
        ];
        for (i, (path, text)) in chunks.iter().enumerate() {
            let chunk = Chunk {
                text: text.to_string(),
                token_count: 0,
                source: ChunkSource {
                    file_path: PathBuf::from(path),
                    start_line: i + 1,
                    end_line: i + 1,
                },
            };
            let mut emb = vec![0.0f32; DEFAULT_EMBEDDING_DIM];
            emb[i] = 1.0;
            store.insert_chunk(&chunk, &[i as u8; 32], &emb, 0).unwrap();
        }
        store
    }

    fn server() -> McpServer<StubEmbedder> {
        McpServer::new(seeded_store(), StubEmbedder)
    }

    #[tokio::test]
    async fn initialize_returns_server_info() {
        let req = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} });
        let resp = server().handle_request(req).await.unwrap();
        assert_eq!(resp["id"], 1);
        assert_eq!(resp["result"]["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(resp["result"]["serverInfo"]["name"], "ucp");
        assert!(resp["result"]["capabilities"]["tools"].is_object());
    }

    #[tokio::test]
    async fn initialized_notification_yields_no_response() {
        let req = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
        assert!(server().handle_request(req).await.is_none());
    }

    #[tokio::test]
    async fn tools_list_advertises_search_tool() {
        let req = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" });
        let resp = server().handle_request(req).await.unwrap();
        let tools = resp["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], TOOL_NAME);
        assert!(tools[0]["inputSchema"]["properties"]["query"].is_object());
    }

    #[tokio::test]
    async fn tools_call_returns_citations() {
        let req = json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": { "name": TOOL_NAME, "arguments": { "query": "Hogwarts", "limit": 2 } }
        });
        let resp = server().handle_request(req).await.unwrap();
        let content = resp["result"]["content"].as_array().unwrap();
        assert!(!content.is_empty());
        let first_text = content[0]["text"].as_str().unwrap();
        assert!(first_text.starts_with("["), "expected citation prefix, got: {first_text}");
        assert!(first_text.contains(":"), "expected line range");
    }

    #[tokio::test]
    async fn tools_call_with_folder_filter() {
        let req = json!({
            "jsonrpc": "2.0", "id": 4, "method": "tools/call",
            "params": {
                "name": TOOL_NAME,
                "arguments": { "query": "Hogwarts", "folder_filter": "/notes" }
            }
        });
        let resp = server().handle_request(req).await.unwrap();
        let content = resp["result"]["content"].as_array().unwrap();
        for item in content {
            let text = item["text"].as_str().unwrap();
            assert!(text.contains("/notes/"), "folder filter not honored: {text}");
        }
    }

    #[tokio::test]
    async fn tools_call_empty_query_errors_invalid_params() {
        let req = json!({
            "jsonrpc": "2.0", "id": 5, "method": "tools/call",
            "params": { "name": TOOL_NAME, "arguments": { "query": "  " } }
        });
        let resp = server().handle_request(req).await.unwrap();
        assert_eq!(resp["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn unknown_method_returns_method_not_found() {
        let req = json!({ "jsonrpc": "2.0", "id": 6, "method": "nonsense" });
        let resp = server().handle_request(req).await.unwrap();
        assert_eq!(resp["error"]["code"], -32601);
    }

    #[tokio::test]
    async fn unknown_tool_returns_error() {
        let req = json!({
            "jsonrpc": "2.0", "id": 7, "method": "tools/call",
            "params": { "name": "bogus", "arguments": {} }
        });
        let resp = server().handle_request(req).await.unwrap();
        assert_eq!(resp["error"]["code"], -32601);
    }

    #[tokio::test]
    async fn no_results_returns_text_explainer() {
        let server = McpServer::new(VectorStore::open_in_memory().unwrap(), StubEmbedder);
        let req = json!({
            "jsonrpc": "2.0", "id": 8, "method": "tools/call",
            "params": { "name": TOOL_NAME, "arguments": { "query": "anything" } }
        });
        let resp = server.handle_request(req).await.unwrap();
        let content = resp["result"]["content"].as_array().unwrap();
        assert_eq!(content.len(), 1);
        assert!(content[0]["text"].as_str().unwrap().contains("No matching"));
    }

    // Suppress dead_code for Path import warning if optimizer trims tests.
    #[allow(dead_code)]
    fn _path_marker() -> &'static Path {
        Path::new("/dev/null")
    }
}
