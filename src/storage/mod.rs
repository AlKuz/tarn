pub mod local;

use std::path::PathBuf;
use futures_core::stream::Stream;
use crate::common::{DataURI, RevisionToken};

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("file not found: {0}")]
    NotFound(PathBuf),
    #[error("permission denied: {0}")]
    PermissionDenied(PathBuf),
    #[error("write conflict: {0} (expected: {1}, actual: {2})")]
    Conflict(PathBuf, RevisionToken, RevisionToken),
}

pub enum FileContent {
    Markdown(String, RevisionToken),
    Image(DataURI, RevisionToken),
}

pub enum StorageEvent {
    Created(PathBuf, RevisionToken),
    Updated(PathBuf, RevisionToken),
    Deleted(PathBuf),
    Renamed(PathBuf, PathBuf, RevisionToken),
}


pub trait Storage {
    async fn list(&self) -> Result<impl Stream<Item = PathBuf>, StorageError>;
    async fn read(&self, path: PathBuf) -> Result<FileContent, StorageError>;
    async fn write(&self, path: PathBuf, data: FileContent) -> Result<(), StorageError>;
    async fn delete(&self, path: PathBuf, expected_token: RevisionToken) -> Result<(), StorageError>;
    async fn rename(&self, from: PathBuf, to: PathBuf, expected_token: RevisionToken) -> Result<(), StorageError>;
    async fn copy(&self, from: PathBuf, to: PathBuf) -> Result<(), StorageError>;
    async fn is_exists(&self, path: PathBuf) -> Result<bool, StorageError>;
}

pub trait StorageEventListener {
    async fn listen(&self) -> impl Stream<Item = StorageEvent>;
}
