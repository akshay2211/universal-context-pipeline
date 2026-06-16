use anyhow::Result;

/// MCP server exposing one tool: `search_local_context`.
///
/// NOTE: `rmcp` macro surface (`#[tool_router]`, `#[tool(description = ...)]`)
/// must be validated against the actual published crate version before relying
/// on the spec's attribute syntax. See CLAUDE.md.
pub struct McpServer;

impl McpServer {
    pub async fn run_stdio() -> Result<()> {
        // TODO Week 2:
        //   - wire rmcp server over stdio
        //   - register tool `search_local_context(query: String, limit: usize, folder_filter: Option<String>)`
        //   - tool body: embed query via OllamaClient, call VectorStore::hybrid_search,
        //     return JSON content blocks with text + citation (file_path:start-end)
        todo!("mcp stdio server")
    }
}
