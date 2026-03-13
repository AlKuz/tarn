use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use thiserror::Error;
use tokio_stream::StreamExt;
use tracing::warn;

use crate::common::RevisionToken;
use crate::core::builder::TarnCore;
use crate::parser::{Frontmatter, Link, Note};
use crate::storage::{FileContent, Storage, StorageError};

#[derive(Debug, Error)]
pub enum CoreError {
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error("note not found: {0}")]
    NoteNotFound(PathBuf),
    #[error("not a markdown file: {0}")]
    NotMarkdown(PathBuf),
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
    pub folder: String,
    pub note_count: usize,
    pub tag_count: usize,
    pub storage_type: String,
    pub root_path: String,
}

#[derive(Debug, Serialize)]
pub struct VaultTagInfo {
    pub tag: String,
    pub count: usize,
    pub children: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct VaultTagsResponse {
    pub folder: String,
    pub tags: Vec<VaultTagInfo>,
}

#[derive(Debug, Serialize)]
pub struct FolderInfo {
    pub path: String,
    pub note_count: usize,
}

#[derive(Debug, Serialize)]
pub struct VaultFoldersResponse {
    pub folder: String,
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

fn is_markdown(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "md")
}

fn is_in_folder(path: &Path, folder: Option<&str>) -> bool {
    match folder {
        None | Some("") | Some("/") => true,
        Some(f) => {
            let f = f.strip_prefix('/').unwrap_or(f);
            path.starts_with(f)
        }
    }
}

fn in_folder_non_recursive(path: &Path, folder: Option<&str>) -> bool {
    let expected_parent = match folder {
        None | Some("") | Some("/") => Path::new(""),
        Some(f) => Path::new(f.strip_prefix('/').unwrap_or(f)),
    };
    path.parent() == Some(expected_parent)
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
    async fn collect_md_files(&self, folder: Option<&str>) -> Result<Vec<PathBuf>, CoreError> {
        let stream = self.storage.list().await?;
        tokio::pin!(stream);

        let mut files = Vec::new();
        while let Some(path) = stream.next().await {
            if is_markdown(&path) && is_in_folder(&path, folder) {
                files.push(path);
            }
        }
        Ok(files)
    }

    async fn read_and_parse(&self, path: &Path) -> Result<(Note, RevisionToken), CoreError> {
        let file = self.storage.read(path.to_path_buf()).await?;
        match file {
            FileContent::Markdown { content, token } => {
                let mut note = Note::from(content.as_str());
                note.path = Some(path.to_path_buf());
                Ok((note, token))
            }
            FileContent::Image { .. } => Err(CoreError::NotMarkdown(path.to_path_buf())),
        }
    }

    pub async fn read_note(
        &self,
        path: &str,
        sections: Option<&[String]>,
        include_frontmatter: bool,
        include_links: bool,
        summary: bool,
    ) -> Result<ReadNoteResponse, CoreError> {
        let file_path = PathBuf::from(path);
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
        folder: Option<&str>,
        tag_filter: Option<&[String]>,
        limit: usize,
        offset: usize,
    ) -> Result<SearchResponse, CoreError> {
        let files = self.collect_md_files(folder).await?;
        let lower_query = query.to_lowercase();
        let mut results = Vec::new();

        for file_path in &files {
            let (note, _token) = match self.read_and_parse(file_path).await {
                Ok(result) => result,
                Err(e) => {
                    warn!(path = %file_path.display(), error = %e, "skipping note in search");
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
                path: file_path.to_string_lossy().to_string(),
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
        folder: Option<&str>,
        recursive: bool,
        tag_filter: Option<&[String]>,
        sort: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<ListNotesResponse, CoreError> {
        let stream = self.storage.list().await?;
        tokio::pin!(stream);

        let mut files = Vec::new();
        while let Some(path) = stream.next().await {
            if !is_markdown(&path) {
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
                    warn!(path = %file_path.display(), error = %e, "skipping note in list");
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
                path: file_path.to_string_lossy().to_string(),
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
        let files = self.collect_md_files(None).await?;
        let mut tag_map: HashMap<String, (usize, Vec<String>)> = HashMap::new();

        for file_path in &files {
            let (note, _token) = match self.read_and_parse(file_path).await {
                Ok(result) => result,
                Err(e) => {
                    warn!(path = %file_path.display(), error = %e, "skipping note in get_tags");
                    continue;
                }
            };

            let note_path = file_path.to_string_lossy().to_string();
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

    pub async fn vault_info(&self, folder: Option<&str>) -> Result<VaultInfo, CoreError> {
        let files = self.collect_md_files(folder).await?;
        let mut all_tags = std::collections::HashSet::new();

        for file_path in &files {
            match self.read_and_parse(file_path).await {
                Ok((note, _)) => {
                    all_tags.extend(note.tags().into_iter().map(String::from));
                }
                Err(e) => {
                    warn!(path = %file_path.display(), error = %e, "skipping note in vault_info");
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
            folder: folder.unwrap_or("/").to_string(),
            note_count: files.len(),
            tag_count: all_tags.len(),
            storage_type: "local".to_string(),
            root_path: self.vault_path.to_string_lossy().to_string(),
        })
    }

    pub async fn vault_tags(&self, folder: Option<&str>) -> Result<VaultTagsResponse, CoreError> {
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
                    warn!(path = %file_path.display(), error = %e, "skipping note in vault_tags");
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
            folder: folder.unwrap_or("/").to_string(),
            tags,
        })
    }

    pub async fn vault_folders(
        &self,
        folder: Option<&str>,
    ) -> Result<VaultFoldersResponse, CoreError> {
        let files = self.collect_md_files(folder).await?;
        let mut folder_counts: HashMap<String, usize> = HashMap::new();

        for file_path in &files {
            let parent = file_path
                .parent()
                .map(|p| {
                    let s = p.to_string_lossy().to_string();
                    if s.is_empty() {
                        "/".to_string()
                    } else {
                        format!("/{s}")
                    }
                })
                .unwrap_or_else(|| "/".to_string());
            *folder_counts.entry(parent).or_default() += 1;
        }

        let mut folders: Vec<FolderInfo> = folder_counts
            .into_iter()
            .map(|(path, note_count)| FolderInfo { path, note_count })
            .collect();

        folders.sort_by(|a, b| a.path.cmp(&b.path));

        Ok(VaultFoldersResponse {
            folder: folder.unwrap_or("/").to_string(),
            folders,
        })
    }

    pub async fn note_resource(&self, path: &str) -> Result<NoteResourceResponse, CoreError> {
        let file_path = PathBuf::from(path);
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
