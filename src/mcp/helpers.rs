use super::types::{McpResult, mcp_invalid_params};
use crate::common::VaultPath;

/// Parse an optional folder string into a validated `VaultPath`.
pub fn parse_folder(folder: Option<&str>) -> McpResult<Option<VaultPath>> {
    folder
        .map(|f| {
            let normalized = format!("{}/", f.trim_end_matches('/'));
            VaultPath::new(normalized).map_err(mcp_invalid_params)
        })
        .transpose()
}
