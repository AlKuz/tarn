use rmcp::{handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router};
use schemars::JsonSchema;

use crate::common::VaultPath;

use super::TarnMcpServer;

fn parse_folder(folder: Option<String>) -> Result<Option<VaultPath>, rmcp::ErrorData> {
    folder
        .map(|f| {
            let normalized = format!("{}/", f.trim_end_matches('/'));
            VaultPath::new(normalized)
                .map_err(|e| rmcp::ErrorData::invalid_params(e.to_string(), None))
        })
        .transpose()
}

#[derive(Debug, serde::Deserialize, JsonSchema)]
pub struct ReadNoteParams {
    #[schemars(description = "Note path (e.g. \"projects/alpha/design.md\")")]
    pub path: String,
    #[schemars(description = "Return only these section headings (fragment retrieval)")]
    pub sections: Option<Vec<String>>,
    #[schemars(description = "Include parsed frontmatter (default: true)")]
    pub include_frontmatter: Option<bool>,
    #[schemars(description = "Include extracted links (default: false)")]
    pub include_links: Option<bool>,
    #[schemars(
        description = "Return heading outline + word counts instead of full content (default: false)"
    )]
    pub summary: Option<bool>,
}

#[derive(Debug, serde::Deserialize, JsonSchema)]
pub struct SearchNotesParams {
    #[schemars(description = "Search query (case-insensitive text match)")]
    pub query: String,
    #[schemars(description = "Restrict search to folder path")]
    pub folder: Option<String>,
    #[schemars(description = "Notes must have all these tags")]
    pub tag_filter: Option<Vec<String>>,
    #[schemars(description = "Max results (default: 20)")]
    pub limit: Option<usize>,
    #[schemars(description = "Pagination offset")]
    pub offset: Option<usize>,
}

#[derive(Debug, serde::Deserialize, JsonSchema)]
pub struct ListNotesParams {
    #[schemars(description = "Folder path (default: root)")]
    pub folder: Option<String>,
    #[schemars(description = "Include subfolders (default: false)")]
    pub recursive: Option<bool>,
    #[schemars(description = "Filter by tags")]
    pub tag_filter: Option<Vec<String>>,
    #[schemars(description = "Sort order: \"title\" (default)")]
    pub sort: Option<String>,
    #[schemars(description = "Max results (default: 50)")]
    pub limit: Option<usize>,
    #[schemars(description = "Pagination offset")]
    pub offset: Option<usize>,
}

#[derive(Debug, serde::Deserialize, JsonSchema)]
pub struct GetTagsParams {
    #[schemars(description = "Filter tags by prefix (e.g. \"project/\")")]
    pub prefix: Option<String>,
    #[schemars(description = "Include list of notes per tag (default: false)")]
    pub include_notes: Option<bool>,
}

#[tool_router(vis = "pub(crate)")]
impl TarnMcpServer {
    #[tool(
        description = "Read note content with control over detail level. Supports fragment retrieval by section headings and summary mode (heading outline + word counts)."
    )]
    async fn tarn_read_note(
        &self,
        Parameters(params): Parameters<ReadNoteParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = self
            .core
            .read_note(
                &params.path,
                params.sections.as_deref(),
                params.include_frontmatter.unwrap_or(true),
                params.include_links.unwrap_or(false),
                params.summary.unwrap_or(false),
            )
            .await;

        match result {
            Ok(response) => {
                let json = serde_json::to_string_pretty(&response)
                    .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;
                Ok(CallToolResult::success(vec![rmcp::model::Content::text(
                    json,
                )]))
            }
            Err(e) => Ok(CallToolResult::error(vec![rmcp::model::Content::text(
                e.to_string(),
            )])),
        }
    }

    #[tool(
        description = "Search across the vault using case-insensitive text matching. Returns matching notes with snippets showing context around matches."
    )]
    async fn tarn_search_notes(
        &self,
        Parameters(params): Parameters<SearchNotesParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let folder = parse_folder(params.folder)?;
        let result = self
            .core
            .search_notes(
                &params.query,
                folder.as_ref(),
                params.tag_filter.as_deref(),
                params.limit.unwrap_or(20),
                params.offset.unwrap_or(0),
            )
            .await;

        match result {
            Ok(response) => {
                let json = serde_json::to_string_pretty(&response)
                    .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;
                Ok(CallToolResult::success(vec![rmcp::model::Content::text(
                    json,
                )]))
            }
            Err(e) => Ok(CallToolResult::error(vec![rmcp::model::Content::text(
                e.to_string(),
            )])),
        }
    }

    #[tool(
        description = "List notes in a folder with optional filtering by tags. Supports pagination and sorting."
    )]
    async fn tarn_list_notes(
        &self,
        Parameters(params): Parameters<ListNotesParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let folder = parse_folder(params.folder)?;
        let result = self
            .core
            .list_notes(
                folder.as_ref(),
                params.recursive.unwrap_or(false),
                params.tag_filter.as_deref(),
                params.sort.as_deref(),
                params.limit.unwrap_or(50),
                params.offset.unwrap_or(0),
            )
            .await;

        match result {
            Ok(response) => {
                let json = serde_json::to_string_pretty(&response)
                    .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;
                Ok(CallToolResult::success(vec![rmcp::model::Content::text(
                    json,
                )]))
            }
            Err(e) => Ok(CallToolResult::error(vec![rmcp::model::Content::text(
                e.to_string(),
            )])),
        }
    }

    #[tool(
        description = "Get tag hierarchy with usage statistics. Shows parent-child relationships and optionally lists which notes use each tag."
    )]
    async fn tarn_get_tags(
        &self,
        Parameters(params): Parameters<GetTagsParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = self
            .core
            .get_tags(
                params.prefix.as_deref(),
                params.include_notes.unwrap_or(false),
            )
            .await;

        match result {
            Ok(response) => {
                let json = serde_json::to_string_pretty(&response)
                    .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;
                Ok(CallToolResult::success(vec![rmcp::model::Content::text(
                    json,
                )]))
            }
            Err(e) => Ok(CallToolResult::error(vec![rmcp::model::Content::text(
                e.to_string(),
            )])),
        }
    }
}
