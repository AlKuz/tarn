use rmcp::{handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router};

use super::TarnMcpServer;
use super::helpers::{frontmatter_from_json, frontmatter_values_from_json};
use super::types::{
    CreateNoteParams, DeleteNoteParams, DeleteNoteResponse, GetTagsParams, GetTagsResponse,
    RenameNoteParams, RenameNoteResponse, RenderMarkdown, ReplaceInNoteParams, SearchParams,
    TagInfo, UpdateFrontmatterParams, UpdateNoteParams, WriteNoteResponse, tool_error, tool_json,
    tool_text,
};
use crate::common::VaultPath;
use crate::core::tarn_core::UpdateMode;
use crate::index::{Index, NoteResult, find_direct_children};
use crate::observer::Observer;
use crate::revisions::RevisionTracker;
use crate::storage::{FileContent, Storage};

#[tool_router(vis = "pub(crate)")]
impl<S, I, O, R> TarnMcpServer<S, I, O, R>
where
    S: Storage + Send + Sync + 'static,
    I: Index + Send + Sync + 'static,
    O: Observer + Send + Sync + 'static,
    R: RevisionTracker + Send + Sync + 'static,
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
                    let mut pairs = Vec::new();
                    for nr in &results {
                        if let Ok(file) = self.core.read(&nr.path).await
                            && let FileContent::Markdown(content) = file.content
                            && let Ok(note) = self.core.parse_content(&content)
                        {
                            pairs.push((nr, note));
                        }
                    }
                    let pair_refs: Vec<_> = pairs.iter().map(|(nr, n)| (*nr, n)).collect();
                    tool_text(RenderMarkdown::new(pair_refs).render())
                } else {
                    tool_json(&results)
                }
            }
            Err(e) => tool_error(e),
        }
    }

    #[tool(
        description = "Get tag hierarchy with usage statistics. Shows parent-child relationships."
    )]
    async fn tarn_get_tags(
        &self,
        Parameters(params): Parameters<GetTagsParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        match self.core.list_tags(params.prefix.as_deref(), None).await {
            Ok(tag_counts) => {
                let all_tags: Vec<String> = tag_counts.keys().cloned().collect();
                let tags: Vec<TagInfo> = tag_counts
                    .into_iter()
                    .map(|(tag, count)| TagInfo {
                        children: find_direct_children(&tag, &all_tags),
                        tag,
                        count,
                        notes: None,
                    })
                    .collect();
                let response = GetTagsResponse { tags };
                tool_json(&response)
            }
            Err(e) => tool_error(e),
        }
    }

    #[tool(
        description = "Create a new note. Fails if a note already exists at the path. Frontmatter is rendered to YAML automatically."
    )]
    async fn tarn_create_note(
        &self,
        Parameters(params): Parameters<CreateNoteParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let path = match VaultPath::new(&params.path) {
            Ok(p) => p,
            Err(e) => return tool_error(e),
        };

        let content = if let Some(fm_json) = params.frontmatter {
            let fm = match frontmatter_from_json(fm_json) {
                Ok(f) => f,
                Err(e) => return tool_error(format!("invalid frontmatter: {e}")),
            };
            format!("{fm}\n{}", params.content)
        } else {
            params.content
        };

        match self.core.write(&path, FileContent::Markdown(content)).await {
            Ok(_) => tool_json(&WriteNoteResponse { path: params.path }),
            Err(e) => tool_error(e),
        }
    }

    #[tool(
        description = "Update a note. In replace mode (default), overwrites content with revision check. In append mode, adds content to end without revision check."
    )]
    async fn tarn_update_note(
        &self,
        Parameters(params): Parameters<UpdateNoteParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let path = match VaultPath::new(&params.path) {
            Ok(p) => p,
            Err(e) => return tool_error(e),
        };

        match params.mode.as_deref() {
            Some("append") => {
                match self
                    .core
                    .append(&path, FileContent::Markdown(params.content))
                    .await
                {
                    Ok(_) => tool_json(&WriteNoteResponse { path: params.path }),
                    Err(e) => tool_error(e),
                }
            }
            Some("replace") | None => {
                let content = if let Some(fm_json) = params.frontmatter {
                    let fm = match frontmatter_from_json(fm_json) {
                        Ok(f) => f,
                        Err(e) => return tool_error(format!("invalid frontmatter: {e}")),
                    };
                    format!("{fm}\n{}", params.content)
                } else {
                    params.content
                };
                match self.core.write(&path, FileContent::Markdown(content)).await {
                    Ok(_) => tool_json(&WriteNoteResponse { path: params.path }),
                    Err(e) => tool_error(e),
                }
            }
            Some(other) => tool_error(format!(
                "invalid mode: {other} (expected: replace or append)"
            )),
        }
    }

    #[tool(
        description = "Replace text within a note without rewriting entire content. Supports first, all, and regex modes."
    )]
    async fn tarn_replace_in_note(
        &self,
        Parameters(params): Parameters<ReplaceInNoteParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let path = match VaultPath::new(&params.path) {
            Ok(p) => p,
            Err(e) => return tool_error(e),
        };

        match params.mode.as_deref() {
            Some("first") | None => {
                // First-match: read, replacen, write back
                let file = match self.core.read(&path).await {
                    Ok(f) => f,
                    Err(e) => return tool_error(e),
                };
                let current = match file.content {
                    FileContent::Markdown(c) => c,
                    _ => return tool_error("not a markdown file"),
                };
                let new_content = current.replacen(&params.old, &params.new, 1);
                match self
                    .core
                    .write(&path, FileContent::Markdown(new_content))
                    .await
                {
                    Ok(_) => tool_json(&WriteNoteResponse { path: params.path }),
                    Err(e) => tool_error(e),
                }
            }
            Some("all") => {
                match self
                    .core
                    .update(&path, &params.old, &params.new, UpdateMode::Text)
                    .await
                {
                    Ok(_) => tool_json(&WriteNoteResponse { path: params.path }),
                    Err(e) => tool_error(e),
                }
            }
            Some("regex") => {
                match self
                    .core
                    .update(&path, &params.old, &params.new, UpdateMode::Regex)
                    .await
                {
                    Ok(_) => tool_json(&WriteNoteResponse { path: params.path }),
                    Err(e) => tool_error(e),
                }
            }
            Some(other) => tool_error(format!(
                "invalid mode: {other} (expected: first, all, or regex)"
            )),
        }
    }

    #[tool(
        description = "Modify note frontmatter. Removes keys first, then sets values. Tags are regular YAML values — use set/remove to modify them."
    )]
    async fn tarn_update_frontmatter(
        &self,
        Parameters(params): Parameters<UpdateFrontmatterParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let path = match VaultPath::new(&params.path) {
            Ok(p) => p,
            Err(e) => return tool_error(e),
        };

        let set = if let Some(json_map) = params.set {
            match frontmatter_values_from_json(json_map) {
                Ok(v) => v,
                Err(e) => return tool_error(format!("invalid set values: {e}")),
            }
        } else {
            std::collections::HashMap::new()
        };
        let remove = params.remove.unwrap_or_default();

        match self.core.update_frontmatter(&path, set, remove).await {
            Ok(_) => tool_json(&WriteNoteResponse { path: params.path }),
            Err(e) => tool_error(e),
        }
    }

    #[tool(description = "Delete a note. Fails if the note does not exist.")]
    async fn tarn_delete_note(
        &self,
        Parameters(params): Parameters<DeleteNoteParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let path = match VaultPath::new(&params.path) {
            Ok(p) => p,
            Err(e) => return tool_error(e),
        };
        match self.core.delete(&path).await {
            Ok(()) => tool_json(&DeleteNoteResponse {
                path: params.path,
                deleted: true,
            }),
            Err(e) => tool_error(e),
        }
    }

    #[tool(
        description = "Rename or move a note. By default updates wikilinks in other notes that reference it."
    )]
    async fn tarn_rename_note(
        &self,
        Parameters(params): Parameters<RenameNoteParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let from = match VaultPath::new(&params.path) {
            Ok(p) => p,
            Err(e) => return tool_error(e),
        };
        let to = match VaultPath::new(&params.new_path) {
            Ok(p) => p,
            Err(e) => return tool_error(e),
        };

        match self.core.rename(&from, &to).await {
            Ok((_meta, links_updated)) => tool_json(&RenameNoteResponse {
                old_path: params.path,
                new_path: params.new_path,
                links_updated,
            }),
            Err(e) => tool_error(e),
        }
    }
}
