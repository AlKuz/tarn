use std::collections::HashMap;

use super::types::{McpResult, mcp_invalid_params};
use crate::common::VaultPath;
use crate::note_handler::{Frontmatter, FrontmatterValue};

/// Parse an optional folder string into a validated `VaultPath`.
pub fn parse_folder(folder: Option<&str>) -> McpResult<Option<VaultPath>> {
    folder
        .map(|f| {
            let normalized = format!("{}/", f.trim_end_matches('/'));
            VaultPath::new(normalized).map_err(mcp_invalid_params)
        })
        .transpose()
}

/// Convert a JSON object to a `Frontmatter` struct.
pub fn frontmatter_from_json(
    map: HashMap<String, serde_json::Value>,
) -> Result<Frontmatter, serde_json::Error> {
    serde_json::from_value(serde_json::Value::Object(map.into_iter().collect()))
}

/// Convert a JSON object to a map of `FrontmatterValue`s.
pub fn frontmatter_values_from_json(
    map: HashMap<String, serde_json::Value>,
) -> Result<HashMap<String, FrontmatterValue>, serde_json::Error> {
    map.into_iter()
        .map(|(k, v)| serde_json::from_value(v).map(|fv| (k, fv)))
        .collect()
}
