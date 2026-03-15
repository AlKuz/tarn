pub mod local;

pub use local::LocalStorage;

use crate::common::{DataURI, RevisionToken, VaultPath};
use futures_core::stream::Stream;

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

pub enum FileContent {
    Markdown {
        content: String,
        token: RevisionToken,
    },
    Image {
        content: DataURI,
        token: RevisionToken,
    },
}

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
