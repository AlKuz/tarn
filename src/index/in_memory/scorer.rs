//! Unified scoring interface for search pipelines.

use std::collections::HashSet;

use crate::common::VaultPath;

/// Unified scoring interface for search pipelines.
///
/// Both `BM25Index` and `TagIndex` implement this trait, allowing the
/// RRF fusion layer to treat all scoring pipelines uniformly.
pub trait Scorer {
    /// Score candidate sections against a query string.
    ///
    /// Only sections in `candidates` are scored. Returns `(section_path, score)`
    /// pairs sorted by score descending. Sections with zero score are excluded.
    fn score(&self, query: &str, candidates: &HashSet<VaultPath>) -> Vec<(VaultPath, f32)>;
}
