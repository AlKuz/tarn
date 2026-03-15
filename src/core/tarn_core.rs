use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use serde::Serialize;
use thiserror::Error;
use tokio::task::JoinHandle;
use tokio_stream::StreamExt;
use tracing::{info, warn};

use crate::common::{RevisionToken, VaultPath};
use crate::core::builder::TarnCore;
use crate::index::{InMemoryIndex, Index, IndexError, SearchParams, SectionEntry};
use crate::note::{Frontmatter, Link, Note};
use crate::observer::{LocalStorageObserver, Observer, ObserverError, StorageEvent};
use crate::storage::{FileContent, Storage, StorageError};

#[derive(Debug, Error)]
pub enum CoreError {
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error(transparent)]
    Index(#[from] IndexError),
    #[error(transparent)]
    Observer(#[from] ObserverError),
    #[error("note not found: {0}")]
    NoteNotFound(VaultPath),
    #[error("not a markdown file: {0}")]
    NotMarkdown(VaultPath),
    #[error("index not configured")]
    IndexNotConfigured,
}

// --- Response types ---

#[derive(Debug, Serialize)]
pub struct SectionSummary {
    pub heading: String,
    pub level: u8,
    pub word_count: usize,
}

#[derive(Debug, Serialize)]
pub struct LinkInfo {
    #[serde(rename = "type")]
    pub link_type: String,
    pub target: String,
    pub display: String,
}

#[derive(Debug, Serialize)]
pub struct ReadNoteResponse {
    pub path: String,
    pub title: Option<String>,
    pub revision: RevisionToken,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontmatter: Option<Frontmatter>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sections: Option<Vec<SectionSummary>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<Vec<LinkInfo>>,
}

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub path: String,
    pub title: Option<String>,
    pub snippet: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct NoteListEntry {
    pub path: String,
    pub title: Option<String>,
    pub tags: Vec<String>,
    pub word_count: usize,
}

#[derive(Debug, Serialize)]
pub struct ListNotesResponse {
    pub notes: Vec<NoteListEntry>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct TagInfo {
    pub tag: String,
    pub count: usize,
    pub children: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct GetTagsResponse {
    pub tags: Vec<TagInfo>,
}

#[derive(Debug, Serialize)]
pub struct VaultInfo {
    pub name: String,
    pub root_path: PathBuf,
    pub folder: Option<VaultPath>,
    pub note_count: usize,
    pub tag_count: usize,
    pub storage_type: String,
}

#[derive(Debug, Serialize)]
pub struct VaultTagInfo {
    pub tag: String,
    pub count: usize,
    pub children: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct VaultTagsResponse {
    pub folder: Option<VaultPath>,
    pub tags: Vec<VaultTagInfo>,
}

#[derive(Debug, Serialize)]
pub struct FolderInfo {
    pub path: VaultPath,
    pub note_count: usize,
}

#[derive(Debug, Serialize)]
pub struct VaultFoldersResponse {
    pub folder: Option<VaultPath>,
    pub folders: Vec<FolderInfo>,
}

#[derive(Debug, Serialize)]
pub struct NoteResourceResponse {
    pub path: String,
    pub title: Option<String>,
    pub revision: RevisionToken,
    pub frontmatter: Frontmatter,
    pub content: String,
    pub word_count: usize,
    pub tags: Vec<String>,
}

// --- Helper functions ---

fn is_in_folder(path: &VaultPath, folder: Option<&VaultPath>) -> bool {
    match folder {
        None => true,
        Some(f) => path.is_under_folder(f),
    }
}

fn in_folder_non_recursive(path: &VaultPath, folder: Option<&VaultPath>) -> bool {
    match folder {
        None => path.parent().is_none(),
        Some(f) => path.is_in_folder(f),
    }
}

fn link_to_info(link: &Link) -> LinkInfo {
    match link {
        Link::Wiki(w) => LinkInfo {
            link_type: "wiki".into(),
            target: w.target.clone(),
            display: link.to_string(),
        },
        Link::Markdown(m) => LinkInfo {
            link_type: "markdown".into(),
            target: m.url.clone(),
            display: link.to_string(),
        },
        Link::Url(u) => LinkInfo {
            link_type: "url".into(),
            target: u.url.clone(),
            display: link.to_string(),
        },
        Link::Email(e) => LinkInfo {
            link_type: "email".into(),
            target: e.address.clone(),
            display: link.to_string(),
        },
    }
}

fn find_direct_children(parent: &str, all_tags: &[String]) -> Vec<String> {
    all_tags
        .iter()
        .filter(|other| {
            other.starts_with(parent)
                && other.len() > parent.len()
                && other.as_bytes().get(parent.len()) == Some(&b'/')
                && !other[parent.len() + 1..].contains('/')
        })
        .cloned()
        .collect()
}

/// Helper for aggregating section data into note-level data.
#[derive(Default)]
struct NoteAggregate {
    title: Option<String>,
    tags: HashSet<String>,
    word_count: usize,
}

fn extract_snippet(content: &str, query: &str, context_chars: usize) -> String {
    let lower_content = content.to_lowercase();
    let lower_query = query.to_lowercase();

    if let Some(pos) = lower_content.find(&lower_query) {
        let start = content[..pos]
            .rfind(char::is_whitespace)
            .map(|i| i + 1)
            .unwrap_or(pos.saturating_sub(context_chars));
        let end_pos = pos + query.len();
        let end = content[end_pos..]
            .find(char::is_whitespace)
            .map(|i| end_pos + i)
            .unwrap_or((end_pos + context_chars).min(content.len()));

        let prefix = if start > 0 { "..." } else { "" };
        let suffix = if end < content.len() { "..." } else { "" };
        format!("{prefix}{}{suffix}", &content[start..end])
    } else {
        content.chars().take(100).collect::<String>()
    }
}

// --- TarnCore implementation ---

impl TarnCore {
    async fn collect_md_files(
        &self,
        folder: Option<&VaultPath>,
    ) -> Result<Vec<VaultPath>, CoreError> {
        let stream = self.storage.list().await?;
        tokio::pin!(stream);

        let mut files = Vec::new();
        while let Some(path) = stream.next().await {
            if path.is_note() && is_in_folder(&path, folder) {
                files.push(path);
            }
        }
        Ok(files)
    }

    async fn read_and_parse(&self, path: &VaultPath) -> Result<(Note, RevisionToken), CoreError> {
        let file = self.storage.read(path).await?;
        match file {
            FileContent::Markdown { content, token } => {
                let mut note = Note::from(content.as_str());
                note.path = Some(path.clone());
                Ok((note, token))
            }
            FileContent::Image { .. } => Err(CoreError::NotMarkdown(path.clone())),
        }
    }

    /// Rebuild the index from all notes in the vault.
    ///
    /// This clears the existing index and re-indexes all markdown files.
    /// No-op if index is not configured.
    pub async fn rebuild_index(&self) -> Result<(), CoreError> {
        let Some(index) = &self.index else {
            return Ok(());
        };

        index.clear().await?;

        let files = self.collect_md_files(None).await?;
        let mut notes = Vec::new();

        for file_path in &files {
            match self.read_and_parse(file_path).await {
                Ok((note, _)) => notes.push(note),
                Err(e) => {
                    warn!(path = %file_path, error = %e, "skipping note during index rebuild");
                }
            }
        }

        index.update_bulk(&notes).await?;
        Ok(())
    }

    /// Start background index synchronization.
    ///
    /// Spawns a task that watches for file changes and updates the index.
    /// Returns a handle to the background task.
    ///
    /// # Errors
    ///
    /// Returns `CoreError::IndexNotConfigured` if no index is configured.
    pub fn start_index_sync(&self) -> Result<JoinHandle<()>, CoreError> {
        let index = self.index.clone().ok_or(CoreError::IndexNotConfigured)?;

        let vault_path = self.vault_path.clone();

        let handle = tokio::spawn(async move {
            let observer = LocalStorageObserver::new(vault_path.clone());
            let storage = crate::storage::local::LocalStorage::new(vault_path);

            let stream = match observer.observe().await {
                Ok(s) => s,
                Err(e) => {
                    warn!(error = %e, "failed to start file watcher");
                    return;
                }
            };
            tokio::pin!(stream);

            while let Some(event) = stream.next().await {
                match event {
                    StorageEvent::Created { path, .. } | StorageEvent::Updated { path, .. } => {
                        if !path.is_note() {
                            continue;
                        }

                        match storage.read(&path).await {
                            Ok(FileContent::Markdown { content, .. }) => {
                                let mut note = Note::from(content.as_str());
                                note.path = Some(path.clone());

                                if let Err(e) = index.update(&note).await {
                                    warn!(path = %path, error = %e, "failed to update index");
                                } else {
                                    info!(path = %path, "indexed note");
                                }
                            }
                            Ok(FileContent::Image { .. }) => {
                                // Skip images
                            }
                            Err(e) => {
                                warn!(path = %path, error = %e, "failed to read note for indexing");
                            }
                        }
                    }
                    StorageEvent::Deleted { path } => {
                        if !path.is_note() {
                            continue;
                        }

                        if let Err(e) = index.remove(&path).await {
                            warn!(path = %path, error = %e, "failed to remove from index");
                        } else {
                            info!(path = %path, "removed note from index");
                        }
                    }
                }
            }
        });

        Ok(handle)
    }

    /// Aggregate sections into notes for list operations.
    fn aggregate_sections_to_notes(sections: &[SectionEntry]) -> HashMap<VaultPath, NoteAggregate> {
        let mut aggregates: HashMap<VaultPath, NoteAggregate> = HashMap::new();

        for section in sections {
            let entry = aggregates.entry(section.note_path.clone()).or_default();

            // Title comes from first heading (root section or first H1)
            if entry.title.is_none() && !section.heading_path.is_empty() {
                entry.title = Some(section.heading_path[0].clone());
            }

            entry.tags.extend(section.tags.iter().cloned());
            entry.word_count += section.word_count;
        }

        aggregates
    }

    /// Search using the index.
    async fn search_notes_indexed(
        &self,
        index: &InMemoryIndex,
        query: &str,
        folder: Option<&VaultPath>,
        tag_filter: Option<&[String]>,
        limit: usize,
        offset: usize,
    ) -> Result<SearchResponse, CoreError> {
        let params = SearchParams {
            folder: folder.cloned(),
            tags: tag_filter.map(|t| t.to_vec()),
            limit: limit + offset, // Get extra for offset
            offset: 0,
        };

        let search_results = index.search(query, params).await?;

        // Deduplicate by note path (index returns sections, API returns notes)
        let mut seen_paths = HashSet::new();
        let mut results = Vec::new();

        for (section, _score) in search_results {
            if !seen_paths.insert(section.note_path.clone()) {
                continue;
            }

            // Read note content to generate snippet
            let (note, _) = match self.read_and_parse(&section.note_path).await {
                Ok(result) => result,
                Err(_) => continue,
            };

            let full_text = note.to_string();
            let snippet = extract_snippet(&full_text, query, 50);

            results.push(SearchResult {
                path: section.note_path.to_string(),
                title: note.title.clone(),
                snippet,
                tags: section.tags,
            });
        }

        let total = results.len();
        let results: Vec<SearchResult> = results.into_iter().skip(offset).take(limit).collect();

        Ok(SearchResponse { results, total })
    }

    /// List notes using the index.
    #[allow(clippy::too_many_arguments)]
    async fn list_notes_indexed(
        &self,
        index: &InMemoryIndex,
        folder: Option<&VaultPath>,
        recursive: bool,
        tag_filter: Option<&[String]>,
        sort: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<ListNotesResponse, CoreError> {
        let sections = index.list(folder, recursive).await?;
        let aggregates = Self::aggregate_sections_to_notes(&sections);

        let mut entries: Vec<NoteListEntry> = aggregates
            .into_iter()
            .filter(|(_, agg)| {
                if let Some(filters) = tag_filter {
                    filters.iter().all(|f| agg.tags.contains(f))
                } else {
                    true
                }
            })
            .map(|(path, agg)| NoteListEntry {
                path: path.to_string(),
                title: agg.title,
                tags: agg.tags.into_iter().collect(),
                word_count: agg.word_count,
            })
            .collect();

        match sort {
            Some("title") | None => {
                entries.sort_by(|a, b| a.title.cmp(&b.title));
            }
            _ => {}
        }

        let total = entries.len();
        let notes: Vec<NoteListEntry> = entries.into_iter().skip(offset).take(limit).collect();

        Ok(ListNotesResponse { notes, total })
    }

    /// Get tags using the index.
    async fn get_tags_indexed(
        &self,
        index: &InMemoryIndex,
        prefix: Option<&str>,
        include_notes: bool,
    ) -> Result<GetTagsResponse, CoreError> {
        let sections = index.list(None, true).await?;
        let mut tag_map: HashMap<String, (usize, Vec<String>)> = HashMap::new();

        for section in &sections {
            for tag in &section.tags {
                let entry = tag_map
                    .entry(tag.clone())
                    .or_insert_with(|| (0, Vec::new()));
                entry.0 += 1;
                let note_path = section.note_path.to_string();
                if !entry.1.contains(&note_path) {
                    entry.1.push(note_path);
                }
            }
        }

        let mut tags: Vec<TagInfo> = tag_map
            .into_iter()
            .filter(|(tag, _)| prefix.is_none_or(|p| tag.starts_with(p)))
            .map(|(tag, (count, note_paths))| TagInfo {
                tag,
                count,
                children: Vec::new(),
                notes: if include_notes {
                    Some(note_paths)
                } else {
                    None
                },
            })
            .collect();

        let all_tags: Vec<String> = tags.iter().map(|t| t.tag.clone()).collect();
        for tag_info in &mut tags {
            tag_info.children = find_direct_children(&tag_info.tag, &all_tags);
        }

        tags.sort_by(|a, b| a.tag.cmp(&b.tag));

        Ok(GetTagsResponse { tags })
    }

    /// Get vault info using the index.
    async fn vault_info_indexed(
        &self,
        index: &InMemoryIndex,
        folder: Option<&VaultPath>,
    ) -> Result<VaultInfo, CoreError> {
        let meta = index.meta().await?;
        let sections = index.list(folder, true).await?;

        let mut all_tags = HashSet::new();
        let mut note_paths = HashSet::new();

        for section in &sections {
            note_paths.insert(&section.note_path);
            all_tags.extend(section.tags.iter().cloned());
        }

        let name = self
            .vault_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "vault".to_string());

        Ok(VaultInfo {
            name,
            root_path: self.vault_path.clone(),
            folder: folder.cloned(),
            note_count: if folder.is_some() {
                note_paths.len()
            } else {
                meta.note_count
            },
            tag_count: all_tags.len(),
            storage_type: "local".to_string(),
        })
    }

    /// Get vault tags using the index.
    async fn vault_tags_indexed(
        &self,
        index: &InMemoryIndex,
        folder: Option<&VaultPath>,
    ) -> Result<VaultTagsResponse, CoreError> {
        let sections = index.list(folder, true).await?;
        let mut tag_counts: HashMap<String, usize> = HashMap::new();

        for section in &sections {
            for tag in &section.tags {
                *tag_counts.entry(tag.clone()).or_default() += 1;
            }
        }

        let all_tags: Vec<String> = tag_counts.keys().cloned().collect();
        let mut tags: Vec<VaultTagInfo> = tag_counts
            .into_iter()
            .map(|(tag, count)| {
                let children = find_direct_children(&tag, &all_tags);
                VaultTagInfo {
                    tag,
                    count,
                    children,
                }
            })
            .collect();

        tags.sort_by(|a, b| a.tag.cmp(&b.tag));

        Ok(VaultTagsResponse {
            folder: folder.cloned(),
            tags,
        })
    }

    pub async fn read_note(
        &self,
        path: &str,
        sections: Option<&[String]>,
        include_frontmatter: bool,
        include_links: bool,
        summary: bool,
    ) -> Result<ReadNoteResponse, CoreError> {
        let file_path: VaultPath = path.try_into().map_err(StorageError::from)?;
        let (note, revision) = self.read_and_parse(&file_path).await?;

        let section_summaries: Vec<SectionSummary> = note
            .sections
            .iter()
            .filter_map(|s| {
                s.heading.as_ref().map(|h| SectionSummary {
                    heading: h.text.clone(),
                    level: h.level,
                    word_count: s.word_count,
                })
            })
            .collect();

        let content = if summary {
            None
        } else if let Some(requested) = sections {
            let requested_lower: Vec<String> = requested.iter().map(|s| s.to_lowercase()).collect();
            let mut filtered = String::new();
            for section in &note.sections {
                if let Some(h) = &section.heading
                    && requested_lower.contains(&h.text.to_lowercase())
                {
                    for _ in 0..h.level {
                        filtered.push('#');
                    }
                    filtered.push(' ');
                    filtered.push_str(&h.text);
                    filtered.push('\n');
                    filtered.push_str(&section.content);
                }
            }
            Some(filtered)
        } else {
            Some(note.to_string())
        };

        let links = if include_links {
            Some(note.links().iter().map(|l| link_to_info(l)).collect())
        } else {
            None
        };

        let frontmatter = if include_frontmatter {
            Some(note.frontmatter.clone())
        } else {
            None
        };

        Ok(ReadNoteResponse {
            path: path.to_string(),
            title: note.title.clone(),
            revision,
            frontmatter,
            content,
            sections: if summary || sections.is_some() {
                Some(section_summaries)
            } else {
                None
            },
            links,
        })
    }

    pub async fn search_notes(
        &self,
        query: &str,
        folder: Option<&VaultPath>,
        tag_filter: Option<&[String]>,
        limit: usize,
        offset: usize,
    ) -> Result<SearchResponse, CoreError> {
        // Use index if available
        if let Some(index) = &self.index {
            return self
                .search_notes_indexed(index.as_ref(), query, folder, tag_filter, limit, offset)
                .await;
        }

        // Fall back to full-scan
        let files = self.collect_md_files(folder).await?;
        let lower_query = query.to_lowercase();
        let mut results = Vec::new();

        for file_path in &files {
            let (note, _token) = match self.read_and_parse(file_path).await {
                Ok(result) => result,
                Err(e) => {
                    warn!(path = %file_path, error = %e, "skipping note in search");
                    continue;
                }
            };

            let tags: Vec<String> = note.tags().into_iter().map(String::from).collect();

            if let Some(filters) = tag_filter
                && !filters.iter().all(|f| tags.contains(f))
            {
                continue;
            }

            let full_text = note.to_string();
            if !full_text.to_lowercase().contains(&lower_query) {
                continue;
            }

            let snippet = extract_snippet(&full_text, query, 50);

            results.push(SearchResult {
                path: file_path.to_string(),
                title: note.title.clone(),
                snippet,
                tags,
            });
        }

        let total = results.len();
        let results: Vec<SearchResult> = results.into_iter().skip(offset).take(limit).collect();

        Ok(SearchResponse { results, total })
    }

    pub async fn list_notes(
        &self,
        folder: Option<&VaultPath>,
        recursive: bool,
        tag_filter: Option<&[String]>,
        sort: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<ListNotesResponse, CoreError> {
        // Use index if available
        if let Some(index) = &self.index {
            return self
                .list_notes_indexed(
                    index.as_ref(),
                    folder,
                    recursive,
                    tag_filter,
                    sort,
                    limit,
                    offset,
                )
                .await;
        }

        // Fall back to full-scan
        let stream = self.storage.list().await?;
        tokio::pin!(stream);

        let mut files = Vec::new();
        while let Some(path) = stream.next().await {
            if !path.is_note() {
                continue;
            }
            if recursive {
                if !is_in_folder(&path, folder) {
                    continue;
                }
            } else if !in_folder_non_recursive(&path, folder) {
                continue;
            }
            files.push(path);
        }

        let mut entries = Vec::new();
        for file_path in &files {
            let (note, _token) = match self.read_and_parse(file_path).await {
                Ok(result) => result,
                Err(e) => {
                    warn!(path = %file_path, error = %e, "skipping note in list");
                    continue;
                }
            };

            let tags: Vec<String> = note.tags().into_iter().map(String::from).collect();

            if let Some(filters) = tag_filter
                && !filters.iter().all(|f| tags.contains(f))
            {
                continue;
            }

            entries.push(NoteListEntry {
                path: file_path.to_string(),
                title: note.title.clone(),
                tags,
                word_count: note.word_count(),
            });
        }

        match sort {
            Some("title") | None => {
                entries.sort_by(|a, b| a.title.cmp(&b.title));
            }
            _ => {}
        }

        let total = entries.len();
        let notes: Vec<NoteListEntry> = entries.into_iter().skip(offset).take(limit).collect();

        Ok(ListNotesResponse { notes, total })
    }

    pub async fn get_tags(
        &self,
        prefix: Option<&str>,
        include_notes: bool,
    ) -> Result<GetTagsResponse, CoreError> {
        // Use index if available
        if let Some(index) = &self.index {
            return self
                .get_tags_indexed(index.as_ref(), prefix, include_notes)
                .await;
        }

        // Fall back to full-scan
        let files = self.collect_md_files(None).await?;
        let mut tag_map: HashMap<String, (usize, Vec<String>)> = HashMap::new();

        for file_path in &files {
            let (note, _token) = match self.read_and_parse(file_path).await {
                Ok(result) => result,
                Err(e) => {
                    warn!(path = %file_path, error = %e, "skipping note in get_tags");
                    continue;
                }
            };

            let note_path = file_path.to_string();
            for tag in note.tags() {
                let entry = tag_map
                    .entry(tag.to_string())
                    .or_insert_with(|| (0, Vec::new()));
                entry.0 += 1;
                entry.1.push(note_path.clone());
            }
        }

        let mut tags: Vec<TagInfo> = tag_map
            .into_iter()
            .filter(|(tag, _)| prefix.is_none_or(|p| tag.starts_with(p)))
            .map(|(tag, (count, note_paths))| {
                let children: Vec<String> = Vec::new();
                TagInfo {
                    tag,
                    count,
                    children,
                    notes: if include_notes {
                        Some(note_paths)
                    } else {
                        None
                    },
                }
            })
            .collect();

        // Build parent-child relationships
        let all_tags: Vec<String> = tags.iter().map(|t| t.tag.clone()).collect();
        for tag_info in &mut tags {
            tag_info.children = find_direct_children(&tag_info.tag, &all_tags);
        }

        tags.sort_by(|a, b| a.tag.cmp(&b.tag));

        Ok(GetTagsResponse { tags })
    }

    pub async fn vault_info(&self, folder: Option<&VaultPath>) -> Result<VaultInfo, CoreError> {
        // Use index if available
        if let Some(index) = &self.index {
            return self.vault_info_indexed(index.as_ref(), folder).await;
        }

        // Fall back to full-scan
        let files = self.collect_md_files(folder).await?;
        let mut all_tags = std::collections::HashSet::new();

        for file_path in &files {
            match self.read_and_parse(file_path).await {
                Ok((note, _)) => {
                    all_tags.extend(note.tags().into_iter().map(String::from));
                }
                Err(e) => {
                    warn!(path = %file_path, error = %e, "skipping note in vault_info");
                }
            }
        }

        let name = self
            .vault_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "vault".to_string());

        Ok(VaultInfo {
            name,
            root_path: self.vault_path.clone(),
            folder: folder.cloned(),
            note_count: files.len(),
            tag_count: all_tags.len(),
            storage_type: "local".to_string(),
        })
    }

    pub async fn vault_tags(
        &self,
        folder: Option<&VaultPath>,
    ) -> Result<VaultTagsResponse, CoreError> {
        // Use index if available
        if let Some(index) = &self.index {
            return self.vault_tags_indexed(index.as_ref(), folder).await;
        }

        // Fall back to full-scan
        let files = self.collect_md_files(folder).await?;
        let mut tag_counts: HashMap<String, usize> = HashMap::new();

        for file_path in &files {
            match self.read_and_parse(file_path).await {
                Ok((note, _)) => {
                    for tag in note.tags() {
                        *tag_counts.entry(tag.to_string()).or_default() += 1;
                    }
                }
                Err(e) => {
                    warn!(path = %file_path, error = %e, "skipping note in vault_tags");
                }
            }
        }

        let all_tags: Vec<String> = tag_counts.keys().cloned().collect();
        let mut tags: Vec<VaultTagInfo> = tag_counts
            .into_iter()
            .map(|(tag, count)| {
                let children = find_direct_children(&tag, &all_tags);
                VaultTagInfo {
                    tag,
                    count,
                    children,
                }
            })
            .collect();

        tags.sort_by(|a, b| a.tag.cmp(&b.tag));

        Ok(VaultTagsResponse {
            folder: folder.cloned(),
            tags,
        })
    }

    pub async fn vault_folders(
        &self,
        folder: Option<&VaultPath>,
    ) -> Result<VaultFoldersResponse, CoreError> {
        let files = self.collect_md_files(folder).await?;
        let mut folder_counts: HashMap<VaultPath, usize> = HashMap::new();

        for file_path in &files {
            if let Some(parent) = file_path.parent() {
                *folder_counts.entry(parent).or_default() += 1;
            }
        }

        let mut folders: Vec<FolderInfo> = folder_counts
            .into_iter()
            .map(|(path, note_count)| FolderInfo { path, note_count })
            .collect();

        folders.sort_by(|a, b| a.path.cmp(&b.path));

        Ok(VaultFoldersResponse {
            folder: folder.cloned(),
            folders,
        })
    }

    pub async fn note_resource(&self, path: &str) -> Result<NoteResourceResponse, CoreError> {
        let file_path: VaultPath = path.try_into().map_err(StorageError::from)?;
        let (note, revision) = self.read_and_parse(&file_path).await?;

        let tags: Vec<String> = note.tags().into_iter().map(String::from).collect();

        Ok(NoteResourceResponse {
            path: path.to_string(),
            title: note.title.clone(),
            revision,
            frontmatter: note.frontmatter.clone(),
            content: note.to_string(),
            word_count: note.word_count(),
            tags,
        })
    }
}
