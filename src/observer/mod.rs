pub mod config;
pub mod local;

pub use config::ObserverConfig;
pub use local::LocalStorageObserver;

use std::future::Future;
use std::path::PathBuf;

use crate::common::{RevisionToken, VaultPath};
use futures_core::stream::Stream;

#[derive(Debug)]
pub enum StorageEvent {
    Created {
        path: VaultPath,
        token: RevisionToken,
    },
    Updated {
        path: VaultPath,
        token: RevisionToken,
    },
    Deleted {
        path: VaultPath,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum ObserverError {
    #[error("failed to start watcher for {0}: {1}")]
    WatchFailed(PathBuf, String),
}

pub trait Observer: Send + Sync {
    fn observe(
        &self,
    ) -> impl Future<Output = Result<impl Stream<Item = StorageEvent> + Send, ObserverError>> + Send;
}
