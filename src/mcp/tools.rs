use regex::RegexBuilder;
use rmcp::{handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router};
use schemars::JsonSchema;

use super::TarnMcpServer;
use super::helpers::parse_folder;
use super::responses::{SearchResponse, SearchResult, WriteNoteResponse};
use crate::common::RevisionToken;
use crate::core::responses::{ReplaceMode, SearchOptions};
use crate::index::Index;
use crate::observer::Observer;
use crate::storage::Storage;

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

fn tool_success(response: &impl serde::Serialize) -> Result<CallToolResult, rmcp::ErrorData> {
    let json = serde_json::to_string_pretty(response)
        .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;
    Ok(CallToolResult::success(vec![rmcp::model::Content::text(
        json,
    )]))
}

fn tool_error(e: impl std::fmt::Display) -> Result<CallToolResult, rmcp::ErrorData> {
    Ok(CallToolResult::error(vec![rmcp::model::Content::text(
        e.to_string(),
    )]))
}

#[tool_router(vis = "pub(crate)")]
impl<S, I, O> TarnMcpServer<S, I, O>
where
    S: Storage + Send + Sync + 'static,
    I: Index + Send + Sync + 'static,
    O: Observer + Send + Sync + 'static,
{
    #[tool(
        description = "Search across the vault using full-text matching. Returns matching notes with metadata."
    )]
    async fn tarn_search_notes(
        &self,
        Parameters(params): Parameters<SearchNotesParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let folder = parse_folder(params.folder.as_deref())?;
        let limit = params.limit.unwrap_or(20);
        let offset = params.offset.unwrap_or(0);

        let options = SearchOptions {
            folder,
            tags: params.tag_filter,
            limit,
            offset,
        };

        match self.core.search(&params.query, options).await {
            Ok(core_response) => {
                let mut results = Vec::new();
                for hit in &core_response.hits {
                    let path_str = hit.path.to_string();
                    match self.core.read(&path_str).await {
                        Ok((note, _)) => {
                            let tags: Vec<String> =
                                note.tags().into_iter().map(String::from).collect();
                            results.push(SearchResult {
                                path: path_str,
                                title: note.title.clone(),
                                tags,
                            });
                        }
                        Err(_) => continue,
                    }
                }
                let response = SearchResponse {
                    total: core_response.total,
                    results,
                };
                tool_success(&response)
            }
            Err(e) => tool_error(e),
        }
    }

    #[tool(
        description = "Get tag hierarchy with usage statistics. Shows parent-child relationships and optionally lists which notes use each tag."
    )]
    async fn tarn_get_tags(
        &self,
        Parameters(params): Parameters<GetTagsParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let include_notes = params.include_notes.unwrap_or(false);

        match self.core.tags(params.prefix.as_deref(), None).await {
            Ok(entries) => {
                let tags: Vec<super::responses::TagInfo> = entries
                    .into_iter()
                    .map(|e| super::responses::TagInfo {
                        tag: e.tag,
                        count: e.count,
                        children: e.children,
                        notes: if include_notes {
                            Some(e.note_paths.into_iter().map(|p| p.to_string()).collect())
                        } else {
                            None
                        },
                    })
                    .collect();
                let response = super::responses::GetTagsResponse { tags };
                tool_success(&response)
            }
            Err(e) => tool_error(e),
        }
    }

    #[tool(description = "Create a new note. Fails if a note already exists at the path.")]
    async fn tarn_create_note(
        &self,
        Parameters(params): Parameters<CreateNoteParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        match self.core.write(&params.path, &params.content, None).await {
            Ok(revision) => {
                let response = WriteNoteResponse {
                    path: params.path,
                    revision,
                };
                tool_success(&response)
            }
            Err(e) => tool_error(e),
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
        match self
            .core
            .write(&params.path, &params.content, Some(revision))
            .await
        {
            Ok(new_revision) => {
                let response = WriteNoteResponse {
                    path: params.path,
                    revision: new_revision,
                };
                tool_success(&response)
            }
            Err(e) => tool_error(e),
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
                return tool_error(format!(
                    "invalid mode: {other} (expected: first, all, or regex)"
                ));
            }
        };

        let revision: RevisionToken = params.revision.into();

        // Read current content
        let (note, _) = match self.core.read(&params.path).await {
            Ok(result) => result,
            Err(e) => return tool_error(e),
        };

        // Apply replacement
        let current_content = note.to_string();
        let new_content = match mode {
            ReplaceMode::First => current_content.replacen(&params.old, &params.new, 1),
            ReplaceMode::All => current_content.replace(&params.old, &params.new),
            ReplaceMode::Regex => {
                match RegexBuilder::new(&params.old).size_limit(1_000_000).build() {
                    Ok(re) => re.replace_all(&current_content, &*params.new).into_owned(),
                    Err(e) => return tool_error(format!("invalid regex: {e}")),
                }
            }
        };

        // Write back with user's revision for conflict detection
        match self
            .core
            .write(&params.path, &new_content, Some(revision))
            .await
        {
            Ok(new_revision) => {
                let response = WriteNoteResponse {
                    path: params.path,
                    revision: new_revision,
                };
                tool_success(&response)
            }
            Err(e) => tool_error(e),
        }
    }
}
