pub mod local;

pub use local::LocalStorageObserver;

use crate::common::RevisionToken;
use futures_core::stream::Stream;
use std::path::PathBuf;

#[derive(Debug)]
pub enum StorageEvent {
    Created { path: PathBuf, token: RevisionToken },
    Updated { path: PathBuf, token: RevisionToken },
    Deleted { path: PathBuf },
}

#[derive(Debug, thiserror::Error)]
pub enum ObserverError {
    #[error("failed to start watcher for {0}: {1}")]
    WatchFailed(PathBuf, String),
}

#[allow(async_fn_in_trait)]
pub trait Observer {
    async fn observe(&self) -> Result<impl Stream<Item = StorageEvent>, ObserverError>;
}
