use std::path::{Path, PathBuf};

use async_stream::stream;
use futures_core::stream::Stream;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::fs;
use tokio::sync::mpsc;
use tracing::warn;

use crate::common::{RevisionToken, VaultPath};
use crate::observer::{Observer, ObserverError, StorageEvent};

pub struct LocalStorageObserver {
    path: PathBuf,
}

impl LocalStorageObserver {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

// Local filesystem token: mtime:size
// Returns None and logs warning if metadata can't be read
async fn try_revision_token(root: &Path, path: &VaultPath) -> Option<RevisionToken> {
    let full = root.join(path.as_str());
    match fs::metadata(&full).await {
        Ok(meta) => {
            let modified = meta.modified().ok()?;
            let duration = modified.duration_since(std::time::UNIX_EPOCH).ok()?;
            Some(format!("{}:{}", duration.as_nanos(), meta.len()).into())
        }
        Err(e) => {
            warn!("Failed to read metadata for {}: {}", path, e);
            None
        }
    }
}

impl Observer for LocalStorageObserver {
    async fn observe(&self) -> Result<impl Stream<Item = StorageEvent>, ObserverError> {
        // Canonicalize root to match paths from notify (which are canonical on macOS)
        let root = self
            .path
            .canonicalize()
            .unwrap_or_else(|_| self.path.clone());
        let (tx, mut rx) = mpsc::channel(256);

        let mut watcher = RecommendedWatcher::new(
            move |res: Result<notify::Event, notify::Error>| {
                if let Ok(event) = res {
                    let _ = tx.blocking_send(event);
                }
            },
            notify::Config::default(),
        )
        .map_err(|e| ObserverError::WatchFailed(self.path.clone(), e.to_string()))?;

        watcher
            .watch(&self.path, RecursiveMode::Recursive)
            .map_err(|e| ObserverError::WatchFailed(self.path.clone(), e.to_string()))?;

        Ok(stream! {
            let _watcher = watcher;

            while let Some(event) = rx.recv().await {
                let paths: Vec<VaultPath> = event
                    .paths
                    .iter()
                    .filter_map(|p: &PathBuf| p.strip_prefix(&root).ok())
                    .filter_map(|r| VaultPath::try_from(r).ok())
                    .collect();

                if paths.is_empty() {
                    continue;
                }

                match event.kind {
                    EventKind::Create(_) => {
                        for path in paths {
                            if let Some(token) = try_revision_token(&root, &path).await {
                                yield StorageEvent::Created { path, token };
                            }
                        }
                    }
                    EventKind::Modify(_) => {
                        for path in paths {
                            if let Some(token) = try_revision_token(&root, &path).await {
                                yield StorageEvent::Updated { path, token };
                            }
                        }
                    }
                    EventKind::Remove(_) => {
                        for path in paths {
                            yield StorageEvent::Deleted { path };
                        }
                    }
                    _ => {}
                }
            }
        })
    }
}
