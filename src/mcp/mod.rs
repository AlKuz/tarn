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

pub mod helpers;
mod prompts;
mod resources;
mod tools;
pub mod types;

use std::sync::Arc;

use rmcp::{
    RoleServer, ServerHandler,
    handler::server::router::{prompt::PromptRouter, tool::ToolRouter},
    model::*,
    prompt_handler,
    service::RequestContext,
    tool_handler,
};

use self::types::{McpResult, mcp_not_found};
use crate::core::tarn_core::TarnCore;
use crate::index::Index;
use crate::observer::Observer;
use crate::revisions::RevisionTracker;
use crate::storage::Storage;

/// MCP server exposing Tarn vault operations.
///
/// Wraps a [`TarnCore`] instance and provides MCP-compliant tools, resources,
/// and prompts for AI agent integration. The server is clone-cheap (uses `Arc`
/// internally) and can be shared across multiple transport connections.
#[derive(Clone)]
pub struct TarnMcpServer<S, I, O, R>
where
    S: Storage + Send + Sync + 'static,
    I: Index + Send + Sync + 'static,
    O: Observer + Send + Sync + 'static,
    R: RevisionTracker + Send + Sync + 'static,
{
    core: Arc<TarnCore<S, I, O, R>>,
    tool_router: ToolRouter<Self>,
    prompt_router: PromptRouter<Self>,
}

impl<S, I, O, R> TarnMcpServer<S, I, O, R>
where
    S: Storage + Send + Sync + 'static,
    I: Index + Send + Sync + 'static,
    O: Observer + Send + Sync + 'static,
    R: RevisionTracker + Send + Sync + 'static,
{
    /// Create a new MCP server wrapping the given core.
    ///
    /// The core should be fully initialized (index rebuilt if using indexing).
    pub fn new(core: Arc<TarnCore<S, I, O, R>>) -> Self {
        let tool_router = Self::tool_router();
        let prompt_router = Self::prompt_router();
        Self {
            core,
            tool_router,
            prompt_router,
        }
    }
}

#[tool_handler]
#[prompt_handler]
impl<S, I, O, R> ServerHandler for TarnMcpServer<S, I, O, R>
where
    S: Storage + Send + Sync + 'static,
    I: Index + Send + Sync + 'static,
    O: Observer + Send + Sync + 'static,
    R: RevisionTracker + Send + Sync + 'static,
{
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
    ) -> McpResult<ListResourcesResult> {
        Ok(ListResourcesResult {
            resources: vec![
                RawResource::new("tarn://vault/info", "Vault Info")
                    .with_description("Vault metadata: name, note count, tag count, storage type")
                    .with_mime_type("application/json")
                    .no_annotation(),
                RawResource::new("tarn://vault/tags", "Vault Tags")
                    .with_description("Tag hierarchy with counts across the vault")
                    .with_mime_type("application/json")
                    .no_annotation(),
                RawResource::new("tarn://vault/folders", "Vault Folders")
                    .with_description("Directory tree structure with note counts")
                    .with_mime_type("application/json")
                    .no_annotation(),
            ],
            next_cursor: None,
            meta: None,
        })
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> McpResult<ListResourceTemplatesResult> {
        Ok(ListResourceTemplatesResult {
            resource_templates: vec![
                RawResourceTemplate::new("tarn://vault/info/{folder}", "Vault Info (folder)")
                    .with_description("Vault metadata scoped to a folder subtree")
                    .with_mime_type("application/json")
                    .no_annotation(),
                RawResourceTemplate::new("tarn://vault/tags/{folder}", "Vault Tags (folder)")
                    .with_description("Tag hierarchy scoped to a folder subtree")
                    .with_mime_type("application/json")
                    .no_annotation(),
                RawResourceTemplate::new("tarn://vault/folders/{folder}", "Vault Folders (folder)")
                    .with_description("Directory tree scoped to a folder subtree")
                    .with_mime_type("application/json")
                    .no_annotation(),
                RawResourceTemplate::new("tarn://note/{path}", "Note")
                    .with_description("Individual note content and metadata")
                    .with_mime_type("application/json")
                    .no_annotation(),
                RawResourceTemplate::new("tarn://note/{path}#{section_path}", "Note Section")
                    .with_description("Section content by heading path (e.g. Architecture/Backend)")
                    .with_mime_type("application/json")
                    .no_annotation(),
            ],
            next_cursor: None,
            meta: None,
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> McpResult<ReadResourceResult> {
        let uri = &request.uri;

        let rest = uri
            .strip_prefix("tarn://")
            .ok_or_else(|| mcp_not_found(format!("unknown resource: {uri}")))?;

        let segments: Vec<&str> = rest.splitn(3, '/').collect();

        match segments.as_slice() {
            ["vault", "info"] => self.read_vault_info(uri, None).await,
            ["vault", "info", folder] => self.read_vault_info(uri, Some(folder)).await,
            ["vault", "tags"] => self.read_vault_tags(uri, None).await,
            ["vault", "tags", folder] => self.read_vault_tags(uri, Some(folder)).await,
            ["vault", "folders"] => self.read_vault_folders(uri, None).await,
            ["vault", "folders", folder] => self.read_vault_folders(uri, Some(folder)).await,
            ["note", rest @ ..] => {
                let path = rest.join("/");
                if let Some((note_path, section_path)) = path.split_once('#') {
                    self.read_section_resource(uri, note_path, section_path)
                        .await
                } else {
                    self.read_note_resource(uri, &path).await
                }
            }
            _ => Err(mcp_not_found(format!("unknown resource: {uri}"))),
        }
    }
}
