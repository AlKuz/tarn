use std::collections::HashMap;

use regex::RegexBuilder;
use rmcp::{handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router};

use super::TarnMcpServer;
use super::types::{
    CreateNoteParams, GetTagsParams, GetTagsResponse, ReplaceInNoteParams, SearchParams,
    SearchResponse, SearchResult, SectionScore, TagInfo, UpdateNoteParams, WriteNoteResponse,
};
use crate::common::RevisionToken;
use crate::core::responses::ReplaceMode;
use crate::index::Index;
use crate::observer::Observer;
use crate::storage::Storage;

fn tool_success(response: &impl serde::Serialize) -> Result<CallToolResult, rmcp::ErrorData> {
    let value = serde_json::to_value(response)
        .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;
    Ok(CallToolResult::structured(value))
}

fn tool_error(e: impl std::fmt::Display) -> Result<CallToolResult, rmcp::ErrorData> {
    Ok(CallToolResult::structured_error(serde_json::json!({
        "error": e.to_string()
    })))
}

#[tool_router(vis = "pub(crate)")]
impl<S, I, O> TarnMcpServer<S, I, O>
where
    S: Storage + Send + Sync + 'static,
    I: Index + Send + Sync + 'static,
    O: Observer + Send + Sync + 'static,
{
    #[tool(
        description = "Search across the vault or list notes. When query is provided, returns notes ranked by relevance with section scores. When query is omitted, lists notes in a folder."
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

        match query {
            Some(q) if !q.text.is_empty() => {
                // Search mode: score and group by note
                match self
                    .core
                    .search(
                        &q.text,
                        &q.folders,
                        &q.tags,
                        limit * 4, // Over-fetch sections for note dedup
                        params.token_limit,
                    )
                    .await
                {
                    Ok(section_hits) => {
                        // Group sections by note
                        let mut note_groups: HashMap<String, (f32, usize, Vec<SectionScore>)> =
                            HashMap::new();

                        for hit in &section_hits {
                            let note_path = hit
                                .path
                                .note_path()
                                .map(|p| p.to_string())
                                .unwrap_or_else(|| hit.path.to_string());

                            let heading_path: Vec<String> = hit
                                .path
                                .section_headings()
                                .into_iter()
                                .map(|s| s.to_string())
                                .collect();

                            let entry = note_groups
                                .entry(note_path)
                                .or_insert_with(|| (0.0, 0, Vec::new()));

                            if hit.score > entry.0 {
                                entry.0 = hit.score;
                            }
                            entry.1 += hit.token_count;
                            entry.2.push(SectionScore {
                                heading_path,
                                score: hit.score,
                            });
                        }

                        // Sort notes by max score descending
                        let mut note_entries: Vec<_> = note_groups.into_iter().collect();
                        note_entries.sort_by(|a, b| {
                            b.1.0
                                .partial_cmp(&a.1.0)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        });
                        note_entries.truncate(limit);

                        // Apply token_limit and enrich with note metadata
                        let mut results = Vec::new();
                        let mut token_budget = params.token_limit.unwrap_or(usize::MAX);

                        for (path_str, (score, token_count, sections)) in note_entries {
                            if token_budget == 0 {
                                break;
                            }

                            let (title, tags) = match self.core.read(&path_str).await {
                                Ok((note, _)) => {
                                    let tags: Vec<String> =
                                        note.tags().into_iter().map(String::from).collect();
                                    (note.title.clone(), tags)
                                }
                                Err(_) => (None, Vec::new()),
                            };

                            let use_tokens = token_count.min(token_budget);
                            token_budget = token_budget.saturating_sub(use_tokens);

                            results.push(SearchResult {
                                path: path_str,
                                title,
                                score: Some(score),
                                tags,
                                token_count: use_tokens,
                                relevant_sections: Some(sections),
                            });
                        }

                        let response = SearchResponse {
                            total: results.len(),
                            results,
                        };
                        tool_success(&response)
                    }
                    Err(e) => tool_error(e),
                }
            }
            Some(q) => {
                // Filter-only mode: query has tags/folders but no text
                match self
                    .core
                    .search("", &q.folders, &q.tags, limit * 4, params.token_limit)
                    .await
                {
                    Ok(section_hits) => {
                        // Group sections by note (scores are uniform 1.0)
                        let mut note_groups: HashMap<String, (usize, Vec<SectionScore>)> =
                            HashMap::new();

                        for hit in &section_hits {
                            let note_path = hit
                                .path
                                .note_path()
                                .map(|p| p.to_string())
                                .unwrap_or_else(|| hit.path.to_string());

                            let heading_path: Vec<String> = hit
                                .path
                                .section_headings()
                                .into_iter()
                                .map(|s| s.to_string())
                                .collect();

                            let entry = note_groups
                                .entry(note_path)
                                .or_insert_with(|| (0, Vec::new()));
                            entry.0 += hit.token_count;
                            entry.1.push(SectionScore {
                                heading_path,
                                score: hit.score,
                            });
                        }

                        // Sort by path for deterministic order
                        let mut note_entries: Vec<_> = note_groups.into_iter().collect();
                        note_entries.sort_by(|a, b| a.0.cmp(&b.0));
                        note_entries.truncate(limit);

                        // Apply token_limit and enrich with note metadata
                        let mut results = Vec::new();
                        let mut token_budget = params.token_limit.unwrap_or(usize::MAX);

                        for (path_str, (token_count, sections)) in note_entries {
                            if token_budget == 0 {
                                break;
                            }

                            let (title, tags) = match self.core.read(&path_str).await {
                                Ok((note, _)) => {
                                    let tags: Vec<String> =
                                        note.tags().into_iter().map(String::from).collect();
                                    (note.title.clone(), tags)
                                }
                                Err(_) => (None, Vec::new()),
                            };

                            let use_tokens = token_count.min(token_budget);
                            token_budget = token_budget.saturating_sub(use_tokens);

                            results.push(SearchResult {
                                path: path_str,
                                title,
                                score: None, // No relevance score for filter-only
                                tags,
                                token_count: use_tokens,
                                relevant_sections: Some(sections),
                            });
                        }

                        let response = SearchResponse {
                            total: results.len(),
                            results,
                        };
                        tool_success(&response)
                    }
                    Err(e) => tool_error(e),
                }
            }
            None => {
                // List mode: no query, no filters
                match self.core.list(None, true).await {
                    Ok(paths) => {
                        let mut results = Vec::new();
                        let mut token_budget = params.token_limit.unwrap_or(usize::MAX);

                        for path in paths.iter().take(limit) {
                            if token_budget == 0 {
                                break;
                            }

                            let path_str = path.to_string();
                            match self.core.read(&path_str).await {
                                Ok((note, _)) => {
                                    let tags: Vec<String> =
                                        note.tags().into_iter().map(String::from).collect();
                                    let token_count = note.word_count(); // approximate

                                    let use_tokens = token_count.min(token_budget);
                                    token_budget = token_budget.saturating_sub(use_tokens);

                                    results.push(SearchResult {
                                        path: path_str,
                                        title: note.title.clone(),
                                        score: None,
                                        tags,
                                        token_count: use_tokens,
                                        relevant_sections: None,
                                    });
                                }
                                Err(_) => continue,
                            }
                        }

                        let response = SearchResponse {
                            total: results.len(),
                            results,
                        };
                        tool_success(&response)
                    }
                    Err(e) => tool_error(e),
                }
            }
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
