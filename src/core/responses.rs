use serde::{Deserialize, Serialize};

use crate::common::VaultPath;

/// A tag entry with count, children, and associated note paths.
#[derive(Debug, Clone, Serialize)]
pub struct TagEntry {
    pub tag: String,
    pub count: usize,
    pub children: Vec<String>,
    pub note_paths: Vec<VaultPath>,
}

/// Replacement mode for text replacement operations.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReplaceMode {
    First,
    All,
    Regex,
}
