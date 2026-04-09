//! Index module for persisting parsed note metadata.
//!
//! The index enables fast queries by AI agents without re-parsing notes on every operation.
//! It supports multiple backends: InMemoryStore, SqliteStore, DynamoDbStore, PostgresStore.

pub mod config;
pub mod errors;
pub mod in_memory;
pub mod types;

pub use config::{InMemoryIndexConfig, IndexBuildError, IndexConfig, default_persistence_path};
pub use errors::IndexError;
pub use in_memory::InMemoryIndex;
pub use types::{IndexEntry, IndexLink, IndexMeta, NoteResult, SectionResult};

use std::future::Future;

use crate::common::VaultPath;
use crate::note_handler::Note;

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
pub trait Index: Send + Sync {
    // -------------------------------------------------------------------------
    // CRUD operations
    // -------------------------------------------------------------------------

    /// Get all indexed sections for a note.
    fn get(
        &self,
        path: &VaultPath,
    ) -> impl Future<Output = Result<Vec<IndexEntry>, IndexError>> + Send;

    /// Update the index with a note's sections.
    ///
    /// Extracts all sections from the note and replaces any existing entries
    /// for that note path.
    fn update(&self, note: &Note) -> impl Future<Output = Result<(), IndexError>> + Send;

    /// Remove all indexed sections for a note.
    fn remove(&self, path: &VaultPath) -> impl Future<Output = Result<(), IndexError>> + Send;

    // -------------------------------------------------------------------------
    // Search operations
    // -------------------------------------------------------------------------

    /// Search for notes matching a query string.
    ///
    /// Returns notes grouped by path with BM25 relevance scores, filtered by
    /// folders and tags. The `limit` caps the number of sections before grouping.
    /// The `token_limit` caps total tokens across all results.
    /// The `score_threshold` filters out sections with scores below the value.
    fn search(
        &self,
        query: &str,
        folders: &[VaultPath],
        tags: &[String],
        limit: usize,
        token_limit: Option<usize>,
        score_threshold: f32,
    ) -> impl Future<Output = Result<Vec<NoteResult>, IndexError>> + Send;

    /// List all notes, optionally filtered by folder.
    ///
    /// If `recursive` is true, includes notes from all subdirectories.
    /// If false, only includes notes directly in the folder.
    fn list(
        &self,
        folder: Option<&VaultPath>,
        recursive: bool,
    ) -> impl Future<Output = Result<Vec<NoteResult>, IndexError>> + Send;

    // -------------------------------------------------------------------------
    // Graph queries
    // -------------------------------------------------------------------------

    /// Find all notes that link to the given target.
    ///
    /// The `target` is matched against wiki link targets (e.g., `[[target]]`).
    fn backlinks(
        &self,
        target: &str,
    ) -> impl Future<Output = Result<Vec<NoteResult>, IndexError>> + Send;

    /// Get all links from sections of a note.
    fn forward_links(
        &self,
        path: &VaultPath,
    ) -> impl Future<Output = Result<Vec<IndexLink>, IndexError>> + Send;

    // -------------------------------------------------------------------------
    // Metadata operations
    // -------------------------------------------------------------------------

    /// Get index metadata.
    fn meta(&self) -> impl Future<Output = Result<IndexMeta, IndexError>> + Send;

    /// Update index metadata.
    fn set_meta(&self, meta: IndexMeta) -> impl Future<Output = Result<(), IndexError>> + Send;

    // -------------------------------------------------------------------------
    // Bulk operations
    // -------------------------------------------------------------------------

    /// Clear all indexed data.
    fn clear(&self) -> impl Future<Output = Result<(), IndexError>> + Send;

    /// Count the total number of indexed sections.
    fn count(&self) -> impl Future<Output = Result<usize, IndexError>> + Send;

    /// Update the index with multiple notes.
    ///
    /// More efficient than calling `update` repeatedly for bulk operations.
    fn update_bulk(&self, notes: &[Note]) -> impl Future<Output = Result<(), IndexError>> + Send;

    /// Remove indexed sections for multiple notes.
    fn remove_bulk(
        &self,
        paths: &[VaultPath],
    ) -> impl Future<Output = Result<(), IndexError>> + Send;
}

/// Find direct children of a parent tag in a tag hierarchy.
///
/// A child tag is one level deeper than `parent` (e.g., `parent/child` but not
/// `parent/child/grandchild`).
pub fn find_direct_children(parent: &str, all_tags: &[String]) -> Vec<String> {
    all_tags
        .iter()
        .filter(|other| {
            other.starts_with(parent)
                && other.len() > parent.len()
                && other.as_bytes().get(parent.len()) == Some(&b'/')
                && !other[parent.len() + 1..].contains('/')
        })
        .cloned()
        .collect()
}
