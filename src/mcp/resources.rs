use rmcp::model::{
    AnnotateAble, ListResourceTemplatesResult, ListResourcesResult, RawResource,
    RawResourceTemplate, ReadResourceResult, ResourceContents,
};

use crate::common::VaultPath;

use super::TarnMcpServer;

fn parse_folder(folder: Option<&str>) -> Result<Option<VaultPath>, rmcp::ErrorData> {
    folder
        .map(|f| {
            let normalized = format!("{}/", f.trim_end_matches('/'));
            VaultPath::new(normalized)
                .map_err(|e| rmcp::ErrorData::invalid_params(e.to_string(), None))
        })
        .transpose()
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
        let info = self
            .core
            .vault_info(folder.as_ref())
            .await
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;

        let json = serde_json::to_string_pretty(&info)
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;

        Ok(ReadResourceResult::new(vec![
            ResourceContents::text(json, uri).with_mime_type("application/json"),
        ]))
    }

    async fn read_vault_tags(
        &self,
        uri: &str,
        folder: Option<&str>,
    ) -> Result<ReadResourceResult, rmcp::ErrorData> {
        let folder = parse_folder(folder)?;
        let tags = self
            .core
            .vault_tags(folder.as_ref())
            .await
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;

        let json = serde_json::to_string_pretty(&tags)
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;

        Ok(ReadResourceResult::new(vec![
            ResourceContents::text(json, uri).with_mime_type("application/json"),
        ]))
    }

    async fn read_vault_folders(
        &self,
        uri: &str,
        folder: Option<&str>,
    ) -> Result<ReadResourceResult, rmcp::ErrorData> {
        let folder = parse_folder(folder)?;
        let folders = self
            .core
            .vault_folders(folder.as_ref())
            .await
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;

        let json = serde_json::to_string_pretty(&folders)
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;

        Ok(ReadResourceResult::new(vec![
            ResourceContents::text(json, uri).with_mime_type("application/json"),
        ]))
    }

    async fn read_note_resource(
        &self,
        uri: &str,
        path: &str,
    ) -> Result<ReadResourceResult, rmcp::ErrorData> {
        let note = self
            .core
            .note_resource(path)
            .await
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;

        let json = serde_json::to_string_pretty(&note)
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;

        Ok(ReadResourceResult::new(vec![
            ResourceContents::text(json, uri).with_mime_type("application/json"),
        ]))
    }
}
