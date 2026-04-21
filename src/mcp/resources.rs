use std::collections::HashMap;

use rmcp::model::ReadResourceResult;

use super::TarnMcpServer;
use super::helpers::parse_folder;
use super::types::{
    FolderInfo, LinkInfo, McpResult, NoteResourceResponse, SectionResourceResponse, TagInfo,
    VaultFoldersResponse, VaultInfo, VaultTagsResponse, mcp_err, mcp_not_found, resource_json,
};
use crate::TarnCore;
use crate::common::VaultPath;
use crate::index::Index;
use crate::index::find_direct_children;
use crate::observer::Observer;
use crate::revisions::RevisionTracker;
use crate::storage::{FileContent, Storage};

impl<S, I, O, R> TarnMcpServer<S, I, O, R>
where
    S: Storage + Send + Sync + 'static,
    I: Index + Send + Sync + 'static,
    O: Observer + Send + Sync + 'static,
    R: RevisionTracker + Send + Sync + 'static,
{
    pub(crate) async fn read_vault_info(
        &self,
        uri: &str,
        folder: Option<&str>,
    ) -> McpResult<ReadResourceResult> {
        let folder = parse_folder(folder)?;
        let list_path = folder.as_ref().unwrap_or(&VaultPath::Root);

        let paths = self.core.list_paths(list_path).await.map_err(mcp_err)?;
        let tag_counts = self
            .core
            .list_tags(None, folder.as_ref())
            .await
            .map_err(mcp_err)?;

        let info = VaultInfo {
            name: self.core.vault_name().to_string(),
            folder,
            note_count: paths.len(),
            tag_count: tag_counts.len(),
            storage_type: "local".to_string(),
        };

        resource_json(uri, &info)
    }

    pub(crate) async fn read_vault_tags(
        &self,
        uri: &str,
        folder: Option<&str>,
    ) -> McpResult<ReadResourceResult> {
        let folder = parse_folder(folder)?;

        let tag_counts = self
            .core
            .list_tags(None, folder.as_ref())
            .await
            .map_err(mcp_err)?;

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

        let response = VaultTagsResponse { folder, tags };
        resource_json(uri, &response)
    }

    pub(crate) async fn read_vault_folders(
        &self,
        uri: &str,
        folder: Option<&str>,
    ) -> McpResult<ReadResourceResult> {
        let folder = parse_folder(folder)?;
        let list_path = folder.as_ref().unwrap_or(&VaultPath::Root);

        let paths = self.core.list_paths(list_path).await.map_err(mcp_err)?;

        let mut folder_counts: HashMap<VaultPath, usize> = HashMap::new();
        for path in &paths {
            if let Some(parent) = path.parent() {
                *folder_counts.entry(parent).or_default() += 1;
            }
        }

        let mut folders: Vec<FolderInfo> = folder_counts
            .into_iter()
            .map(|(path, note_count)| FolderInfo { path, note_count })
            .collect();
        folders.sort_by(|a, b| a.path.cmp(&b.path));

        let response = VaultFoldersResponse { folder, folders };
        resource_json(uri, &response)
    }

    pub(crate) async fn read_section_resource(
        &self,
        uri: &str,
        note_path: &str,
        section_path: &str,
    ) -> McpResult<ReadResourceResult> {
        let vault_path =
            VaultPath::new(note_path).map_err(|e| mcp_not_found(format!("invalid path: {e}")))?;
        let file = self.core.read(&vault_path).await.map_err(mcp_err)?;
        let content = match file.content {
            FileContent::Markdown(c) => c,
            _ => return Err(mcp_err("not a markdown file")),
        };
        let note = self.core.parse_content(&content).map_err(mcp_err)?;

        let heading_path: Vec<&str> = section_path.split('/').collect();
        let section = TarnCore::<S, I, O, R>::resolve_section(&note, &heading_path);

        match section {
            Some(section) => {
                // Combine frontmatter tags with section inline tags
                let mut tags: Vec<String> = note
                    .frontmatter
                    .as_ref()
                    .map(|fm| fm.tags.clone())
                    .unwrap_or_default();
                tags.extend(section.tags.iter().map(|t| t.name().to_string()));
                tags.sort();
                tags.dedup();

                let links: Vec<LinkInfo> = section.links.iter().map(LinkInfo::from).collect();

                let response = SectionResourceResponse {
                    path: format!("{}#{}", note_path, section_path),
                    note_path: note_path.to_string(),
                    heading_path: section.heading_path.clone(),
                    content: section.content.clone(),
                    tags,
                    links,
                    token_count: section.word_count(),
                };

                resource_json(uri, &response)
            }
            None => {
                let available: Vec<String> = note
                    .sections
                    .iter()
                    .filter(|s| !s.heading_path.is_empty())
                    .map(|s| s.heading_path.join("/"))
                    .collect();

                Err(mcp_not_found(format!(
                    "section not found: '{}'. Available sections: [{}]",
                    section_path,
                    available.join(", ")
                )))
            }
        }
    }

    pub(crate) async fn read_note_resource(
        &self,
        uri: &str,
        path: &str,
    ) -> McpResult<ReadResourceResult> {
        let vault_path =
            VaultPath::new(path).map_err(|e| mcp_not_found(format!("invalid path: {e}")))?;
        let file = self.core.read(&vault_path).await.map_err(mcp_err)?;
        let content = match file.content {
            FileContent::Markdown(c) => c,
            _ => return Err(mcp_err("not a markdown file")),
        };
        let note = self.core.parse_content(&content).map_err(mcp_err)?;

        let tags: Vec<String> = note.tags().into_iter().map(String::from).collect();

        let response = NoteResourceResponse {
            path: path.to_string(),
            title: note.title.clone(),
            frontmatter: note.frontmatter.clone(),
            content: note.to_string(),
            token_count: note.word_count(),
            tags,
        };

        resource_json(uri, &response)
    }
}
