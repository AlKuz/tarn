pub mod config;
mod errors;
pub mod in_memory;

pub use config::{InMemoryRevisionTrackerConfig, RevisionTrackerConfig};
pub use errors::RevisionTrackerError;
pub use in_memory::InMemoryRevisionTracker;

use std::future::Future;

use crate::common::{RevisionToken, VaultPath};

/// Tracks revision tokens for vault paths, enabling server-side conflict detection.
///
/// The revision tracker maintains a mapping of vault paths to their last-known
/// revision tokens. This allows the server to detect stale writes without
/// hitting the filesystem, and to track external changes via observer events.
pub trait RevisionTracker: Send + Sync {
    /// Get the stored revision token for a path, if any.
    fn get_revision(&self, path: &VaultPath) -> impl Future<Output = Option<RevisionToken>> + Send;

    /// Store or update the revision token for a path.
    fn update_revision(
        &self,
        path: &VaultPath,
        token: RevisionToken,
    ) -> impl Future<Output = ()> + Send;

    /// Remove the revision token for a path.
    fn remove_revision(&self, path: &VaultPath) -> impl Future<Output = ()> + Send;

    /// Check if the provided token matches the stored one.
    ///
    /// Returns `true` if:
    /// - No revision is stored for this path (unknown = no conflict from tracker)
    /// - The stored revision matches the provided token
    ///
    /// Returns `false` only when a stored revision exists and doesn't match.
    /// The caller (TarnCore) decides per-operation policy for the "no stored revision" case.
    fn validate_revision(
        &self,
        path: &VaultPath,
        token: &RevisionToken,
    ) -> impl Future<Output = bool> + Send;

    /// Return all tracked paths and their revision tokens.
    ///
    /// Used for startup validation to reconcile tracker state with storage.
    fn all_revisions(&self) -> impl Future<Output = Vec<(VaultPath, RevisionToken)>> + Send;
}
