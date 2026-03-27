use rmcp::{handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router};
use schemars::JsonSchema;

use super::TarnMcpServer;
use crate::common::{RevisionToken, VaultPath};
use crate::core::responses::ReplaceMode;

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
pub struct GetTagsParams {
    #[schemars(description = "Filter tags by prefix (e.g. \"project/\")")]
    pub prefix: Option<String>,
    #[schemars(description = "Include list of notes per tag (default: false)")]
    pub include_notes: Option<bool>,
}

#[derive(Debug, serde::Deserialize, JsonSchema)]
pub struct CreateNoteParams {
    #[schemars(description = "Note path (e.g. \"projects/alpha/design.md\")")]
    pub path: String,
    #[schemars(description = "Markdown content for the new note")]
    pub content: String,
}

#[derive(Debug, serde::Deserialize, JsonSchema)]
pub struct UpdateNoteParams {
    #[schemars(description = "Note path (e.g. \"projects/alpha/design.md\")")]
    pub path: String,
    #[schemars(description = "New markdown content for the note")]
    pub content: String,
    #[schemars(description = "Revision token from a prior read for conflict detection")]
    pub revision: String,
}

#[derive(Debug, serde::Deserialize, JsonSchema)]
pub struct ReplaceInNoteParams {
    #[schemars(description = "Note path (e.g. \"projects/alpha/design.md\")")]
    pub path: String,
    #[schemars(description = "Text or regex pattern to find")]
    pub old: String,
    #[schemars(description = "Replacement text")]
    pub new: String,
    #[schemars(description = "Replacement mode: \"first\" (default), \"all\", or \"regex\"")]
    pub mode: Option<String>,
    #[schemars(description = "Revision token from a prior read for conflict detection")]
    pub revision: String,
}

#[tool_router(vis = "pub(crate)")]
impl TarnMcpServer {
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

    #[tool(description = "Create a new note. Fails if a note already exists at the path.")]
    async fn tarn_create_note(
        &self,
        Parameters(params): Parameters<CreateNoteParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = self.core.create_note(&params.path, &params.content).await;

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
        description = "Update an existing note. Requires revision token from a prior read for conflict detection."
    )]
    async fn tarn_update_note(
        &self,
        Parameters(params): Parameters<UpdateNoteParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let revision: RevisionToken = params.revision.into();
        let result = self
            .core
            .update_note(&params.path, &params.content, revision)
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
        description = "Replace text within a note without rewriting entire content. Supports first, all, and regex modes."
    )]
    async fn tarn_replace_in_note(
        &self,
        Parameters(params): Parameters<ReplaceInNoteParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let mode = match params.mode.as_deref() {
            Some("all") => ReplaceMode::All,
            Some("regex") => ReplaceMode::Regex,
            Some("first") | None => ReplaceMode::First,
            Some(other) => {
                return Ok(CallToolResult::error(vec![rmcp::model::Content::text(
                    format!("invalid mode: {other} (expected: first, all, or regex)"),
                )]));
            }
        };

        let revision: RevisionToken = params.revision.into();
        let result = self
            .core
            .replace_in_note(&params.path, &params.old, &params.new, mode, revision)
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
