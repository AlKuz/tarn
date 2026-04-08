use regex::RegexBuilder;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content},
    tool, tool_router,
};

use super::TarnMcpServer;
use super::types::{
    CreateNoteParams, GetTagsParams, GetTagsResponse, RenderMarkdown, ReplaceInNoteParams,
    SearchParams, TagInfo, UpdateNoteParams, WriteNoteResponse,
};
use crate::common::RevisionToken;
use crate::core::responses::ReplaceMode;
use crate::index::{Index, NoteResult};
use crate::observer::Observer;
use crate::storage::Storage;

fn tool_json(
    response: &(impl serde::Serialize + ?Sized),
) -> Result<CallToolResult, rmcp::ErrorData> {
    let value = serde_json::to_value(response)
        .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;
    Ok(CallToolResult::structured(value))
}

fn tool_text(text: String) -> Result<CallToolResult, rmcp::ErrorData> {
    Ok(CallToolResult::success(vec![Content::text(text)]))
}

fn tool_error(e: impl std::fmt::Display) -> Result<CallToolResult, rmcp::ErrorData> {
    Ok(CallToolResult::error(vec![Content::text(e.to_string())]))
}

#[tool_router(vis = "pub(crate)")]
impl<S, I, O> TarnMcpServer<S, I, O>
where
    S: Storage + Send + Sync + 'static,
    I: Index + Send + Sync + 'static,
    O: Observer + Send + Sync + 'static,
{
    #[tool(
        description = "Search across the vault. Returns notes ranked by relevance with section scores. Supports text search, tag filters (tag:name), and folder filters (folder:path). Set rendered=true to get markdown content instead of JSON."
    )]
    async fn tarn_search_notes(
        &self,
        Parameters(params): Parameters<SearchParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let limit = params.limit.unwrap_or(20);

        // Normalize: whitespace-only query with no filters becomes None
        let query = params
            .query
            .filter(|q| !q.text.trim().is_empty() || !q.tags.is_empty() || !q.folders.is_empty());

        let Some(q) = query else {
            // No query provided — return empty results
            return if params.rendered {
                tool_text(String::new())
            } else {
                tool_json(&Vec::<NoteResult>::new())
            };
        };

        match self
            .core
            .search(
                &q.text,
                &q.folders,
                &q.tags,
                limit,
                params.token_limit,
                params.score_threshold,
            )
            .await
        {
            Ok(mut results) => {
                // Filter-only mode: scores are meaningless without a text query
                if q.text.is_empty() {
                    for nr in &mut results {
                        for s in &mut nr.sections {
                            s.score = None;
                        }
                    }
                }
                if params.rendered {
                    let mut loaded = Vec::new();
                    for nr in &results {
                        if let Ok((note, _)) = self.core.read(&nr.path.to_string()).await {
                            loaded.push(note);
                        }
                    }
                    tool_text(RenderMarkdown::new(&results, &loaded).render())
                } else {
                    tool_json(&results)
                }
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
                let tags: Vec<TagInfo> = entries
                    .into_iter()
                    .map(|e| TagInfo {
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
                let response = GetTagsResponse { tags };
                tool_json(&response)
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
                tool_json(&response)
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
                tool_json(&response)
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
                tool_json(&response)
            }
            Err(e) => tool_error(e),
        }
    }
}
