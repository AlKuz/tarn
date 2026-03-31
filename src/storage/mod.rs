//! Storage abstraction for vault file operations.
//!
//! This module defines the [`Storage`] trait for reading/writing vault files and
//! provides [`LocalStorage`] for filesystem-backed vaults.
//!
//! ## Symlink Handling
//!
//! [`LocalStorage`] resolves symlinks via path canonicalization. Symlinks within the
//! vault that point to locations inside the vault root are followed transparently.
//! Symlinks pointing outside the vault root are rejected to prevent directory traversal.
//!
//! ## Revision Tokens
//!
//! All write operations use [`RevisionToken`] for optimistic concurrency control.
//! A token represents the file's state at read time. Write/delete/rename operations
//! fail with [`StorageError::Conflict`] if the file was modified since the token
//! was issued.

pub mod config;
pub mod local;

pub use config::{LocalStorageConfig, StorageConfig};
pub use local::LocalStorage;

use std::future::Future;

use crate::common::{DataURI, RevisionToken, VaultPath};
use futures_core::stream::Stream;

/// Errors from storage operations.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("file not found: {0}")]
    NotFound(VaultPath),
    #[error("file {0} already exists")]
    FileAlreadyExists(VaultPath),
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

#[derive(Debug, Clone)]
pub struct FileMeta {
    pub path: VaultPath,
    pub size: u64,
    pub modified: std::time::SystemTime,
    pub revision_token: RevisionToken,
}

#[derive(Debug, Clone)]
pub enum FileContent {
    /// Markdown note content as UTF-8 string.
    Markdown(String),
    /// Image content as a data URI (base64-encoded).
    Image(DataURI),
}

#[derive(Debug)]
pub struct File {
    pub meta: FileMeta,
    pub content: FileContent,
}

/// Vault storage backend.
///
/// Implementations must provide async file operations with revision-based
/// conflict detection.
pub trait Storage: Send + Sync {
    fn list(
        &self,
        folder: &VaultPath,
    ) -> impl Future<Output = Result<impl Stream<Item = FileMeta> + Send, StorageError>> + Send;
    fn read(&self, path: &VaultPath) -> impl Future<Output = Result<File, StorageError>> + Send;
    fn write(
        &self,
        path: &VaultPath,
        data: FileContent,
        expected_token: Option<RevisionToken>,
    ) -> impl Future<Output = Result<RevisionToken, StorageError>> + Send;
    fn delete(
        &self,
        path: &VaultPath,
        expected_token: RevisionToken,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;
    fn r#move(
        &self,
        from: &VaultPath,
        to: &VaultPath,
        expected_token: RevisionToken,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;
    fn copy(
        &self,
        from: &VaultPath,
        to: &VaultPath,
    ) -> impl Future<Output = Result<RevisionToken, StorageError>> + Send;
    fn exists(&self, path: &VaultPath) -> impl Future<Output = Result<bool, StorageError>> + Send;
    fn deny_access(&self, paths: &[VaultPath]);
    fn read_only_access(&self, paths: &[VaultPath]);
}
