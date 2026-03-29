use serde::{Deserialize, Serialize};

use crate::common::VaultPath;

/// Options for search queries.
#[derive(Debug, Clone, Default)]
pub struct SearchOptions {
    /// Limit results to notes under this folder.
    pub folder: Option<VaultPath>,
    /// Limit results to notes with all of these tags.
    pub tags: Option<Vec<String>>,
    /// Maximum number of results to return.
    pub limit: usize,
    /// Number of results to skip (for pagination).
    pub offset: usize,
}

/// A single search hit from the index.
#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    /// Path to the matching note.
    pub path: VaultPath,
    /// BM25 relevance score.
    pub score: f32,
    /// Heading paths of matching sections.
    pub sections: Vec<Vec<String>>,
}

/// Search response with hits and total count.
#[derive(Debug, Serialize)]
pub struct CoreSearchResponse {
    /// Matching notes (deduplicated from sections).
    pub hits: Vec<SearchHit>,
    /// Total number of matching notes (before pagination).
    pub total: usize,
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
