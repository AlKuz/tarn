//! Storage abstraction for vault file operations.
//!
//! This module defines the [`Storage`] trait for reading/writing vault files and
//! provides [`LocalStorage`] for filesystem-backed vaults.
//!
//! ## Symlink Handling
//!
//! [`LocalStorage`] follows symlinks transparently - symlinked files and directories
//! are accessed as if they were regular files. However, symlinks pointing outside
//! the vault root are rejected by path validation to prevent directory traversal.
//!
//! ## Revision Tokens
//!
//! All write operations use [`RevisionToken`] for optimistic concurrency control.
//! A token represents the file's state at read time. Write/delete/rename operations
//! fail with [`StorageError::Conflict`] if the file was modified since the token
//! was issued.

pub mod local;

pub use local::LocalStorage;

use crate::common::{DataURI, RevisionToken, VaultPath};
use futures_core::stream::Stream;

/// Errors from storage operations.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("file not found: {0}")]
    NotFound(VaultPath),
    #[error("permission denied: {0}")]
    PermissionDenied(VaultPath),
    #[error("write conflict: {0} (expected: {1}, actual: {2})")]
    Conflict(VaultPath, RevisionToken, RevisionToken),
    #[error("IO error on {0}: {1}")]
    Io(VaultPath, std::io::Error),
    #[error("invalid data at {0}: {1}")]
    InvalidData(VaultPath, String),
    #[error("invalid path: {0}")]
    InvalidPath(#[from] crate::common::VaultPathError),
}

/// File content with type-specific payload and revision token.
pub enum FileContent {
    /// Markdown note content as UTF-8 string.
    Markdown {
        content: String,
        token: RevisionToken,
    },
    /// Image content as a data URI (base64-encoded).
    Image {
        content: DataURI,
        token: RevisionToken,
    },
}

/// Vault storage backend.
///
/// Implementations must provide async file operations with revision-based
/// conflict detection. The trait is object-safe for dynamic dispatch.
#[allow(async_fn_in_trait)]
pub trait Storage {
    async fn list(&self) -> Result<impl Stream<Item = VaultPath>, StorageError>;
    async fn read(&self, path: &VaultPath) -> Result<FileContent, StorageError>;
    async fn write(
        &self,
        path: &VaultPath,
        data: FileContent,
    ) -> Result<RevisionToken, StorageError>;
    async fn delete(
        &self,
        path: &VaultPath,
        expected_token: RevisionToken,
    ) -> Result<(), StorageError>;
    async fn rename(
        &self,
        from: &VaultPath,
        to: &VaultPath,
        expected_token: RevisionToken,
    ) -> Result<(), StorageError>;
    async fn copy(&self, from: &VaultPath, to: &VaultPath) -> Result<RevisionToken, StorageError>;
    async fn is_exists(&self, path: &VaultPath) -> Result<bool, StorageError>;
}
