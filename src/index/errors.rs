//! Index error types.

use thiserror::Error;

use crate::common::VaultPath;

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
