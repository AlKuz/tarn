//! MCP response types.

use serde::Serialize;

use crate::common::VaultPath;
use crate::note_handler::{Frontmatter, Link};

#[derive(Debug, Serialize)]
pub struct TagInfo {
    pub tag: String,
    pub count: usize,
    pub children: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct GetTagsResponse {
    pub tags: Vec<TagInfo>,
}

#[derive(Debug, Serialize)]
pub struct VaultInfo {
    pub name: String,
    pub folder: Option<VaultPath>,
    pub note_count: usize,
    pub tag_count: usize,
    pub storage_type: String,
}

#[derive(Debug, Serialize)]
pub struct VaultTagsResponse {
    pub folder: Option<VaultPath>,
    pub tags: Vec<TagInfo>,
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
pub struct WriteNoteResponse {
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct DeleteNoteResponse {
    pub path: String,
    pub deleted: bool,
}

#[derive(Debug, Serialize)]
pub struct RenameNoteResponse {
    pub old_path: String,
    pub new_path: String,
    pub links_updated: usize,
}

#[derive(Debug, Serialize)]
pub struct NoteResourceResponse {
    pub path: String,
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontmatter: Option<Frontmatter>,
    pub content: String,
    pub token_count: usize,
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SectionResourceResponse {
    pub path: String,
    pub note_path: String,
    pub heading_path: Vec<String>,
    pub content: String,
    pub tags: Vec<String>,
    pub links: Vec<LinkInfo>,
    pub token_count: usize,
}

#[derive(Debug, Serialize)]
pub struct LinkInfo {
    pub link_type: String,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display: Option<String>,
}

impl From<&Link> for LinkInfo {
    fn from(link: &Link) -> Self {
        match link {
            Link::Wiki(w) => LinkInfo {
                link_type: "wiki".to_string(),
                target: w.target.clone(),
                display: w.alias.clone(),
            },
            Link::Markdown(m) => LinkInfo {
                link_type: "markdown".to_string(),
                target: m.url.clone(),
                display: Some(m.text.clone()),
            },
            Link::Url(u) => LinkInfo {
                link_type: "url".to_string(),
                target: u.url.clone(),
                display: None,
            },
            Link::Email(e) => LinkInfo {
                link_type: "email".to_string(),
                target: e.address.clone(),
                display: None,
            },
        }
    }
}
