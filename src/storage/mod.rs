pub mod local;

use crate::common::{DataURI, RevisionToken};
use futures_core::stream::Stream;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("file not found: {0}")]
    NotFound(PathBuf),
    #[error("permission denied: {0}")]
    PermissionDenied(PathBuf),
    #[error("write conflict: {0} (expected: {1}, actual: {2})")]
    Conflict(PathBuf, RevisionToken, RevisionToken),
    #[error("IO error on {0}: {1}")]
    Io(PathBuf, std::io::Error),
    #[error("invalid data at {0}: {1}")]
    InvalidData(PathBuf, String),
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

pub enum StorageEvent {
    Created { path: PathBuf, token: RevisionToken },
    Updated { path: PathBuf, token: RevisionToken },
    Deleted { path: PathBuf },
}

#[allow(async_fn_in_trait)]
pub trait Storage {
    async fn list(&self) -> Result<impl Stream<Item = PathBuf>, StorageError>;
    async fn read(&self, path: PathBuf) -> Result<FileContent, StorageError>;
    async fn write(&self, path: PathBuf, data: FileContent) -> Result<RevisionToken, StorageError>;
    async fn delete(
        &self,
        path: PathBuf,
        expected_token: RevisionToken,
    ) -> Result<(), StorageError>;
    async fn rename(
        &self,
        from: PathBuf,
        to: PathBuf,
        expected_token: RevisionToken,
    ) -> Result<(), StorageError>;
    async fn copy(&self, from: PathBuf, to: PathBuf) -> Result<RevisionToken, StorageError>;
    async fn is_exists(&self, path: PathBuf) -> Result<bool, StorageError>;
}

#[allow(async_fn_in_trait)]
pub trait StorageEventListener {
    async fn listen(&self) -> Result<impl Stream<Item = StorageEvent>, StorageError>;
}
