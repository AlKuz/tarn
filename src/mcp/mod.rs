mod tools;
mod resources;
mod prompts;

use std::sync::Arc;

use rmcp::{
    ServerHandler,
    handler::server::router::tool::ToolRouter,
    model::*,
    service::RequestContext,
    tool_handler,
    RoleServer,
};

use crate::core::builder::TarnCore;

#[derive(Clone)]
pub struct TarnMcpServer {
    core: Arc<TarnCore>,
    tool_router: ToolRouter<Self>,
}

impl TarnMcpServer {
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
        self.get_prompt_by_name(
            &request.name,
            &request.arguments.unwrap_or_default(),
        )
    }
}
