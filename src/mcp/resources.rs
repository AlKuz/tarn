use std::collections::HashMap;

use rmcp::model::{
    AnnotateAble, ListResourceTemplatesResult, ListResourcesResult, RawResource,
    RawResourceTemplate, ReadResourceResult, ResourceContents,
};

use super::TarnMcpServer;
use super::helpers::find_direct_children;
use super::responses::{
    FolderInfo, LinkInfo, NoteResourceResponse, SectionResourceResponse, VaultFoldersResponse,
    VaultInfo, VaultTagInfo, VaultTagsResponse,
};
use crate::TarnCore;
use crate::common::VaultPath;

fn parse_folder(folder: Option<&str>) -> Result<Option<VaultPath>, rmcp::ErrorData> {
    folder
        .map(|f| {
            let normalized = format!("{}/", f.trim_end_matches('/'));
            VaultPath::new(normalized)
                .map_err(|e| rmcp::ErrorData::invalid_params(e.to_string(), None))
        })
        .transpose()
}

fn internal_err(e: impl std::fmt::Display) -> rmcp::ErrorData {
    rmcp::ErrorData::internal_error(e.to_string(), None)
}

fn json_resource(
    uri: &str,
    value: &impl serde::Serialize,
) -> Result<ReadResourceResult, rmcp::ErrorData> {
    let json = serde_json::to_string_pretty(value).map_err(internal_err)?;
    Ok(ReadResourceResult::new(vec![
        ResourceContents::text(json, uri).with_mime_type("application/json"),
    ]))
}

impl TarnMcpServer {
    pub fn list_static_resources(&self) -> ListResourcesResult {
        ListResourcesResult {
            resources: vec![
                RawResource::new("tarn://vault/info", "Vault Info")
                    .with_description("Vault metadata: name, note count, tag count, storage type")
                    .with_mime_type("application/json")
                    .no_annotation(),
                RawResource::new("tarn://vault/tags", "Vault Tags")
                    .with_description("Tag hierarchy with counts across the vault")
                    .with_mime_type("application/json")
                    .no_annotation(),
                RawResource::new("tarn://vault/folders", "Vault Folders")
                    .with_description("Directory tree structure with note counts")
                    .with_mime_type("application/json")
                    .no_annotation(),
            ],
            next_cursor: None,
            meta: None,
        }
    }

    pub fn list_resource_templates_static(&self) -> ListResourceTemplatesResult {
        ListResourceTemplatesResult {
            resource_templates: vec![
                RawResourceTemplate::new("tarn://vault/info/{folder}", "Vault Info (folder)")
                    .with_description("Vault metadata scoped to a folder subtree")
                    .with_mime_type("application/json")
                    .no_annotation(),
                RawResourceTemplate::new("tarn://vault/tags/{folder}", "Vault Tags (folder)")
                    .with_description("Tag hierarchy scoped to a folder subtree")
                    .with_mime_type("application/json")
                    .no_annotation(),
                RawResourceTemplate::new("tarn://vault/folders/{folder}", "Vault Folders (folder)")
                    .with_description("Directory tree scoped to a folder subtree")
                    .with_mime_type("application/json")
                    .no_annotation(),
                RawResourceTemplate::new("tarn://note/{path}", "Note")
                    .with_description("Individual note content and metadata")
                    .with_mime_type("application/json")
                    .no_annotation(),
                RawResourceTemplate::new("tarn://note/{path}#{section_path}", "Note Section")
                    .with_description("Section content by heading path (e.g. Architecture/Backend)")
                    .with_mime_type("application/json")
                    .no_annotation(),
            ],
            next_cursor: None,
            meta: None,
        }
    }

    pub async fn read_resource_by_uri(
        &self,
        uri: &str,
    ) -> Result<ReadResourceResult, rmcp::ErrorData> {
        if let Some(rest) = uri.strip_prefix("tarn://") {
            if rest == "vault/info" {
                return self.read_vault_info(uri, None).await;
            }
            if let Some(folder) = rest.strip_prefix("vault/info/") {
                return self.read_vault_info(uri, Some(folder)).await;
            }
            if rest == "vault/tags" {
                return self.read_vault_tags(uri, None).await;
            }
            if let Some(folder) = rest.strip_prefix("vault/tags/") {
                return self.read_vault_tags(uri, Some(folder)).await;
            }
            if rest == "vault/folders" {
                return self.read_vault_folders(uri, None).await;
            }
            if let Some(folder) = rest.strip_prefix("vault/folders/") {
                return self.read_vault_folders(uri, Some(folder)).await;
            }
            if let Some(path) = rest.strip_prefix("note/") {
                if let Some((note_path, section_path)) = path.split_once('#') {
                    return self
                        .read_section_resource(uri, note_path, section_path)
                        .await;
                }
                return self.read_note_resource(uri, path).await;
            }
        }

        Err(rmcp::ErrorData::resource_not_found(
            format!("unknown resource: {uri}"),
            None,
        ))
    }

    async fn read_vault_info(
        &self,
        uri: &str,
        folder: Option<&str>,
    ) -> Result<ReadResourceResult, rmcp::ErrorData> {
        let folder = parse_folder(folder)?;

        let paths = self
            .core
            .list(folder.as_ref(), true)
            .await
            .map_err(internal_err)?;
        let tag_entries = self
            .core
            .tags(None, folder.as_ref())
            .await
            .map_err(internal_err)?;

        let info = VaultInfo {
            name: self.core.vault_name(),
            root_path: self.core.vault_root().to_path_buf(),
            folder,
            note_count: paths.len(),
            tag_count: tag_entries.len(),
            storage_type: "local".to_string(),
        };

        json_resource(uri, &info)
    }

    async fn read_vault_tags(
        &self,
        uri: &str,
        folder: Option<&str>,
    ) -> Result<ReadResourceResult, rmcp::ErrorData> {
        let folder = parse_folder(folder)?;

        let entries = self
            .core
            .tags(None, folder.as_ref())
            .await
            .map_err(internal_err)?;

        let all_tags: Vec<String> = entries.iter().map(|e| e.tag.clone()).collect();
        let tags: Vec<VaultTagInfo> = entries
            .into_iter()
            .map(|e| VaultTagInfo {
                children: find_direct_children(&e.tag, &all_tags),
                tag: e.tag,
                count: e.count,
            })
            .collect();

        let response = VaultTagsResponse { folder, tags };
        json_resource(uri, &response)
    }

    async fn read_vault_folders(
        &self,
        uri: &str,
        folder: Option<&str>,
    ) -> Result<ReadResourceResult, rmcp::ErrorData> {
        let folder = parse_folder(folder)?;

        let paths = self
            .core
            .list(folder.as_ref(), true)
            .await
            .map_err(internal_err)?;

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
        json_resource(uri, &response)
    }

    async fn read_section_resource(
        &self,
        uri: &str,
        note_path: &str,
        section_path: &str,
    ) -> Result<ReadResourceResult, rmcp::ErrorData> {
        let (note, revision) = self.core.read(note_path).await.map_err(internal_err)?;

        let heading_path: Vec<&str> = section_path.split('/').collect();
        let section = TarnCore::resolve_section(&note, &heading_path);

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
                    revision,
                    content: section.content.clone(),
                    tags,
                    links,
                    token_count: section.word_count(),
                };

                json_resource(uri, &response)
            }
            None => {
                let available: Vec<String> = note
                    .sections
                    .iter()
                    .filter(|s| !s.heading_path.is_empty())
                    .map(|s| s.heading_path.join("/"))
                    .collect();

                Err(rmcp::ErrorData::resource_not_found(
                    format!(
                        "section not found: '{}'. Available sections: [{}]",
                        section_path,
                        available.join(", ")
                    ),
                    None,
                ))
            }
        }
    }

    async fn read_note_resource(
        &self,
        uri: &str,
        path: &str,
    ) -> Result<ReadResourceResult, rmcp::ErrorData> {
        let (note, revision) = self.core.read(path).await.map_err(internal_err)?;

        let tags: Vec<String> = note.tags().into_iter().map(String::from).collect();

        let response = NoteResourceResponse {
            path: path.to_string(),
            title: note.title.clone(),
            revision,
            frontmatter: note.frontmatter.clone(),
            content: note.to_string(),
            token_count: note.word_count(),
            tags,
        };

        json_resource(uri, &response)
    }
}
