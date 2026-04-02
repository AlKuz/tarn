use serde::{Deserialize, Serialize};

use crate::common::VaultPath;

/// A single section hit from search.
#[derive(Debug, Clone, Serialize)]
pub struct SectionHit {
    /// Section path (e.g., `note.md#Goals/Q1`).
    pub path: VaultPath,
    /// RRF score from fused pipelines.
    pub score: f32,
    /// Token count of the section content.
    pub token_count: usize,
}

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
