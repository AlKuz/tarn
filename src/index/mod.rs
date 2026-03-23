//! Index module for persisting parsed note metadata.
//!
//! The index enables fast queries by AI agents without re-parsing notes on every operation.
//! It supports multiple backends: InMemoryStore, SqliteStore, DynamoDbStore, PostgresStore.

pub mod in_memory;

pub use in_memory::{InMemoryIndex, SectionId};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::common::{Configurable, RevisionToken, VaultPath};
use crate::note_handler::Note;
use std::path::PathBuf;

use crate::tokenizer::TokenizerConfig;

// ---------------------------------------------------------------------------
// Search parameters
// ---------------------------------------------------------------------------

/// Parameters for search queries.
#[derive(Debug, Clone, Default)]
pub struct SearchParams {
    /// Limit results to notes under this folder.
    pub folder: Option<VaultPath>,
    /// Limit results to notes with any of these tags.
    pub tags: Option<Vec<String>>,
    /// Maximum number of results to return.
    pub limit: usize,
    /// Number of results to skip (for pagination).
    pub offset: usize,
}

// ---------------------------------------------------------------------------
// Index link types
// ---------------------------------------------------------------------------

/// Simplified link representation for index storage.
///
/// Compared to the full `Link` enum, this drops details like heading references,
/// block refs, embed flags, and titles - keeping only what's needed for queries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IndexLink {
    /// Wiki link: `[[target]]` or `[[target|alias]]`
    Wiki {
        target: String,
        alias: Option<String>,
    },
    /// Markdown link: `[text](url)`
    Markdown { url: String, text: String },
    /// URL autolink: `<https://example.com>`
    Url { url: String },
    /// Email autolink: `<user@example.com>`
    Email { address: String },
}

// ---------------------------------------------------------------------------
// Section entry
// ---------------------------------------------------------------------------

/// An indexed section from a note.
///
/// The index is section-based - each indexed unit is a section delimited by headings.
/// This enables token-optimized retrieval for AI agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionEntry {
    /// Path to the note containing this section.
    pub note_path: VaultPath,
    /// Full heading path from root to this section.
    /// Example: `["Project Alpha", "Goals", "Q1"]` for `## Goals` under `# Project Alpha`.
    pub heading_path: Vec<String>,
    /// Tags attached to this section.
    /// Includes note frontmatter tags (attached to ALL sections) and inline tags.
    pub tags: Vec<String>,
    /// Links found in this section.
    pub links: Vec<IndexLink>,
    /// Word count of the section content.
    pub word_count: usize,
    /// Revision token for change detection.
    pub revision: RevisionToken,
}

// ---------------------------------------------------------------------------
// Index metadata
// ---------------------------------------------------------------------------

/// Metadata about the index state.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IndexMeta {
    /// Total number of indexed notes.
    pub note_count: usize,
    /// Timestamp of last indexing operation.
    pub last_indexed: Option<chrono::DateTime<chrono::Utc>>,
    /// Tokenizer configuration used for this index.
    #[serde(default)]
    pub tokenizer_config: TokenizerConfig,
}

// ---------------------------------------------------------------------------
// Index error
// ---------------------------------------------------------------------------

/// Errors that can occur during index operations.
#[derive(Debug, Error)]
pub enum IndexError {
    /// The requested note was not found in the index.
    #[error("note not found: {0}")]
    NotFound(VaultPath),
    /// The operation is not supported by this backend.
    #[error("operation not supported by this backend")]
    NotSupported,
    /// The index data is corrupted.
    #[error("index corrupted: {0}")]
    Corrupted(String),
    /// A backend-specific error occurred.
    #[error("backend error: {0}")]
    Backend(String),
}

// ---------------------------------------------------------------------------
// Index config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IndexConfig {
    InMemory {
        #[serde(default)]
        tokenizer: TokenizerConfig,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        persistence_path: Option<PathBuf>,
    },
}


// ---------------------------------------------------------------------------
// Index trait
// ---------------------------------------------------------------------------

/// Trait for indexing and querying note sections.
///
/// The index accepts `Note` objects directly, extracts all sections with their
/// heading paths, and stores them for fast retrieval. Different backends can
/// implement this trait: InMemoryStore (with JSON persistence), SqliteStore,
/// DynamoDbStore, PostgresStore.
///
/// # Design Goals
///
/// - **Token-optimized responses**: Section-level retrieval minimizes tokens
///   returned to AI agents.
/// - **Heading path navigation**: Full paths like `["Project", "Goals", "Q1"]`
///   enable precise section targeting.
/// - **BM25 keyword search**: The `search` method supports relevance-ranked results.
/// - **Graph queries**: Backlinks and forward links enable knowledge graph traversal.
#[allow(async_fn_in_trait)]
pub trait Index: Send + Sync + Configurable<Config = IndexConfig> {
    // -------------------------------------------------------------------------
    // CRUD operations
    // -------------------------------------------------------------------------

    /// Get all indexed sections for a note.
    async fn get(&self, path: &VaultPath) -> Result<Vec<SectionEntry>, IndexError>;

    /// Update the index with a note's sections.
    ///
    /// Extracts all sections from the note and replaces any existing entries
    /// for that note path.
    async fn update(&self, note: &Note) -> Result<(), IndexError>;

    /// Remove all indexed sections for a note.
    async fn remove(&self, path: &VaultPath) -> Result<(), IndexError>;

    // -------------------------------------------------------------------------
    // Search operations
    // -------------------------------------------------------------------------

    /// Search for sections matching a query string.
    ///
    /// Returns sections with BM25 relevance scores, filtered by `SearchParams`.
    async fn search(
        &self,
        query: &str,
        params: SearchParams,
    ) -> Result<Vec<(SectionEntry, f32)>, IndexError>;

    /// List all sections, optionally filtered by folder.
    ///
    /// If `recursive` is true, includes sections from all notes under the folder.
    /// If false, only includes sections from notes directly in the folder.
    async fn list(
        &self,
        folder: Option<&VaultPath>,
        recursive: bool,
    ) -> Result<Vec<SectionEntry>, IndexError>;

    // -------------------------------------------------------------------------
    // Graph queries
    // -------------------------------------------------------------------------

    /// Find all sections that link to the given target.
    ///
    /// The `target` is matched against wiki link targets (e.g., `[[target]]`).
    async fn backlinks(&self, target: &str) -> Result<Vec<SectionEntry>, IndexError>;

    /// Get all links from sections of a note.
    async fn forward_links(&self, path: &VaultPath) -> Result<Vec<IndexLink>, IndexError>;

    // -------------------------------------------------------------------------
    // Metadata operations
    // -------------------------------------------------------------------------

    /// Get index metadata.
    async fn meta(&self) -> Result<IndexMeta, IndexError>;

    /// Update index metadata.
    async fn set_meta(&self, meta: IndexMeta) -> Result<(), IndexError>;

    // -------------------------------------------------------------------------
    // Bulk operations
    // -------------------------------------------------------------------------

    /// Clear all indexed data.
    async fn clear(&self) -> Result<(), IndexError>;

    /// Count the total number of indexed sections.
    async fn count(&self) -> Result<usize, IndexError>;

    /// Update the index with multiple notes.
    ///
    /// More efficient than calling `update` repeatedly for bulk operations.
    async fn update_bulk(&self, notes: &[Note]) -> Result<(), IndexError>;

    /// Remove indexed sections for multiple notes.
    async fn remove_bulk(&self, paths: &[VaultPath]) -> Result<(), IndexError>;
}
