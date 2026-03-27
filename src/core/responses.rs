use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::common::{RevisionToken, VaultPath};
use crate::note_handler::Frontmatter;

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
    pub token_count: usize,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReplaceMode {
    First,
    All,
    Regex,
}

#[derive(Debug, Serialize)]
pub struct WriteNoteResponse {
    pub path: String,
    pub revision: RevisionToken,
}

#[derive(Debug, Serialize)]
pub struct NoteResourceResponse {
    pub path: String,
    pub title: Option<String>,
    pub revision: RevisionToken,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontmatter: Option<Frontmatter>,
    pub content: String,
    pub token_count: usize,
    pub tags: Vec<String>,
}
