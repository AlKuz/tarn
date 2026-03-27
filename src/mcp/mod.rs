//! MCP (Model Context Protocol) server implementation for Tarn.
//!
//! This module provides an MCP server that exposes vault operations to AI agents
//! and other MCP clients. The server implements three MCP primitives:
//!
//! ## Tools
//!
//! Discover and modify notes:
//!
//! - `tarn_search_notes` - Full-text search with BM25 ranking (if index configured)
//! - `tarn_get_tags` - Get tag hierarchy with usage statistics
//! - `tarn_create_note` - Create a new note (fails if already exists)
//! - `tarn_update_note` - Update an existing note with revision-based conflict detection
//! - `tarn_replace_in_note` - Replace text within a note (first, all, or regex modes)
//!
//! ## Resources
//!
//! Read-only vault metadata exposed as URIs:
//!
//! - `tarn://vault/info` - Vault metadata (name, note count, tag count)
//! - `tarn://vault/tags` - Tag hierarchy with counts
//! - `tarn://vault/folders` - Directory tree with note counts
//! - `tarn://note/{path}` - Individual note content and metadata
//!
//! Resources support folder scoping via URI templates (e.g., `tarn://vault/info/{folder}`).
//!
//! ## Prompts
//!
//! Guided workflows for common agent tasks:
//!
//! - `tarn_explore_topic` - Deep-dive into a topic with link following
//! - `tarn_summarize_project` - Generate project status from folder notes
//!
//! ## Usage
//!
//! ```ignore
//! use std::sync::Arc;
//! use tarn::{TarnConfig, TarnMcpServer};
//! use tarn::common::Buildable;
//!
//! let core = TarnConfig::local("/path/to/vault".into()).build()?;
//!
//! let server = TarnMcpServer::new(Arc::new(core));
//! // Use with rmcp transport (stdio, HTTP, etc.)
//! ```

mod prompts;
mod resources;
mod tools;

use std::sync::Arc;

use rmcp::{
    RoleServer, ServerHandler, handler::server::router::tool::ToolRouter, model::*,
    service::RequestContext, tool_handler,
};

use crate::core::tarn_core::TarnCore;

/// MCP server exposing Tarn vault operations.
///
/// Wraps a [`TarnCore`] instance and provides MCP-compliant tools, resources,
/// and prompts for AI agent integration. The server is clone-cheap (uses `Arc`
/// internally) and can be shared across multiple transport connections.
#[derive(Clone)]
pub struct TarnMcpServer {
    core: Arc<TarnCore>,
    tool_router: ToolRouter<Self>,
}

impl TarnMcpServer {
    /// Create a new MCP server wrapping the given core.
    ///
    /// The core should be fully initialized (index rebuilt if using indexing).
    pub fn new(core: Arc<TarnCore>) -> Self {
        let tool_router = Self::tool_router();
        Self { core, tool_router }
    }
}

#[tool_handler]
impl ServerHandler for TarnMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .build(),
        )
        .with_server_info(Implementation::from_build_env())
        .with_instructions(
            "Tarn MCP server for Obsidian vault access. Use tools to search, list, and read notes. \
             Browse resources for vault structure (info, tags, folders). \
             Use prompts for guided workflows like topic exploration and project summarization."
                .to_string(),
        )
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, rmcp::ErrorData> {
        Ok(self.list_static_resources())
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, rmcp::ErrorData> {
        Ok(self.list_resource_templates_static())
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, rmcp::ErrorData> {
        self.read_resource_by_uri(&request.uri).await
    }

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, rmcp::ErrorData> {
        Ok(self.list_prompts_static())
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResult, rmcp::ErrorData> {
        self.get_prompt_by_name(&request.name, &request.arguments.unwrap_or_default())
    }
}
