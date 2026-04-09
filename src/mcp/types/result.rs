//! Unified MCP result helpers for tools and resources.

use rmcp::model::{CallToolResult, Content, ReadResourceResult, ResourceContents};

/// Result type alias for MCP handler methods.
pub type McpResult<T> = Result<T, rmcp::ErrorData>;

// --- Error constructors ---

pub fn mcp_err(e: impl std::fmt::Display) -> rmcp::ErrorData {
    rmcp::ErrorData::internal_error(e.to_string(), None)
}

pub fn mcp_not_found(msg: impl std::fmt::Display) -> rmcp::ErrorData {
    rmcp::ErrorData::resource_not_found(msg.to_string(), None)
}

pub fn mcp_invalid_params(msg: impl std::fmt::Display) -> rmcp::ErrorData {
    rmcp::ErrorData::invalid_params(msg.to_string(), None)
}

// --- Tool success builders ---

pub fn tool_json(
    response: &(impl serde::Serialize + ?Sized),
) -> Result<CallToolResult, rmcp::ErrorData> {
    let value = serde_json::to_value(response).map_err(mcp_err)?;
    Ok(CallToolResult::structured(value))
}

pub fn tool_text(text: String) -> Result<CallToolResult, rmcp::ErrorData> {
    Ok(CallToolResult::success(vec![Content::text(text)]))
}

pub fn tool_error(e: impl std::fmt::Display) -> Result<CallToolResult, rmcp::ErrorData> {
    Ok(CallToolResult::error(vec![Content::text(e.to_string())]))
}

// --- Resource success builders ---

pub fn resource_json(
    uri: &str,
    value: &impl serde::Serialize,
) -> Result<ReadResourceResult, rmcp::ErrorData> {
    let json = serde_json::to_string_pretty(value).map_err(mcp_err)?;
    Ok(ReadResourceResult::new(vec![
        ResourceContents::text(json, uri).with_mime_type("application/json"),
    ]))
}
